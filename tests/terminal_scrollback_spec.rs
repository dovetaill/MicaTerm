use mica_term::app::ssh::runtime::{
    TerminalMouseButton, TerminalMouseEventKind, TerminalMouseInput, TerminalSession,
    TerminalSurfaceState,
};
use mica_term::app::terminal_core::TerminalCoreKind;
use mica_term::app::terminal_model::TerminalModelFrame;
use mica_term::app::terminal_semantic::{
    SemanticStyleRole, detect_input_line_spans, detect_output_block_spans,
};
use std::fs;
#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;
use uuid::Uuid;

fn semantic_surface(lines: &[&str]) -> TerminalSurfaceState {
    TerminalSurfaceState::from_visible_lines(
        Uuid::new_v4(),
        1,
        lines.len() as u32,
        120,
        lines.iter().map(|line| (*line).to_string()).collect(),
    )
}

fn semantic_model_frame(lines: &[&str]) -> TerminalModelFrame {
    let surface = semantic_surface(lines);
    TerminalModelFrame::from_surface(&surface, None)
}

#[test]
fn wheel_without_mouse_grab_scrolls_local_viewport() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");

    let latest = session.surface_state(Uuid::new_v4());
    session.scroll_viewport_lines(2);
    let scrolled = session.surface_state(Uuid::new_v4());

    assert!(!latest.mouse_grabbed);
    assert_ne!(latest.visible_lines, scrolled.visible_lines);
    assert!(scrolled.visible_lines.iter().any(|line| line == "3"));
}

#[test]
fn experimental_alacritty_core_preserves_local_scrollback_contract() {
    let mut session =
        TerminalSession::new_with_core_kind(4, 20, TerminalCoreKind::AlacrittyExperimental);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");

    let latest = session.surface_state(Uuid::new_v4());
    session.scroll_viewport_lines(2);
    let scrolled = session.surface_state(Uuid::new_v4());

    assert!(!latest.mouse_grabbed);
    assert_ne!(latest.visible_lines, scrolled.visible_lines);
    assert!(scrolled.visible_lines.iter().any(|line| line == "3"));
}

#[test]
fn wheel_with_mouse_grab_is_forwarded_as_remote_mouse_input() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.apply_remote_bytes(b"\x1b[?1000h\x1b[?1002h\x1b[?1006h");

    let before = session.surface_state(Uuid::new_v4());
    let bytes = session
        .send_mouse_input(TerminalMouseInput {
            kind: TerminalMouseEventKind::Scroll,
            button: TerminalMouseButton::WheelUp,
            row: 3,
            col: 0,
            shift: false,
            ctrl: false,
            alt: false,
        })
        .expect("encode mouse wheel");
    let after = session.surface_state(Uuid::new_v4());

    assert!(before.mouse_grabbed);
    assert_eq!(before.visible_lines, after.visible_lines);
    assert_eq!(bytes, b"\x1b[<64;1;4M");
}

#[test]
fn remote_output_preserves_scrollback_view_when_user_is_not_at_bottom() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(2);
    let before = session.surface_state(Uuid::new_v4());
    session.apply_remote_bytes(b"7\r\n");

    let after = session.surface_state(Uuid::new_v4());

    assert!(!before.viewport_at_bottom);
    assert!(!after.viewport_at_bottom);
    assert_eq!(after.visible_lines, before.visible_lines);
    assert_eq!(
        after.viewport_offset_lines,
        before.viewport_offset_lines + 1
    );
    assert!(!after.visible_lines.iter().any(|line| line == "7"));
}

#[test]
fn remote_output_keeps_following_when_viewport_is_at_bottom() {
    let mut session = TerminalSession::new(4, 20);

    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    let before = session.surface_state(Uuid::new_v4());

    session.apply_remote_bytes(b"7\r\n");

    let after = session.surface_state(Uuid::new_v4());

    assert!(before.viewport_at_bottom);
    assert!(after.viewport_at_bottom);
    assert!(after.visible_lines.iter().any(|line| line == "7"));
}

#[test]
fn semantic_overlay_detects_json_blocks_over_normal_shell_output() {
    let frame = semantic_model_frame(&["{", "  \"name\": \"mica-term\"", "}"]);

    let spans = detect_output_block_spans(&frame);

    assert_eq!(spans.len(), 3);
    assert!(
        spans
            .iter()
            .all(|span| span.role == SemanticStyleRole::OutputJson)
    );
    assert_eq!(spans[0].row, 0);
    assert_eq!(spans[2].row, 2);
    assert_eq!(frame.rows[1].text, "  \"name\": \"mica-term\"");
}

#[test]
fn semantic_overlay_detects_xml_blocks_over_normal_shell_output() {
    let frame = semantic_model_frame(&["<root>", "  <item>mica-term</item>", "</root>"]);

    let spans = detect_output_block_spans(&frame);

    assert_eq!(spans.len(), 3);
    assert!(
        spans
            .iter()
            .all(|span| span.role == SemanticStyleRole::OutputXml)
    );
    assert_eq!(spans[0].row, 0);
    assert_eq!(spans[2].row, 2);
}

#[test]
fn semantic_overlay_detects_log_blocks_over_normal_shell_output() {
    let frame = semantic_model_frame(&[
        "[INFO] booting mica-term",
        "[WARN] startup fallback disabled",
        "[ERROR] native surface unavailable",
    ]);

    let spans = detect_output_block_spans(&frame);

    assert_eq!(spans.len(), 3);
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::OutputLevelInfo)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::OutputLevelWarn)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::OutputLevelError)
    );
}

