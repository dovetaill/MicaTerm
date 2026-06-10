use std::fs;

use mica_term::app::bootstrap::{
    WorkspaceTerminalLinkAffordance, workspace_terminal_link_affordance_for_test,
    workspace_terminal_openable_url_at_surface_for_test,
};
use mica_term::app::ssh::runtime::{
    TerminalCellState, TerminalKeyEvent, TerminalSession, TerminalSurfaceState,
    extract_current_working_directory_from_osc7,
};
use mica_term::app::terminal_theme::preset_for_theme_mode;
use mica_term::theme::ThemeMode;
use uuid::Uuid;

#[test]
fn osc7_sequence_updates_current_working_directory_snapshot() {
    let bytes = b"\x1b]7;file://prod-host/srv/app/releases\x07";

    let cwd = extract_current_working_directory_from_osc7(bytes)
        .expect("cwd should be extracted from osc7 payload");

    assert_eq!(cwd, "/srv/app/releases");
}

#[test]
fn malformed_osc7_sequence_is_ignored() {
    assert_eq!(
        extract_current_working_directory_from_osc7(b"\x1b]7;not-a-file-url\x07"),
        None
    );
}

#[test]
fn wide_char_trailing_cell_hit_normalizes_back_to_the_leading_cell() {
    let session_id = Uuid::new_v4();
    let mut surface =
        TerminalSurfaceState::from_visible_lines(session_id, 1, 1, 4, vec!["条a ".into()]);
    surface.cells = vec![
        TerminalCellState {
            row: 0,
            col: 0,
            width: 2,
            text: "条".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff0d_1117,
        },
        TerminalCellState {
            row: 0,
            col: 2,
            width: 1,
            text: "a".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff0d_1117,
        },
    ];

    assert_eq!(
        surface.normalize_hit_col(0, 1),
        0,
        "the trailing half of a wide glyph should normalize back to the leading logical cell"
    );
    assert_eq!(
        surface.normalize_hit_col(0, 2),
        2,
        "single-cell glyphs should keep their own hit column"
    );
    assert_eq!(
        surface.normalize_hit_col(0, 3),
        3,
        "empty cells should preserve their raw hit column"
    );
}

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
    let f24 = session
        .send_key_event(TerminalKeyEvent::function(24, false, false, false))
        .expect("encode f24");

    assert_eq!(up, b"\x1bOA");
    assert_eq!(f5, b"\x1b[15~");
    assert_eq!(f24, b"\x1b[45~");
}

#[test]
fn insert_key_is_encoded_for_terminal_writeback() {
    let mut session = TerminalSession::new(24, 80);

    let insert = session
        .send_key_event(TerminalKeyEvent::named("insert", false, false, false))
        .expect("encode insert");

    assert_eq!(insert, b"\x1b[2~");
}

#[test]
fn shifted_tab_is_encoded_for_terminal_writeback() {
    let mut session = TerminalSession::new(24, 80);

    let backtab = session
        .send_key_event(TerminalKeyEvent::named("tab", false, false, true))
        .expect("encode shifted tab");

    assert_eq!(backtab, b"\x1b[Z");
}

#[test]
fn terminal_host_forwards_backtab_into_terminal_tab_input() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("event.text == Key.Backtab"),
        "TerminalSessionHost should recognize Slint's Backtab key token for Shift+Tab"
    );
    assert!(
        terminal_host.contains(
            "root.key-input(\"tab\", event.modifiers.alt, event.modifiers.control, true);"
        ),
        "TerminalSessionHost should forward Backtab as a shifted terminal Tab sequence instead of leaving Shift+Tab to local focus navigation"
    );
}

#[test]
fn paste_wraps_payload_when_bracketed_paste_is_enabled() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?2004h");

    let bytes = session.encode_paste("echo hi\n").expect("encode paste");
    assert_eq!(bytes, b"\x1b[200~echo hi\n\x1b[201~");
}

#[test]
fn bracketed_paste_echo_suppresses_active_region_reverse_video() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[?2004h");
    session
        .encode_paste("echo hi")
        .expect("arm bracketed paste echo filter");
    session.apply_remote_bytes(b"\x1b[7mecho hi\x1b[27m");

    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(
        snapshot
            .cells
            .iter()
            .take(7)
            .all(|cell| cell.bg_rgba == snapshot.default_bg_rgba),
        "paste echo highlight should not invert the background colors"
    );
}

