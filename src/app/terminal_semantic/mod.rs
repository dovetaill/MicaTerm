//! Semantic annotations derived from terminal model frames.

mod command_blocks;
mod input_line;
mod output_blocks;
mod types;

pub use command_blocks::{
    CommandBlock, CommandBlockStatus, CommandLedger, OverviewMarker, OverviewMarkerKind,
    command_blocks_from_frame, command_blocks_from_lines,
};
pub use input_line::detect_input_line_spans;
pub use output_blocks::detect_output_block_spans;
pub use types::{SemanticPriority, SemanticSpan, SemanticStyleRole};
