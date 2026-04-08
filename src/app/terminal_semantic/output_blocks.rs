//! Semantic output block detection for normal shell text flows.

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};

const JSON_OVERLAY_RGBA: u32 = 0x3328_7dff;
const XML_OVERLAY_RGBA: u32 = 0x3348_bf91;
const LOG_OVERLAY_RGBA: u32 = 0x33f5_a524;
const LOG_LEVEL_PREFIXES: &[&str] = &[
    "[TRACE]", "[DEBUG]", "[INFO]", "[WARN]", "[ERROR]", "TRACE ", "DEBUG ", "INFO ", "WARN ",
    "ERROR ",
];

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
    let mut overlays = Vec::new();
    let mut row_index = 0;

    while row_index < frame.rows.len() {
        if let Some((overlay, next_row_index)) = detect_json_block(&frame.rows, row_index)
            .or_else(|| detect_xml_block(&frame.rows, row_index))
            .or_else(|| detect_log_block(&frame.rows, row_index))
        {
            overlays.push(overlay);
            row_index = next_row_index;
        } else {
            row_index += 1;
        }
    }

    overlays
}

fn detect_json_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(SemanticOutputOverlay, usize)> {
    let first_line = rows.get(start_index)?.text.trim_start();
    if !(first_line.starts_with('{') || first_line.starts_with('[')) {
        return None;
    }
    if first_line.starts_with('[') && is_log_line(first_line) {
        return None;
    }

    let mut curly_depth = 0i32;
    let mut square_depth = 0i32;

    for (offset, row) in rows.iter().enumerate().skip(start_index) {
        update_json_depths(row.text.as_str(), &mut curly_depth, &mut square_depth);
        if curly_depth < 0 || square_depth < 0 {
            return None;
        }
        if curly_depth == 0 && square_depth == 0 {
            return Some((
                build_overlay(rows, start_index, offset, SemanticOutputBlockKind::Json),
                offset + 1,
            ));
        }
    }

    None
}

fn detect_xml_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(SemanticOutputOverlay, usize)> {
    let first_line = rows.get(start_index)?.text.trim_start();
    if !first_line.starts_with('<') || first_line.starts_with("<?") || first_line.starts_with("<!")
    {
        return None;
    }

    let mut balance = 0i32;

    for (offset, row) in rows.iter().enumerate().skip(start_index) {
        let trimmed = row.text.trim();
        if trimmed.is_empty() {
            break;
        }
        let (open_tags, close_tags) = count_xml_tags(trimmed);
        if open_tags == 0 && close_tags == 0 {
            break;
        }
        balance += open_tags as i32 - close_tags as i32;
        if balance <= 0 {
            return Some((
                build_overlay(rows, start_index, offset, SemanticOutputBlockKind::Xml),
                offset + 1,
            ));
        }
    }

    None
}

fn detect_log_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(SemanticOutputOverlay, usize)> {
    if !is_log_line(rows.get(start_index)?.text.as_str()) {
        return None;
    }

    let mut end_index = start_index;
    while let Some(row) = rows.get(end_index) {
        if !is_log_line(row.text.as_str()) {
            break;
        }
        end_index += 1;
    }

    if end_index.saturating_sub(start_index) < 2 {
        return None;
    }

    Some((
        build_overlay(
            rows,
            start_index,
            end_index - 1,
            SemanticOutputBlockKind::Log,
        ),
        end_index,
    ))
}

fn build_overlay(
    rows: &[TerminalModelRow],
    start_index: usize,
    end_index: usize,
    kind: SemanticOutputBlockKind,
) -> SemanticOutputOverlay {
    SemanticOutputOverlay {
        kind,
        start_row: rows[start_index].row_index,
        end_row: rows[end_index].row_index,
        row_ranges: rows[start_index..=end_index]
            .iter()
            .map(|row| SemanticOverlayRowRange {
                row: row.row_index,
                start_col: 0,
                end_col: row.text.chars().count().saturating_sub(1) as u32,
                overlay_rgba: overlay_rgba(kind),
            })
            .collect(),
    }
}

fn overlay_rgba(kind: SemanticOutputBlockKind) -> u32 {
    match kind {
        SemanticOutputBlockKind::Json => JSON_OVERLAY_RGBA,
        SemanticOutputBlockKind::Xml => XML_OVERLAY_RGBA,
        SemanticOutputBlockKind::Log => LOG_OVERLAY_RGBA,
    }
}

fn update_json_depths(line: &str, curly_depth: &mut i32, square_depth: &mut i32) {
    let mut in_string = false;
    let mut escaped = false;

    for ch in line.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' if in_string => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => *curly_depth += 1,
            '}' if !in_string => *curly_depth -= 1,
            '[' if !in_string => *square_depth += 1,
            ']' if !in_string => *square_depth -= 1,
            _ => {}
        }
    }
}

fn count_xml_tags(line: &str) -> (usize, usize) {
    let mut open_tags = 0usize;
    let mut close_tags = 0usize;
    let mut cursor = line;

    while let Some(start) = cursor.find('<') {
        let tail = &cursor[start + 1..];
        if tail.starts_with('/') {
            close_tags += 1;
        } else if tail.starts_with('!') || tail.starts_with('?') {
        } else {
            open_tags += 1;
            if let Some(end) = tail.find('>') {
                let tag_body = &tail[..end];
                if tag_body.trim_end().ends_with('/') {
                    close_tags += 1;
                }
            }
        }

        cursor = tail;
    }

    (open_tags, close_tags)
}

fn is_log_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    LOG_LEVEL_PREFIXES
        .iter()
        .any(|prefix| trimmed.starts_with(prefix))
}
