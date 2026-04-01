//! Semantic input-line highlighting for normal shell prompts.

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};

const PROMPT_OVERLAY_RGBA: u32 = 0x336a_5acd;
const COMMAND_OVERLAY_RGBA: u32 = 0x334f_c3f7;
const ARGUMENT_OVERLAY_RGBA: u32 = 0x3334_d399;
const OPTION_OVERLAY_RGBA: u32 = 0x33f5_a524;
const OPERATOR_OVERLAY_RGBA: u32 = 0x33ef_6c00;
const PROMPT_MARKERS: &[&str] = &["$ ", "# ", "% ", "> "];
const SHELL_OPERATORS: &[&str] = &["|", "||", "&&", ";", ">", ">>", "<", "2>", "2>>"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SemanticInputSpanKind {
    Prompt,
    Command,
    Argument,
    Option,
    Operator,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SemanticInputOverlay {
    pub kind: SemanticInputSpanKind,
    pub row: u32,
    pub start_col: u32,
    pub end_col: u32,
    pub overlay_rgba: u32,
}

pub fn detect_input_line_overlays(frame: &TerminalModelFrame) -> Vec<SemanticInputOverlay> {
    if !input_highlighting_is_safe(frame) {
        return Vec::new();
    }

    let Some((row, input_start_col)) = frame
        .rows
        .iter()
        .rev()
        .find_map(|row| prompt_input_start(row).map(|input_start_col| (row, input_start_col)))
    else {
        return Vec::new();
    };

    let input_start = input_start_col as usize;
    let line_cols = row.text.chars().count();
    if input_start >= line_cols {
        return Vec::new();
    }

    let tokens = shell_tokens(row.text.as_str(), input_start_col);
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut overlays = vec![SemanticInputOverlay {
        kind: SemanticInputSpanKind::Prompt,
        row: row.row_index,
        start_col: 0,
        end_col: input_start_col.saturating_sub(1),
        overlay_rgba: overlay_rgba(SemanticInputSpanKind::Prompt),
    }];
    let mut saw_command = false;

    for token in tokens {
        let kind = if is_shell_operator(token.text.as_str()) {
            SemanticInputSpanKind::Operator
        } else if !saw_command {
            saw_command = true;
            SemanticInputSpanKind::Command
        } else if token.text.starts_with('-') {
            SemanticInputSpanKind::Option
        } else {
            SemanticInputSpanKind::Argument
        };
        overlays.push(SemanticInputOverlay {
            kind,
            row: row.row_index,
            start_col: token.start_col,
            end_col: token.end_col,
            overlay_rgba: overlay_rgba(kind),
        });
    }

    overlays
}

fn input_highlighting_is_safe(frame: &TerminalModelFrame) -> bool {
    !frame.alternate_screen_active && !frame.mouse_grabbed && frame.viewport_at_bottom
}

fn prompt_input_start(row: &TerminalModelRow) -> Option<u32> {
    let trimmed = row.text.trim_end();
    if trimmed.is_empty() || row.wrapped {
        return None;
    }

    let (marker_start, marker) = PROMPT_MARKERS
        .iter()
        .filter_map(|marker| row.text.rmatch_indices(marker).next().map(|(index, _)| (index, *marker)))
        .max_by_key(|(index, _)| *index)?;
    let input_start = marker_start + marker.len();
    if row.text[input_start..].trim().is_empty() {
        return None;
    }

    Some(char_col(&row.text, input_start) as u32)
}

#[derive(Clone, Debug)]
struct ShellToken {
    text: String,
    start_col: u32,
    end_col: u32,
}

fn shell_tokens(line: &str, input_start_col: u32) -> Vec<ShellToken> {
    let chars = line.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = input_start_col as usize;

    while index < chars.len() {
        while index < chars.len() && chars[index].is_whitespace() {
            index += 1;
        }
        if index >= chars.len() {
            break;
        }

        let start = index;
        while index < chars.len() && !chars[index].is_whitespace() {
            index += 1;
        }
        let end = index.saturating_sub(1);
        let text = chars[start..=end].iter().collect::<String>();
        tokens.push(ShellToken {
            text,
            start_col: start as u32,
            end_col: end as u32,
        });
    }

    tokens
}

fn overlay_rgba(kind: SemanticInputSpanKind) -> u32 {
    match kind {
        SemanticInputSpanKind::Prompt => PROMPT_OVERLAY_RGBA,
        SemanticInputSpanKind::Command => COMMAND_OVERLAY_RGBA,
        SemanticInputSpanKind::Argument => ARGUMENT_OVERLAY_RGBA,
        SemanticInputSpanKind::Option => OPTION_OVERLAY_RGBA,
        SemanticInputSpanKind::Operator => OPERATOR_OVERLAY_RGBA,
    }
}

fn is_shell_operator(token: &str) -> bool {
    SHELL_OPERATORS.contains(&token)
}

fn char_col(text: &str, byte_index: usize) -> usize {
    text[..byte_index].chars().count()
}
