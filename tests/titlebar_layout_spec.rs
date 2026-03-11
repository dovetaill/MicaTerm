use mica_term::shell::metrics::ShellMetrics;

#[test]
fn top_status_bar_layout_matches_bugfix2_budget() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_NAV_WIDTH, 44);
    assert_eq!(ShellMetrics::TITLEBAR_BRAND_WIDTH, 188);
    assert_eq!(ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE, 36);
    assert_eq!(ShellMetrics::TITLEBAR_TOOL_ICON_SIZE, 20);
    assert!(ShellMetrics::TITLEBAR_UTILITY_WIDTH >= 136);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
}
