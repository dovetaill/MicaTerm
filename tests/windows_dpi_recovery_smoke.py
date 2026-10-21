"""Exercise production DPI/focus handlers with simulated desktop transitions."""

from windows_drag_recovery_smoke import EVENT_LOOP, ROOT, run_harness


def main():
    source = EVENT_LOOP.read_text().split("WM_DPICHANGED => {", 1)[1]
    geometry = source.split("            let new_outer_rect: RECT;", 1)[1]
    geometry = geometry.split("\n            unsafe {\n                SetWindowPos(", 1)[0]
    source = (ROOT / "vendor/i-slint-backend-winit/event_loop.rs").read_text()
    focus = source.split("            WindowEvent::Focused(have_focus) => {", 1)[1]
    focus = focus.split("\n            WindowEvent::KeyboardInput", 1)[0]
    scale = source.split("            WindowEvent::ScaleFactorChanged { scale_factor, inner_size_writer: _ } => {", 1)[1]
    scale = scale.split("\n            WindowEvent::ThemeChanged", 1)[0]
    # Compile the Windows-only branches on the host; OS operations below are test doubles.
    focus = focus.replace('#[cfg(target_os = "windows")]', '')
    scale = scale.replace('#[cfg(target_os = "windows")]', '')
    run_harness(r'''
#![allow(non_snake_case, non_camel_case_types, dead_code, unused_imports)]
use std::{cell::Cell, mem};
#[derive(Clone, Copy, Debug, PartialEq)]
struct Size { width: u32, height: u32 }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct RECT { left: i32, top: i32, right: i32, bottom: i32 }
#[derive(Default)]
struct POINT { x: i32, y: i32 }
#[derive(Default)]
struct MONITORINFO { cbSize: u32, rcMonitor: RECT }
const MONITOR_DEFAULTTONULL: u32 = 0;
thread_local! { static BUILD: Cell<Option<u32>> = const { Cell::new(None) }; }
mod dark_mode {
    pub struct Build;
    pub static WIN10_BUILD_VERSION: Build = Build;
    impl Build {
        pub fn is_some_and(&self, f: impl FnOnce(u32) -> bool) -> bool {
            super::BUILD.get().is_some_and(f)
        }
    }
}
struct WindowFlags;
impl WindowFlags {
    fn adjust_rect(&self, _: isize, rect: RECT) -> Result<RECT, ()> { Ok(rect) }
}
unsafe fn GetCursorPos(_: *mut POINT) -> i32 { panic!("unexpected cursor correction") }
unsafe fn MonitorFromWindow(_: isize, _: u32) -> isize { 1 }
unsafe fn MonitorFromRect(_: *const RECT, _: u32) -> isize { 1 }
unsafe fn GetMonitorInfoW(_: isize, _: *mut MONITORINFO) -> i32 { panic!("unexpected monitor nudge") }
mod dpi {
    use super::*;
    pub fn placement(build: Option<u32>, suggested_rect: RECT, requested_inner_size: Size) -> RECT {
        BUILD.set(build);
        let new_physical_inner_size = Size { width: 1200, height: 800 };
        let (window, margin_left, margin_top, dragging_window) = (1, 0, 0, false);
        let window_flags = WindowFlags;
        let new_outer_rect: RECT;
        __GEOMETRY__
        new_outer_rect
    }
}
#[test]
fn modern_windows_keeps_os_rect_across_scale_and_screen_changes() {
    for build in [22000, 26100] {
        for rect in [
            RECT { left: 0, top: 0, right: 960, bottom: 1040 },
            RECT { left: -1920, top: 40, right: -420, bottom: 1040 },
            RECT { left: 0, top: -1080, right: 1920, bottom: 0 },
        ] {
            assert_eq!(dpi::placement(Some(build), rect, Size { width: 1200, height: 800 }), rect);
        }
    }
}
#[test]
fn older_windows_and_explicit_size_requests_keep_their_size() {
    let rect = RECT { left: -1920, top: 20, right: -320, bottom: 1000 };
    for build in [None, Some(19045)] {
        let placed = dpi::placement(build, rect, Size { width: 1200, height: 800 });
        assert_eq!((placed.right - placed.left, placed.bottom - placed.top), (1200, 800));
    }
    let placed = dpi::placement(Some(26100), rect, Size { width: 700, height: 500 });
    assert_eq!((placed.left, placed.top), (-1920, 20));
    assert_eq!((placed.right - placed.left, placed.bottom - placed.top), (700, 500));
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Point { x: f32, y: f32 }
impl std::ops::Mul<f32> for Point {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self { Self { x: self.x * rhs, y: self.y * rhs } }
}
enum MouseEvent { Exit }
mod corelib { pub mod platform { pub enum WindowEvent { ScaleFactorChanged { scale_factor: f32 } } } }
struct Runtime {
    scale: Cell<f32>, grabbed: Cell<bool>, logical_size: Cell<(f32, f32)>,
    active: Cell<bool>, fail_scale: Cell<bool>,
}
impl Runtime {
    fn scale_factor(&self) -> f32 { self.scale.get() }
    fn process_mouse_input(&self, _: MouseEvent) { self.grabbed.set(false); }
    fn try_dispatch_event(&self, event: corelib::platform::WindowEvent) -> Result<(), ()> {
        if self.fail_scale.get() { return Err(()); }
        let corelib::platform::WindowEvent::ScaleFactorChanged { scale_factor } = event;
        self.scale.set(scale_factor);
        Ok(())
    }
}
struct NativeWindow;
impl NativeWindow {
    fn has_focus(&self) -> bool { true }
    fn inner_size(&self) -> Size { Size { width: 1600, height: 900 } }
}
struct Window { runtime: Runtime, resize_count: Cell<usize> }
impl Window {
    fn window(&self) -> &Runtime { &self.runtime }
    fn winit_window(&self) -> Option<NativeWindow> { Some(NativeWindow) }
    fn resize_event(&self, size: Size) -> Result<(), ()> {
        self.resize_count.set(self.resize_count.get() + 1);
        let scale = self.runtime.scale.get();
        self.runtime.logical_size.set((size.width as f32 / scale, size.height as f32 / scale));
        Ok(())
    }
    fn activation_changed(&self, active: bool) -> Result<(), ()> { self.runtime.active.set(active); Ok(()) }
    fn handle_partial_visibility_change(&self) -> Result<(), ()> { Ok(()) }
}
struct Backend {
    cursor_pos: Point, current_resize_direction: Option<()>, current_touch_id: Option<u64>,
    pressed: bool, loop_error: Option<()>,
}
impl Backend {
    fn focus(&mut self, window: &Window, have_focus: bool) {
        let runtime_window = window.window();
        __FOCUS__
    fn scale(&mut self, window: &Window, scale_factor: f64) {
        let runtime_window = window.window();
        __SCALE__
}
fn setup() -> (Window, Backend) {
    (Window { runtime: Runtime {
        scale: Cell::new(2.), grabbed: Cell::new(true), logical_size: Cell::new((800., 450.)),
        active: Cell::new(true), fail_scale: Cell::new(false),
    }, resize_count: Cell::new(0) }, Backend {
        cursor_pos: Point { x: 400., y: 100. }, current_resize_direction: Some(()),
        current_touch_id: Some(42), pressed: true, loop_error: None,
    })
}
#[test]
fn reconnect_cancels_old_press_and_dpi_change_resyncs_layout_without_resize_message() {
    let (window, mut backend) = setup();
    backend.focus(&window, false);
    assert!(!backend.pressed && !window.runtime.grabbed.get());
    assert!(backend.current_touch_id.is_none() && backend.current_resize_direction.is_none());
    backend.scale(&window, 1.);
    assert_eq!(backend.cursor_pos, Point { x: 800., y: 200. });
    assert_eq!(window.runtime.logical_size.get(), (1600., 900.));
    backend.focus(&window, true);
    assert!(window.runtime.active.get());
    // A second client with fractional scaling must use the same physical pointer.
    backend.scale(&window, 1.25);
    assert_eq!(backend.cursor_pos, Point { x: 640., y: 160. });
    assert_eq!(window.runtime.logical_size.get(), (1280., 720.));
    backend.scale(&window, 2.);
    assert_eq!(backend.cursor_pos, Point { x: 400., y: 100. });
    assert_eq!(window.runtime.logical_size.get(), (800., 450.));
    assert_eq!(window.resize_count.get(), 3);
}
#[test]
fn scale_dispatch_failure_preserves_error_and_does_not_resize() {
    let (window, mut backend) = setup();
    window.runtime.fail_scale.set(true);
    backend.scale(&window, 1.);
    assert!(backend.loop_error.is_some());
    assert_eq!(window.resize_count.get(), 0);
    assert_eq!(backend.cursor_pos, Point { x: 400., y: 100. });
}
'''.replace("__GEOMETRY__", geometry).replace("__FOCUS__", focus).replace("__SCALE__", scale))


if __name__ == "__main__":
    main()
