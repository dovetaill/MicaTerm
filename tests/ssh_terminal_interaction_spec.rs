use mica_term::app::ssh::runtime::{TerminalKeyEvent, TerminalSession};
use mica_term::theme::ThemeMode;
use uuid::Uuid;

#[test]
fn function_keys_and_application_cursor_keys_are_encoded_from_live_terminal_state() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?1h");

    let up = session
        .send_key_event(TerminalKeyEvent::named("up", false, false, false))
        .expect("encode up");
    let f5 = session
        .send_key_event(TerminalKeyEvent::function(5, false, false, false))
        .expect("encode f5");

    assert_eq!(up, b"\x1bOA");
    assert_eq!(f5, b"\x1b[15~");
}

#[test]
fn bracketed_paste_wraps_clipboard_payload_when_enabled() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?2004h");

    let bytes = session.encode_paste("echo hi\n").expect("encode paste");
    assert_eq!(bytes, b"\x1b[200~echo hi\n\x1b[201~");
}

#[test]
fn local_scrollback_changes_visible_projection_without_mutating_remote_screen() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");

    let latest = session.surface_state(Uuid::new_v4());
    session.scroll_viewport_lines(2);
    let scrolled = session.surface_state(Uuid::new_v4());

    assert_ne!(latest.visible_lines, scrolled.visible_lines);
    assert!(scrolled.visible_lines.iter().any(|line| line == "3"));
}

#[test]
fn keyboard_input_snaps_local_scrollback_back_to_bottom() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(2);

    let before = session.surface_state(Uuid::new_v4());
    session
        .send_key_event(TerminalKeyEvent::character('a', false, false, false))
        .expect("encode key input");
    let after = session.surface_state(Uuid::new_v4());

    assert!(!before.viewport_at_bottom);
    assert!(after.viewport_at_bottom);
}

#[test]
fn light_theme_palette_changes_default_background_projection() {
    let mut session = TerminalSession::new(24, 80);

    session.set_theme_mode(ThemeMode::Light);
    session.apply_remote_bytes(b"[root@host ~]# ");

    let snapshot = session.surface_state(Uuid::new_v4());
    let prompt = snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("prompt cell");

    assert_ne!(prompt.bg_rgba, 0xff00_0000);
}
