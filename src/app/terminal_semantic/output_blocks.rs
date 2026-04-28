//! Conservative output block overlays.
//!
//! The current repair intentionally keeps this layer disabled. Broad JSON/XML/log
//! block tinting proved too error-prone for ordinary terminal prose and TUI text.

use crate::app::terminal_model::TerminalModelFrame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticOutputBlockKind {
    Json,
    Xml,
    Log,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticOverlayRowRange {
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SemanticOutputOverlay {
    pub kind: SemanticOutputBlockKind,
    pub start_row: u32,
    pub end_row: u32,
    pub row_ranges: Vec<SemanticOverlayRowRange>,
}

pub fn detect_output_block_overlays(frame: &TerminalModelFrame) -> Vec<SemanticOutputOverlay> {
    let _ = frame;
    Vec::new()
}
