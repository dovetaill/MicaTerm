use anyhow::Result;
use termwiz::input::{KeyCode, Modifiers as KeyModifiers};

use crate::app::ssh::runtime::{
    TerminalCellState, TerminalCursorState, TerminalKeyEvent, TerminalMouseInput, TerminalRowState,
};
use crate::theme::{ThemeMode, ThemeVariant};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TerminalCoreKind {
    #[default]
    Wezterm,
    AlacrittyExperimental,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ViewportState {
    pub offset_lines: u32,
    pub max_offset_lines: u32,
    pub at_bottom: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelectionState {
    pub active: bool,
    pub start_row: u32,
    pub start_col: u32,
    pub end_row: u32,
    pub end_col: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFrameSnapshot {
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport: ViewportState,
    pub visible_rows: Vec<TerminalRowState>,
    pub visible_lines: Vec<String>,
    pub cells: Vec<TerminalCellState>,
    pub cursor: TerminalCursorState,
    pub selection: SelectionState,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub application_cursor_keys: bool,
    pub bracketed_paste_enabled: bool,
}

pub trait TerminalCoreAdapter: Send {
    fn sequence_number(&self) -> usize;
    fn apply_remote_bytes(&mut self, bytes: &[u8]);
    fn screen_text(&self) -> String;
    fn visible_rows(&self) -> Vec<TerminalRowState>;
    fn visible_lines(&self) -> Vec<String>;
    fn frame_snapshot(&self) -> TerminalFrameSnapshot;
    fn resize(&mut self, rows: usize, cols: usize);
    fn set_theme(&mut self, mode: ThemeMode, variant: ThemeVariant);
    fn set_theme_mode(&mut self, mode: ThemeMode) {
        self.set_theme(mode, ThemeVariant::PremiumDefault);
    }
    fn scroll_viewport_lines(&mut self, delta: i32);
    fn send_key_down(&mut self, key: KeyCode, modifiers: KeyModifiers) -> Result<Vec<u8>>;
    fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>>;
    fn send_mouse_input(&mut self, event: TerminalMouseInput) -> Result<Vec<u8>>;
    fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>>;
}
