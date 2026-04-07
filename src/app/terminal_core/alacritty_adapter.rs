//! Experimental alacritty-style adapter seam.
//!
//! This keeps the control behavior aligned with the existing wezterm adapter while the
//! explicit runtime-selection and parity harness come online. The real core swap can
//! replace the wrapped implementation later without changing the session contract.

use anyhow::Result;
use termwiz::input::{KeyCode, Modifiers as KeyModifiers};

use crate::app::ssh::runtime::{
    TerminalKeyEvent, TerminalMouseInput, TerminalRowState,
};
use crate::theme::ThemeMode;

use super::{
    TerminalCoreAdapter, TerminalFrameSnapshot, WeztermTerminalCoreAdapter,
};

pub struct AlacrittyTerminalCoreAdapter {
    inner: WeztermTerminalCoreAdapter,
}

impl AlacrittyTerminalCoreAdapter {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            inner: WeztermTerminalCoreAdapter::new(rows, cols),
        }
    }
}

impl TerminalCoreAdapter for AlacrittyTerminalCoreAdapter {
    fn sequence_number(&self) -> usize {
        self.inner.sequence_number()
    }

    fn apply_remote_bytes(&mut self, bytes: &[u8]) {
        self.inner.apply_remote_bytes(bytes);
    }

    fn screen_text(&self) -> String {
        self.inner.screen_text()
    }

    fn visible_rows(&self) -> Vec<TerminalRowState> {
        self.inner.visible_rows()
    }

    fn visible_lines(&self) -> Vec<String> {
        self.inner.visible_lines()
    }

    fn frame_snapshot(&self) -> TerminalFrameSnapshot {
        self.inner.frame_snapshot()
    }

    fn resize(&mut self, rows: usize, cols: usize) {
        self.inner.resize(rows, cols);
    }

    fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.inner.set_theme_mode(mode);
    }

    fn scroll_viewport_lines(&mut self, delta: i32) {
        self.inner.scroll_viewport_lines(delta);
    }

    fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>> {
        self.inner.send_key_down(key, modifiers)
    }

    fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
        self.inner.send_key_event(event)
    }

    fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>> {
        self.inner.send_mouse_input(event)
    }

    fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>> {
        self.inner.encode_paste(text)
    }
}
