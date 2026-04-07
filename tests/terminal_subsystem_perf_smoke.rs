use mica_term::app::ssh::runtime::TerminalSession;
use mica_term::theme::ThemeMode;
use uuid::Uuid;

#[test]
fn surface_snapshot_stays_compact_after_scrollback() {
    let mut session = TerminalSession::new(3, 8);

    session.apply_remote_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");

    let snapshot = session.surface_state(Uuid::new_v4());

    assert!(snapshot.visible_rows.len() <= snapshot.rows as usize);
    assert!(snapshot.visible_lines.len() <= snapshot.rows as usize);
    assert!(snapshot.cells.iter().all(|cell| cell.row < snapshot.rows));
}

#[test]
fn theme_toggle_reprojects_palette_without_resetting_viewport_text() {
    let mut session = TerminalSession::new(3, 8);

    session.apply_remote_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
    session.scroll_viewport_lines(1);
    let dark = session.surface_state(Uuid::new_v4());

    session.set_theme_mode(ThemeMode::Light);
    let light = session.surface_state(Uuid::new_v4());

    assert_eq!(light.visible_lines, dark.visible_lines);
    assert_eq!(light.viewport_offset_lines, dark.viewport_offset_lines);
    assert_ne!(light.default_bg_rgba, dark.default_bg_rgba);
}
