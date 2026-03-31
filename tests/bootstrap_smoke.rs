//! Basic bootstrap helper coverage for the binary entrypoint.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedAssetSocks5ProxySpec,
    PersistedAssetSshProxySpec, PersistedSnippetSpec, PersistedSshConnectionSpec,
    catalog_to_asset_tree,
};
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter, VaultProviderFactory, VaultRuntimeOptions, app_title,
    bind_top_status_bar_with_injected_services_and_vault_runtime, bind_top_status_bar_with_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer,
    build_shared_app_credential_store_for_paths, default_window_size,
};
use mica_term::app::sftp::{
    SftpBackend, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpRuntimeHandle,
};
use mica_term::app::keychain::KeychainCatalog;
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::ssh::connection_progress::{
    ConnectionProgressEvent, ConnectionStepState, ConnectionStepStateItem,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, StoredSshSecretBundle,
    load_secret_bundle, persist_secret_bundle, ssh_credential_ref,
};
use mica_term::app::ssh::known_hosts::{
    KnownHostCheck, KnownHostsService, default_known_hosts_path,
};
use mica_term::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalKeyKind, TerminalMouseInput, TerminalSession,
    TerminalSurfaceState, UnknownHostKeyError,
};
use mica_term::app::ssh::session_manager::{
    EnhancementPolicy, SessionManager, SessionRuntimeControl, SessionRuntimeLauncher,
};
use mica_term::app::terminal_theme::preset_for_theme_mode;
use mica_term::app::vault::bootstrap::{
    LocalVaultBootstrapState, load_local_vault_bootstrap_state, load_runtime_vault_key,
    save_local_vault_bootstrap_state,
};
use mica_term::app::vault::cache::{load_encrypted_cache, store_encrypted_cache};
use mica_term::app::vault::crypto::{encrypt_snapshot, generate_vault_key, wrap_vault_key};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind,
    KdfConfig, PackLayout, PackRef, ProviderAuthKind, ProviderKind, RemoteRole,
    SnapshotSyncPreferences, VaultAssetPayload, VaultHead, VaultManifest,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, ProviderRevision, VaultProvider};
use mica_term::app::vault::recovery::{RecoverySource, load_recovery_snapshots};
use mica_term::app::vault::snapshot::export_vault_snapshot;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use mica_term::shell::metrics::ShellMetrics;
use mica_term::theme::ThemeMode;
use russh::keys::{HashAlg, PublicKey};
use secrecy::SecretString;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use tokio::sync::mpsc;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn bootstrap_source_uses_terminal_presenter_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("TerminalPresenter"),
        "bootstrap should depend on a terminal presenter seam instead of calling the atlas renderer directly"
    );
    assert!(
        bootstrap_source.contains("PresentedTerminalFrame"),
        "bootstrap should project presenter output through a PresentedTerminalFrame contract"
    );
    assert!(
        !bootstrap_source.contains("TerminalAtlasRenderer::new()"),
        "bootstrap should stop constructing TerminalAtlasRenderer directly once the presenter boundary exists"
    );
}

#[test]
fn bootstrap_source_threads_native_terminal_surface_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("NativeTerminalSurface"),
        "bootstrap should depend on a native terminal surface hook once native terminal rendering is introduced"
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_render_mode"),
        "bootstrap should publish the active terminal render mode to the Slint window"
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_native_frame_token"),
        "bootstrap should publish native frame tokens for the renderer hook path"
    );
}

#[test]
fn bootstrap_source_uses_windows_native_terminal_presenter_for_native_frames() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("WindowsNativePresenter"),
        "bootstrap should install a Windows native terminal presenter once the native text path exists"
    );
    assert!(
        bootstrap_source.contains("PresentedTerminalFrame::Native(frame)"),
        "bootstrap should consume native terminal frames from the presenter seam"
    );
    assert!(
        !bootstrap_source.contains("frame_token: u64::try_from(surface.seqno)"),
        "bootstrap should stop synthesizing native frame tokens directly from surface seqno once the native renderer owns frame preparation"
    );
}

#[test]
fn session_manager_skips_auto_bootstrap_for_cached_fallback_host() {
    let runtime = mica_term::app::async_runtime::AppAsyncRuntime::new()
        .expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(runtime.handle(), Arc::new(FakeLauncher));
    let profile = ConnectionProfile {
        asset_id: Some("asset-prod".into()),
        name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: 22,
        auth_method: SshAuthMethod::Password,
        credential_ref: Some("draft://ssh-password/ops@10.0.0.12:22".into()),
        private_key_path: None,
        password: Some("secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy: ConnectionProxyProfile::None,
        resolved_proxy_hops: Vec::new(),
        remark: String::new(),
    };

    manager.remember_enhancement_fallback(&profile, "bash");

    let policy = manager.enhancement_policy_for(&profile);

    assert_eq!(policy, EnhancementPolicy::SkipAutoBootstrap);
}

#[test]
fn path_errors_render_as_lightweight_status_rows_instead_of_full_height_empty_cards() {
    let panel_source = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");

    assert!(
        panel_source.contains("status-row"),
        "right panel should expose a lightweight status row for loading, error, and disconnected feedback"
    );
    assert!(
        !panel_source.contains("root.sftp-panel-mode == \"empty\" || root.sftp-panel-mode == \"disconnected\" || root.sftp-panel-mode == \"error\" : empty-state"),
        "error and disconnected states should no longer be rendered through the full-height empty state shell"
    );
    assert!(
        !panel_source.contains("copy-card :="),
        "right panel should remove the legacy full-height copy card for path errors"
    );
}

#[derive(Default)]
struct AssetRepoState {
    load_calls: usize,
    save_attempts: Vec<PersistedAssetCatalog>,
}

struct RecordingAssetRepo {
    loaded_catalog: PersistedAssetCatalog,
    state: Rc<RefCell<AssetRepoState>>,
    save_error: Option<&'static str>,
}

impl RecordingAssetRepo {
    fn new(
        loaded_catalog: PersistedAssetCatalog,
        state: Rc<RefCell<AssetRepoState>>,
        save_error: Option<&'static str>,
    ) -> Self {
        Self {
            loaded_catalog,
            state,
            save_error,
        }
    }
}

impl AssetCatalogRepository for RecordingAssetRepo {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        self.state.borrow_mut().load_calls += 1;
        Ok(self.loaded_catalog.clone())
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        self.state.borrow_mut().save_attempts.push(catalog.clone());
        if let Some(message) = self.save_error {
            return Err(anyhow!(message));
        }

        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeLauncher;

#[derive(Clone)]
struct TofuAwareLauncher {
    host_key: PublicKey,
}

#[derive(Clone, Default)]
struct PendingConnectionLauncher;

#[derive(Clone)]
struct AsyncProjectionLauncher;

#[derive(Clone, Default)]
struct InteractiveProjectionLauncher;

#[derive(Clone, Default)]
struct PasteProjectionLauncher;

#[derive(Clone, Copy)]
struct PasteWarningProjectionLauncher {
    bracketed_paste_enabled: bool,
}

#[derive(Clone, Default)]
struct ScrollProjectionLauncher;

#[derive(Clone)]
struct FollowProjectionLauncher {
    state: FollowProjectionState,
}

#[derive(Clone)]
struct FailingProbeLauncher {
    message: &'static str,
}

#[derive(Clone)]
struct StoredSecretProbeLauncher {
    store: Arc<dyn CredentialStore>,
    message: &'static str,
}

#[derive(Default)]
struct RecordingLauncherState {
    launch_profiles: Vec<ConnectionProfile>,
    probe_profiles: Vec<ConnectionProfile>,
}

#[derive(Clone)]
struct RecordingLauncher {
    state: Arc<Mutex<RecordingLauncherState>>,
}

#[derive(Clone)]
struct SlowOpeningLauncher {
    state: Arc<Mutex<RecordingLauncherState>>,
    probe_delay: Duration,
    launch_delay: Duration,
}

#[derive(Clone)]
struct SuccessfulPrivateKeyImporter {
    path: std::path::PathBuf,
    content: &'static str,
}

#[derive(Clone, Default)]
struct CancelledPrivateKeyImporter;

#[derive(Clone)]
struct FailingPrivateKeyImporter {
    message: &'static str,
}

#[derive(Default)]
struct UnavailableCredentialStore;

struct NoopRuntimeControl;

struct PendingConnectionRuntimeControl {
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct RecordingSftpState {
    read_dir_calls: Arc<Mutex<Vec<String>>>,
}

impl RecordingSftpState {
    fn take_read_dir_calls(&self) -> Vec<String> {
        std::mem::take(&mut *self.read_dir_calls.lock().expect("lock sftp read_dir calls"))
    }
}

#[derive(Clone)]
struct RecordingSftpLauncher {
    state: RecordingSftpState,
}

struct RecordingSftpRuntimeControl {
    runtime: SftpRuntimeHandle,
}

struct RecordingSftpBackend {
    responses: BTreeMap<String, Vec<SftpDirectoryEntry>>,
    state: RecordingSftpState,
}

#[derive(Clone, Default)]
struct RecordingVaultProviderFactory {
    providers: Arc<Mutex<BTreeMap<String, Arc<MockVaultProvider>>>>,
}

impl RecordingVaultProviderFactory {
    fn insert(&self, provider: Arc<MockVaultProvider>) {
        self.providers
            .lock()
            .expect("lock vault provider factory")
            .insert(provider.remote_id().to_string(), provider);
    }
}

impl VaultProviderFactory for RecordingVaultProviderFactory {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>> {
        let provider = self
            .providers
            .lock()
            .expect("lock vault provider factory")
            .get(&remote.remote_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing mock vault provider `{}`", remote.remote_id))?;
        Ok(provider as Arc<dyn VaultProvider>)
    }
}

struct InteractiveProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct KeyboardMatrixState {
    key_inputs: Arc<Mutex<Vec<TerminalKeyEvent>>>,
    paste_inputs: Arc<Mutex<Vec<String>>>,
}

impl KeyboardMatrixState {
    fn take_key_inputs(&self) -> Vec<TerminalKeyEvent> {
        std::mem::take(
            &mut *self
                .key_inputs
                .lock()
                .expect("lock keyboard matrix key inputs"),
        )
    }

    fn take_paste_inputs(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .paste_inputs
                .lock()
                .expect("lock keyboard matrix paste inputs"),
        )
    }
}

#[derive(Clone)]
struct KeyboardMatrixLauncher {
    state: KeyboardMatrixState,
}

impl KeyboardMatrixLauncher {
    fn new(state: KeyboardMatrixState) -> Self {
        Self { state }
    }
}

struct PasteProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct ScrollProjectionState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
}

#[derive(Clone, Default)]
struct FollowProjectionState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
}

impl FollowProjectionState {
    fn emit_remote_output(&self, appended_lines: u32) {
        let mut surface_guard = self.surface.lock().expect("lock follow projection surface");
        let current = surface_guard
            .clone()
            .expect("current follow projection surface");
        let next_offset = if current.viewport_at_bottom {
            0
        } else {
            current.viewport_offset_lines.saturating_add(appended_lines)
        };
        let next_surface = bootstrap_surface_with_viewport(
            current.session_id,
            current.seqno.saturating_add(1),
            next_offset,
            current
                .viewport_max_offset_lines
                .saturating_add(appended_lines),
        );
        *surface_guard = Some(next_surface.clone());
        drop(surface_guard);

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock follow projection event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(next_surface));
        }
    }

    fn emit_live_surface(&self, label: &str) {
        let mut surface_guard = self.surface.lock().expect("lock follow projection surface");
        let current = surface_guard
            .clone()
            .expect("current follow projection surface");
        let mut next_surface = bootstrap_surface_with_viewport(
            current.session_id,
            current.seqno.saturating_add(1),
            0,
            current.viewport_max_offset_lines,
        );
        next_surface.visible_lines = vec!["live".into(), label.into()];
        *surface_guard = Some(next_surface.clone());
        drop(surface_guard);

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock follow projection event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(next_surface));
        }
    }
}

