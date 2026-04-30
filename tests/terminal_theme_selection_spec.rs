use mica_term::app::terminal_theme::{
    preset_for_theme, preset_for_theme_mode, selection_overlay_rgba, selection_overlay_rgba_for,
};
use mica_term::theme::{ThemeMode, ThemeVariant};
use std::fs;

#[test]
fn dark_theme_maps_terminal_palette_to_premium_default_graphite() {
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(preset.name, "Mica Graphite");
    assert_eq!(preset.background, 0x0c_141c);
    assert_eq!(preset.foreground, 0xe3_eaf2);
    assert_eq!(preset.cursor_bg, 0xdc_e6f3);
    assert_eq!(preset.cursor_fg, 0x0c_141c);
    assert_eq!(preset.scrollbar_thumb, (0x53, 0x62, 0x74));
    assert_eq!(preset.scrollbar_thumb_active, (0x66, 0x78, 0x8e));
    assert_eq!(preset.split, (0x31, 0x3d, 0x4b));
    assert_eq!(preset.ansi[4], (0x7d, 0x9b, 0xc2));
}

#[test]
fn light_theme_maps_terminal_palette_to_premium_default_canvas() {
    let preset = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(preset.name, "Mica Canvas");
    assert_eq!(preset.background, 0xf8_fafc);
    assert_eq!(preset.foreground, 0x26_3240);
    assert_eq!(preset.cursor_bg, 0x2c_3948);
    assert_eq!(preset.cursor_fg, 0xf8_fafc);
    assert_eq!(preset.scrollbar_thumb, (0xb7, 0xc3, 0xd0));
    assert_eq!(preset.scrollbar_thumb_active, (0x9f, 0xaf, 0xbe));
    assert_eq!(preset.split, (0xc7, 0xd2, 0xde));
    assert_eq!(preset.ansi[4], (0x5b, 0x80, 0xae));
}

#[test]
fn default_theme_mode_wrapper_still_points_at_premium_default() {
    let wrapped = preset_for_theme_mode(ThemeMode::Dark);
    let explicit = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(wrapped.background, explicit.background);
    assert_eq!(wrapped.foreground, explicit.foreground);
}

#[test]
fn legacy_hacker_green_variant_only_swaps_terminal_projection_values() {
    let premium = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let legacy = preset_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_ne!(premium.background, legacy.background);
    assert_ne!(premium.foreground, legacy.foreground);
    assert_eq!(premium.split, legacy.split);
}

#[test]
fn selection_overlay_colors_stay_translucent_and_theme_specific() {
    let dark_preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light_preset = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);
    let dark_overlay = selection_overlay_rgba_for(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light_overlay = selection_overlay_rgba_for(ThemeMode::Light, ThemeVariant::PremiumDefault);

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

    assert_eq!(
        selection_overlay_rgba(ThemeMode::Dark),
        dark_overlay,
        "default helper should stay aligned with the Premium Default variant"
    );
}

#[test]
fn slint_terminal_tokens_match_shared_no_frame_defaults() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let dark_preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light_preset = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert!(
        tokens.contains(&format!(
            "terminal-default-fg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.foreground),
            hex_rgb(light_preset.foreground),
        )),
        "Slint no-frame terminal foreground tokens should match the shared Mica Graphite/Canvas defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-default-bg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.background),
            hex_rgb(light_preset.background),
        )) || tokens.contains("terminal-default-bg: terminal-canvas-surface;"),
        "Slint no-frame terminal background tokens should match the shared Mica Graphite/Canvas defaults used by the Rust fallback preset projection"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-cursor-fg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.cursor_fg),
            hex_rgb(light_preset.cursor_fg),
        )) && tokens.contains(&format!(
            "terminal-cursor-bg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.cursor_bg),
            hex_rgb(light_preset.cursor_bg),
        )),
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

fn hex_rgb(value: u32) -> String {
    format!("#{:06x}", value)
}
