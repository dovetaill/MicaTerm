use mica_term::app::terminal_theme::{
    preset_for_theme, preset_for_theme_mode, selection_overlay_rgba, selection_overlay_rgba_for,
};
use mica_term::theme::{ThemeMode, ThemeVariant};
use std::fs;

#[test]
fn terminal_theme_dark_theme_maps_terminal_palette_to_ayu_dark() {
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(preset.name, "Ayu Dark");
    assert_eq!(preset.background, 0x0a_0e14);
    assert_eq!(preset.foreground, 0xc5_c1b8);
    assert_eq!(preset.viewport_bg_top, 0x0a_0e14);
    assert_eq!(preset.viewport_bg_bottom, 0x0a_0e14);
    assert_eq!(preset.cursor_bg, 0xe6_b450);
    assert_eq!(preset.cursor_fg, 0x0a_0e14);
    assert_eq!(preset.selection_bg, (0x2a, 0x35, 0x41, 0.78));
    assert_eq!(preset.scrollbar_track, (0x11, 0x18, 0x21));
    assert_eq!(preset.scrollbar_thumb, (0x2f, 0x39, 0x44));
    assert_eq!(preset.scrollbar_thumb_active, (0x3c, 0x48, 0x56));
    assert_eq!(preset.ansi[0], (0x01, 0x06, 0x0e));
    assert_eq!(preset.ansi[7], (0xc7, 0xc7, 0xc7));
    assert_eq!(preset.ansi[8], (0x68, 0x68, 0x68));
    assert_eq!(preset.ansi[15], (0xff, 0xff, 0xff));
}

#[test]
fn terminal_theme_background_constants_follow_the_ayu_migration_targets() {
    let dark = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(dark.background, 0x0a_0e14);
    assert_eq!(dark.foreground, 0xc5_c1b8);
    assert_eq!(dark.viewport_bg_top, 0x0a_0e14);
    assert_eq!(dark.viewport_bg_bottom, 0x0a_0e14);
    assert_eq!(dark.cursor_bg, 0xe6_b450);
    assert_eq!(light.background, 0xf7_f8fa);
    assert_eq!(light.viewport_bg_top, 0xf7_f8fa);
    assert_eq!(light.viewport_bg_bottom, 0xf7_f8fa);
}

#[test]
fn terminal_theme_light_theme_maps_terminal_palette_to_ayu_light() {
    let preset = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(preset.name, "Ayu Light");
    assert_eq!(preset.background, 0xf7_f8fa);
    assert_eq!(preset.foreground, 0x5c_6166);
    assert_eq!(preset.viewport_bg_top, 0xf7_f8fa);
    assert_eq!(preset.viewport_bg_bottom, 0xf7_f8fa);
    assert_eq!(preset.cursor_bg, 0xff_aa33);
    assert_eq!(preset.cursor_fg, 0xf7_f8fa);
    assert_eq!(preset.selection_bg, (0x55, 0xb4, 0xd4, 0.20));
    assert_eq!(preset.scrollbar_track, (0xf4, 0xf6, 0xf8));
    assert_eq!(preset.scrollbar_thumb, (0xd6, 0xdc, 0xe3));
    assert_eq!(preset.scrollbar_thumb_active, (0xc6, 0xcd, 0xd6));
    assert_eq!(preset.ansi[0], (0x00, 0x00, 0x00));
    assert_eq!(preset.ansi[7], (0xc7, 0xc7, 0xc7));
    assert_eq!(preset.ansi[8], (0x68, 0x68, 0x68));
    assert_eq!(preset.ansi[15], (0xd1, 0xd1, 0xd1));
}

