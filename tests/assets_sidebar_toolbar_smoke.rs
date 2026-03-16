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
    assert!(!app.get_asset_create_menu_open());
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
