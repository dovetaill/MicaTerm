use mica_term::app::sftp::{
    SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode, SftpPathHistory,
    SftpSessionBindingState,
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

#[test]
fn shell_view_model_cycles_sftp_sort_state_and_restores_default_projection() {
    let mut view_model = ShellViewModel::default();
    let entries = vec![
        SftpDirectoryEntry {
            id: "file-zeta".into(),
            name: "zeta.log".into(),
            path: "/srv/app/zeta.log".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(3),
            size_bytes: Some(100),
        },
        SftpDirectoryEntry {
            id: "dir-app".into(),
            name: "app".into(),
            path: "/srv/app/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: Some(2),
            size_bytes: None,
        },
        SftpDirectoryEntry {
            id: "file-alpha".into(),
            name: "alpha.log".into(),
            path: "/srv/app/alpha.log".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(1),
            size_bytes: Some(10),
        },
    ];

    assert_eq!(view_model.sftp_panel_sort_column_id(), "default");
    assert_eq!(view_model.sftp_panel_sort_direction_id(), "none");
    assert_eq!(
        view_model
            .project_sftp_panel_entries(entries.as_slice())
            .into_iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["app", "alpha.log", "zeta.log"]
    );

    assert!(view_model.cycle_sftp_panel_sort("modified"));
    assert_eq!(view_model.sftp_panel_sort_column_id(), "modified");
    assert_eq!(view_model.sftp_panel_sort_direction_id(), "asc");
    assert_eq!(
        view_model
            .project_sftp_panel_entries(entries.as_slice())
            .into_iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["app", "alpha.log", "zeta.log"]
    );

    assert!(view_model.cycle_sftp_panel_sort("modified"));
    assert_eq!(view_model.sftp_panel_sort_column_id(), "modified");
    assert_eq!(view_model.sftp_panel_sort_direction_id(), "desc");
    assert_eq!(
        view_model
            .project_sftp_panel_entries(entries.as_slice())
            .into_iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["app", "zeta.log", "alpha.log"]
    );

    assert!(view_model.cycle_sftp_panel_sort("modified"));
    assert_eq!(view_model.sftp_panel_sort_column_id(), "default");
    assert_eq!(view_model.sftp_panel_sort_direction_id(), "none");
    assert_eq!(
        view_model
            .project_sftp_panel_entries(entries.as_slice())
            .into_iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["app", "alpha.log", "zeta.log"]
    );
}

#[test]
fn shell_view_model_clamps_sftp_column_widths_in_window_runtime_state() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.sftp_panel_name_column_width_px(), 226.0);
    assert_eq!(view_model.sftp_panel_type_column_width_px(), 78.0);
    assert_eq!(view_model.sftp_panel_modified_column_width_px(), 150.0);
    assert_eq!(view_model.sftp_panel_size_column_width_px(), 72.0);

    assert!(view_model.set_sftp_panel_column_width("name", 320.0));
    assert_eq!(view_model.sftp_panel_name_column_width_px(), 320.0);

    assert!(view_model.set_sftp_panel_column_width("type", 10.0));
    assert_eq!(view_model.sftp_panel_type_column_width_px(), 72.0);

    assert!(view_model.set_sftp_panel_column_width("modified", 40.0));
    assert_eq!(view_model.sftp_panel_modified_column_width_px(), 132.0);

    assert!(view_model.set_sftp_panel_column_width("size", 24.0));
    assert_eq!(view_model.sftp_panel_size_column_width_px(), 72.0);
}
