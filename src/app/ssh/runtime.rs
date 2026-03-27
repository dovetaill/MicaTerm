//! SSH session runtime primitives and terminal-state wrapper.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{error::Error as StdError, fmt};

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::ChannelMsg;
use russh::Disconnect;
use russh::client;
use russh::client::AuthResult;
use russh::keys::{self, PrivateKeyWithHashAlg};
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers as KeyModifiers};
use tokio::sync::mpsc;
use uuid::Uuid;
use wezterm_term::color::{ColorPalette, ColorAttribute, RgbColor, SrgbaTuple};
use wezterm_term::{Line, Terminal, TerminalConfiguration, TerminalSize};
use wezterm_surface::{CursorShape, CursorVisibility};

use crate::app::ssh::credentials::{
    CredentialStore, StoredSecretLookupError, StoredSshSecretBundle, SystemCredentialStore,
    load_secret_bundle_with_diagnostics, required_secret_bundle_field,
};
use crate::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{ConnectionProfile, SshAuthMethod};
use crate::app::ssh::session_manager::SessionRuntimeControl;
use crate::theme::ThemeMode;

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;
const SSH_KEEPALIVE_INTERVAL: Duration = Duration::from_secs(15);
const SSH_KEEPALIVE_MAX_MISSES: usize = 3;
const FILTERED_EXACT_BANNER: &str =
    "Activate the web console with: systemctl enable --now cockpit.socket";

