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
    fn update_fallback_paint_state(&self, rgba: Option<u32>, enabled: Option<bool>) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::Graphics::Gdi::InvalidateRect;
        use windows::Win32::UI::WindowsAndMessaging::{
            GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
        };

        if self.surface_hwnd == 0 {
            return;
        }

        let hwnd = HWND(self.surface_hwnd as _);
        let stored = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
        let current_state = u64::try_from(stored.max(0)).unwrap_or_default();
        let current_rgba = retained_native_child_host_background_rgba(hwnd);
        let current_enabled = retained_native_child_host_fallback_paint_enabled(hwnd);
        let next_rgba = rgba.map(opaque_background_rgba).unwrap_or(current_rgba);
        let next_enabled = enabled.unwrap_or(current_enabled);
        let next_state = encode_retained_native_child_host_state(next_rgba, next_enabled);
        if next_state == current_state {
            return;
        }

        unsafe {
            let _ = SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                isize::try_from(next_state).unwrap_or_default(),
            );
            if current_enabled != next_enabled || (next_enabled && current_rgba != next_rgba) {
                let _ = InvalidateRect(hwnd, None, false);
            }
        }
    }

    #[cfg(target_os = "windows")]
    pub fn create(parent_hwnd: isize, rect: NativeTerminalSurfaceRect) -> Result<Self> {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, HMENU, SW_SHOWNA, ShowWindow, WINDOW_EX_STYLE, WS_CHILD,
            WS_CLIPCHILDREN, WS_CLIPSIBLINGS, WS_DISABLED, WS_VISIBLE,
        };
        use windows::core::{PCWSTR, w};

        ensure_retained_native_child_host_class()?;
        let instance = retained_native_child_host_instance()?;
        let surface_hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("MicaTermRetainedNativeChildHost"),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS | WS_DISABLED | WS_VISIBLE,
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
    pub fn set_background_rgba(&self, rgba: u32) {
        self.update_fallback_paint_state(Some(rgba), None);
    }

    #[cfg(not(target_os = "windows"))]
    pub fn set_background_rgba(&self, _rgba: u32) {}

    #[cfg(target_os = "windows")]
    pub fn set_fallback_paint_enabled(&self, enabled: bool) {
        self.update_fallback_paint_state(None, Some(enabled));
    }

    #[cfg(not(target_os = "windows"))]
    pub fn set_fallback_paint_enabled(&self, _enabled: bool) {}

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
            anyhow::bail!("failed to register retained-native child host window class: {error:?}");
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
    use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, HDC, PAINTSTRUCT};
    use windows::Win32::UI::WindowsAndMessaging::{
        DefWindowProcW, HTTRANSPARENT, MA_NOACTIVATE, WM_ERASEBKGND, WM_MOUSEACTIVATE,
        WM_NCHITTEST, WM_PAINT,
    };

    match msg {
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as _);
            if retained_native_child_host_fallback_paint_enabled(hwnd) && hdc.0 as usize != 0 {
                paint_retained_native_child_host_background(hwnd, hdc);
            }
            LRESULT(1)
        }
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            unsafe {
                let hdc = BeginPaint(hwnd, &mut paint);
                if retained_native_child_host_fallback_paint_enabled(hwnd) {
                    paint_retained_native_child_host_background(hwnd, hdc);
                }
                let _ = EndPaint(hwnd, &paint);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(target_os = "windows")]
fn retained_native_child_host_background_rgba(hwnd: windows::Win32::Foundation::HWND) -> u32 {
    use windows::Win32::UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW};

    let stored = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    let stored = u64::try_from(stored.max(0)).unwrap_or_default();
    let stored = u32::try_from(stored >> 1).unwrap_or_default();
    if stored == 0 {
        0xff11_1821
    } else {
        opaque_background_rgba(stored)
    }
}

#[cfg(target_os = "windows")]
fn retained_native_child_host_fallback_paint_enabled(
    hwnd: windows::Win32::Foundation::HWND,
) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::{GWLP_USERDATA, GetWindowLongPtrW};

    let stored = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    let stored = u64::try_from(stored.max(0)).unwrap_or_default();
    if stored == 0 { true } else { stored & 1 != 0 }
}

#[cfg(target_os = "windows")]
fn opaque_background_rgba(rgba: u32) -> u32 {
    0xff00_0000 | (rgba & 0x00ff_ffff)
}

#[cfg(target_os = "windows")]
fn encode_retained_native_child_host_state(rgba: u32, enabled: bool) -> u64 {
    (u64::from(opaque_background_rgba(rgba)) << 1) | u64::from(enabled)
}

#[cfg(target_os = "windows")]
fn paint_retained_native_child_host_background(
    hwnd: windows::Win32::Foundation::HWND,
    hdc: windows::Win32::Graphics::Gdi::HDC,
) {
    use windows::Win32::Foundation::{COLORREF, RECT};
    use windows::Win32::Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect};
    use windows::Win32::UI::WindowsAndMessaging::GetClientRect;

    let mut rect = RECT::default();
    if unsafe { GetClientRect(hwnd, &mut rect) }.is_err() {
        return;
    }

    let rgba = retained_native_child_host_background_rgba(hwnd);
    let red = rgba >> 16 & 0xff;
    let green = rgba >> 8 & 0xff;
    let blue = rgba & 0xff;
    let brush = unsafe { CreateSolidBrush(COLORREF(red | (green << 8) | (blue << 16))) };
    if brush.0.is_null() {
        return;
    }

    unsafe {
        let _ = FillRect(hdc, &rect, brush);
        let _ = DeleteObject(brush);
    }
}