#[test]
fn semantic_input_overlay_highlights_shell_prompt_command_option_and_argument() {
    let frame = semantic_model_frame(&["[root@host ~]$ cargo test --workspace"]);

    let spans = detect_input_line_spans(&frame);

    assert_eq!(spans.len(), 4);
    assert_eq!(spans[0].role, SemanticStyleRole::InputPrompt);
    assert_eq!(spans[1].role, SemanticStyleRole::InputCommand);
    assert_eq!(spans[2].role, SemanticStyleRole::InputArgument);
    assert_eq!(spans[3].role, SemanticStyleRole::InputOption);
    assert_eq!(spans[1].start_col, 15);
    assert_eq!(spans[1].end_col, 19);
    assert_eq!(spans[3].start_col, 26);
    assert_eq!(spans[3].end_col, 36);
}

#[test]
fn semantic_input_roles_cover_command_path_variable_and_operator() {
    let frame = semantic_model_frame(&["$ cargo run --bin app ./fixtures $HOME && echo done"]);

    let spans = detect_input_line_spans(&frame);

    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::InputCommand)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::InputOption)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::InputPath)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::InputVariable)
    );
    assert!(
        spans
            .iter()
            .any(|span| span.role == SemanticStyleRole::InputOperator)
    );
}

#[test]
fn semantic_input_overlay_is_disabled_in_alternate_screen_mode() {
    let mut surface = semantic_surface(&["$ cargo test --workspace"]);
    surface.alternate_screen_active = true;
    let frame = TerminalModelFrame::from_surface(&surface, None);

    let spans = detect_input_line_spans(&frame);

    assert!(spans.is_empty());
}

#[test]
fn semantic_input_overlay_is_disabled_when_tui_mouse_grab_is_active() {
    let mut surface = semantic_surface(&["$ cargo test --workspace"]);
    surface.mouse_grabbed = true;
    let frame = TerminalModelFrame::from_surface(&surface, None);

    let spans = detect_input_line_spans(&frame);

    assert!(spans.is_empty());
}

#[test]
fn semantic_analyzers_emit_roles_without_overlay_rgba_fields() {
    let input = fs::read_to_string("src/app/terminal_semantic/input_line.rs")
        .expect("read semantic input analyzer");
    let output = fs::read_to_string("src/app/terminal_semantic/output_blocks.rs")
        .expect("read semantic output analyzer");
    let shared = fs::read_to_string("src/app/terminal_semantic/types.rs")
        .expect("read semantic shared types");

    assert!(
        !input.contains("overlay_rgba"),
        "input analyzer should emit semantic roles instead of color overlays"
    );
    assert!(
        !output.contains("overlay_rgba"),
        "output analyzer should emit semantic roles instead of color overlays"
    );
    assert!(
        shared.contains("pub enum SemanticStyleRole"),
        "semantic shared types should expose semantic style roles"
    );
    assert!(
        shared.contains("pub struct SemanticSpan"),
        "semantic shared types should expose semantic spans"
    );
}

#[test]
fn renderer_migration_docs_describe_windows_native_status_and_native_only_shipping_path() {
    let readme = fs::read_to_string("readme.md").expect("read readme");
    let verification = fs::read_to_string("verification.md").expect("read verification");
    let mainline_build =
        fs::read_to_string("build-win-x64.sh").expect("read mainline build script");
    let software_build =
        fs::read_to_string("build-win-x64-software.sh").expect("read software build script");

    assert!(
        readme.contains("Windows-first native renderer"),
        "readme should document the current Windows-first native renderer status"
    );
    assert!(
        readme.contains("Linux/macOS"),
        "readme should document the pending Linux/macOS follow-up work"
    );
    assert!(
        verification.contains("bitmap fallback"),
        "verification notes should call out the bitmap fallback path"
    );
    assert!(
        verification.contains("Windows-first native renderer"),
        "verification notes should include the Windows-first native renderer verification status"
    );
    assert!(
        mainline_build.contains("MICA_TERM_PACKAGE_TERMINAL_RENDERER=\"native\""),
        "the primary Windows build wrapper should package the native terminal renderer"
    );
    assert!(
        software_build.contains("Native-first Windows software compatibility wrapper."),
        "the software compatibility wrapper should describe itself as the native-first compatibility path"
    );
    assert!(
        software_build.contains("MICA_TERM_PACKAGE_TERMINAL_RENDERER=\"native\""),
        "the software compatibility wrapper should package the native terminal renderer"
    );
}

#[test]
fn runtime_profile_source_keeps_windows_terminal_native_only() {
    let runtime_profile =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(
        !runtime_profile.contains("pub enum TerminalCompositionMode"),
        "runtime profile should stop exposing a terminal composition mode once Windows no longer supports multiple terminal subsystem paths"
    );
    assert!(
        !runtime_profile.contains("pub enum TerminalSubsystemMode"),
        "runtime profile should stop exposing a terminal subsystem mode once retained-native is the only supported Windows subsystem"
    );
    assert!(
        !runtime_profile.contains("MICA_TERM_TERMINAL_SUBSYSTEM"),
        "runtime profile should stop parsing runtime terminal subsystem overrides once the retired Windows software path is removed"
    );
    assert!(
        !runtime_profile.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"),
        "runtime profile should stop parsing packaged terminal subsystem overrides once the retired Windows software path is removed"
    );
    assert!(
        !runtime_profile.contains(&retired_windows_subsystem::retired_subsystem_match_expr()),
        "runtime profile should stop accepting the retired Windows software path as a supported terminal subsystem"
    );
}