struct ScrollProjectionRuntimeControl {
    state: ScrollProjectionState,
}

struct FollowProjectionRuntimeControl {
    state: FollowProjectionState,
}

struct KeyboardMatrixRuntimeControl {
    state: KeyboardMatrixState,
}

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SftpBackend for RecordingSftpBackend {
    fn read_dir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        let state = self.state.clone();
        let response = self.responses.get(path).cloned().unwrap_or_default();
        let path = path.to_string();
        Box::pin(async move {
            state
                .read_dir_calls
                .lock()
                .expect("lock sftp read_dir calls")
                .push(path);
            Ok(response)
        })
    }

    fn mkdir<'a>(&'a self, _path: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn rename<'a>(
        &'a self,
        _from: &'a str,
        _to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn path_exists<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move { Ok(true) })
    }

    fn upload_file<'a>(
        &'a self,
        _remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move { Ok(data.len() as u64) })
    }

    fn download_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn remove_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn remove_dir<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeControl for RecordingSftpRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.runtime.clone())
    }
}

impl SessionRuntimeLauncher for RecordingSftpLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let responses = match profile.host.as_str() {
                "10.0.0.12" => BTreeMap::from([
                    (
                        "/srv/app".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-logs".into(),
                            name: "logs".into(),
                            path: "/srv/app/logs".into(),
                            kind: SftpDirectoryEntryKind::Directory,
                            size_bytes: None,
                        }],
                    ),
                    (
                        "/srv/app/releases".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-release".into(),
                            name: "release.tar.gz".into(),
                            path: "/srv/app/releases/release.tar.gz".into(),
                            kind: SftpDirectoryEntryKind::File,
                            size_bytes: Some(14 * 1024),
                        }],
                    ),
                ]),
                _ => BTreeMap::from([(
                    "/srv/db".to_string(),
                    vec![SftpDirectoryEntry {
                        id: "entry-backup".into(),
                        name: "backup.sql".into(),
                        path: "/srv/db/backup.sql".into(),
                        kind: SftpDirectoryEntryKind::File,
                        size_bytes: Some(7 * 1024),
                    }],
                )]),
            };

            let cwd = if profile.host == "10.0.0.24" {
                "/srv/db"
            } else {
                "/srv/app"
            };
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.into()));
            Ok(Box::new(RecordingSftpRuntimeControl {
                runtime: SftpRuntimeHandle::new(Arc::new(RecordingSftpBackend {
                    responses,
                    state,
                })),
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeControl for PendingConnectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::Disconnected);
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl TofuAwareLauncher {
    fn new(host_key: PublicKey) -> Self {
        Self { host_key }
    }

    fn ensure_trusted(&self, profile: &ConnectionProfile) -> Result<()> {
        let known_hosts = KnownHostsService::new(default_known_hosts_path()?);
        match known_hosts.check(&profile.host, profile.port, &self.host_key)? {
            KnownHostCheck::Trusted => Ok(()),
            KnownHostCheck::Unknown { fingerprint } => Err(UnknownHostKeyError {
                host: profile.host.clone(),
                port: profile.port,
                fingerprint,
                public_key_openssh: self.host_key.to_openssh().expect("encode tofu host key"),
            }
            .into()),
            KnownHostCheck::Changed { expected, actual } => Err(anyhow!(
                "SSH host key changed for `{}`:{} (expected {}, got {})",
                profile.host,
                profile.port,
                expected,
                actual
            )),
        }
    }
}

impl SessionRuntimeLauncher for TofuAwareLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let launcher = self.clone();
        Box::pin(async move {
            launcher.ensure_trusted(&profile)?;
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let launcher = self.clone();
        Box::pin(async move { launcher.ensure_trusted(&profile) })
    }
}

impl SessionRuntimeLauncher for PendingConnectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                ConnectionProgressEvent::StepUpdated {
                    attempt_id,
                    step: ConnectionStepStateItem {
                        step_id: "00-connect-target".into(),
                        step_kind: "connect-target".into(),
                        title: "Connect Target".into(),
                        detail: "Opening SSH transport to 10.0.0.12".into(),
                        hop_label: "Target".into(),
                        state: ConnectionStepState::Running,
                    },
                },
            ));
            Ok(Box::new(PendingConnectionRuntimeControl { event_tx })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for AsyncProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let _ = event_tx.send(SessionRuntimeEvent::Connected);
                let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                    mica_term::app::ssh::runtime::TerminalSurfaceState::from_visible_lines(
                        session_id,
                        1,
                        24,
                        80,
                        vec!["welcome to mica-term".into()],
                    ),
                ));
            });
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for InteractiveProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                terminal_surface_with_cells(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(InteractiveProjectionRuntimeControl {
                session_id,
                event_tx,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for KeyboardMatrixLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                terminal_surface_with_cells(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(KeyboardMatrixRuntimeControl { state }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for PasteProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                TerminalSurfaceState::from_visible_lines(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(PasteProjectionRuntimeControl {
                session_id,
                event_tx,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for PasteWarningProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let bracketed_paste_enabled = self.bracketed_paste_enabled;
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let mut surface = TerminalSurfaceState::from_visible_lines(
                session_id,
                1,
                24,
                80,
                vec!["welcome to mica-term".into()],
            );
            surface.bracketed_paste_enabled = bracketed_paste_enabled;
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(PasteProjectionRuntimeControl {
                session_id,
                event_tx,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for ScrollProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let state = ScrollProjectionState::default();
            let surface = bootstrap_surface_with_viewport(session_id, 1, 3, 8);
            *state
                .surface
                .lock()
                .expect("lock scroll projection surface") = Some(surface.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(ScrollProjectionRuntimeControl { state })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for FollowProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let surface = bootstrap_surface_with_viewport(session_id, 1, 0, 8);
            *state
                .surface
                .lock()
                .expect("lock follow projection surface") = Some(surface.clone());
            *state
                .event_tx
                .lock()
                .expect("lock follow projection event tx") = Some(event_tx.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(FollowProjectionRuntimeControl { state })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for FailingProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let message = self.message;
        Box::pin(async move { Err(anyhow!(message)) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let message = self.message;
        Box::pin(async move { Err(anyhow!(message)) })
    }
}

impl SessionRuntimeControl for InteractiveProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("$ {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("$ {}", rendered)],
            ),
        ));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.send_text_input(text)
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec![
                    "welcome to mica-term".into(),
                    "mouse input forwarded".into(),
                ],
            ),
        ));
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for KeyboardMatrixRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        self.state
            .key_inputs
            .lock()
            .expect("lock keyboard matrix key inputs")
            .push(event);
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.state
            .paste_inputs
            .lock()
            .expect("lock keyboard matrix paste inputs")
            .push(text);
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for PasteProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("text {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("key {}", rendered)],
            ),
        ));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("paste {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for ScrollProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        let mut surface = self
            .state
            .surface
            .lock()
            .expect("lock scroll projection surface")
            .clone()
            .expect("current scroll projection surface");
        let next_offset = (surface.viewport_offset_lines as i32 + delta)
            .clamp(0, surface.viewport_max_offset_lines as i32) as u32;
        surface = bootstrap_surface_with_viewport(
            surface.session_id,
            surface.seqno.saturating_add(1),
            next_offset,
            surface.viewport_max_offset_lines,
        );
        *self
            .state
            .surface
            .lock()
            .expect("lock scroll projection surface") = Some(surface.clone());
        Ok(surface)
    }
}

impl SessionRuntimeControl for FollowProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        self.state.emit_live_surface(&format!("text {text}"));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        self.state.emit_live_surface(&format!("key {rendered}"));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.state.emit_live_surface(&format!("paste {text}"));
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        self.state.emit_live_surface("mouse");
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        let mut surface = self
            .state
            .surface
            .lock()
            .expect("lock follow projection surface")
            .clone()
            .expect("current follow projection surface");
        let next_offset = (surface.viewport_offset_lines as i32 + delta)
            .clamp(0, surface.viewport_max_offset_lines as i32) as u32;
        surface = bootstrap_surface_with_viewport(
            surface.session_id,
            surface.seqno,
            next_offset,
            surface.viewport_max_offset_lines,
        );
        *self
            .state
            .surface
            .lock()
            .expect("lock follow projection surface") = Some(surface.clone());
        Ok(surface)
    }
}

impl SessionRuntimeLauncher for StoredSecretProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let store = Arc::clone(&self.store);
        let message = self.message;
        Box::pin(async move {
            if profile
                .password
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Ok(());
            }

            let credential_ref = profile
                .credential_ref
                .as_deref()
                .ok_or_else(|| anyhow!(message))?;
            let bundle = load_secret_bundle(store.as_ref(), credential_ref)?;
            if bundle
                .password
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                Ok(())
            } else {
                Err(anyhow!(message))
            }
        })
    }
}

impl SessionRuntimeLauncher for RecordingLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .expect("lock recording launcher state")
                .launch_profiles
                .push(profile);
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .expect("lock recording launcher state")
                .probe_profiles
                .push(profile);
            Ok(())
        })
    }
}

impl SessionRuntimeLauncher for SlowOpeningLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        let launch_delay = self.launch_delay;
        Box::pin(async move {
            state
                .lock()
                .expect("lock slow opening launcher state")
                .launch_profiles
                .push(profile);
            tokio::time::sleep(launch_delay).await;
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let state = Arc::clone(&self.state);
        let probe_delay = self.probe_delay;
        Box::pin(async move {
            state
                .lock()
                .expect("lock slow opening launcher state")
                .probe_profiles
                .push(profile);
            tokio::time::sleep(probe_delay).await;
            Ok(())
        })
    }
}

impl PrivateKeyImporter for SuccessfulPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(Some(ImportedPrivateKey {
            path: self.path.clone(),
            content: self.content.into(),
        }))
    }
}

impl PrivateKeyImporter for CancelledPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(None)
    }
}

impl PrivateKeyImporter for FailingPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Err(anyhow!(self.message))
    }
}

impl CredentialStore for UnavailableCredentialStore {
    fn put_secret(&self, _key: &str, _value: &str) -> Result<()> {
        Err(anyhow!("system credential store unavailable"))
    }

    fn get_secret(&self, _key: &str) -> Result<Option<String>> {
        Err(anyhow!("system credential store unavailable"))
    }

    fn delete_secret(&self, _key: &str) -> Result<()> {
        Err(anyhow!("system credential store unavailable"))
    }
}

fn bind_with_fake_sessions(app: &AppWindow, asset_repo: Option<Rc<dyn AssetCatalogRepository>>) {
    bind_with_launcher(app, asset_repo, Arc::new(FakeLauncher));
}

fn bind_with_launcher(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        Arc::new(MemoryCredentialStore::default()),
    );
}

fn bind_with_launcher_and_credential_store(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        credential_store,
    );
}

fn bind_with_launcher_and_credential_store_and_private_key_importer(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        credential_store,
        private_key_importer,
    );
}

fn bind_with_vault_runtime(
    app: &AppWindow,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    vault_runtime: VaultRuntimeOptions,
) {
    bind_top_status_bar_with_injected_services_and_vault_runtime(
        app,
        None,
        default_platform_window_effects(),
        None,
        launcher,
        credential_store,
        Arc::new(CancelledPrivateKeyImporter),
        vault_runtime,
    );
}

