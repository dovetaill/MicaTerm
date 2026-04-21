//! Incremental output rule analysis for terminal scrollback.

use serde::{Deserialize, Serialize};

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};

use super::types::{SemanticPriority, SemanticSpan, SemanticStyleRole};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OutputRuleProfile {
    #[default]
    Default,
}

impl OutputRuleProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "default" => Self::Default,
            _ => Self::Default,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputRule {
    Url,
    FilePath,
    LineColumn,
    IpPort,
    Timestamp,
    LevelToken,
    SuccessKeyword,
    FailureKeyword,
    GrepMatch,
    GitAdded,
    GitRemoved,
    GitHunk,
    Json,
    Xml,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutputRuleConfig {
    pub enabled: bool,
    pub max_lookbehind_lines: u32,
    pub profile: OutputRuleProfile,
    pub rules: Vec<OutputRule>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OutputRuleAnalysis {
    pub spans: Vec<SemanticSpan>,
    pub analyzed_rows: Vec<u32>,
}

impl Default for OutputRuleConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_lookbehind_lines: 2,
            profile: OutputRuleProfile::Default,
            rules: vec![
                OutputRule::Url,
                OutputRule::FilePath,
                OutputRule::LineColumn,
                OutputRule::IpPort,
                OutputRule::Timestamp,
                OutputRule::LevelToken,
                OutputRule::SuccessKeyword,
                OutputRule::FailureKeyword,
                OutputRule::GrepMatch,
                OutputRule::GitAdded,
                OutputRule::GitRemoved,
                OutputRule::GitHunk,
                OutputRule::Json,
                OutputRule::Xml,
            ],
        }
    }
}

pub fn analyze_output_rules(frame: &TerminalModelFrame, dirty_rows: &[u32]) -> OutputRuleAnalysis {
    analyze_output_rules_with_config(frame, dirty_rows, &OutputRuleConfig::default())
}

pub fn analyze_output_rules_with_config(
    frame: &TerminalModelFrame,
    dirty_rows: &[u32],
    config: &OutputRuleConfig,
) -> OutputRuleAnalysis {
    if !config.enabled || frame.alternate_screen_active || frame.mouse_grabbed {
        return OutputRuleAnalysis::default();
    }

    let analyzed_rows = analyzed_row_window(frame, dirty_rows, config.max_lookbehind_lines);
    let mut spans = Vec::new();

    for row in &analyzed_rows {
        let Some(model_row) = frame.rows.get(*row as usize) else {
            continue;
        };
        analyze_row_rules(model_row, &mut spans);
    }
    analyze_json_blocks(frame, &analyzed_rows, &mut spans);
    analyze_xml_blocks(frame, &analyzed_rows, &mut spans);

    OutputRuleAnalysis {
        spans,
        analyzed_rows,
    }
}

pub fn analyzed_row_window(
    frame: &TerminalModelFrame,
    dirty_rows: &[u32],
    max_lookbehind_lines: u32,
) -> Vec<u32> {
    if dirty_rows.is_empty() {
        return frame.rows.iter().map(|row| row.row_index).collect();
    }

    let mut rows = Vec::new();
    for dirty_row in dirty_rows {
        let start = dirty_row.saturating_sub(max_lookbehind_lines);
        for row in start..=*dirty_row {
            push_unique_row(&mut rows, row);
        }
    }
    rows
}

