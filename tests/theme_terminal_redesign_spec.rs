use mica_term::app::ui_preferences::UiPreferences;
use mica_term::theme::{ThemeMode, ThemeVariant, app_theme_spec};

#[test]
fn theme_terminal_redesign_spec_app_theme_spec_exposes_premium_default_and_legacy_variant() {
    let premium = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let legacy = app_theme_spec(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_eq!(premium.variant, ThemeVariant::PremiumDefault);
    assert_eq!(legacy.variant, ThemeVariant::LegacyHackerGreen);
    assert_ne!(
        premium.terminal.background.base,
        legacy.terminal.background.base
    );
}

#[test]
fn theme_terminal_redesign_spec_ui_preferences_round_trip_persists_theme_variant() {
    let prefs = UiPreferences {
        theme_variant: ThemeVariant::PremiumDefault,
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: UiPreferences = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.theme_variant, ThemeVariant::PremiumDefault);
}

#[test]
fn theme_terminal_redesign_spec_premium_default_dark_theme_uses_the_approved_ayu_shell_neighborhood()
 {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0x0a_0e14);
    assert_eq!(spec.shell.titlebar_background, 0x10_151d);
    assert_eq!(spec.shell.tabbar_background, 0x10_151d);
    assert_eq!(spec.shell.sidebar_background, 0x10_151d);
    assert_eq!(spec.shell.sidebar_panel_background, 0x11_1821);
    assert_eq!(spec.shell.right_panel_background, 0x11_1821);
    assert_eq!(spec.shell.terminal_frame_background, 0x14_1b24);
    assert_eq!(spec.shell.separator, 0x18_212b);
    assert_eq!(spec.shell.border, 0x1b_2530);
    assert_eq!(spec.shell.sidebar_item_focus_border, 0x1b_2530);
    assert_eq!(spec.shell.text_primary, 0xc5_c1b8);
    assert_eq!(spec.shell.text_secondary, 0x9a_a4ae);
    assert_eq!(spec.shell.text_muted, 0x7d_8790);
    assert_eq!(spec.shell.accent, 0xe6_b450);
    assert_eq!(spec.shell.sidebar_item_selected, 0x14_1b24);
    assert_eq!(spec.shell.sidebar_item_selected_border, 0xe6_b450);
    assert_eq!(spec.shell.sidebar_item_hover, 0x11_1821);
    assert_eq!(spec.shell.panel_scrollbar_track, 0x11_1821);
    assert_eq!(spec.shell.panel_scrollbar_thumb, 0x2f_3944);
    assert_eq!(spec.shell.panel_scrollbar_thumb_active, 0x3c_4856);
}

#[test]
fn theme_terminal_redesign_spec_premium_default_light_theme_uses_the_approved_ayu_shell_neighborhood()
 {
    let spec = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0xf8_f9fa);
    assert_eq!(spec.shell.titlebar_background, 0xf8_f9fa);
    assert_eq!(spec.shell.tabbar_background, 0xf8_f9fa);
    assert_eq!(spec.shell.sidebar_background, 0xf8_f9fa);
    assert_eq!(spec.shell.sidebar_panel_background, 0xf4_f6f8);
    assert_eq!(spec.shell.right_panel_background, 0xf4_f6f8);
    assert_eq!(spec.shell.terminal_frame_background, 0xf4_f6f8);
    assert_eq!(spec.shell.separator, 0xe5_e9ef);
    assert_eq!(spec.shell.border, 0xe1_e6ec);
    assert_eq!(spec.shell.sidebar_item_focus_border, 0xe5_e9ef);
    assert_eq!(spec.shell.text_primary, 0x5c_6166);
    assert_eq!(spec.shell.text_secondary, 0x7a_838c);
    assert_eq!(spec.shell.text_muted, 0x8a_939c);
    assert_eq!(spec.shell.accent, 0xff_aa33);
    assert_eq!(spec.shell.sidebar_item_selected, 0xff_f7ea);
    assert_eq!(spec.shell.sidebar_item_selected_border, 0xff_aa33);
    assert_eq!(spec.shell.sidebar_item_hover, 0xee_f2f5);
    assert_eq!(spec.shell.panel_scrollbar_track, 0xf4_f6f8);
    assert_eq!(spec.shell.panel_scrollbar_thumb, 0xd6_dce3);
    assert_eq!(spec.shell.panel_scrollbar_thumb_active, 0xc6_cdd6);
}

#[test]
fn theme_terminal_redesign_spec_legacy_hacker_green_keeps_the_same_shell_chrome_but_swaps_terminal_palette()
 {
    let premium = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    let legacy = app_theme_spec(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);

    assert_eq!(
        legacy.shell.titlebar_background,
        premium.shell.titlebar_background
    );
    assert_eq!(
        legacy.shell.tabbar_background,
        premium.shell.tabbar_background
    );
    assert_eq!(
        legacy.shell.sidebar_background,
        premium.shell.sidebar_background
    );
    assert_ne!(
        legacy.terminal.background.base,
        premium.terminal.background.base
    );
    assert_ne!(
        legacy.terminal.foreground.default,
        premium.terminal.foreground.default
    );
}
