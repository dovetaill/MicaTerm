use mica_term::app::sftp::{
    FileBrowserSession, HostProfileRef, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode,
};
use mica_term::shell::tabs::WorkspaceTabKind;
use mica_term::shell::view_model::ShellViewModel;
use uuid::Uuid;

#[test]
fn expanding_quick_browser_creates_an_active_sftp_workspace_tab_with_a_cloned_browser_session() {
    let mut view_model = ShellViewModel::default();
    let mut quick_browser = FileBrowserSession::quick_browser(
        HostProfileRef::with_label("asset-prod", "Prod"),
        "/srv/app",
    );
    quick_browser.selected_entry_ids = vec!["entry-current".into()];
    quick_browser.attach_terminal_session_id(Uuid::new_v4().to_string());
    quick_browser.entries = vec![SftpDirectoryEntry {
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
    let quick_browser_id = quick_browser.file_browser_session_id.clone();
    view_model.quick_browser_session_id = Some(quick_browser_id.clone());
    view_model.set_file_browser_session(quick_browser);

    let tab_id = view_model
        .expand_quick_browser_to_workspace()
        .expect("quick browser should expand");

    let active_tab = view_model
        .active_workspace_tab()
        .expect("workspace tab should be active after expand");
    assert_eq!(active_tab.tab_id, tab_id);
    assert_eq!(active_tab.kind, WorkspaceTabKind::Sftp);
    assert_eq!(active_tab.title, "Files: Prod");
    assert_eq!(view_model.workspace_session_host_mode(), "sftp");

    let expanded_session = view_model
        .active_workspace_sftp_session()
        .expect("expanded sftp browser session");
    assert_ne!(expanded_session.file_browser_session_id, quick_browser_id);
    assert_eq!(expanded_session.current_path, "/srv/app");
    assert!(expanded_session.selected_entry_ids.is_empty());
    assert_eq!(
        expanded_session.follow_mode,
        SftpFollowMode::ManualBrowse,
        "promoted workspace tabs should lock to the expanded path instead of inheriting quick-browser follow mode"
    );
    assert_eq!(
        expanded_session
            .entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        vec!["logs"],
        "the workspace should immediately inherit the quick-browser snapshot instead of starting from an empty shell"
    );
    assert_eq!(
        expanded_session.linked_terminal_session_id,
        view_model
            .quick_browser_session()
            .expect("quick browser session")
            .linked_terminal_session_id
    );
}
