//! Session manager for SSH tabs and runtime event projection.

use anyhow::{Context, Result, anyhow};
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, DownloadTransferEntry, SftpDirectoryEntry,
    SftpDirectoryEntryKind, SftpRemoteMetadata, SftpRuntimeHandle, SftpSessionBinding,
    SftpSessionBindingState, SftpWriteMode, TransferQueue, collect_download_targets,
    delete_entries as delete_sftp_entries, execute_queued_transfers,
    execute_queued_transfers_with_progress, move_entry_between_directories,
};
use crate::app::ssh::connection_progress::{
    ConnectionAttemptState, ConnectionDiagnosticLine, ConnectionHeadlineState,
    ConnectionHostKeyPrompt, ConnectionProgressEvent, ConnectionStepState, ConnectionStepStateItem,
};
use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput, TerminalShellIntegrationState,
    TerminalSurfaceSignature, TerminalSurfaceState, UnknownHostKeyError,
    ZmodemDownloadConflictPolicy, ZmodemTransferState,
};
use crate::theme::{ThemeMode, ThemeVariant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenSessionMode {
    ActivateExisting,
    ForceNewTab,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Connecting,
    WaitingUser,
    Connected,
    Cancelled,
    Disconnected,
    Error(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnhancedSessionState {
    Plain,
    Enhanced,
    Fallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnhancementCacheKey {
    pub user: String,
    pub host: String,
    pub port: u16,
    pub shell: String,
}

impl EnhancementCacheKey {
    fn new(user: &str, host: &str, port: u16, shell: &str) -> Self {
        Self {
            user: user.trim().to_string(),
            host: host.trim().to_ascii_lowercase(),
            port,
            shell: shell.trim().to_ascii_lowercase(),
        }
    }

    fn from_profile(profile: &ConnectionProfile, shell: &str) -> Self {
        Self::new(
            profile.user.as_str(),
            profile.host.as_str(),
            profile.port,
            shell,
        )
    }

    fn matches_profile(&self, profile: &ConnectionProfile) -> bool {
        self.user == profile.user
            && self.host == profile.host.trim().to_ascii_lowercase()
            && self.port == profile.port
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnhancementPolicy {
    AutoTry,
    SkipAutoBootstrap,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHandle {
    pub session_id: Uuid,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: SessionState,
    pub can_reconnect: bool,
    pub enhanced_session_state: EnhancedSessionState,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SessionRegistryDiagnosticsSnapshot {
    pub session_count: usize,
    pub open_order_count: usize,
    pub asset_session_count: usize,
    pub terminal_surface_count: usize,
    pub runtime_control_count: usize,
    pub pending_disconnect_count: usize,
    pub pending_resize_count: usize,
    pub current_working_directory_count: usize,
    pub disabled_enhancement_count: usize,
    pub sftp_binding_count: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RuntimeControlReleaseOutcome {
    pub terminal_memory_release_attempted: bool,
    pub terminal_memory_release_succeeded: bool,
    pub runtime_disconnect_attempted: bool,
    pub runtime_disconnect_succeeded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedSessionDiagnostics {
    pub removed: SessionHandle,
    pub before_registry: SessionRegistryDiagnosticsSnapshot,
    pub after_registry: SessionRegistryDiagnosticsSnapshot,
    pub runtime_control_present_before: bool,
    pub terminal_surface_present_before: bool,
    pub sftp_binding_present_before: bool,
    pub release_outcome: Option<RuntimeControlReleaseOutcome>,
}

pub trait SessionRuntimeControl: Send {
    fn disconnect(&self) -> Result<()>;
    fn release_terminal_memory(&self) -> Result<()> {
        Ok(())
    }
    fn send_text_input(&self, text: String) -> Result<()>;
    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()>;
    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()>;
    fn send_paste(&self, text: String) -> Result<()>;
    fn start_zmodem_upload(&self, _local_paths: Vec<PathBuf>) -> Result<()> {
        Err(anyhow!("session runtime does not support zmodem uploads"))
    }
    fn start_interactive_zmodem_upload(&self, _local_paths: Vec<PathBuf>) -> Result<()> {
        Err(anyhow!(
            "session runtime does not support interactive zmodem uploads"
        ))
    }
    fn remote_command_exists(&self, _command_name: String) -> Result<bool> {
        Err(anyhow!(
            "session runtime does not support remote command probes"
        ))
    }
    fn resolve_current_working_directory(&self) -> Result<Option<String>> {
        Ok(None)
    }
    fn start_zmodem_upload_to_remote_dir(
        &self,
        _local_paths: Vec<PathBuf>,
        _remote_dir: String,
    ) -> Result<()> {
        Err(anyhow!(
            "session runtime does not support zmodem exec uploads"
        ))
    }
    fn start_zmodem_download(
        &self,
        _local_dir: PathBuf,
        _conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        Err(anyhow!("session runtime does not support zmodem downloads"))
    }
    fn cancel_zmodem_transfer(&self) -> Result<()> {
        Err(anyhow!("session runtime does not support zmodem transfers"))
    }
    fn dismiss_zmodem_transfer(&self) -> Result<()> {
        Err(anyhow!("session runtime does not support zmodem transfers"))
    }
    fn selection_text_from_buffer_rows(
        &self,
        _start_row: u32,
        _start_col: u32,
        _end_row: u32,
        _end_col: u32,
    ) -> Result<Option<String>> {
        Ok(None)
    }
    fn resize(&self, rows: u32, cols: u32) -> Result<()>;
    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        Err(anyhow!(
            "session runtime does not expose terminal surface snapshots"
        ))
    }
    fn update_theme(
        &self,
        _mode: ThemeMode,
        _variant: ThemeVariant,
    ) -> Result<Option<TerminalSurfaceState>> {
        Ok(None)
    }
    fn update_theme_mode(&self, mode: ThemeMode) -> Result<Option<TerminalSurfaceState>> {
        self.update_theme(mode, ThemeVariant::PremiumDefault)
    }
    fn scroll_viewport_lines(&self, _delta: i32) -> Result<TerminalSurfaceState> {
        Err(anyhow!("session runtime does not support local scrollback"))
    }
    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        None
    }
}

type LaunchFuture =
    Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>;
type SharedSessionRuntimeControl = Arc<Mutex<Box<dyn SessionRuntimeControl>>>;

fn release_and_disconnect_runtime_control(runtime_control: SharedSessionRuntimeControl) {
    let _ = release_and_disconnect_runtime_control_with_outcome(runtime_control);
}

fn release_and_disconnect_runtime_control_with_outcome(
    runtime_control: SharedSessionRuntimeControl,
) -> RuntimeControlReleaseOutcome {
    let runtime_control = runtime_control
        .lock()
        .expect("lock session runtime control for release");
    // Drop scrollback/terminal state before the network-side graceful disconnect finishes.
    let terminal_memory_release_succeeded = runtime_control.release_terminal_memory().is_ok();
    let runtime_disconnect_succeeded = runtime_control.disconnect().is_ok();

    RuntimeControlReleaseOutcome {
        terminal_memory_release_attempted: true,
        terminal_memory_release_succeeded,
        runtime_disconnect_attempted: true,
        runtime_disconnect_succeeded,
    }
}
type ProbeFuture = Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>>;

pub trait SessionRuntimeLauncher: Send + Sync {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> LaunchFuture;

    fn probe(&self, profile: ConnectionProfile) -> ProbeFuture;
}

#[derive(Clone)]
pub struct SessionManager {
    runtime_handle: tokio::runtime::Handle,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    registry: Arc<Mutex<SessionRegistry>>,
}

impl SessionManager {
    pub fn new_with_launcher(
        runtime_handle: tokio::runtime::Handle,
        launcher: Arc<dyn SessionRuntimeLauncher>,
    ) -> Self {
        Self {
            runtime_handle,
            launcher,
            registry: Arc::new(Mutex::new(SessionRegistry::default())),
        }
    }

    pub fn runtime_handle(&self) -> tokio::runtime::Handle {
        self.runtime_handle.clone()
    }

    fn runtime_control_for_session(&self, session_id: Uuid) -> Result<SharedSessionRuntimeControl> {
        self.registry
            .lock()
            .expect("lock session registry")
            .runtime_controls
            .get(&session_id)
            .cloned()
            .ok_or_else(|| anyhow!("session runtime is not ready for `{session_id}`"))
    }

    pub fn open_session(
        &self,
        profile: ConnectionProfile,
        mode: OpenSessionMode,
    ) -> Result<SessionHandle> {
        let asset_id = profile
            .asset_id
            .clone()
            .context("session profile requires asset_id")?;

        if matches!(mode, OpenSessionMode::ActivateExisting) {
            let registry = self.registry.lock().expect("lock session registry");
            if let Some(existing_id) = registry.asset_sessions.get(&asset_id)
                && let Some(existing) = registry.sessions.get(existing_id)
            {
                return Ok(existing.clone());
            }
        }

        let session_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let display_title = {
            let registry = self.registry.lock().expect("lock session registry");
            next_session_display_title(&registry, profile.name.as_str())
        };
        let handle = SessionHandle {
            session_id,
            asset_id: asset_id.clone(),
            title: display_title,
            subtitle: format!("{}@{}:{}", profile.user, profile.host, profile.port),
            state: SessionState::Connecting,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        };

        {
            let mut registry = self.registry.lock().expect("lock session registry");
            registry.asset_sessions.insert(asset_id, session_id);
            registry.sessions.insert(session_id, handle.clone());
            registry
                .session_profiles
                .insert(session_id, profile.clone());
            registry.connection_attempts.insert(
                session_id,
                ConnectionAttemptState::with_attempt_id(
                    attempt_id,
                    ConnectionHeadlineState::Connecting,
                ),
            );
            registry.open_order.push(session_id);
        }

        self.spawn_session_attempt(session_id, profile, attempt_id);

        Ok(handle)
    }

    pub fn session(&self, session_id: Uuid) -> Option<SessionHandle> {
        self.registry
            .lock()
            .expect("lock session registry")
            .sessions
            .get(&session_id)
            .cloned()
    }

    pub fn connection_attempt(&self, session_id: Uuid) -> Option<ConnectionAttemptState> {
        self.registry
            .lock()
            .expect("lock session registry")
            .connection_attempts
            .get(&session_id)
            .cloned()
    }

    pub fn session_profile(&self, session_id: Uuid) -> Option<ConnectionProfile> {
        self.registry
            .lock()
            .expect("lock session registry")
            .session_profiles
            .get(&session_id)
            .cloned()
    }

    pub fn ordered_sessions(&self) -> Vec<SessionHandle> {
        let registry = self.registry.lock().expect("lock session registry");
        registry
            .open_order
            .iter()
            .filter_map(|session_id| registry.sessions.get(session_id).cloned())
            .collect()
    }

    pub fn diagnostics_snapshot(&self) -> SessionRegistryDiagnosticsSnapshot {
        let registry = self.registry.lock().expect("lock session registry");
        SessionRegistryDiagnosticsSnapshot::capture(&registry)
    }

    pub fn terminal_surface(&self, session_id: Uuid) -> Option<TerminalSurfaceState> {
        let should_refresh = {
            let registry = self.registry.lock().expect("lock session registry");
            terminal_surface_stale(&registry, session_id)
        };
        if should_refresh {
            refresh_runtime_surface(&self.registry, session_id);
        }

        self.registry
            .lock()
            .expect("lock session registry")
            .terminal_surfaces
            .get(&session_id)
            .cloned()
    }

    pub fn terminal_surface_signature(&self, session_id: Uuid) -> Option<TerminalSurfaceSignature> {
        let registry = self.registry.lock().expect("lock session registry");
        terminal_surface_signature_for_registry(&registry, session_id)
    }

    pub fn sftp_binding(&self, session_id: Uuid) -> Option<SftpSessionBinding> {
        self.registry
            .lock()
            .expect("lock session registry")
            .sftp_bindings
            .get(&session_id)
            .cloned()
    }

    pub fn current_working_directory(&self, session_id: Uuid) -> Option<String> {
        self.registry
            .lock()
            .expect("lock session registry")
            .current_working_directories
            .get(&session_id)
            .cloned()
    }

    pub fn resolve_current_working_directory(&self, session_id: Uuid) -> Result<Option<String>> {
        let cwd = self
            .runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for cwd probe")
            .resolve_current_working_directory()?;
        if let Some(cwd) = cwd.as_ref() {
            let mut registry = self.registry.lock().expect("lock session registry");
            if registry.sessions.contains_key(&session_id) {
                registry
                    .current_working_directories
                    .insert(session_id, cwd.clone());
            }
        }
        Ok(cwd)
    }

    pub fn zmodem_state(&self, session_id: Uuid) -> Option<ZmodemTransferState> {
        self.registry
            .lock()
            .expect("lock session registry")
            .zmodem_transfers
            .get(&session_id)
            .cloned()
    }

    pub fn remember_enhancement_fallback(&self, profile: &ConnectionProfile, shell: &str) {
        let mut registry = self.registry.lock().expect("lock session registry");
        registry
            .enhancement_fallback_cache
            .insert(EnhancementCacheKey::from_profile(profile, shell));
    }

    pub fn enhancement_policy_for(&self, profile: &ConnectionProfile) -> EnhancementPolicy {
        let registry = self.registry.lock().expect("lock session registry");
        if registry
            .enhancement_fallback_cache
            .iter()
            .any(|key| key.matches_profile(profile))
        {
            EnhancementPolicy::SkipAutoBootstrap
        } else {
            EnhancementPolicy::AutoTry
        }
    }

    pub fn disable_enhancement_for_session(&self, session_id: Uuid) -> Result<SessionHandle> {
        let mut registry = self.registry.lock().expect("lock session registry");
        registry.disabled_enhancement_sessions.insert(session_id);
        let session = registry
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("session `{session_id}` does not exist"))?;
        session.enhanced_session_state = EnhancedSessionState::Plain;
        Ok(session.clone())
    }

    pub fn disable_enhancement_for_host(
        &self,
        session_id: Uuid,
        shell: &str,
    ) -> Result<SessionHandle> {
        let mut registry = self.registry.lock().expect("lock session registry");
        let profile = registry
            .session_profiles
            .get(&session_id)
            .cloned()
            .ok_or_else(|| anyhow!("session profile is not available for `{session_id}`"))?;
        registry
            .enhancement_fallback_cache
            .insert(EnhancementCacheKey::from_profile(&profile, shell));
        registry.disabled_enhancement_sessions.insert(session_id);
        let session = registry
            .sessions
            .get_mut(&session_id)
            .ok_or_else(|| anyhow!("session `{session_id}` does not exist"))?;
        session.enhanced_session_state = EnhancedSessionState::Plain;
        Ok(session.clone())
    }

    pub fn sftp_read_dir(&self, session_id: Uuid, path: &str) -> Result<Vec<SftpDirectoryEntry>> {
        self.runtime_handle
            .block_on(self.sftp_read_dir_async(session_id, path))
    }

    pub async fn sftp_read_dir_async(
        &self,
        session_id: Uuid,
        path: &str,
    ) -> Result<Vec<SftpDirectoryEntry>> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.read_dir(path).await
    }

    pub async fn sftp_download_file_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
    ) -> Result<Vec<u8>> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.download_file(remote_path).await
    }

    pub fn sftp_download_file(&self, session_id: Uuid, remote_path: &str) -> Result<Vec<u8>> {
        self.runtime_handle
            .block_on(self.sftp_download_file_async(session_id, remote_path))
    }

    pub async fn sftp_upload_file_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
        data: Vec<u8>,
    ) -> Result<u64> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.upload_file(remote_path, data).await
    }

    pub fn sftp_upload_file(
        &self,
        session_id: Uuid,
        remote_path: &str,
        data: Vec<u8>,
    ) -> Result<u64> {
        self.runtime_handle
            .block_on(self.sftp_upload_file_async(session_id, remote_path, data))
    }

    pub async fn sftp_stat_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
    ) -> Result<SftpRemoteMetadata> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.stat(remote_path).await
    }

    pub async fn sftp_path_exists_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
    ) -> Result<bool> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.path_exists(remote_path).await
    }

    pub async fn sftp_open_file_reader_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
    ) -> Result<BoxedSftpReader> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.open_file_reader(remote_path).await
    }

    pub async fn sftp_open_file_writer_async(
        &self,
        session_id: Uuid,
        remote_path: &str,
        mode: SftpWriteMode,
    ) -> Result<BoxedSftpWriter> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.open_file_writer(remote_path, mode).await
    }

    pub async fn sftp_create_directory_async(&self, session_id: Uuid, path: &str) -> Result<()> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.mkdir(path).await
    }

    pub async fn sftp_rename_entry_async(
        &self,
        session_id: Uuid,
        from: &str,
        to: &str,
    ) -> Result<()> {
        let runtime = self.sftp_runtime(session_id)?;
        runtime.rename(from, to).await
    }

    pub async fn sftp_delete_entries_async(
        &self,
        session_id: Uuid,
        entries: Vec<SftpDirectoryEntry>,
    ) -> Result<()> {
        let runtime = self.sftp_runtime(session_id)?;
        for entry in entries {
            if entry.kind == SftpDirectoryEntryKind::Directory {
                runtime.delete_dir(entry.path.as_str()).await?;
            } else {
                runtime.delete_file(entry.path.as_str()).await?;
            }
        }
        Ok(())
    }

    pub fn sftp_execute_queued_transfers(
        &self,
        session_id: Uuid,
        queue: &mut TransferQueue,
    ) -> Result<()> {
        let runtime = self.sftp_runtime(session_id)?;
        self.runtime_handle
            .block_on(execute_queued_transfers(&runtime, queue))
    }

    pub fn sftp_execute_queued_transfers_with_progress<F>(
        &self,
        session_id: Uuid,
        queue: &mut TransferQueue,
        on_queue_updated: F,
    ) -> Result<()>
    where
        F: FnMut(&TransferQueue) -> bool,
    {
        let runtime = self.sftp_runtime(session_id)?;
        self.runtime_handle
            .block_on(execute_queued_transfers_with_progress(
                &runtime,
                queue,
                on_queue_updated,
            ))
    }

    pub fn sftp_collect_download_targets(
        &self,
        session_id: Uuid,
        local_root: &std::path::Path,
        entries: &[SftpDirectoryEntry],
    ) -> Result<Vec<DownloadTransferEntry>> {
        let runtime = self.sftp_runtime(session_id)?;
        self.runtime_handle
            .block_on(collect_download_targets(&runtime, local_root, entries))
    }

    pub fn sftp_delete_entries(
        &self,
        session_id: Uuid,
        queue: &mut TransferQueue,
        state: &mut SftpSessionBindingState,
        entry_ids: &[String],
    ) -> Result<usize> {
        let runtime = self.sftp_runtime(session_id)?;
        self.runtime_handle.block_on(delete_sftp_entries(
            &runtime,
            queue,
            session_id.to_string().as_str(),
            state,
            entry_ids,
        ))
    }

    pub fn sftp_move_entry_between_directories(
        &self,
        session_id: Uuid,
        state: &mut SftpSessionBindingState,
        entry_id: &str,
        destination_dir: &str,
    ) -> Result<bool> {
        let runtime = self.sftp_runtime(session_id)?;
        self.runtime_handle.block_on(move_entry_between_directories(
            &runtime,
            state,
            entry_id,
            destination_dir,
        ))
    }

    pub async fn probe_connection_async(&self, profile: ConnectionProfile) -> Result<()> {
        let result = self.launcher.probe(profile).await;
        match &result {
            Ok(()) => {}
            Err(error) => tracing::error!(
                target: "app.ssh",
                error = %error,
                "session manager probe failed"
            ),
        }
        result
    }

    pub fn probe_connection(&self, profile: ConnectionProfile) -> Result<()> {
        self.runtime_handle
            .block_on(self.probe_connection_async(profile))
    }

    pub fn retry_session(&self, session_id: Uuid) -> Result<SessionHandle> {
        let (profile, attempt_id, updated, runtime_control) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            let profile = registry
                .session_profiles
                .get(&session_id)
                .cloned()
                .ok_or_else(|| anyhow!("session profile is not available for `{session_id}`"))?;
            let session = registry
                .sessions
                .get_mut(&session_id)
                .ok_or_else(|| anyhow!("session `{session_id}` does not exist"))?;
            session.state = SessionState::Connecting;
            session.can_reconnect = false;
            let updated = session.clone();
            let attempt_id = Uuid::new_v4();
            registry.connection_attempts.insert(
                session_id,
                ConnectionAttemptState::with_attempt_id(
                    attempt_id,
                    ConnectionHeadlineState::Connecting,
                ),
            );
            registry.terminal_surfaces.remove(&session_id);
            registry.terminal_surface_revisions.remove(&session_id);
            let runtime_control = registry.runtime_controls.remove(&session_id);
            mark_sftp_binding_disconnected(&mut registry, session_id);
            registry.pending_disconnects.remove(&session_id);
            (profile, attempt_id, updated, runtime_control)
        };

        if let Some(runtime_control) = runtime_control {
            release_and_disconnect_runtime_control(runtime_control);
        }

        self.spawn_session_attempt(session_id, profile, attempt_id);

        Ok(updated)
    }

    pub fn cancel_connection_attempt(&self, session_id: Uuid) -> Option<SessionHandle> {
        let (updated, runtime_control) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            if let Some(attempt) = registry.connection_attempts.get_mut(&session_id) {
                finalize_connection_attempt(
                    attempt,
                    ConnectionHeadlineState::Cancelled,
                    ConnectionStepState::Cancelled,
                    "SSH connection attempt cancelled.",
                );
            }
            let session = registry.sessions.get_mut(&session_id)?;
            session.state = SessionState::Cancelled;
            session.can_reconnect = true;
            let updated = session.clone();
            let runtime_control = registry.runtime_controls.remove(&session_id);
            mark_sftp_binding_disconnected(&mut registry, session_id);
            registry.pending_disconnects.remove(&session_id);
            (updated, runtime_control)
        };

        if let Some(runtime_control) = runtime_control {
            release_and_disconnect_runtime_control(runtime_control);
        }

        Some(updated)
    }

    pub fn reject_host_key_prompt(&self, session_id: Uuid) -> Option<SessionHandle> {
        let mut registry = self.registry.lock().expect("lock session registry");
        let prompt = registry
            .connection_attempts
            .get(&session_id)
            .and_then(|attempt| attempt.prompt.clone())?;
        let message = format!(
            "Rejected unknown SSH host key for `{}`:{}.",
            prompt.host, prompt.port
        );

        if let Some(attempt) = registry.connection_attempts.get_mut(&session_id) {
            finalize_connection_attempt(
                attempt,
                ConnectionHeadlineState::Cancelled,
                ConnectionStepState::Failed,
                message,
            );
        }
        let session = registry.sessions.get_mut(&session_id)?;
        session.state = SessionState::Cancelled;
        session.can_reconnect = true;

        Some(session.clone())
    }

    pub fn disconnect_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        let (updated, runtime_control) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            let session = registry.sessions.get_mut(&session_id)?;
            session.state = SessionState::Disconnected;
            session.can_reconnect = true;
            let updated = session.clone();
            let runtime_control = registry.runtime_controls.remove(&session_id);
            mark_sftp_binding_disconnected(&mut registry, session_id);
            if runtime_control.is_none() {
                registry.pending_disconnects.insert(session_id);
            }
            (updated, runtime_control)
        };

        if let Some(runtime_control) = runtime_control {
            release_and_disconnect_runtime_control(runtime_control);
        }

        Some(updated)
    }

    pub fn send_session_text_input(&self, session_id: Uuid, text: String) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for text input")
            .send_text_input(text)
    }

    pub fn send_session_key_input(&self, session_id: Uuid, event: TerminalKeyEvent) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for key input")
            .send_key_input(event)
    }

    pub fn resize_session(&self, session_id: Uuid, rows: u32, cols: u32) -> Result<()> {
        let runtime_control = {
            let mut registry = self.registry.lock().expect("lock session registry");
            if let Some(runtime_control) = registry.runtime_controls.get(&session_id).cloned() {
                Some(runtime_control)
            } else if registry.sessions.contains_key(&session_id) {
                registry.pending_resizes.insert(session_id, (rows, cols));
                return Ok(());
            } else {
                None
            }
        };
        if let Some(runtime_control) = runtime_control {
            return runtime_control
                .lock()
                .expect("lock session runtime control for resize")
                .resize(rows, cols);
        }
        Err(anyhow!("session runtime is not ready for `{session_id}`"))
    }

    pub fn send_session_mouse_input(
        &self,
        session_id: Uuid,
        event: TerminalMouseInput,
    ) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for mouse input")
            .send_mouse_input(event)
    }

    pub fn send_session_paste(&self, session_id: Uuid, text: String) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for paste")
            .send_paste(text)
    }

    pub fn start_zmodem_upload(&self, session_id: Uuid, local_paths: Vec<PathBuf>) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for zmodem upload")
            .start_zmodem_upload(local_paths)
    }

    pub fn start_interactive_zmodem_upload(
        &self,
        session_id: Uuid,
        local_paths: Vec<PathBuf>,
    ) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for interactive zmodem upload")
            .start_interactive_zmodem_upload(local_paths)
    }

    pub fn remote_command_exists(&self, session_id: Uuid, command_name: &str) -> Result<bool> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for remote command probe")
            .remote_command_exists(command_name.to_string())
    }

    pub fn start_zmodem_upload_to_remote_dir(
        &self,
        session_id: Uuid,
        local_paths: Vec<PathBuf>,
        remote_dir: String,
    ) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for zmodem exec upload")
            .start_zmodem_upload_to_remote_dir(local_paths, remote_dir)
    }

    pub fn start_zmodem_download(
        &self,
        session_id: Uuid,
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for zmodem download")
            .start_zmodem_download(local_dir, conflict_policy)
    }

    pub fn cancel_zmodem_transfer(&self, session_id: Uuid) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for zmodem cancel")
            .cancel_zmodem_transfer()
    }

    pub fn dismiss_zmodem_transfer(&self, session_id: Uuid) -> Result<()> {
        self.runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for zmodem dismiss")
            .dismiss_zmodem_transfer()
    }

    pub fn selection_text_from_buffer_rows(
        &self,
        session_id: Uuid,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String> {
        let runtime_result = self
            .runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for selection text")
            .selection_text_from_buffer_rows(start_row, start_col, end_row, end_col);

        match runtime_result {
            Ok(Some(text)) => return Ok(text),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(
                    target: "app.terminal",
                    session_id = session_id.to_string(),
                    start_row,
                    start_col,
                    end_row,
                    end_col,
                    error = %error,
                    "failed to read selection text from terminal runtime buffer; falling back to projected surface"
                );
            }
        }

        let surface = self
            .terminal_surface(session_id)
            .ok_or_else(|| anyhow!("session terminal surface is not ready for `{session_id}`"))?;
        Ok(surface.selection_text_from_buffer_rows(start_row, start_col, end_row, end_col))
    }

    pub fn scroll_session_viewport(&self, session_id: Uuid, delta: i32) -> Result<()> {
        let surface = self
            .runtime_control_for_session(session_id)?
            .lock()
            .expect("lock session runtime control for scroll")
            .scroll_viewport_lines(delta)?;

        update_terminal_surface(&self.registry, session_id, surface);
        Ok(())
    }

    pub fn scroll_session_to_top(&self, session_id: Uuid) -> Result<()> {
        let (_, max_offset) = self.viewport_offsets(session_id)?;
        self.scroll_session_to_offset(session_id, max_offset)
    }

    pub fn scroll_session_to_bottom(&self, session_id: Uuid) -> Result<()> {
        self.scroll_session_to_offset(session_id, 0)
    }

    pub fn scroll_session_to_ratio(&self, session_id: Uuid, ratio: f32) -> Result<()> {
        let (_, max_offset) = self.viewport_offsets(session_id)?;
        let target = ((max_offset as f32) * ratio.clamp(0.0, 1.0)).round() as u32;
        self.scroll_session_to_offset(session_id, target)
    }

    fn viewport_offsets(&self, session_id: Uuid) -> Result<(u32, u32)> {
        let registry = self.registry.lock().expect("lock session registry");
        let surface = registry
            .terminal_surfaces
            .get(&session_id)
            .ok_or_else(|| anyhow!("session terminal surface is not ready for `{session_id}`"))?;
        Ok((
            surface.viewport_offset_lines,
            surface.viewport_max_offset_lines,
        ))
    }

    fn scroll_session_to_offset(&self, session_id: Uuid, target_offset: u32) -> Result<()> {
        let (current_offset, max_offset) = self.viewport_offsets(session_id)?;
        let target_offset = target_offset.min(max_offset);
        let delta = i64::from(target_offset) - i64::from(current_offset);
        let delta =
            i32::try_from(delta).context("session viewport delta exceeded supported range")?;
        if delta == 0 {
            return Ok(());
        }

        self.scroll_session_viewport(session_id, delta)
    }

    pub fn set_theme(&self, mode: ThemeMode, variant: ThemeVariant) -> Result<()> {
        let session_ids = {
            let mut registry = self.registry.lock().expect("lock session registry");
            registry.theme_mode = mode;
            registry.theme_variant = variant;
            registry
                .runtime_controls
                .keys()
                .copied()
                .collect::<Vec<_>>()
        };

        for session_id in session_ids {
            let Some(runtime_control) = ({
                let registry = self.registry.lock().expect("lock session registry");
                registry.runtime_controls.get(&session_id).cloned()
            }) else {
                continue;
            };
            let surface = runtime_control
                .lock()
                .expect("lock session runtime control for theme update")
                .update_theme(mode, variant)?;

            if let Some(surface) = surface {
                update_terminal_surface(&self.registry, session_id, surface);
            }
        }

        Ok(())
    }

    pub fn set_theme_mode(&self, mode: ThemeMode) -> Result<()> {
        self.set_theme(mode, ThemeVariant::PremiumDefault)
    }

    pub fn close_session(&self, session_id: Uuid) -> Option<SessionHandle> {
        self.close_session_with_diagnostics(session_id)
            .map(|diagnostics| diagnostics.removed)
    }

    pub fn close_session_with_diagnostics(
        &self,
        session_id: Uuid,
    ) -> Option<ClosedSessionDiagnostics> {
        let (
            removed,
            before_registry,
            after_registry,
            runtime_control_present_before,
            terminal_surface_present_before,
            sftp_binding_present_before,
            runtime_control,
        ) = {
            let mut registry = self.registry.lock().expect("lock session registry");
            let before_registry = SessionRegistryDiagnosticsSnapshot::capture(&registry);
            let removed = registry.sessions.remove(&session_id)?;
            registry
                .open_order
                .retain(|existing_id| *existing_id != session_id);
            registry.connection_attempts.remove(&session_id);
            registry.session_profiles.remove(&session_id);
            let terminal_surface_present_before =
                registry.terminal_surfaces.remove(&session_id).is_some();
            registry.current_working_directories.remove(&session_id);
            registry.zmodem_transfers.remove(&session_id);
            registry.terminal_surface_revisions.remove(&session_id);
            registry.pending_disconnects.remove(&session_id);
            registry.pending_resizes.remove(&session_id);
            registry.disabled_enhancement_sessions.remove(&session_id);
            let sftp_binding_present_before = registry.sftp_bindings.remove(&session_id).is_some();
            let runtime_control = registry.runtime_controls.remove(&session_id);
            let runtime_control_present_before = runtime_control.is_some();
            if registry.asset_sessions.get(&removed.asset_id) == Some(&session_id) {
                let replacement = registry
                    .open_order
                    .iter()
                    .rev()
                    .copied()
                    .find(|existing_id| {
                        registry
                            .sessions
                            .get(existing_id)
                            .map(|session| session.asset_id == removed.asset_id)
                            .unwrap_or(false)
                    });

                if let Some(existing_id) = replacement {
                    registry
                        .asset_sessions
                        .insert(removed.asset_id.clone(), existing_id);
                } else {
                    registry.asset_sessions.remove(&removed.asset_id);
                }
            }
            let after_registry = SessionRegistryDiagnosticsSnapshot::capture(&registry);
            (
                removed,
                before_registry,
                after_registry,
                runtime_control_present_before,
                terminal_surface_present_before,
                sftp_binding_present_before,
                runtime_control,
            )
        };

        let release_outcome =
            runtime_control.map(release_and_disconnect_runtime_control_with_outcome);

        Some(ClosedSessionDiagnostics {
            removed,
            before_registry,
            after_registry,
            runtime_control_present_before,
            terminal_surface_present_before,
            sftp_binding_present_before,
            release_outcome,
        })
    }

    fn spawn_session_attempt(
        &self,
        session_id: Uuid,
        profile: ConnectionProfile,
        attempt_id: Uuid,
    ) {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        let registry_for_events = Arc::clone(&self.registry);
        self.runtime_handle.spawn(async move {
            let mut pending_events = VecDeque::new();

            loop {
                let event = if let Some(event) = pending_events.pop_front() {
                    Some(event)
                } else {
                    event_rx.recv().await
                };
                let Some(event) = event else {
                    break;
                };

                match event {
                    SessionRuntimeEvent::SurfaceChanged(surface) => {
                        let mut backlog = VecDeque::new();
                        while let Ok(next) = event_rx.try_recv() {
                            backlog.push_back(next);
                        }
                        let (surface, remaining) = coalesce_surface_backlog(surface, backlog);
                        pending_events.extend(remaining);
                        apply_runtime_event(
                            &registry_for_events,
                            session_id,
                            attempt_id,
                            SessionRuntimeEvent::SurfaceChanged(surface),
                        );
                    }
                    SessionRuntimeEvent::SurfaceDirty => {
                        let mut backlog = VecDeque::new();
                        while let Ok(next) = event_rx.try_recv() {
                            backlog.push_back(next);
                        }
                        pending_events.extend(coalesce_surface_dirty_backlog(backlog));
                        apply_runtime_event(
                            &registry_for_events,
                            session_id,
                            attempt_id,
                            SessionRuntimeEvent::SurfaceDirty,
                        );
                    }
                    other => {
                        apply_runtime_event(&registry_for_events, session_id, attempt_id, other)
                    }
                }
            }
        });

        let launcher = Arc::clone(&self.launcher);
        let registry_for_launch = Arc::clone(&self.registry);
        self.runtime_handle.spawn(async move {
            match launcher
                .launch(profile, session_id, attempt_id, event_tx)
                .await
            {
                Ok(runtime_control) => {
                    attach_runtime_control(
                        &registry_for_launch,
                        session_id,
                        attempt_id,
                        runtime_control,
                    );
                }
                Err(error) => {
                    if let Some(unknown) = error.downcast_ref::<UnknownHostKeyError>() {
                        apply_unknown_host_key_prompt(
                            &registry_for_launch,
                            session_id,
                            attempt_id,
                            unknown,
                        );
                        return;
                    }

                    if !current_attempt_matches(&registry_for_launch, session_id, attempt_id) {
                        return;
                    }
                    project_connection_attempt_error(
                        &registry_for_launch,
                        session_id,
                        error.to_string(),
                    );
                    update_session(
                        &registry_for_launch,
                        session_id,
                        SessionState::Error(error.to_string()),
                        true,
                    );
                }
            }
        });
    }
}

