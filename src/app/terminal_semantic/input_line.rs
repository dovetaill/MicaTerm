//! Semantic input-line highlighting for normal shell prompts.

use crate::app::terminal_model::{TerminalModelFrame, TerminalModelRow};

use super::types::{SemanticPriority, SemanticSpan, SemanticStyleRole};

const PROMPT_MARKERS: &[&str] = &["$ ", "# ", "% ", "> "];
const SHELL_OPERATORS: &[&str] = &[
    "|", "||", "&&", ";", ">", ">>", "<", "2>", "2>>", "2>&1", "&",
];

pub fn detect_input_line_spans(frame: &TerminalModelFrame) -> Vec<SemanticSpan> {
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

    let mut spans = vec![SemanticSpan::new(
        row.row_index,
        0,
        input_start_col.saturating_sub(1),
        SemanticStyleRole::InputPrompt,
        SemanticPriority::Normal,
    )];
    let mut command_context = CommandContext::default();

    for token in tokens {
        let role = classify_token(token.text.as_str(), &command_context);
        spans.push(SemanticSpan::new(
            row.row_index,
            token.start_col,
            token.end_col,
            role,
            priority_for(role),
        ));
        command_context.observe(token.text.as_str(), role);
    }

    spans
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
        .filter_map(|marker| {
            row.text.match_indices(marker).next().map(|(index, _)| (index, *marker))
        })
        .min_by_key(|(index, _)| *index)?;
    let input_start = marker_start + marker.len();
    if row.text[input_start..].trim().is_empty() {
        return None;
    }

    Some(char_col(&row.text, input_start) as u32)
}

#[derive(Clone, Copy, Debug, Default)]
struct CommandContext {
    expecting_command: bool,
    saw_primary_command: bool,
}

impl CommandContext {
    fn observe(&mut self, token: &str, role: SemanticStyleRole) {
        if is_shell_operator(token) {
            self.expecting_command = true;
            return;
        }

        match role {
            SemanticStyleRole::InputCommand => {
                self.expecting_command = false;
                self.saw_primary_command = true;
            }
            SemanticStyleRole::InputSubcommand => {
                self.expecting_command = false;
            }
            _ => {}
        }
    }
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

fn classify_token(token: &str, context: &CommandContext) -> SemanticStyleRole {
    if is_shell_operator(token) {
        return SemanticStyleRole::InputOperator;
    }
    if token.starts_with('$') {
        return SemanticStyleRole::InputVariable;
    }
    if is_quoted(token) {
        return SemanticStyleRole::InputString;
    }
    if token.starts_with('-') {
        return SemanticStyleRole::InputOption;
    }
    if is_path_like(token) {
        return SemanticStyleRole::InputPath;
    }
    if context.expecting_command || !context.saw_primary_command {
        return SemanticStyleRole::InputCommand;
    }

    SemanticStyleRole::InputArgument
}

fn priority_for(role: SemanticStyleRole) -> SemanticPriority {
    match role {
        SemanticStyleRole::InputPrompt => SemanticPriority::Low,
        SemanticStyleRole::InputCommand
        | SemanticStyleRole::InputSubcommand
        | SemanticStyleRole::InputInvalidCommand => SemanticPriority::High,
        SemanticStyleRole::InputVariable | SemanticStyleRole::InputOperator => {
            SemanticPriority::High
        }
        SemanticStyleRole::InputOption
        | SemanticStyleRole::InputArgument
        | SemanticStyleRole::InputString
        | SemanticStyleRole::InputPath => SemanticPriority::Normal,
        _ => SemanticPriority::Normal,
    }
}

fn is_quoted(token: &str) -> bool {
    (token.starts_with('"') && token.ends_with('"'))
        || (token.starts_with('\'') && token.ends_with('\''))
}

fn is_path_like(token: &str) -> bool {
    token.starts_with("./")
        || token.starts_with("../")
        || token.starts_with('/')
        || token.starts_with("~/")
        || token.contains('/')
}

fn is_shell_operator(token: &str) -> bool {
    SHELL_OPERATORS.contains(&token)
}

fn char_col(text: &str, byte_index: usize) -> usize {
    text[..byte_index].chars().count()
}
