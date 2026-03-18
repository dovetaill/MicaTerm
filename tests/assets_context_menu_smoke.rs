use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

#[test]
fn folder_target_create_action_projects_child_row_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let folder_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();
    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(1).unwrap().depth, 1);
    assert_eq!(rows.row_data(1).unwrap().kind.as_str(), "ssh");
}
