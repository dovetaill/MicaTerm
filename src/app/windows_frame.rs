//! Windows-specific non-client frame interop for hit-testing, snap layouts, and placement queries.

use crate::AppWindow;
#[cfg(target_os = "windows")]
use crate::app::window_geometry::MonitorWorkArea;
use crate::app::terminal_renderer::NativeTerminalSurfaceDiagnostics;
use crate::app::terminal_renderer::diagnostics::NativeTerminalSurfaceGlyphBoundsTrace;
use crate::app::window_state::WindowPlacementKind;
#[cfg(target_os = "windows")]
use crate::app::window_state::{Rect, classify_window_placement};
#[cfg(target_os = "windows")]
use slint::ComponentHandle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptionButtonGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl CaptionButtonGeometry {
    pub fn contains_window_point(self, x: i32, y: i32) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowFrameState {
    maximize_button: CaptionButtonGeometry,
    native_maximize_button_hit_test: bool,
    reserved_resize_band: i32,
}

pub fn uses_native_maximize_button_hit_test(placement: WindowPlacementKind) -> bool {
    let _ = placement;
    false
}

pub fn resolve_host_window_hwnd(window: &AppWindow) -> Option<isize> {
    #[cfg(target_os = "windows")]
    {
        use slint::winit_030::WinitWindowAccessor;

        let mut hwnd = None;
        let _ = window.window().with_winit_window(|winit_window| {
            hwnd = hwnd_from_winit_window(winit_window).map(|value| value as isize);
        });
        hwnd
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = window;
        None
    }
}

pub fn native_surface_diagnostics_hwnd(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<isize> {
    diagnostics
        .host_surface_hwnd
        .or(diagnostics.hwnd)
        .or(diagnostics.host_hwnd)
}

pub fn native_surface_host_hwnd(diagnostics: &NativeTerminalSurfaceDiagnostics) -> Option<isize> {
    diagnostics.host_hwnd
}

pub fn native_surface_host_surface_hwnd(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<isize> {
    diagnostics.host_surface_hwnd
}

pub fn native_surface_host_surface_visible(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<bool> {
    diagnostics.host_surface_visible
}

pub fn native_surface_host_surface_ready(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<bool> {
    diagnostics.host_surface_ready
}

pub fn native_surface_text_renderer_path(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics.text_renderer_path
}

pub fn native_surface_text_antialias_mode(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.text_antialias_mode)
}

pub fn native_surface_render_target_alpha_mode(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.render_target_alpha_mode)
}

pub fn native_surface_rendering_params_source(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.rendering_params_source)
}

pub fn native_surface_rendering_mode(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.rendering_mode)
}

pub fn native_surface_pixel_geometry(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.pixel_geometry)
}

pub fn native_surface_gamma_per_mille(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.gamma_per_mille)
}

pub fn native_surface_enhanced_contrast_per_mille(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.enhanced_contrast_per_mille)
}

pub fn native_surface_text_fallback_reason(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.fallback_reason)
}

pub fn native_surface_clear_type_level_per_mille(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.clear_type_level_per_mille)
}

pub fn native_surface_font_chain(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&[String]> {
    diagnostics
        .windows_text
        .as_ref()
        .map(|text| text.font_chain.as_slice())
}

pub fn native_surface_baseline_px(diagnostics: &NativeTerminalSurfaceDiagnostics) -> Option<i32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.baseline_px)
}

pub fn native_surface_pixel_alignment(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&'static str> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.pixel_alignment)
}

pub fn native_surface_scale_factor_percent(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.scale_factor_percent)
}

pub fn native_surface_dpi_x(diagnostics: &NativeTerminalSurfaceDiagnostics) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.dpi_x)
}

pub fn native_surface_dpi_y(diagnostics: &NativeTerminalSurfaceDiagnostics) -> Option<u32> {
    diagnostics
        .windows_text
        .as_ref()
        .and_then(|text| text.dpi_y)
}

pub fn native_surface_glyph_bounds_trace(
    diagnostics: &NativeTerminalSurfaceDiagnostics,
) -> Option<&[NativeTerminalSurfaceGlyphBoundsTrace]> {
    diagnostics
        .windows_text
        .as_ref()
        .map(|text| text.glyph_bounds.as_slice())
}

pub fn native_surface_is_attached(diagnostics: &NativeTerminalSurfaceDiagnostics) -> bool {
    diagnostics.hwnd.is_some()
        || diagnostics.host_surface_hwnd.is_some()
        || diagnostics.host_hwnd.is_some()
}

