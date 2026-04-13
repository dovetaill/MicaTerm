//! Shared platform backend contract for native terminal surface hosting.

use anyhow::Result;
use slint::Image;

use crate::AppWindow;
use crate::app::terminal_presenter::NativeTerminalFrame;
use crate::app::terminal_renderer::{NativeSurfaceDamage, NativeTerminalSurfaceDiagnostics};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NativeTerminalSurfaceRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetainedNativeTerminalSurfaceFrame {
    pub frame: NativeTerminalFrame,
    pub rect: NativeTerminalSurfaceRect,
}

pub trait PlatformNativeSurfaceBackend {
    fn attach(&mut self, window: &AppWindow) -> Result<()>;
    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect);
    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>);
    fn present(&mut self, damage: NativeSurfaceDamage);
    fn host_image_snapshot(&self) -> Option<Image>;
    fn diagnostics_snapshot(&self) -> NativeTerminalSurfaceDiagnostics;
    fn detach(&mut self);
}
