//! Semantic input-line highlighting for normal shell prompts.

use crate::app::terminal_model::{
    TerminalModelFrame, TerminalModelRow, TerminalPresentationMode,
};
use crate::app::terminal_semantic::{SemanticSpan, push_unique_span};
use crate::theme::SemanticStyleRole;

const PROMPT_OVERLAY_RGBA: u32 = 0x336a_5acd;
const COMMAND_OVERLAY_RGBA: u32 = 0x334f_c3f7;
const ARGUMENT_OVERLAY_RGBA: u32 = 0x3334_d399;
const OPTION_OVERLAY_RGBA: u32 = 0x33f5_a524;
const OPERATOR_OVERLAY_RGBA: u32 = 0x33ef_6c00;
pub(crate) const PROMPT_MARKERS: &[&str] = &["$ ", "% ", "]# "];
const SHELL_OPERATORS: &[&str] = &["||", "&&", ">>", "2>>", "2>", "|", ";", ">", "<"];
const COMMAND_SEPARATORS: &[&str] = &["||", "&&", "|", ";"];
const COMPOUND_COMMANDS: &[&str] = &[
    "git", "cargo", "docker", "kubectl", "ssh", "sftp", "rsync", "npm", "pnpm", "yarn",
];

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

#[derive(Clone, Debug)]
pub(crate) struct ShellToken {
    pub text: String,
    pub start_col: u32,
    pub end_col: u32,
    pub quoted: bool,
}

