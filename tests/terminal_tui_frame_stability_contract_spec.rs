//! Source-level contract coverage for TUI frame-stability guards.

use std::fs;

#[test]
fn terminal_host_hides_scrollbar_during_alternate_screen_frames() {
    let content = fs::read_to_string("ui/shell/terminal-session-host.slint")
        .expect("read terminal session host");

    assert!(
        content.contains("!root.session-alternate-screen-active"),
        "terminal session host should suppress the local scrollbar while alternate-screen TUIs own the viewport so host width does not wobble during full-screen redraws"
    );
}

#[test]
fn terminal_host_ignores_single_line_scrollback_for_scrollbar_visibility() {
    let content = fs::read_to_string("ui/shell/terminal-session-host.slint")
        .expect("read terminal session host");

    assert!(
        content.contains("root.session-viewport-max-offset-lines > 1"),
        "terminal session host should avoid reserving scrollbar gutter for a transient one-line scrollback because that width wobble feeds back into PTY cols and makes main-screen TUIs thrash between layouts"
    );
}

#[test]
fn bootstrap_native_present_path_does_not_republish_cached_host_snapshot() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        !content.contains("surface.host_image_snapshot()"),
        "bootstrap should stop re-reading the retained native snapshot immediately after present because the native surface bridge already republishes the freshest host-owned image"
    );
}
