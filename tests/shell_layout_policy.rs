use mica_term::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn layout_policy_keeps_full_shell_when_width_budget_is_sufficient() {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::WINDOW_DEFAULT_WIDTH,
        request_assets_sidebar: true,
        request_right_panel: true,
    });

    assert!(layout.show_assets_sidebar);
    assert!(layout.show_right_panel);
    assert!(layout.main_workspace_width >= ShellMetrics::MAIN_WORKSPACE_MIN_WIDTH);
}

#[test]
fn layout_policy_collapses_assets_sidebar_before_right_panel() {
    let collapse_assets = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::FULL_LAYOUT_MIN_WIDTH - 1,
        request_assets_sidebar: true,
        request_right_panel: true,
    });
    assert!(!collapse_assets.show_assets_sidebar);
    assert!(collapse_assets.show_right_panel);

    let collapse_right = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::RIGHT_PANEL_ONLY_MIN_WIDTH - 1,
        request_assets_sidebar: true,
        request_right_panel: true,
    });
    assert!(!collapse_right.show_assets_sidebar);
    assert!(!collapse_right.show_right_panel);
}

#[test]
fn layout_policy_preserves_requested_state_when_regions_are_not_requested() {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: ShellMetrics::WINDOW_DEFAULT_WIDTH,
        request_assets_sidebar: false,
        request_right_panel: false,
    });

    assert!(!layout.show_assets_sidebar);
    assert!(!layout.show_right_panel);
}
