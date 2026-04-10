//! Win32 child-HWND host used by the retained native terminal renderer.

use anyhow::Result;

use super::backend::NativeTerminalSurfaceRect;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WindowsChildSurfaceHost {
    pub parent_hwnd: isize,
    pub surface_hwnd: isize,
}

impl WindowsChildSurfaceHost {
    #[cfg(target_os = "windows")]
    pub fn create(parent_hwnd: isize, rect: NativeTerminalSurfaceRect) -> Result<Self> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, SW_SHOWNA, ShowWindow, WINDOW_EX_STYLE, WS_CHILD, WS_CLIPCHILDREN,
            WS_CLIPSIBLINGS, WS_VISIBLE,
        };
        use windows::core::PCWSTR;
        use windows::w;

        let surface_hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("STATIC"),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE,
                rect.x,
                rect.y,
                rect.width.max(1),
                rect.height.max(1),
                Some(HWND(parent_hwnd as _)),
                None,
                None,
                None,
            )?
        };

        unsafe {
            ShowWindow(surface_hwnd, SW_SHOWNA);
        }

        Ok(Self {
            parent_hwnd,
            surface_hwnd: surface_hwnd.0 as isize,
        })
    }

    #[cfg(not(target_os = "windows"))]
    pub fn create(parent_hwnd: isize, _rect: NativeTerminalSurfaceRect) -> Result<Self> {
        Ok(Self {
            parent_hwnd,
            surface_hwnd: 0,
        })
    }

    #[cfg(target_os = "windows")]
    pub fn sync_rect(&self, rect: NativeTerminalSurfaceRect) -> Result<()> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos,
        };

        unsafe {
            SetWindowPos(
                HWND(self.surface_hwnd as _),
                None,
                rect.x,
                rect.y,
                rect.width.max(1),
                rect.height.max(1),
                SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_NOZORDER | SWP_SHOWWINDOW,
            )?;
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    pub fn sync_rect(&self, _rect: NativeTerminalSurfaceRect) -> Result<()> {
        Ok(())
    }

    #[cfg(target_os = "windows")]
    pub fn set_visible(&self, visible: bool) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{SW_HIDE, SW_SHOWNA, ShowWindow};

        unsafe {
            ShowWindow(
                HWND(self.surface_hwnd as _),
                if visible { SW_SHOWNA } else { SW_HIDE },
            );
        }
    }

    #[cfg(not(target_os = "windows"))]
    pub fn set_visible(&self, _visible: bool) {}

    #[cfg(target_os = "windows")]
    pub fn destroy(&mut self) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::DestroyWindow;

        if self.surface_hwnd == 0 {
            return;
        }

        let _ = unsafe { DestroyWindow(HWND(self.surface_hwnd as _)) };
        self.surface_hwnd = 0;
    }

    #[cfg(not(target_os = "windows"))]
    pub fn destroy(&mut self) {
        self.surface_hwnd = 0;
    }
}
