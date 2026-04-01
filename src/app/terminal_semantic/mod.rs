//! Semantic overlay descriptors derived from terminal model frames.

mod input_line;
mod output_blocks;

pub use input_line::{
    SemanticInputOverlay, SemanticInputSpanKind, detect_input_line_overlays,
};
pub use output_blocks::{
    SemanticOutputBlockKind, SemanticOutputOverlay, SemanticOverlayRowRange,
    detect_output_block_overlays,
};
