//! Command block ledgering for prompt-oriented terminal sessions.

use crate::app::ssh::runtime::{TerminalRowState, TerminalSurfaceState};
use crate::app::terminal_model::TerminalModelFrame;

const PROMPT_MARKERS: &[&str] = &["$ ", "# ", "% ", "> "];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandBlockStatus {
    Running,
    Success,
    Failure,
    Unknown,
}

impl CommandBlockStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandBlock {
    pub id: u64,
    pub command_text: String,
    pub prompt_row: u32,
    pub command_start_row: u32,
    pub command_end_row: u32,
    pub output_start_row: u32,
    pub output_end_row: u32,
    pub status: CommandBlockStatus,
    pub exit_code: Option<i32>,
    pub cwd: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverviewMarkerKind {
    CommandRunning,
    CommandFailure,
    CommandSuccess,
    SearchMatch,
    Error,
    Warning,
}

impl OverviewMarkerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CommandRunning => "command-running",
            Self::CommandFailure => "command-failure",
            Self::CommandSuccess => "command-success",
            Self::SearchMatch => "search-match",
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverviewMarker {
    pub row: u32,
    pub kind: OverviewMarkerKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommandLedger {
    pub blocks: Vec<CommandBlock>,
    pub overview_markers: Vec<OverviewMarker>,
}

pub fn command_blocks_from_lines(lines: &[&str]) -> CommandLedger {
    let indexed = lines
        .iter()
        .enumerate()
        .map(|(row, text)| IndexedLine {
            row: row as u32,
            text,
            wrapped: false,
        })
        .collect::<Vec<_>>();
    command_blocks_from_indexed_lines(&indexed)
}

pub fn command_blocks_from_frame(frame: &TerminalModelFrame) -> CommandLedger {
    if frame.alternate_screen_active || frame.mouse_grabbed {
        return CommandLedger::default();
    }

    let indexed = frame
        .rows
        .iter()
        .map(|row| IndexedLine {
            row: row.row_index,
            text: row.text.as_str(),
            wrapped: row.wrapped,
        })
        .collect::<Vec<_>>();
    command_blocks_from_indexed_lines(&indexed)
}

pub fn command_blocks_from_surface(surface: &TerminalSurfaceState) -> CommandLedger {
    if surface.alternate_screen_active || surface.mouse_grabbed {
        return CommandLedger::default();
    }

    let indexed = surface
        .visible_rows
        .iter()
        .map(indexed_line_from_visible_row)
        .collect::<Vec<_>>();
    command_blocks_from_indexed_lines(&indexed)
}

#[derive(Clone, Copy)]
struct IndexedLine<'a> {
    row: u32,
    text: &'a str,
    wrapped: bool,
}

fn indexed_line_from_visible_row(row: &TerminalRowState) -> IndexedLine<'_> {
    IndexedLine {
        row: row.index,
        text: row.text.as_str(),
        wrapped: row.wrapped,
    }
}

fn command_blocks_from_indexed_lines(lines: &[IndexedLine<'_>]) -> CommandLedger {
    let mut blocks = Vec::new();
    let mut next_id = 1u64;
    let mut current = None::<CommandBlock>;

    for line in lines {
        if let Some(command_text) = prompt_command_text(*line) {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(CommandBlock {
                id: next_id,
                command_text,
                prompt_row: line.row,
                command_start_row: line.row,
                command_end_row: line.row,
                output_start_row: line.row.saturating_add(1),
                output_end_row: line.row,
                status: CommandBlockStatus::Unknown,
                exit_code: None,
                cwd: None,
            });
            next_id += 1;
            continue;
        }

        let Some(block) = current.as_mut() else {
            continue;
        };
        block.output_end_row = line.row;
        update_status(block, line.text);
    }

    if let Some(block) = current.take() {
        blocks.push(block);
    }

    let overview_markers = blocks
        .iter()
        .filter_map(|block| {
            let kind = match block.status {
                CommandBlockStatus::Running => OverviewMarkerKind::CommandRunning,
                CommandBlockStatus::Failure => OverviewMarkerKind::CommandFailure,
                CommandBlockStatus::Success => OverviewMarkerKind::CommandSuccess,
                CommandBlockStatus::Unknown => return None,
            };
            Some(OverviewMarker {
                row: block.output_end_row.max(block.prompt_row),
                kind,
            })
        })
        .collect();

    CommandLedger {
        blocks,
        overview_markers,
    }
}

fn prompt_command_text(line: IndexedLine<'_>) -> Option<String> {
    if line.wrapped || line.text.trim_end().is_empty() {
        return None;
    }

    let (marker_start, marker) = PROMPT_MARKERS
        .iter()
        .filter_map(|marker| {
            line.text
                .rmatch_indices(marker)
                .next()
                .map(|(index, _)| (index, *marker))
        })
        .max_by_key(|(index, _)| *index)?;
    let command = line.text[marker_start + marker.len()..].trim();
    (!command.is_empty()).then(|| command.to_string())
}

fn update_status(block: &mut CommandBlock, line: &str) {
    if let Some(exit_code) = parse_exit_code(line) {
        block.exit_code = Some(exit_code);
        block.status = if exit_code == 0 {
            CommandBlockStatus::Success
        } else {
            CommandBlockStatus::Failure
        };
        return;
    }

    let lower = line.trim().to_ascii_lowercase();
    if lower.contains("running") {
        if matches!(block.status, CommandBlockStatus::Unknown) {
            block.status = CommandBlockStatus::Running;
        }
        return;
    }
    if lower.contains("failed") || lower.contains("error") {
        block.status = CommandBlockStatus::Failure;
        return;
    }
    if lower.contains("success") || lower.contains("completed successfully") {
        block.status = CommandBlockStatus::Success;
    }
}

fn parse_exit_code(line: &str) -> Option<i32> {
    let lower = line.to_ascii_lowercase();
    for prefix in ["command exited with ", "exit code ", "exited with status "] {
        if let Some(rest) = lower.strip_prefix(prefix) {
            let digits = rest
                .chars()
                .take_while(|ch| ch.is_ascii_digit() || *ch == '-')
                .collect::<String>();
            if !digits.is_empty() {
                return digits.parse().ok();
            }
        }
    }

    None
}