fn ssh_client_config() -> client::Config {
    client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(SSH_KEEPALIVE_INTERVAL),
        keepalive_max: SSH_KEEPALIVE_MAX_MISSES,
        nodelay: true,
        ..Default::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    pub visible_rows: Vec<TerminalRowState>,
    pub visible_lines: Vec<String>,
    pub cells: Vec<TerminalCellState>,
    pub cursor: TerminalCursorState,
    pub mouse_grabbed: bool,
    pub bracketed_paste_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRowState {
    pub index: u32,
    pub text: String,
    pub wrapped: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCellState {
    pub row: u32,
    pub col: u32,
    pub width: u32,
    pub text: String,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalCursorShape {
    Block,
    Underline,
    Bar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCursorState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub blinking: bool,
    pub shape: TerminalCursorShape,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseEventKind {
    Down,
    Up,
    Move,
    Scroll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalMouseButton {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalMouseInput {
    pub kind: TerminalMouseEventKind,
    pub button: TerminalMouseButton,
    pub row: u32,
    pub col: u32,
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKeyKind {
    Named(&'static str),
    Function(u8),
    Char(char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalKeyEvent {
    pub key: TerminalKeyKind,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl TerminalKeyEvent {
    pub fn named(key_name: &'static str, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Named(key_name),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn function(number: u8, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Function(number),
            alt,
            ctrl,
            shift,
        }
    }

    pub fn character(ch: char, alt: bool, ctrl: bool, shift: bool) -> Self {
        Self {
            key: TerminalKeyKind::Char(ch),
            alt,
            ctrl,
            shift,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    SurfaceChanged(TerminalSurfaceState),
    Disconnected,
    Error(String),
}

pub struct SshSessionRuntime {
    session_id: Uuid,
    #[allow(dead_code)]
    profile: ConnectionProfile,
    terminal: Arc<Mutex<TerminalSession>>,
    command_tx: mpsc::UnboundedSender<RuntimeCommand>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownHostKeyError {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub public_key_openssh: String,
}

impl fmt::Display for UnknownHostKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "unknown SSH host key for `{}`:{} ({})",
            self.host, self.port, self.fingerprint
        )
    }
}

impl StdError for UnknownHostKeyError {}

#[derive(Debug)]
enum RuntimeCommand {
    TextInput(String),
    KeyInput(TerminalKeyEvent),
    MouseInput(TerminalMouseInput),
    Paste(String),
    Resize { rows: u32, cols: u32 },
    Disconnect,
}

struct RuntimeClientHandler {
    host: String,
    port: u16,
    known_hosts: KnownHostsService,
}

impl client::Handler for RuntimeClientHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        match self
            .known_hosts
            .check(&self.host, self.port, server_public_key)?
        {
            KnownHostCheck::Trusted => Ok(true),
            KnownHostCheck::Unknown { fingerprint } => Err(UnknownHostKeyError {
                host: self.host.clone(),
                port: self.port,
                fingerprint,
                public_key_openssh: server_public_key
                    .to_openssh()
                    .context("failed to encode unknown SSH host key")?,
            }
            .into()),
            KnownHostCheck::Changed { expected, actual } => bail!(
                "SSH host key changed for `{}`:{} (expected {}, got {})",
                self.host,
                self.port,
                expected,
                actual
            ),
        }
    }
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Result<Self> {
        Self::connect_with_credential_store(
            profile,
            session_id,
            event_tx,
            Arc::new(SystemCredentialStore),
        )
        .await
    }

    pub async fn connect_with_credential_store(
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        credential_store: Arc<dyn CredentialStore>,
    ) -> Result<Self> {
        let terminal = Arc::new(Mutex::new(TerminalSession::new(
            DEFAULT_TERMINAL_ROWS,
            DEFAULT_TERMINAL_COLS,
        )));
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        let known_hosts = KnownHostsService::new(default_known_hosts_path()?);
        let handler = RuntimeClientHandler {
            host: profile.host.clone(),
            port: profile.port,
            known_hosts,
        };
        let config = Arc::new(ssh_client_config());

        let mut handle = client::connect(config, (profile.host.as_str(), profile.port), handler)
            .await
            .with_context(|| {
                format!(
                    "failed to connect to SSH server `{}:{}`",
                    profile.host, profile.port
                )
            })?;

        authenticate_client(&mut handle, &profile, credential_store.as_ref()).await?;

        let mut channel = handle
            .channel_open_session()
            .await
            .context("failed to open SSH session channel")?;
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

        channel
            .request_shell(true)
            .await
            .context("failed to request remote shell")?;
        await_channel_success(&mut channel, "shell", &mut pending_output).await?;

        let _ = event_tx.send(SessionRuntimeEvent::Connected);
        if !pending_output.is_empty() {
            apply_remote_output(&terminal, &pending_output);
        }
        if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
        }

        let runtime = Self {
            session_id,
            profile: profile.clone(),
            terminal: Arc::clone(&terminal),
            command_tx,
        };

        tokio::spawn(run_channel_pump(
            session_id, handle, channel, terminal, event_tx, command_rx,
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

    fn update_theme_mode(&self, mode: ThemeMode) -> Result<Option<TerminalSurfaceState>> {
        SshSessionRuntime::update_theme_mode(self, mode).map(Some)
    }

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        SshSessionRuntime::scroll_viewport_lines(self, delta)
    }
}

async fn authenticate_client(
    handle: &mut client::Handle<RuntimeClientHandler>,
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    match profile.auth_method {
        SshAuthMethod::Password => {
            let password = match profile
                .password
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(password) => password,
                None => {
                    let stored_bundle = load_required_stored_secret_bundle(
                        profile,
                        credential_store,
                        "SSH password secret",
                    )?;
                    require_profile_secret_field(
                        profile,
                        "SSH password secret",
                        stored_bundle.as_ref(),
                        "password",
                    )?
                }
            };
            let auth_result = handle
                .authenticate_password(profile.user.clone(), password)
                .await
                .context("password authentication failed")?;
            ensure_auth_success(auth_result, "password")?;
        }
        SshAuthMethod::PrivateKeyPath => {
            let private_key_path = profile
                .private_key_path
                .as_deref()
                .filter(|path| !path.trim().is_empty())
                .ok_or_else(|| anyhow!("missing private key path for `{}`", profile.name))?;
            let stored_bundle = if profile
                .passphrase
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                None
            } else {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH passphrase secret",
                        &err,
                    ))
                })?
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::load_secret_key(private_key_path, passphrase.as_deref())
                .with_context(|| {
                    format!("failed to load SSH private key from `{private_key_path}`")
                })?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("private key path authentication failed")?;
            ensure_auth_success(auth_result, "private key path")?;
        }
        SshAuthMethod::PrivateKeyContent => {
            let stored_bundle = if profile
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                load_optional_stored_secret_bundle(profile, credential_store).map_err(|err| {
                    anyhow!(stored_secret_lookup_message(
                        profile,
                        "SSH inline private key secret",
                        &err,
                    ))
                })?
            } else {
                load_required_stored_secret_bundle(
                    profile,
                    credential_store,
                    "SSH inline private key secret",
                )?
            };
            let private_key_content = match profile
                .private_key_content
                .clone()
                .filter(|value| !value.trim().is_empty())
            {
                Some(private_key_content) => private_key_content,
                None => require_profile_secret_field(
                    profile,
                    "SSH inline private key secret",
                    stored_bundle.as_ref(),
                    "private_key_content",
                )?,
            };
            let passphrase = profile
                .passphrase
                .clone()
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    stored_bundle
                        .as_ref()
                        .and_then(|(_, bundle)| non_empty_secret(bundle.passphrase.as_deref()))
                });
            let private_key = keys::decode_secret_key(&private_key_content, passphrase.as_deref())
                .context("failed to decode inline SSH private key")?;
            let auth_result = handle
                .authenticate_publickey(
                    profile.user.clone(),
                    PrivateKeyWithHashAlg::new(
                        Arc::new(private_key),
                        handle.best_supported_rsa_hash().await?.flatten(),
                    ),
                )
                .await
                .context("inline private key authentication failed")?;
            ensure_auth_success(auth_result, "inline private key")?;
        }
    }

    Ok(())
}

fn ensure_auth_success(result: AuthResult, method: &str) -> Result<()> {
    if result.success() {
        Ok(())
    } else {
        bail!("SSH authentication was rejected for {method}")
    }
}

pub(crate) fn load_optional_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> std::result::Result<Option<(String, StoredSshSecretBundle)>, StoredSecretLookupError> {
    let Some(credential_ref) = profile.credential_ref.as_deref() else {
        return Ok(None);
    };

    let bundle = load_secret_bundle_with_diagnostics(credential_store, Some(credential_ref))?;
    let bundle = match profile.auth_method {
        SshAuthMethod::Password => bundle,
        SshAuthMethod::PrivateKeyContent
            if bundle
                .private_key_content
                .as_ref()
                .is_some_and(|value| !value.trim().is_empty()) =>
        {
            bundle
        }
        SshAuthMethod::PrivateKeyContent => StoredSshSecretBundle {
            private_key_content: bundle.password,
            passphrase: bundle.passphrase,
            ..StoredSshSecretBundle::default()
        },
        SshAuthMethod::PrivateKeyPath => bundle,
    };
    Ok(Some((credential_ref.to_string(), bundle)))
}

fn load_required_stored_secret_bundle(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
    secret_label: &str,
) -> Result<Option<(String, StoredSshSecretBundle)>> {
    load_optional_stored_secret_bundle(profile, credential_store)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

fn require_profile_secret_field(
    profile: &ConnectionProfile,
    secret_label: &str,
    stored_bundle: Option<&(String, StoredSshSecretBundle)>,
    field: &'static str,
) -> Result<String> {
    let Some((credential_ref, bundle)) = stored_bundle else {
        return Err(anyhow!(stored_secret_lookup_message(
            profile,
            secret_label,
            &StoredSecretLookupError::MissingCredentialRef,
        )));
    };

    required_secret_bundle_field(bundle, credential_ref, field)
        .map_err(|err| anyhow!(stored_secret_lookup_message(profile, secret_label, &err)))
}

pub(crate) fn stored_secret_lookup_message(
    profile: &ConnectionProfile,
    secret_label: &str,
    error: &StoredSecretLookupError,
) -> String {
    match error {
        StoredSecretLookupError::MissingCredentialRef => format!(
            "missing credential binding for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::MissingEntry { credential_ref } => format!(
            "missing saved entry `{credential_ref}` for {secret_label} on `{}`",
            profile.name
        ),
        StoredSecretLookupError::ReadFailed {
            credential_ref,
            message,
        } => format!(
            "failed to read saved entry `{credential_ref}` for {secret_label} on `{}`: {message}",
            profile.name
        ),
        StoredSecretLookupError::EmptyBundleField {
            credential_ref,
            field,
        } => format!(
            "saved entry `{credential_ref}` for `{}` is missing field `{field}` required by {secret_label}",
            profile.name
        ),
    }
}

fn non_empty_secret(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

async fn await_channel_success(
    channel: &mut Channel<client::Msg>,
    request_label: &str,
    pending_output: &mut Vec<u8>,
) -> Result<()> {
    loop {
        let Some(message) = channel.wait().await else {
            bail!("SSH channel closed before `{request_label}` completed");
        };

        match message {
            ChannelMsg::Success => return Ok(()),
            ChannelMsg::Failure => bail!("SSH channel rejected `{request_label}` request"),
            ChannelMsg::Data { data } | ChannelMsg::ExtendedData { data, .. } => {
                pending_output.extend_from_slice(data.as_ref());
            }
            ChannelMsg::Close | ChannelMsg::Eof => {
                bail!("SSH channel closed during `{request_label}` request");
            }
            _ => {}
        }
    }
}

async fn run_channel_pump(
    session_id: Uuid,
    handle: client::Handle<RuntimeClientHandler>,
    mut channel: Channel<client::Msg>,
    terminal: Arc<Mutex<TerminalSession>>,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    mut command_rx: mpsc::UnboundedReceiver<RuntimeCommand>,
) {
    let mut command_channel_open = true;

    loop {
        tokio::select! {
            maybe_command = command_rx.recv(), if command_channel_open => {
                match maybe_command {
                    Some(RuntimeCommand::TextInput(text)) => {
                        let bytes = text.into_bytes();
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::KeyInput(event)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_key_event(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode key input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for key input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} key bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::MouseInput(event)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.send_mouse_input(event) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode mouse input for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for mouse input".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} mouse bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Paste(text)) => {
                        let bytes = match terminal.lock() {
                            Ok(mut terminal) => match terminal.encode_paste(&text) {
                                Ok(bytes) => bytes,
                                Err(err) => {
                                    let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                        "failed to encode paste for SSH channel: {err}"
                                    )));
                                    break;
                                }
                            },
                            Err(_) => {
                                let _ = event_tx.send(SessionRuntimeEvent::Error(
                                    "failed to lock terminal for paste".into()
                                ));
                                break;
                            }
                        };
                        if bytes.is_empty() {
                            continue;
                        }
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} paste bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Resize { rows, cols }) => {
                        if let Ok(mut terminal) = terminal.lock() {
                            terminal.resize(rows as usize, cols as usize);
                        }
                        if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
                        }
                        if let Err(err) = channel
                            .window_change(cols, rows, cols.saturating_mul(8), rows.saturating_mul(16))
                            .await
                        {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to resize SSH PTY: {err}"
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Disconnect) => {
                        let _ = channel.eof().await;
                        let _ = channel.close().await;
                        let _ = handle
                            .disconnect(Disconnect::ByApplication, "session closed", "en-US")
                            .await;
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    None => {
                        command_channel_open = false;
                    }
                }
            }
            maybe_message = channel.wait() => {
                match maybe_message {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        apply_remote_output(&terminal, data.as_ref());
                        if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
                            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
                        }
                    }
                    Some(ChannelMsg::Close) | Some(ChannelMsg::Eof) | None => {
                        let _ = event_tx.send(SessionRuntimeEvent::Disconnected);
                        break;
                    }
                    Some(ChannelMsg::Failure) => {
                        let _ = event_tx.send(SessionRuntimeEvent::Error(
                            "remote SSH channel reported failure".into()
                        ));
                        break;
                    }
                    Some(_) => {}
                }
            }
        }
    }
}

