//! SSH session runtime primitives and terminal-state wrapper.

use std::io::{self, Write};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use termwiz::input::{KeyCode, KeyCodeEncodeModes, KeyboardEncoding, Modifiers};
use tokio::sync::mpsc;
use uuid::Uuid;
use wezterm_term::color::ColorPalette;
use wezterm_term::{Terminal, TerminalConfiguration, TerminalSize};

use crate::app::ssh::profile::ConnectionProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionRuntimeEvent {
    Connected,
    Output(Vec<u8>),
    Disconnected,
    Error(String),
}

pub struct SshSessionRuntime {
    session_id: Uuid,
    #[allow(dead_code)]
    profile: ConnectionProfile,
    terminal: TerminalSession,
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Result<Self> {
        let _ = event_tx.send(SessionRuntimeEvent::Connected);

        Ok(Self {
            session_id,
            profile,
            terminal: TerminalSession::new(24, 80),
        })
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn terminal(&self) -> &TerminalSession {
        &self.terminal
    }

    pub fn terminal_mut(&mut self) -> &mut TerminalSession {
        &mut self.terminal
    }
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
