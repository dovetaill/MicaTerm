"""Run the actual Win32 drag code with mocked OS calls, without a Windows desktop."""

import os
from pathlib import Path
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
WINDOW = ROOT / "vendor/winit/src/platform_impl/windows/window.rs"
EVENT_LOOP = ROOT / "vendor/winit/src/platform_impl/windows/event_loop.rs"


def main():
    source = WINDOW.read_text()
    start = source.index("    unsafe fn handle_os_dragging(")
    drag = source[start:source.index("\n    #[inline]", start)]
    source = EVENT_LOOP.read_text().split("unsafe fn public_window_callback_inner(", 1)[1]
    source = source.split('unsafe extern "system" fn thread_event_target_callback(', 1)[0]
    completion = source[source.rindex("    let result = match result {"):].strip()

    harness = r'''
#![allow(non_snake_case, dead_code)]
use std::{cell::RefCell, mem, sync::{Arc, Mutex, MutexGuard}};
type LPARAM = isize;
type WPARAM = usize;
const WM_NCLBUTTONDOWN: u32 = 0x00a1;
const WM_LBUTTONUP: u32 = 0x0202;
const HTCAPTION: WPARAM = 2;
#[derive(Default)]
struct Point { x: i32, y: i32 }
#[derive(Default)]
struct WindowState { dragging: bool }
#[derive(Default)]
struct Native {
    position: (i32, i32),
    cursor_ok: bool,
    post_ok: bool,
    releases: usize,
    messages: Vec<(u32, WPARAM, LPARAM)>,
    normal_exit: bool,
    state: Option<Arc<Mutex<WindowState>>>,
}
thread_local! { static NATIVE: RefCell<Native> = RefCell::new(Native::default()); }
unsafe fn GetCursorPos(point: *mut Point) -> i32 {
    NATIVE.with_borrow(|n| {
        unsafe { *point = Point { x: n.position.0, y: n.position.1 }; }
        i32::from(n.cursor_ok)
    })
}
unsafe fn ReleaseCapture() {
    NATIVE.with_borrow_mut(|n| n.releases += 1);
}
unsafe fn PostMessageW(_: isize, msg: u32, wparam: WPARAM, lparam: LPARAM) -> i32 {
    NATIVE.with_borrow_mut(|n| {
        if n.post_ok { n.messages.push((msg, wparam, lparam)); }
        i32::from(n.post_ok)
    })
}
unsafe fn DefWindowProcW(_: isize, _: u32, _: WPARAM, _: LPARAM) -> isize {
    NATIVE.with_borrow_mut(|n| {
        if n.normal_exit {
            // Model the existing WM_EXITSIZEMOVE handler's release.
            n.state.as_ref().unwrap().lock().unwrap().dragging = false;
            n.messages.push((WM_LBUTTONUP, 0, 0));
        }
    });
    123
}
struct ThreadExecutor;
impl ThreadExecutor {
    fn execute_in_thread(&self, callback: impl FnOnce()) { callback(); }
}
struct Window {
    window: isize,
    window_state: Arc<Mutex<WindowState>>,
    thread_executor: ThreadExecutor,
}
impl Window {
    __DRAG__
}
struct WindowData(Arc<Mutex<WindowState>>);
impl WindowData {
    fn window_state_lock(&self) -> MutexGuard<'_, WindowState> { self.0.lock().unwrap() }
}
enum ProcResult { DefWindowProc(WPARAM), Value(isize) }
fn complete(userdata: &WindowData, msg: u32, result: ProcResult) -> isize {
    let (window, lparam) = (1, 0);
    __COMPLETION__

fn setup() -> (Window, WindowData) {
    let state = Arc::new(Mutex::new(WindowState::default()));
    NATIVE.with_borrow_mut(|n| *n = Native {
        cursor_ok: true, post_ok: true, state: Some(state.clone()), ..Native::default()
    });
    (Window { window: 1, window_state: state.clone(), thread_executor: ThreadExecutor }, WindowData(state))
}
#[test]
fn screen_coordinates_survive_packing_including_negative_monitors() {
    for (x, y) in [(0, 0), (1900, 30), (-1920, 40), (100, -1080), (-32768, 32767)] {
        let (window, _) = setup();
        NATIVE.with_borrow_mut(|n| n.position = (x, y));
        unsafe { window.handle_os_dragging(HTCAPTION); }
        NATIVE.with_borrow(|n| {
            assert_eq!(n.releases, 1);
            assert_eq!(n.messages.len(), 1);
            let (msg, hit, packed) = n.messages[0];
            assert_eq!((msg, hit), (WM_NCLBUTTONDOWN, HTCAPTION));
            assert_eq!((packed as u16 as i16 as i32, (packed >> 16) as u16 as i16 as i32), (x, y));
        });
    }
}
#[test]
fn rejected_native_drag_releases_press_and_allows_the_next_drag() {
    let (window, data) = setup();
    unsafe { window.handle_os_dragging(HTCAPTION); }
    assert_eq!(complete(&data, WM_NCLBUTTONDOWN, ProcResult::DefWindowProc(HTCAPTION)), 123);
    assert!(!data.window_state_lock().dragging);
    NATIVE.with_borrow(|n| assert_eq!(n.messages.last(), Some(&(WM_LBUTTONUP, 0, 0))));
    unsafe { window.handle_os_dragging(HTCAPTION); }
    assert!(data.window_state_lock().dragging);
    NATIVE.with_borrow(|n| assert_eq!(n.messages.len(), 3));
}
#[test]
fn normal_drag_does_not_release_twice() {
    let (window, data) = setup();
    unsafe { window.handle_os_dragging(HTCAPTION); }
    NATIVE.with_borrow_mut(|n| n.normal_exit = true);
    complete(&data, WM_NCLBUTTONDOWN, ProcResult::DefWindowProc(HTCAPTION));
    assert!(!data.window_state_lock().dragging);
    NATIVE.with_borrow(|n| assert_eq!(n.messages.iter().filter(|m| m.0 == WM_LBUTTONUP).count(), 1));
}
#[test]
fn cursor_or_post_failure_does_not_latch_dragging() {
    for cursor_ok in [false, true] {
        let (window, data) = setup();
        NATIVE.with_borrow_mut(|n| { n.cursor_ok = cursor_ok; n.post_ok = false; });
        unsafe { window.handle_os_dragging(HTCAPTION); }
        assert!(!data.window_state_lock().dragging);
        NATIVE.with_borrow(|n| {
            assert!(n.messages.is_empty());
            assert_eq!(n.releases, usize::from(cursor_ok));
        });
    }
}
#[test]
fn duplicate_request_and_unrelated_messages_preserve_active_drag() {
    let (window, data) = setup();
    unsafe { window.handle_os_dragging(HTCAPTION); window.handle_os_dragging(HTCAPTION); }
    assert_eq!(complete(&data, WM_LBUTTONUP, ProcResult::Value(7)), 7);
    assert!(data.window_state_lock().dragging);
    NATIVE.with_borrow(|n| { assert_eq!(n.messages.len(), 1); assert_eq!(n.releases, 1); });
}
'''.replace("__DRAG__", drag).replace("__COMPLETION__", completion)

    run_harness(harness)


def run_harness(harness):
    with tempfile.TemporaryDirectory(prefix="mica-windows-drag-") as directory:
        source = Path(directory) / "drag.rs"
        binary = Path(directory) / "drag-tests"
        source.write_text(harness)
        subprocess.run(
            [os.environ.get("RUSTC", "rustc"), "--edition=2021", "--test", str(source),
             "-C", "debuginfo=0", "-o", str(binary)], cwd=directory, check=True,
        )
        subprocess.run([str(binary)], cwd=directory, check=True)


if __name__ == "__main__":
    main()
