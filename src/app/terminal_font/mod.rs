//! Shared terminal font backends and test fixtures for the shaping pipeline.

pub mod backend;
pub mod mock;
#[cfg(feature = "terminal-native-renderer")]
pub mod windows_dwrite;

pub use backend::{FontFaceKey, FontMetrics, FontRequest, FontSystem};
#[cfg(feature = "terminal-native-renderer")]
pub use windows_dwrite::{DirectWriteFontSystem, GlyphRasterRequest, RasterizedGlyph};