fn apply_remote_output(terminal: &Arc<Mutex<TerminalSession>>, bytes: &[u8]) {
    if let Ok(mut terminal) = terminal.lock() {
        terminal.apply_remote_bytes(bytes);
    }
}

fn snapshot_terminal_surface(
    terminal: &Arc<Mutex<TerminalSession>>,
    session_id: Uuid,
) -> Option<TerminalSurfaceState> {
    terminal
        .lock()
        .ok()
        .map(|terminal| terminal.surface_state(session_id))
}

pub struct TerminalSession {
    terminal: Terminal,
    config: Arc<SessionTerminalConfig>,
    writer: SharedWriteBuffer,
    fallback_mouse_button: Option<TerminalMouseButton>,
    pending_remote_line_buffer: PendingRemoteLineBuffer,
    keyboard_modes: TerminalKeyboardModes,
    viewport_offset_lines: usize,
}

impl TerminalSession {
    pub fn new(rows: usize, cols: usize) -> Self {
        let writer = SharedWriteBuffer::default();
        let config = Arc::new(SessionTerminalConfig::new(ThemeMode::Dark));
        let terminal = Terminal::new(
            TerminalSize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
                dpi: 96,
            },
            config.clone(),
            "MicaTerm",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer.clone()),
        );

        Self {
            terminal,
            config,
            writer,
            fallback_mouse_button: None,
            pending_remote_line_buffer: PendingRemoteLineBuffer::default(),
            keyboard_modes: TerminalKeyboardModes::default(),
            viewport_offset_lines: 0,
        }
    }

    pub fn sequence_number(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        let filtered = self.pending_remote_line_buffer.push_and_filter(bytes);
        self.keyboard_modes.observe(filtered.as_slice());
        if !filtered.is_empty() {
            self.snap_viewport_to_bottom();
            self.terminal.advance_bytes(filtered.as_slice());
        }
        self.clamp_viewport_offset();
    }

    pub fn screen_text(&self) -> String {
        self.visible_lines().join("\n")
    }

    pub fn visible_rows(&self) -> Vec<TerminalRowState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut rows = Vec::with_capacity(size.rows.max(1));
        self.terminal.screen().for_each_phys_line(|phys_idx, line| {
            if phys_idx < visible_start || phys_idx >= visible_end {
                return;
            }

            rows.push(project_terminal_row(
                line,
                (phys_idx - visible_start) as u32,
                size.cols.max(1),
            ));
        });

        while rows.len() < size.rows.max(1) {
            rows.push(TerminalRowState {
                index: rows.len() as u32,
                text: String::new(),
                wrapped: false,
            });
        }

        rows
    }

    pub fn visible_lines(&self) -> Vec<String> {
        let mut lines = self
            .visible_rows()
            .into_iter()
            .map(|row| row.text)
            .collect::<Vec<_>>();
        while lines.first().is_some_and(String::is_empty) {
            let _ = lines.remove(0);
        }
        while lines.last().is_some_and(String::is_empty) {
            let _ = lines.pop();
        }
        lines
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.terminal.resize(TerminalSize {
            rows: rows.max(1),
            cols: cols.max(1),
            pixel_width: cols.max(1) * 8,
            pixel_height: rows.max(1) * 16,
            dpi: 96,
        });
        self.clamp_viewport_offset();
    }

    pub fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
        let size = self.terminal.get_size();
        let palette = self.terminal.palette();
        let visible_rows = self.visible_rows();
        let cells = self.visible_cells(&palette);
        let cursor = self.cursor_state(&palette);
        TerminalSurfaceState {
            session_id,
            seqno: self.sequence_number(),
            rows: size.rows as u32,
            cols: size.cols as u32,
            viewport_offset_lines: self.viewport_offset_lines as u32,
            viewport_max_offset_lines: self.max_viewport_offset_lines() as u32,
            viewport_at_bottom: self.viewport_offset_lines == 0,
            visible_lines: self.visible_lines(),
            visible_rows,
            cells,
            cursor,
            mouse_grabbed: self.terminal.is_mouse_grabbed(),
            bracketed_paste_enabled: self.terminal.bracketed_paste_enabled(),
        }
    }

    pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        let key = match event.key {
            TerminalKeyKind::Named(name) => match named_key_code(name) {
                Some(key) => key,
                None => bail!("unsupported named terminal key `{name}`"),
            },
            TerminalKeyKind::Function(number) => KeyCode::Function(number),
            TerminalKeyKind::Char(ch) => KeyCode::Char(ch),
        };

        self.send_key_down(key, key_modifiers(event.alt, event.ctrl, event.shift))
    }

    pub fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let sanitized = text.replace("\x1b[200~", "").replace("\x1b[201~", "");
        if self.terminal.bracketed_paste_enabled() {
            Ok(format!("\x1b[200~{sanitized}\x1b[201~").into_bytes())
        } else {
            Ok(sanitized.into_bytes())
        }
    }

    pub fn scroll_viewport_lines(&mut self, delta: i32) {
        if delta > 0 {
            self.viewport_offset_lines = self
                .viewport_offset_lines
                .saturating_add(delta as usize);
        } else if delta < 0 {
            self.viewport_offset_lines = self
                .viewport_offset_lines
                .saturating_sub(delta.unsigned_abs() as usize);
        }
        self.clamp_viewport_offset();
    }

    pub fn scroll_viewport_to_top(&mut self) {
        self.viewport_offset_lines = self.max_viewport_offset_lines();
    }

    pub fn scroll_viewport_to_bottom(&mut self) {
        self.viewport_offset_lines = 0;
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        if self.config.set_theme_mode(mode) {
            self.terminal.increment_seqno();
        }
    }

    pub fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.snap_viewport_to_bottom();
        let encoded = key.encode(
            modifiers,
            KeyCodeEncodeModes {
                encoding: KeyboardEncoding::Xterm,
                newline_mode: false,
                application_cursor_keys: self.keyboard_modes.application_cursor_keys,
                modify_other_keys: self.keyboard_modes.modify_other_keys,
            },
            true,
        )?;
        let bytes = encoded.into_bytes();

        let mut writer = self.writer.clone();
        writer.write_all(&bytes)?;
        writer.flush()?;

        Ok(self.writer.take())
    }

    pub fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        let fallback_button = self.resolve_fallback_mouse_button(event);
        self.terminal.mouse_event(wezterm_term::MouseEvent {
            kind: match event.kind {
                TerminalMouseEventKind::Down => wezterm_term::MouseEventKind::Press,
                TerminalMouseEventKind::Up => wezterm_term::MouseEventKind::Release,
                TerminalMouseEventKind::Move => wezterm_term::MouseEventKind::Move,
                TerminalMouseEventKind::Scroll => wezterm_term::MouseEventKind::Press,
            },
            x: event.col as usize,
            y: event.row as i64,
            x_pixel_offset: 0,
            y_pixel_offset: 0,
            button: match event.button {
                TerminalMouseButton::Left => wezterm_term::MouseButton::Left,
                TerminalMouseButton::Middle => wezterm_term::MouseButton::Middle,
                TerminalMouseButton::Right => wezterm_term::MouseButton::Right,
                TerminalMouseButton::WheelUp => wezterm_term::MouseButton::WheelUp(1),
                TerminalMouseButton::WheelDown => wezterm_term::MouseButton::WheelDown(1),
                TerminalMouseButton::None => wezterm_term::MouseButton::None,
            },
            modifiers: mouse_modifiers(event),
        })?;

        let bytes = self.writer.take();
        if !self.terminal.is_mouse_grabbed() {
            return Ok(bytes);
        }
        if matches!(
            event.kind,
            TerminalMouseEventKind::Down | TerminalMouseEventKind::Scroll
        ) && !bytes.is_empty()
        {
            return Ok(bytes);
        }

        Ok(encode_sgr_mouse_fallback(event, fallback_button))
    }

    fn visible_cells(&self, palette: &ColorPalette) -> Vec<TerminalCellState> {
        let size = self.terminal.get_size();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let mut cells = Vec::new();

        self.terminal.screen().for_each_phys_line(|phys_idx, line| {
            if phys_idx < visible_start || phys_idx >= visible_end {
                return;
            }

            let row = (phys_idx - visible_start) as u32;
            for cell in line.visible_cells() {
                if cell.cell_index() >= size.cols {
                    continue;
                }

                let (fg_rgba, bg_rgba) = resolve_cell_colors(palette, cell.attrs());
                cells.push(TerminalCellState {
                    row,
                    col: cell.cell_index() as u32,
                    width: cell.width() as u32,
                    text: cell.str().to_string(),
                    fg_rgba,
                    bg_rgba,
                });
            }
        });

        cells
    }

    fn cursor_state(&self, palette: &ColorPalette) -> TerminalCursorState {
        let cursor = self.terminal.cursor_pos();
        let (visible_start, visible_end) = self.visible_phys_row_bounds();
        let cursor_phys = self.terminal.screen().phys_row(cursor.y);
        let cursor_visible = matches!(cursor.visibility, CursorVisibility::Visible)
            && cursor_phys >= visible_start
            && cursor_phys < visible_end;
        TerminalCursorState {
            row: cursor_phys.saturating_sub(visible_start) as u32,
            col: cursor.x as u32,
            visible: cursor_visible,
            blinking: cursor_shape_blinks(cursor.shape),
            shape: project_cursor_shape(cursor.shape),
            fg_rgba: pack_color(palette.cursor_fg),
            bg_rgba: pack_color(palette.cursor_bg),
        }
    }

    fn resolve_fallback_mouse_button(
        &mut self,
        event: TerminalMouseInput,
    ) -> TerminalMouseButton {
        match event.kind {
            TerminalMouseEventKind::Down => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button.unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Move => {
                if event.button != TerminalMouseButton::None {
                    self.fallback_mouse_button = Some(event.button);
                    event.button
                } else {
                    self.fallback_mouse_button.unwrap_or(TerminalMouseButton::None)
                }
            }
            TerminalMouseEventKind::Up => {
                let effective = if event.button != TerminalMouseButton::None {
                    event.button
                } else {
                    self.fallback_mouse_button.unwrap_or(TerminalMouseButton::None)
                };
                self.fallback_mouse_button = None;
                effective
            }
            TerminalMouseEventKind::Scroll => event.button,
        }
    }

    fn visible_phys_row_bounds(&self) -> (usize, usize) {
        let size = self.terminal.get_size();
        let visible_rows = size.rows.max(1);
        let visible_start = self
            .terminal
            .screen()
            .scrollback_or_visible_row(-(self.viewport_offset_lines as i32));
        let visible_end = visible_start.saturating_add(visible_rows);
        (visible_start, visible_end)
    }

    fn max_viewport_offset_lines(&self) -> usize {
        let size = self.terminal.get_size();
        self.terminal
            .screen()
            .scrollback_rows()
            .saturating_sub(size.rows.max(1))
    }

    fn clamp_viewport_offset(&mut self) {
        self.viewport_offset_lines = self
            .viewport_offset_lines
            .min(self.max_viewport_offset_lines());
    }

    #[allow(dead_code)]
    fn snap_viewport_to_bottom(&mut self) {
        if self.viewport_offset_lines > 0 {
            self.scroll_viewport_to_bottom();
        }
    }
}