fn analyze_row_rules(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    if let Some((start, end)) = find_url_range(&row.text) {
        push_unique_span(
            spans,
            SemanticSpan::new(
                row.row_index,
                start,
                end,
                SemanticStyleRole::OutputUrl,
                SemanticPriority::High,
            ),
        );
    }

    if let Some((path_start, path_end, line_col_end)) = find_path_reference(&row.text) {
        push_unique_span(
            spans,
            SemanticSpan::new(
                row.row_index,
                path_start,
                path_end,
                SemanticStyleRole::OutputFilePath,
                SemanticPriority::High,
            ),
        );
        if let Some(line_col_end) = line_col_end {
            push_unique_span(
                spans,
                SemanticSpan::new(
                    row.row_index,
                    path_end.saturating_add(1),
                    line_col_end,
                    SemanticStyleRole::OutputLineColumn,
                    SemanticPriority::Normal,
                ),
            );
        }
    }

    let trimmed = row.text.trim_start();
    if trimmed.starts_with("@@") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputGitHunk,
            SemanticPriority::High,
            spans,
        );
    } else if trimmed.starts_with('+') && !trimmed.starts_with("+++") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputGitAdded,
            SemanticPriority::Normal,
            spans,
        );
    } else if trimmed.starts_with('-') && !trimmed.starts_with("---") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputGitRemoved,
            SemanticPriority::Normal,
            spans,
        );
    }

    let lower = row.text.to_ascii_lowercase();
    if lower.contains("error:") || trimmed.starts_with("[ERROR]") || trimmed.starts_with("ERROR ") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputLevelError,
            SemanticPriority::High,
            spans,
        );
    } else if lower.contains("warn:")
        || trimmed.starts_with("[WARN]")
        || trimmed.starts_with("WARN ")
    {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputLevelWarn,
            SemanticPriority::High,
            spans,
        );
    } else if lower.contains("info:")
        || trimmed.starts_with("[INFO]")
        || trimmed.starts_with("INFO ")
    {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputLevelInfo,
            SemanticPriority::Normal,
            spans,
        );
    } else if trimmed.starts_with("[DEBUG]") || trimmed.starts_with("DEBUG ") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputLevelDebug,
            SemanticPriority::Normal,
            spans,
        );
    }

    if lower.contains("success") || lower.contains("completed successfully") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputSuccessKeyword,
            SemanticPriority::Normal,
            spans,
        );
    }
    if lower.contains("failed") || lower.contains("command exited with 1") {
        push_full_line_span(
            row,
            SemanticStyleRole::OutputFailureKeyword,
            SemanticPriority::High,
            spans,
        );
    }

    analyze_inline_json(row, spans);
}

fn analyze_json_blocks(
    frame: &TerminalModelFrame,
    analyzed_rows: &[u32],
    spans: &mut Vec<SemanticSpan>,
) {
    for row in analyzed_rows {
        let Some(start_index) = frame.rows.get(*row as usize).map(|_| *row as usize) else {
            continue;
        };
        let Some((start, end)) = detect_json_block(&frame.rows, start_index) else {
            continue;
        };
        for row in &frame.rows[start..=end] {
            push_full_line_span(
                row,
                SemanticStyleRole::OutputJson,
                SemanticPriority::Normal,
                spans,
            );
        }
    }
}

fn analyze_xml_blocks(
    frame: &TerminalModelFrame,
    analyzed_rows: &[u32],
    spans: &mut Vec<SemanticSpan>,
) {
    for row in analyzed_rows {
        let Some(start_index) = frame.rows.get(*row as usize).map(|_| *row as usize) else {
            continue;
        };
        let Some((start, end)) = detect_xml_block(&frame.rows, start_index) else {
            continue;
        };
        for row in &frame.rows[start..=end] {
            push_full_line_span(
                row,
                SemanticStyleRole::OutputXml,
                SemanticPriority::Normal,
                spans,
            );
        }
    }
}

fn analyze_inline_json(row: &TerminalModelRow, spans: &mut Vec<SemanticSpan>) {
    let trimmed = row.text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return;
    }

    let chars = row.text.chars().collect::<Vec<_>>();
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] != '"' {
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && chars[index] != '"' {
            if chars[index] == '\\' {
                index += 1;
            }
            index += 1;
        }
        if index >= chars.len() {
            break;
        }
        let end = index;
        let mut lookahead = index + 1;
        while lookahead < chars.len() && chars[lookahead].is_whitespace() {
            lookahead += 1;
        }
        let role = if lookahead < chars.len() && chars[lookahead] == ':' {
            SemanticStyleRole::OutputJsonKey
        } else {
            SemanticStyleRole::OutputJsonString
        };
        push_unique_span(
            spans,
            SemanticSpan::new(
                row.row_index,
                start as u32,
                end as u32,
                role,
                SemanticPriority::Normal,
            ),
        );
        index += 1;
    }

    for literal in [
        ("true", SemanticStyleRole::OutputJsonBoolean),
        ("false", SemanticStyleRole::OutputJsonBoolean),
    ] {
        if let Some((start, end)) = find_literal_range(&row.text, literal.0) {
            push_unique_span(
                spans,
                SemanticSpan::new(
                    row.row_index,
                    start,
                    end,
                    literal.1,
                    SemanticPriority::Normal,
                ),
            );
        }
    }
}

