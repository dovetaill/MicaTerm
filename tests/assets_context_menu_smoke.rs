//! Smoke coverage for the bootstrap-level assets context menu state bridge.

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::ComponentHandle;
use slint::Model;
use slint::PhysicalSize;

#[test]
fn bootstrap_exposes_context_menu_closed_by_default() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_assets_context_menu_open());
    assert_eq!(app.get_assets_context_menu_anchor_x(), 0.0);
    assert_eq!(app.get_assets_context_menu_anchor_y(), 0.0);
    assert_eq!(app.get_context_menu_feedback_text().as_str(), "");
}

#[test]
fn right_click_request_opens_context_menu_and_sets_anchor() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested(
        "ssh-prod-01".into(),
        "ssh".into(),
        144.0,
        188.0,
    );

    assert!(app.get_assets_context_menu_open());
    assert_eq!(app.get_assets_context_menu_anchor_x(), 144.0);
    assert_eq!(app.get_assets_context_menu_anchor_y(), 188.0);
}

#[test]
fn bootstrap_starts_with_empty_console_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn right_click_request_populates_primary_menu_title() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested(
        "ssh-prod-01".into(),
        "ssh".into(),
        144.0,
        188.0,
    );

    assert!(app.get_assets_context_menu_open());
    assert_eq!(app.get_assets_context_menu_primary_title().as_str(), "操作");
    assert!(app.get_assets_context_menu_primary_items().row_count() > 0);
}

#[test]
fn blank_area_right_click_opens_minimal_primary_menu() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let primary = app.get_assets_context_menu_primary_items();
    let ids: Vec<String> = (0..primary.row_count())
        .filter_map(|index| primary.row_data(index))
        .map(|item| item.id.to_string())
        .collect();

    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
    assert_eq!(app.get_assets_context_menu_secondary_items().row_count(), 0);
}

#[test]
fn create_menu_action_projects_placeholder_item_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());

    let items = app.get_console_asset_items();
    assert_eq!(items.row_count(), 1);
    assert_eq!(items.row_data(0).unwrap().label.as_str(), "New Folder");
}

#[test]
fn rename_commit_round_trips_through_window_callbacks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_rename_text_changed(asset_id.clone().into(), "Prod Bastion".into());
    app.invoke_asset_rename_commit_requested(asset_id.into(), "Prod Bastion".into());

    assert_eq!(
        app.get_console_asset_items().row_data(0).unwrap().label.as_str(),
        "Prod Bastion"
    );
}

#[test]
fn right_click_near_edge_still_keeps_overlay_within_window_bounds() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.window().set_size(PhysicalSize::new(760, 640));
    app.invoke_shell_layout_invalidated(760.0, 640.0);

    app.invoke_asset_context_menu_requested(
        "".into(),
        "blank".into(),
        748.0,
        632.0,
    );

    let size = app.window().size();
    let origin_x = app.get_layout_assets_context_menu_origin_x();
    let origin_y = app.get_layout_assets_context_menu_origin_y();
    let overlay_width = app.get_layout_assets_context_menu_width();
    let overlay_height = app.get_layout_assets_context_menu_height();

    assert!(origin_x >= 0.0);
    assert!(origin_y >= 0.0);
    assert!(origin_x + overlay_width <= size.width as f32);
    assert!(origin_y + overlay_height <= size.height as f32);
}

#[test]
fn invoking_planned_action_shows_status_pill_feedback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested(
        "ssh-prod-01".into(),
        "ssh".into(),
        144.0,
        188.0,
    );
    app.invoke_assets_context_menu_action_invoked("proxy-chrome-via-server".into());

    assert!(app.get_assets_context_menu_open());
    assert_eq!(
        app.get_context_menu_feedback_text().as_str(),
        "Proxy Chrome via Server is not wired yet."
    );
}
