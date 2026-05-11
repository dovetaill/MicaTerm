use mica_term::app::ui_preferences::UiPreferences;
use mica_term::theme::{ThemeMode, ThemeVariant, app_theme_spec};

#[test]
fn app_theme_spec_exposes_premium_default_and_legacy_variant() {
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
fn ui_preferences_round_trip_persists_theme_variant() {
    let prefs = UiPreferences {
        theme_variant: ThemeVariant::PremiumDefault,
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).unwrap();
    let decoded: UiPreferences = serde_json::from_str(&json).unwrap();

    assert_eq!(decoded.theme_variant, ThemeVariant::PremiumDefault);
}

#[test]
fn premium_default_dark_theme_uses_the_product_grade_calm_shell_ladder() {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0x14_1b23);
    assert_eq!(spec.shell.titlebar_background, 0x18_1f27);
    assert_eq!(spec.shell.tabbar_background, 0x1a_222c);
    assert_eq!(spec.shell.sidebar_background, 0x18_212b);
    assert_eq!(spec.shell.sidebar_panel_background, 0x1b_2430);
    assert_eq!(spec.shell.right_panel_background, 0x1c_2431);
    assert_eq!(spec.shell.terminal_frame_background, 0x11_151c);
    assert_eq!(spec.shell.separator, 0x26_303b);
    assert_eq!(spec.shell.border, 0x11_151c);
    assert_eq!(spec.shell.hairline, 0x3a_4857);
    assert_eq!(spec.shell.text_primary, 0xe6_ecf3);
    assert_eq!(spec.shell.text_secondary, 0xba_c4d0);
    assert_eq!(spec.shell.accent, 0x6f_8fb7);
    assert_eq!(spec.shell.tab_active, 0x22_3040);
    assert_eq!(spec.shell.tab_hover, 0x20_2b38);
    assert_eq!(spec.shell.sidebar_item_selected, 0x29_3846);
    assert_eq!(spec.terminal.background.base, 0x0a_0e14);
    assert_eq!(spec.terminal.foreground.default, 0xb3_b1ad);
    assert_eq!(spec.terminal.foreground.inactive, 0x82_8c99);
    assert_eq!(spec.decoration.warning, 0xff_b454);
}

#[test]
fn premium_default_light_theme_avoids_the_flat_white_sheet_look() {
    let spec = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0xf2_f5f8);
    assert_eq!(spec.shell.titlebar_background, 0xf7_f9fc);
    assert_eq!(spec.shell.tabbar_background, 0xee_f2f7);
    assert_eq!(spec.shell.sidebar_background, 0xeb_f0f5);
    assert_eq!(spec.shell.sidebar_panel_background, 0xf1_f5f9);
    assert_eq!(spec.shell.right_panel_background, 0xf3_f6fa);
    assert_eq!(spec.shell.terminal_frame_background, 0xe6_e9ef);
    assert_eq!(spec.shell.text_primary, 0x24_303d);
    assert_eq!(spec.shell.text_secondary, 0x49_586a);
    assert_eq!(spec.shell.tab_active_indicator, 0x63_88b4);
    assert_eq!(spec.terminal.background.base, 0xfa_fafa);
    assert_eq!(spec.terminal.foreground.default, 0x5c_6166);
    assert_eq!(spec.terminal.foreground.inactive, 0x6b_7480);
    assert_eq!(spec.decoration.info, 0x39_9ee6);
}

#[test]
fn legacy_hacker_green_keeps_the_same_shell_chrome_but_swaps_terminal_palette() {
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
