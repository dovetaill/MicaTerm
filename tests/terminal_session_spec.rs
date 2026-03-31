use std::fs;

use mica_term::app::ssh::runtime::{
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalSession,
};
use mica_term::theme::ThemeMode;
use termwiz::input::{KeyCode, Modifiers};
use uuid::Uuid;

fn extract_debug_u32_field(debug: &str, field_name: &str) -> u32 {
    let needle = format!("{field_name}: ");
    let start = debug
        .find(&needle)
        .unwrap_or_else(|| panic!("missing `{field_name}` in snapshot debug output: {debug}"))
        + needle.len();
    let value = debug[start..]
        .split([',', '}'])
        .next()
        .expect("debug field terminator")
        .trim();

    value
        .parse::<u32>()
        .unwrap_or_else(|error| panic!("parse `{field_name}` from `{value}`: {error}"))
}

#[test]
fn terminal_session_applies_remote_bytes_and_tracks_seqno() {
    let mut session = TerminalSession::new(24, 80);
    let initial_seqno = session.sequence_number();

    session.apply_remote_bytes(b"hello from ssh\r\n");

    assert!(session.sequence_number() > initial_seqno);
    assert!(session.screen_text().contains("hello from ssh"));
}

#[test]
fn exact_cockpit_banner_is_filtered_before_terminal_parser() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(
        b"Activate the web console with: systemctl enable --now cockpit.socket\r\n[root@host ~]# ",
    );

    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(
        !snapshot
            .visible_lines
            .iter()
            .any(|line| line.contains("cockpit.socket"))
    );
    assert!(
        snapshot
            .visible_lines
            .iter()
            .any(|line| line.contains("[root@host ~]#"))
    );
}

#[test]
fn similar_lines_are_not_filtered_when_they_do_not_exactly_match() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(
        b"Activate the web console with: systemctl enable --now cockpit.socket now\r\n",
    );

    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(
        snapshot
            .visible_lines
            .iter()
            .any(|line| line.contains("cockpit.socket now"))
    );
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
    assert_eq!(snapshot.viewport_offset_lines, 0);
    assert_eq!(snapshot.viewport_max_offset_lines, 0);
    assert!(snapshot.viewport_at_bottom);
}

#[test]
fn surface_projection_exposes_scrollback_metadata() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(3);

    let snapshot = session.surface_state(Uuid::new_v4());

    assert!(snapshot.viewport_offset_lines > 0);
    assert!(snapshot.viewport_max_offset_lines >= snapshot.viewport_offset_lines);
}

#[test]
fn terminal_session_preserves_deeper_scrollback_history_for_large_bursts() {
    let mut session = TerminalSession::new(4, 20);
    let configured_scrollback_lines = 3_500usize;
    let transcript = (0..9000)
        .map(|line| format!("{line:04}\r\n"))
        .collect::<String>();

    session.apply_remote_bytes(transcript.as_bytes());
    session.scroll_viewport_lines(20_000);

    let snapshot = session.surface_state(Uuid::new_v4());
    let first_visible = snapshot
        .visible_lines
        .iter()
        .find(|line| !line.is_empty())
        .expect("top retained scrollback line");
    let first_visible_line = first_visible
        .parse::<usize>()
        .expect("parse retained scrollback line number");
    let expected_floor = 9_000usize.saturating_sub(configured_scrollback_lines + 4);

    assert!(
        first_visible_line >= expected_floor,
        "bounded scrollback should retain only the newest configured history window when a burst exceeds the cap"
    );
}

#[test]
fn dark_theme_surface_projection_exposes_default_canvas_palette_fields() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"[root@host ~]# ");

    let debug = format!("{:?}", session.surface_state(Uuid::new_v4()));
    let default_fg_rgba = extract_debug_u32_field(&debug, "default_fg_rgba");
    let default_bg_rgba = extract_debug_u32_field(&debug, "default_bg_rgba");

    assert_eq!(default_fg_rgba, 0xffd8_dfe8);
    assert_eq!(default_bg_rgba, 0xff0c_1014);
}