pub fn point_hits_outer_resize_band(
    x: i32,
    y: i32,
    window_width: i32,
    window_height: i32,
    band: i32,
) -> bool {
    if band <= 0 {
        return false;
    }

    x < band
        || y < band
        || x >= window_width.saturating_sub(band)
        || y >= window_height.saturating_sub(band)
}

#[cfg(target_os = "windows")]
fn hwnd_from_winit_window(
    window: &slint::winit_030::winit::window::Window,
) -> Option<windows_sys::Win32::Foundation::HWND> {
    use slint::winit_030::winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = window.window_handle().ok()?;

    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => {
            Some(handle.hwnd.get() as windows_sys::Win32::Foundation::HWND)
        }
        _ => None,
    }
}

#[cfg(target_os = "windows")]
fn rect_from_win32_rect(rect: windows_sys::Win32::Foundation::RECT) -> Option<Rect> {
    let width = u32::try_from(rect.right - rect.left).ok()?;
    let height = u32::try_from(rect.bottom - rect.top).ok()?;
    Some(Rect::new(rect.left, rect.top, width, height))
}

#[cfg(target_os = "windows")]
pub fn work_area_from_hmonitor(
    hmonitor: slint::winit_030::winit::platform::windows::HMONITOR,
) -> Option<MonitorWorkArea> {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};

    unsafe {
        let mut monitor_info = MONITORINFO {
            cbSize: core::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: core::mem::zeroed(),
            rcWork: core::mem::zeroed(),
            dwFlags: 0,
        };
        if GetMonitorInfoW(
            hmonitor as windows_sys::Win32::Graphics::Gdi::HMONITOR,
            &mut monitor_info,
        ) == 0
        {
            return None;
        }

        let work = rect_from_win32_rect(monitor_info.rcWork)?;
        Some(MonitorWorkArea::new(work.x, work.y, work.width, work.height))
    }
}

#[cfg(target_os = "windows")]
const WINDOW_FRAME_SUBCLASS_ID: usize = 0x4D_54_57_46;

#[cfg(target_os = "windows")]
fn window_frame_property_name() -> Vec<u16> {
    "MicaTermWindowFrameGeometry\0".encode_utf16().collect()
}

#[cfg(target_os = "windows")]
fn screen_point_from_lparam(lparam: windows_sys::Win32::Foundation::LPARAM) -> (i32, i32) {
    let packed = lparam as u32;
    let x = (packed & 0xffff) as u16 as i16 as i32;
    let y = ((packed >> 16) & 0xffff) as u16 as i16 as i32;
    (x, y)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn window_frame_subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    umsg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::Foundation::{LRESULT, RECT};
    use windows_sys::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetPropW, GetWindowRect, HTCLIENT, HTMAXBUTTON, RemovePropW, WM_NCDESTROY, WM_NCHITTEST,
    };

    // Intercept non-client hit testing so the custom maximize button still triggers Windows-native
    // maximize behavior and snap layouts.
    if umsg == WM_NCHITTEST {
        let result = unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) };
        if result != HTCLIENT as LRESULT {
            return result;
        }

        let property_name = window_frame_property_name();
        let Some(frame_state) = (unsafe {
            (GetPropW(hwnd, property_name.as_ptr()) as *const WindowFrameState).as_ref()
        }) else {
            return result;
        };

        if !frame_state.native_maximize_button_hit_test {
            return result;
        }

        let mut window_rect: RECT = unsafe { core::mem::zeroed() };
        if unsafe { GetWindowRect(hwnd, &mut window_rect) } == 0 {
            return result;
        }

        let (screen_x, screen_y) = screen_point_from_lparam(lparam);
        let window_x = screen_x - window_rect.left;
        let window_y = screen_y - window_rect.top;
        let window_width = window_rect.right.saturating_sub(window_rect.left);
        let window_height = window_rect.bottom.saturating_sub(window_rect.top);

        // Keep the reserved resize band for explicit edge grips out of the maximize hit-test path.
        if point_hits_outer_resize_band(
            window_x,
            window_y,
            window_width,
            window_height,
            frame_state.reserved_resize_band,
        ) {
            return result;
        }

        if frame_state
            .maximize_button
            .contains_window_point(window_x, window_y)
        {
            return HTMAXBUTTON as LRESULT;
        }

        return result;
    }

    let result = unsafe { DefSubclassProc(hwnd, umsg, wparam, lparam) };

    if umsg == WM_NCDESTROY {
        unsafe {
            let property_name = window_frame_property_name();
            let geometry = RemovePropW(hwnd, property_name.as_ptr());
            RemoveWindowSubclass(
                hwnd,
                Some(window_frame_subclass_proc),
                WINDOW_FRAME_SUBCLASS_ID,
            );
            if !geometry.is_null() {
                drop(Box::from_raw(geometry as *mut WindowFrameState));
            }
        }
    }

    result
}

