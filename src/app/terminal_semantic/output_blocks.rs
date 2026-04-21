//! Semantic output block detection for normal shell text flows.

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};

use super::types::{SemanticPriority, SemanticSpan, SemanticStyleRole};

const LOG_LEVEL_PREFIXES: &[(&str, SemanticStyleRole)] = &[
    ("[TRACE]", SemanticStyleRole::OutputLevelDebug),
    ("[DEBUG]", SemanticStyleRole::OutputLevelDebug),
    ("[INFO]", SemanticStyleRole::OutputLevelInfo),
    ("[WARN]", SemanticStyleRole::OutputLevelWarn),
    ("[ERROR]", SemanticStyleRole::OutputLevelError),
    ("TRACE ", SemanticStyleRole::OutputLevelDebug),
    ("DEBUG ", SemanticStyleRole::OutputLevelDebug),
    ("INFO ", SemanticStyleRole::OutputLevelInfo),
    ("WARN ", SemanticStyleRole::OutputLevelWarn),
    ("ERROR ", SemanticStyleRole::OutputLevelError),
];

pub fn detect_output_block_spans(frame: &TerminalModelFrame) -> Vec<SemanticSpan> {
    let mut spans = Vec::new();
    let mut row_index = 0;

    while row_index < frame.rows.len() {
        if let Some((block_spans, next_row_index)) = detect_json_block(&frame.rows, row_index)
            .or_else(|| detect_xml_block(&frame.rows, row_index))
            .or_else(|| detect_log_block(&frame.rows, row_index))
        {
            spans.extend(block_spans);
            row_index = next_row_index;
        } else {
            row_index += 1;
        }
    }

    spans
}

fn detect_json_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(Vec<SemanticSpan>, usize)> {
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
                build_block_spans(rows, start_index, offset, SemanticStyleRole::OutputJson),
                offset + 1,
            ));
        }
    }

    None
}

fn detect_xml_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(Vec<SemanticSpan>, usize)> {
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
                build_block_spans(rows, start_index, offset, SemanticStyleRole::OutputXml),
                offset + 1,
            ));
        }
    }

    None
}

fn detect_log_block(
    rows: &[TerminalModelRow],
    start_index: usize,
) -> Option<(Vec<SemanticSpan>, usize)> {
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

    let spans = rows[start_index..end_index]
        .iter()
        .filter_map(|row| {
            let role = log_line_role(row.text.as_str())?;
            Some(SemanticSpan::new(
                row.row_index,
                0,
                row.text.chars().count().saturating_sub(1) as u32,
                role,
                SemanticPriority::High,
            ))
        })
        .collect();

    Some((spans, end_index))
}

fn build_block_spans(
    rows: &[TerminalModelRow],
    start_index: usize,
    end_index: usize,
    role: SemanticStyleRole,
) -> Vec<SemanticSpan> {
    rows[start_index..=end_index]
        .iter()
        .map(|row| {
            SemanticSpan::new(
                row.row_index,
                0,
                row.text.chars().count().saturating_sub(1) as u32,
                role,
                SemanticPriority::Normal,
            )
        })
        .collect()
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
    log_line_role(line).is_some()
}

fn log_line_role(line: &str) -> Option<SemanticStyleRole> {
    let trimmed = line.trim_start();
    LOG_LEVEL_PREFIXES
        .iter()
        .find_map(|(prefix, role)| trimmed.starts_with(prefix).then_some(*role))
}
