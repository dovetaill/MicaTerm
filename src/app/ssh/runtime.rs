//! SSH session runtime primitives and terminal-state wrapper.

mod auth;
mod contracts;
mod pump;
mod sftp_backend;
mod terminal;
mod transport;
mod zmodem;

pub use auth::UnknownHostKeyError;
pub(crate) use auth::{load_optional_stored_secret_bundle, stored_secret_lookup_message};
pub use contracts::TerminalSurfaceState as SurfaceState;
pub use contracts::{
    TerminalCellState, TerminalCursorShape, TerminalCursorState, TerminalKeyEvent, TerminalKeyKind,
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalRowState,
    TerminalSelectionGestureMode, TerminalSelectionRange, TerminalShellIntegrationState,
    TerminalSurfaceSignature, TerminalSurfaceState,
};
pub use terminal::{
    DEFAULT_TERMINAL_SCROLLBACK_LINES, TerminalSession, encode_named_key_input,
    extract_current_working_directory_from_osc7, negotiated_terminal_environment,
};
pub use zmodem::{
    ZmodemDownloadConflictPolicy, ZmodemTransferDirection, ZmodemTransferPhase, ZmodemTransferState,
};

use self::auth::{ConnectionProgressReporter, RuntimeClientHandler};
use self::pump::{
    remote_command_exists, resolve_remote_current_working_directory, run_channel_pump,
    run_zmodem_exec_upload,
};
use self::sftp_backend::RusshSftpBackend;
use self::terminal::{apply_remote_output, await_channel_success, negotiate_terminal_environment};
use self::transport::{connect_target_handle_for_profile, ssh_client_config};

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::clipboard_inline_image::surface_allows_inline_image;
use crate::app::sftp::SftpRuntimeHandle;
use crate::app::ssh::connection_progress::{ConnectionHeadlineState, ConnectionProgressEvent};
use crate::app::ssh::credentials::{CredentialStore, SystemCredentialStore};
use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::session_manager::{EnhancedSessionState, SessionRuntimeControl};
use crate::app::ssh::shell_integration::runtime_shell_events;
use crate::app::terminal_core::{LocalTerminalImage, TerminalViewportMetrics};
use crate::theme::{ThemeMode, ThemeVariant};

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;
const DEFAULT_TERMINAL_CELL_WIDTH_PX: u32 = 8;
const DEFAULT_TERMINAL_CELL_HEIGHT_PX: u32 = 16;
const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSH_KEEPALIVE_MAX_MISSES: usize = 3;
const FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(8);
const SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(40);
const INPUT_ACTIVE_SURFACE_DIRTY_WINDOW: Duration = Duration::from_millis(160);
const WORKING_SET_TRIM_IDLE_INTERVAL: Duration = Duration::from_secs(2);
const WORKING_SET_TRIM_MIN_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct TerminalRuntimeDefaults {
    scrollback_lines: Arc<AtomicUsize>,
    theme: Arc<Mutex<TerminalRuntimeThemeDefaults>>,
    viewport: Arc<Mutex<TerminalRuntimeViewportDefaults>>,
}

#[derive(Debug, Clone, Copy)]
struct TerminalRuntimeThemeDefaults {
    mode: ThemeMode,
    variant: ThemeVariant,
}

#[derive(Debug, Clone, Copy)]
struct TerminalRuntimeViewportDefaults {
    rows: usize,
    cols: usize,
    metrics: TerminalViewportMetrics,
}

impl Default for TerminalRuntimeDefaults {
    fn default() -> Self {
        Self::new(DEFAULT_TERMINAL_SCROLLBACK_LINES)
    }
}

impl TerminalRuntimeDefaults {
    pub fn new(scrollback_lines: usize) -> Self {
        Self {
            scrollback_lines: Arc::new(AtomicUsize::new(scrollback_lines.max(1))),
            theme: Arc::new(Mutex::new(TerminalRuntimeThemeDefaults {
                mode: ThemeMode::Dark,
                variant: ThemeVariant::PremiumDefault,
            })),
            viewport: Arc::new(Mutex::new(TerminalRuntimeViewportDefaults {
                rows: DEFAULT_TERMINAL_ROWS,
                cols: DEFAULT_TERMINAL_COLS,
                metrics: TerminalViewportMetrics::fallback(
                    DEFAULT_TERMINAL_ROWS,
                    DEFAULT_TERMINAL_COLS,
                ),
            })),
        }
    }

