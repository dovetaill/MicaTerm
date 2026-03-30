//! Terminal renderer integration seams for native surface presentation.

#[cfg(feature = "terminal-native-renderer")]
pub mod atlas;
pub mod native_surface;
#[cfg(feature = "terminal-native-renderer")]
pub mod wgpu_renderer;

#[cfg(feature = "terminal-native-renderer")]
pub use atlas::{GlyphAtlas, GlyphAtlasEntry, GlyphAtlasKey};
pub use native_surface::{NativeTerminalSurface, NativeTerminalSurfaceRect};
#[cfg(feature = "terminal-native-renderer")]
pub use wgpu_renderer::{PreparedNativeFrame, ShapedTerminalFrame, WgpuTerminalRenderer};
