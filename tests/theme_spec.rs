use mica_term::theme::{ThemeMode, theme_spec};

#[test]
fn dark_theme_matches_tinted_console_rules() {
    let spec = theme_spec(ThemeMode::Dark);
    assert!(spec.terminal_is_neutral);
    assert!(spec.panel_uses_tint);
    assert_eq!(spec.accent_name, "electric-blue");
}

#[test]
fn light_theme_is_supported_from_day_one() {
    let spec = theme_spec(ThemeMode::Light);
    assert!(spec.supports_light);
    assert!(spec.supports_dark);
}
