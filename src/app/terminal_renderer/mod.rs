//! Terminal renderer integration seams for native-only surface presentation.

#[cfg(feature = "terminal-native-renderer")]
pub mod atlas;
pub mod custom_grid_glyphs;
pub mod damage;
pub mod diagnostics;
pub mod host;
pub mod native_surface;
pub mod platform;
pub mod present_driver;
#[cfg(feature = "terminal-native-renderer")]
pub mod wgpu_renderer;

#[cfg(feature = "terminal-native-renderer")]
pub use atlas::{GlyphAtlas, GlyphAtlasEntry, GlyphAtlasKey};
pub use damage::{NativeFrameDamageTracker, NativeSurfaceDamage, NativeSurfaceDamageKind};
pub use diagnostics::{NativeTerminalSurfaceDiagnostics, NativeTerminalSurfaceDrawCounters};
pub use host::TerminalRendererHost;
pub use host::TerminalRendererHostOptions;
pub use native_surface::NativeTerminalSurface;
pub use platform::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
    WindowsNativeSurfaceBackend,
};
pub use present_driver::{
    EventLoopPresentDriver, NativeSurfacePresentDriver, RenderingNotifierPresentDriver,
};
#[cfg(feature = "terminal-native-renderer")]
pub use wgpu_renderer::{PreparedNativeFrame, ShapedTerminalFrame, WgpuTerminalRenderer};