#[test]
fn inverse_video_still_renders_without_a_pending_paste_echo() {
    let mut session = TerminalSession::new(24, 80);

    session.apply_remote_bytes(b"\x1b[7mX\x1b[27m");

    let snapshot = session.surface_state(Uuid::new_v4());
    let cell = snapshot
        .cells
        .iter()
        .find(|cell| cell.text == "X")
        .expect("inverse cell");
    assert_ne!(cell.bg_rgba, snapshot.default_bg_rgba);
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
fn surface_state_tracks_alternate_screen_activity_for_semantic_guards() {
    let mut session = TerminalSession::new(24, 80);

    let initial = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"\x1b[?1049h");
    let alternate = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"\x1b[?1049l");
    let restored = session.surface_state(Uuid::new_v4());

    assert!(!initial.alternate_screen_active);
    assert!(alternate.alternate_screen_active);
    assert!(!restored.alternate_screen_active);
}

#[test]
fn surface_state_tracks_application_cursor_key_mode_for_inline_app_guards() {
    let mut session = TerminalSession::new(24, 80);

    let initial = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"\x1b[?1h");
    let app_cursor = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"\x1b[?1l");
    let restored = session.surface_state(Uuid::new_v4());

    assert!(!initial.application_cursor_keys);
    assert!(app_cursor.application_cursor_keys);
    assert!(!restored.application_cursor_keys);
}

#[test]
fn workspace_terminal_trims_trailing_punctuation_from_http_tokens() {
    let mut session = TerminalSession::new(4, 80);
    session.apply_remote_bytes(b"see https://example.com).\r\n");
    let surface = session.surface_state(Uuid::new_v4());

    assert_eq!(
        workspace_terminal_openable_url_at_surface_for_test(&surface, 0, 8),
        Some("https://example.com".into()),
        "terminal link detection should trim trailing punctuation so log-style URL tokens open the actual URL instead of the surrounding sentence punctuation"
    );
}

#[test]
fn workspace_terminal_link_affordance_requires_safe_surface_state_and_ctrl_for_arming() {
    let mut session = TerminalSession::new(4, 80);
    session.apply_remote_bytes(b"see https://example.com\r\n");
    let mut surface = session.surface_state(Uuid::new_v4());

    assert_eq!(
        workspace_terminal_link_affordance_for_test(&surface, 0, 8, false),
        WorkspaceTerminalLinkAffordance {
            hovered: true,
            armed: false,
        },
        "hovering a URL without Ctrl should expose a detectable link affordance but keep activation gated"
    );
    assert_eq!(
        workspace_terminal_link_affordance_for_test(&surface, 0, 8, true),
        WorkspaceTerminalLinkAffordance {
            hovered: true,
            armed: true,
        },
        "holding Ctrl over a URL should arm the link affordance instead of requiring a separate parser path in the UI"
    );

    surface.alternate_screen_active = true;
    assert_eq!(
        workspace_terminal_link_affordance_for_test(&surface, 0, 8, true),
        WorkspaceTerminalLinkAffordance::default(),
        "alternate-screen TUI content should suppress host link affordances"
    );

    surface.alternate_screen_active = false;
    surface.mouse_grabbed = true;
    assert_eq!(
        workspace_terminal_link_affordance_for_test(&surface, 0, 8, true),
        WorkspaceTerminalLinkAffordance::default(),
        "mouse-grabbed TUI sessions should suppress host link affordances so applications keep control of the pointer"
    );

    surface.mouse_grabbed = false;
    surface.application_cursor_keys = true;
    assert_eq!(
        workspace_terminal_link_affordance_for_test(&surface, 0, 8, true),
        WorkspaceTerminalLinkAffordance::default(),
        "application-cursor inline apps should also suppress host link affordances so app-mode terminal UIs keep ownership of navigation"
    );
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

#[test]
fn light_theme_palette_preserves_active_ansi_black_bright_white_and_default_backgrounds() {
    let mut session = TerminalSession::new(24, 80);
    let preset = preset_for_theme_mode(ThemeMode::Light);

    session.set_theme_mode(ThemeMode::Light);
    session.apply_remote_bytes(b"\x1b[40mA\x1b[0m\x1b[107mB\x1b[0mC");

    let snapshot = session.surface_state(Uuid::new_v4());
    let ansi_black = snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 0)
        .expect("ansi black background cell");
    let ansi_bright_white = snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 1)
        .expect("ansi bright white background cell");
    let default_after_reset = snapshot
        .cells
        .iter()
        .find(|cell| cell.col == 2)
        .expect("default background cell after reset");

    let (ansi_black_r, ansi_black_g, ansi_black_b) = preset.ansi[0];
    let (ansi_bright_white_r, ansi_bright_white_g, ansi_bright_white_b) = preset.ansi[15];
    assert_eq!(
        ansi_black.bg_rgba,
        0xff00_0000
            | (u32::from(ansi_black_r) << 16)
            | (u32::from(ansi_black_g) << 8)
            | u32::from(ansi_black_b)
    );
    assert_eq!(
        ansi_bright_white.bg_rgba,
        0xff00_0000
            | (u32::from(ansi_bright_white_r) << 16)
            | (u32::from(ansi_bright_white_g) << 8)
            | u32::from(ansi_bright_white_b)
    );
    assert_eq!(default_after_reset.bg_rgba, 0xff00_0000 | preset.background);
}

