use mica_term::app::ssh::runtime::TerminalSession;
use termwiz::input::{KeyCode, Modifiers};

#[test]
fn terminal_session_applies_remote_bytes_to_wezterm_terminal() {
    let mut session = TerminalSession::new(24, 80);
    let initial_seqno = session.sequence_number();

    session.apply_remote_bytes(b"hello from ssh\r\n");

    assert!(session.sequence_number() > initial_seqno);
    assert!(session.screen_text().contains("hello from ssh"));
}

#[test]
fn terminal_session_encodes_keyboard_input_with_termwiz_before_write() {
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