impl SessionManager {
    fn sftp_runtime(&self, session_id: Uuid) -> Result<SftpRuntimeHandle> {
        self.sftp_binding(session_id)
            .and_then(|binding| binding.runtime())
            .ok_or_else(|| anyhow!("sftp runtime is unavailable for session `{session_id}`"))
    }
}

fn next_session_display_title(registry: &SessionRegistry, base_title: &str) -> String {
    let base_title = base_title.trim();
    if base_title.is_empty() {
        return String::new();
    }

    let mut used_slots = HashSet::new();
    for (session_id, session) in &registry.sessions {
        let Some(profile) = registry.session_profiles.get(session_id) else {
            continue;
        };
        if profile.name.trim() != base_title {
            continue;
        }
        if let Some(slot) = session_title_slot(session.title.as_str(), base_title) {
            used_slots.insert(slot);
        }
    }

    let next_slot = (1..)
        .find(|slot| !used_slots.contains(slot))
        .expect("numeric suffix space should not be exhausted");
    if next_slot == 1 {
        base_title.to_string()
    } else {
        format!("{base_title}({next_slot})")
    }
}

fn session_title_slot(title: &str, base_title: &str) -> Option<usize> {
    if title == base_title {
        return Some(1);
    }

    let suffix = title.strip_prefix(base_title)?;
    let suffix = suffix.strip_prefix('(')?.strip_suffix(')')?;
    suffix.parse::<usize>().ok().filter(|slot| *slot >= 2)
}

