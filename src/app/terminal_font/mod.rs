//! Shared terminal font backends and test fixtures for the shaping pipeline.

pub mod backend;
pub mod mock;
pub mod windows_dwrite;

pub use backend::{FontFaceKey, FontMetrics, FontRequest, FontSystem};
pub use windows_dwrite::{DirectWriteFontSystem, GlyphRasterRequest, RasterizedGlyph};
