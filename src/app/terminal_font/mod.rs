//! Shared terminal font backends and test fixtures for the shaping pipeline.

pub mod backend;
pub mod mock;
pub mod wezterm_font;
#[cfg(feature = "terminal-native-renderer")]
pub mod windows_dwrite;
#[cfg(feature = "terminal-native-renderer")]
pub mod windows_fallback;
#[cfg(feature = "terminal-native-renderer")]
pub mod windows_locator;

#[cfg(feature = "terminal-native-renderer")]
pub use backend::{
    ColorGlyphRaster, FontFallbackFace, GlyphRasterRequest, OpenTypeFeatureSet, RasterizedGlyph,
    ShapedGlyph, ShapedGlyphRun, TextShapingRequest,
};
pub use backend::{
    DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY, DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY,
    DEFAULT_TERMINAL_FONT_FAMILY, DEFAULT_TERMINAL_FONT_SIZE_PX, DEFAULT_TERMINAL_FONT_WEIGHT,
    DEFAULT_TERMINAL_LETTER_SPACING_PX, DEFAULT_TERMINAL_LINE_HEIGHT, FontFaceKey, FontMetrics,
    FontRenderProfile, FontRequest, FontSystem, LoadedFont, LoadedFontKey,
    WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN,
};
pub use wezterm_font::WeztermFontSystem;
#[cfg(feature = "terminal-native-renderer")]
pub use windows_dwrite::DirectWriteFontSystem;
#[cfg(feature = "terminal-native-renderer")]
pub use windows_fallback::WindowsFontFallbackResolver;
#[cfg(feature = "terminal-native-renderer")]
pub use windows_locator::WindowsFontLocator;
