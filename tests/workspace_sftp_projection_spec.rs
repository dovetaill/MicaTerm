use std::fs;

use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::{RightPanelView, ShellViewModel};
use uuid::Uuid;

fn sample_handle(title: &str, subtitle: &str, state: SessionState) -> SessionHandle {
    SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: "asset-prod".into(),
        title: title.into(),
        subtitle: subtitle.into(),
        state,
        can_reconnect: false,
        enhanced_session_state: EnhancedSessionState::Plain,
    }
}

#[test]
fn active_workspace_identity_tracks_tab_id_instead_of_terminal_session_id() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab.clone(), sftp_tab.clone()]);
    view_model.set_active_workspace_terminal_surface(Some(
        TerminalSurfaceState::from_visible_lines(
            Uuid::parse_str(terminal_tab.session_id.as_str()).expect("terminal session uuid"),
            1,
            24,
            80,
            vec!["pwd".into()],
        ),
    ));

    assert_eq!(
        view_model.active_workspace_tab_id(),
        Some(terminal_tab.tab_id.as_str())
    );
    assert_eq!(
        view_model.active_workspace_terminal_session_id(),
        Some(terminal_tab.session_id.as_str())
    );

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));
    assert_eq!(
        view_model.active_workspace_tab_id(),
        Some(sftp_tab.tab_id.as_str())
    );
    assert_eq!(view_model.workspace_session_host_mode(), "sftp");
    assert!(view_model.active_workspace_terminal_session_id().is_none());
    assert!(
        view_model.active_workspace_terminal_surface().is_none(),
        "switching to an sftp tab should stop projecting the previous terminal surface"
    );
}

#[test]
fn closing_active_sftp_tab_falls_back_like_any_other_workspace_tab() {
    let first_terminal = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let second_terminal = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Connected,
    ));
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![
        first_terminal.clone(),
        sftp_tab.clone(),
        second_terminal.clone(),
    ]);

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));
    assert!(view_model.close_workspace_tab(sftp_tab.tab_id.as_str()));
    assert_eq!(
        view_model.active_workspace_tab_id(),
        Some(second_terminal.tab_id.as_str())
    );
    assert_eq!(
        view_model.active_workspace_terminal_session_id(),
        Some(second_terminal.session_id.as_str())
    );
}

#[test]
fn active_sftp_workspace_policy_hides_duplicate_sftp_panel_without_forgetting_user_preference() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab.clone(), sftp_tab.clone()]);
    view_model.toggle_right_panel();

    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
    assert!(view_model.right_panel_can_revive());

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));

    assert_eq!(
        view_model.right_panel_display_policy_id(),
        "policy-hidden-sftp-workspace"
    );
    assert!(view_model.show_right_panel);
    assert!(
        !view_model.requested_right_panel(),
        "policy-hidden workspace tabs should release right-panel width without clearing the user's remembered open preference"
    );
    assert!(
        !view_model.right_panel_can_revive(),
        "policy-hidden workspace tabs should not offer a revive affordance for the duplicate quick browser"
    );

    assert!(view_model.activate_workspace_tab(terminal_tab.tab_id.as_str()));
    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
}

#[test]
fn active_sftp_workspace_only_policy_hides_the_sftp_right_panel_view() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab, sftp_tab.clone()]);
    view_model.toggle_right_panel();
    view_model.set_right_panel_view(RightPanelView::Appearance);

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));

    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
    assert!(view_model.right_panel_can_revive());
}

#[test]
fn right_panel_policy_hidden_contract_is_projected_for_active_sftp_workspace_tabs() {
    let view_model = fs::read_to_string("src/shell/view_model.rs").expect("read shell view model");
    let projection =
        fs::read_to_string("src/shell/view_model/projection.rs").expect("read projection");
    let shell_chrome =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        view_model.contains("RightPanelDisplayPolicy")
            && view_model.contains("PolicyHiddenForSftpWorkspace"),
        "view model should expose an explicit right-panel display policy enum so active SFTP workspace tabs can hide duplicate quick-browser lists without pretending the user collapsed them"
    );
    assert!(
        projection.contains("right_panel_display_policy")
            && projection.contains("right_panel_can_revive")
            && projection.contains("policy-hidden-sftp-workspace"),
        "projection should distinguish visible, user-collapsed, and policy-hidden SFTP workspace states"
    );
    assert!(
        shell_chrome.contains("window.set_right_panel_display_policy(")
            && shell_chrome.contains("window.set_right_panel_can_revive("),
        "bootstrap shell chrome sync should publish the right-panel display policy and revive capability into Slint"
    );
    assert!(
        app_window.contains("in-out property <string> right-panel-display-policy: \"visible\";")
            && app_window.contains("in-out property <bool> right-panel-can-revive: true;")
            && app_window.contains(
                "if !root.effective-show-right-panel && root.right-panel-can-revive : right-panel-revive-strip := Rectangle {"
            ),
        "AppWindow should thread policy-hidden SFTP workspace semantics into the right-panel revive-strip contract"
    );
}
