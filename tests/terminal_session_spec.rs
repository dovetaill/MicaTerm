use mica_term::app::ssh::runtime::TerminalSession;
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
    assert!(snapshot.screen_text.contains("welcome to mica-term"));
}