struct SessionRegistry {
    sessions: HashMap<Uuid, SessionHandle>,
    asset_sessions: HashMap<String, Uuid>,
    open_order: Vec<Uuid>,
    session_profiles: HashMap<Uuid, ConnectionProfile>,
    enhancement_fallback_cache: HashSet<EnhancementCacheKey>,
    connection_attempts: HashMap<Uuid, ConnectionAttemptState>,
    terminal_surfaces: HashMap<Uuid, TerminalSurfaceState>,
    current_working_directories: HashMap<Uuid, String>,
    zmodem_transfers: HashMap<Uuid, ZmodemTransferState>,
    terminal_shell_integration: HashMap<Uuid, TerminalShellIntegrationState>,
    terminal_surface_revisions: HashMap<Uuid, usize>,
    runtime_controls: HashMap<Uuid, SharedSessionRuntimeControl>,
    sftp_bindings: HashMap<Uuid, SftpSessionBinding>,
    disabled_enhancement_sessions: HashSet<Uuid>,
    pending_disconnects: HashSet<Uuid>,
    pending_resizes: HashMap<Uuid, (u32, u32)>,
    theme_mode: ThemeMode,
    theme_variant: ThemeVariant,
}

impl Default for SessionRegistry {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
            asset_sessions: HashMap::new(),
            open_order: Vec::new(),
            session_profiles: HashMap::new(),
            enhancement_fallback_cache: HashSet::new(),
            connection_attempts: HashMap::new(),
            terminal_surfaces: HashMap::new(),
            current_working_directories: HashMap::new(),
            zmodem_transfers: HashMap::new(),
            terminal_shell_integration: HashMap::new(),
            terminal_surface_revisions: HashMap::new(),
            runtime_controls: HashMap::new(),
            sftp_bindings: HashMap::new(),
            disabled_enhancement_sessions: HashSet::new(),
            pending_disconnects: HashSet::new(),
            pending_resizes: HashMap::new(),
            theme_mode: ThemeMode::Dark,
            theme_variant: ThemeVariant::PremiumDefault,
        }
    }
}

