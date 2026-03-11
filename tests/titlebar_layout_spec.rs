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

#[test]
fn top_status_bar_tooltip_budget_matches_bugfix3_overlay_design() {
    assert_eq!(ShellMetrics::TITLEBAR_TOOLTIP_DELAY_MS, 280);
    assert_eq!(ShellMetrics::TITLEBAR_TOOLTIP_CLOSE_DEBOUNCE_MS, 80);
    assert_eq!(ShellMetrics::TITLEBAR_TOOLTIP_OFFSET_Y, 8);
    assert!(ShellMetrics::TITLEBAR_TOOLTIP_MIN_WIDTH >= 96);
}