#[cfg(target_os = "windows")]
pub fn query_true_window_placement(
    window: &slint::winit_030::winit::window::Window,
) -> Option<WindowPlacementKind> {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowPlacement, GetWindowRect, SW_SHOWMAXIMIZED, WINDOWPLACEMENT,
    };

    let hwnd = hwnd_from_winit_window(window)?;

    unsafe {
        let mut placement = WINDOWPLACEMENT {
            length: core::mem::size_of::<WINDOWPLACEMENT>() as u32,
            flags: 0,
            showCmd: 0,
            ptMinPosition: core::mem::zeroed(),
            ptMaxPosition: core::mem::zeroed(),
            rcNormalPosition: core::mem::zeroed(),
        };
        if GetWindowPlacement(hwnd, &mut placement) == 0 {
            return None;
        }

        let mut window_rect: RECT = core::mem::zeroed();
        if GetWindowRect(hwnd, &mut window_rect) == 0 {
            return None;
        }

        let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return None;
        }

        let mut monitor_info = MONITORINFO {
            cbSize: core::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: core::mem::zeroed(),
            rcWork: core::mem::zeroed(),
            dwFlags: 0,
        };
        if GetMonitorInfoW(monitor, &mut monitor_info) == 0 {
            return None;
        }

        let window_rect = rect_from_win32_rect(window_rect)?;
        let work_area = rect_from_win32_rect(monitor_info.rcWork)?;

        Some(classify_window_placement(
            window_rect,
            work_area,
            placement.showCmd == SW_SHOWMAXIMIZED as u32,
        ))
    }
}

#[cfg(not(target_os = "windows"))]
pub fn query_true_window_placement(
    _window: &slint::winit_030::winit::window::Window,
) -> Option<WindowPlacementKind> {
    None
}

#[cfg(target_os = "windows")]
pub fn install_window_frame_adapter(
    window: &slint::winit_030::winit::window::Window,
    maximize_button: CaptionButtonGeometry,
    placement: WindowPlacementKind,
    reserved_resize_band: i32,
) {
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetPropW, RemovePropW, SetPropW};

    let Some(hwnd) = hwnd_from_winit_window(window) else {
        return;
    };

    ensure_window_clips_child_surfaces(hwnd);

    unsafe {
        // Reuse the same property-backed state across updates so geometry changes do not require
        // re-registering the subclass each time the titlebar layout shifts.
        let property_name = window_frame_property_name();
        let frame_state = GetPropW(hwnd, property_name.as_ptr()) as *mut WindowFrameState;
        let created_geometry = if let Some(frame_state) = frame_state.as_mut() {
            *frame_state = WindowFrameState {
                maximize_button,
                native_maximize_button_hit_test: uses_native_maximize_button_hit_test(placement),
                reserved_resize_band,
            };
            false
        } else {
            let frame_state = Box::into_raw(Box::new(WindowFrameState {
                maximize_button,
                native_maximize_button_hit_test: uses_native_maximize_button_hit_test(placement),
                reserved_resize_band,
            }));
            if SetPropW(hwnd, property_name.as_ptr(), frame_state.cast()) == 0 {
                drop(Box::from_raw(frame_state));
                return;
            }
            true
        };

        if SetWindowSubclass(
            hwnd,
            Some(window_frame_subclass_proc),
            WINDOW_FRAME_SUBCLASS_ID,
            0,
        ) == 0
            && created_geometry
        {
            let geometry = RemovePropW(hwnd, property_name.as_ptr());
            if !geometry.is_null() {
                drop(Box::from_raw(geometry as *mut WindowFrameState));
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn ensure_window_clips_child_surfaces(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
        SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_CLIPCHILDREN,
    };

    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        if style & WS_CLIPCHILDREN != 0 {
            return;
        }

        let _ = SetWindowLongPtrW(hwnd, GWL_STYLE, (style | WS_CLIPCHILDREN) as isize);
        let _ = SetWindowPos(
            hwnd,
            core::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

#[cfg(not(target_os = "windows"))]
pub fn install_window_frame_adapter(
    _window: &slint::winit_030::winit::window::Window,
    _maximize_button: CaptionButtonGeometry,
    _placement: WindowPlacementKind,
    _reserved_resize_band: i32,
) {
}