impl SessionRegistryDiagnosticsSnapshot {
    fn capture(registry: &SessionRegistry) -> Self {
        Self {
            session_count: registry.sessions.len(),
            open_order_count: registry.open_order.len(),
            asset_session_count: registry.asset_sessions.len(),
            terminal_surface_count: registry.terminal_surfaces.len(),
            runtime_control_count: registry.runtime_controls.len(),
            pending_disconnect_count: registry.pending_disconnects.len(),
            pending_resize_count: registry.pending_resizes.len(),
            current_working_directory_count: registry.current_working_directories.len(),
            disabled_enhancement_count: registry.disabled_enhancement_sessions.len(),
            sftp_binding_count: registry.sftp_bindings.len(),
        }
    }
}

fn apply_runtime_event(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    attempt_id: Uuid,
    event: SessionRuntimeEvent,
) {
    if !current_attempt_matches(registry, session_id, attempt_id) {
        return;
    }

    match event {
        SessionRuntimeEvent::Connected => {
            update_connection_attempt_headline(
                registry,
                session_id,
                ConnectionHeadlineState::Connected,
            );
            update_session(registry, session_id, SessionState::Connected, false);
        }
        SessionRuntimeEvent::Disconnected => {
            clear_runtime_control(registry, session_id);
            update_connection_attempt_headline(
                registry,
                session_id,
                ConnectionHeadlineState::Cancelled,
            );
            update_session(registry, session_id, SessionState::Disconnected, true);
        }
        SessionRuntimeEvent::Error(message) => {
            tracing::error!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                error = message.as_str(),
                "session manager received runtime error event"
            );
            clear_runtime_control(registry, session_id);
            project_connection_attempt_error(registry, session_id, message.clone());
            update_session(registry, session_id, SessionState::Error(message), true);
        }
        SessionRuntimeEvent::ConnectionProgress(progress_event) => {
            apply_connection_progress_event(registry, session_id, progress_event);
        }
        SessionRuntimeEvent::EnhancedSessionStateChanged(state) => {
            update_enhanced_session_state(registry, session_id, state);
        }
        SessionRuntimeEvent::CurrentDirectoryChanged(path) => {
            registry
                .lock()
                .expect("lock session registry")
                .current_working_directories
                .insert(session_id, path);
        }
        SessionRuntimeEvent::ZmodemStateChanged(state) => {
            let mut registry = registry.lock().expect("lock session registry");
            if let Some(state) = state {
                registry.zmodem_transfers.insert(session_id, state);
            } else {
                registry.zmodem_transfers.remove(&session_id);
            }
        }
        SessionRuntimeEvent::ShellIntegrationChanged(shell_state) => {
            let mut registry = registry.lock().expect("lock session registry");
            registry
                .terminal_shell_integration
                .insert(session_id, shell_state);
            if let Some(surface) = registry.terminal_surfaces.get_mut(&session_id) {
                surface.shell_integration = shell_state;
            }
        }
        SessionRuntimeEvent::SurfaceChanged(surface) => {
            update_terminal_surface(registry, session_id, surface);
        }
        SessionRuntimeEvent::SurfaceDirty => {
            mark_runtime_surface_dirty(registry, session_id);
        }
    }
}

