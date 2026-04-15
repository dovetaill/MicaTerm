use mica_term::app::sftp::{FileBrowserSession, HostProfileRef, SftpFollowMode};
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn quick_browser_and_workspace_browser_do_not_share_mutable_selection_or_sort_identity() {
    let mut quick = FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    quick.selected_entry_ids = vec!["entry-current".into()];
    quick.sort_state.column = Some(mica_term::app::sftp::FileBrowserSortColumn::Modified);
    quick.sort_state.direction = Some(mica_term::app::sftp::FileBrowserSortDirection::Desc);

    let expanded = quick.clone_for_workspace();

    assert_ne!(
        quick.file_browser_session_id,
        expanded.file_browser_session_id
    );
    assert_eq!(expanded.current_path, "/srv/app");
    assert_eq!(expanded.sort_state, quick.sort_state);
    assert!(expanded.selected_entry_ids.is_empty());
}

#[test]
fn active_sftp_projection_uses_quick_browser_until_workspace_sftp_tab_takes_over() {
    let quick = FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let workspace = quick.clone_for_workspace();

    let mut view_model = ShellViewModel::default();
    view_model.quick_browser_session_id = Some(quick.file_browser_session_id.clone());
    view_model
        .file_browser_sessions
        .insert(quick.file_browser_session_id.clone(), quick.clone());
    view_model
        .file_browser_sessions
        .insert(workspace.file_browser_session_id.clone(), workspace.clone());

    assert_eq!(
        view_model
            .active_sftp_session_state()
            .expect("quick browser session")
            .file_browser_session_id,
        quick.file_browser_session_id
    );

    let sftp_tab = WorkspaceTab::sftp(
        "tab-files-1",
        workspace.file_browser_session_id.clone(),
        "Files: Prod",
    );
    view_model.set_workspace_tabs(vec![sftp_tab.clone()]);
    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));

    assert_eq!(
        view_model
            .active_sftp_session_state()
            .expect("workspace browser session")
            .file_browser_session_id,
        workspace.file_browser_session_id
    );
}

#[test]
fn quick_browser_lock_mode_is_independent_from_follow_mode_inside_browser_session() {
    let mut view_model = ShellViewModel::default();
    let quick = FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let quick_id = quick.file_browser_session_id.clone();
    view_model.quick_browser_session_id = Some(quick_id.clone());
    view_model.file_browser_sessions.insert(quick_id, quick);

    view_model.quick_browser_state.follows_active_terminal = false;

    let state = view_model
        .active_sftp_session_state()
        .expect("quick browser state should remain accessible");
    assert!(!view_model.quick_browser_state.follows_active_terminal);
    assert_eq!(state.follow_mode, SftpFollowMode::FollowCwd);
}