    pub fn scrollback_lines(&self) -> usize {
        self.scrollback_lines.load(Ordering::Relaxed).max(1)
    }

    pub fn set_scrollback_lines(&self, scrollback_lines: usize) {
        self.scrollback_lines
            .store(scrollback_lines.max(1), Ordering::Relaxed);
    }

    pub fn theme_mode(&self) -> ThemeMode {
        self.theme.lock().expect("lock terminal runtime theme").mode
    }

    pub fn theme_variant(&self) -> ThemeVariant {
        self.theme
            .lock()
            .expect("lock terminal runtime theme")
            .variant
    }

    pub fn set_theme(&self, mode: ThemeMode, variant: ThemeVariant) {
        let mut theme = self.theme.lock().expect("lock terminal runtime theme");
        theme.mode = mode;
        theme.variant = variant;
    }

    pub fn viewport_rows(&self) -> usize {
        self.viewport().0
    }

    pub fn viewport_cols(&self) -> usize {
        self.viewport().1
    }

    pub fn viewport_pixel_width(&self) -> u32 {
        self.viewport_metrics().pixel_width
    }

    pub fn viewport_pixel_height(&self) -> u32 {
        self.viewport_metrics().pixel_height
    }

    pub fn viewport_dpi(&self) -> u32 {
        self.viewport_metrics().dpi
    }

    pub fn viewport_metrics(&self) -> TerminalViewportMetrics {
        self.viewport().2
    }

    pub fn viewport(&self) -> (usize, usize, TerminalViewportMetrics) {
        let viewport = *self
            .viewport
            .lock()
            .expect("lock terminal runtime viewport defaults");
        (viewport.rows, viewport.cols, viewport.metrics)
    }

    pub fn set_viewport_size(&self, rows: usize, cols: usize, pixel_width: u32, pixel_height: u32) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        let pixel_width = if pixel_width == 0 {
            cols.saturating_mul(DEFAULT_TERMINAL_CELL_WIDTH_PX as usize) as u32
        } else {
            pixel_width
        };
        let pixel_height = if pixel_height == 0 {
            rows.saturating_mul(DEFAULT_TERMINAL_CELL_HEIGHT_PX as usize) as u32
        } else {
            pixel_height
        };
        let dpi = self.viewport_dpi();
        self.set_viewport_metrics(
            rows,
            cols,
            TerminalViewportMetrics::new(pixel_width, pixel_height, dpi),
        );
    }

    pub fn set_viewport_metrics(
        &self,
        rows: usize,
        cols: usize,
        viewport: TerminalViewportMetrics,
    ) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        *self
            .viewport
            .lock()
            .expect("lock terminal runtime viewport defaults") = TerminalRuntimeViewportDefaults {
            rows,
            cols,
            metrics: TerminalViewportMetrics::new(
                viewport.pixel_width,
                viewport.pixel_height,
                viewport.dpi,
            ),
        };
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    ConnectionProgress(ConnectionProgressEvent),
    EnhancedSessionStateChanged(EnhancedSessionState),
    CurrentDirectoryChanged(String),
    ZmodemStateChanged(Option<ZmodemTransferState>),
    ShellIntegrationChanged(TerminalShellIntegrationState),
    SurfaceChanged(TerminalSurfaceState),
    SurfaceDirty,
    Disconnected,
    Error(String),
}