#[test]
fn default_theme_mode_wrapper_points_at_ayu_default() {
    let wrapped = preset_for_theme_mode(ThemeMode::Dark);
    let explicit = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(wrapped.name, "Ayu Dark");
    assert_eq!(wrapped.background, explicit.background);
    assert_eq!(wrapped.foreground, explicit.foreground);
    assert_eq!(wrapped.cursor_bg, explicit.cursor_bg);
    assert_eq!(wrapped.cursor_fg, explicit.cursor_fg);
    assert_eq!(wrapped.selection_bg, explicit.selection_bg);
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
        "Slint no-frame terminal foreground tokens should match the shared Ayu default terminal preset used by the Rust fallback projection"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-default-bg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.background),
            hex_rgb(light_preset.background),
        )) || tokens.contains("terminal-default-bg: terminal-canvas-surface;"),
        "Slint no-frame terminal background tokens should match the shared Ayu default terminal preset used by the Rust fallback projection"
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
        tokens.contains(&format!(
            "terminal-selection-surface: dark-mode ? {} : {};",
            hex_rgba(dark_preset.selection_bg),
            hex_rgba(light_preset.selection_bg),
        )),
        "Slint selection tokens should stay aligned with the shared Ayu preset so bitmap host overlays do not drift from Rust-side selection colors"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-scrollbar-track-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_track),
            hex_rgb_tuple(light_preset.scrollbar_track),
        )) && tokens.contains(&format!(
            "terminal-scrollbar-thumb-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_thumb),
            hex_rgb_tuple(light_preset.scrollbar_thumb),
        )) && tokens.contains(&format!(
            "terminal-scrollbar-thumb-active-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_thumb_active),
            hex_rgb_tuple(light_preset.scrollbar_thumb_active),
        )),
        "Slint scrollbar chrome tokens should stay aligned with the shared Ayu preset so no-frame terminal states do not drift from the live shell chrome palette"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-frame-background: dark-mode ? {} : {};",
            hex_rgb(dark_preset.frame_bg),
            hex_rgb(light_preset.frame_bg),
        )),
        "Slint terminal frame tokens should stay aligned with the shared Ayu preset so workspace chrome does not drift from the terminal split/frame palette"
    );
    assert!(
        !tokens.contains("terminal-jump-to-latest"),
        "terminal fallback tokens should not keep styling for a removed jump-to-latest pill"
    );
}

#[test]
fn boot_time_terminal_tokens_match_approved_ayu_defaults() {
    let tokens = fs::read_to_string("ui/theme/tokens.slint").expect("read theme tokens");
    let dark_preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let light_preset = preset_for_theme(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert!(
        tokens.contains(&format!(
            "terminal-canvas-surface: dark-mode ? {} : {};",
            hex_rgb(dark_preset.background),
            hex_rgb(light_preset.background),
        )),
        "boot-time terminal canvas tokens should match the approved Ayu dark/light viewport backgrounds before Rust publishes the runtime palette"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-default-fg: dark-mode ? {} : {};",
            hex_rgb(dark_preset.foreground),
            hex_rgb(light_preset.foreground),
        )),
        "boot-time terminal foreground tokens should match the approved Ayu dark/light defaults before runtime projection takes over"
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
        "boot-time cursor tokens should match the approved Ayu dark/light cursor colors before runtime projection takes over"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-selection-surface: dark-mode ? {} : {};",
            hex_rgba(dark_preset.selection_bg),
            hex_rgba(light_preset.selection_bg),
        )),
        "boot-time selection tokens should match the approved Ayu dark/light overlay colors before runtime projection takes over"
    );
    assert!(
        tokens.contains(&format!(
            "terminal-scrollbar-track-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_track),
            hex_rgb_tuple(light_preset.scrollbar_track),
        )) && tokens.contains(&format!(
            "terminal-scrollbar-thumb-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_thumb),
            hex_rgb_tuple(light_preset.scrollbar_thumb),
        )) && tokens.contains(&format!(
            "terminal-scrollbar-thumb-active-surface: dark-mode ? {} : {};",
            hex_rgb_tuple(dark_preset.scrollbar_thumb_active),
            hex_rgb_tuple(light_preset.scrollbar_thumb_active),
        )),
        "boot-time scrollbar tokens should match the approved Ayu dark/light terminal chrome before runtime projection takes over"
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
fn projected_shell_palette_keeps_selected_focus_and_panel_scrollbar_semantics_on_one_runtime_path()
{
    let projection_source =
        fs::read_to_string("src/app/terminal_theme.rs").expect("read terminal theme");

    for field in [
        "pub sidebar_item_focus_border: u32,",
        "pub panel_scrollbar_track: u32,",
        "pub panel_scrollbar_thumb: u32,",
        "pub panel_scrollbar_thumb_active: u32,",
        "sidebar_item_focus_border: spec.shell.sidebar_item_focus_border,",
        "panel_scrollbar_track: spec.shell.panel_scrollbar_track,",
        "panel_scrollbar_thumb: spec.shell.panel_scrollbar_thumb,",
        "panel_scrollbar_thumb_active: spec.shell.panel_scrollbar_thumb_active,",
    ] {
        assert!(
            projection_source.contains(field),
            "projected theme preset should carry `{field}` so selected/focus rows and shell panel scrollbars stay on the shared runtime palette path"
        );
    }
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

fn hex_rgb_tuple((red, green, blue): (u8, u8, u8)) -> String {
    format!("#{:02x}{:02x}{:02x}", red, green, blue)
}

fn hex_rgba((red, green, blue, alpha): (u8, u8, u8, f32)) -> String {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", red, green, blue, alpha)
}
