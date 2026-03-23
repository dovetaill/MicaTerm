use mica_term::app::ssh::session_manager::{SessionHandle, SessionState};
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
    }
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
