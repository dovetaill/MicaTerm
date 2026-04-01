//! Terminal renderer integration seams for native-only surface presentation.

#[cfg(feature = "terminal-native-renderer")]
pub mod atlas;
pub mod native_surface;
pub mod platform;
#[cfg(feature = "terminal-native-renderer")]
pub mod wgpu_renderer;

#[cfg(feature = "terminal-native-renderer")]
pub use atlas::{GlyphAtlas, GlyphAtlasEntry, GlyphAtlasKey};
pub use native_surface::NativeTerminalSurface;
pub use platform::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
    WindowsNativeSurfaceBackend,
};
#[cfg(feature = "terminal-native-renderer")]
pub use wgpu_renderer::{PreparedNativeFrame, ShapedTerminalFrame, WgpuTerminalRenderer};