#[derive(Debug, Default)]
struct PendingRemoteLineBuffer {
    bytes: Vec<u8>,
    passthrough_until_newline: bool,
}

impl PendingRemoteLineBuffer {
    fn push_and_filter(&mut self, incoming: &[u8]) -> Vec<u8> {
        let mut forwarded = Vec::with_capacity(incoming.len());

        for &byte in incoming {
            if self.passthrough_until_newline {
                forwarded.push(byte);
                if byte == b'\n' {
                    self.passthrough_until_newline = false;
                }
                continue;
            }

            self.bytes.push(byte);
            if byte == b'\n' {
                if !matches_filtered_exact_banner(&self.bytes) {
                    forwarded.extend_from_slice(&self.bytes);
                }
                self.bytes.clear();
                continue;
            }

            if !matches_filtered_banner_prefix(&self.bytes) {
                forwarded.extend_from_slice(&self.bytes);
                self.bytes.clear();
                self.passthrough_until_newline = true;
            }
        }

        forwarded
    }
}

#[derive(Debug, Default)]
struct TerminalKeyboardModes {
    application_cursor_keys: bool,
    modify_other_keys: Option<i64>,
    trailing_bytes: Vec<u8>,
}

impl TerminalKeyboardModes {
    fn observe(&mut self, incoming: &[u8]) {
        let mut observed = Vec::with_capacity(self.trailing_bytes.len() + incoming.len());
        observed.extend_from_slice(&self.trailing_bytes);
        observed.extend_from_slice(incoming);

        for index in 0..observed.len() {
            let remaining = &observed[index..];
            if remaining.starts_with(b"\x1b[?1h") {
                self.application_cursor_keys = true;
                continue;
            }
            if remaining.starts_with(b"\x1b[?1l") {
                self.application_cursor_keys = false;
                continue;
            }
        }

        const MAX_TRAILING_BYTES: usize = 8;
        if observed.len() <= MAX_TRAILING_BYTES {
            self.trailing_bytes = observed;
        } else {
            self.trailing_bytes = observed[observed.len() - MAX_TRAILING_BYTES..].to_vec();
        }
    }
}

