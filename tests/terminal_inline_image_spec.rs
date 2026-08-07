use std::io::Cursor;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

use mica_term::app::ssh::runtime::TerminalSession;
use mica_term::app::terminal_core::{
    LocalTerminalImage, TerminalCoreAdapter, TerminalViewportMetrics, WeztermTerminalCoreAdapter,
};

fn fixture_png(width: u32, height: u32) -> Vec<u8> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        width,
        height,
        Rgba([0x20, 0x80, 0xe0, 0x80]),
    ));
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .expect("encode fixture png");
    bytes
}

fn one_pixel_png() -> Vec<u8> {
    fixture_png(1, 1)
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

fn local_image(width: u32, height: u32, columns: u32, rows: u32) -> LocalTerminalImage {
    LocalTerminalImage {
        png_bytes: fixture_png(width, height),
        source_width: width,
        source_height: height,
        columns,
        rows,
    }
}

#[test]
fn local_clipboard_image_uses_unowned_cells_and_advances_below_it() {
    let mut terminal = WeztermTerminalCoreAdapter::new(6, 12, 32);
    assert!(terminal.apply_remote_bytes(b"prompt> ").is_empty());
    let before = terminal.frame_snapshot();

    let result: anyhow::Result<()> = terminal.apply_local_image(local_image(40, 20, 4, 2));
    result.expect("place local clipboard image");

    let after = terminal.frame_snapshot();
    assert!(after.seqno > before.seqno);
    assert_eq!(after.image_resources.len(), 1);
    assert!(!after.image_placements.is_empty());
    assert!(
        after
            .image_placements
            .iter()
            .all(|placement| { placement.image_id.is_none() && placement.placement_id.is_none() })
    );
    assert_eq!(after.cursor.col, 0);
    assert!(after.cursor.row >= before.cursor.row.saturating_add(2));
}

#[test]
fn local_clipboard_image_is_not_owned_by_remote_kitty_delete_commands() {
    let mut terminal = WeztermTerminalCoreAdapter::new(6, 12, 32);
    terminal
        .apply_local_image(local_image(20, 20, 2, 2))
        .expect("place local image");
    let local_placement_count = terminal.frame_snapshot().image_placements.len();

    terminal.apply_remote_bytes(b"\x1b_Ga=d,d=I,i=405,q=0;\x1b\\");
    assert_eq!(
        terminal.frame_snapshot().image_placements.len(),
        local_placement_count
    );

    terminal.apply_remote_bytes(b"\x1b_Ga=d,d=A,q=0;\x1b\\");
    let after_delete_all = terminal.frame_snapshot();
    assert_eq!(
        after_delete_all.image_placements.len(),
        local_placement_count
    );
    assert!(
        after_delete_all
            .image_placements
            .iter()
            .all(|placement| { placement.image_id.is_none() && placement.placement_id.is_none() })
    );
}

#[test]
fn local_clipboard_image_follows_terminal_scrollback() {
    let mut terminal = WeztermTerminalCoreAdapter::new(3, 8, 32);
    terminal
        .apply_local_image(local_image(20, 10, 2, 1))
        .expect("place local image");
    terminal.apply_remote_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
    assert!(terminal.frame_snapshot().image_placements.is_empty());

    terminal.scroll_viewport_lines(5);
    assert!(
        !terminal.frame_snapshot().image_placements.is_empty(),
        "local image cells should remain available in scrollback"
    );
}

#[test]
fn local_clipboard_image_rejects_invalid_input_without_terminal_mutation() {
    let mut terminal = WeztermTerminalCoreAdapter::new(6, 12, 32);
    terminal.apply_remote_bytes(b"prompt> ");
    let before = terminal.frame_snapshot();

    let result: anyhow::Result<()> = terminal.apply_local_image(LocalTerminalImage {
        png_bytes: fixture_png(1, 1),
        source_width: 5_001,
        source_height: 5_001,
        columns: 1,
        rows: 1,
    });

    assert!(result.is_err());
    let after = terminal.frame_snapshot();
    assert_eq!(after.seqno, before.seqno);
    assert_eq!(after.cursor, before.cursor);
    assert_eq!(after.image_resources, before.image_resources);
    assert_eq!(after.image_placements, before.image_placements);
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