fn update_session(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    state: SessionState,
    can_reconnect: bool,
) {
    if let Some(session) = registry
        .lock()
        .expect("lock session registry")
        .sessions
        .get_mut(&session_id)
    {
        session.state = state;
        session.can_reconnect = can_reconnect;
    }
}

fn update_connection_attempt_headline(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    headline: ConnectionHeadlineState,
) {
    if let Some(attempt) = registry
        .lock()
        .expect("lock session registry")
        .connection_attempts
        .get_mut(&session_id)
    {
        attempt.headline = headline;
    }
}

fn update_enhanced_session_state(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    state: EnhancedSessionState,
) {
    let mut registry = registry.lock().expect("lock session registry");
    if registry.disabled_enhancement_sessions.contains(&session_id) {
        if let Some(session) = registry.sessions.get_mut(&session_id) {
            session.enhanced_session_state = EnhancedSessionState::Plain;
        }
        return;
    }
    if let Some(session) = registry.sessions.get_mut(&session_id) {
        session.enhanced_session_state = state;
    }
}

fn apply_connection_progress_event(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    event: ConnectionProgressEvent,
) {
    let mut registry = registry.lock().expect("lock session registry");
    let Some(attempt) = registry.connection_attempts.get_mut(&session_id) else {
        return;
    };

    let mut next_session_state = None::<(SessionState, bool)>;

    match event {
        ConnectionProgressEvent::AttemptStarted {
            attempt_id,
            headline,
        } => {
            *attempt = ConnectionAttemptState::with_attempt_id(attempt_id, headline);
            next_session_state = session_state_for_headline(headline);
        }
        ConnectionProgressEvent::HeadlineChanged {
            attempt_id,
            headline,
        } => {
            if attempt.attempt_id == attempt_id {
                attempt.headline = headline;
                next_session_state = session_state_for_headline(headline);
            }
        }
        ConnectionProgressEvent::StepUpdated { attempt_id, step } => {
            if attempt.attempt_id != attempt_id {
                return;
            }
            upsert_connection_step(&mut attempt.steps, step);
        }
        ConnectionProgressEvent::DiagnosticAppended {
            attempt_id,
            message,
        } => {
            if attempt.attempt_id != attempt_id {
                return;
            }
            attempt.diagnostics.push(ConnectionDiagnosticLine {
                attempt_id,
                message,
            });
        }
    }

    if let Some((state, can_reconnect)) = next_session_state
        && let Some(session) = registry.sessions.get_mut(&session_id)
    {
        session.state = state;
        session.can_reconnect = can_reconnect;
    }
}

