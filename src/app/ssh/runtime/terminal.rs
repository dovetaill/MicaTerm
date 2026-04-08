//! SSH runtime terminal engine and surface projection helpers.

use std::sync::{Arc, Mutex};

use anyhow::{Result, bail};
use russh::Channel;
use russh::ChannelMsg;
use russh::client;
use termwiz::input::{KeyCode, Modifiers as KeyModifiers};
use uuid::Uuid;

use crate::app::terminal_core::{
    TerminalCoreAdapter, TerminalCoreKind, TerminalFrameSnapshot, create_terminal_core_adapter,
};
use crate::theme::ThemeMode;

use super::{TerminalKeyEvent, TerminalMouseInput, TerminalRowState, TerminalSurfaceState};

pub(super) fn apply_remote_output(terminal: &Arc<Mutex<TerminalSession>>, bytes: &[u8]) {
    if let Ok(mut terminal) = terminal.lock() {
        terminal.apply_remote_bytes(bytes);
    }
}

pub(super) async fn await_channel_success(
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

pub fn negotiated_terminal_environment() -> [(&'static str, &'static str); 1] {
    [("COLORTERM", "truecolor")]
}

pub(super) async fn negotiate_terminal_environment(
    channel: &mut Channel<client::Msg>,
    pending_output: &mut Vec<u8>,
) {
    for (variable_name, variable_value) in negotiated_terminal_environment() {
        if let Err(err) = channel.set_env(true, variable_name, variable_value).await {
            tracing::warn!(
                variable_name,
                variable_value,
                error = %err,
                "failed to send negotiated terminal environment request",
            );
            continue;
        }

        let request_label = format!("env {variable_name}");
        if let Err(err) = await_channel_success(channel, &request_label, pending_output).await {
            tracing::warn!(
                variable_name,
                variable_value,
                error = %err,
                "SSH server rejected negotiated terminal environment request",
            );
        }
    }
}

pub fn extract_current_working_directory_from_osc7(bytes: &[u8]) -> Option<String> {
    const PREFIX: &[u8] = b"\x1b]7;file://";

    let start = bytes
        .windows(PREFIX.len())
        .position(|window| window == PREFIX)?;
    let payload_start = start + PREFIX.len();
    let payload_end = bytes[payload_start..]
        .iter()
        .position(|byte| *byte == 0x07)
        .map(|offset| payload_start + offset)
        .or_else(|| {
            bytes[payload_start..]
                .windows(2)
                .position(|window| window == b"\x1b\\")
                .map(|offset| payload_start + offset)
        })?;

    let payload = std::str::from_utf8(&bytes[payload_start..payload_end]).ok()?;
    let path_start = payload.find('/')?;
    let decoded = percent_decode_path(&payload[path_start..])?;

    if decoded.starts_with('/') {
        Some(decoded)
    } else {
        None
    }
}

fn percent_decode_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = *bytes.get(index + 1)?;
            let low = *bytes.get(index + 2)?;
            let value = (hex_value(high)? << 4) | hex_value(low)?;
            decoded.push(value);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(super) fn snapshot_terminal_surface(
    terminal: &Arc<Mutex<TerminalSession>>,
    session_id: Uuid,
) -> Option<TerminalSurfaceState> {
    terminal
        .lock()
        .ok()
        .map(|terminal| terminal.surface_state(session_id))
}

pub struct TerminalSession {
    core: Box<dyn TerminalCoreAdapter>,
}

impl TerminalSession {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self::new_with_core_kind(rows, cols, TerminalCoreKind::Wezterm)
    }

    pub fn new_with_core_kind(rows: usize, cols: usize, kind: TerminalCoreKind) -> Self {
        Self::with_core(create_terminal_core_adapter(kind, rows, cols))
    }

    pub fn new_with_experimental_alacritty_core(rows: usize, cols: usize) -> Self {
        Self::new_with_core_kind(rows, cols, TerminalCoreKind::AlacrittyExperimental)
    }

    pub fn with_core(core: Box<dyn TerminalCoreAdapter>) -> Self {
        Self { core }
    }

    pub fn sequence_number(&self) -> usize {
        self.core.sequence_number()
    }

    pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        self.core.apply_remote_bytes(bytes);
    }

    pub fn screen_text(&self) -> String {
        self.core.screen_text()
    }

    pub fn visible_rows(&self) -> Vec<TerminalRowState> {
        self.core.visible_rows()
    }

    pub fn visible_lines(&self) -> Vec<String> {
        self.core.visible_lines()
    }

    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.core.resize(rows, cols);
    }

    pub fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
        TerminalSurfaceState::from_frame_snapshot(session_id, self.core.frame_snapshot())
    }

    pub fn frame_snapshot(&self) -> TerminalFrameSnapshot {
        self.core.frame_snapshot()
    }

    pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        self.core.send_key_event(event)
    }

    pub fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.core.send_key_down(key, modifiers)
    }

    pub fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.core.encode_paste(text)
    }

    pub fn scroll_viewport_lines(&mut self, delta: i32) {
        self.core.scroll_viewport_lines(delta);
    }

    pub fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.core.set_theme_mode(mode);
    }

    pub fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        self.core.send_mouse_input(event)
    }
}

pub fn encode_named_key_input(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Result<Option<Vec<u8>>> {
    crate::app::terminal_core::wezterm_adapter::encode_named_key_input(key_name, alt, ctrl, shift)
}

#[cfg(test)]
pub(super) fn visible_lines_from_rows(rows: &[TerminalRowState]) -> Vec<String> {
    let mut lines = rows.iter().map(|row| row.text.clone()).collect::<Vec<_>>();
    while lines.first().is_some_and(String::is_empty) {
        let _ = lines.remove(0);
    }
    while lines.last().is_some_and(String::is_empty) {
        let _ = lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

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
