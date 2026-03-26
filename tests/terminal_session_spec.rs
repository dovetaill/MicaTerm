use mica_term::app::ssh::runtime::{
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalSession,
};
use termwiz::input::{KeyCode, Modifiers};
use uuid::Uuid;

#[test]
fn terminal_session_applies_remote_bytes_and_tracks_seqno() {
    let mut session = TerminalSession::new(24, 80);
    let initial_seqno = session.sequence_number();

    session.apply_remote_bytes(b"hello from ssh\r\n");

    assert!(session.sequence_number() > initial_seqno);
    assert!(session.screen_text().contains("hello from ssh"));
}

#[test]
fn terminal_session_encodes_keyboard_input_for_shell_writeback() {
    let mut session = TerminalSession::new(24, 80);

    let enter = session
        .send_key_down(KeyCode::Enter, Modifiers::NONE)
        .expect("encode enter");
    let ctrl_c = session
        .send_key_down(KeyCode::Char('c'), Modifiers::CTRL)
        .expect("encode ctrl+c");

    assert_eq!(enter, b"\r");
    assert_eq!(ctrl_c, vec![3]);
}

#[test]
fn terminal_runtime_snapshot_can_be_polled_without_exposing_transport_objects() {
    let mut session = TerminalSession::new(24, 80);
    let session_id = Uuid::new_v4();

    session.apply_remote_bytes(b"welcome to mica-term\r\n");

    let snapshot = session.surface_state(session_id);

    assert_eq!(snapshot.session_id, session_id);
    assert!(snapshot.seqno > 0);
    assert_eq!(snapshot.rows, 24);
    assert_eq!(snapshot.cols, 80);
    assert!(
        snapshot
            .visible_lines
            .iter()
            .any(|line| line.contains("welcome to mica-term"))
    );
}

#[test]
fn terminal_surface_projection_tracks_cursor_colors_and_filters_bracketed_paste_state() {
    let mut session = TerminalSession::new(24, 80);
    let session_id = Uuid::new_v4();

    session.apply_remote_bytes(b"\x1b[?2004h\x1b[31m[root@host ~]#\x1b[0m ");

    let snapshot = session.surface_state(session_id);
    let prompt_row = snapshot
        .visible_rows
        .iter()
        .find(|row| row.text.contains("[root@host ~]#"))
        .expect("projected prompt row");
    let prompt_prefix = snapshot
        .cells
        .iter()
        .find(|cell| cell.row == prompt_row.index && cell.col == 0)
        .expect("first prompt cell");

    assert!(snapshot.bracketed_paste_enabled);
    assert_eq!(snapshot.cursor.row, prompt_row.index);
    assert_eq!(snapshot.cursor.col, 15);
    assert!(snapshot.cursor.visible);
    assert!(
        !snapshot
            .visible_rows
            .iter()
            .any(|row| row.text.contains("[?2004h")),
        "bracketed paste enable sequence should remain terminal state, not visible prompt text"
    );
    assert_ne!(
        prompt_prefix.fg_rgba, prompt_prefix.bg_rgba,
        "ANSI-colored prompt text should retain distinct foreground/background colors in the surface projection"
    );
}

#[test]
fn terminal_session_encodes_mouse_input_using_active_tracking_mode() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let press = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Down,
            button: TerminalMouseButton::Left,
            row: 2,
            col: 4,
            shift: true,
            ctrl: false,
            alt: false,
        })
        .expect("encode mouse press");
    let drag = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Move,
            button: TerminalMouseButton::None,
            row: 2,
            col: 5,
            shift: true,
            ctrl: false,
            alt: false,
        })
        .expect("encode mouse drag");
    let release = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Up,
            button: TerminalMouseButton::Left,
            row: 2,
            col: 5,
            shift: true,
            ctrl: false,
            alt: false,
        })
        .expect("encode mouse release");

    assert_eq!(press, b"\x1b[<4;5;3M");
    assert_eq!(drag, b"\x1b[<36;6;3M");
    assert_eq!(release, b"\x1b[<4;6;3m");
}
