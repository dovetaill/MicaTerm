use mica_term::shell::metrics::ShellMetrics;

#[test]
fn top_status_bar_layout_preserves_drag_and_action_budget() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_ACTIONS_WIDTH, 120);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
    assert_eq!(ShellMetrics::TITLEBAR_MIN_DRAG_WIDTH, 96);
}