impl TerminalSurfaceState {
    pub fn from_visible_lines(
        session_id: Uuid,
        seqno: usize,
        rows: u32,
        cols: u32,
        visible_lines: Vec<String>,
    ) -> Self {
        Self {
            session_id,
            seqno,
            rows,
            cols,
            viewport_offset_lines: 0,
            viewport_max_offset_lines: 0,
            viewport_at_bottom: true,
            visible_rows: visible_lines
                .iter()
                .enumerate()
                .map(|(index, text)| TerminalRowState {
                    index: index as u32,
                    text: text.clone(),
                    wrapped: false,
                })
                .collect(),
            visible_lines,
            cells: Vec::new(),
            cursor: TerminalCursorState {
                row: 0,
                col: 0,
                visible: false,
                blinking: false,
                shape: TerminalCursorShape::Block,
                fg_rgba: 0xff00_0000,
                bg_rgba: 0xff52_ad70,
            },
            mouse_grabbed: false,
            bracketed_paste_enabled: false,
        }
    }

    pub fn selection_text(&self, start_row: u32, start_col: u32, end_row: u32, end_col: u32) -> String {
        let ((start_row, start_col), (end_row, end_col)) =
            normalized_selection((start_row, start_col), (end_row, end_col));
        let mut text = String::new();

        for row in start_row..=end_row {
            let row_start = if row == start_row { start_col } else { 0 };
            let row_end = if row == end_row {
                end_col
            } else {
                self.cols.saturating_sub(1)
            };
            let mut row_text = String::new();

            for cell in self.cells.iter().filter(|cell| cell.row == row) {
                let cell_start = cell.col;
                let cell_end = cell
                    .col
                    .saturating_add(cell.width.saturating_sub(1));
                if cell_end < row_start || cell_start > row_end {
                    continue;
                }
                row_text.push_str(&cell.text);
            }

            text.push_str(row_text.trim_end_matches(' '));
            let wrapped = self
                .visible_rows
                .iter()
                .find(|visible_row| visible_row.index == row)
                .map(|visible_row| visible_row.wrapped)
                .unwrap_or(false);
            if row < end_row && !wrapped {
                text.push('\n');
            }
        }

        text
    }
}

