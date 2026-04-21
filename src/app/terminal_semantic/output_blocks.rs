//! Compatibility wrapper for full-frame semantic output analysis.

use crate::app::terminal_model::TerminalModelFrame;

use super::rules::analyze_output_rules;
use super::types::SemanticSpan;

pub fn detect_output_block_spans(frame: &TerminalModelFrame) -> Vec<SemanticSpan> {
    let all_rows = frame
        .rows
        .iter()
        .map(|row| row.row_index)
        .collect::<Vec<_>>();
    analyze_output_rules(frame, &all_rows).spans
}
