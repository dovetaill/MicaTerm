use mica_term::app::sftp::{FileBrowserSession, HostProfileRef};
use mica_term::app::sftp::{SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode};

#[test]
fn cloned_workspace_browser_inherits_host_and_path_but_not_shared_selection() {
    let quick = FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");

    let expanded = quick.clone_for_workspace();

    assert_eq!(expanded.host_profile_ref, quick.host_profile_ref);
    assert_eq!(expanded.current_path, "/srv/app");
    assert_ne!(
        expanded.file_browser_session_id,
        quick.file_browser_session_id
    );
    assert!(expanded.selected_entry_ids.is_empty());
}

#[test]
fn cloned_workspace_browser_keeps_snapshot_but_resets_follow_and_request_identity() {
    let mut quick =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    quick.follow_mode = SftpFollowMode::FollowCwd;
    quick.active_request_id = Some(77);
    quick.entries = vec![SftpDirectoryEntry {
        id: "entry-logs".into(),
        name: "logs".into(),
        path: "/srv/app/logs".into(),
        kind: SftpDirectoryEntryKind::Directory,
        modified_unix_seconds: None,
        size_bytes: None,
    permissions_label: None,
    owner_label: None,
    group_label: None,
    }];

    let expanded = quick.clone_for_workspace();

    assert_eq!(expanded.current_path, "/srv/app");
    assert_eq!(expanded.entries, quick.entries);
    assert_eq!(
        expanded.follow_mode,
        SftpFollowMode::ManualBrowse,
        "workspace clones should freeze the promoted path instead of continuing to follow terminal cwd updates"
    );
    assert_eq!(
        expanded.active_request_id, None,
        "workspace clones must start with a fresh async identity so they can issue their own refresh generation"
    );
}
