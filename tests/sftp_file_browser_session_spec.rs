use mica_term::app::sftp::{FileBrowserSession, HostProfileRef};

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
