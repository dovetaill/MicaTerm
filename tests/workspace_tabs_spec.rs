use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ssh::session_manager::{SessionHandle, SessionState};
use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::ShellViewModel;
use slint::Model;
use uuid::Uuid;

fn sample_handle(title: &str, subtitle: &str, state: SessionState) -> SessionHandle {
    SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: "asset-prod".into(),
        title: title.into(),
        subtitle: subtitle.into(),
        state,
        can_reconnect: false,
    }
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

#[test]
fn tab_model_prefers_asset_name_then_host_for_title() {
    let named = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connecting,
    ));
    let unnamed = WorkspaceTab::from_session(&sample_handle(
        "",
        "ops@example.com:22",
        SessionState::Connecting,
    ));

    assert_eq!(named.title, "Prod Bastion");
    assert_eq!(unnamed.title, "example.com");
}

#[test]
fn tab_model_tracks_active_session_and_closeability() {
    let first = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let second = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Connecting,
    ));

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![first.clone(), second.clone()]);

    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(first.session_id.as_str())
    );
    assert!(view_model.active_workspace_session_can_close());

    view_model.activate_workspace_session(second.session_id.as_str());

    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(second.session_id.as_str())
    );
    assert!(!view_model.show_welcome);
}

#[test]
fn disconnected_session_stays_visible_and_can_reconnect() {
    let mut disconnected = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Disconnected,
    ));
    disconnected.active = true;

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![disconnected.clone()]);

    assert_eq!(view_model.workspace_tabs().len(), 1);
    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());
}

#[test]
fn workspace_tab_projection_does_not_encode_container_width_behavior() {
    let tabs_projection = fs::read_to_string("src/shell/tabs.rs").expect("read tabs projection");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        !tabs_projection.contains("216px"),
        "workspace tab projection should not encode fixed container widths"
    );
    assert!(
        workspace_pane.contains("export component WorkspacePane"),
        "workspace width behavior should live in WorkspacePane instead of the tab projection"
    );
    assert!(
        workspace_pane.contains("TabBar {"),
        "WorkspacePane should own the tab strip contract"
    );
    assert!(
        workspace_pane.contains("TerminalSessionHost {"),
        "WorkspacePane should own the content host contract"
    );
}

#[test]
fn closing_active_tab_falls_back_to_right_then_left_then_welcome() {
    let first = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let second = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Connected,
    ));
    let third = WorkspaceTab::from_session(&sample_handle(
        "Dev Bastion",
        "ops@dev.example.com:22",
        SessionState::Connected,
    ));

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![first.clone(), second.clone(), third.clone()]);
    view_model.activate_workspace_session(second.session_id.as_str());

    assert!(view_model.close_workspace_session(second.session_id.as_str()));
    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(third.session_id.as_str()),
        "closing the active tab should activate the tab on the right first"
    );

    assert!(view_model.close_workspace_session(third.session_id.as_str()));
    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(first.session_id.as_str()),
        "closing the rightmost active tab should fall back to the left tab"
    );

    assert!(view_model.close_workspace_session(first.session_id.as_str()));
    assert_eq!(view_model.active_workspace_session_id(), None);
    assert!(view_model.show_welcome);
    assert_eq!(view_model.workspace_session_host_mode(), "welcome");
}

#[test]
fn close_affordance_is_modeled_separately_from_select_action() {
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");

    assert!(
        active_tab.contains("callback selected();"),
        "ActiveTab should expose a dedicated select callback"
    );
    assert!(
        active_tab.contains("callback close-requested();"),
        "ActiveTab should expose a dedicated close callback"
    );
    assert!(
        active_tab.contains("clicked => { root.close-requested(); }"),
        "close affordance should be wired independently instead of relying on the root select action"
    );
}

#[test]
fn connected_session_projects_terminal_surface_state_without_placeholder_copy() {
    let handle = sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    );
    let tab = WorkspaceTab::from_session(&handle);
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![tab]);
    view_model.set_active_workspace_terminal_surface(Some(TerminalSurfaceState {
        session_id: handle.session_id,
        seqno: 3,
        screen_text: "last login: Tue Mar 24".into(),
    }));

    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert_eq!(view_model.workspace_session_host_mode(), "terminal");
    assert!(view_model.workspace_terminal_surface_ready());
    assert_eq!(view_model.workspace_terminal_surface_seqno(), 3);
    assert!(
        view_model
            .workspace_terminal_screen_text()
            .contains("last login")
    );
    assert!(
        terminal_host.contains("terminal-surface-ready"),
        "terminal host should expose a ready-state marker once a terminal snapshot exists"
    );
    assert!(
        !terminal_host.contains("Renderer host is reserved for the terminal surface."),
        "terminal host should stop rendering placeholder copy once a real terminal surface contract exists"
    );
}

#[test]
fn disconnected_and_error_tabs_remain_reconnectable() {
    let disconnected = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Disconnected,
    ));
    let errored = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Error("authentication failed".into()),
    ));
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![disconnected.clone(), errored.clone()]);

    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());

    assert!(view_model.activate_workspace_session(errored.session_id.as_str()));
    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());
    assert!(
        tabbar.contains("connecting"),
        "tabbar should carry the connecting state contract through to the tab items"
    );
    assert!(
        tabbar.contains("error"),
        "tabbar should carry the error state contract through to the tab items"
    );
}

#[test]
fn reopening_same_asset_activates_existing_session_by_default() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_selected(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_selected(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_active_workspace_session_id().as_str(), first_session_id);
}

#[test]
fn explicit_open_in_new_tab_creates_second_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_selected(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-in-new-tab".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(app.get_active_workspace_session_id().as_str(), second_session_id);
}