pub struct SshSessionRuntime {
    session_id: Uuid,
    #[allow(dead_code)]
    profile: ConnectionProfile,
    handle: Arc<russh::client::Handle<RuntimeClientHandler>>,
    async_runtime: tokio::runtime::Handle,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    terminal: Arc<Mutex<TerminalSession>>,
    terminal_defaults: TerminalRuntimeDefaults,
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
    exec_zmodem_transfer: Arc<Mutex<ExecZmodemTransferSlot>>,
    sftp_runtime: SftpRuntimeHandle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecZmodemCommand {
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExecZmodemCancelRoute {
    Routed(u64),
    NotActive,
}

struct ActiveExecZmodemTransfer {
    generation: u64,
    command_tx: mpsc::UnboundedSender<ExecZmodemCommand>,
}

#[derive(Default)]
struct ExecZmodemTransferSlot {
    next_generation: u64,
    active: Option<ActiveExecZmodemTransfer>,
}

impl ExecZmodemTransferSlot {
    fn register(&mut self, command_tx: mpsc::UnboundedSender<ExecZmodemCommand>) -> Result<u64> {
        if self
            .active
            .as_ref()
            .is_some_and(|active| !active.command_tx.is_closed())
        {
            return Err(anyhow!("a dedicated exec zmodem upload is already active"));
        }
        self.active = None;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| anyhow!("exec zmodem generation overflow"))?;
        let generation = self.next_generation;
        self.active = Some(ActiveExecZmodemTransfer {
            generation,
            command_tx,
        });
        Ok(generation)
    }

    fn clear_if_generation(&mut self, expected_generation: u64) -> bool {
        let matches = self
            .active
            .as_ref()
            .is_some_and(|active| active.generation == expected_generation);
        if matches {
            self.active = None;
        }
        matches
    }

    #[cfg(test)]
    fn active_generation(&self) -> Option<u64> {
        self.active.as_ref().map(|active| active.generation)
    }
}

pub(super) struct ExecZmodemTransferRegistration {
    session_id: Uuid,
    slot: std::sync::Weak<Mutex<ExecZmodemTransferSlot>>,
    generation: u64,
}

pub(super) struct ExecZmodemTransferContext {
    pub(super) generation: u64,
    pub(super) command_rx: mpsc::UnboundedReceiver<ExecZmodemCommand>,
    pub(super) registration: ExecZmodemTransferRegistration,
}

impl Drop for ExecZmodemTransferRegistration {
    fn drop(&mut self) {
        let Some(slot) = self.slot.upgrade() else {
            return;
        };
        let cleared = slot
            .lock()
            .expect("lock exec zmodem slot for task cleanup")
            .clear_if_generation(self.generation);
        tracing::debug!(
            target: "app.zmodem",
            session_id = %self.session_id,
            transfer_generation = self.generation,
            owner = "exec",
            outcome = if cleared { "cleared" } else { "stale" },
            "released dedicated exec zmodem lifecycle registration"
        );
    }
}

fn route_exec_zmodem_cancel(slot: &Arc<Mutex<ExecZmodemTransferSlot>>) -> ExecZmodemCancelRoute {
    let active = slot
        .lock()
        .expect("lock exec zmodem lifecycle slot")
        .active
        .as_ref()
        .map(|active| (active.generation, active.command_tx.clone()));
    let Some((generation, command_tx)) = active else {
        return ExecZmodemCancelRoute::NotActive;
    };
    if command_tx.send(ExecZmodemCommand::Cancel).is_ok() {
        return ExecZmodemCancelRoute::Routed(generation);
    }
    slot.lock()
        .expect("lock stale exec zmodem lifecycle slot")
        .clear_if_generation(generation);
    ExecZmodemCancelRoute::NotActive
}

#[derive(Debug)]
enum RuntimeCommand {
    TextInput(String),
    KeyInput(TerminalKeyEvent),
    MouseInput(TerminalMouseInput),
    Paste(String),
    StartZmodemUpload {
        local_paths: Vec<PathBuf>,
    },
    StartInteractiveZmodemUpload {
        local_paths: Vec<PathBuf>,
    },
    StartZmodemDownload {
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    },
    CancelZmodem,
    DismissZmodem {
        expected_state: Box<ZmodemTransferState>,
    },
    Resize {
        rows: u32,
        cols: u32,
        viewport: TerminalViewportMetrics,
    },
    Disconnect,
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Result<Self> {
        Self::connect_with_credential_store(
            profile,
            session_id,
            attempt_id,
            event_tx,
            Arc::new(SystemCredentialStore),
            TerminalRuntimeDefaults::default(),
        )
        .await
    }