#[test]
fn theme_mode_switch_preserves_visible_text_and_cursor_state() {
    let session_id = Uuid::new_v4();
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"echo hi\r\nline two");
    let dark = session.surface_state(session_id);

    session.set_theme_mode(ThemeMode::Light);
    let light = session.surface_state(session_id);

    assert_eq!(light.visible_lines, dark.visible_lines);
    assert_eq!(light.cursor.row, dark.cursor.row);
    assert_eq!(light.cursor.col, dark.cursor.col);
    assert_ne!(light.default_bg_rgba, dark.default_bg_rgba);
    assert!(light.seqno > dark.seqno);
}

#[test]
fn terminal_host_declares_accumulated_multi_line_wheel_scrollback_contract() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("private property <float> wheel-delta-remainder: 0;"),
        "TerminalSessionHost should retain partial wheel deltas between scroll events"
    );
    assert!(
        terminal_host.contains("private property <float> wheel-delta-threshold: 120;"),
        "TerminalSessionHost should define the wheel normalization threshold used to scale partial deltas into scrollback lines"
    );
    assert!(
        terminal_host.contains("private property <int> wheel-lines-per-notch: 6;"),
        "TerminalSessionHost should map one wheel notch to multiple local scrollback lines"
    );
    assert!(
        terminal_host.contains("function accumulated-wheel-lines() -> int {")
            && terminal_host.contains("let delta-lines = root.accumulated-wheel-lines();")
            && terminal_host.contains("root.wheel-delta-remainder = root.wheel-delta-remainder\n                                + (((event.delta-y / 1px) / root.wheel-delta-threshold)")
            && terminal_host.contains("root.wheel-delta-remainder = root.wheel-delta-remainder - delta-lines;"),
        "TerminalSessionHost should convert partial wheel deltas into proportional line scrollback instead of waiting for full-notch jumps"
    );
}

#[test]
fn terminal_host_bitmap_selection_overlay_stays_in_blank_surface_local_coordinates() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");
    let blank_surface_start = terminal_host
        .find("blank-surface := Rectangle {")
        .expect("blank surface block");
    let input_capture_start = terminal_host[blank_surface_start..]
        .find("input-capture := TouchArea {")
        .map(|offset| blank_surface_start + offset)
        .expect("input capture block");
    let blank_surface_block = &terminal_host[blank_surface_start..input_capture_start];

    assert!(
        terminal_host.contains("function terminal-local-cell-x(col: int) -> length {")
            && terminal_host.contains("function terminal-local-cell-y(row: int) -> length {"),
        "bitmap selection overlay should have dedicated local cell helpers so blank-surface overlays stay in the same coordinate space as the bitmap grid"
    );
    assert!(
        blank_surface_block
            .contains("x: root.selection-local-span-x(root.selection-visible-start-col());")
            && blank_surface_block
                .contains("y: root.terminal-local-cell-y(root.selection-visible-start-row());"),
        "bitmap selection rectangles should stay in blank-surface local coordinates even when the stored selection rows are rebased against scrollback instead of the current viewport"
    );
    assert!(
        !blank_surface_block.contains("x: root.selection-span-x(")
            && !blank_surface_block.contains("y: root.terminal-cell-y("),
        "blank-surface selection rectangles should not add terminal-surface-origin twice through the global cell helpers"
    );
}

