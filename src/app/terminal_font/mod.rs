//! Shared terminal font backends and test fixtures for the shaping pipeline.

pub mod backend;
pub mod mock;
pub mod wezterm_font;
#[cfg(feature = "terminal-native-renderer")]
pub mod windows_dwrite;

pub use backend::{
    FontFaceKey, FontMetrics, FontRenderProfile, FontRequest, FontSystem, LoadedFont,
    LoadedFontKey,
};
#[cfg(feature = "terminal-native-renderer")]
pub use backend::{
    ColorGlyphRaster, FontFallbackFace, GlyphRasterRequest, OpenTypeFeatureSet, RasterizedGlyph,
    ShapedGlyph, ShapedGlyphRun, TextShapingRequest,
};
pub use wezterm_font::WeztermFontSystem;
#[cfg(feature = "terminal-native-renderer")]
pub use windows_dwrite::DirectWriteFontSystem;
