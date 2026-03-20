use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

fn create_root_folder(app: &AppWindow, name: &str) -> String {
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed(name.into());
    app.invoke_confirm_asset_modal_requested();

    app.get_console_asset_items().row_data(0).unwrap().id.to_string()
}

fn create_child_ssh_via_context_menu(
    app: &AppWindow,
    parent_id: &str,
    name: &str,
    host: &str,
) {
    app.invoke_asset_context_menu_requested(parent_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_confirm_asset_modal_requested();
}

#[test]
fn created_folder_projects_depth_and_icon_metadata_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    create_root_folder(&app, "Folder 1");
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

    let folder_id = create_root_folder(&app, "Folder 1");
    create_child_ssh_via_context_menu(&app, &folder_id, "SSH Connection 1", "10.0.0.12");
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_toggle_expanded_requested(folder_id.into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_assets_search_query_changed("SSH Connection 1".into());
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_assets_search_query_changed("".into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
}

#[test]
fn flat_projection_rows_keep_path_hints_without_multiline_row_contract() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();

    let folder_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();
    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_confirm_asset_modal_requested();
    app.invoke_toggle_assets_view_mode_requested();

    let row = app.get_console_asset_items().row_data(0).unwrap();
    assert_eq!(row.label.as_str(), "Prod Bastion");
    assert_eq!(row.path_hint.as_str(), "Prod");
    assert!(row.compact_flat_mode);
}