    pub async fn connect_with_credential_store(
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        credential_store: Arc<dyn CredentialStore>,
        terminal_defaults: TerminalRuntimeDefaults,
    ) -> Result<Self> {
        let mut progress = ConnectionProgressReporter::new(
            attempt_id,
            event_tx.clone(),
            ConnectionHeadlineState::Connecting,
        );
        let resolve_step = progress.start_step(
            "resolve-profile",
            "Resolve Profile",
            format!("Resolving connection profile for {}", profile.name),
            "Target",
        );
        let config = Arc::new(ssh_client_config());
        resolve_step.finish(format!("Resolved connection profile for {}", profile.name));
        let (transport_chain_guard, handle) = connect_target_handle_for_profile(
            Arc::clone(&config),
            &profile,
            credential_store.as_ref(),
            &mut progress,
        )
        .await?;

        let open_session_step = progress.start_step(
            "open-session-channel",
            "Open Session Channel",
            format!("Opening SSH session channel for {}", profile.host),
            "Target",
        );
        let mut channel = handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel")?;
        let (pty_rows, pty_cols, pty_viewport) = terminal_defaults.viewport();
        let mut terminal_session = TerminalSession::new_with_scrollback_and_viewport(
            pty_rows,
            pty_cols,
            terminal_defaults.scrollback_lines(),
            pty_viewport,
        );
        terminal_session.set_theme(
            terminal_defaults.theme_mode(),
            terminal_defaults.theme_variant(),
        );
        let terminal = Arc::new(Mutex::new(terminal_session));
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        open_session_step.finish(format!("Opened SSH session channel for {}", profile.host));
        let pty_step = progress.start_step(
            "request-pty",
            "Request PTY",
            "Requesting terminal PTY".to_string(),
            "Target",
        );
        channel
            .request_pty(
                true,
                "xterm-256color",
                pty_cols as u32,
                pty_rows as u32,
                pty_viewport.pixel_width,
                pty_viewport.pixel_height,
                &[],
            )
            .await
            .context("failed to request SSH PTY")?;

        let mut pending_output = Vec::new();
        await_channel_success(&mut channel, "pty", &mut pending_output).await?;
        pty_step.finish("SSH PTY request accepted");
        negotiate_terminal_environment(&mut channel, &mut pending_output).await;

        let shell_step = progress.start_step(
            "request-shell",
            "Request Shell",
            "Requesting interactive shell".to_string(),
            "Target",
        );
        channel
            .request_shell(true)
            .await
            .context("failed to request remote shell")?;
        await_channel_success(&mut channel, "shell", &mut pending_output).await?;
        shell_step.finish("Interactive shell request accepted");

        progress.set_headline(ConnectionHeadlineState::Connected);
        let _ = event_tx.send(SessionRuntimeEvent::Connected);
        if !pending_output.is_empty() {
            let terminal_replies =
                apply_initial_remote_output(&terminal, &event_tx, &pending_output);
            if !terminal_replies.is_empty()
                && let Err(bytes) = handle.data(channel.id(), terminal_replies).await
            {
                return Err(anyhow!(
                    "failed to write {} initial terminal response bytes to SSH channel",
                    bytes.len()
                ));
            }
        }

        let handle = Arc::new(handle);
        let sftp_runtime = SftpRuntimeHandle::new(Arc::new(RusshSftpBackend {
            handle: Arc::clone(&handle),
        }));

        let runtime = Self {
            session_id,
            profile: profile.clone(),
            handle: Arc::clone(&handle),
            async_runtime: tokio::runtime::Handle::current(),
            event_tx: event_tx.clone(),
            terminal: Arc::clone(&terminal),
            terminal_defaults,
            command_tx,
            exec_zmodem_transfer: Arc::new(Mutex::new(ExecZmodemTransferSlot::default())),
            sftp_runtime,
        };

        tokio::spawn(run_channel_pump(
            session_id,
            handle,
            channel,
            terminal,
            event_tx,
            command_rx,
            transport_chain_guard,
        ));

        Ok(runtime)
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn terminal(&self) -> Arc<Mutex<TerminalSession>> {
        Arc::clone(&self.terminal)
    }

    pub fn send_text_input(&self, text: String) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::TextInput(text))
            .map_err(|_| anyhow!("ssh runtime text input channel is closed"))
    }

