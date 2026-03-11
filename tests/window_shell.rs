use mica_term::app::windowing::{
    MaterialKind, next_maximize_state, window_appearance, window_command_spec,
};
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn balanced_desktop_metrics_match_the_design_doc() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::TAB_BAR_HEIGHT, 38);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
}

#[test]
fn window_shell_prefers_frameless_mica_alt() {
    let appearance = window_appearance();
    assert!(appearance.no_frame);
    assert_eq!(appearance.material, MaterialKind::MicaAlt);
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
