//! Shared terminal text layout and shaping contracts.

pub mod run_segmentation;
#[cfg(feature = "terminal-native-renderer")]
pub mod shaper;

pub use run_segmentation::{SegmentedRun, TextStyleKey, segment_row};
#[cfg(feature = "terminal-native-renderer")]
pub use shaper::{GlyphRun, HarfBuzzTextShaper, PositionedGlyph, ShapedRow, TextShaper, shape_row};
