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
fn blank_area_menu_projects_compact_overlay_height() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let overlay_height = app.get_layout_assets_context_menu_height();
    assert!(overlay_height > 0.0);
    assert!(overlay_height < 160.0);
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
fn create_menu_action_opens_folder_modal_without_placeholder_item() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn folder_modal_confirm_projects_root_row_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_modal_requested();

    assert_eq!(
        app.get_console_asset_items().row_data(0).unwrap().label.as_str(),
        "Infra"
    );
}

#[test]
fn folder_modal_cancel_and_reopen_resets_name_draft() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "");

    app.invoke_assets_create_action_selected("new-folder".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "");
}

#[test]
fn closing_folder_modal_resets_kind_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "");
}

#[test]
fn ssh_modal_cancel_and_reopen_resets_tab_and_draft_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_tab_selected("proxy".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_method".into(), "jump-host".into());
    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_proxy_method().as_str(), "");

    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_proxy_method().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "22");
}

#[test]
fn closing_ssh_modal_resets_kind_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
}

#[test]
fn folder_context_create_opens_child_targeted_ssh_modal() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(asset_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().id.as_str(), asset_id.as_str());
    assert_eq!(rows.row_data(1).unwrap().depth, 1);
    assert_eq!(rows.row_data(1).unwrap().kind.as_str(), "ssh");
}

#[test]
fn right_click_near_edge_still_keeps_overlay_within_window_bounds() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.window().set_size(PhysicalSize::new(760, 640));
    app.invoke_shell_layout_invalidated(760.0, 640.0);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 748.0, 632.0);

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

#[test]
fn closing_context_menu_clears_planned_action_feedback() {
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
    app.invoke_close_assets_context_menu_requested();

    assert!(!app.get_assets_context_menu_open());
    assert_eq!(app.get_context_menu_feedback_text().as_str(), "");
}

#[test]
fn pointer_move_callback_exists_for_context_menu_corridor() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_pointer_moved(120.0, 180.0);

    assert!(app.get_assets_context_menu_open());
}
