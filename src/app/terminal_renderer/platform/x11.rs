//! X11 native surface backend scaffold.

use anyhow::Result;

use crate::AppWindow;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct X11NativeSurfaceState {
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub last_presented_frame_token: u64,
}

#[derive(Debug, Default)]
pub struct X11NativeSurfaceBackend {
    state: X11NativeSurfaceState,
}

impl PlatformNativeSurfaceBackend for X11NativeSurfaceBackend {
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
        self.state = X11NativeSurfaceState::default();
    }
}
