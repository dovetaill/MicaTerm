//! High-value output rule highlighting over the visible terminal window.

use std::collections::BTreeSet;

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};
use crate::app::terminal_semantic::{SemanticSpan, push_unique_span};
use crate::theme::SemanticStyleRole;

use super::{OutputRuleProfile, SemanticOutputBlockKind, detect_output_block_overlays};

const FAILURE_PHRASES: &[(&str, SemanticStyleRole)] = &[
    ("Permission denied", SemanticStyleRole::OutputFailureKeyword),
    (
        "Host key verification failed",
        SemanticStyleRole::OutputFailureKeyword,
    ),
    (
        "No such file or directory",
        SemanticStyleRole::OutputFailureKeyword,
    ),
    (
        "connection refused",
        SemanticStyleRole::OutputFailureKeyword,
    ),
    ("timed out", SemanticStyleRole::OutputFailureKeyword),
    ("failed", SemanticStyleRole::OutputFailureKeyword),
    ("failure", SemanticStyleRole::OutputFailureKeyword),
    ("fatal", SemanticStyleRole::OutputFailureKeyword),
    ("ErrImagePull", SemanticStyleRole::OutputFailureKeyword),
    ("CrashLoopBackOff", SemanticStyleRole::OutputFailureKeyword),
];
const SUCCESS_PHRASES: &[&str] = &["success", "succeeded", "completed", "done", "ok"];
const SEVERITY_KEYWORDS: &[(&str, SemanticStyleRole)] = &[
    ("ERROR", SemanticStyleRole::OutputSeverityError),
    ("WARN", SemanticStyleRole::OutputSeverityWarning),
    ("INFO", SemanticStyleRole::OutputSeverityInfo),
    ("DEBUG", SemanticStyleRole::OutputSeverityDebug),
];

pub fn detect_output_rule_spans(
    frame: &TerminalModelFrame,
    profile: OutputRuleProfile,
) -> Vec<SemanticSpan> {
    let mut spans = Vec::new();
    let candidate_rows = expanded_dirty_rows(frame, 2);
    let json_rows = detect_output_block_overlays(frame)
        .into_iter()
        .filter(|overlay| overlay.kind == SemanticOutputBlockKind::Json)
        .flat_map(|overlay| overlay.row_ranges.into_iter().map(|range| range.row))
        .collect::<BTreeSet<_>>();

    for row in &frame.rows {
        if !candidate_rows.contains(&row.row_index) {
            continue;
        }
        detect_diff_spans(row, &mut spans);
        detect_phrase_spans(row, &mut spans);
        detect_token_spans(row, &mut spans);
        if json_rows.contains(&row.row_index) {
            detect_json_spans(row, &mut spans);
        }
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
                | SemanticStyleRole::OutputSeverityError
                | SemanticStyleRole::OutputSeverityWarning
                | SemanticStyleRole::OutputFailureKeyword
                | SemanticStyleRole::OutputDiffAdded
                | SemanticStyleRole::OutputDiffRemoved
                | SemanticStyleRole::OutputDiffHunk
                | SemanticStyleRole::OutputJsonKey
                | SemanticStyleRole::OutputJsonString
                | SemanticStyleRole::OutputJsonNumber
                | SemanticStyleRole::OutputJsonBoolean
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

fn detect_diff_spans(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    let trimmed = row.text.trim_start();
    let role = if trimmed.starts_with("@@") {
        Some(SemanticStyleRole::OutputDiffHunk)
    } else if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        Some(SemanticStyleRole::OutputDiffAdded)
    } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        Some(SemanticStyleRole::OutputDiffRemoved)
    } else {
        None
    };

    if let Some(role) = role {
        push_unique_span(
            spans,
            SemanticSpan {
                row: row.row_index,
                start_col: 0,
                end_col: row.text.chars().count().saturating_sub(1) as u32,
                role,
                text: row.text.clone(),
            },
        );
    }
}

fn detect_phrase_spans(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    for (keyword, role) in SEVERITY_KEYWORDS {
        for (start_byte, end_byte) in find_ascii_occurrences(&row.text, keyword) {
            push_unique_span(spans, slice_span(row, start_byte, end_byte, *role));
        }
    }

    for (phrase, role) in FAILURE_PHRASES {
        for (start_byte, end_byte) in find_case_insensitive_occurrences(&row.text, phrase) {
            push_unique_span(spans, slice_span(row, start_byte, end_byte, *role));
        }
    }

    for phrase in SUCCESS_PHRASES {
        for (start_byte, end_byte) in find_case_insensitive_occurrences(&row.text, phrase) {
            push_unique_span(
                spans,
                slice_span(
                    row,
                    start_byte,
                    end_byte,
                    SemanticStyleRole::OutputSuccessKeyword,
                ),
            );
        }
    }
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
        } else if is_timestamp(cleaned) {
            Some(SemanticStyleRole::OutputTimestamp)
        } else {
            None
        };

        if let Some(role) = role {
            push_unique_span(spans, slice_span(row, start_byte, end_byte, role));
        }
    }
}

fn detect_json_spans(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    let bytes = row.text.as_bytes();
    let mut index = 0usize;
    let mut in_string = false;
    let mut string_start = 0usize;
    let mut escaped = false;

    while index < bytes.len() {
        let ch = bytes[index] as char;
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                let end = index + 1;
                let role = if row.text[end..].trim_start().starts_with(':') {
                    SemanticStyleRole::OutputJsonKey
                } else {
                    SemanticStyleRole::OutputJsonString
                };
                push_unique_span(spans, slice_span(row, string_start, end, role));
                in_string = false;
            }
            index += 1;
            continue;
        }

        if ch == '"' {
            in_string = true;
            string_start = index;
            index += 1;
            continue;
        }

        if ch.is_ascii_digit() || ch == '-' {
            let start = index;
            index += 1;
            while index < bytes.len()
                && ((bytes[index] as char).is_ascii_digit() || bytes[index] == b'.')
            {
                index += 1;
            }
            if row.text[start..index]
                .chars()
                .any(|value| value.is_ascii_digit())
            {
                push_unique_span(
                    spans,
                    slice_span(row, start, index, SemanticStyleRole::OutputJsonNumber),
                );
            }
            continue;
        }

        let mut matched_literal = false;
        for literal in ["true", "false", "null"] {
            if row.text[index..].starts_with(literal) {
                push_unique_span(
                    spans,
                    slice_span(
                        row,
                        index,
                        index + literal.len(),
                        SemanticStyleRole::OutputJsonBoolean,
                    ),
                );
                index += literal.len();
                matched_literal = true;
                break;
            }
        }
        if matched_literal {
            continue;
        }

        index += 1;
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

fn is_timestamp(token: &str) -> bool {
    let bytes = token.as_bytes();
    token.len() >= 10
        && bytes.get(4) == Some(&b'-')
        && bytes.get(7) == Some(&b'-')
        && (token.contains('T') || token.contains(':'))
}
