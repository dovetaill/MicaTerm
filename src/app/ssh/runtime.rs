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
    TerminalSurfaceSignature, TerminalSurfaceState,
};
pub use terminal::{
    TerminalSession, encode_named_key_input, extract_current_working_directory_from_osc7,
    negotiated_terminal_environment,
};

use self::auth::ConnectionProgressReporter;
use self::pump::run_channel_pump;
use self::sftp_backend::RusshSftpBackend;
use self::terminal::{apply_remote_output, await_channel_success, negotiate_terminal_environment};
use self::transport::{connect_target_handle_for_profile, ssh_client_config};

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
use crate::theme::ThemeMode;

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;
const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSH_KEEPALIVE_MAX_MISSES: usize = 3;
const FAST_SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(8);
const SURFACE_DIRTY_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(40);
const INPUT_ACTIVE_SURFACE_DIRTY_WINDOW: Duration = Duration::from_millis(160);
const WORKING_SET_TRIM_IDLE_INTERVAL: Duration = Duration::from_secs(2);
const WORKING_SET_TRIM_MIN_OUTPUT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    ConnectionProgress(ConnectionProgressEvent),
    EnhancedSessionStateChanged(EnhancedSessionState),
    CurrentDirectoryChanged(String),
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
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
    sftp_runtime: SftpRuntimeHandle,
}

#[derive(Debug)]
enum RuntimeCommand {
    TextInput(String),
    KeyInput(TerminalKeyEvent),
    MouseInput(TerminalMouseInput),
    Paste(String),
    Resize { rows: u32, cols: u32 },
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
        )
        .await
    }

    pub async fn connect_with_credential_store(
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        credential_store: Arc<dyn CredentialStore>,
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
        let terminal = Arc::new(Mutex::new(TerminalSession::new(
            DEFAULT_TERMINAL_ROWS,
            DEFAULT_TERMINAL_COLS,
        )));
        let (command_tx, command_rx) = mpsc::unbounded_channel();
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
                DEFAULT_TERMINAL_COLS as u32,
                DEFAULT_TERMINAL_ROWS as u32,
                (DEFAULT_TERMINAL_COLS * 8) as u32,
                (DEFAULT_TERMINAL_ROWS * 16) as u32,
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
        self.command_tx
            .send(RuntimeCommand::Resize { rows, cols })
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

    pub fn update_theme_mode(&self, mode: ThemeMode) -> Result<TerminalSurfaceState> {
        let mut terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("failed to lock terminal for theme update"))?;
        terminal.set_theme_mode(mode);
        Ok(terminal.surface_state(self.session_id))
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

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::terminal_surface(self)
    }

    fn update_theme_mode(&self, mode: ThemeMode) -> Result<Option<TerminalSurfaceState>> {
        SshSessionRuntime::update_theme_mode(self, mode).map(Some)
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
}
