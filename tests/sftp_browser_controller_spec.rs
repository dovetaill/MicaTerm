use mica_term::app::sftp::{
    FileBrowserSession, HostProfileRef, SftpBrowserController, SftpDirectoryEntry,
    SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode,
};
use uuid::Uuid;

fn entry(id: &str, name: &str, path: &str, kind: SftpDirectoryEntryKind) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        id: id.into(),
        name: name.into(),
        path: path.into(),
        kind,
        modified_unix_seconds: None,
        size_bytes: None,
    }
}

#[test]
fn open_loads_active_session_directory_and_marks_ready() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let request = controller.open(session_id, "/srv/app");

    let state = controller
        .session_state(session_id)
        .expect("session state should exist after open");
    assert_eq!(state.mode, SftpPanelMode::Connecting);
    assert_eq!(state.current_path, "/srv/app");
    assert_eq!(request.session_id, session_id);
    assert_eq!(request.path, "/srv/app");

    controller.apply_loaded_directory(
        session_id,
        request.request_id,
        "/srv/app",
        vec![entry(
            "entry-logs",
            "logs",
            "/srv/app/logs",
            SftpDirectoryEntryKind::Directory,
        )],
    );

    let state = controller
        .session_state(session_id)
        .expect("session state should remain available");
    assert_eq!(state.mode, SftpPanelMode::Ready);
    assert_eq!(state.current_path, "/srv/app");
    assert_eq!(state.entries.len(), 1);
    assert_eq!(state.entries[0].path, "/srv/app/logs");
}

#[test]
fn stale_directory_results_do_not_overwrite_newer_requests() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let first = controller.open(session_id, "/srv/app");
    let second = controller.navigate(session_id, "/srv/app/releases");

    controller.apply_loaded_directory(
        session_id,
        second.request_id,
        "/srv/app/releases",
        vec![entry(
            "entry-release",
            "release.tar.gz",
            "/srv/app/releases/release.tar.gz",
            SftpDirectoryEntryKind::File,
        )],
    );
    controller.apply_loaded_directory(
        session_id,
        first.request_id,
        "/srv/app",
        vec![entry(
            "entry-stale",
            "stale.log",
            "/srv/app/stale.log",
            SftpDirectoryEntryKind::File,
        )],
    );

    let state = controller
        .session_state(session_id)
        .expect("session state should be available");
    assert_eq!(state.mode, SftpPanelMode::Ready);
    assert_eq!(state.current_path, "/srv/app/releases");
    assert_eq!(state.entries.len(), 1);
    assert_eq!(state.entries[0].path, "/srv/app/releases/release.tar.gz");
}

#[test]
fn navigate_switches_to_manual_browse_and_pushes_history() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let open = controller.open(session_id, "/srv/app");
    controller.apply_loaded_directory(session_id, open.request_id, "/srv/app", Vec::new());

    let navigate = controller.navigate(session_id, "/srv/app/releases");

    let state = controller
        .session_state(session_id)
        .expect("session state should be available");
    assert_eq!(navigate.path, "/srv/app/releases");
    assert_eq!(state.mode, SftpPanelMode::Loading);
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
    assert_eq!(state.current_path, "/srv/app/releases");
    assert_eq!(
        state.history.entries(),
        ["/srv/app".to_string(), "/srv/app/releases".to_string()]
    );
}

#[test]
fn follow_cwd_only_updates_when_follow_mode_is_enabled() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let open = controller.open(session_id, "/srv/app");
    controller.apply_loaded_directory(session_id, open.request_id, "/srv/app", Vec::new());

    let follow = controller
        .follow_cwd(session_id, "/srv/app/current")
        .expect("follow mode should accept cwd updates");
    assert_eq!(follow.path, "/srv/app/current");

    let manual = controller.navigate(session_id, "/srv/manual");
    controller.apply_loaded_directory(session_id, manual.request_id, "/srv/manual", Vec::new());

    assert!(
        controller
            .follow_cwd(session_id, "/srv/app/ignored")
            .is_none(),
        "manual browse should ignore cwd updates"
    );

    let state = controller
        .session_state(session_id)
        .expect("session state should be available");
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
    assert_eq!(state.current_path, "/srv/manual");
}

#[test]
fn retry_moves_disconnected_session_back_to_connecting() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let open = controller.open(session_id, "/srv/app");
    controller.apply_loaded_directory(session_id, open.request_id, "/srv/app", Vec::new());
    controller.mark_disconnected(session_id);

    let retry = controller
        .retry(session_id)
        .expect("disconnected session should retry");

    let state = controller
        .session_state(session_id)
        .expect("session state should be available");
    assert_eq!(retry.path, "/srv/app");
    assert_eq!(state.mode, SftpPanelMode::Connecting);
    assert_eq!(state.current_path, "/srv/app");
}

#[test]
fn back_forward_and_up_navigation_issue_real_load_requests() {
    let session_id = Uuid::new_v4();
    let mut controller = SftpBrowserController::default();

    let open = controller.open(session_id, "/srv/app");
    controller.apply_loaded_directory(session_id, open.request_id, "/srv/app", Vec::new());

    let navigate = controller.navigate(session_id, "/srv/app/releases");
    controller.apply_loaded_directory(
        session_id,
        navigate.request_id,
        "/srv/app/releases",
        Vec::new(),
    );

    let back = controller
        .navigate_back(session_id)
        .expect("history back should produce a load request");
    assert_eq!(back.path, "/srv/app");

    controller.apply_loaded_directory(session_id, back.request_id, "/srv/app", Vec::new());

    let forward = controller
        .navigate_forward(session_id)
        .expect("history forward should produce a load request");
    assert_eq!(forward.path, "/srv/app/releases");

    controller.apply_loaded_directory(
        session_id,
        forward.request_id,
        "/srv/app/releases",
        Vec::new(),
    );

    let up = controller
        .navigate_up(session_id)
        .expect("navigate up should produce a load request");
    assert_eq!(up.path, "/srv/app");

    let state = controller
        .session_state(session_id)
        .expect("session state should be available");
    assert_eq!(state.mode, SftpPanelMode::Loading);
    assert_eq!(state.follow_mode, SftpFollowMode::ManualBrowse);
    assert_eq!(state.current_path, "/srv/app");
}

#[test]
fn load_requests_can_route_by_file_browser_session_id() {
    let mut controller = SftpBrowserController::default();
    let browser_session =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let browser_session_id = browser_session.file_browser_session_id.clone();

    let request = controller.open_file_browser_session(browser_session.clone());

    assert_eq!(request.file_browser_session_id, browser_session_id);
    assert_eq!(request.path, "/srv/app");

    controller.apply_loaded_directory_for_browser_session(
        request.file_browser_session_id.as_str(),
        request.request_id,
        "/srv/app",
        Vec::new(),
    );

    let state = controller
        .browser_session_state(browser_session_id.as_str())
        .expect("browser session state should remain available");
    assert_eq!(state.current_path, "/srv/app");
}
