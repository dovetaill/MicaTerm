//! Cross-module shell/window contract tests for appearance and command specifications.

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
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 320);
    assert_eq!(ShellMetrics::TAB_BAR_HEIGHT, 38);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
}

#[test]
fn sidebar_metrics_match_the_navigation_design() {
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 320);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_BUTTON_SIZE, 36);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_ICON_SIZE, 20);
}

#[test]
fn assets_toolbar_metrics_match_the_design_budget() {
    assert_eq!(ShellMetrics::ASSETS_TOOLBAR_HEIGHT, 44);
    assert_eq!(ShellMetrics::ASSETS_TOOLBAR_BUTTON_SIZE, 28);
    assert_eq!(ShellMetrics::ASSETS_SEARCH_ROW_HEIGHT, 40);
}

#[test]
fn shell_layout_metrics_match_the_layout_bugfix_budget() {
    assert_eq!(ShellMetrics::WINDOW_DEFAULT_WIDTH, 1440);
    assert_eq!(ShellMetrics::WINDOW_DEFAULT_HEIGHT, 900);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 320);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
    assert_eq!(ShellMetrics::MAIN_WORKSPACE_MIN_WIDTH, 640);
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

#[test]
fn top_status_bar_window_commands_match_the_windows_restore_strategy() {
    let spec = window_command_spec();

    assert!(spec.uses_winit_drag);
    assert!(spec.uses_winit_drag_resize);
    assert!(spec.supports_true_window_state_tracking);
    assert!(spec.supports_native_frame_adapter);
}

#[test]
fn window_shell_exposes_resize_border_for_frameless_resize() {
    let spec = window_command_spec();

    assert_eq!(spec.resize_border_width, 6);
}

#[test]
fn tab_bar_contract_requires_workspace_tab_model_instead_of_single_placeholder() {
    let content = std::fs::read_to_string("ui/shell/tabbar.slint").unwrap();

    assert!(content.contains("export struct WorkspaceTabItem {"));
    assert!(content.contains("in property <[WorkspaceTabItem]> items: [];"));
    assert!(content.contains("for item in root.items : ActiveTab {"));
}

#[test]
fn window_shell_exposes_minimum_window_budget() {
    let spec = window_command_spec();

    assert_eq!(spec.min_window_width, ShellMetrics::WINDOW_MIN_WIDTH);
    assert_eq!(spec.min_window_height, ShellMetrics::WINDOW_MIN_HEIGHT);
}

#[test]
fn semantic_surface_tokens_define_dual_theme_ladder() {
    let content = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    for token in [
        "window-surface",
        "titlebar-surface",
        "activity-surface",
        "assets-surface",
        "workspace-surface",
        "inspector-surface",
        "divider-subtle",
        "divider-strong",
        "control-hover-surface",
        "control-active-surface",
    ] {
        assert!(
            content.contains(token),
            "missing semantic token in ui/theme/tokens.slint: {token}"
        );
    }
}

#[test]
fn semantic_surface_tokens_lock_the_approved_dual_theme_values() {
    let content = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    for line in [
        "out property <brush> window-surface: dark-mode ? #171c24 : #f6f8fb;",
        "out property <brush> titlebar-surface: dark-mode ? #1b212be8 : #fbfcfeee;",
        "out property <brush> activity-surface: dark-mode ? #151b24 : #f2f5f9;",
        "out property <brush> assets-surface: dark-mode ? #181f29 : #f7f9fc;",
        "out property <brush> workspace-surface: dark-mode ? #10151d : #ffffff;",
        "out property <brush> inspector-surface: panel-surface;",
        "out property <brush> divider-subtle: dark-mode ? #ffffff12 : #0f172a14;",
        "out property <brush> divider-strong: dark-mode ? #ffffff24 : #0f172a26;",
    ] {
        assert!(
            content.contains(line),
            "missing approved token value: {line}"
        );
    }
}

#[test]
fn semantic_surface_tokens_remove_legacy_surface_aliases() {
    let content = std::fs::read_to_string("ui/theme/tokens.slint").unwrap();

    for legacy in [
        "out property <brush> shell-surface:",
        "out property <brush> shell-stroke:",
        "out property <brush> command-tint:",
        "out property <brush> panel-tint:",
        "out property <brush> terminal-surface:",
    ] {
        assert!(
            !content.contains(legacy),
            "legacy semantic alias should be removed from ui/theme/tokens.slint: {legacy}"
        );
    }
}
