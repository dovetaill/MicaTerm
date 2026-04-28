//! Command-block and overview marker detection for prompt-delimited terminal history.

use crate::app::terminal_model::TerminalModelFrame;
use crate::app::terminal_semantic::SemanticSpan;
use crate::app::terminal_semantic::input_line::prompt_input_start;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandBlockStatus {
    Running,
    Success,
    Failure,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlock {
    pub start_row: u32,
    pub end_row: u32,
    pub command_row: u32,
    pub command_text: String,
    pub status: CommandBlockStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewMarkerKind {
    CommandRunning,
    CommandSuccess,
    CommandFailure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewMarker {
    pub row: u32,
    pub kind: OverviewMarkerKind,
}

pub fn detect_command_blocks(
    frame: &TerminalModelFrame,
    spans: &[SemanticSpan],
) -> Vec<CommandBlock> {
    let _ = spans;
    if !frame.shell_integration.has_markers {
        return Vec::new();
    }

    let mut prompt_rows = Vec::new();
    for (index, row) in frame.rows.iter().enumerate() {
        let Some(input_start_col) = prompt_input_start(row) else {
            continue;
        };
        let command_text = substring_by_cols(&row.text, input_start_col)
            .trim()
            .to_string();
        if command_text.is_empty() {
            continue;
        }
        prompt_rows.push((index, row.row_index, command_text));
    }

    let mut blocks = Vec::new();
    for (position, (_prompt_index, prompt_row, command_text)) in prompt_rows.iter().enumerate() {
        let next_prompt_index = prompt_rows.get(position + 1).map(|entry| entry.0);
        let end_index = next_prompt_index
            .map(|value| value.saturating_sub(1))
            .unwrap_or_else(|| frame.rows.len().saturating_sub(1));
        let end_row = frame
            .rows
            .get(end_index)
            .map(|row| row.row_index)
            .unwrap_or(*prompt_row);
        let status = if next_prompt_index.is_none() && frame.shell_integration.input_active {
            CommandBlockStatus::Running
        } else {
            CommandBlockStatus::Success
        };
        blocks.push(CommandBlock {
            start_row: *prompt_row,
            end_row,
            command_row: *prompt_row,
            command_text: command_text.clone(),
            status,
        });
    }

    blocks
}

pub fn overview_markers_for(blocks: &[CommandBlock]) -> Vec<OverviewMarker> {
    blocks
        .iter()
        .map(|block| OverviewMarker {
            row: block.command_row,
            kind: match block.status {
                CommandBlockStatus::Running => OverviewMarkerKind::CommandRunning,
                CommandBlockStatus::Success => OverviewMarkerKind::CommandSuccess,
                CommandBlockStatus::Failure => OverviewMarkerKind::CommandFailure,
            },
        })
        .collect()
}

fn substring_by_cols(text: &str, start_col: u32) -> String {
    text.chars().skip(start_col as usize).collect()
}
