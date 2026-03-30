//! Terminal renderer integration seams for native surface presentation.

pub mod atlas;
pub mod native_surface;
pub mod wgpu_renderer;

pub use atlas::{GlyphAtlas, GlyphAtlasEntry, GlyphAtlasKey};
pub use native_surface::{NativeTerminalSurface, NativeTerminalSurfaceRect};
pub use wgpu_renderer::{PreparedNativeFrame, ShapedTerminalFrame, WgpuTerminalRenderer};
