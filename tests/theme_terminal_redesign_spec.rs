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