    pub fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::KeyInput(event))
            .map_err(|_| anyhow!("ssh runtime key input channel is closed"))
    }

    pub fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        self.resize_with_viewport(rows, cols, self.terminal_defaults.viewport_metrics())
    }

    pub fn resize_with_viewport(
        &self,
        rows: u32,
        cols: u32,
        viewport: TerminalViewportMetrics,
    ) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Resize {
                rows,
                cols,
                viewport,
            })
            .map_err(|_| anyhow!("ssh runtime resize channel is closed"))
    }

    pub fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::MouseInput(event))
            .map_err(|_| anyhow!("ssh runtime mouse input channel is closed"))
    }

    pub fn send_paste(&self, text: String) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Paste(text))
            .map_err(|_| anyhow!("ssh runtime paste channel is closed"))
    }

    pub fn start_zmodem_upload(&self, local_paths: Vec<PathBuf>) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::StartZmodemUpload { local_paths })
            .map_err(|_| anyhow!("ssh runtime zmodem upload channel is closed"))
    }

    pub fn start_interactive_zmodem_upload(&self, local_paths: Vec<PathBuf>) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::StartInteractiveZmodemUpload { local_paths })
            .map_err(|_| anyhow!("ssh runtime interactive zmodem upload channel is closed"))
    }

    pub fn remote_command_exists(&self, command_name: String) -> Result<bool> {
        self.async_runtime.block_on(remote_command_exists(
            Arc::clone(&self.handle),
            command_name,
        ))
    }

    pub fn resolve_current_working_directory(&self) -> Result<Option<String>> {
        self.async_runtime
            .block_on(resolve_remote_current_working_directory(Arc::clone(
                &self.handle,
            )))
    }

    pub fn start_zmodem_upload_to_remote_dir(
        &self,
        local_paths: Vec<PathBuf>,
        remote_dir: String,
    ) -> Result<()> {
        let (exec_command_tx, exec_command_rx) = mpsc::unbounded_channel();
        let generation = self
            .exec_zmodem_transfer
            .lock()
            .map_err(|_| anyhow!("failed to lock exec zmodem lifecycle slot"))?
            .register(exec_command_tx)?;
        let registration = ExecZmodemTransferRegistration {
            session_id: self.session_id,
            slot: Arc::downgrade(&self.exec_zmodem_transfer),
            generation,
        };
        let exec_transfer = ExecZmodemTransferContext {
            generation,
            command_rx: exec_command_rx,
            registration,
        };
        self.async_runtime.spawn(run_zmodem_exec_upload(
            self.session_id,
            Arc::clone(&self.handle),
            self.event_tx.clone(),
            local_paths,
            remote_dir,
            exec_transfer,
        ));
        Ok(())
    }

    pub fn start_zmodem_download(
        &self,
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::StartZmodemDownload {
                local_dir,
                conflict_policy,
            })
            .map_err(|_| anyhow!("ssh runtime zmodem download channel is closed"))
    }

    pub fn cancel_zmodem_transfer(&self) -> Result<()> {
        match route_exec_zmodem_cancel(&self.exec_zmodem_transfer) {
            ExecZmodemCancelRoute::Routed(generation) => {
                tracing::info!(
                    target: "app.zmodem",
                    session_id = %self.session_id,
                    transfer_generation = generation,
                    lifecycle_command = "cancel",
                    owner = "exec",
                    outcome = "routed",
                    "routed zmodem cancellation"
                );
                Ok(())
            }
            ExecZmodemCancelRoute::NotActive => {
                tracing::debug!(
                    target: "app.zmodem",
                    session_id = %self.session_id,
                    lifecycle_command = "cancel",
                    owner = "interactive",
                    outcome = "routed",
                    "routed zmodem cancellation"
                );
                self.command_tx
                    .send(RuntimeCommand::CancelZmodem)
                    .map_err(|_| anyhow!("ssh runtime zmodem cancel channel is closed"))
            }
        }
    }

    pub fn dismiss_zmodem_transfer(&self, expected_state: ZmodemTransferState) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::DismissZmodem {
                expected_state: Box::new(expected_state),
            })
            .map_err(|_| anyhow!("ssh runtime zmodem dismiss channel is closed"))
    }

    pub fn selection_text_from_buffer_rows(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<String> {
        let terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for selection copy"))?;
        Ok(terminal.selection_text_from_buffer_rows(start_row, start_col, end_row, end_col))
    }

    pub fn update_theme(
        &self,
        mode: ThemeMode,
        variant: ThemeVariant,
    ) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for theme update"))?;
        terminal.set_theme(mode, variant);
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn update_theme_mode(&self, mode: ThemeMode) -> Result<TerminalSurfaceState> {
        self.update_theme(mode, ThemeVariant::PremiumDefault)
    }

    pub fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for local scrollback"))?;
        terminal.scroll_viewport_lines(delta);
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        let terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for surface snapshot"))?;
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn apply_local_image(&self, image: LocalTerminalImage) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for local image"))?;
        let surface = terminal.surface_state(self.session_id);
        if !surface_allows_inline_image(&surface) {
            return Err(anyhow!(
                "local clipboard images are unavailable in the current terminal mode"
            ));
        }
        terminal.apply_local_image(image)?;
        Ok(terminal.surface_state(self.session_id))
    }

    pub fn release_terminal_memory(&self) -> Result<()> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for memory release"))?;
        terminal.release_memory();
        Ok(())
    }

    pub fn disconnect(&self) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Disconnect)
            .map_err(|_| anyhow!("ssh runtime disconnect channel is closed"))
    }
}

