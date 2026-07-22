use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

use mica_term::app::ssh::runtime::TerminalSession;
use mica_term::app::terminal_core::TerminalViewportMetrics;

fn one_pixel_png() -> Vec<u8> {
    let image =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(1, 1, Rgba([0x20, 0x80, 0xe0, 0x80])));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("encode fixture png");
    bytes
}

fn iterm_inline_sequence(inline: bool) -> Vec<u8> {
    format!(
        "\x1b]1337;File=inline={};width=1;height=1:{}\x07",
        u8::from(inline),
        STANDARD.encode(one_pixel_png())
    )
    .into_bytes()
}

fn kitty_direct_sequence(image_id: u32, placement_id: u32) -> Vec<u8> {
    let rgba = [0x20, 0x80, 0xe0, 0x80];
    format!(
        "\x1b_Ga=T,f=32,s=1,v=1,i={image_id},p={placement_id},c=1,r=1,C=1;{}\x1b\\",
        STANDARD.encode(rgba)
    )
    .into_bytes()
}

#[test]
fn iterm_inline_fixture_projects_one_static_resource_and_placement() {
    let mut terminal = TerminalSession::new(4, 8);
    assert!(
        terminal
            .apply_remote_bytes(iterm_inline_sequence(true).as_slice())
            .is_empty()
    );

    let frame = terminal.frame_snapshot();
    assert_eq!(frame.image_resources.len(), 1);
    assert!(!frame.image_placements.is_empty());
    assert!(
        frame
            .image_placements
            .iter()
            .all(|placement| placement.image_id.is_none())
    );
    assert_eq!(frame.image_resources[0].rgba.len(), 4);
}

#[test]
fn iterm_download_fixture_never_projects_or_writes_a_local_file() {
    let mut terminal = TerminalSession::new(4, 8);
    assert!(
        terminal
            .apply_remote_bytes(iterm_inline_sequence(false).as_slice())
            .is_empty()
    );
    let frame = terminal.frame_snapshot();
    assert!(frame.image_resources.is_empty());
    assert!(frame.image_placements.is_empty());
}

#[test]
fn sixel_fixture_projects_rgba_cells() {
    let mut terminal = TerminalSession::new(4, 8);
    terminal.apply_remote_bytes(b"\x1bPq#1;2;100;0;0#1~\x1b\\");

    let frame = terminal.frame_snapshot();
    assert_eq!(frame.image_resources.len(), 1);
    assert!(!frame.image_placements.is_empty());
    assert_eq!(frame.image_resources[0].width, 1);
    assert_eq!(frame.image_resources[0].height, 6);
}

#[test]
fn kitty_direct_fixture_reuses_one_resource_and_replies_immediately() {
    let mut terminal = TerminalSession::new(4, 8);
    let first_reply = terminal.apply_remote_bytes(kitty_direct_sequence(7, 11).as_slice());
    let first_frame = terminal.frame_snapshot();
    assert!(first_reply.is_empty());
    assert_eq!(first_frame.image_resources.len(), 1);
    assert_eq!(first_frame.image_placements.len(), 1);

    terminal.apply_remote_bytes(b"\x1b_Ga=p,i=7,p=12,c=1,r=1,C=1;\x1b\\");
    let frame = terminal.frame_snapshot();
    assert_eq!(frame.image_resources.len(), 1);
    assert!(frame.image_placements.len() >= 2);
    assert!(
        frame
            .image_placements
            .iter()
            .any(|placement| placement.placement_id == Some(11))
    );
    assert!(
        frame
            .image_placements
            .iter()
            .any(|placement| placement.placement_id == Some(12))
    );
    assert!(
        frame
            .image_placements
            .iter()
            .all(|placement| placement.resource_key == frame.image_resources[0].content_hash)
    );
}

#[test]
fn kitty_chunked_direct_fixture_is_coalesced() {
    let mut terminal = TerminalSession::new(4, 8);
    assert!(
        terminal
            .apply_remote_bytes(b"\x1b_Ga=T,f=32,s=1,v=1,i=9,p=21,c=1,r=1,C=1,m=1;IIA=\x1b\\")
            .is_empty()
    );
    let reply = terminal.apply_remote_bytes(b"\x1b_Gm=0;4IA=\x1b\\");
    assert!(reply.is_empty());
    assert_eq!(terminal.frame_snapshot().image_resources.len(), 1);
}

#[test]
fn kitty_query_reply_is_available_from_the_same_remote_apply() {
    let mut terminal = TerminalSession::new(4, 8);
    let reply = terminal.apply_remote_bytes(b"\x1b_Ga=q,s=1,v=1,i=42;YWJjZA==\x1b\\");
    assert_eq!(String::from_utf8_lossy(&reply), "\x1b_Gi=42;OK\x1b\\");
}

#[test]
fn kitty_delete_reply_is_available_from_the_same_remote_apply() {
    let mut terminal = TerminalSession::new(4, 8);
    let reply = terminal.apply_remote_bytes(b"\x1b_Ga=d,d=I,i=404,q=0;\x1b\\");
    assert_eq!(String::from_utf8_lossy(&reply), "\x1b_Gi=404;OK\x1b\\");
}

#[test]
fn kitty_delete_removes_an_existing_placement() {
    let mut terminal = TerminalSession::new(4, 8);
    terminal.apply_remote_bytes(kitty_direct_sequence(405, 32).as_slice());
    assert!(!terminal.frame_snapshot().image_placements.is_empty());
    terminal.apply_remote_bytes(b"\x1b_Ga=d,d=I,i=405,q=0;\x1b\\");
    assert!(terminal.frame_snapshot().image_placements.is_empty());
}

#[test]
fn kitty_external_media_is_rejected_without_reading_the_local_path() {
    let local_path = std::env::temp_dir().join(format!(
        "mica-term-kitty-external-media-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&local_path, b"abcd").expect("write guarded local fixture");
    let sequence = format!(
        "\x1b_Ga=q,t=f,s=1,v=1,i=73;{}\x1b\\",
        STANDARD.encode(local_path.to_string_lossy().as_bytes())
    );

    let mut terminal = TerminalSession::new(4, 8);
    let reply = terminal.apply_remote_bytes(sequence.as_bytes());
    assert_eq!(
        String::from_utf8_lossy(&reply),
        "\x1b_Gi=73;ERROR:external kitty image media is disabled\x1b\\"
    );
    assert_eq!(
        std::fs::read(&local_path).expect("guarded fixture remains readable"),
        b"abcd"
    );
    std::fs::remove_file(local_path).expect("remove guarded local fixture");
}

#[test]
fn real_viewport_metrics_reach_the_terminal_frame() {
    let mut terminal = TerminalSession::new(4, 8);
    terminal.resize_with_viewport(5, 10, TerminalViewportMetrics::new(1370, 845, 144));
    let frame = terminal.frame_snapshot();
    assert_eq!(frame.rows, 5);
    assert_eq!(frame.cols, 10);
    assert_eq!(
        frame.viewport_metrics,
        TerminalViewportMetrics::new(1370, 845, 144)
    );
}
