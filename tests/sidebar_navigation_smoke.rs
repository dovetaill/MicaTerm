//! End-to-end smoke coverage for sidebar navigation bindings in the Slint window.

use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::UiPreferencesStore;
use mica_term::shell::metrics::ShellMetrics;
use slint::{ComponentHandle, PhysicalSize};

#[test]
fn bootstrap_initializes_sidebar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-defaults.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn bootstrap_toggles_assets_sidebar_without_losing_destination() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-toggle.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_sidebar_destination_selected("snippets".into());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn selecting_destination_auto_expands_assets_sidebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-select.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());

    app.invoke_sidebar_destination_selected("keychain".into());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "keychain");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn clicking_active_destination_toggles_assets_sidebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-active-destination-toggle.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    app.invoke_sidebar_destination_selected("console".into());
    assert!(!app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    app.invoke_sidebar_destination_selected("console".into());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn collapsing_sidebar_hides_search_and_create_menu_bindings() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    assert!(app.get_asset_search_expanded());

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());
    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "prod");

    app.invoke_toggle_assets_sidebar_requested();
    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());

    app.invoke_sidebar_destination_selected("console".into());
    assert!(!app.get_show_assets_sidebar());
    assert!(!app.get_asset_create_menu_open());
}

#[test]
fn narrow_width_preserves_requested_right_panel_but_hides_it_effectively() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_right_panel_requested();
    app.window().set_size(PhysicalSize::new(1000, 900));
    app.invoke_shell_layout_invalidated(1000.0, 900.0);

    assert!(app.get_show_right_panel());
    assert!(!app.get_effective_show_assets_sidebar());
    assert!(!app.get_effective_show_right_panel());
}

#[test]
fn bootstrap_syncs_shell_panel_width_memory_into_window_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(
        app.get_assets_sidebar_expanded_width() as u32,
        ShellMetrics::ASSETS_SIDEBAR_DEFAULT_WIDTH
    );
    assert_eq!(
        app.get_right_panel_expanded_width() as u32,
        ShellMetrics::RIGHT_PANEL_DEFAULT_WIDTH
    );
}

#[test]
fn edge_handle_callbacks_resize_restore_and_collapse_shell_panels() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_sidebar_edge_drag_start_requested(
        ShellMetrics::ASSETS_SIDEBAR_DEFAULT_WIDTH as f32,
    );
    app.invoke_assets_sidebar_edge_drag_move_requested(368.0);
    app.invoke_assets_sidebar_edge_drag_end_requested(368.0);
    assert_eq!(app.get_assets_sidebar_expanded_width() as u32, 368);

    app.invoke_assets_sidebar_edge_toggle_requested();
    assert!(!app.get_show_assets_sidebar());

    app.invoke_assets_sidebar_edge_toggle_requested();
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_assets_sidebar_expanded_width() as u32, 368);

    app.invoke_right_panel_edge_toggle_requested();
    assert!(app.get_show_right_panel());

    app.invoke_right_panel_edge_drag_start_requested(
        ShellMetrics::RIGHT_PANEL_DEFAULT_WIDTH as f32,
    );
    app.invoke_right_panel_edge_drag_move_requested(452.0);
    app.invoke_right_panel_edge_drag_end_requested(452.0);
    assert_eq!(app.get_right_panel_expanded_width() as u32, 452);

    app.invoke_right_panel_edge_drag_start_requested(452.0);
    app.invoke_right_panel_edge_drag_move_requested(
        (ShellMetrics::RIGHT_PANEL_COLLAPSE_THRESHOLD - 1) as f32,
    );
    app.invoke_right_panel_edge_drag_end_requested(
        (ShellMetrics::RIGHT_PANEL_COLLAPSE_THRESHOLD - 1) as f32,
    );
    assert!(!app.get_show_right_panel());
    assert_eq!(app.get_right_panel_expanded_width() as u32, 452);

    app.invoke_right_panel_edge_toggle_requested();
    assert!(app.get_show_right_panel());
    assert_eq!(app.get_right_panel_expanded_width() as u32, 452);
}

#[test]
fn titlebar_focus_mode_action_hides_both_side_regions_and_restores_the_prior_layout() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_assets_sidebar());
    assert!(app.get_show_right_panel());

    app.invoke_toggle_workspace_focus_mode_requested();

    assert!(app.get_workspace_focus_mode());
    assert!(!app.get_show_assets_sidebar());
    assert!(!app.get_show_right_panel());

    app.invoke_toggle_workspace_focus_mode_requested();

    assert!(!app.get_workspace_focus_mode());
    assert!(app.get_show_assets_sidebar());
    assert!(app.get_show_right_panel());
}
