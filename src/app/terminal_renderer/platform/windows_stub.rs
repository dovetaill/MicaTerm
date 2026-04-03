//! Stub Windows native surface backend used when the native terminal renderer feature is off.

use anyhow::Result;

use crate::AppWindow;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[derive(Default)]
pub struct WindowsNativeSurfaceBackend {
    rect: NativeTerminalSurfaceRect,
    frame: Option<RetainedNativeTerminalSurfaceFrame>,
}

impl PlatformNativeSurfaceBackend for WindowsNativeSurfaceBackend {
    fn attach(&mut self, _window: &AppWindow) -> Result<()> {
        Ok(())
    }

    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect) {
        self.rect = rect;
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        self.frame = frame;
    }

    fn present(&mut self) {}

    fn detach(&mut self) {
        self.frame = None;
        self.rect = NativeTerminalSurfaceRect::default();
    }
}
