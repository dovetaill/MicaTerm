//! High-value output rule highlighting over the visible terminal window.

use std::collections::BTreeSet;

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};
use crate::app::terminal_semantic::{SemanticSpan, push_unique_span};
use crate::theme::SemanticStyleRole;

use super::OutputRuleProfile;

pub fn detect_output_rule_spans(
    frame: &TerminalModelFrame,
    profile: OutputRuleProfile,
) -> Vec<SemanticSpan> {
    let mut spans = Vec::new();
    let candidate_rows = expanded_dirty_rows(frame, 2);

    for row in &frame.rows {
        if !candidate_rows.contains(&row.row_index) {
            continue;
        }
        detect_token_spans(row, &mut spans);
    }

    spans.retain(|span| profile_allows_role(profile, span.role));
    spans
}

pub fn detect_search_match_spans(frame: &TerminalModelFrame, query: &str) -> Vec<SemanticSpan> {
    if query.trim().is_empty() {
        return Vec::new();
    }

    let mut spans = Vec::new();
    for row in &frame.rows {
        for (start_byte, end_byte) in find_case_insensitive_occurrences(&row.text, query) {
            push_unique_span(
                &mut spans,
                slice_span(
                    row,
                    start_byte,
                    end_byte,
                    SemanticStyleRole::OutputGrepMatch,
                ),
            );
        }
    }
    spans
}

pub fn count_search_query_matches_in_lines(lines: &[String], query: &str) -> usize {
    if query.trim().is_empty() {
        return 0;
    }

    lines
        .iter()
        .map(|line| find_case_insensitive_occurrences(line, query).len())
        .sum()
}

fn profile_allows_role(profile: OutputRuleProfile, role: SemanticStyleRole) -> bool {
    match profile {
        OutputRuleProfile::Default => true,
        OutputRuleProfile::Focused => matches!(
            role,
            SemanticStyleRole::OutputUrl
                | SemanticStyleRole::OutputUnixPath
                | SemanticStyleRole::OutputWindowsPath
                | SemanticStyleRole::OutputLineReference
                | SemanticStyleRole::OutputNetworkEndpoint
        ),
    }
}

fn expanded_dirty_rows(frame: &TerminalModelFrame, lookaround: usize) -> BTreeSet<u32> {
    if frame.dirty_rows.is_empty() {
        return frame.rows.iter().map(|row| row.row_index).collect();
    }

    let mut rows = BTreeSet::new();
    let max_row = frame.rows.last().map(|row| row.row_index).unwrap_or(0);
    for dirty in &frame.dirty_rows {
        let start = dirty.saturating_sub(lookaround as u32);
        let end = (*dirty + lookaround as u32).min(max_row);
        for row in start..=end {
            rows.insert(row);
        }
    }
    rows
}

fn detect_token_spans(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    for token in whitespace_tokens(&row.text) {
        let Some((start_byte, end_byte, cleaned)) = trim_token(&row.text, token.0, token.1) else {
            continue;
        };
        let role = if is_url(cleaned) {
            Some(SemanticStyleRole::OutputUrl)
        } else if is_line_reference(cleaned) {
            Some(SemanticStyleRole::OutputLineReference)
        } else if is_windows_path(cleaned) {
            Some(SemanticStyleRole::OutputWindowsPath)
        } else if is_unix_path(cleaned) {
            Some(SemanticStyleRole::OutputUnixPath)
        } else if is_network_endpoint(cleaned) {
            Some(SemanticStyleRole::OutputNetworkEndpoint)
        } else {
            None
        };

        if let Some(role) = role {
            push_unique_span(spans, slice_span(row, start_byte, end_byte, role));
        }
    }
}

fn whitespace_tokens(line: &str) -> Vec<(usize, usize)> {
    let mut tokens = Vec::new();
    let mut current_start = None;

    for (index, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = current_start.take() {
                tokens.push((start, index));
            }
        } else if current_start.is_none() {
            current_start = Some(index);
        }
    }

    if let Some(start) = current_start {
        tokens.push((start, line.len()));
    }

    tokens
}

fn trim_token<'a>(line: &'a str, start: usize, end: usize) -> Option<(usize, usize, &'a str)> {
    let mut trimmed_start = start;
    let mut trimmed_end = end;

    while trimmed_start < trimmed_end {
        let ch = line[trimmed_start..].chars().next()?;
        if matches!(ch, '(' | '[') {
            trimmed_start += ch.len_utf8();
        } else {
            break;
        }
    }
    while trimmed_end > trimmed_start {
        let ch = line[..trimmed_end].chars().next_back()?;
        if matches!(ch, ',' | ';' | ')' | ']') {
            trimmed_end -= ch.len_utf8();
        } else if ch == ':' && !line[trimmed_start..trimmed_end].contains("::") {
            trimmed_end -= ch.len_utf8();
        } else {
            break;
        }
    }

    if trimmed_start >= trimmed_end {
        None
    } else {
        Some((
            trimmed_start,
            trimmed_end,
            &line[trimmed_start..trimmed_end],
        ))
    }
}

fn slice_span(
    row: &TerminalModelRow,
    start_byte: usize,
    end_byte: usize,
    role: SemanticStyleRole,
) -> SemanticSpan {
    SemanticSpan {
        row: row.row_index,
        start_col: char_col(&row.text, start_byte) as u32,
        end_col: char_col(&row.text, end_byte).saturating_sub(1) as u32,
        role,
        text: row.text[start_byte..end_byte].to_string(),
    }
}

fn find_ascii_occurrences(line: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let mut search_from = 0usize;
    while let Some(found) = line[search_from..].find(needle) {
        let start = search_from + found;
        let end = start + needle.len();
        ranges.push((start, end));
        search_from = end;
    }
    ranges
}

fn find_case_insensitive_occurrences(line: &str, needle: &str) -> Vec<(usize, usize)> {
    let lowered_line = line.to_ascii_lowercase();
    let lowered_needle = needle.to_ascii_lowercase();
    find_ascii_occurrences(&lowered_line, &lowered_needle)
}

fn char_col(text: &str, byte_index: usize) -> usize {
    text[..byte_index].chars().count()
}

fn is_url(token: &str) -> bool {
    token.starts_with("http://") || token.starts_with("https://")
}

fn is_unix_path(token: &str) -> bool {
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/")
}

fn is_windows_path(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() > 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && (bytes[2] == b'\\' || bytes[2] == b'/')
}

fn is_line_reference(token: &str) -> bool {
    let Some((prefix, tail)) = token.rsplit_once(':') else {
        return false;
    };
    if !tail.chars().all(|ch| ch.is_ascii_digit()) {
        let Some((prefix, column)) = prefix.rsplit_once(':') else {
            return false;
        };
        column.chars().all(|ch| ch.is_ascii_digit()) && line_reference_prefix(prefix)
    } else {
        line_reference_prefix(prefix)
    }
}

fn line_reference_prefix(prefix: &str) -> bool {
    if prefix.contains("://") {
        return false;
    }

    prefix.contains('/')
        || prefix.contains('\\')
        || prefix.ends_with(".rs")
        || prefix.ends_with(".json")
}

fn is_network_endpoint(token: &str) -> bool {
    let Some((host, port)) = token.rsplit_once(':') else {
        return false;
    };
    port.chars().all(|ch| ch.is_ascii_digit())
        && host.split('.').count() == 4
        && host
            .split('.')
            .all(|segment| !segment.is_empty() && segment.chars().all(|ch| ch.is_ascii_digit()))
}