#[test]
fn snippet_create_modal_projects_runtime_rows_through_window_callbacks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");

    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Deploy prod".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/api".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_snippet_asset_items();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().kind.as_str(), "snippet");
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Deploy prod");
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn snippet_edit_and_delete_actions_route_through_bootstrap_callbacks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let snippet_id = create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_context_menu_requested(
        snippet_id.clone().into(),
        "snippet".into(),
        96.0,
        160.0,
    );
    app.invoke_assets_context_menu_action_invoked("edit-snippet".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");
    assert_eq!(app.get_asset_snippet_modal_name().as_str(), "Deploy prod");
    assert_eq!(
        app.get_asset_snippet_modal_script().as_str(),
        "kubectl rollout restart deploy/api"
    );

    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Restart api".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/web".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_snippet_asset_items();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Restart api");

    app.invoke_asset_context_menu_requested(snippet_id.into(), "snippet".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-snippet".into());

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(
        app.get_asset_delete_confirm_target_label().as_str(),
        "Restart api"
    );

    app.invoke_confirm_delete_asset_requested();
    assert_eq!(app.get_snippet_asset_items().row_count(), 0);
}

#[test]
fn snippet_double_click_activation_defaults_to_paste() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PasteProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    let snippet_id = create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_selected(snippet_id.clone().into());
    app.invoke_asset_selected(snippet_id.into());
    flush_runtime_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "paste kubectl rollout restart deploy/api"
    );
}

fn sample_known_hosts_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-bootstrap-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_vault_runtime_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-runtime-{}-{}",
        label,
        Uuid::new_v4()
    ))
}

fn sample_vault_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "vault-bootstrap-smoke-salt".into(),
    }
}

fn sample_bootstrap_bundle_with_primary_and_mirror() -> BootstrapBundle {
    BootstrapBundle {
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
                remote_id: "remote-primary".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::S3Compatible,
                locator: BootstrapRemoteLocator::S3 {
                    bucket: "vault-bucket".into(),
                    prefix: "mica".into(),
                    endpoint: None,
                    region: Some("us-east-1".into()),
                    force_path_style: false,
                },
                credential_ref: Some("vault/bootstrap/remote-primary".into()),
                auth_kind: ProviderAuthKind::AwsStandardChain,
                last_health: None,
            },
            BootstrapRemoteConfig {
                remote_id: "remote-mirror".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitHubGist,
                locator: BootstrapRemoteLocator::GitHubGist {
                    gist_id: "gist-mirror".into(),
                },
                credential_ref: Some("vault/bootstrap/remote-mirror".into()),
                auth_kind: ProviderAuthKind::Pat,
                last_health: None,
            },
        ],
        auto_sync_enabled: false,
        ..BootstrapBundle::default()
    }
}

fn sample_vault_asset_tree(host: &str) -> (AssetTree, String) {
    let mut tree = AssetTree::new();
    let credential_ref = format!("ssh/saved-secrets/imported-{host}");
    tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Imported Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: host.into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: Some(credential_ref.clone()),
        }),
    );
    (tree, credential_ref)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn sample_remote_revision_for_tree(
    password: &SecretString,
    asset_tree: &AssetTree,
    credential_store: &dyn CredentialStore,
    revision: &str,
) -> ProviderRevision {
    let snapshot = export_vault_snapshot(
        asset_tree,
        &KeychainCatalog::default(),
        credential_store,
        &sample_known_hosts_path("remote-revision"),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .expect("export vault snapshot");
    let vault_key = generate_vault_key();
    let encrypted_snapshot = encrypt_snapshot(&snapshot, &vault_key).expect("encrypt snapshot");
    let manifest = VaultManifest {
        packs: vec![PackRef {
            pack_id: format!("pack-{revision}"),
            object_name: format!("bundle/{revision}/snapshot.bin"),
            size_bytes: encrypted_snapshot.ciphertext.len() as u64,
            digest: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        }],
        provider_capability_fallbacks: BTreeMap::from([
            (
                "snapshot.nonce_hex".into(),
                hex(encrypted_snapshot.nonce.as_slice()),
            ),
            (
                "snapshot.plaintext_len".into(),
                encrypted_snapshot.plaintext_len.to_string(),
            ),
            (
                "snapshot.compressed_len".into(),
                encrypted_snapshot.compressed_len.to_string(),
            ),
            (
                "snapshot.payload_sha256".into(),
                encrypted_snapshot.payload_sha256.clone(),
            ),
        ]),
        ..VaultManifest::default()
    };
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0003".into()),
        device_id: "device-remote".into(),
        committed_at: "2026-03-31T10:00:00Z".into(),
        committed_by_device: "device-remote".into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: serde_json::to_string(
            &wrap_vault_key(password, &sample_vault_kdf(), &vault_key)
                .expect("wrap remote vault key"),
        )
        .expect("encode wrapped vault key"),
        kdf: sample_vault_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    };

    ProviderRevision {
        head,
        manifest,
        encrypted_snapshot,
    }
}

fn sample_remote_revision_for_existing_vault_key(
    asset_tree: &AssetTree,
    credential_store: &dyn CredentialStore,
    revision: &str,
    vault_key: &[u8; 32],
    wrapped_vault_key: &str,
    kdf: &KdfConfig,
) -> ProviderRevision {
    let snapshot = export_vault_snapshot(
        asset_tree,
        &KeychainCatalog::default(),
        credential_store,
        &sample_known_hosts_path("remote-revision"),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .expect("export vault snapshot");
    let encrypted_snapshot = encrypt_snapshot(&snapshot, vault_key).expect("encrypt snapshot");
    let manifest = VaultManifest {
        packs: vec![PackRef {
            pack_id: format!("pack-{revision}"),
            object_name: format!("bundle/{revision}/snapshot.bin"),
            size_bytes: encrypted_snapshot.ciphertext.len() as u64,
            digest: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        }],
        provider_capability_fallbacks: BTreeMap::from([
            (
                "snapshot.nonce_hex".into(),
                hex(encrypted_snapshot.nonce.as_slice()),
            ),
            (
                "snapshot.plaintext_len".into(),
                encrypted_snapshot.plaintext_len.to_string(),
            ),
            (
                "snapshot.compressed_len".into(),
                encrypted_snapshot.compressed_len.to_string(),
            ),
            (
                "snapshot.payload_sha256".into(),
                encrypted_snapshot.payload_sha256.clone(),
            ),
        ]),
        ..VaultManifest::default()
    };
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0003".into()),
        device_id: "device-remote".into(),
        committed_at: "2026-03-31T10:00:00Z".into(),
        committed_by_device: "device-remote".into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: wrapped_vault_key.into(),
        kdf: kdf.clone(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    };

    ProviderRevision {
        head,
        manifest,
        encrypted_snapshot,
    }
}

fn sample_credential_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-bootstrap-credentials-{}-{}",
        label,
        Uuid::new_v4()
    ))
}

fn bootstrap_surface_with_viewport(
    session_id: uuid::Uuid,
    seqno: usize,
    offset: u32,
    max_offset: u32,
) -> TerminalSurfaceState {
    let mut surface = TerminalSurfaceState::from_visible_lines(
        session_id,
        seqno,
        24,
        80,
        vec![format!("offset {offset}")],
    );
    surface.viewport_offset_lines = offset;
    surface.viewport_max_offset_lines = max_offset;
    surface.viewport_at_bottom = offset == 0;
    surface.default_fg_rgba = 0xff1f_2328;
    surface.default_bg_rgba = 0xfff7_f9fc;
    surface.cursor.fg_rgba = 0xfff7_f9fc;
    surface.cursor.bg_rgba = 0xff4b_5058;
    surface
}

fn terminal_surface_with_cells(
    session_id: uuid::Uuid,
    seqno: usize,
    rows: u32,
    cols: u32,
    visible_lines: Vec<String>,
) -> TerminalSurfaceState {
    let mut session = TerminalSession::new(rows as usize, cols as usize);
    let transcript = visible_lines
        .iter()
        .map(|line| format!("{line}\r\n"))
        .collect::<String>();
    session.apply_remote_bytes(transcript.as_bytes());

    let mut surface = session.surface_state(session_id);
    surface.seqno = seqno;
    surface
}

fn settle_terminal_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn settle_sync_scheduler(delay: Duration) {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(delay);
    slint::platform::update_timers_and_animations();
}

fn terminal_interaction_position(app: &AppWindow) -> LogicalPosition {
    LogicalPosition::new(
        app.get_layout_main_workspace_x() + 96.0,
        app.get_layout_titlebar_height() + 96.0,
    )
}

fn dispatch_modifier_pressed(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: modifier.into(),
    });
}

fn dispatch_modifier_released(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: modifier.into(),
    });
}

fn dispatch_shared_key_chord(
    app: &AppWindow,
    key_text: slint::SharedString,
    ctrl: bool,
    shift: bool,
    alt: bool,
) {
    if shift {
        dispatch_modifier_pressed(app, Key::Shift);
    }
    if ctrl {
        dispatch_modifier_pressed(app, Key::Control);
    }
    if alt {
        dispatch_modifier_pressed(app, Key::Alt);
    }

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: key_text.clone(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key_text });

    if alt {
        dispatch_modifier_released(app, Key::Alt);
    }
    if ctrl {
        dispatch_modifier_released(app, Key::Control);
    }
    if shift {
        dispatch_modifier_released(app, Key::Shift);
    }
}

fn dispatch_text_key_chord(app: &AppWindow, key_text: &str, ctrl: bool, shift: bool, alt: bool) {
    dispatch_shared_key_chord(app, key_text.into(), ctrl, shift, alt);
}

fn dispatch_named_key_chord(app: &AppWindow, key_name: &str, ctrl: bool, shift: bool, alt: bool) {
    let key_text = match key_name {
        "left" => Key::LeftArrow.into(),
        "right" => Key::RightArrow.into(),
        "up" => Key::UpArrow.into(),
        "down" => Key::DownArrow.into(),
        "home" => Key::Home.into(),
        "end" => Key::End.into(),
        "insert" => Key::Insert.into(),
        "page-up" => Key::PageUp.into(),
        "page-down" => Key::PageDown.into(),
        other => panic!("unsupported named key `{other}`"),
    };
    dispatch_shared_key_chord(app, key_text, ctrl, shift, alt);
}

fn dispatch_function_key(app: &AppWindow, number: u8) {
    let key_text = match number {
        1 => Key::F1.into(),
        2 => Key::F2.into(),
        3 => Key::F3.into(),
        4 => Key::F4.into(),
        5 => Key::F5.into(),
        6 => Key::F6.into(),
        7 => Key::F7.into(),
        8 => Key::F8.into(),
        9 => Key::F9.into(),
        10 => Key::F10.into(),
        11 => Key::F11.into(),
        12 => Key::F12.into(),
        13 => Key::F13.into(),
        14 => Key::F14.into(),
        15 => Key::F15.into(),
        16 => Key::F16.into(),
        17 => Key::F17.into(),
        18 => Key::F18.into(),
        19 => Key::F19.into(),
        20 => Key::F20.into(),
        21 => Key::F21.into(),
        22 => Key::F22.into(),
        23 => Key::F23.into(),
        24 => Key::F24.into(),
        other => panic!("unsupported function key F{other}"),
    };
    dispatch_shared_key_chord(app, key_text, false, false, false);
}

fn focus_workspace_terminal(app: &AppWindow) {
    let position = terminal_interaction_position(app);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn select_terminal_welcome_span(app: &AppWindow) {
    let selection_start = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 0.5),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );
    let selection_end = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 10.5),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );

    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: selection_start,
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: selection_start,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: selection_end,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: selection_end,
        button: PointerEventButton::Left,
    });
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti bootstrap-tofu@example.com",
    )
    .expect("parse public key")
}

