use std::fs;

use mica_term::app::ssh::runtime::{TerminalKeyEvent, TerminalSession};
use mica_term::app::terminal_core::TerminalCoreKind;
use mica_term::app::terminal_theme::preset_for_theme;
use mica_term::theme::{ThemeMode, ThemeVariant};
use uuid::Uuid;

fn parity_session(kind: TerminalCoreKind, rows: usize, cols: usize) -> TerminalSession {
    TerminalSession::new_with_core_kind(rows, cols, kind)
}

#[test]
fn repository_exposes_experimental_alacritty_core_selection_contract() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo manifest");
    let terminal_core_mod =
        fs::read_to_string("src/app/terminal_core/mod.rs").expect("read terminal_core mod");
    let runtime_terminal =
        fs::read_to_string("src/app/ssh/runtime/terminal.rs").expect("read runtime terminal");

    assert!(
        cargo_toml.contains("terminal-core-alacritty-experimental"),
        "Cargo.toml should expose an explicit experimental feature switch for the alacritty-style core adapter while parity is being evaluated"
    );
    assert!(
        terminal_core_mod.contains("alacritty_adapter"),
        "terminal_core module should export the experimental alacritty-style adapter alongside the wezterm control adapter"
    );
    assert!(
        runtime_terminal.contains("new_with_core_kind")
            && runtime_terminal.contains("TerminalCoreKind::AlacrittyExperimental"),
        "terminal runtime should expose an explicit core-selection seam so parity tests can drive the control and candidate adapters through the same session contract"
    );
}

#[test]
fn experimental_alacritty_core_binds_to_real_upstream_terminal_state() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read cargo manifest");
    let adapter_source = fs::read_to_string("src/app/terminal_core/alacritty_adapter.rs")
        .expect("read alacritty adapter source");

    assert!(
        cargo_toml.contains("alacritty_terminal"),
        "Cargo.toml should pull in the upstream alacritty_terminal crate once the experimental adapter stops proxying through wezterm state"
    );
    assert!(
        adapter_source.contains("alacritty_terminal"),
        "the alacritty adapter should reference the upstream alacritty terminal crate instead of staying as a rename-only seam"
    );
    assert!(
        !adapter_source.contains("inner: WeztermTerminalCoreAdapter"),
        "the experimental alacritty adapter should stop storing a WeztermTerminalCoreAdapter internally once the upstream core is wired in"
    );
    assert!(
        !adapter_source.contains("WeztermTerminalCoreAdapter::new"),
        "the experimental alacritty adapter should construct upstream alacritty terminal state instead of booting a nested wezterm adapter"
    );
}

#[test]
fn experimental_alacritty_core_matches_wezterm_for_viewport_cursor_and_selection_contracts() {
    let mut wezterm = parity_session(TerminalCoreKind::Wezterm, 4, 20);
    let mut alacritty = parity_session(TerminalCoreKind::AlacrittyExperimental, 4, 20);

    let script = b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n";
    wezterm.apply_remote_bytes(script);
    alacritty.apply_remote_bytes(script);
    wezterm.scroll_viewport_lines(2);
    alacritty.scroll_viewport_lines(2);

    let wezterm_surface = wezterm.surface_state(Uuid::new_v4());
    let alacritty_surface = alacritty.surface_state(Uuid::new_v4());
    let wezterm_frame = wezterm.frame_snapshot();
    let alacritty_frame = alacritty.frame_snapshot();

    assert_eq!(
        alacritty_surface.visible_lines,
        wezterm_surface.visible_lines
    );
    assert_eq!(alacritty_surface.cursor, wezterm_surface.cursor);
    assert_eq!(
        alacritty_surface.viewport_offset_lines,
        wezterm_surface.viewport_offset_lines
    );
    assert_eq!(
        alacritty_surface.viewport_max_offset_lines,
        wezterm_surface.viewport_max_offset_lines
    );
    assert_eq!(alacritty_frame.selection, wezterm_frame.selection);
}

#[test]
fn experimental_alacritty_core_matches_wezterm_for_truecolor_and_writeback_contracts() {
    let mut wezterm = parity_session(TerminalCoreKind::Wezterm, 4, 20);
    let mut alacritty = parity_session(TerminalCoreKind::AlacrittyExperimental, 4, 20);

    wezterm.set_theme_mode(ThemeMode::Light);
    alacritty.set_theme_mode(ThemeMode::Light);

    let color_script = b"\x1b[38;2;1;2;3mA\x1b[48;2;4;5;6mB\x1b[0m";
    wezterm.apply_remote_bytes(color_script);
    alacritty.apply_remote_bytes(color_script);

    let wezterm_surface = wezterm.surface_state(Uuid::new_v4());
    let alacritty_surface = alacritty.surface_state(Uuid::new_v4());
    let wezterm_writeback = wezterm
        .send_key_event(TerminalKeyEvent::named("tab", false, false, true))
        .expect("wezterm writeback");
    let alacritty_writeback = alacritty
        .send_key_event(TerminalKeyEvent::named("tab", false, false, true))
        .expect("alacritty writeback");

    assert_eq!(
        alacritty_surface.default_fg_rgba,
        wezterm_surface.default_fg_rgba
    );
    assert_eq!(
        alacritty_surface.default_bg_rgba,
        wezterm_surface.default_bg_rgba
    );
    assert_eq!(alacritty_surface.cells, wezterm_surface.cells);
    assert_eq!(alacritty_writeback, wezterm_writeback);
}

#[test]
fn experimental_alacritty_core_matches_wezterm_for_variant_aware_palette_projection() {
    let mut wezterm = parity_session(TerminalCoreKind::Wezterm, 4, 20);
    let mut alacritty = parity_session(TerminalCoreKind::AlacrittyExperimental, 4, 20);
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    let color_script = b"\x1b[32mA\x1b[34mB\x1b[0m";
    wezterm.apply_remote_bytes(color_script);
    alacritty.apply_remote_bytes(color_script);
    wezterm.set_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);
    alacritty.set_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    let wezterm_surface = wezterm.surface_state(Uuid::new_v4());
    let alacritty_surface = alacritty.surface_state(Uuid::new_v4());

    assert_eq!(
        alacritty_surface.default_fg_rgba,
        0xff00_0000 | preset.foreground
    );
    assert_eq!(
        alacritty_surface.default_bg_rgba,
        0xff00_0000 | preset.background
    );
    assert_eq!(
        alacritty_surface.default_fg_rgba,
        wezterm_surface.default_fg_rgba
    );
    assert_eq!(
        alacritty_surface.default_bg_rgba,
        wezterm_surface.default_bg_rgba
    );
    assert_eq!(alacritty_surface.cursor, wezterm_surface.cursor);
    assert_eq!(alacritty_surface.cells, wezterm_surface.cells);
}
