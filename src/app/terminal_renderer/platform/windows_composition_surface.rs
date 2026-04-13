//! Thin host-attachment helper kept only for Windows native-surface lifecycle state.

use anyhow::Result;

use super::backend::NativeTerminalSurfaceRect;

#[derive(Clone, Debug, Default)]
pub struct WindowsCompositionSurfaceHost {
    pub host_hwnd: isize,
    pub attached: bool,
    pub rect: NativeTerminalSurfaceRect,
}

impl WindowsCompositionSurfaceHost {
    pub fn create(host_hwnd: isize, rect: NativeTerminalSurfaceRect) -> Result<Self> {
        Ok(Self {
            host_hwnd,
            attached: rect.width > 0 && rect.height > 0,
            rect,
        })
    }

    pub fn sync_rect(&mut self, rect: NativeTerminalSurfaceRect) -> Result<()> {
        self.rect = rect;
        self.attached = rect.width > 0 && rect.height > 0;
        Ok(())
    }

    pub fn set_visible(&mut self, visible: bool) {
        self.attached = visible && self.rect.width > 0 && self.rect.height > 0;
    }

    pub fn is_visible(&self) -> bool {
        self.attached && self.rect.width > 0 && self.rect.height > 0
    }

    pub fn surface_hwnd(&self) -> isize {
        self.host_hwnd
    }

    pub fn destroy(&mut self) {
        self.attached = false;
        self.rect = NativeTerminalSurfaceRect::default();
    }
}