#[test]
fn light_theme_surface_projection_exposes_default_canvas_palette_fields() {
    let mut session = TerminalSession::new(24, 80);

    session.set_theme_mode(ThemeMode::Light);
    session.apply_remote_bytes(b"[root@host ~]# ");

    let debug = format!("{:?}", session.surface_state(Uuid::new_v4()));
    let default_fg_rgba = extract_debug_u32_field(&debug, "default_fg_rgba");
    let default_bg_rgba = extract_debug_u32_field(&debug, "default_bg_rgba");

    assert_eq!(default_fg_rgba, 0xff24_292f);
    assert_eq!(default_bg_rgba, 0xfffc_fdff);
}

#[test]
fn dark_theme_surface_projection_exposes_mica_code_dark_cursor_palette() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"[root@host ~]# ");

    let snapshot = session.surface_state(Uuid::new_v4());

    assert_eq!(snapshot.cursor.fg_rgba, 0xff0c_1014);
    assert_eq!(snapshot.cursor.bg_rgba, 0xffd8_dfe8);
}

#[test]
fn light_theme_surface_projection_exposes_mica_code_light_cursor_palette() {
    let mut session = TerminalSession::new(24, 80);

    session.set_theme_mode(ThemeMode::Light);
    session.apply_remote_bytes(b"[root@host ~]# ");

    let snapshot = session.surface_state(Uuid::new_v4());

    assert_eq!(snapshot.cursor.fg_rgba, 0xfffc_fdff);
    assert_eq!(snapshot.cursor.bg_rgba, 0xff4c_5561);
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
fn terminal_font_backend_owns_cell_metrics_contract_beyond_legacy_atlas_renderer() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");
    let font_backend =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read terminal font backend");

    assert!(
        atlas_source.contains("compute_terminal_metrics"),
        "legacy atlas renderer still computes bitmap metrics during the migration"
    );
    assert!(
        font_backend.contains("pub struct FontMetrics"),
        "terminal font backend should expose a shared FontMetrics contract outside the legacy atlas renderer"
    );
    assert!(
        font_backend.contains("pub cell_width_px"),
        "shared font metrics should own cell width information for the shaping pipeline"
    );
}

#[test]
fn light_theme_default_background_projection_is_not_black() {
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

#[test]
fn theme_toggle_refreshes_existing_terminal_palette_projection() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"[root@host ~]# ");

    let dark_snapshot = session.surface_state(Uuid::new_v4());
    let dark_prompt = dark_snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("dark prompt cell");

    session.set_theme_mode(ThemeMode::Light);

    let light_snapshot = session.surface_state(Uuid::new_v4());
    let light_prompt = light_snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("light prompt cell");

    assert_ne!(dark_prompt.bg_rgba, light_prompt.bg_rgba);
    assert_ne!(light_prompt.bg_rgba, 0xff00_0000);
}

#[test]
fn dark_theme_palette_uses_bright_default_foreground() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"[root@host ~]# ");

    let snapshot = session.surface_state(Uuid::new_v4());
    let prompt = snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("prompt cell");

    assert_ne!(prompt.fg_rgba, 0xff00_0000);
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

#[test]
fn terminal_session_ignores_hover_motion_when_only_button_event_tracking_is_enabled() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let hover = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Move,
            button: TerminalMouseButton::None,
            row: 2,
            col: 4,
            shift: false,
            ctrl: false,
            alt: false,
        })
        .expect("encode hover motion");

    assert!(
        hover.is_empty(),
        "button-event tracking should not synthesize no-button hover motion reports"
    );
}

#[test]
fn terminal_session_encodes_hover_motion_when_any_event_tracking_is_enabled() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?1000h\x1b[?1003h\x1b[?1006h");

    let hover = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Move,
            button: TerminalMouseButton::None,
            row: 2,
            col: 4,
            shift: false,
            ctrl: false,
            alt: false,
        })
        .expect("encode any-event hover motion");

    assert_eq!(hover, b"\x1b[<35;5;3M");
}