fn upsert_connection_step(steps: &mut Vec<ConnectionStepStateItem>, step: ConnectionStepStateItem) {
    if let Some(existing) = steps.iter_mut().find(|item| item.step_id == step.step_id) {
        *existing = step;
    } else if let Some(existing) = steps
        .iter_mut()
        .find(|item| item.step_id == "verify-host-key" && step.step_kind == "verify-host-key")
    {
        *existing = step;
    } else {
        steps.push(step);
    }
}

fn update_terminal_surface(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    mut surface: TerminalSurfaceState,
) {
    let mut registry = registry.lock().expect("lock session registry");
    if registry.sessions.contains_key(&session_id) {
        if let Some(shell_state) = registry
            .terminal_shell_integration
            .get(&session_id)
            .copied()
        {
            surface.shell_integration = shell_state;
        }
        registry
            .terminal_surface_revisions
            .insert(session_id, surface.seqno);
        registry.terminal_surfaces.insert(session_id, surface);
    }
}

fn mark_runtime_surface_dirty(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let mut registry = registry.lock().expect("lock session registry");
    if !registry.sessions.contains_key(&session_id) {
        return;
    }

    let next_revision = registry
        .terminal_surface_revisions
        .get(&session_id)
        .copied()
        .or_else(|| {
            registry
                .terminal_surfaces
                .get(&session_id)
                .map(|surface| surface.seqno)
        })
        .unwrap_or(0)
        .saturating_add(1);
    registry
        .terminal_surface_revisions
        .insert(session_id, next_revision);
}

fn refresh_runtime_surface(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let runtime_control = {
        let registry = registry.lock().expect("lock session registry");
        registry.runtime_controls.get(&session_id).cloned()
    };
    let surface = runtime_control.and_then(|runtime_control| {
        runtime_control
            .lock()
            .expect("lock session runtime control for surface refresh")
            .terminal_surface()
            .ok()
    });

    if let Some(surface) = surface {
        update_terminal_surface(registry, session_id, surface);
    }
}

fn terminal_surface_stale(registry: &SessionRegistry, session_id: Uuid) -> bool {
    let revision = registry
        .terminal_surface_revisions
        .get(&session_id)
        .copied()
        .unwrap_or(0);

    match registry.terminal_surfaces.get(&session_id) {
        Some(surface) => revision > surface.seqno,
        None => revision > 0 && registry.runtime_controls.contains_key(&session_id),
    }
}

fn terminal_surface_signature_for_registry(
    registry: &SessionRegistry,
    session_id: Uuid,
) -> Option<TerminalSurfaceSignature> {
    let mut signature = registry.terminal_surfaces.get(&session_id)?.signature();
    if let Some(revision) = registry
        .terminal_surface_revisions
        .get(&session_id)
        .copied()
        && revision > signature.seqno
    {
        signature.seqno = revision;
    }

    Some(signature)
}

