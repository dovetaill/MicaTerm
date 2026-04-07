use mica_term::app::terminal_theme::{preset_for_theme_mode, selection_overlay_rgba};
use mica_term::theme::ThemeMode;

#[test]
fn dark_theme_maps_terminal_palette_to_catppuccin_mocha() {
    let preset = preset_for_theme_mode(ThemeMode::Dark);

    assert_eq!(preset.name, "Catppuccin Mocha");
    assert_eq!(preset.background, 0x1e_1e2e);
    assert_eq!(preset.foreground, 0xcd_d6f4);
    assert_eq!(preset.cursor_bg, 0xcd_d6f4);
    assert_eq!(preset.cursor_fg, 0x1e_1e2e);
    assert_eq!(preset.scrollbar_thumb, (0x58, 0x5b, 0x70));
    assert_eq!(preset.split, (0x31, 0x32, 0x44));
    assert_eq!(preset.ansi[4], (0x89, 0xb4, 0xfa));
}

#[test]
fn light_theme_maps_terminal_palette_to_catppuccin_latte() {
    let preset = preset_for_theme_mode(ThemeMode::Light);

    assert_eq!(preset.name, "Catppuccin Latte");
    assert_eq!(preset.background, 0xef_f1f5);
    assert_eq!(preset.foreground, 0x4c_4f69);
    assert_eq!(preset.cursor_bg, 0x4c_4f69);
    assert_eq!(preset.cursor_fg, 0xef_f1f5);
    assert_eq!(preset.scrollbar_thumb, (0xac, 0xb0, 0xbe));
    assert_eq!(preset.split, (0xcc, 0xd0, 0xda));
    assert_eq!(preset.ansi[4], (0x1e, 0x66, 0xf5));
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
