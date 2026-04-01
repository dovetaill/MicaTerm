use mica_term::app::ssh::runtime::{
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalSession,
};
use std::fs;
use uuid::Uuid;

#[test]
fn wheel_without_mouse_grab_scrolls_local_viewport() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");

    let latest = session.surface_state(Uuid::new_v4());
    session.scroll_viewport_lines(2);
    let scrolled = session.surface_state(Uuid::new_v4());

    assert!(!latest.mouse_grabbed);
    assert_ne!(latest.visible_lines, scrolled.visible_lines);
    assert!(scrolled.visible_lines.iter().any(|line| line == "3"));
}

#[test]
fn wheel_with_mouse_grab_is_forwarded_as_remote_mouse_input() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.apply_remote_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let before = session.surface_state(Uuid::new_v4());
    let bytes = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Scroll,
            button: TerminalMouseButton::WheelUp,
            row: 3,
            col: 0,
            shift: false,
            ctrl: false,
            alt: false,
        })
        .expect("encode mouse wheel");
    let after = session.surface_state(Uuid::new_v4());

    assert!(before.mouse_grabbed);
    assert_eq!(before.visible_lines, after.visible_lines);
    assert_eq!(bytes, b"\x1b[<64;1;4M");
}

#[test]
fn remote_output_preserves_scrollback_view_when_user_is_not_at_bottom() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(2);
    let before = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"7\r\n");

    let after = session.surface_state(Uuid::new_v4());

    assert!(!before.viewport_at_bottom);
    assert!(!after.viewport_at_bottom);
    assert_eq!(after.visible_lines, before.visible_lines);
    assert_eq!(
        after.viewport_offset_lines,
        before.viewport_offset_lines + 1
    );
    assert!(!after.visible_lines.iter().any(|line| line == "7"));
}

#[test]
fn remote_output_keeps_following_when_viewport_is_at_bottom() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    let before = session.surface_state(Uuid::new_v4());

    session.apply_remote_bytes(b"7\r\n");

    let after = session.surface_state(Uuid::new_v4());

    assert!(before.viewport_at_bottom);
    assert!(after.viewport_at_bottom);
    assert!(after.visible_lines.iter().any(|line| line == "7"));
}

#[test]
fn renderer_migration_docs_describe_windows_native_status_and_bitmap_fallback() {
    let readme = fs::read_to_string("readme.md").expect("read readme");
    let verification = fs::read_to_string("verification.md").expect("read verification");
    let mainline_build = fs::read_to_string("build-win-x64.sh").expect("read mainline build script");
    let software_build =
        fs::read_to_string("build-win-x64-software.sh").expect("read software build script");

    assert!(
        readme.contains("Windows-first native renderer"),
        "readme should document the current Windows-first native renderer status"
    );
    assert!(
        readme.contains("Linux/macOS"),
        "readme should document the pending Linux/macOS follow-up work"
    );
    assert!(
        verification.contains("bitmap fallback"),
        "verification notes should call out the bitmap fallback path"
    );
    assert!(
        verification.contains("Windows-first native renderer"),
        "verification notes should include the Windows-first native renderer verification status"
    );
    assert!(
        mainline_build.contains("native-first terminal renderer path"),
        "the primary Windows build wrapper should document the native renderer as the preferred shipping path"
    );
    assert!(
        mainline_build.contains("MICA_TERM_PACKAGE_TERMINAL_RENDERER=\"native\""),
        "the primary Windows build wrapper should package the native terminal renderer"
    );
    assert!(
        software_build.contains("fallback-only bitmap compatibility path"),
        "the software compatibility wrapper should describe itself as the fallback-only bitmap path"
    );
    assert!(
        software_build.contains("MICA_TERM_PACKAGE_TERMINAL_RENDERER=\"bitmap\""),
        "the software compatibility wrapper should keep packaging the bitmap terminal renderer"
    );
}
