use mica_term::app::window_effects::{BackdropPreference, build_native_window_appearance_request};
use mica_term::app::windowing::{
    MaterialKind, next_maximize_state, window_appearance, window_command_spec,
};
use mica_term::shell::metrics::ShellMetrics;
use mica_term::theme::ThemeMode;

#[test]
fn balanced_desktop_metrics_match_the_design_doc() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::TAB_BAR_HEIGHT, 38);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
}

#[test]
fn sidebar_metrics_match_the_navigation_design() {
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_BUTTON_SIZE, 36);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_ICON_SIZE, 20);
}

#[test]
fn window_shell_prefers_frameless_mica_alt() {
    let appearance = window_appearance();
    assert!(appearance.no_frame);
    assert_eq!(appearance.material, MaterialKind::MicaAlt);
}

#[test]
fn window_shell_prefers_alt_mica_backdrop_for_both_themes() {
    let appearance = window_appearance();

    let dark = build_native_window_appearance_request(ThemeMode::Dark, appearance);
    let light = build_native_window_appearance_request(ThemeMode::Light, appearance);

    assert_eq!(dark.backdrop, BackdropPreference::MicaAlt);
    assert_eq!(light.backdrop, BackdropPreference::MicaAlt);
}

#[test]
fn top_status_bar_window_commands_match_the_approved_strategy() {
    let spec = window_command_spec();

    assert!(spec.uses_winit_drag);
    assert!(spec.self_drawn_controls);
    assert!(spec.supports_double_click_maximize);
    assert!(spec.supports_always_on_top);

    assert!(next_maximize_state(false));
    assert!(!next_maximize_state(true));
}