pub fn encode_named_key_input(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Result<Option<Vec<u8>>> {
    let Some(key) = named_key_code(key_name) else {
        return Ok(None);
    };

    let mut session = TerminalSession::new(DEFAULT_TERMINAL_ROWS, DEFAULT_TERMINAL_COLS);
    let bytes = session.send_key_down(key, key_modifiers(alt, ctrl, shift))?;
    Ok(Some(bytes))
}

#[derive(Debug)]
struct SessionTerminalConfig {
    state: Mutex<SessionTerminalConfigState>,
}

#[derive(Debug, Clone, Copy)]
struct SessionTerminalConfigState {
    theme_mode: ThemeMode,
    generation: usize,
}

impl SessionTerminalConfig {
    fn new(theme_mode: ThemeMode) -> Self {
        Self {
            state: Mutex::new(SessionTerminalConfigState {
                theme_mode,
                generation: 0,
            }),
        }
    }

    fn set_theme_mode(&self, theme_mode: ThemeMode) -> bool {
        let mut state = self.state.lock().expect("lock session terminal config");
        if state.theme_mode == theme_mode {
            return false;
        }
        state.theme_mode = theme_mode;
        state.generation = state.generation.saturating_add(1);
        true
    }

    fn theme_mode(&self) -> ThemeMode {
        self.state
            .lock()
            .expect("lock session terminal config")
            .theme_mode
    }
}

impl TerminalConfiguration for SessionTerminalConfig {
    fn generation(&self) -> usize {
        self.state
            .lock()
            .expect("lock session terminal config")
            .generation
    }

    fn color_palette(&self) -> ColorPalette {
        build_terminal_color_palette(self.theme_mode())
    }
}

fn build_terminal_color_palette(theme_mode: ThemeMode) -> ColorPalette {
    let mut palette = ColorPalette::default();

    match theme_mode {
        ThemeMode::Dark => {
            palette.colors.0[0] = rgb8(0x0d, 0x11, 0x17);
            palette.colors.0[1] = rgb8(0xff, 0x7b, 0x72);
            palette.colors.0[2] = rgb8(0x3f, 0xb9, 0x50);
            palette.colors.0[3] = rgb8(0xd2, 0x99, 0x22);
            palette.colors.0[4] = rgb8(0x58, 0xa6, 0xff);
            palette.colors.0[5] = rgb8(0xbc, 0x8c, 0xff);
            palette.colors.0[6] = rgb8(0x39, 0xc5, 0xcf);
            palette.colors.0[7] = rgb8(0xd7, 0xe2, 0xf0);
            palette.colors.0[8] = rgb8(0x6e, 0x76, 0x81);
            palette.colors.0[9] = rgb8(0xff, 0xa1, 0x98);
            palette.colors.0[10] = rgb8(0x56, 0xd3, 0x64);
            palette.colors.0[11] = rgb8(0xe3, 0xb3, 0x41);
            palette.colors.0[12] = rgb8(0x79, 0xc0, 0xff);
            palette.colors.0[13] = rgb8(0xd2, 0xa8, 0xff);
            palette.colors.0[14] = rgb8(0x56, 0xd4, 0xdd);
            palette.colors.0[15] = rgb8(0xf0, 0xf6, 0xfc);
            palette.foreground = rgb8(0xe6, 0xed, 0xf3);
            palette.background = rgb8(0x0d, 0x11, 0x17);
            palette.cursor_fg = rgb8(0x0d, 0x11, 0x17);
            palette.cursor_bg = rgb8(0x79, 0xc0, 0xff);
            palette.cursor_border = rgb8(0x79, 0xc0, 0xff);
            palette.selection_bg = rgba8(0x58, 0xa6, 0xff, 0.30);
            palette.scrollbar_thumb = rgb8(0x30, 0x36, 0x3d);
            palette.split = rgb8(0x21, 0x26, 0x2d);
        }
        ThemeMode::Light => {
            palette.colors.0[0] = rgb8(0xff, 0xff, 0xff);
            palette.colors.0[1] = rgb8(0xcf, 0x22, 0x2e);
            palette.colors.0[2] = rgb8(0x1a, 0x7f, 0x37);
            palette.colors.0[3] = rgb8(0x9a, 0x67, 0x00);
            palette.colors.0[4] = rgb8(0x09, 0x69, 0xda);
            palette.colors.0[5] = rgb8(0x82, 0x50, 0xdf);
            palette.colors.0[6] = rgb8(0x1b, 0x7c, 0x83);
            palette.colors.0[7] = rgb8(0x57, 0x60, 0x6a);
            palette.colors.0[8] = rgb8(0xd0, 0xd7, 0xde);
            palette.colors.0[9] = rgb8(0xff, 0x81, 0x82);
            palette.colors.0[10] = rgb8(0x2d, 0xa4, 0x4e);
            palette.colors.0[11] = rgb8(0xbf, 0x87, 0x00);
            palette.colors.0[12] = rgb8(0x21, 0x8b, 0xff);
            palette.colors.0[13] = rgb8(0xa4, 0x75, 0xf9);
            palette.colors.0[14] = rgb8(0x31, 0x92, 0xaa);
            palette.colors.0[15] = rgb8(0x1f, 0x23, 0x28);
            palette.foreground = rgb8(0x1f, 0x23, 0x28);
            palette.background = rgb8(0xff, 0xff, 0xff);
            palette.cursor_fg = rgb8(0xff, 0xff, 0xff);
            palette.cursor_bg = rgb8(0x09, 0x69, 0xda);
            palette.cursor_border = rgb8(0x09, 0x69, 0xda);
            palette.selection_bg = rgba8(0x09, 0x69, 0xda, 0.18);
            palette.scrollbar_thumb = rgb8(0xd0, 0xd7, 0xde);
            palette.split = rgb8(0xd8, 0xde, 0xe4);
        }
    }

    palette
}

fn rgb8(red: u8, green: u8, blue: u8) -> SrgbaTuple {
    RgbColor::new_8bpc(red, green, blue).into()
}

fn rgba8(red: u8, green: u8, blue: u8, alpha: f32) -> SrgbaTuple {
    SrgbaTuple(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    )
}

fn project_terminal_row(line: &Line, index: u32, cols: usize) -> TerminalRowState {
    TerminalRowState {
        index,
        text: line.columns_as_str(0..cols).trim_end().to_string(),
        wrapped: line.last_cell_was_wrapped(),
    }
}

fn matches_filtered_exact_banner(bytes: &[u8]) -> bool {
    normalized_remote_line(bytes) == FILTERED_EXACT_BANNER.as_bytes()
}

fn matches_filtered_banner_prefix(bytes: &[u8]) -> bool {
    let normalized = bytes.strip_suffix(b"\r").unwrap_or(bytes);
    FILTERED_EXACT_BANNER.as_bytes().starts_with(normalized)
}

fn normalized_remote_line(bytes: &[u8]) -> &[u8] {
    let bytes = bytes.strip_suffix(b"\n").unwrap_or(bytes);
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

fn named_key_code(key_name: &str) -> Option<KeyCode> {
    match key_name {
        "enter" => Some(KeyCode::Enter),
        "tab" => Some(KeyCode::Tab),
        "escape" => Some(KeyCode::Escape),
        "backspace" => Some(KeyCode::Backspace),
        "delete" => Some(KeyCode::Delete),
        "up" => Some(KeyCode::UpArrow),
        "down" => Some(KeyCode::DownArrow),
        "left" => Some(KeyCode::LeftArrow),
        "right" => Some(KeyCode::RightArrow),
        "home" => Some(KeyCode::Home),
        "end" => Some(KeyCode::End),
        "page-up" => Some(KeyCode::PageUp),
        "page-down" => Some(KeyCode::PageDown),
        _ => None,
    }
}

fn key_modifiers(alt: bool, ctrl: bool, shift: bool) -> KeyModifiers {
    let mut modifiers = KeyModifiers::NONE;
    if alt {
        modifiers |= KeyModifiers::ALT;
    }
    if ctrl {
        modifiers |= KeyModifiers::CTRL;
    }
    if shift {
        modifiers |= KeyModifiers::SHIFT;
    }
    modifiers
}

fn resolve_cell_colors(palette: &ColorPalette, attrs: &wezterm_term::CellAttributes) -> (u32, u32) {
    let mut fg = resolve_palette_color(palette, attrs.foreground(), false);
    let mut bg = resolve_palette_color(palette, attrs.background(), true);
    if attrs.reverse() {
        std::mem::swap(&mut fg, &mut bg);
    }
    if attrs.invisible() {
        fg = bg;
    }
    (fg, bg)
}

fn resolve_palette_color(
    palette: &ColorPalette,
    color: ColorAttribute,
    background: bool,
) -> u32 {
    let rgba = if background {
        palette.resolve_bg(color)
    } else {
        palette.resolve_fg(color)
    };
    pack_color(rgba)
}

fn pack_color(color: SrgbaTuple) -> u32 {
    let channel = |value: f32| -> u32 { (value.clamp(0.0, 1.0) * 255.0).round() as u32 };
    let r = channel(color.0);
    let g = channel(color.1);
    let b = channel(color.2);
    let a = channel(color.3);
    (a << 24) | (r << 16) | (g << 8) | b
}

fn project_cursor_shape(shape: CursorShape) -> TerminalCursorShape {
    match shape {
        CursorShape::BlinkingUnderline | CursorShape::SteadyUnderline => {
            TerminalCursorShape::Underline
        }
        CursorShape::BlinkingBar | CursorShape::SteadyBar => TerminalCursorShape::Bar,
        CursorShape::Default | CursorShape::BlinkingBlock | CursorShape::SteadyBlock => {
            TerminalCursorShape::Block
        }
    }
}

fn cursor_shape_blinks(shape: CursorShape) -> bool {
    matches!(
        shape,
        CursorShape::Default
            | CursorShape::BlinkingBlock
            | CursorShape::BlinkingUnderline
            | CursorShape::BlinkingBar
    )
}

fn normalized_selection(
    start: (u32, u32),
    end: (u32, u32),
) -> ((u32, u32), (u32, u32)) {
    if start.0 < end.0 || (start.0 == end.0 && start.1 <= end.1) {
        (start, end)
    } else {
        (end, start)
    }
}

fn mouse_modifiers(event: TerminalMouseInput) -> wezterm_term::KeyModifiers {
    let mut modifiers = wezterm_term::KeyModifiers::NONE;
    if event.shift {
        modifiers |= wezterm_term::KeyModifiers::SHIFT;
    }
    if event.ctrl {
        modifiers |= wezterm_term::KeyModifiers::CTRL;
    }
    if event.alt {
        modifiers |= wezterm_term::KeyModifiers::ALT;
    }
    modifiers
}

fn encode_sgr_mouse_fallback(
    event: TerminalMouseInput,
    button: TerminalMouseButton,
) -> Vec<u8> {
    let mut code = match button {
        TerminalMouseButton::Left => 0,
        TerminalMouseButton::Middle => 1,
        TerminalMouseButton::Right => 2,
        TerminalMouseButton::WheelUp => 64,
        TerminalMouseButton::WheelDown => 65,
        TerminalMouseButton::None => 3,
    };
    if event.shift {
        code += 4;
    }
    if event.alt {
        code += 8;
    }
    if event.ctrl {
        code += 16;
    }
    if matches!(event.kind, TerminalMouseEventKind::Move) {
        code += 32;
    }

    format!(
        "\x1b[<{};{};{}{}",
        code,
        event.col + 1,
        event.row + 1,
        if matches!(event.kind, TerminalMouseEventKind::Up) {
            "m"
        } else {
            "M"
        }
    )
    .into_bytes()
}

#[derive(Clone, Debug, Default)]
struct SharedWriteBuffer {
    inner: Arc<Mutex<Vec<u8>>>,
}

impl SharedWriteBuffer {
    fn take(&self) -> Vec<u8> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        std::mem::take(&mut *buffer)
    }
}

impl Write for SharedWriteBuffer {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut buffer = self.inner.lock().expect("lock terminal write buffer");
        buffer.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssh_client_config_uses_keepalive_without_inactivity_disconnects() {
        let config = ssh_client_config();

        assert_eq!(config.inactivity_timeout, None);
        assert_eq!(config.keepalive_interval, Some(SSH_KEEPALIVE_INTERVAL));
        assert_eq!(config.keepalive_max, SSH_KEEPALIVE_MAX_MISSES);
        assert!(config.nodelay);
    }
}