fn coalesce_surface_backlog(
    initial_surface: TerminalSurfaceState,
    mut backlog: VecDeque<SessionRuntimeEvent>,
) -> (TerminalSurfaceState, VecDeque<SessionRuntimeEvent>) {
    let mut latest_surface = initial_surface;

    while matches!(
        backlog.front(),
        Some(SessionRuntimeEvent::SurfaceChanged(_))
    ) {
        let Some(SessionRuntimeEvent::SurfaceChanged(surface)) = backlog.pop_front() else {
            break;
        };
        latest_surface = surface;
    }

    (latest_surface, backlog)
}

fn coalesce_surface_dirty_backlog(
    mut backlog: VecDeque<SessionRuntimeEvent>,
) -> VecDeque<SessionRuntimeEvent> {
    while matches!(backlog.front(), Some(SessionRuntimeEvent::SurfaceDirty)) {
        let _ = backlog.pop_front();
    }

    backlog
}

fn attach_runtime_control(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    attempt_id: Uuid,
    runtime_control: Box<dyn SessionRuntimeControl>,
) {
    let mut runtime_control = Some(runtime_control);
    let (should_disconnect, pending_resize, theme_mode, theme_variant) = {
        let mut registry = registry.lock().expect("lock session registry");
        if !registry.sessions.contains_key(&session_id)
            || registry
                .connection_attempts
                .get(&session_id)
                .map(|attempt| attempt.attempt_id != attempt_id)
                .unwrap_or(true)
            || registry.pending_disconnects.remove(&session_id)
        {
            (true, None, registry.theme_mode, registry.theme_variant)
        } else {
            let shared_runtime_control = Arc::new(Mutex::new(
                runtime_control.take().expect("runtime control available"),
            ));
            if let Some(binding) = shared_runtime_control
                .lock()
                .expect("lock session runtime control for sftp binding")
                .sftp_runtime()
                .map(|runtime| SftpSessionBinding::connecting(session_id, runtime))
            {
                registry.sftp_bindings.insert(session_id, binding);
            }
            registry
                .runtime_controls
                .insert(session_id, shared_runtime_control);
            (
                false,
                registry.pending_resizes.remove(&session_id),
                registry.theme_mode,
                registry.theme_variant,
            )
        }
    };

    if should_disconnect && let Some(runtime_control) = runtime_control {
        release_and_disconnect_runtime_control(Arc::new(Mutex::new(runtime_control)));
        return;
    }

    let theme_runtime_control = {
        let registry_guard = registry.lock().expect("lock session registry");
        registry_guard.runtime_controls.get(&session_id).cloned()
    };
    let theme_surface = theme_runtime_control.and_then(|runtime_control| {
        runtime_control
            .lock()
            .expect("lock session runtime control for theme update")
            .update_theme(theme_mode, theme_variant)
            .ok()
            .flatten()
    });
    if let Some(surface) = theme_surface {
        update_terminal_surface(registry, session_id, surface);
    } else {
        refresh_runtime_surface(registry, session_id);
    }

    if let Some((rows, cols)) = pending_resize {
        let runtime_control = {
            let registry = registry.lock().expect("lock session registry");
            registry.runtime_controls.get(&session_id).cloned()
        };
        if let Some(runtime_control) = runtime_control {
            let _ = runtime_control
                .lock()
                .expect("lock session runtime control for pending resize")
                .resize(rows, cols);
        }
    }
}

fn clear_runtime_control(registry: &Arc<Mutex<SessionRegistry>>, session_id: Uuid) {
    let mut registry = registry.lock().expect("lock session registry");
    registry.runtime_controls.remove(&session_id);
    mark_sftp_binding_disconnected(&mut registry, session_id);
    registry.pending_disconnects.remove(&session_id);
    registry.pending_resizes.remove(&session_id);
    registry.zmodem_transfers.remove(&session_id);
    let surface_seqno = registry
        .terminal_surfaces
        .get(&session_id)
        .map(|surface| surface.seqno);
    if let Some(surface_seqno) = surface_seqno {
        registry
            .terminal_surface_revisions
            .insert(session_id, surface_seqno);
    } else {
        registry.terminal_surface_revisions.remove(&session_id);
    }
}

fn mark_sftp_binding_disconnected(registry: &mut SessionRegistry, session_id: Uuid) {
    if let Some(binding) = registry.sftp_bindings.get_mut(&session_id) {
        binding.mark_disconnected();
    }
}

fn current_attempt_matches(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    attempt_id: Uuid,
) -> bool {
    registry
        .lock()
        .expect("lock session registry")
        .connection_attempts
        .get(&session_id)
        .map(|attempt| attempt.attempt_id == attempt_id)
        .unwrap_or(false)
}

fn session_state_for_headline(headline: ConnectionHeadlineState) -> Option<(SessionState, bool)> {
    match headline {
        ConnectionHeadlineState::Connecting => Some((SessionState::Connecting, false)),
        ConnectionHeadlineState::WaitingUser => Some((SessionState::WaitingUser, false)),
        ConnectionHeadlineState::Connected => Some((SessionState::Connected, false)),
        ConnectionHeadlineState::Cancelled => Some((SessionState::Cancelled, true)),
        ConnectionHeadlineState::Error => None,
    }
}

fn finalize_connection_attempt(
    attempt: &mut ConnectionAttemptState,
    headline: ConnectionHeadlineState,
    step_state: ConnectionStepState,
    message: impl Into<String>,
) {
    let message = message.into();
    let final_attempt_id = Uuid::new_v4();
    attempt.attempt_id = final_attempt_id;
    attempt.headline = headline;
    attempt.prompt = None;
    if let Some(step) = attempt.steps.iter_mut().rfind(|step| {
        matches!(
            step.state,
            ConnectionStepState::Running | ConnectionStepState::Blocked
        )
    }) {
        step.state = step_state;
        step.detail = message.clone();
    }
    attempt.diagnostics.push(ConnectionDiagnosticLine {
        attempt_id: final_attempt_id,
        message,
    });
}

fn project_connection_attempt_error(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    message: impl Into<String>,
) {
    let message = message.into();
    let mut registry = registry.lock().expect("lock session registry");
    let Some(attempt) = registry.connection_attempts.get_mut(&session_id) else {
        return;
    };
    attempt.headline = ConnectionHeadlineState::Error;
    attempt.prompt = None;
    if let Some(step) = attempt.steps.iter_mut().rfind(|step| {
        matches!(
            step.state,
            ConnectionStepState::Running | ConnectionStepState::Blocked
        )
    }) {
        step.state = ConnectionStepState::Failed;
        step.detail = message.clone();
    }
    if attempt.diagnostics.last().map(|line| line.message.as_str()) != Some(message.as_str()) {
        attempt.diagnostics.push(ConnectionDiagnosticLine {
            attempt_id: attempt.attempt_id,
            message,
        });
    }
}