fn sample_logging_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join(format!("{label}-{}", uuid::Uuid::new_v4()))
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) -> String {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    app.get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string()
}

fn create_root_snippet(app: &AppWindow, name: &str, script: &str) -> String {
    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());
    app.invoke_asset_snippet_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_snippet_modal_draft_changed("script".into(), script.into());
    app.invoke_confirm_asset_modal_requested();

    app.get_snippet_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn flush_runtime_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn lock_known_hosts_env() -> std::sync::MutexGuard<'static, ()> {
    KNOWN_HOSTS_ENV_LOCK
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

#[test]
fn settings_panel_can_create_a_vault_and_persist_local_bootstrap_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("create");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("correct horse battery staple".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert!(
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .is_some()
    );
}

#[test]
fn missing_local_vault_state_recovers_from_primary_remote_without_uploading_empty_data() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("recover-remote");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let remote_revision =
        sample_remote_revision_for_tree(&password, &asset_tree, source_store.as_ref(), "rev-0004");
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision.clone()));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
    assert_eq!(primary.recorded_writes().len(), 0);

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("recovered local bootstrap state");
    assert_eq!(local_state.current_revision.as_deref(), Some("rev-0004"));
    assert_eq!(local_state.bundle.vault_id, "vault-main");
    assert_eq!(
        local_state.wrapped_vault_key,
        remote_revision.head.wrapped_vault_key
    );

    let cached = load_encrypted_cache(&temp_root.join("cache"), "vault-main")
        .unwrap()
        .expect("recovered encrypted cache");
    assert_eq!(
        cached.payload_sha256,
        remote_revision.encrypted_snapshot.payload_sha256
    );
}

#[test]
fn missing_local_vault_state_surfaces_legacy_remote_as_unrecoverable() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("recover-legacy-remote");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.100");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let mut remote_revision =
        sample_remote_revision_for_tree(&password, &asset_tree, source_store.as_ref(), "rev-0004");
    remote_revision
        .manifest
        .provider_capability_fallbacks
        .clear();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("legacy remote revision"),
        "unexpected error: {}",
        app.get_sync_modal_error_text()
    );
    assert!(
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .is_none()
    );
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn unlocking_existing_vault_restores_cached_snapshot_without_loading_while_locked() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("unlock");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.42");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();
    let known_hosts_path = sample_known_hosts_path("vault-unlock");
    let snapshot = export_vault_snapshot(
        &asset_tree,
        &KeychainCatalog::default(),
        source_store.as_ref(),
        &known_hosts_path,
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .unwrap();
    let vault_key = generate_vault_key();
    let encrypted = encrypt_snapshot(&snapshot, &vault_key).unwrap();
    let wrapped_vault_key =
        serde_json::to_string(&wrap_vault_key(&password, &sample_vault_kdf(), &vault_key).unwrap())
            .unwrap();
    save_local_vault_bootstrap_state(
        &temp_root.join("vault-bootstrap-state.json"),
        &LocalVaultBootstrapState {
            bundle: sample_bootstrap_bundle_with_primary_and_mirror(),
            wrapped_vault_key,
            kdf: sample_vault_kdf(),
            current_revision: Some("rev-0001".into()),
            local_snapshot_hash: Some(format!("sha256:{}", encrypted.payload_sha256)),
            last_local_change_at: Some("2026-03-31T10:00:00Z".into()),
            last_successful_push_at: None,
            last_successful_pull_at: None,
            last_sync_error: None,
        },
    )
    .unwrap();
    store_encrypted_cache(&temp_root.join("cache"), "vault-main", &encrypted).unwrap();

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
        },
    );

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_none()
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn enabling_sync_persists_runtime_vault_key_material() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("runtime-key-persist");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    let local_state = load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
        .unwrap()
        .expect("persisted local bootstrap state");
    let runtime_key = load_runtime_vault_key(credential_store.as_ref(), &local_state.bundle.vault_id)
        .expect("load persisted runtime vault key");

    assert!(runtime_key.is_some());
    assert_ne!(runtime_key.expect("runtime key"), [0u8; 32]);
}

#[test]
fn restart_recovers_vault_session_without_prompting_for_unlock() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("restart-runtime-key");
    let initial_provider_factory = RecordingVaultProviderFactory::default();
    initial_provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));

    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(initial_provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    let asset_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let credential_ref = ssh_credential_ref(&asset_id, SshCredentialKind::SavedSecrets);
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    let restarted = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &restarted,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
        },
    );

    assert_eq!(restarted.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );

    restarted.invoke_open_sync_modal_requested();
    assert_eq!(restarted.get_sync_modal_mode().as_str(), "ready");
}

#[test]
fn recovery_pull_persists_local_snapshot_before_remote_replacement() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("recovery-pull-before-replace");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    assert_eq!(primary.recorded_writes().len(), 1);

    create_root_ssh(&app, "DB Replica", "10.0.0.24");
    let remote_store = Arc::new(MemoryCredentialStore::default());
    let (remote_tree, remote_credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        remote_store.as_ref(),
        &remote_credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();
    let local_state = load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
        .unwrap()
        .expect("local bootstrap state after first sync");
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_revision = sample_remote_revision_for_existing_vault_key(
        &remote_tree,
        remote_store.as_ref(),
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    app.invoke_sync_modal_sync_now_requested();

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "newer remote data should pull instead of overwriting the primary head"
    );
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&remote_credential_ref)
            .unwrap()
            .is_some()
    );

    let recovery_entries = load_recovery_snapshots(temp_root.join("recovery").as_path(), "vault-main")
        .expect("load persisted recovery snapshots");
    assert_eq!(recovery_entries.len(), 1);
    assert_eq!(recovery_entries[0].source, RecoverySource::LocalBeforePull);
    assert!(
        recovery_entries[0]
            .snapshot
            .asset_catalog
            .nodes
            .values()
            .any(|node| matches!(
                &node.payload,
                VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.24"
            )),
        "local recovery snapshot should preserve the pending local SSH asset before pull"
    );
}

#[test]
fn unlocking_existing_vault_with_auto_sync_enabled_waits_for_a_real_mutation() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("unlock-auto-sync");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle.auto_sync_enabled = true;
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(primary.recorded_writes().len(), 0);

    app.invoke_sync_modal_close_requested();
    app.invoke_open_sync_modal_requested();

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(primary.recorded_writes().len(), 0);
}

#[test]
fn asset_mutation_syncs_without_auto_sync_toggle() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("asset-auto-sync");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();
    assert_eq!(primary.recorded_writes().len(), 0);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(primary.recorded_writes().len(), 0);
    settle_sync_scheduler(Duration::from_millis(1300));
    assert_eq!(primary.recorded_writes().len(), 1);

    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved folder asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();
    assert_eq!(primary.recorded_writes().len(), 1);
    settle_sync_scheduler(Duration::from_millis(1300));
    assert_eq!(primary.recorded_writes().len(), 2);

    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();
    assert_eq!(primary.recorded_writes().len(), 2);
    settle_sync_scheduler(Duration::from_millis(1300));
    assert_eq!(primary.recorded_writes().len(), 3);
}

#[test]
fn periodic_sync_pulls_remote_changes_even_without_local_dirty_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("periodic-pull-clean-local");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    app.invoke_sync_modal_close_requested();

    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(app.get_console_asset_items().row_count(), 0);

    let remote_store = Arc::new(MemoryCredentialStore::default());
    let (remote_tree, remote_credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        remote_store.as_ref(),
        &remote_credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();
    let local_state = load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
        .unwrap()
        .expect("local bootstrap state after initial sync");
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_revision = sample_remote_revision_for_existing_vault_key(
        &remote_tree,
        remote_store.as_ref(),
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    settle_sync_scheduler(Duration::from_secs(121));

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "periodic sync should pull clean remote changes instead of pushing a new head"
    );
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&remote_credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn back_to_back_mutations_share_one_debounced_auto_sync_upload() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("debounced-auto-sync");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");
    create_root_snippet(&app, "Restart api", "kubectl rollout restart deploy/web");

    assert_eq!(primary.recorded_writes().len(), 0);
    settle_sync_scheduler(Duration::from_millis(1300));

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "two quick local mutations should collapse into one debounced upload"
    );
}

#[test]
fn periodic_auto_sync_retries_failed_dirty_changes() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("periodic-auto-sync-retry");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_write_error(Some("temporary outage"));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");
    settle_sync_scheduler(Duration::from_millis(1300));
    assert_eq!(primary.recorded_writes().len(), 0);

    primary.set_write_error(None);
    settle_sync_scheduler(Duration::from_secs(121));

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "periodic sync should retry a dirty local change after the provider becomes writable again"
    );
}

#[test]
fn manual_vault_sync_reports_mirror_degradation_after_primary_commit() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("mirror-degraded");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    mirror.set_write_error(Some("mirror unavailable"));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    provider_factory.insert(mirror.clone());

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    app.invoke_sync_modal_sync_now_requested();

    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(mirror.recorded_writes().len(), 0);
    assert!(
        app.get_sync_modal_status_text()
            .as_str()
            .contains("mirror unavailable")
    );
}

#[test]
fn manual_vault_sync_surfaces_provider_auth_errors_in_panel_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("provider-auth");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    provider_factory.insert(mirror);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    primary.set_read_error(Some("token expired"));

    app.invoke_sync_modal_sync_now_requested();

    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("token expired")
    );
}

#[test]
fn locking_vault_clears_decrypted_assets_and_secrets_from_memory() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("lock");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let credential_ref = ssh_credential_ref(&ssh_id, SshCredentialKind::SavedSecrets);
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );

    app.invoke_sync_modal_close_requested();

    assert!(!app.get_sync_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn locking_and_unlocking_vault_round_trips_snippet_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("snippet-lock");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    create_root_snippet(&app, "Restart API", "kubectl rollout restart deploy/api");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_snippet_asset_items().row_count(), 1);

    app.invoke_sync_modal_close_requested();
    assert!(!app.get_sync_modal_open());
    assert_eq!(app.get_snippet_asset_items().row_count(), 1);

    app.invoke_open_sync_modal_requested();
    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_snippet_asset_items().row_count(), 1);
    assert_eq!(
        app.get_snippet_asset_items()
            .row_data(0)
            .expect("snippet row after unlock")
            .kind
            .as_str(),
        "snippet"
    );
}

fn loaded_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["folder-root".into(), "ssh-root".into()],
        nodes: BTreeMap::from([
            (
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                "ssh-root".into(),
                PersistedAssetNode {
                    id: "ssh-root".into(),
                    parent_id: None,
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "2022".into(),
                        auth_method: "password".into(),
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
        ]),
    }
}

fn loaded_catalog_with_snippets_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec![
            "folder-root".into(),
            "ssh-root".into(),
            "snippet-package-root".into(),
            "snippet-root".into(),
        ],
        nodes: BTreeMap::from([
            (
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                "ssh-root".into(),
                PersistedAssetNode {
                    id: "ssh-root".into(),
                    parent_id: None,
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "2022".into(),
                        auth_method: "password".into(),
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
            (
                "snippet-package-root".into(),
                PersistedAssetNode {
                    id: "snippet-package-root".into(),
                    parent_id: None,
                    title: "Deploy".into(),
                    kind: PersistedAssetKind::SnippetPackage,
                    child_ids: vec!["snippet-child".into()],
                    payload: PersistedAssetPayload::SnippetPackage,
                },
            ),
            (
                "snippet-child".into(),
                PersistedAssetNode {
                    id: "snippet-child".into(),
                    parent_id: Some("snippet-package-root".into()),
                    title: "Deploy prod".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl apply -f prod.yaml".into(),
                        package_id: Some("snippet-package-root".into()),
                    }),
                },
            ),
            (
                "snippet-root".into(),
                PersistedAssetNode {
                    id: "snippet-root".into(),
                    parent_id: None,
                    title: "Restart API".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl rollout restart deploy/api".into(),
                        package_id: None,
                    }),
                },
            ),
        ]),
    }
}

