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
            CreateWindowExW, HMENU, SW_SHOWNA, ShowWindow, WINDOW_EX_STYLE, WS_CHILD,
            WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_VISIBLE,
        };
        use windows::core::{PCWSTR, w};

        ensure_retained_native_child_host_class()?;
        let instance = retained_native_child_host_instance()?;
        let surface_hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("MicaTermRetainedNativeChildHost"),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_VISIBLE,
                rect.x,
                rect.y,
                rect.width.max(1),
                rect.height.max(1),
                HWND(parent_hwnd as _),
                HMENU::default(),
                instance,
                None,
            )?
        };

        unsafe {
            let _ = ShowWindow(surface_hwnd, SW_SHOWNA);
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
                HWND::default(),
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
            let _ = ShowWindow(
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

#[cfg(target_os = "windows")]
fn retained_native_child_host_instance() -> Result<windows::Win32::Foundation::HINSTANCE> {
    use windows::Win32::Foundation::HINSTANCE;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::core::PCWSTR;

    let module = unsafe { GetModuleHandleW(PCWSTR::null())? };
    Ok(HINSTANCE(module.0))
}

#[cfg(target_os = "windows")]
fn ensure_retained_native_child_host_class() -> Result<()> {
    use windows::Win32::Foundation::{ERROR_CLASS_ALREADY_EXISTS, GetLastError};
    use windows::Win32::UI::WindowsAndMessaging::{RegisterClassW, WNDCLASSW};
    use windows::core::w;

    let atom = unsafe {
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(retained_native_child_host_wndproc),
            hInstance: retained_native_child_host_instance()?,
            lpszClassName: w!("MicaTermRetainedNativeChildHost"),
            ..Default::default()
        })
    };
    if atom == 0 {
        let error = unsafe { GetLastError() };
        if error != ERROR_CLASS_ALREADY_EXISTS {
            anyhow::bail!(
                "failed to register retained-native child host window class: {error:?}"
            );
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
extern "system" fn retained_native_child_host_wndproc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::LRESULT;
    use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, HTTRANSPARENT, MA_NOACTIVATE, WM_ERASEBKGND, WM_MOUSEACTIVATE,
        WM_NCHITTEST, WM_PAINT,
    };

    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            unsafe {
                BeginPaint(hwnd, &mut paint);
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
