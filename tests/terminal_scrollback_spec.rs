use mica_term::app::ssh::runtime::{
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalSession,
};
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