fn loaded_legacy_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-legacy".into()],
        nodes: BTreeMap::from([(
            "ssh-legacy".into(),
            PersistedAssetNode {
                id: "ssh-legacy".into(),
                parent_id: None,
                title: "Legacy Gateway".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "legacy.example.com".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: String::new(),
                    auth_source: String::new(),
                    keychain_identity_id: None,
                    private_key_source: String::new(),
                    private_key_path: String::new(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: String::new(),
                    credential_ref: None,
                }),
            },
        )]),
    }
}

fn loaded_saved_password_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-prod".into()],
        nodes: BTreeMap::from([(
            "ssh-prod".into(),
            PersistedAssetNode {
                id: "ssh-prod".into(),
                parent_id: None,
                title: "Prod Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.12".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Saved credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-prod".into()),
                }),
            },
        )]),
    }
}

fn loaded_keychain_identity_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-identity".into()],
        nodes: BTreeMap::from([(
            "ssh-identity".into(),
            PersistedAssetNode {
                id: "ssh-identity".into(),
                parent_id: None,
                title: "Identity Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.99".into(),
                    user: String::new(),
                    port: "22".into(),
                    auth_method: String::new(),
                    auth_source: "keychain-identity".into(),
                    keychain_identity_id: Some("identity-prod".into()),
                    private_key_source: String::new(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Identity-backed".into(),
                    credential_ref: None,
                }),
            },
        )]),
    }
}

fn loaded_saved_private_key_path_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-path".into()],
        nodes: BTreeMap::from([(
            "ssh-path".into(),
            PersistedAssetNode {
                id: "ssh-path".into(),
                parent_id: None,
                title: "Path Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.20".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "private-key".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "path".into(),
                    private_key_path: "/tmp/id_ed25519".into(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Saved passphrase".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-path".into()),
                }),
            },
        )]),
    }
}

fn loaded_saved_socks5_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-socks5".into()],
        nodes: BTreeMap::from([(
            "ssh-socks5".into(),
            PersistedAssetNode {
                id: "ssh-socks5".into(),
                parent_id: None,
                title: "SOCKS5 Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.30".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                        host: "proxy.example.net".into(),
                        port: "1080".into(),
                        username: "ops-proxy".into(),
                        password_credential_ref: Some("ssh/saved-secrets/ssh-socks5".into()),
                    }),
                    remark: "Saved proxy credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-socks5".into()),
                }),
            },
        )]),
    }
}

fn loaded_saved_upstream_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-upstream".into(), "ssh-target".into()],
        nodes: BTreeMap::from([
            (
                "ssh-upstream".into(),
                PersistedAssetNode {
                    id: "ssh-upstream".into(),
                    parent_id: None,
                    title: "Upstream Bastion".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "10.0.0.40".into(),
                        user: "ops".into(),
                        port: "22".into(),
                        auth_method: "password".into(),
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
            (
                "ssh-target".into(),
                PersistedAssetNode {
                    id: "ssh-target".into(),
                    parent_id: None,
                    title: "Target Bastion".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "10.0.0.41".into(),
                        user: "ops".into(),
                        port: "22".into(),
                        auth_method: "password".into(),
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::SshAsset {
                            asset_id: "ssh-upstream".into(),
                        },
                        remark: "Saved upstream reference".into(),
                        credential_ref: None,
                    }),
                },
            ),
        ]),
    }
}

fn loaded_saved_http_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-http".into()],
        nodes: BTreeMap::from([(
            "ssh-http".into(),
            PersistedAssetNode {
                id: "ssh-http".into(),
                parent_id: None,
                title: "HTTP Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.35".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::Http(PersistedAssetSocks5ProxySpec {
                        host: "proxy.example.net".into(),
                        port: "8080".into(),
                        username: "ops-proxy".into(),
                        password_credential_ref: Some("ssh/saved-secrets/ssh-http".into()),
                    }),
                    remark: "Saved HTTP proxy credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-http".into()),
                }),
            },
        )]),
    }
}

fn loaded_missing_upstream_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-missing-upstream".into()],
        nodes: BTreeMap::from([(
            "ssh-missing-upstream".into(),
            PersistedAssetNode {
                id: "ssh-missing-upstream".into(),
                parent_id: None,
                title: "Broken Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.50".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::SshAsset {
                        asset_id: "ssh-upstream-missing".into(),
                    },
                    remark: "Missing upstream reference".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-missing-upstream".into()),
                }),
            },
        )]),
    }
}

fn context_menu_item_enabled(app: &AppWindow, action_id: &str) -> bool {
    let items = app.get_assets_context_menu_primary_items();
    (0..items.row_count())
        .filter_map(|index| items.row_data(index))
        .find(|item| item.id.as_str() == action_id)
        .map(|item| item.enabled)
        .unwrap_or(false)
}

#[test]
fn bootstrap_exposes_shell_default_window_budget() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(
        default_window_size(),
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
    );
}

#[test]
fn bootstrap_shared_credential_store_prefers_encrypted_cache_when_preferred_store_is_unavailable() {
    let encrypted_root = sample_credential_root("secure");
    let recovery_root = sample_credential_root("recovery");
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);
    let store = build_shared_app_credential_store_for_paths(
        Some(Arc::new(UnavailableCredentialStore) as Arc<dyn CredentialStore>),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    persist_secret_bundle(
        store.as_ref(),
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist bundle through shared bootstrap store");

    assert_eq!(
        load_secret_bundle(store.as_ref(), credential_ref.as_str())
            .expect("reload shared credential bundle")
            .password
            .as_deref(),
        Some("super-secret")
    );

    let encrypted_bytes = fs::read(
        encrypted_root
            .join("ssh")
            .join("saved-secrets")
            .join("asset-prod.bin"),
    )
    .expect("read encrypted fallback file");
    assert!(!String::from_utf8_lossy(&encrypted_bytes).contains("super-secret"));
    assert!(
        !recovery_root
            .join("ssh")
            .join("saved-secrets")
            .join("asset-prod.json")
            .exists(),
        "plain recovery store should not be used when encrypted fallback succeeds"
    );

    let _ = fs::remove_dir_all(encrypted_root);
    let _ = fs::remove_dir_all(recovery_root);
}

#[test]
fn bootstrap_shared_credential_store_reloads_saved_secret_when_system_store_is_empty_after_restart()
{
    let encrypted_root = sample_credential_root("secure-restart");
    let recovery_root = sample_credential_root("recovery-restart");
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);
    let first_primary = Arc::new(MemoryCredentialStore::default()) as Arc<dyn CredentialStore>;
    let first_store = build_shared_app_credential_store_for_paths(
        Some(first_primary),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    persist_secret_bundle(
        first_store.as_ref(),
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist bundle through initial shared bootstrap store");

    let second_store = build_shared_app_credential_store_for_paths(
        Some(Arc::new(MemoryCredentialStore::default()) as Arc<dyn CredentialStore>),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    assert_eq!(
        load_secret_bundle(second_store.as_ref(), credential_ref.as_str())
            .expect("reload shared credential bundle after restart")
            .password
            .as_deref(),
        Some("super-secret")
    );

    let _ = fs::remove_dir_all(encrypted_root);
    let _ = fs::remove_dir_all(recovery_root);
}

#[test]
fn bootstrap_loads_catalog_before_first_asset_projection_sync() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Team");
    assert_eq!(rows.row_data(1).unwrap().label.as_str(), "Gateway");

    let state = repo_state.borrow();
    assert_eq!(state.load_calls, 1);
    assert!(state.save_attempts.is_empty());
}

#[test]
fn bootstrap_loads_snippets_from_repository_without_leaking_them_into_console_projection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_with_snippets_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    assert_eq!(repo_state.borrow().load_calls, 1);
    assert_eq!(app.get_console_asset_items().row_count(), 2);
    assert_eq!(app.get_snippet_asset_items().row_count(), 2);
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("console folder row")
            .kind
            .as_str(),
        "folder"
    );
    assert_eq!(
        app.get_snippet_asset_items()
            .row_data(0)
            .expect("snippet package row")
            .kind
            .as_str(),
        "snippet-package"
    );
}

#[test]
fn unrelated_catalog_saves_preserve_keychain_identity_host_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_keychain_identity_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Scratch".into());
    app.invoke_confirm_asset_modal_requested();

    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after unrelated save")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-identity")
        .expect("persisted keychain-backed ssh node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.auth_source, "keychain-identity");
    assert_eq!(spec.keychain_identity_id.as_deref(), Some("identity-prod"));
}

#[test]
fn activating_legacy_saved_ssh_asset_defaults_missing_auth_fields_and_opens_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_legacy_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("legacy ssh asset")
        .id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
}

#[test]
fn opening_slow_saved_ssh_asset_returns_before_probe_completes() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(SlowOpeningLauncher {
            state: Arc::clone(&launcher_state),
            probe_delay: Duration::from_millis(250),
            launch_delay: Duration::from_millis(250),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let started = Instant::now();
    app.invoke_asset_activated(ssh_id.into());

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "opening a workspace SSH asset should not block on probe_connection()"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
    assert!(
        launcher_state
            .lock()
            .expect("lock slow opening launcher state")
            .probe_profiles
            .is_empty(),
        "opening a workspace SSH asset should create the tab before any synchronous probe runs"
    );
}

#[test]
fn opening_saved_ssh_asset_twice_creates_two_tabs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn editing_legacy_saved_ssh_asset_reuses_fallback_saved_secret_for_test_connection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_legacy_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        &ssh_credential_ref("ssh-legacy", SshCredentialKind::SavedSecrets),
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist legacy saved ssh secret");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(StoredSecretProbeLauncher {
            store: Arc::clone(&credential_store),
            message: "missing SSH password secret for `Legacy Gateway`",
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("legacy ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());

    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );
}

#[test]
fn editing_saved_password_modal_hydrates_real_secret_masked() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_password_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-prod",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh secret");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());
}

#[test]
fn editing_saved_socks5_modal_hydrates_proxy_fields_and_proxy_password() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_socks5_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-socks5",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: Some("proxy-secret".into()),
        },
    )
    .expect("persist saved socks5 secret bundle");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved socks5 ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "socks5");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_host().as_str(),
        "proxy.example.net"
    );
    assert_eq!(app.get_asset_ssh_modal_proxy_socks5_port().as_str(), "1080");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_username().as_str(),
        "ops-proxy"
    );
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_password().as_str(),
        "proxy-secret"
    );
    assert!(!app.get_asset_ssh_modal_proxy_socks5_password_visible());
}

#[test]
fn editing_saved_upstream_ssh_modal_projects_selected_upstream_asset_id() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    bind_with_fake_sessions(&app, Some(asset_repo));

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .expect("target ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "ssh-asset");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_ssh_asset_id().as_str(),
        "ssh-upstream"
    );
}

#[test]
fn editing_saved_upstream_ssh_modal_excludes_current_asset_from_dropdown_options() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    bind_with_fake_sessions(&app, Some(asset_repo));

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .expect("target ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    let options = app.get_asset_ssh_modal_proxy_ssh_options();
    assert_eq!(options.row_count(), 1);
    assert_eq!(options.row_data(0).unwrap().as_str(), "Upstream Bastion");
}

#[test]
fn editing_saved_http_modal_hydrates_proxy_fields_and_proxy_password() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_http_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-http",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: Some("proxy-secret".into()),
        },
    )
    .expect("persist saved http secret bundle");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved http ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "http");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_host().as_str(),
        "proxy.example.net"
    );
    assert_eq!(app.get_asset_ssh_modal_proxy_socks5_port().as_str(), "8080");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_username().as_str(),
        "ops-proxy"
    );
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_password().as_str(),
        "proxy-secret"
    );
    assert!(!app.get_asset_ssh_modal_proxy_socks5_password_visible());
}