fn detect_json_block(rows: &[TerminalModelRow], start_index: usize) -> Option<(usize, usize)> {
    let first_line = rows.get(start_index)?.text.trim_start();
    if !(first_line.starts_with('{') || first_line.starts_with('[')) {
        return None;
    }
    if first_line.starts_with('[') && is_bracketed_log_level(first_line) {
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
            return Some((start_index, offset));
        }
    }

    None
}

fn detect_xml_block(rows: &[TerminalModelRow], start_index: usize) -> Option<(usize, usize)> {
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
            return Some((start_index, offset));
        }
    }

    None
}

fn is_bracketed_log_level(line: &str) -> bool {
    matches!(
        line,
        value
            if value.starts_with("[TRACE]")
                || value.starts_with("[DEBUG]")
                || value.starts_with("[INFO]")
                || value.starts_with("[WARN]")
                || value.starts_with("[ERROR]")
    )
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

fn find_url_range(line: &str) -> Option<(u32, u32)> {
    for needle in ["https://", "http://"] {
        if let Some(start_byte) = line.find(needle) {
            let end_byte = line[start_byte..]
                .find(char::is_whitespace)
                .map(|offset| start_byte + offset)
                .unwrap_or(line.len());
            return Some((
                char_col(line, start_byte),
                char_col(line, end_byte).saturating_sub(1),
            ));
        }
    }

    None
}

fn find_path_reference(line: &str) -> Option<(u32, u32, Option<u32>)> {
    for token in line.split_whitespace() {
        if token.starts_with('<') || token.contains('>') || !looks_like_path(token) {
            continue;
        }
        let start_byte = line.find(token)?;
        if let Some((path_end, line_col_end)) = split_path_line_column(token) {
            return Some((
                char_col(line, start_byte),
                char_col(line, start_byte + path_end).saturating_sub(1),
                Some(char_col(line, start_byte + line_col_end).saturating_sub(1)),
            ));
        }
        let end_byte = start_byte + token.len();
        return Some((
            char_col(line, start_byte),
            char_col(line, end_byte).saturating_sub(1),
            None,
        ));
    }

    None
}

fn split_path_line_column(token: &str) -> Option<(usize, usize)> {
    let bytes = token.as_bytes();
    for index in 0..bytes.len() {
        if bytes[index] != b':' {
            continue;
        }
        let rest = &token[index + 1..];
        let digit_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
        if digit_count == 0 {
            continue;
        }
        let mut end = index + 1 + digit_count;
        if token.get(end..=end) == Some(":") {
            let rest = &token[end + 1..];
            let column_count = rest.chars().take_while(|ch| ch.is_ascii_digit()).count();
            if column_count > 0 {
                end += 1 + column_count;
            }
        }
        return Some((index, end));
    }

    None
}

fn looks_like_path(token: &str) -> bool {
    token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with('/')
        || token.starts_with("~/")
        || token.contains('/')
        || token.contains('\\')
        || token.ends_with(".rs")
        || token.ends_with(".toml")
        || token.ends_with(".json")
}

fn find_literal_range(line: &str, literal: &str) -> Option<(u32, u32)> {
    let start_byte = line.find(literal)?;
    let end_byte = start_byte + literal.len();
    Some((
        char_col(line, start_byte),
        char_col(line, end_byte).saturating_sub(1),
    ))
}

fn push_full_line_span(
    row: &TerminalModelRow,
    role: SemanticStyleRole,
    priority: SemanticPriority,
    spans: &mut Vec<SemanticSpan>,
) {
    push_unique_span(
        spans,
        SemanticSpan::new(
            row.row_index,
            0,
            row.text.chars().count().saturating_sub(1) as u32,
            role,
            priority,
        ),
    );
}

fn push_unique_span(spans: &mut Vec<SemanticSpan>, span: SemanticSpan) {
    if !spans.contains(&span) {
        spans.push(span);
    }
}

fn push_unique_row(rows: &mut Vec<u32>, row: u32) {
    if !rows.contains(&row) {
        rows.push(row);
    }
}

fn char_col(text: &str, byte_index: usize) -> u32 {
    text[..byte_index].chars().count() as u32
}
