use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher;
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::session_manager::{SessionHandle, SessionState};
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalSurfaceState};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::ShellViewModel;
use slint::Model;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone, Default)]
struct FakeLauncher;

struct NoopRuntimeControl;

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_input(&self, _bytes: Vec<u8>) -> Result<()> {
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
        _session_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
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
fn active_tab_layout_preserves_close_hit_target_and_elides_text() {
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert!(
        active_tab.contains("min-width: 0px;"),
        "title/subtitle container should opt into shrink-safe layout"
    );
    assert!(
        active_tab.contains("overflow: elide;"),
        "tab text should elide instead of overflowing into the close hit target"
    );
    assert!(
        !active_tab.contains("width: 16px;"),
        "close affordance should expose a larger hit target than the old 16px box"
    );
    assert!(
        !tabbar.contains("width: 216px;"),
        "tab strip should not hard-code a fixed tab width"
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
        rows: 24,
        cols: 80,
        visible_lines: vec!["last login: Tue Mar 24".into(), "pwd".into()],
    }));

    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert_eq!(view_model.workspace_session_host_mode(), "terminal");
    assert!(view_model.workspace_terminal_surface_ready());
    assert_eq!(view_model.workspace_terminal_surface_seqno(), 3);
    assert!(
        view_model
            .workspace_terminal_visible_lines()
            .iter()
            .any(|line| line.contains("last login"))
    );
    assert!(
        terminal_host.contains("Interactive terminal ready."),
        "terminal host should present an interactive-ready state once a terminal snapshot exists"
    );
    assert!(
        !terminal_host.contains("Remote shell is ready but has not produced output yet."),
        "terminal host should stop rendering placeholder copy once a real terminal surface contract exists"
    );
}

#[test]
fn terminal_session_host_exposes_text_key_and_resize_callbacks() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window.contains("callback workspace-session-text-input(string);"),
        "AppWindow should expose a workspace text-input callback for the active terminal session"
    );
    assert!(
        app_window.contains("callback workspace-session-key-input(string, bool, bool, bool);"),
        "AppWindow should expose a workspace key-input callback for non-printable terminal keys"
    );
    assert!(
        app_window.contains("callback workspace-session-resize-requested(int, int);"),
        "AppWindow should expose a workspace resize callback for terminal surfaces"
    );
    assert!(
        workspace_pane.contains("text-input(text) =>"),
        "WorkspacePane should forward printable terminal input back to the app shell"
    );
    assert!(
        workspace_pane.contains("key-input(key, alt, ctrl, shift) =>"),
        "WorkspacePane should forward named key input back to the app shell"
    );
    assert!(
        workspace_pane.contains("surface-resize-requested(rows, cols) =>"),
        "WorkspacePane should forward terminal resize events back to the app shell"
    );
    assert!(
        terminal_host.contains("callback text-input(string);"),
        "TerminalSessionHost should emit printable text input"
    );
    assert!(
        terminal_host.contains("callback key-input(string, bool, bool, bool);"),
        "TerminalSessionHost should emit named key input with modifier state"
    );
    assert!(
        terminal_host.contains("callback surface-resize-requested(int, int);"),
        "TerminalSessionHost should emit resize requests with terminal rows/cols"
    );
    assert!(
        !terminal_host.contains("Remote shell is ready but has not produced output yet."),
        "interactive terminal host should stop rendering the old placeholder copy"
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
fn single_click_only_selects_saved_asset_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_selected(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("selected ssh row")
            .selected
    );
}

#[test]
fn double_click_and_open_in_new_tab_create_distinct_sessions() {
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
