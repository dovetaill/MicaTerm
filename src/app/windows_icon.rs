use crate::AppWindow;

#[cfg(target_os = "windows")]
pub fn log_window_icon_state(window: &AppWindow, phase: &str) {
    use crate::app::windows_frame::resolve_host_window_hwnd;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GCLP_HICON, GCLP_HICONSM, GetClassLongPtrW, ICON_BIG, ICON_SMALL, SendMessageW, WM_GETICON,
    };

    let Some(hwnd) = resolve_host_window_hwnd(window).map(|value| value as HWND) else {
        tracing::warn!(
            target: "app.windows_icon",
            phase,
            "unable to resolve host hwnd for windows icon diagnostics"
        );
        return;
    };

    let small_icon_handle = unsafe { SendMessageW(hwnd, WM_GETICON, ICON_SMALL as usize, 0) };
    let big_icon_handle = unsafe { SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0) };
    let class_small_icon_handle = unsafe { GetClassLongPtrW(hwnd, GCLP_HICONSM) };
    let class_big_icon_handle = unsafe { GetClassLongPtrW(hwnd, GCLP_HICON) };

    tracing::info!(
        target: "app.windows_icon",
        phase,
        hwnd = hwnd as usize,
        small_icon_handle = small_icon_handle as usize,
        big_icon_handle = big_icon_handle as usize,
        class_small_icon_handle = class_small_icon_handle as usize,
        class_big_icon_handle = class_big_icon_handle as usize,
        small_icon_present = small_icon_handle != 0 || class_small_icon_handle != 0,
        big_icon_present = big_icon_handle != 0 || class_big_icon_handle != 0,
        "captured windows icon handle state"
    );
}

#[cfg(not(target_os = "windows"))]
pub fn log_window_icon_state(_window: &AppWindow, _phase: &str) {}
