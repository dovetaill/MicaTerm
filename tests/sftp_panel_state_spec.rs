use mica_term::app::sftp::{
    SftpFollowMode, SftpPanelMode, SftpPathHistory, SftpSessionBindingState,
};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn panel_mode_transitions_cover_connecting_loading_ready_and_disconnected() {
    let mut state = SftpSessionBindingState::default();

    assert_eq!(state.mode, SftpPanelMode::Empty);

    state.mark_connecting();
    assert_eq!(state.mode, SftpPanelMode::Connecting);

    state.mark_loading();
    assert_eq!(state.mode, SftpPanelMode::Loading);

    state.mark_ready();
    assert_eq!(state.mode, SftpPanelMode::Ready);

    state.mark_disconnected();
    assert_eq!(state.mode, SftpPanelMode::Disconnected);
}

#[test]
fn manual_browse_breaks_follow_mode_until_reenabled() {
    let mut state = SftpSessionBindingState::follow("/srv/app");

    assert_eq!(state.follow_mode, SftpFollowMode::FollowCwd);
    assert_eq!(state.current_path, "/srv/app");

    state.navigate_manual("/srv/app/releases");
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
    assert_eq!(state.current_path, "/srv/app/releases");

    state.follow_terminal_path("/srv/app/current");
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
    assert_eq!(state.current_path, "/srv/app/releases");

    state.reenable_follow("/srv/app/current");
    assert_eq!(state.follow_mode, SftpFollowMode::FollowCwd);
    assert_eq!(state.current_path, "/srv/app/current");
}

#[test]
fn path_history_supports_back_forward_and_push() {
    let mut history = SftpPathHistory::with_initial("/srv/app");

    history.push("/srv/app/releases");
    history.push("/srv/app/shared");

    assert_eq!(history.current(), Some("/srv/app/shared"));
    assert_eq!(history.back(), Some("/srv/app/releases"));
    assert_eq!(history.back(), Some("/srv/app"));
    assert_eq!(history.forward(), Some("/srv/app/releases"));

    history.push("/srv/app/logs");

    assert_eq!(history.current(), Some("/srv/app/logs"));
    assert_eq!(history.forward(), None);
}

#[test]
fn shell_view_model_exposes_raw_sftp_state_containers() {
    let view_model = ShellViewModel::default();

    assert!(view_model.sftp_sessions.is_empty());
    assert_eq!(view_model.sftp_queue_summary.active_count, 0);
    assert_eq!(view_model.sftp_queue_summary.failed_count, 0);
    assert_eq!(view_model.sftp_queue_summary.current_session_count, 0);
}