#[test]
fn terminal_host_uses_half_cell_selection_boundaries_and_wide_char_normalization_hooks() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("callback normalize-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose a Rust-backed callback that normalizes wide-char trailing cell mouse hits back to the owning leading cell"
    );
    assert!(
        terminal_host.contains("callback normalize-selection-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose a Rust-backed callback that can collapse half-cell selection boundaries away from wide-char interior columns"
    );
    assert!(
        terminal_host.contains("function terminal-selection-hit-col("),
        "TerminalSessionHost should separate selection boundary hit-testing from plain mouse cell hit-testing"
    );
    assert!(
        terminal_host
            .contains("Math.floor(((pointer-x / 1px) / (root.terminal-cell-width / 1px)) + 0.5)"),
        "TerminalSessionHost selection hit-testing should use half-cell edge rounding instead of a plain left-edge floor"
    );
    assert!(
        terminal_host.contains("root.normalize-selection-hit-col("),
        "TerminalSessionHost should normalize half-cell selection hits through the active terminal surface metadata before mutating selection state"
    );
    assert!(
        terminal_host.contains("root.normalize-hit-col("),
        "TerminalSessionHost should normalize mouse-reporting hit cells through the active terminal surface metadata before forwarding them"
    );
}

#[test]
fn terminal_host_treats_selection_end_columns_as_exclusive_boundaries() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains(
            "let clamped-end-col = max(clamped-start-col, min(root.session-cols, end-col));"
        ),
        "TerminalSessionHost selection overlay width should treat the focus column as an exclusive boundary so wide-char cluster ends line up with the runtime selection model"
    );
    assert!(
        !terminal_host.contains("end-col + 1"),
        "TerminalSessionHost should not re-expand exclusive selection boundaries by adding an extra cell during overlay sizing"
    );
}

#[test]
fn workspace_terminal_input_handlers_avoid_per_keystroke_full_projection_refresh() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    let text_input_start = bootstrap_source
        .find("window.on_workspace_session_text_input(move |text| {")
        .expect("text input handler");
    let key_input_start = bootstrap_source
        .find("window.on_workspace_session_key_input(move |key, alt, ctrl, shift| {")
        .expect("key input handler");
    let resize_handler_start = bootstrap_source[key_input_start..]
        .find("window.on_workspace_session_resize_requested(move |rows, cols| {")
        .map(|offset| key_input_start + offset)
        .expect("resize handler");
    let text_input_block = &bootstrap_source[text_input_start..key_input_start];
    let key_input_block = &bootstrap_source[key_input_start..resize_handler_start];

    assert!(
        text_input_block
            .contains("workspace_terminal::apply_local_input_projection_hint(&mut state)")
            && key_input_block
                .contains("workspace_terminal::apply_local_input_projection_hint(&mut state)"),
        "local terminal input handlers should apply the lightweight viewport snap hint instead of forcing a manager projection poll on every repeated key event"
    );
    assert!(
        !text_input_block.contains("workspace_terminal::refresh_active_workspace_projection(")
            && !key_input_block
                .contains("workspace_terminal::refresh_active_workspace_projection("),
        "per-keystroke full projection refreshes block the UI thread during key repeat and should stay out of the local input handlers"
    );
}

#[test]
fn terminal_host_selection_hit_testing_routes_half_cell_boundaries_through_rust_normalization() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("callback normalize-selection-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose a Rust-backed selection boundary normalization callback so wide trailing cells can collapse onto stable cluster edges"
    );
    assert!(
        terminal_host.contains("function terminal-selection-hit-col(pointer-x: length) -> int {"),
        "TerminalSessionHost should derive half-cell boundary hits instead of treating every pointer move as a plain left-edge floor() cell index"
    );
    assert!(
        terminal_host.contains("root.normalize-selection-hit-col("),
        "TerminalSessionHost should route selection hit-testing through a dedicated half-cell helper plus Rust-side boundary normalization instead of reusing raw floor() cell hits"
    );
}