fn apply_unknown_host_key_prompt(
    registry: &Arc<Mutex<SessionRegistry>>,
    session_id: Uuid,
    attempt_id: Uuid,
    error: &UnknownHostKeyError,
) {
    let mut registry = registry.lock().expect("lock session registry");
    let Some(attempt) = registry.connection_attempts.get_mut(&session_id) else {
        return;
    };
    if attempt.attempt_id != attempt_id {
        return;
    }

    let message = format!(
        "Host key verification required for {}:{} ({})",
        error.host, error.port, error.fingerprint
    );
    attempt.headline = ConnectionHeadlineState::WaitingUser;
    attempt.prompt = Some(ConnectionHostKeyPrompt {
        host: error.host.clone(),
        port: error.port,
        fingerprint: error.fingerprint.clone(),
        public_key_openssh: error.public_key_openssh.clone(),
    });
    if let Some(step) = attempt
        .steps
        .iter_mut()
        .rfind(|step| step.step_kind == "verify-host-key")
    {
        step.state = ConnectionStepState::Blocked;
        step.detail = message.clone();
    } else {
        attempt.steps.push(ConnectionStepStateItem {
            step_id: "verify-host-key".into(),
            step_kind: "verify-host-key".into(),
            title: "Verify Host Key".into(),
            detail: message.clone(),
            hop_label: "Target".into(),
            state: ConnectionStepState::Blocked,
        });
    }
    let should_append_diagnostic =
        attempt.diagnostics.last().map(|line| line.message.as_str()) != Some(message.as_str());
    if should_append_diagnostic {
        attempt.diagnostics.push(ConnectionDiagnosticLine {
            attempt_id,
            message,
        });
    }

    if let Some(session) = registry.sessions.get_mut(&session_id) {
        session.state = SessionState::WaitingUser;
        session.can_reconnect = false;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use anyhow::Result;
    use uuid::Uuid;

    use super::{
        ConnectionAttemptState, ConnectionHeadlineState, EnhancedSessionState, SessionHandle,
        SessionRegistry, SessionRuntimeControl, SessionRuntimeEvent, SessionState,
        apply_runtime_event, coalesce_surface_backlog, coalesce_surface_dirty_backlog,
        refresh_runtime_surface, terminal_surface_signature_for_registry, terminal_surface_stale,
        update_terminal_surface,
    };
    use crate::app::ssh::runtime::{TerminalKeyEvent, TerminalMouseInput, TerminalSurfaceState};

    #[test]
    fn coalesces_consecutive_surface_updates_but_preserves_following_control_events() {
        let session_id = Uuid::new_v4();
        let initial =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let newer =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let latest =
            TerminalSurfaceState::from_visible_lines(session_id, 3, 24, 80, vec!["three".into()]);
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::SurfaceChanged(newer),
            SessionRuntimeEvent::SurfaceChanged(latest.clone()),
            SessionRuntimeEvent::Error("boom".into()),
            SessionRuntimeEvent::Disconnected,
        ]);

        let (coalesced, remaining) = coalesce_surface_backlog(initial, backlog);

        assert_eq!(coalesced, latest);
        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Error(message)) if message == "boom"
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::Disconnected)
        ));
    }

    #[test]
    fn leaves_non_surface_prefix_events_in_place() {
        let session_id = Uuid::new_v4();
        let initial =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let later =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::Connected,
            SessionRuntimeEvent::SurfaceChanged(later.clone()),
        ]);

        let (coalesced, remaining) = coalesce_surface_backlog(initial.clone(), backlog);

        assert_eq!(coalesced, initial);
        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Connected)
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::SurfaceChanged(surface)) if *surface == later
        ));
    }

    #[test]
    fn coalesces_consecutive_surface_dirty_events_but_preserves_following_control_events() {
        let backlog = VecDeque::from(vec![
            SessionRuntimeEvent::SurfaceDirty,
            SessionRuntimeEvent::SurfaceDirty,
            SessionRuntimeEvent::Error("boom".into()),
            SessionRuntimeEvent::Disconnected,
        ]);

        let remaining = coalesce_surface_dirty_backlog(backlog);

        assert_eq!(remaining.len(), 2);
        assert!(matches!(
            remaining.front(),
            Some(SessionRuntimeEvent::Error(message)) if message == "boom"
        ));
        assert!(matches!(
            remaining.get(1),
            Some(SessionRuntimeEvent::Disconnected)
        ));
    }

    #[test]
    fn surface_dirty_does_not_pull_runtime_snapshot_immediately() {
        let session_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let registry = Arc::new(Mutex::new(SessionRegistry::default()));
        let initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let runtime_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let terminal_surface_calls = Arc::new(AtomicUsize::new(0));

        {
            let mut registry_guard = registry.lock().expect("lock session registry");
            registry_guard.sessions.insert(
                session_id,
                SessionHandle {
                    session_id,
                    asset_id: "asset-prod".into(),
                    title: "Prod Bastion".into(),
                    subtitle: "ops@example.com:22".into(),
                    state: SessionState::Connected,
                    can_reconnect: false,
                    enhanced_session_state: EnhancedSessionState::Plain,
                },
            );
            registry_guard.connection_attempts.insert(
                session_id,
                ConnectionAttemptState::with_attempt_id(
                    attempt_id,
                    ConnectionHeadlineState::Connected,
                ),
            );
            registry_guard.runtime_controls.insert(
                session_id,
                Arc::new(Mutex::new(Box::new(CountingRuntimeControl::new(
                    runtime_surface,
                    Arc::clone(&terminal_surface_calls),
                )))),
            );
        }
        update_terminal_surface(&registry, session_id, initial_surface);

        apply_runtime_event(
            &registry,
            session_id,
            attempt_id,
            SessionRuntimeEvent::SurfaceDirty,
        );

        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn surface_dirty_marks_snapshot_stale_until_on_demand_refresh() {
        let session_id = Uuid::new_v4();
        let attempt_id = Uuid::new_v4();
        let registry = Arc::new(Mutex::new(SessionRegistry::default()));
        let initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["one".into()]);
        let runtime_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 2, 24, 80, vec!["two".into()]);
        let terminal_surface_calls = Arc::new(AtomicUsize::new(0));

        {
            let mut registry_guard = registry.lock().expect("lock session registry");
            registry_guard.sessions.insert(
                session_id,
                SessionHandle {
                    session_id,
                    asset_id: "asset-prod".into(),
                    title: "Prod Bastion".into(),
                    subtitle: "ops@example.com:22".into(),
                    state: SessionState::Connected,
                    can_reconnect: false,
                    enhanced_session_state: EnhancedSessionState::Plain,
                },
            );
            registry_guard.connection_attempts.insert(
                session_id,
                ConnectionAttemptState::with_attempt_id(
                    attempt_id,
                    ConnectionHeadlineState::Connected,
                ),
            );
            registry_guard.runtime_controls.insert(
                session_id,
                Arc::new(Mutex::new(Box::new(CountingRuntimeControl::new(
                    runtime_surface.clone(),
                    Arc::clone(&terminal_surface_calls),
                )))),
            );
        }
        update_terminal_surface(&registry, session_id, initial_surface);

        apply_runtime_event(
            &registry,
            session_id,
            attempt_id,
            SessionRuntimeEvent::SurfaceDirty,
        );

        {
            let registry_guard = registry.lock().expect("lock session registry");
            assert!(terminal_surface_stale(&registry_guard, session_id));
            assert_eq!(
                terminal_surface_signature_for_registry(&registry_guard, session_id)
                    .expect("signature after dirty")
                    .seqno,
                2
            );
        }
        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 0);

        refresh_runtime_surface(&registry, session_id);

        {
            let registry_guard = registry.lock().expect("lock session registry");
            assert!(!terminal_surface_stale(&registry_guard, session_id));
            let surface = registry_guard
                .terminal_surfaces
                .get(&session_id)
                .expect("refreshed runtime surface");
            assert_eq!(surface.seqno, runtime_surface.seqno);
            assert_eq!(surface.visible_lines, runtime_surface.visible_lines);
        }
        assert_eq!(terminal_surface_calls.load(Ordering::SeqCst), 1);
    }

    struct CountingRuntimeControl {
        surface: TerminalSurfaceState,
        terminal_surface_calls: Arc<AtomicUsize>,
    }

    impl CountingRuntimeControl {
        fn new(surface: TerminalSurfaceState, terminal_surface_calls: Arc<AtomicUsize>) -> Self {
            Self {
                surface,
                terminal_surface_calls,
            }
        }
    }

    impl SessionRuntimeControl for CountingRuntimeControl {
        fn disconnect(&self) -> Result<()> {
            Ok(())
        }

        fn send_text_input(&self, _text: String) -> Result<()> {
            Ok(())
        }

        fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
            Ok(())
        }

        fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
            Ok(())
        }

        fn send_paste(&self, _text: String) -> Result<()> {
            Ok(())
        }

        fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
            Ok(())
        }

        fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
            self.terminal_surface_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.surface.clone())
        }
    }
}