#[test]
fn test_connection_with_missing_upstream_reports_inline_feedback_without_probe() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_missing_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-missing-upstream",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist missing-upstream ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("broken ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "upstream SSH asset `ssh-upstream-missing` was not found"
    );
}

#[test]
fn connect_with_missing_upstream_reports_inline_feedback_without_launch() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_missing_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-missing-upstream",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist missing-upstream ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("broken ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "upstream SSH asset `ssh-upstream-missing` was not found"
    );
}

#[test]
fn saved_password_asset_rehydrates_after_rebinding_with_same_store() {
    i_slint_backend_testing::init_no_event_loop();

    let shared_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let first_app = AppWindow::new().unwrap();
    let first_repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let first_asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&first_repo_state),
        None,
    ));
    bind_with_launcher_and_credential_store(
        &first_app,
        Some(first_asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&shared_store),
    );

    first_app.invoke_assets_create_action_selected("new-ssh-connection".into());
    first_app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    first_app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    first_app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    first_app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    first_app.invoke_asset_ssh_modal_action_requested("save".into());

    let rebound_catalog = first_repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("saved catalog snapshot")
        .clone();

    let second_app = AppWindow::new().unwrap();
    let second_repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let second_asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        rebound_catalog,
        Rc::clone(&second_repo_state),
        None,
    ));
    bind_with_launcher_and_credential_store(
        &second_app,
        Some(second_asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&shared_store),
    );

    let ssh_id = second_app
        .get_console_asset_items()
        .row_data(0)
        .expect("rebound ssh asset")
        .id
        .to_string();

    second_app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    second_app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(second_app.get_asset_modal_open());
    assert_eq!(second_app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!second_app.get_asset_ssh_modal_password_visible());
}

#[test]
fn editing_saved_private_key_path_modal_hydrates_saved_passphrase() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-path",
        &StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh passphrase");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_path().as_str(),
        "/tmp/id_ed25519"
    );
    assert_eq!(app.get_asset_ssh_modal_passphrase().as_str(), "hunter2");
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_private_key_content().as_str(), "");
}

#[test]
fn editing_saved_private_key_path_modal_saving_blank_passphrase_deletes_saved_passphrase() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-path",
        &StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh passphrase");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("passphrase".into(), "".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert!(
        load_secret_bundle(credential_store.as_ref(), "ssh/saved-secrets/ssh-path")
            .expect("load cleared saved secret")
            .is_empty()
    );
    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after clear")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-path")
        .expect("saved ssh path node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.credential_ref, None);
}

#[test]
fn importing_private_key_into_saved_path_asset_migrates_it_to_content_mode_on_save() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(SuccessfulPrivateKeyImporter {
            path: std::path::PathBuf::from("/tmp/id_ed25519"),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n",
        }),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(
        app.get_asset_ssh_modal_private_key_content().as_str(),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n"
    );

    app.invoke_asset_ssh_modal_action_requested("save".into());

    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after import")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-path")
        .expect("saved ssh path node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.auth_method, "private-key");
    assert_eq!(spec.private_key_source, "content");
    assert_eq!(spec.private_key_path, "");
    assert_eq!(
        spec.credential_ref.as_deref(),
        Some("ssh/saved-secrets/ssh-path")
    );

    let bundle = load_secret_bundle(credential_store.as_ref(), "ssh/saved-secrets/ssh-path")
        .expect("load imported secret bundle");
    assert_eq!(
        bundle.private_key_content.as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n")
    );
}

#[test]
fn importing_private_key_can_be_cancelled_without_mutating_modal_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(CancelledPrivateKeyImporter),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_ssh_modal_private_key_content().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "idle");
    assert_eq!(app.get_asset_ssh_modal_feedback_message().as_str(), "");
}

#[test]
fn manual_ssh_modal_private_key_import_still_populates_inline_content() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(SuccessfulPrivateKeyImporter {
            path: std::path::PathBuf::from("/tmp/id_ed25519"),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n",
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(
        app.get_asset_ssh_modal_private_key_content().as_str(),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n"
    );
    assert_eq!(
        app.get_asset_ssh_modal_auth_method().as_str(),
        "private-key"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_source().as_str(),
        "content"
    );
}

#[test]
fn importing_private_key_reports_feedback_when_file_selection_fails() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(FailingPrivateKeyImporter {
            message: "failed to read private key file",
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "failed to read private key file"
    );
}

#[test]
fn create_rename_delete_and_ssh_edit_trigger_repository_save() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_toggle_assets_view_mode_requested();
    assert!(repo_state.borrow().save_attempts.is_empty());
    app.invoke_toggle_assets_view_mode_requested();
    app.invoke_assets_search_query_changed("".into());
    app.invoke_close_assets_search_requested();

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 1);

    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "socks5".into());
    app.invoke_asset_ssh_modal_draft_changed(
        "proxy_socks5_host".into(),
        "proxy.example.net".into(),
    );
    app.invoke_asset_ssh_modal_draft_changed("proxy_socks5_port".into(), "1080".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 2);

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 3);

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .unwrap()
        .id
        .to_string();
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 4);
}

#[test]
fn snippet_create_persists_into_repository_catalog_alongside_console_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());
    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Restart API".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/api".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    assert!(
        save_attempts[0]
            .nodes
            .values()
            .any(|node| node.kind == PersistedAssetKind::Snippet)
    );
    assert!(
        save_attempts[0]
            .nodes
            .values()
            .any(|node| node.kind == PersistedAssetKind::Folder)
    );
}

#[test]
fn saving_self_referential_upstream_proxy_is_blocked_before_runtime_launch() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_password_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-prod",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist existing ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "ssh-asset".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_ssh_asset_id".into(), ssh_id.into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(repo_state.borrow().save_attempts.is_empty());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Upstream SSH connection cannot reference itself."
    );
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "idle");
}

#[test]
fn save_failure_logs_error_without_persisting_ui_session_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("assets-persistence-save-error");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: vec!["folder-root".into()],
            nodes: BTreeMap::from([(
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Prod".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            )]),
        },
        Rc::clone(&repo_state),
        Some("disk full"),
    ));

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_top_status_bar_with_store_and_effects_and_asset_repo(
            &app,
            None,
            default_platform_window_effects(),
            Some(asset_repo),
        );

        let folder_id = app
            .get_console_asset_items()
            .row_data(0)
            .unwrap()
            .id
            .to_string();
        app.invoke_toggle_assets_tree_expansion_requested();
        app.invoke_toggle_assets_search_requested();
        app.invoke_assets_search_query_changed("Prod".into());
        app.invoke_asset_selected(folder_id.clone().into());
        app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
        app.invoke_assets_context_menu_action_invoked("rename-asset".into());
        app.invoke_asset_rename_modal_name_changed("Infra".into());
        app.invoke_confirm_asset_rename_requested();
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(log_content.contains("failed to save asset catalog"));
    assert!(log_content.contains("error=disk full"));

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    let persisted_tree = catalog_to_asset_tree(&save_attempts[0]);
    assert_eq!(persisted_tree.is_expanded("folder-root"), Some(false));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn save_action_persists_asset_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(app.get_active_workspace_session_id().is_empty());
}

#[test]
fn connect_action_opens_temporary_session_without_persisting_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert!(!app.get_active_workspace_session_id().is_empty());
}

#[test]
fn connect_action_keeps_session_ephemeral_and_does_not_persist_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        None,
    ));
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert!(repo_state.borrow().save_attempts.is_empty());
    assert_eq!(launcher_state.probe_profiles.len(), 1);
    assert_eq!(launcher_state.launch_profiles.len(), 1);
    assert!(
        launcher_state.launch_profiles[0]
            .asset_id
            .as_deref()
            .expect("ephemeral asset id")
            .starts_with("session:")
    );
}

#[test]
fn connect_action_reuses_existing_ephemeral_session_for_same_draft() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    for _ in 0..2 {
        app.invoke_assets_create_action_selected("new-ssh-connection".into());
        app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
        app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
        app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
        app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
        app.invoke_asset_ssh_modal_action_requested("connect".into());
    }

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
}

#[test]
fn quick_launch_connect_opens_saved_asset_session_and_updates_recent_order() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    create_root_ssh(&app, "DB Replica", "10.0.0.24");
    let find_asset_id = |label: &str| {
        let rows = app.get_console_asset_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .find(|row| row.label.as_str() == label)
            .map(|row| row.id.to_string())
            .expect("asset id by label")
    };
    let prod_id = find_asset_id("Prod Bastion");
    let db_id = find_asset_id("DB Replica");

    app.invoke_welcome_quick_launch_connect_requested(prod_id.clone().into());
    app.invoke_welcome_quick_launch_connect_requested(db_id.clone().into());

    let recent = app.get_welcome_quick_launch_recent_items();
    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_eq!(recent.row_count(), 2);
    assert_eq!(
        recent.row_data(0).expect("recent row 0").asset_id.as_str(),
        db_id.as_str()
    );
    assert_eq!(
        recent.row_data(1).expect("recent row 1").asset_id.as_str(),
        prod_id.as_str()
    );
    assert_eq!(
        app.get_welcome_quick_launch_selected_detail()
            .asset_id
            .as_str(),
        db_id.as_str()
    );
}

#[test]
fn quick_launch_reveal_in_assets_selects_console_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_sidebar_destination_selected("snippets".into());

    app.invoke_welcome_quick_launch_reveal_in_assets_requested(ssh_id.clone().into());

    let row = app
        .get_console_asset_items()
        .row_data(0)
        .expect("console row after reveal");
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");
    assert_eq!(row.id.as_str(), ssh_id.as_str());
    assert!(row.selected);
    assert!(row.focused);
}

#[test]
fn quick_launch_toggle_favorite_and_search_refresh_dashboard_projection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    create_root_ssh(&app, "DB Replica", "10.0.0.24");
    let find_asset_id = |label: &str| {
        let rows = app.get_console_asset_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .find(|row| row.label.as_str() == label)
            .map(|row| row.id.to_string())
            .expect("asset id by label")
    };
    let prod_id = find_asset_id("Prod Bastion");
    let db_id = find_asset_id("DB Replica");

    app.invoke_welcome_quick_launch_toggle_favorite_requested(prod_id.clone().into());

    let favorites = app.get_welcome_quick_launch_favorite_items();
    assert_eq!(favorites.row_count(), 1);
    assert_eq!(
        favorites
            .row_data(0)
            .expect("favorite row 0")
            .asset_id
            .as_str(),
        prod_id.as_str()
    );
    assert!(favorites.row_data(0).expect("favorite row 0").favorite);

    app.invoke_welcome_quick_launch_search_changed("db".into());

    let visible_group_items = app.get_welcome_quick_launch_visible_group_items();
    assert_eq!(app.get_welcome_quick_launch_search_query().as_str(), "db");
    assert_eq!(visible_group_items.row_count(), 1);
    assert_eq!(
        visible_group_items
            .row_data(0)
            .expect("visible group row 0")
            .asset_id
            .as_str(),
        db_id.as_str()
    );
    assert_eq!(
        app.get_welcome_quick_launch_selected_detail()
            .asset_id
            .as_str(),
        db_id.as_str()
    );
}

#[test]
fn workspace_new_tab_request_opens_single_launcher_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_workspace_new_tab_requested();
    app.invoke_workspace_new_tab_requested();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        "workspace-launcher"
    );
}

