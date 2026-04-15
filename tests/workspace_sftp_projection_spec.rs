use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::ShellViewModel;
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
    view_model.set_active_workspace_terminal_surface(Some(TerminalSurfaceState::from_visible_lines(
        Uuid::parse_str(terminal_tab.session_id.as_str()).expect("terminal session uuid"),
        1,
        24,
        80,
        vec!["pwd".into()],
    )));

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
