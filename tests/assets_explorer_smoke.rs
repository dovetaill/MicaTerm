use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use i_slint_backend_testing::ElementHandle;
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher;
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::ComponentHandle;
use slint::Model;
use slint::platform::PointerEventButton;
use tokio::sync::mpsc;

#[derive(Clone, Default)]
struct FakeLauncher;

struct NoopRuntimeControl;

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

fn bind_with_fake_sessions(app: &AppWindow) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
    );
}

fn create_root_folder(app: &AppWindow, name: &str) -> String {
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed(name.into());
    app.invoke_confirm_asset_modal_requested();

    app.get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn create_child_ssh_via_context_menu(app: &AppWindow, parent_id: &str, name: &str, host: &str) {
    app.invoke_asset_context_menu_requested(parent_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) -> String {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    app.get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn context_menu_item_enabled(app: &AppWindow, action_id: &str) -> bool {
    let items = app.get_assets_context_menu_primary_items();
    (0..items.row_count())
        .filter_map(|index| items.row_data(index))
        .find(|item| item.id.as_str() == action_id)
        .map(|item| item.enabled)
        .unwrap_or(false)
}

fn click_element(app: &AppWindow, element: &ElementHandle) {
    let position = slint::LogicalPosition::new(
        element.absolute_position().x + element.size().width / 2.0,
        element.absolute_position().y + element.size().height / 2.0,
    );
    let window = app.window();
    window.dispatch_event(slint::platform::WindowEvent::PointerMoved { position });
    window.dispatch_event(slint::platform::WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    window.dispatch_event(slint::platform::WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

#[test]
fn created_folder_projects_depth_and_icon_metadata_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

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
    bind_with_fake_sessions(&app);

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
    bind_with_fake_sessions(&app);
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
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();
    app.invoke_toggle_assets_view_mode_requested();

    let row = app.get_console_asset_items().row_data(0).unwrap();
    assert_eq!(row.label.as_str(), "Prod Bastion");
    assert_eq!(row.path_hint.as_str(), "Prod");
    assert!(row.compact_flat_mode);
}

#[test]
fn single_clicking_ssh_asset_only_selects_it() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);

    app.invoke_asset_selected(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("ssh row")
            .id
            .as_str(),
        ssh_id.as_str()
    );
    assert!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("ssh row")
            .selected
    );
}

#[test]
fn second_click_on_same_ssh_asset_opens_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_selected(ssh_id.clone().into());
    app.invoke_asset_selected(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
}

#[test]
fn double_clicking_same_ssh_asset_twice_creates_two_distinct_tabs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn explicit_open_context_action_opens_distinct_tabs_each_time() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "open-connection"));
    app.invoke_assets_context_menu_action_invoked("open-connection".into());

    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn clicking_workspace_tabs_switches_active_session_and_close_hit_target_closes_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);

    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();

    let mut tabs = ElementHandle::find_by_element_type_name(&app, "ActiveTab").collect::<Vec<_>>();
    tabs.sort_by(|left, right| {
        left.absolute_position()
            .x
            .partial_cmp(&right.absolute_position().x)
            .expect("compare tab x positions")
    });
    assert_eq!(tabs.len(), 2, "expected two rendered ActiveTab instances");

    click_element(&app, &tabs[0]);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id
    );

    let mut close_targets =
        ElementHandle::find_by_element_id(&app, "ActiveTab::close-hit-target").collect::<Vec<_>>();
    close_targets.sort_by(|left, right| {
        left.absolute_position()
            .x
            .partial_cmp(&right.absolute_position().x)
            .expect("compare close hit target x positions")
    });
    assert_eq!(
        close_targets.len(),
        2,
        "expected close hit target for each rendered tab"
    );

    click_element(&app, &close_targets[0]);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn clicking_open_connection_row_from_context_menu_opens_new_tab_after_session_exists() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert!(app.get_assets_context_menu_open());

    let mut rows =
        ElementHandle::find_by_element_type_name(&app, "AssetsContextMenuRow").collect::<Vec<_>>();
    rows.sort_by(|top, bottom| {
        top.absolute_position()
            .y
            .partial_cmp(&bottom.absolute_position().y)
            .expect("compare context menu row y positions")
    });
    assert!(
        !rows.is_empty(),
        "expected at least one rendered AssetsContextMenuRow"
    );

    click_element(&app, &rows[0]);
    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert!(!app.get_assets_context_menu_open());
}

#[test]
fn ssh_context_menu_only_exposes_single_open_action() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);

    let primary = app.get_assets_context_menu_primary_items();
    let ids: Vec<String> = (0..primary.row_count())
        .filter_map(|index| primary.row_data(index))
        .map(|item| item.id.to_string())
        .collect();

    assert!(ids.contains(&"open-connection".to_string()));
    assert!(!ids.contains(&"open-in-new-tab".to_string()));
    assert!(!ids.contains(&"close-connection".to_string()));
}

#[test]
fn double_clicking_folder_toggles_expanded_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let folder_id = create_root_folder(&app, "Folder 1");
    create_child_ssh_via_context_menu(&app, &folder_id, "SSH Connection 1", "10.0.0.12");
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_asset_activated(folder_id.clone().into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_asset_activated(folder_id.into());
    assert_eq!(app.get_console_asset_items().row_count(), 2);
}

#[test]
fn second_click_on_same_folder_toggles_expanded_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let folder_id = create_root_folder(&app, "Folder 1");
    create_child_ssh_via_context_menu(&app, &folder_id, "SSH Connection 1", "10.0.0.12");
    assert_eq!(app.get_console_asset_items().row_count(), 2);

    app.invoke_asset_selected(folder_id.clone().into());
    app.invoke_asset_selected(folder_id.clone().into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_asset_selected(folder_id.clone().into());
    app.invoke_asset_selected(folder_id.into());
    assert_eq!(app.get_console_asset_items().row_count(), 2);
}

#[test]
fn blank_area_create_actions_still_work_after_opening_ssh_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-folder".into());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    app.invoke_close_asset_modal_requested();

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
}
