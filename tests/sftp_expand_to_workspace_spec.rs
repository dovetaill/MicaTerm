use mica_term::app::sftp::{FileBrowserSession, HostProfileRef};
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
        expanded_session.linked_terminal_session_id,
        view_model
            .quick_browser_session()
            .expect("quick browser session")
            .linked_terminal_session_id
    );
}
