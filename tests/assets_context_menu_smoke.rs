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
fn bootstrap_exposes_mock_console_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let items = app.get_console_asset_items();
    let kinds: Vec<String> = (0..items.row_count())
        .filter_map(|index| items.row_data(index))
        .map(|item| item.kind.to_string())
        .collect();

    assert_eq!(items.row_count(), 3);
    assert!(kinds.iter().any(|kind| kind == "ssh"));
    assert!(kinds.iter().any(|kind| kind == "folder"));
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
fn hovering_or_selecting_new_connection_populates_secondary_column() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested(
        "ssh-prod-01".into(),
        "ssh".into(),
        144.0,
        188.0,
    );

    let primary_items = app.get_assets_context_menu_primary_items();
    let new_connection_index = (0..primary_items.row_count())
        .find(|index| {
            primary_items
                .row_data(*index)
                .map(|item| item.id.as_str() == "new-connection")
                .unwrap_or(false)
        })
        .expect("ssh primary menu should expose the new-connection row");

    app.invoke_assets_context_menu_row_hovered(0, new_connection_index as i32);

    let secondary_items = app.get_assets_context_menu_secondary_items();
    let secondary_ids: Vec<String> = (0..secondary_items.row_count())
        .filter_map(|index| secondary_items.row_data(index))
        .map(|item| item.id.to_string())
        .collect();

    assert_eq!(app.get_assets_context_menu_secondary_title().as_str(), "New Connection");
    assert_eq!(
        secondary_ids,
        vec!["ssh", "local-terminal", "serial", "telnet", "ssh-tunnel"]
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
