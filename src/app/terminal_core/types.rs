use std::sync::Arc;

use anyhow::{Result, bail};
use termwiz::input::{KeyCode, Modifiers as KeyModifiers};

use crate::app::ssh::runtime::{
    TerminalCellState, TerminalCursorState, TerminalKeyEvent, TerminalMouseInput, TerminalRowState,
};
use crate::theme::{ThemeMode, ThemeVariant};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalViewportMetrics {
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub dpi: u32,
}

impl TerminalViewportMetrics {
    pub fn new(pixel_width: u32, pixel_height: u32, dpi: u32) -> Self {
        Self {
            pixel_width: pixel_width.max(1),
            pixel_height: pixel_height.max(1),
            dpi: dpi.max(1),
        }
    }

    pub fn fallback(rows: usize, cols: usize) -> Self {
        Self::new(
            cols.max(1).saturating_mul(8) as u32,
            rows.max(1).saturating_mul(16) as u32,
            96,
        )
    }
}

impl Default for TerminalViewportMetrics {
    fn default() -> Self {
        Self::fallback(24, 80)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTerminalImage {
    pub png_bytes: Vec<u8>,
    pub source_width: u32,
    pub source_height: u32,
    pub columns: u32,
    pub rows: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalImageResource {
    pub content_hash: [u8; 32],
    pub width: u32,
    pub height: u32,
    pub rgba: Arc<[u8]>,
}

impl TerminalImageResource {
    pub fn decoded_bytes(&self) -> usize {
        self.rgba.len()
    }
}

pub const TERMINAL_IMAGE_UV_SCALE: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TerminalImageUvRect {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

impl TerminalImageUvRect {
    pub fn from_unit_f32(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        let scale =
            |value: f32| (value.clamp(0.0, 1.0) * TERMINAL_IMAGE_UV_SCALE as f32).round() as u32;
        Self {
            left: scale(left),
            top: scale(top),
            right: scale(right),
            bottom: scale(bottom),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalImagePlacement {
    pub resource_key: [u8; 32],
    pub row: u32,
    pub col: u32,
    pub row_span: u32,
    pub col_span: u32,
    pub uv: TerminalImageUvRect,
    pub padding_left_px: u16,
    pub padding_top_px: u16,
    pub padding_right_px: u16,
    pub padding_bottom_px: u16,
    pub z_index: i32,
    pub image_id: Option<u32>,
    pub placement_id: Option<u32>,
    pub protocol_order: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalFrameSnapshot {
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub viewport_metrics: TerminalViewportMetrics,
    pub default_fg_rgba: u32,
    pub default_bg_rgba: u32,
    pub row_bg_even_rgba: u32,
    pub row_bg_odd_rgba: u32,
    pub viewport: ViewportState,
    pub visible_rows: Vec<TerminalRowState>,
    pub visible_lines: Vec<String>,
    pub cells: Vec<TerminalCellState>,
    pub image_resources: Vec<Arc<TerminalImageResource>>,
    pub image_placements: Vec<TerminalImagePlacement>,
    pub cursor: TerminalCursorState,
    pub selection: SelectionState,
    pub alternate_screen_active: bool,
    pub mouse_grabbed: bool,
    pub application_cursor_keys: bool,
    pub bracketed_paste_enabled: bool,
}

pub trait TerminalCoreAdapter: Send {
    fn sequence_number(&self) -> usize;
    fn apply_remote_bytes(&mut self, bytes: &[u8]) -> Vec<u8>;
    fn apply_local_image(&mut self, _image: LocalTerminalImage) -> Result<()> {
        bail!("terminal core does not support local images")
    }
    fn screen_text(&self) -> String;
    fn visible_rows(&self) -> Vec<TerminalRowState>;
    fn visible_lines(&self) -> Vec<String>;
    fn selection_text_from_buffer_rows(
        &self,
        _start_row: u32,
        _start_col: u32,
        _end_row: u32,
        _end_col: u32,
    ) -> String {
        String::new()
    }
    fn frame_snapshot(&self) -> TerminalFrameSnapshot;
    fn resize(&mut self, rows: usize, cols: usize);
    fn resize_with_viewport(
        &mut self,
        rows: usize,
        cols: usize,
        viewport: TerminalViewportMetrics,
    ) {
        let _ = viewport;
        self.resize(rows, cols);
    }
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
