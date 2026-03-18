//! Smoke coverage for the assets sidebar toolbar defaults exposed during bootstrap.

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;

#[test]
fn bootstrap_initializes_assets_toolbar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_asset_view_mode().as_str(), "tree");
    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "");
    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_primary_create_tooltip().as_str(), "New SSH Connection");
    assert!(!app.get_asset_tree_fully_expanded());
}

#[test]
fn search_toggle_and_query_binding_round_trip() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_assets_search_query_changed("prod".into());
    assert_eq!(app.get_assets_search_query().as_str(), "prod");

    app.invoke_collapse_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_assets_search_query_changed("".into());
    app.invoke_collapse_assets_search_requested();
    assert!(!app.get_asset_search_expanded());
}

#[test]
fn close_assets_search_requested_hides_non_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_close_assets_search_requested();

    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "prod");
}

#[test]
fn view_mode_toggle_and_tree_expansion_follow_the_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_tree_expansion_requested();
    assert!(app.get_asset_tree_fully_expanded());

    app.invoke_toggle_assets_view_mode_requested();
    assert_eq!(app.get_asset_view_mode().as_str(), "flat");

    app.invoke_toggle_assets_tree_expansion_requested();
    assert!(app.get_asset_tree_fully_expanded());
}

#[test]
fn switching_destination_updates_toolbar_descriptor_projection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "new-ssh-connection");

    app.invoke_sidebar_destination_selected("snippets".into());
    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "new-snippet");
    assert_eq!(app.get_asset_primary_create_tooltip().as_str(), "New Snippet");
}

#[test]
fn search_row_occupies_height_only_when_expanded() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_layout_assets_search_row_height(), 0.0);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_layout_assets_search_row_height() >= 40.0);

    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_collapse_assets_search_requested();
    assert!(app.get_layout_assets_search_row_height() >= 40.0);

    app.invoke_close_assets_search_requested();
    assert_eq!(app.get_layout_assets_search_row_height(), 0.0);
}

#[test]
fn sidebar_destination_click_collapses_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_sidebar_destination_selected("snippets".into());
    assert!(!app.get_asset_search_expanded());
}

#[test]
fn context_menu_request_collapses_empty_search_but_keeps_non_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    assert!(!app.get_asset_search_expanded());

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    assert!(app.get_asset_search_expanded());
}