#[test]
fn terminal_host_declares_double_and_triple_click_selection_gestures() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("private property <int> selection-click-streak: 0;"),
        "TerminalSessionHost should track pointer click streak state so double-click and triple-click can map to stable local selection gestures"
    );
    assert!(
        terminal_host.contains("in property <bool> selection-drag-active: false;")
            && terminal_host.contains("callback selection-begin-requested(int, int, int, int);")
            && terminal_host.contains("callback selection-update-requested(int, int, int);")
            && terminal_host.contains("callback selection-finish-requested();"),
        "TerminalSessionHost should route gesture mode and drag ownership through explicit Rust callbacks plus drag-active projection so follow-up drags keep token vs row expansion semantics without reviving host-owned selection state"
    );
    assert!(
        terminal_host.contains("if event.kind == PointerEventKind.down && event.button == PointerEventButton.left && root.selection-click-streak == 2")
            && terminal_host.contains("if event.kind == PointerEventKind.down && event.button == PointerEventButton.left && root.selection-click-streak >= 3"),
        "TerminalSessionHost should branch double-click and triple-click pointer-down events into dedicated gesture handling instead of treating every click as a plain drag anchor"
    );
}

#[test]
fn terminal_host_uses_shift_override_before_mouse_grabbed_forwarding() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("if root.session-mouse-grabbed && !event.modifiers.shift {"),
        "mouse-grabbed terminals should only keep remote pointer ownership while Shift is not pressed so users still have a stable local selection escape hatch"
    );
    assert!(
        !terminal_host.contains("if root.session-mouse-grabbed {"),
        "TerminalSessionHost should not unconditionally forward left-click gestures once the runtime grabs the mouse because that blocks Shift+drag/double/triple local selection overrides"
    );
}

#[test]
fn terminal_host_uses_startup_safe_font_stack_and_stable_clipboard_shortcut_tokens() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains("in property <image> session-surface-image;"),
        "TerminalSessionHost should keep the rendered image surface contract for the bitmap software fallback"
    );
    assert!(
        terminal_host.contains("private property <length> terminal-font-size: 16px;"),
        "TerminalSessionHost should ship with a desktop-sized default font instead of the current too-small compact size"
    );
    assert!(
        !terminal_host.contains("for cell in root.session-cells"),
        "TerminalSessionHost should not render terminal text through a per-cell repeater"
    );
    assert!(
        terminal_host.contains("\\u{3}"),
        "TerminalSessionHost should treat ETX as a Ctrl+Shift+C copy shortcut token when the backend emits control characters"
    );
    assert!(
        terminal_host.contains("\\u{16}"),
        "TerminalSessionHost should treat SYN as a Ctrl+Shift+V paste shortcut token when the backend emits control characters"
    );
    assert!(
        terminal_host.contains("event.text == Key.F1")
            && terminal_host.contains("event.text == Key.F12")
            && terminal_host.contains("event.text == Key.F13")
            && terminal_host.contains("event.text == Key.F24"),
        "TerminalSessionHost should recognize the terminal function-key range from F1 through F24"
    );
    assert!(
        terminal_host.contains("root.key-input(\"f1\"")
            && terminal_host.contains("root.key-input(\"f12\"")
            && terminal_host.contains("root.key-input(\"f13\"")
            && terminal_host.contains("root.key-input(\"f24\""),
        "TerminalSessionHost should forward the supported function-key range into the terminal key-input contract"
    );
    assert!(
        terminal_host.contains("event.text == Key.Insert")
            && terminal_host.contains("root.key-input(\"insert\""),
        "TerminalSessionHost should forward a plain Insert key when it is not part of the local clipboard shortcuts"
    );
}

#[test]
fn winit_backend_maps_named_copy_and_paste_keys_into_terminal_shortcut_chars() {
    let event_loop =
        fs::read_to_string("vendor/i-slint-backend-winit/event_loop.rs").expect("read event loop");

    assert!(
        event_loop.contains("NamedKey::Copy") && event_loop.contains("\"c\".into()"),
        "the patched winit backend should translate NamedKey::Copy into a textual copy shortcut token"
    );
    assert!(
        event_loop.contains("NamedKey::Paste") && event_loop.contains("\"v\".into()"),
        "the patched winit backend should translate NamedKey::Paste into a textual paste shortcut token"
    );
}

#[test]
fn logging_runtime_source_omits_terminal_render_mode_metadata() {
    let logging_runtime =
        fs::read_to_string("src/app/logging/runtime.rs").expect("read logging runtime");

    assert!(
        logging_runtime
            .contains("pub fn emit_runtime_profile_metadata(_profile: AppRuntimeProfile)")
            && !logging_runtime.contains("terminal_render_mode = ?profile.terminal_render_mode()"),
        "runtime logging should keep the runtime-profile helper as an intentional no-op instead of reintroducing terminal render mode startup noise"
    );
}