pub fn detect_input_semantic_spans(frame: &TerminalModelFrame) -> Vec<SemanticSpan> {
    if !input_highlighting_is_safe(frame) {
        return Vec::new();
    }

    if frame.shell_integration.has_markers && !frame.shell_integration.input_active {
        return Vec::new();
    }

    let Some(row) = frame.rows.last()
    else {
        return Vec::new();
    };
    let Some(input_start_col) = prompt_input_start(row) else {
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

    let mut spans = Vec::new();
    if input_start_col > 0 {
        push_unique_span(
            &mut spans,
            SemanticSpan {
                row: row.row_index,
                start_col: 0,
                end_col: input_start_col.saturating_sub(1),
                role: SemanticStyleRole::InputPrompt,
                text: row.text.chars().take(input_start).collect(),
            },
        );
    }

    let mut command_expected = true;
    let mut compound_mode = false;
    let mut subcommand_consumed = false;

    for token in tokens {
        let role = if is_shell_operator(token.text.as_str()) {
            if is_command_separator(token.text.as_str()) {
                command_expected = true;
                compound_mode = false;
                subcommand_consumed = false;
            }
            SemanticStyleRole::InputOperator
        } else if command_expected {
            command_expected = false;
            compound_mode = COMPOUND_COMMANDS.contains(&token.text.as_str());
            subcommand_consumed = false;
            SemanticStyleRole::InputCommand
        } else if compound_mode && !subcommand_consumed && !token.text.starts_with('-') {
            subcommand_consumed = true;
            SemanticStyleRole::InputSubcommand
        } else if token.quoted {
            SemanticStyleRole::InputString
        } else if token.text.starts_with('-') {
            SemanticStyleRole::InputOption
        } else if is_variable_token(token.text.as_str()) {
            SemanticStyleRole::InputVariable
        } else if is_path_token(token.text.as_str()) {
            SemanticStyleRole::InputPath
        } else {
            SemanticStyleRole::InputArgument
        };
        push_unique_span(
            &mut spans,
            SemanticSpan {
                row: row.row_index,
                start_col: token.start_col,
                end_col: token.end_col,
                role,
                text: token.text,
            },
        );
    }

    spans
}

pub fn detect_input_line_overlays(frame: &TerminalModelFrame) -> Vec<SemanticInputOverlay> {
    detect_input_semantic_spans(frame)
        .into_iter()
        .map(|span| {
            let kind = match span.role {
                SemanticStyleRole::InputPrompt => SemanticInputSpanKind::Prompt,
                SemanticStyleRole::InputCommand => SemanticInputSpanKind::Command,
                SemanticStyleRole::InputOption => SemanticInputSpanKind::Option,
                SemanticStyleRole::InputOperator => SemanticInputSpanKind::Operator,
                _ => SemanticInputSpanKind::Argument,
            };
            SemanticInputOverlay {
                kind,
                row: span.row,
                start_col: span.start_col,
                end_col: span.end_col,
                overlay_rgba: overlay_rgba(kind),
            }
        })
        .collect()
}

fn input_highlighting_is_safe(frame: &TerminalModelFrame) -> bool {
    matches!(frame.presentation_mode, TerminalPresentationMode::ShellLive)
}

pub(crate) fn prompt_input_start(row: &TerminalModelRow) -> Option<u32> {
    let trimmed = row.text.trim_end();
    if trimmed.is_empty() || row.wrapped {
        return None;
    }

    let (marker_start, marker) = PROMPT_MARKERS
        .iter()
        .filter_map(|marker| {
            row.text
                .rmatch_indices(marker)
                .next()
                .map(|(index, _)| (index, *marker))
        })
        .max_by_key(|(index, _)| *index)?;
    let input_start = marker_start + marker.len();
    if row.text[input_start..].trim().is_empty() {
        return None;
    }

    Some(char_col(&row.text, input_start) as u32)
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

        if let Some(operator_len) = shell_operator_len_at(&chars, index) {
            let text = chars[index..index + operator_len]
                .iter()
                .collect::<String>();
            tokens.push(ShellToken {
                text,
                start_col: index as u32,
                end_col: (index + operator_len - 1) as u32,
                quoted: false,
            });
            index += operator_len;
            continue;
        }

        let start = index;
        let mut in_single = false;
        let mut in_double = false;
        let mut escaped = false;
        let mut quoted = false;

        while index < chars.len() {
            let ch = chars[index];
            if in_single {
                if ch == '\'' {
                    in_single = false;
                }
                index += 1;
                continue;
            }
            if in_double {
                if escaped {
                    escaped = false;
                    index += 1;
                    continue;
                }
                if ch == '\\' {
                    escaped = true;
                    index += 1;
                    continue;
                }
                if ch == '"' {
                    in_double = false;
                    index += 1;
                    continue;
                }
                index += 1;
                continue;
            }
            if ch.is_whitespace() {
                break;
            }
            if index > start && shell_operator_len_at(&chars, index).is_some() {
                break;
            }
            if ch == '\'' {
                quoted = true;
                in_single = true;
                index += 1;
                continue;
            }
            if ch == '"' {
                quoted = true;
                in_double = true;
                index += 1;
                continue;
            }
            index += 1;
        }

        let end = index.saturating_sub(1);
        if start <= end {
            tokens.push(ShellToken {
                text: chars[start..=end].iter().collect::<String>(),
                start_col: start as u32,
                end_col: end as u32,
                quoted,
            });
        }
    }

    tokens
}

fn shell_operator_len_at(chars: &[char], index: usize) -> Option<usize> {
    for operator in SHELL_OPERATORS {
        let operator_chars = operator.chars().collect::<Vec<_>>();
        if chars.get(index..index + operator_chars.len()) == Some(operator_chars.as_slice()) {
            return Some(operator_chars.len());
        }
    }
    None
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

fn is_command_separator(token: &str) -> bool {
    COMMAND_SEPARATORS.contains(&token)
}

fn is_variable_token(token: &str) -> bool {
    token.starts_with('$') || token.contains("${")
}

fn is_path_token(token: &str) -> bool {
    token.starts_with('/')
        || token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with("~/")
        || (token.len() > 3
            && token.as_bytes()[0].is_ascii_alphabetic()
            && token.as_bytes()[1] == b':'
            && (token.as_bytes()[2] == b'\\' || token.as_bytes()[2] == b'/'))
}

fn char_col(text: &str, byte_index: usize) -> usize {
    text[..byte_index].chars().count()
}
