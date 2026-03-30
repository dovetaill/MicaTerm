//! Shared terminal text layout and shaping contracts.

pub mod run_segmentation;
pub mod shaper;

pub use run_segmentation::{SegmentedRun, TextStyleKey, segment_row};
pub use shaper::{GlyphRun, HarfBuzzTextShaper, PositionedGlyph, ShapedRow, TextShaper, shape_row};
