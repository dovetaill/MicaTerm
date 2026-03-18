use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

#[test]
fn created_folder_projects_depth_and_icon_metadata_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let row = app.get_console_asset_items().row_data(0).unwrap();

    assert_eq!(row.kind.as_str(), "folder");
    assert_eq!(row.depth, 0);
    assert!(!row.has_children);
}

#[test]
fn search_filters_rows_without_destroying_collapsed_tree_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let folder_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();
    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_toggle_expanded_requested(folder_id.into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_assets_search_query_changed("SSH Connection 1".into());
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_assets_search_query_changed("".into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
}
