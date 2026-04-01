//! Windows native surface backend scaffold.

use anyhow::Result;

use crate::AppWindow;
use crate::app::windows_frame::resolve_host_window_hwnd;

use super::backend::{
    NativeTerminalSurfaceRect, PlatformNativeSurfaceBackend, RetainedNativeTerminalSurfaceFrame,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsNativeSurfaceState {
    pub hwnd: Option<isize>,
    pub rect: NativeTerminalSurfaceRect,
    pub retained_frame: Option<RetainedNativeTerminalSurfaceFrame>,
    pub last_presented_frame_token: u64,
}

#[derive(Debug, Default)]
pub struct WindowsNativeSurfaceBackend {
    state: WindowsNativeSurfaceState,
}

impl WindowsNativeSurfaceBackend {
    fn resolve_host_hwnd(window: &AppWindow) -> Option<isize> {
        resolve_host_window_hwnd(window)
    }
}

impl PlatformNativeSurfaceBackend for WindowsNativeSurfaceBackend {
    fn attach(&mut self, window: &AppWindow) -> Result<()> {
        self.state.hwnd = Self::resolve_host_hwnd(window);
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
        self.state = WindowsNativeSurfaceState::default();
    }
}