#[test]
fn launcher_recent_connection_replaces_launcher_tab_with_real_session_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_quick_launch_connect_requested(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let item = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("workspace tab after launcher connect");
    assert_ne!(item.session_id.as_str(), "workspace-launcher");
    assert_eq!(item.title.as_str(), "Prod Bastion");
}

#[test]
fn launcher_picker_activation_replaces_launcher_tab_and_closes_modal() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "DB Admin", "10.0.0.24");

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_open_saved_ssh_requested();
    assert!(app.get_open_saved_ssh_modal_open());

    app.invoke_open_saved_ssh_modal_asset_activated(ssh_id.into());

    assert!(!app.get_open_saved_ssh_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let item = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("workspace tab after picker activation");
    assert_ne!(item.session_id.as_str(), "workspace-launcher");
    assert_eq!(item.title.as_str(), "DB Admin");
}

#[test]
fn launcher_picker_folder_activation_does_not_attempt_to_open_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    bind_with_fake_sessions(&app, Some(asset_repo));

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_open_saved_ssh_requested();
    assert!(app.get_open_saved_ssh_modal_open());

    app.invoke_open_saved_ssh_modal_asset_activated("folder-root".into());

    assert!(app.get_open_saved_ssh_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let item = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("launcher tab after folder activation");
    assert_eq!(item.session_id.as_str(), "workspace-launcher");
    assert_eq!(item.title.as_str(), "New Tab");
}

#[test]
fn save_and_connect_persists_saved_secret_before_probe() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store(
        &app,
        None,
        Arc::new(StoredSecretProbeLauncher {
            store: Arc::clone(&credential_store),
            message: "missing SSH password secret for `Prod Bastion`",
        }),
        Arc::clone(&credential_store),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
}

#[test]
fn save_and_connect_persists_asset_then_opens_session_with_saved_identity() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        Some("persist failed before launch"),
    ));
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert_eq!(repo_state.borrow().save_attempts.len(), 1);
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "persist failed before launch"
    );
}

#[test]
fn test_connection_updates_feedback_without_creating_workspace_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(launcher_state.probe_profiles.len(), 1);
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );
}

#[test]
fn asset_activation_omits_internal_ssh_runtime_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-activation");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!log_content.contains("asset activated from explorer"));
    assert!(!log_content.contains("activating asset"));
    assert!(!log_content.contains("attempting to open ssh session after probe gate"));
    assert!(!log_content.contains("reusing existing workspace tab for activated ssh asset"));
    assert!(!log_content.contains("ssh probe succeeded, opening workspace session"));
    assert!(!log_content.contains("session manager registered new session handle"));
    assert!(!log_content.contains("session manager reused existing session handle"));
    assert!(!log_content.contains("resolved saved ssh asset profile inputs"));
    assert!(!log_content.contains("session manager probing ssh connection"));
    assert!(!log_content.contains("starting ssh runtime connection"));
    assert!(!log_content.contains("ssh runtime established transport connection"));
    assert!(!log_content.contains("authenticating ssh client"));
    assert!(!log_content.contains("loading stored ssh secret bundle"));
    assert!(!log_content.contains("ssh runtime completed authentication"));
    assert!(!log_content.contains("ssh runtime opened session channel"));
    assert!(!log_content.contains("ssh runtime negotiated pty"));
    assert!(!log_content.contains("ssh runtime requested remote shell"));
    assert!(!log_content.contains("session manager probe completed"));
    assert!(!log_content.contains("session manager received connected event"));
    assert!(!log_content.contains("session manager received disconnected event"));
    assert!(!log_content.contains("session manager received terminal surface update"));
    assert!(!log_content.contains("synchronized workspace projection from session manager"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn context_menu_open_omits_ssh_action_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-context-menu");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
        app.invoke_assets_context_menu_action_invoked("open-connection".into());
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!log_content.contains("opening ssh asset from context menu"));
    assert!(!log_content.contains("opening ssh asset in a new tab from context menu"));
    assert!(!log_content.contains("activating asset"));
    assert!(!log_content.contains("attempting to open ssh session after probe gate"));
    assert!(!log_content.contains("ssh probe succeeded, opening workspace session"));
    assert!(!log_content.contains("session manager registered new session handle"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn save_action_persists_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(app.get_active_workspace_session_id().is_empty());
}

#[test]
fn ssh_context_menu_keeps_open_as_the_only_connection_action() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "open-connection"));
    assert!(!context_menu_item_enabled(&app, "open-in-new-tab"));
    assert!(!context_menu_item_enabled(&app, "close-connection"));

    app.invoke_close_assets_context_menu_requested();
    app.invoke_asset_activated(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "open-connection"));
    assert!(!context_menu_item_enabled(&app, "open-in-new-tab"));
    assert!(!context_menu_item_enabled(&app, "close-connection"));
}

#[test]
fn accepting_unknown_host_key_retries_test_connection_and_persists_known_host() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("accept-test");
    let host_key = sample_public_key();
    let expected_fingerprint = host_key.fingerprint(HashAlg::Sha256).to_string();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "10.0.0.12:22");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        expected_fingerprint
    );

    app.invoke_ssh_host_key_modal_accept_requested();

    assert!(!app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );

    let known_hosts = KnownHostsService::new(&known_hosts_path);
    assert_eq!(
        known_hosts
            .check("10.0.0.12", 22, &host_key)
            .expect("check trusted host"),
        KnownHostCheck::Trusted
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn unknown_host_key_blocks_connection_in_workspace_timeline() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-block");
    let host_key = sample_public_key();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    assert!(
        !app.get_ssh_host_key_modal_open(),
        "workspace session host-key confirmation should stay inline instead of reusing the modal flow"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn trusting_unknown_host_key_retries_connection_in_same_workspace_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-trust");
    let host_key = sample_public_key();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    app.invoke_workspace_session_local_action_requested("trust-host-key".into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert_eq!(app.get_workspace_session_state().as_str(), "connected");
    assert_eq!(
        KnownHostsService::new(&known_hosts_path)
            .check("10.0.0.12", 22, &host_key)
            .expect("check trusted host after inline confirmation"),
        KnownHostCheck::Trusted
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn rejecting_unknown_host_key_keeps_connection_timeline_in_same_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-reject");
    let host_key = sample_public_key();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    app.invoke_workspace_session_local_action_requested("reject-host-key".into());
    flush_runtime_projection();

    let headline = app.get_workspace_session_connection_headline().to_string();
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert!(
        matches!(headline.as_str(), "cancelled" | "error"),
        "rejecting the host key should keep the timeline surface active with a terminal-free final state"
    );
    assert!(
        app.get_workspace_session_connection_current_detail()
            .as_str()
            .contains("Rejected unknown SSH host key"),
        "rejecting the host key should preserve a useful rejection detail in the timeline"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        KnownHostsService::new(&known_hosts_path)
            .check("10.0.0.12", 22, &host_key)
            .expect("recheck rejected host"),
        KnownHostCheck::Unknown {
            fingerprint: host_key.fingerprint(HashAlg::Sha256).to_string()
        }
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn cancelling_running_connection_attempt_marks_timeline_cancelled() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PendingConnectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "connecting"
    );

    app.invoke_workspace_session_local_action_requested("cancel-connection-attempt".into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "cancelled"
    );
}

#[test]
fn runtime_events_refresh_workspace_terminal_projection_after_opening_saved_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(AsyncProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(80));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(100));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connected");
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert_eq!(app.get_workspace_session_surface_seqno(), 1);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("first projected terminal row")
            .as_str(),
        "welcome to mica-term"
    );
}

#[test]
fn workspace_terminal_input_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_surface_seqno(), 1);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("initial visible row")
            .as_str(),
        "welcome to mica-term"
    );

    app.invoke_workspace_session_text_input("pwd".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(visible_lines.row_data(1).unwrap().as_str(), "$ pwd");
}

#[test]
fn workspace_terminal_paste_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PasteProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(visible_lines.row_data(1).unwrap().as_str(), "paste pwd");
}

#[test]
fn workspace_terminal_multiline_paste_warning_defers_unprotected_paste_until_confirmed() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(PasteWarningProjectionLauncher {
            bracketed_paste_enabled: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd\necho hi", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed multiline clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_paste_warning_line_count(), 2);
    assert_eq!(app.get_workspace_session_surface_seqno(), 1);
    assert_eq!(app.get_workspace_session_visible_lines().row_count(), 1);

    app.invoke_workspace_paste_warning_confirm_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(!app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(app.get_workspace_session_visible_lines().row_count(), 2);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(1)
            .unwrap()
            .as_str(),
        "paste pwd\necho hi"
    );
}

#[test]
fn workspace_terminal_multiline_paste_warning_skips_bracketed_paste_sessions() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(PasteWarningProjectionLauncher {
            bracketed_paste_enabled: true,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd\necho hi", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed multiline clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(!app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(app.get_workspace_session_visible_lines().row_count(), 2);
}

#[test]
fn workspace_terminal_long_multiline_paste_opens_editor_and_sends_edited_text() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(PasteWarningProjectionLauncher {
            bracketed_paste_enabled: true,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(
            "one\ntwo\nthree\nfour",
            slint::platform::Clipboard::DefaultClipboard,
        );
        Ok(())
    })
    .expect("seed long multiline clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(app.get_workspace_paste_warning_modal_open());
    assert!(app.get_workspace_paste_warning_editor_mode());
    assert_eq!(
        app.get_workspace_paste_warning_text(),
        "one\ntwo\nthree\nfour"
    );
    assert_eq!(app.get_workspace_session_surface_seqno(), 1);

    app.set_workspace_paste_warning_text("one\ntwo\nfour".into());
    app.invoke_workspace_paste_warning_confirm_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(!app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(1)
            .unwrap()
            .as_str(),
        "paste one\ntwo\nfour"
    );
}

#[test]
fn workspace_terminal_single_line_trailing_newline_pastes_without_warning() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(PasteWarningProjectionLauncher {
            bracketed_paste_enabled: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd\n", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed single-line clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(!app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(1)
            .unwrap()
            .as_str(),
        "paste pwd\n"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_c_copies_selected_text_to_clipboard() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    select_terminal_welcome_span(&app);

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "C".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "C".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard");

    assert!(
        copied
            .as_deref()
            .is_some_and(|text| text.contains("welcome")),
        "Ctrl+Shift+C should copy the current terminal selection into the system clipboard"
    );
}

#[test]
fn workspace_terminal_selection_updates_surface_image() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let before = app
        .get_workspace_session_surface_image()
        .to_rgba8()
        .expect("rgba image before selection");

    select_terminal_welcome_span(&app);
    settle_terminal_projection();

    let after = app
        .get_workspace_session_surface_image()
        .to_rgba8()
        .expect("rgba image after selection");
    let cell_width = after.width() / app.get_workspace_session_cols() as u32;
    let cell_height = after.height() / app.get_workspace_session_rows() as u32;
    let selected_space_x = 7 * cell_width + (cell_width / 2);
    let selected_space_y = cell_height / 2;
    let selected_pixel =
        after.as_slice()[(selected_space_y * after.width() + selected_space_x) as usize];

    assert!(
        app.get_workspace_session_selection_active(),
        "pointer drag should activate terminal selection state"
    );
    assert_ne!(
        before.as_slice(),
        after.as_slice(),
        "terminal selection should visibly repaint the atlas surface image"
    );
    assert!(
        selected_pixel.r >= 90
            && selected_pixel.r <= 140
            && selected_pixel.g >= 100
            && selected_pixel.g <= 150
            && selected_pixel.b >= 130
            && selected_pixel.b <= 175
            && selected_pixel.b > selected_pixel.g
            && selected_pixel.g >= selected_pixel.r,
        "selected blank cells should render as a muted iris-blue highlight instead of a saturated bright blue"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_c_copies_selected_text_when_backend_emits_etx() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    select_terminal_welcome_span(&app);

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: "\u{3}".into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: "\u{3}".into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard");

    assert!(
        copied
            .as_deref()
            .is_some_and(|text| text.contains("welcome")),
        "Ctrl+Shift+C should still copy when the backend emits ETX instead of a literal C"
    );
}

#[test]
fn workspace_terminal_plain_ctrl_a_forwards_prefix_key_without_selecting_all() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_workspace_session_selection_active(),
        "terminal selection should start inactive before testing Ctrl+A forwarding"
    );

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "a".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "a".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    settle_terminal_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(
        visible_lines
            .row_data(1)
            .expect("forwarded Ctrl+A line")
            .as_str(),
        "$ a",
        "plain Ctrl+A should stay in the terminal input stream so screen/tmux prefix chords still work"
    );
    assert!(
        !app.get_workspace_session_selection_active(),
        "plain Ctrl+A should not trigger a local select-all gesture inside the terminal host"
    );
}

#[test]
fn workspace_terminal_ctrl_key_matrix_forwards_common_shell_shortcuts() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = ['a', 'c', 'v', 'z', 'd', 'l'];
    for key in cases {
        dispatch_text_key_chord(&app, &key.to_string(), true, false, false);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::character(key, false, true, false)],
            "Ctrl+{key} should be forwarded to the remote terminal as a control chord"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "Ctrl+{key} should not be converted into a local paste action"
        );
    }
}

