//! Semantic annotations derived from terminal model frames.

mod input_line;
mod output_blocks;
mod types;

pub use input_line::detect_input_line_spans;
pub use output_blocks::detect_output_block_spans;
pub use types::{SemanticPriority, SemanticSpan, SemanticStyleRole};
