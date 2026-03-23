use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;

#[test]
fn folder_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-folder".into());
    app.set_asset_folder_modal_name("Infra".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Infra");
}

#[test]
fn ssh_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_active_tab("proxy".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "proxy");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
}

#[test]
fn ssh_modal_resets_to_standard_english_shell_when_reopened() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_tab_selected("proxy".into());
    app.invoke_close_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
}

#[test]
fn rename_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_rename_modal_open(true);
    app.set_asset_rename_modal_name("Prod".into());
    app.set_asset_rename_modal_validation_message("Duplicate name".into());
    app.set_asset_rename_modal_can_confirm(false);

    assert!(app.get_asset_rename_modal_open());
    assert_eq!(app.get_asset_rename_modal_name().as_str(), "Prod");
    assert_eq!(
        app.get_asset_rename_modal_validation_message().as_str(),
        "Duplicate name"
    );
    assert!(!app.get_asset_rename_modal_can_confirm());
}

#[test]
fn delete_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_delete_confirm_modal_open(true);
    app.set_asset_delete_confirm_target_label("Prod".into());
    app.set_asset_delete_confirm_descendant_count(3);

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(app.get_asset_delete_confirm_target_label().as_str(), "Prod");
    assert_eq!(app.get_asset_delete_confirm_descendant_count(), 3);
}
