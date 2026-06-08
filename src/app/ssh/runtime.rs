//! SSH session runtime primitives and terminal-state wrapper.

mod auth;
mod contracts;
mod pump;
mod sftp_backend;
mod terminal;
mod transport;

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

use self::auth::ConnectionProgressReporter;
use self::pump::run_channel_pump;
use self::sftp_backend::RusshSftpBackend;
use self::terminal::{apply_remote_output, await_channel_success, negotiate_terminal_environment};
use self::transport::{connect_target_handle_for_profile, ssh_client_config};

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::sftp::SftpRuntimeHandle;
use crate::app::ssh::connection_progress::{ConnectionHeadlineState, ConnectionProgressEvent};
use crate::app::ssh::credentials::{CredentialStore, SystemCredentialStore};
use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::session_manager::{EnhancedSessionState, SessionRuntimeControl};
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
    viewport_rows: Arc<AtomicUsize>,
    viewport_cols: Arc<AtomicUsize>,
    viewport_pixel_width: Arc<AtomicUsize>,
    viewport_pixel_height: Arc<AtomicUsize>,
}

#[derive(Debug, Clone, Copy)]
struct TerminalRuntimeThemeDefaults {
    mode: ThemeMode,
    variant: ThemeVariant,
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
            viewport_rows: Arc::new(AtomicUsize::new(DEFAULT_TERMINAL_ROWS)),
            viewport_cols: Arc::new(AtomicUsize::new(DEFAULT_TERMINAL_COLS)),
            viewport_pixel_width: Arc::new(AtomicUsize::new(
                DEFAULT_TERMINAL_COLS * DEFAULT_TERMINAL_CELL_WIDTH_PX as usize,
            )),
            viewport_pixel_height: Arc::new(AtomicUsize::new(
                DEFAULT_TERMINAL_ROWS * DEFAULT_TERMINAL_CELL_HEIGHT_PX as usize,
            )),
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
        self.viewport_rows.load(Ordering::Relaxed).max(1)
    }

    pub fn viewport_cols(&self) -> usize {
        self.viewport_cols.load(Ordering::Relaxed).max(1)
    }

    pub fn viewport_pixel_width(&self) -> u32 {
        self.viewport_pixel_width.load(Ordering::Relaxed).max(1) as u32
    }

    pub fn viewport_pixel_height(&self) -> u32 {
        self.viewport_pixel_height.load(Ordering::Relaxed).max(1) as u32
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

        self.viewport_rows.store(rows, Ordering::Relaxed);
        self.viewport_cols.store(cols, Ordering::Relaxed);
        self.viewport_pixel_width
            .store(pixel_width as usize, Ordering::Relaxed);
        self.viewport_pixel_height
            .store(pixel_height as usize, Ordering::Relaxed);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    ConnectionProgress(ConnectionProgressEvent),
    EnhancedSessionStateChanged(EnhancedSessionState),
    CurrentDirectoryChanged(String),
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
    terminal: Arc<Mutex<TerminalSession>>,
    terminal_defaults: TerminalRuntimeDefaults,
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
    sftp_runtime: SftpRuntimeHandle,
}

#[derive(Debug)]
enum RuntimeCommand {
    TextInput(String),
    KeyInput(TerminalKeyEvent),
    MouseInput(TerminalMouseInput),
    Paste(String),
    Resize {
        rows: u32,
        cols: u32,
        pixel_width: u32,
        pixel_height: u32,
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
        let mut terminal_session = TerminalSession::new_with_scrollback(
            terminal_defaults.viewport_rows(),
            terminal_defaults.viewport_cols(),
            terminal_defaults.scrollback_lines(),
        );
        terminal_session.set_theme(
            terminal_defaults.theme_mode(),
            terminal_defaults.theme_variant(),
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
        let pty_rows = terminal_defaults.viewport_rows();
        let pty_cols = terminal_defaults.viewport_cols();
        let pty_pixel_width = terminal_defaults.viewport_pixel_width();
        let pty_pixel_height = terminal_defaults.viewport_pixel_height();
        terminal_session.resize(pty_rows, pty_cols);
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
                pty_pixel_width,
                pty_pixel_height,
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
            apply_remote_output(&terminal, &pending_output);
        }

        let handle = Arc::new(handle);
        let sftp_runtime = SftpRuntimeHandle::new(Arc::new(RusshSftpBackend {
            handle: Arc::clone(&handle),
        }));

        let runtime = Self {
            session_id,
            profile: profile.clone(),
            terminal: Arc::clone(&terminal),
            terminal_defaults,
            command_tx,
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
        let pixel_width = self.terminal_defaults.viewport_pixel_width();
        let pixel_height = self.terminal_defaults.viewport_pixel_height();
        self.command_tx
            .send(RuntimeCommand::Resize {
                rows,
                cols,
                pixel_width,
                pixel_height,
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

    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        SshSessionRuntime::send_mouse_input(self, event)
    }

    fn send_paste(&self, text: String) -> Result<()> {
        SshSessionRuntime::send_paste(self, text)
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

        defaults.set_viewport_size(48, 132, 1584, 1056);
        assert_eq!(defaults.viewport_rows(), 48);
        assert_eq!(defaults.viewport_cols(), 132);
        assert_eq!(defaults.viewport_pixel_width(), 1584);
        assert_eq!(defaults.viewport_pixel_height(), 1056);

        defaults.set_viewport_size(0, 0, 0, 0);
        assert_eq!(defaults.viewport_rows(), 1);
        assert_eq!(defaults.viewport_cols(), 1);
        assert_eq!(defaults.viewport_pixel_width(), 8);
        assert_eq!(defaults.viewport_pixel_height(), 16);
    }
}
