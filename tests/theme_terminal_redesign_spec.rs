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
fn premium_default_v2_dark_palette_uses_blue_black_surface_and_soft_fg() {
    let spec = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0x0f_16_1d);
    assert_eq!(spec.terminal.background.base, 0x08_13_1d);
    assert_eq!(spec.terminal.foreground.default, 0xd7_e0_e8);
    assert_eq!(spec.terminal.ansi[4], 0x7f_9e_c4);
}

#[test]
fn premium_default_v2_light_palette_uses_mist_surface_and_charcoal_fg() {
    let spec = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);

    assert_eq!(spec.shell.app_background, 0xe8_ed_f1);
    assert_eq!(spec.terminal.background.base, 0xf4_f6_f8);
    assert_eq!(spec.terminal.foreground.default, 0x1f_29_33);
    assert_eq!(spec.terminal.ansi[4], 0x56_7c_a8);
}