fn apply_initial_remote_output(
    terminal: &Arc<Mutex<TerminalSession>>,
    event_tx: &mpsc::UnboundedSender<SessionRuntimeEvent>,
    bytes: &[u8],
) -> Vec<u8> {
    let parsed = runtime_shell_events(bytes);
    if let Some(cwd) = parsed.cwd {
        let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd));
    }
    if !parsed.sanitized_bytes.is_empty() {
        apply_remote_output(terminal, &parsed.sanitized_bytes)
    } else {
        Vec::new()
    }
}

impl SessionRuntimeControl for SshSessionRuntime {
    fn disconnect(&self) -> Result<()> {
        SshSessionRuntime::disconnect(self)
    }

    fn release_terminal_memory(&self) -> Result<()> {
        SshSessionRuntime::release_terminal_memory(self)
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        SshSessionRuntime::send_text_input(self, text)
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        SshSessionRuntime::send_key_input(self, event)
    }

    fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        SshSessionRuntime::resize(self, rows, cols)
    }

    fn resize_with_viewport(
        &self,
        rows: u32,
        cols: u32,
        viewport: TerminalViewportMetrics,
    ) -> Result<()> {
        SshSessionRuntime::resize_with_viewport(self, rows, cols, viewport)
    }

    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        SshSessionRuntime::send_mouse_input(self, event)
    }

    fn send_paste(&self, text: String) -> Result<()> {
        SshSessionRuntime::send_paste(self, text)
    }

    fn apply_local_image(&self, image: LocalTerminalImage) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::apply_local_image(self, image)
    }

    fn start_zmodem_upload(&self, local_paths: Vec<PathBuf>) -> Result<()> {
        SshSessionRuntime::start_zmodem_upload(self, local_paths)
    }

    fn start_interactive_zmodem_upload(&self, local_paths: Vec<PathBuf>) -> Result<()> {
        SshSessionRuntime::start_interactive_zmodem_upload(self, local_paths)
    }

    fn remote_command_exists(&self, command_name: String) -> Result<bool> {
        SshSessionRuntime::remote_command_exists(self, command_name)
    }

    fn resolve_current_working_directory(&self) -> Result<Option<String>> {
        SshSessionRuntime::resolve_current_working_directory(self)
    }

    fn start_zmodem_upload_to_remote_dir(
        &self,
        local_paths: Vec<PathBuf>,
        remote_dir: String,
    ) -> Result<()> {
        SshSessionRuntime::start_zmodem_upload_to_remote_dir(self, local_paths, remote_dir)
    }

    fn start_zmodem_download(
        &self,
        local_dir: PathBuf,
        conflict_policy: ZmodemDownloadConflictPolicy,
    ) -> Result<()> {
        SshSessionRuntime::start_zmodem_download(self, local_dir, conflict_policy)
    }

    fn cancel_zmodem_transfer(&self) -> Result<()> {
        SshSessionRuntime::cancel_zmodem_transfer(self)
    }

    fn dismiss_zmodem_transfer(&self, expected_state: ZmodemTransferState) -> Result<()> {
        SshSessionRuntime::dismiss_zmodem_transfer(self, expected_state)
    }

    fn selection_text_from_buffer_rows(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<Option<String>> {
        SshSessionRuntime::selection_text_from_buffer_rows(
            self, start_row, start_col, end_row, end_col,
        )
        .map(Some)
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::terminal_surface(self)
    }

    fn update_theme(
        &self,
        mode: ThemeMode,
        variant: ThemeVariant,
    ) -> Result<Option<TerminalSurfaceState>> {
        SshSessionRuntime::update_theme(self, mode, variant).map(Some)
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::scroll_viewport_lines(self, delta)
    }

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.sftp_runtime.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::terminal::visible_lines_from_rows;
    use super::*;

    #[test]
    fn ssh_client_config_uses_keepalive_without_inactivity_disconnects() {
        let config = ssh_client_config();

        assert_eq!(config.inactivity_timeout, None);
        assert_eq!(config.keepalive_interval, Some(SSH_KEEPALIVE_INTERVAL));
        assert_eq!(config.keepalive_max, SSH_KEEPALIVE_MAX_MISSES);
        assert!(config.nodelay);
    }

    #[test]
    fn visible_lines_from_rows_trims_only_outer_empty_rows() {
        let rows = vec![
            TerminalRowState {
                index: 0,
                text: String::new(),
                wrapped: false,
            },
            TerminalRowState {
                index: 1,
                text: "top".into(),
                wrapped: false,
            },
            TerminalRowState {
                index: 2,
                text: String::new(),
                wrapped: false,
            },
            TerminalRowState {
                index: 3,
                text: "bottom".into(),
                wrapped: false,
            },
            TerminalRowState {
                index: 4,
                text: String::new(),
                wrapped: false,
            },
        ];

        assert_eq!(
            visible_lines_from_rows(&rows),
            vec!["top".to_string(), String::new(), "bottom".to_string()]
        );
    }

    #[test]
    fn terminal_runtime_defaults_store_clamps_scrollback_to_positive_values() {
        let defaults = TerminalRuntimeDefaults::default();

        assert_eq!(
            defaults.scrollback_lines(),
            DEFAULT_TERMINAL_SCROLLBACK_LINES
        );

        defaults.set_scrollback_lines(3000);
        assert_eq!(defaults.scrollback_lines(), 3000);

        defaults.set_scrollback_lines(0);
        assert_eq!(defaults.scrollback_lines(), 1);
    }

    #[test]
    fn terminal_runtime_defaults_store_theme_mode_and_variant() {
        let defaults = TerminalRuntimeDefaults::default();

        assert_eq!(defaults.theme_mode(), ThemeMode::Dark);
        assert_eq!(defaults.theme_variant(), ThemeVariant::PremiumDefault);

        defaults.set_theme(ThemeMode::Light, ThemeVariant::LegacyHackerGreen);
        assert_eq!(defaults.theme_mode(), ThemeMode::Light);
        assert_eq!(defaults.theme_variant(), ThemeVariant::LegacyHackerGreen);
    }

    #[test]
    fn terminal_runtime_defaults_store_live_terminal_viewport_contract() {
        let defaults = TerminalRuntimeDefaults::default();

        assert_eq!(defaults.viewport_rows(), DEFAULT_TERMINAL_ROWS);
        assert_eq!(defaults.viewport_cols(), DEFAULT_TERMINAL_COLS);
        assert_eq!(
            defaults.viewport_pixel_width(),
            (DEFAULT_TERMINAL_COLS * 8) as u32
        );
        assert_eq!(
            defaults.viewport_pixel_height(),
            (DEFAULT_TERMINAL_ROWS * 16) as u32
        );
        assert_eq!(defaults.viewport_dpi(), 96);

        let viewport = TerminalViewportMetrics::new(1584, 1056, 144);
        defaults.set_viewport_metrics(48, 132, viewport);
        assert_eq!(defaults.viewport_rows(), 48);
        assert_eq!(defaults.viewport_cols(), 132);
        assert_eq!(defaults.viewport_pixel_width(), 1584);
        assert_eq!(defaults.viewport_pixel_height(), 1056);
        assert_eq!(defaults.viewport_dpi(), 144);
        assert_eq!(defaults.viewport(), (48, 132, viewport));

        defaults.set_viewport_size(0, 0, 0, 0);
        assert_eq!(defaults.viewport_rows(), 1);
        assert_eq!(defaults.viewport_cols(), 1);
        assert_eq!(defaults.viewport_pixel_width(), 8);
        assert_eq!(defaults.viewport_pixel_height(), 16);
        assert_eq!(defaults.viewport_dpi(), 144);
    }

    #[test]
    fn initial_remote_output_drains_terminal_protocol_replies() {
        let terminal = Arc::new(Mutex::new(TerminalSession::new(4, 8)));
        let (event_tx, _event_rx) = mpsc::unbounded_channel();

        let replies = apply_initial_remote_output(
            &terminal,
            &event_tx,
            b"\x1b_Ga=q,s=1,v=1,i=42;YWJjZA==\x1b\\",
        );

        assert_eq!(replies, b"\x1b_Gi=42;OK\x1b\\");
    }

    #[test]
    fn exec_zmodem_cancel_routes_to_active_generation() {
        let slot = Arc::new(Mutex::new(ExecZmodemTransferSlot::default()));
        let (command_tx, mut command_rx) = mpsc::unbounded_channel();
        let generation = slot
            .lock()
            .expect("lock exec zmodem slot")
            .register(command_tx)
            .expect("register exec zmodem transfer");

        assert_eq!(
            route_exec_zmodem_cancel(&slot),
            ExecZmodemCancelRoute::Routed(generation)
        );
        assert!(matches!(
            command_rx.try_recv(),
            Ok(ExecZmodemCommand::Cancel)
        ));
    }

    #[test]
    fn stale_exec_zmodem_registration_cannot_clear_newer_generation() {
        let slot = Arc::new(Mutex::new(ExecZmodemTransferSlot::default()));
        let (first_tx, first_rx) = mpsc::unbounded_channel();
        let first = slot
            .lock()
            .expect("lock first exec zmodem slot")
            .register(first_tx)
            .expect("register first exec zmodem transfer");
        drop(first_rx);
        assert_eq!(
            route_exec_zmodem_cancel(&slot),
            ExecZmodemCancelRoute::NotActive
        );

        let (second_tx, _second_rx) = mpsc::unbounded_channel();
        let second = slot
            .lock()
            .expect("lock second exec zmodem slot")
            .register(second_tx)
            .expect("register second exec zmodem transfer");
        assert_ne!(first, second);
        assert!(
            !slot
                .lock()
                .expect("lock stale exec zmodem slot")
                .clear_if_generation(first)
        );
        assert_eq!(
            slot.lock()
                .expect("lock current exec zmodem slot")
                .active_generation(),
            Some(second)
        );
    }

    #[test]
    fn exec_zmodem_slot_rejects_overlapping_live_registration() {
        let mut slot = ExecZmodemTransferSlot::default();
        let (first_tx, _first_rx) = mpsc::unbounded_channel();
        slot.register(first_tx)
            .expect("register first exec zmodem transfer");
        let (second_tx, _second_rx) = mpsc::unbounded_channel();

        assert!(slot.register(second_tx).is_err());
    }

    #[test]
    fn stale_exec_zmodem_task_guard_cannot_clear_newer_generation() {
        let session_id = Uuid::new_v4();
        let slot = Arc::new(Mutex::new(ExecZmodemTransferSlot::default()));
        let (first_tx, _first_rx) = mpsc::unbounded_channel();
        let first = slot
            .lock()
            .expect("lock first exec zmodem slot")
            .register(first_tx)
            .expect("register first exec zmodem transfer");
        let stale_registration = ExecZmodemTransferRegistration {
            session_id,
            slot: Arc::downgrade(&slot),
            generation: first,
        };
        assert!(
            slot.lock()
                .expect("lock first exec zmodem cleanup")
                .clear_if_generation(first)
        );
        let (second_tx, _second_rx) = mpsc::unbounded_channel();
        let second = slot
            .lock()
            .expect("lock second exec zmodem slot")
            .register(second_tx)
            .expect("register second exec zmodem transfer");

        drop(stale_registration);

        assert_eq!(
            slot.lock()
                .expect("lock current exec zmodem slot")
                .active_generation(),
            Some(second)
        );
    }
}
