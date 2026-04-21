use mica_term::app::terminal_theme::{
    preset_for_theme, preset_for_theme_mode, selection_overlay_rgba, selection_overlay_rgba_for_theme,
};
use mica_term::theme::{ThemeMode, ThemeVariant};
use std::fs;

#[test]
fn dark_theme_maps_terminal_palette_to_premium_default_graphite() {
    let preset = preset_for_theme_mode(ThemeMode::Dark);

    assert_eq!(preset.name, "Mica Graphite");
    assert_eq!(preset.background, 0x08_131d);
    assert_eq!(preset.foreground, 0xd7_e0e8);
    assert_eq!(preset.cursor_bg, 0xdc_e6ee);
    assert_eq!(preset.cursor_fg, 0x08_131d);
    assert_eq!(preset.scrollbar_thumb, (0x5a, 0x6a, 0x79));
    assert_eq!(preset.scrollbar_thumb_active, (0x72, 0x84, 0x95));
    assert_eq!(preset.split, (0x2d, 0x3a, 0x48));
    assert_eq!(preset.ansi[4], (0x7f, 0x9e, 0xc4));
}

#[test]
fn light_theme_maps_terminal_palette_to_premium_default_mist() {
    let preset = preset_for_theme_mode(ThemeMode::Light);

    assert_eq!(preset.name, "Mica Canvas");
    assert_eq!(preset.background, 0xf4_f6f8);
    assert_eq!(preset.foreground, 0x1f_2933);
    assert_eq!(preset.cursor_bg, 0x24_313c);
    assert_eq!(preset.cursor_fg, 0xf4_f6f8);
    assert_eq!(preset.scrollbar_thumb, (0xb6, 0xc0, 0xca));
    assert_eq!(preset.scrollbar_thumb_active, (0x9f, 0xac, 0xb8));
    assert_eq!(preset.split, (0xc9, 0xd3, 0xdd));
    assert_eq!(preset.ansi[4], (0x56, 0x7c, 0xa8));
}

#[test]
fn selection_overlay_colors_stay_translucent_and_theme_specific() {
    let dark_preset = preset_for_theme_mode(ThemeMode::Dark);
    let light_preset = preset_for_theme_mode(ThemeMode::Light);
    let dark_overlay = selection_overlay_rgba(ThemeMode::Dark);
    let light_overlay = selection_overlay_rgba(ThemeMode::Light);

    assert!(
        dark_preset.selection_bg.3 > 0.0 && dark_preset.selection_bg.3 < 1.0,
        "dark terminal selection should stay translucent so selected glyphs remain readable"
    );
    assert!(
        light_preset.selection_bg.3 > 0.0 && light_preset.selection_bg.3 < 1.0,
        "light terminal selection should stay translucent so selected glyphs remain readable"
    );
    assert_ne!(
        dark_overlay & 0x00ff_ffff,
        light_overlay & 0x00ff_ffff,
        "dark and light selection overlays should keep distinct RGB colors"
    );
    assert!(
        ((dark_overlay >> 24) & 0xff) < 0xff && ((light_overlay >> 24) & 0xff) < 0xff,
        "selection overlays should avoid a fully opaque alpha channel"
    );
}

#[test]
fn legacy_hacker_green_variant_projects_distinct_terminal_palette() {
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);
    let overlay = selection_overlay_rgba_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_eq!(preset.name, "Legacy Hacker Green");
    assert_eq!(preset.background, 0x05_0b08);
    assert_eq!(preset.foreground, 0x9b_e6b3);
    assert_eq!(preset.cursor_bg, 0xb4_f0c6);
    assert_eq!(preset.cursor_fg, 0x05_0b08);
    assert_eq!(preset.ansi[2], (0x73, 0xc0, 0x8c));
    assert_eq!(overlay & 0x00ff_ffff, 0x3f7a57);
}

#[test]
fn slint_terminal_tokens_match_shared_no_frame_defaults() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");

    assert!(
        tokens.contains("terminal-default-fg: legacy-hacker-green"),
        "Slint no-frame terminal foreground tokens should match the shared Mica Graphite/Canvas defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains("terminal-default-bg: legacy-hacker-green"),
        "Slint no-frame terminal background tokens should match the shared Mica Graphite/Canvas defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains("terminal-cursor-fg: legacy-hacker-green")
            && tokens.contains("terminal-cursor-bg: legacy-hacker-green"),
        "Slint cursor tokens should stay aligned with the terminal fallback preset so no-frame terminal states do not drift from the live terminal palette"
    );
    assert!(
        !tokens.contains("terminal-jump-to-latest"),
        "terminal fallback tokens should not keep styling for a removed jump-to-latest pill"
    );
}

#[test]
fn terminal_session_host_reads_terminal_shell_chrome_from_session_properties() {
    let host_source =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        host_source.contains("in property <color> session-scrollbar-thumb")
            && host_source.contains("in property <color> session-scrollbar-thumb-active"),
        "terminal session host should accept scrollbar thumb colors from the projected terminal session contract instead of hard-coding ThemeTokens values"
    );
    assert!(
        host_source.contains("? root.session-scrollbar-thumb-active")
            && host_source.contains(": root.session-scrollbar-thumb;"),
        "terminal session host should render the scrollbar thumb states directly from session-scoped shell chrome properties"
    );
    assert!(
        !host_source.contains("session-jump-to-latest")
            && !host_source.contains("jump-to-latest-requested();"),
        "terminal session host should stop exposing jump-to-latest shell chrome after removing that affordance"
    );
}

#[test]
fn terminal_adjacent_shell_chrome_contracts_match_shared_preset_values() {
    let theme_spec = fs::read_to_string("src/theme/spec.rs").expect("read terminal theme spec");
    let terminal_theme =
        fs::read_to_string("src/app/terminal_theme.rs").expect("read terminal theme preset code");
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !theme_spec.contains("jump_to_latest") && !terminal_theme.contains("jump_to_latest"),
        "the shared terminal theme structs should drop jump-to-latest palette fields once the shell affordance is removed"
    );
    assert!(
        !tokens.contains("terminal-jump-to-latest"),
        "Slint fallback tokens should not retain removed jump-to-latest palette entries"
    );
    assert!(
        terminal_host.contains(
            "in property <color> session-scrollbar-thumb: ThemeTokens.terminal-scrollbar-thumb-surface;"
        ) && terminal_host.contains(
            "in property <color> session-scrollbar-thumb-active: ThemeTokens.terminal-scrollbar-thumb-active-surface;"
        ) && terminal_host.contains("? root.session-scrollbar-thumb-active")
            && terminal_host.contains(": root.session-scrollbar-thumb;")
            && !terminal_host.contains("session-jump-to-latest"),
        "TerminalSessionHost should keep terminal-specific scrollbar chrome while dropping the removed jump-to-latest contract"
    );
}
