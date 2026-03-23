//! Smoke coverage for the assets sidebar toolbar defaults exposed during bootstrap.

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

#[test]
fn bootstrap_initializes_assets_toolbar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_asset_view_mode().as_str(), "tree");
    assert!(!app.get_asset_search_expanded());
    assert_eq!(app.get_assets_search_query().as_str(), "");
    assert!(!app.get_asset_create_menu_open());
    assert!(app.get_asset_uses_create_popover());
    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "");
    assert_eq!(
        app.get_asset_primary_create_tooltip().as_str(),
        "Create Asset"
    );
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
fn create_menu_toggle_and_close_round_trip() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());

    app.invoke_close_assets_create_menu_requested();
    assert!(!app.get_asset_create_menu_open());
}

#[test]
fn assets_create_menu_anchor_is_exposed_at_root_window() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(app.get_layout_assets_create_menu_anchor_width() > 0.0);
    assert!(app.get_layout_assets_create_menu_anchor_height() > 0.0);
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
fn search_and_create_are_mutually_exclusive_in_window_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());
    assert!(!app.get_asset_create_menu_open());

    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());
    assert!(!app.get_asset_search_expanded());

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());
    assert!(!app.get_asset_create_menu_open());
}

#[test]
fn snippets_destination_keeps_direct_create_action_projection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_sidebar_destination_selected("snippets".into());
    assert!(!app.get_asset_uses_create_popover());
    assert_eq!(
        app.get_asset_primary_create_action_id().as_str(),
        "new-snippet"
    );
    assert_eq!(
        app.get_asset_primary_create_tooltip().as_str(),
        "New Snippet"
    );
}

#[test]
fn toolbar_create_action_stays_root_level_even_after_folder_context_target() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().depth, 0);
    assert_eq!(rows.row_data(1).unwrap().depth, 0);
    assert_eq!(rows.row_data(1).unwrap().kind.as_str(), "ssh");
}
