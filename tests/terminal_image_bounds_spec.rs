use base64::Engine;
use base64::engine::general_purpose::STANDARD;

use mica_term::app::ssh::runtime::TerminalSession;

#[test]
fn oversized_unterminated_iterm_payload_recovers_only_after_bel() {
    let mut terminal = TerminalSession::new(4, 32);
    terminal.apply_remote_bytes(b"\x1b]1337;File=inline=1:");

    let payload = [b'A'; 4096];
    for _ in 0..(28 * 1024 * 1024 / payload.len()) {
        terminal.apply_remote_bytes(&payload);
    }
    terminal.apply_remote_bytes(b"never-visible");
    terminal.apply_remote_bytes(b"\x07recovered");

    let frame = terminal.frame_snapshot();
    assert!(frame.image_resources.is_empty());
    assert!(!terminal.screen_text().contains("never-visible"));
    assert!(terminal.screen_text().contains("recovered"));
}

#[test]
fn kitty_chunk_accumulation_fails_closed_and_next_transfer_succeeds() {
    let mut terminal = TerminalSession::new(4, 32);
    let payload = "A".repeat(64 * 1024);
    let mut limit_reply = None;

    for chunk_index in 0..512 {
        let control = if chunk_index == 0 {
            "a=t,f=32,s=1,v=1,i=99,m=1"
        } else {
            "m=1"
        };
        let sequence = format!("\x1b_G{control};{payload}\x1b\\");
        let reply = terminal.apply_remote_bytes(sequence.as_bytes());
        if !reply.is_empty() {
            limit_reply = Some(reply);
            break;
        }
    }

    let reply = limit_reply.expect("chunk accumulation must reach the encoded image limit");
    let reply = String::from_utf8_lossy(&reply);
    assert!(reply.contains("i=99;ERROR:"));
    assert!(reply.contains("encoded data exceeds"));
    assert!(terminal.frame_snapshot().image_resources.is_empty());

    terminal.apply_remote_bytes(b"\x1b_Gm=0;\x1b\\");
    let rgba = STANDARD.encode([0x20, 0x80, 0xe0, 0x80]);
    let valid = format!("\x1b_Ga=T,f=32,s=1,v=1,i=100,c=1,r=1,C=1;{rgba}\x1b\\");
    terminal.apply_remote_bytes(valid.as_bytes());
    assert_eq!(terminal.frame_snapshot().image_resources.len(), 1);
}

#[test]
fn oversized_sixel_raster_declaration_is_rejected_before_allocation() {
    let mut terminal = TerminalSession::new(4, 32);
    terminal.apply_remote_bytes(b"\x1bPq\"1;1;5001;5000");
    terminal.apply_remote_bytes(b"@never-visible\x1b");
    terminal.apply_remote_bytes(b"\\recovered");

    assert!(terminal.frame_snapshot().image_resources.is_empty());
    assert!(!terminal.screen_text().contains("never-visible"));
    assert!(terminal.screen_text().contains("recovered"));
}

#[test]
fn kitty_dimensions_over_pixel_limit_return_an_error_without_a_resource() {
    let mut terminal = TerminalSession::new(4, 32);
    let sequence = format!(
        "\x1b_Ga=T,f=32,s=5001,v=5000,i=101;{}\x1b\\",
        STANDARD.encode([0u8; 4])
    );
    let reply = terminal.apply_remote_bytes(sequence.as_bytes());

    assert!(String::from_utf8_lossy(&reply).contains("i=101;ERROR:"));
    assert!(terminal.frame_snapshot().image_resources.is_empty());
}
