use mica_term::app::window_effects::{
    BackdropApplyStatus, BackdropPreference, NativeWindowTheme, WindowAppearanceSyncReport,
    build_native_window_appearance_request, default_platform_window_effects,
};
use mica_term::app::windowing::window_appearance;
use mica_term::theme::ThemeMode;

#[test]
fn dark_theme_maps_to_dark_native_theme_and_alt_mica_backdrop() {
    let request = build_native_window_appearance_request(ThemeMode::Dark, window_appearance());

    assert_eq!(request.theme, NativeWindowTheme::Dark);
    assert_eq!(request.backdrop, BackdropPreference::MicaAlt);
    assert!(request.request_redraw);
}

#[test]
fn light_theme_maps_to_light_native_theme_and_alt_mica_backdrop() {
    let request = build_native_window_appearance_request(ThemeMode::Light, window_appearance());

    assert_eq!(request.theme, NativeWindowTheme::Light);
    assert_eq!(request.backdrop, BackdropPreference::MicaAlt);
    assert!(request.request_redraw);
}

#[test]
fn skipped_sync_report_is_explicit() {
    let report = WindowAppearanceSyncReport::skipped();

    assert!(!report.theme_applied);
    assert_eq!(report.backdrop_status, BackdropApplyStatus::Skipped);
    assert!(!report.redraw_requested);
}

#[test]
fn default_platform_window_effects_is_constructible() {
    let _ = default_platform_window_effects();
}
