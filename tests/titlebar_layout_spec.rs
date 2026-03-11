use mica_term::shell::metrics::ShellMetrics;

#[test]
fn top_status_bar_layout_preserves_brand_utility_and_drag_budget() {
    let min_drag_width = ShellMetrics::TITLEBAR_MIN_DRAG_WIDTH;
    let utility_width = ShellMetrics::TITLEBAR_UTILITY_WIDTH;

    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::TITLEBAR_BRAND_WIDTH, 220);
    assert!(min_drag_width >= 96);
    assert!(utility_width >= 84);
    assert_eq!(ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH, 138);
}
