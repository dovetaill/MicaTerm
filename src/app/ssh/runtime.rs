//! SSH session runtime primitives and terminal-state wrapper.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use russh::Channel;
use russh::ChannelMsg;
use russh::Disconnect;
use russh::client;
use russh::client::AuthResult;
use russh::keys::{self, PrivateKeyWithHashAlg};
use serde::Deserialize;
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers};
use tokio::sync::mpsc;
use uuid::Uuid;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

use crate::app::ssh::credentials::{CredentialStore, SystemCredentialStore};
use crate::app::ssh::known_hosts::{KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{ConnectionProfile, SshAuthMethod};

const DEFAULT_TERMINAL_ROWS: usize = 24;
const DEFAULT_TERMINAL_COLS: usize = 80;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub screen_text: String,
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

#[derive(Debug)]
enum RuntimeCommand {
    Input(Vec<u8>),
    Resize { rows: u32, cols: u32 },
    Disconnect,
}

#[derive(Debug, Default, Deserialize)]
struct StoredSshSecretBundle {
    password: Option<String>,
    private_key_content: Option<String>,
    passphrase: Option<String>,
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
        self.known_hosts
            .ensure_trusted(&self.host, self.port, server_public_key)?;
        Ok(true)
    }
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
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
        let config = Arc::new(client::Config {
            inactivity_timeout: Some(Duration::from_secs(30)),
            nodelay: true,
            ..Default::default()
        });

        let mut handle = client::connect(config, (profile.host.as_str(), profile.port), handler)
            .await
            .with_context(|| {
                format!(
                    "failed to connect to SSH server `{}:{}`",
                    profile.host, profile.port
                )
            })?;

        authenticate_client(&mut handle, &profile).await?;

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
            if let Some(surface) = snapshot_terminal_surface(&terminal, session_id) {
                let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            }
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

    pub fn send_input(&self, bytes: Vec<u8>) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Input(bytes))
            .map_err(|_| anyhow!("ssh runtime input channel is closed"))
    }

    pub fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Resize { rows, cols })
            .map_err(|_| anyhow!("ssh runtime resize channel is closed"))
    }

    pub fn disconnect(&self) -> Result<()> {
        self.command_tx
            .send(RuntimeCommand::Disconnect)
            .map_err(|_| anyhow!("ssh runtime disconnect channel is closed"))
    }
}

async fn authenticate_client(
    handle: &mut client::Handle<RuntimeClientHandler>,
    profile: &ConnectionProfile,
) -> Result<()> {
    let stored_bundle = load_stored_secret_bundle(profile)?;

    match profile.auth_method {
        SshAuthMethod::Password => {
            let password = profile
                .password
                .clone()
                .or(stored_bundle.password)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("missing SSH password secret for `{}`", profile.name))?;
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
            let passphrase = profile.passphrase.clone().or(stored_bundle.passphrase);
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
            let private_key_content = profile
                .private_key_content
                .clone()
                .or(stored_bundle.private_key_content)
                .filter(|value| !value.trim().is_empty())
                .ok_or_else(|| anyhow!("missing inline SSH private key secret for `{}`", profile.name))?;
            let passphrase = profile.passphrase.clone().or(stored_bundle.passphrase);
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

fn load_stored_secret_bundle(profile: &ConnectionProfile) -> Result<StoredSshSecretBundle> {
    let Some(credential_ref) = profile.credential_ref.as_deref() else {
        return Ok(StoredSshSecretBundle::default());
    };

    let store = SystemCredentialStore;
    let raw = store
        .get_secret(credential_ref)
        .with_context(|| format!("failed to load SSH secret bundle `{credential_ref}`"))?;
    let Some(raw) = raw else {
        return Ok(StoredSshSecretBundle::default());
    };

    Ok(match serde_json::from_str::<StoredSshSecretBundle>(&raw) {
        Ok(bundle) => bundle,
        Err(_) => match profile.auth_method {
            SshAuthMethod::Password => StoredSshSecretBundle {
                password: Some(raw),
                ..StoredSshSecretBundle::default()
            },
            SshAuthMethod::PrivateKeyContent => StoredSshSecretBundle {
                private_key_content: Some(raw),
                ..StoredSshSecretBundle::default()
            },
            SshAuthMethod::PrivateKeyPath => StoredSshSecretBundle::default(),
        },
    })
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
                    Some(RuntimeCommand::Input(bytes)) => {
                        if let Err(bytes) = handle.data(channel.id(), bytes).await {
                            let _ = event_tx.send(SessionRuntimeEvent::Error(format!(
                                "failed to write {} bytes to SSH channel",
                                bytes.len()
                            )));
                            break;
                        }
                    }
                    Some(RuntimeCommand::Resize { rows, cols }) => {
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
    writer: SharedWriteBuffer,
}

impl TerminalSession {
    pub fn new(rows: usize, cols: usize) -> Self {
        let writer = SharedWriteBuffer::default();
        let terminal = Terminal::new(
            TerminalSize {
                rows,
                cols,
                pixel_width: cols * 8,
                pixel_height: rows * 16,
                dpi: 96,
            },
            Arc::new(SessionTerminalConfig),
            "MicaTerm",
            env!("CARGO_PKG_VERSION"),
            Box::new(writer.clone()),
        );

        Self { terminal, writer }
    }

    pub fn sequence_number(&self) -> usize {
        self.terminal.current_seqno()
    }

    pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        self.terminal.advance_bytes(bytes);
    }

    pub fn screen_text(&self) -> String {
        let mut lines = Vec::new();
        self.terminal.screen().for_each_phys_line(|_, line| {
            lines.push(line.as_str().trim_end().to_string());
        });
        lines.join("\n")
    }

    pub fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
        TerminalSurfaceState {
            session_id,
            seqno: self.sequence_number(),
            screen_text: self.screen_text(),
        }
    }

    pub fn send_key_down(&mut self, key: KeyCode, modifiers: Modifiers) -> Result<Vec<u8>> {
        let encoded = key.encode(
            modifiers,
            KeyCodeEncodeModes {
                encoding: KeyboardEncoding::Xterm,
                newline_mode: false,
                application_cursor_keys: false,
                modify_other_keys: None,
            },
            true,
        )?;
        let bytes = encoded.into_bytes();

        let mut writer = self.writer.clone();
        writer.write_all(&bytes)?;
        writer.flush()?;

        Ok(self.writer.take())
    }
}

#[derive(Debug, Default)]
struct SessionTerminalConfig;

impl TerminalConfiguration for SessionTerminalConfig {
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
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