#[test]
fn workspace_terminal_ctrl_shift_shortcut_matrix_keeps_local_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);
    select_terminal_welcome_span(&app);

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    dispatch_text_key_chord(&app, "C", true, true, false);
    settle_terminal_projection();

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard")
    .expect("clipboard text after Ctrl+Shift+C");
    assert!(
        copied.contains("welcome"),
        "Ctrl+Shift+C should still copy the active terminal selection"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+C should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+C should not touch the remote paste channel"
    );

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(
            "printf 'matrix paste'",
            slint::platform::Clipboard::DefaultClipboard,
        );
        Ok(())
    })
    .expect("seed clipboard");

    dispatch_text_key_chord(&app, "V", true, true, false);
    settle_terminal_projection();

    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+V should not forward a remote key chord"
    );
    assert_eq!(
        state.take_paste_inputs(),
        vec!["printf 'matrix paste'".to_string()],
        "Ctrl+Shift+V should use the terminal paste channel"
    );

    for key in ["T", "W", "P", "F"] {
        dispatch_text_key_chord(&app, key, true, true, false);
        settle_terminal_projection();
        assert!(
            state.take_key_inputs().is_empty(),
            "reserved Ctrl+Shift+{key} should stay local and never forward to the remote terminal"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "reserved Ctrl+Shift+{key} should not hit the terminal paste channel"
        );
    }
}

#[test]
fn workspace_terminal_ctrl_shift_t_opens_new_tab_from_active_terminal_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let first_session_id = app.get_active_workspace_session_id().to_string();

    dispatch_text_key_chord(&app, "T", true, true, false);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_ne!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id,
        "Ctrl+Shift+T should create and activate a fresh workspace tab"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+T should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+T should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_w_closes_active_terminal_tab_locally() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    settle_terminal_projection();
    let first_session_id = app.get_active_workspace_session_id().to_string();

    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_ne!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id
    );

    dispatch_text_key_chord(&app, "W", true, true, false);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id,
        "Ctrl+Shift+W should close the active workspace tab and fall back to the previous one"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+W should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+W should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_f_expands_asset_search_locally() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_asset_search_expanded(),
        "asset search should start collapsed"
    );

    dispatch_text_key_chord(&app, "F", true, true, false);
    settle_terminal_projection();

    assert!(
        app.get_asset_search_expanded(),
        "Ctrl+Shift+F should expand the local asset search"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+F should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+F should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_p_opens_global_menu_locally() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_show_global_menu(),
        "global menu should start closed"
    );

    dispatch_text_key_chord(&app, "P", true, true, false);
    settle_terminal_projection();
    assert!(
        app.get_show_global_menu(),
        "Ctrl+Shift+P should open the local global menu"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+P should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+P should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_alt_arrow_matrix_forwards_modifier_aware_named_keys() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = [
        ("left", "left"),
        ("right", "right"),
        ("up", "up"),
        ("down", "down"),
    ];
    for (named_key, expected_name) in cases {
        dispatch_named_key_chord(&app, named_key, false, false, true);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::named(expected_name, true, false, false)],
            "Alt+{named_key} should preserve the alt modifier in the remote terminal event"
        );
    }
}

#[test]
fn workspace_terminal_named_key_matrix_forwards_navigation_keys() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = [("home", "home"), ("end", "end"), ("insert", "insert")];
    for (named_key, expected_name) in cases {
        dispatch_named_key_chord(&app, named_key, false, false, false);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::named(expected_name, false, false, false)],
            "{named_key} should forward as a named terminal key event"
        );
    }
}

#[test]
fn workspace_terminal_function_key_matrix_forwards_f1_through_f24() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    for number in 1u8..=24 {
        dispatch_function_key(&app, number);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::function(number, false, false, false)],
            "F{number} should forward to the remote terminal function-key path"
        );
    }
}

#[test]
fn workspace_terminal_shift_page_shortcuts_scroll_locally() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);

    dispatch_named_key_chord(&app, "page-up", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "Shift+PageUp should move local scrollback toward the top"
    );

    dispatch_named_key_chord(&app, "page-down", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        0,
        "Shift+PageDown should move local scrollback back toward the bottom"
    );
}

#[test]
fn workspace_terminal_shift_home_end_shortcuts_jump_scrollback_locally() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);

    dispatch_named_key_chord(&app, "home", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "Shift+Home should jump local scrollback to the top"
    );
    assert!(!app.get_workspace_session_viewport_at_bottom());

    dispatch_named_key_chord(&app, "end", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        0,
        "Shift+End should jump local scrollback back to the bottom"
    );
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_mouse_input_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_mouse_input(
        "down".into(),
        "left".into(),
        2,
        4,
        false,
        false,
        false,
    );

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "mouse input forwarded"
    );
}

#[test]
fn bootstrap_projects_terminal_scrollback_state_into_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);
    assert_eq!(app.get_workspace_session_viewport_max_offset_lines(), 8);
    assert!(!app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn bootstrap_projects_terminal_canvas_palette_into_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff1f_2328
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xfff7_f9fc
    );
    assert_eq!(
        app.get_workspace_session_cursor_fg().as_argb_encoded(),
        0xfff7_f9fc
    );
    assert_eq!(
        app.get_workspace_session_cursor_bg().as_argb_encoded(),
        0xff4b_5058
    );
}

#[test]
fn bootstrap_projects_dark_terminal_cursor_as_light_grey() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_cursor_bg().as_argb_encoded(),
        0xff00_0000 | preset_for_theme_mode(ThemeMode::Dark).cursor_bg
    );
}

#[test]
fn terminal_input_callback_snaps_scrolled_session_back_to_latest_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("scrolled visible line")
            .as_str(),
        "offset 3"
    );

    app.invoke_workspace_session_text_input("pwd".into());

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("bottom visible line")
            .as_str(),
        "offset 0"
    );
}

#[test]
fn ctrl_shift_letter_shortcuts_do_not_forward_remote_terminal_input() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "f".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "f".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    settle_terminal_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(
        app.get_workspace_session_surface_seqno(),
        1,
        "reserved Ctrl+Shift shortcuts should stay local and must not trigger a remote surface update"
    );
    assert_eq!(visible_lines.row_count(), 1);
    assert_eq!(
        visible_lines
            .row_data(0)
            .expect("initial terminal line")
            .as_str(),
        "welcome to mica-term"
    );
}

#[test]
fn ctrl_shift_non_reserved_letter_shortcuts_forward_remote_terminal_input() {
    i_slint_backend_testing::init_no_event_loop();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    for key in ['A', 'B', 'K', 'L', 'N', 'O', 'R'] {
        dispatch_text_key_chord(&app, &key.to_string(), true, true, false);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::character(key, false, true, true)],
            "Ctrl+Shift+{key} should forward to the remote terminal once it is no longer reserved locally"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "Ctrl+Shift+{key} should not hit the terminal paste channel"
        );
    }
}

#[test]
fn workspace_terminal_scroll_callbacks_update_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 8);
    assert!(!app.get_workspace_session_viewport_at_bottom());

    app.invoke_workspace_session_scroll_thumb_drag_requested(0.0);
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_pointer_wheel_accumulates_before_multi_line_scrollback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let position = terminal_interaction_position(&app);
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 60.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        3,
        "half-wheel motion should be retained locally until the accumulation threshold is crossed"
    );

    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 60.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "one accumulated wheel notch should request six local lines, capped by the current viewport max offset"
    );
}

#[test]
fn workspace_terminal_paused_follow_tracks_pending_output_until_jump_to_latest() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let follow_state = FollowProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FollowProjectionLauncher {
            state: follow_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    assert!(app.get_workspace_session_follow_paused());
    assert_eq!(app.get_workspace_session_pending_output_lines(), 0);

    follow_state.emit_remote_output(3);
    settle_terminal_projection();

    assert!(app.get_workspace_session_follow_paused());
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 11);
    assert_eq!(app.get_workspace_session_pending_output_lines(), 3);

    app.invoke_workspace_session_jump_to_latest_requested();
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
    assert!(!app.get_workspace_session_follow_paused());
    assert_eq!(app.get_workspace_session_pending_output_lines(), 0);
}

#[test]
fn workspace_terminal_live_input_resumes_follow_and_clears_pending_output() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let follow_state = FollowProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FollowProjectionLauncher {
            state: follow_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    follow_state.emit_remote_output(2);
    settle_terminal_projection();

    assert!(app.get_workspace_session_follow_paused());
    assert_eq!(app.get_workspace_session_pending_output_lines(), 2);

    app.invoke_workspace_session_text_input("a".into());
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
    assert!(!app.get_workspace_session_follow_paused());
    assert_eq!(app.get_workspace_session_pending_output_lines(), 0);
}

#[test]
fn async_launch_failure_projects_error_tab_after_projection_timer_ticks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FailingProbeLauncher {
            message: "missing SSH password secret for `SSH Connection 1`",
        }),
    );

    let ssh_id = create_root_ssh(&app, "SSH Connection 1", "157.254.53.77");
    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );

    std::thread::sleep(Duration::from_millis(80));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(100));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "error");
    assert_eq!(
        app.get_workspace_session_error_detail().as_str(),
        "missing SSH password secret for `SSH Connection 1`"
    );
}

#[test]
fn sftp_navigation_callbacks_update_projected_path_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_sftp_panel_path_submitted("/srv/app".into());
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(app.get_sftp_panel_follow_mode().as_str(), "manual-browse");

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");

    app.invoke_sftp_panel_back_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");

    app.invoke_sftp_panel_forward_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");

    app.invoke_sftp_panel_up_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
}

#[test]
fn opening_sftp_reads_the_active_session_directory_instead_of_staying_connecting() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(app.get_sftp_panel_items().row_count(), 1);
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .name
            .as_str(),
        "logs"
    );
    assert_eq!(sftp_state.take_read_dir_calls(), vec!["/srv/app".to_string()]);
}

#[test]
fn refresh_and_path_submit_trigger_real_directory_reads() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");
    assert_eq!(app.get_sftp_panel_items().row_count(), 1);
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .name
            .as_str(),
        "release.tar.gz"
    );

    app.invoke_sftp_panel_refresh_requested();
    flush_runtime_projection();

    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app/releases".to_string(),
        ]
    );
}
