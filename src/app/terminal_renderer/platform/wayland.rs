//! Wayland native surface backend scaffold.

use anyhow::Result;

use crate::AppWindow;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WaylandNativeSurfaceState {
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub last_presented_frame_token: u64,
}

#[derive(Debug, Default)]
pub struct WaylandNativeSurfaceBackend {
    state: WaylandNativeSurfaceState,
}

impl PlatformNativeSurfaceBackend for WaylandNativeSurfaceBackend {
    fn attach(&mut self, _window: &AppWindow) -> Result<()> {
        Ok(())
    }

    fn update_surface_rect(&mut self, rect: NativeTerminalSurfaceRect) {
        self.state.rect = rect;
    }

    fn update_frame(&mut self, frame: Option<RetainedNativeTerminalSurfaceFrame>) {
        self.state.retained_frame = frame;
    }

    fn present(&mut self) {
        if let Some(frame) = self.state.retained_frame.as_ref() {
            self.state.last_presented_frame_token = frame.frame.frame_token;
        }
    }

    fn detach(&mut self) {
        self.state = WaylandNativeSurfaceState::default();
    }
}
