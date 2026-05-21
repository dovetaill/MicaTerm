use std::fs;

use mica_term::app::sftp::{
    FileBrowserSession, HostProfileRef, SftpDirectoryEntry, SftpDirectoryEntryKind,
    SftpFollowMode, SftpPanelMode, SftpPathHistory,
};
use mica_term::app::ssh::runtime::TerminalSurfaceState;
use mica_term::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::{RightPanelView, ShellViewModel};
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

fn sample_sftp_entry(
    id: &str,
    name: &str,
    path: &str,
    kind: SftpDirectoryEntryKind,
    modified_unix_seconds: Option<u64>,
    size_bytes: Option<u64>,
    permissions_label: Option<&str>,
    owner_label: Option<&str>,
    group_label: Option<&str>,
) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        id: id.into(),
        name: name.into(),
        path: path.into(),
        kind,
        modified_unix_seconds,
        size_bytes,
        permissions_label: permissions_label.map(str::to_string),
        owner_label: owner_label.map(str::to_string),
        group_label: group_label.map(str::to_string),
    }
}

fn sample_workspace_sftp_session(mode: SftpPanelMode, follow_mode: SftpFollowMode) -> FileBrowserSession {
    FileBrowserSession {
        file_browser_session_id: "browser-1".into(),
        host_profile_ref: HostProfileRef::with_label("asset-prod", "Interserver"),
        linked_terminal_session_id: Some(Uuid::new_v4().to_string()),
        mode,
        follow_mode,
        current_path: "/srv/app/releases".into(),
        history: SftpPathHistory::with_initial("/srv/app/releases"),
        entries: vec![
            sample_sftp_entry(
                "/srv/app/releases/logs",
                "logs",
                "/srv/app/releases/logs",
                SftpDirectoryEntryKind::Directory,
                Some(1_777_000_001),
                None,
                Some("rwxr-xr-x"),
                Some("deploy"),
                Some("www-data"),
            ),
            sample_sftp_entry(
                "/srv/app/releases/release.tar.gz",
                "release.tar.gz",
                "/srv/app/releases/release.tar.gz",
                SftpDirectoryEntryKind::File,
                Some(1_777_000_777),
                Some(14 * 1024),
                Some("rw-r--r--"),
                Some("deploy"),
                Some("www-data"),
            ),
        ],
        selected_entry_ids: vec!["/srv/app/releases/release.tar.gz".into()],
        last_error: None,
        active_request_id: None,
        sort_state: Default::default(),
        column_layout: Default::default(),
    }
}

fn root_scroll_fixture_session(current_path: &str) -> FileBrowserSession {
    let root_names = [
        "bin",
        "boot",
        "dev",
        "etc",
        "home",
        "lib",
        "lib32",
        "lib64",
        "media",
        "mnt",
        "opt",
        "proc",
        "root",
        "run",
        "sbin",
        "snap",
        "srv",
        "sys",
        "tmp",
        "usr",
        "var",
        "workspace",
        "www",
        "wwwroot",
        "zfs",
        "zzz-last",
    ];
    FileBrowserSession {
        file_browser_session_id: "browser-root".into(),
        host_profile_ref: HostProfileRef::with_label("asset-prod", "Interserver"),
        linked_terminal_session_id: Some(Uuid::new_v4().to_string()),
        mode: SftpPanelMode::Ready,
        follow_mode: SftpFollowMode::ManualBrowse,
        current_path: current_path.into(),
        history: SftpPathHistory::with_initial(current_path),
        entries: root_names
            .iter()
            .map(|name| {
                sample_sftp_entry(
                    &format!("root-{name}"),
                    name,
                    &format!("/{name}"),
                    SftpDirectoryEntryKind::Directory,
                    Some(1_777_000_000),
                    None,
                    Some("rwxr-xr-x"),
                    Some("root"),
                    Some("root"),
                )
            })
            .collect(),
        selected_entry_ids: vec![],
        last_error: None,
        active_request_id: None,
        sort_state: Default::default(),
        column_layout: Default::default(),
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
    view_model.set_active_workspace_terminal_surface(Some(
        TerminalSurfaceState::from_visible_lines(
            Uuid::parse_str(terminal_tab.session_id.as_str()).expect("terminal session uuid"),
            1,
            24,
            80,
            vec!["pwd".into()],
        ),
    ));

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

#[test]
fn active_sftp_workspace_policy_hides_duplicate_sftp_panel_without_forgetting_user_preference() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab.clone(), sftp_tab.clone()]);
    view_model.toggle_right_panel();

    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
    assert!(view_model.right_panel_can_revive());

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));

    assert_eq!(
        view_model.right_panel_display_policy_id(),
        "policy-hidden-sftp-workspace"
    );
    assert!(view_model.show_right_panel);
    assert!(
        !view_model.requested_right_panel(),
        "policy-hidden workspace tabs should release right-panel width without clearing the user's remembered open preference"
    );
    assert!(
        !view_model.right_panel_can_revive(),
        "policy-hidden workspace tabs should not offer a revive affordance for the duplicate quick browser"
    );

    assert!(view_model.activate_workspace_tab(terminal_tab.tab_id.as_str()));
    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
}

#[test]
fn active_sftp_workspace_only_policy_hides_the_sftp_right_panel_view() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab, sftp_tab.clone()]);
    view_model.toggle_right_panel();
    view_model.set_right_panel_view(RightPanelView::Appearance);

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));

    assert_eq!(view_model.right_panel_display_policy_id(), "visible");
    assert!(view_model.requested_right_panel());
    assert!(view_model.right_panel_can_revive());
}

#[test]
fn right_panel_policy_hidden_contract_is_projected_for_active_sftp_workspace_tabs() {
    let view_model = fs::read_to_string("src/shell/view_model.rs").expect("read shell view model");
    let projection =
        fs::read_to_string("src/shell/view_model/projection.rs").expect("read projection");
    let shell_chrome =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        view_model.contains("RightPanelDisplayPolicy")
            && view_model.contains("PolicyHiddenForSftpWorkspace"),
        "view model should expose an explicit right-panel display policy enum so active SFTP workspace tabs can hide duplicate quick-browser lists without pretending the user collapsed them"
    );
    assert!(
        projection.contains("right_panel_display_policy")
            && projection.contains("right_panel_can_revive")
            && projection.contains("policy-hidden-sftp-workspace"),
        "projection should distinguish visible, user-collapsed, and policy-hidden SFTP workspace states"
    );
    assert!(
        shell_chrome.contains("window.set_right_panel_display_policy(")
            && shell_chrome.contains("window.set_right_panel_can_revive("),
        "bootstrap shell chrome sync should publish the right-panel display policy and revive capability into Slint"
    );
    assert!(
        app_window.contains("in-out property <string> right-panel-display-policy: \"visible\";")
            && app_window.contains("in-out property <bool> right-panel-can-revive: true;")
            && app_window.contains(
                "if !root.effective-show-right-panel && root.right-panel-can-revive : right-panel-revive-strip := Rectangle {"
            ),
        "AppWindow should thread policy-hidden SFTP workspace semantics into the right-panel revive-strip contract"
    );
}

#[test]
fn workspace_sftp_projection_rows_preserve_productized_icon_and_metadata_contract() {
    let session = sample_workspace_sftp_session(SftpPanelMode::Ready, SftpFollowMode::ManualBrowse);
    let sftp_tab = WorkspaceTab::sftp(
        "tab-files-1",
        session.file_browser_session_id.clone(),
        "Files: Prod",
    );
    let mut view_model = ShellViewModel::default();
    view_model.set_file_browser_session(session);
    view_model.set_workspace_tabs(vec![sftp_tab]);

    let rows = view_model.workspace_sftp_render_rows();

    assert_eq!(view_model.workspace_sftp_total_row_count(), 3);
    assert_eq!(view_model.workspace_sftp_selected_row_count(), 1);
    assert_eq!(rows[0].icon_kind, "parent-directory");
    assert_eq!(rows[1].icon_kind, "directory");
    assert_eq!(rows[1].permissions_label, "rwxr-xr-x");
    assert_eq!(rows[1].owner_label, "deploy");
    assert_eq!(rows[1].group_label, "www-data");
    assert_eq!(rows[2].icon_kind, "archive");
    assert_eq!(rows[2].kind, "archive");
    assert_eq!(rows[2].permissions_label, "rw-r--r--");
    assert!(rows[2].selected);
}

#[test]
fn active_sftp_workspace_summary_prefers_live_host_status_and_binding_metadata() {
    let session =
        sample_workspace_sftp_session(SftpPanelMode::Loading, SftpFollowMode::ManualBrowse);
    let sftp_tab = WorkspaceTab::sftp(
        "tab-files-1",
        session.file_browser_session_id.clone(),
        "Files: Prod",
    );
    let mut view_model = ShellViewModel::default();
    view_model.set_file_browser_session(session);
    view_model.set_workspace_tabs(vec![sftp_tab]);

    let summary = view_model
        .active_workspace_tab_summary()
        .expect("active workspace tab summary");

    assert_eq!(summary.display_name, "Files: Prod");
    assert_eq!(summary.primary_summary_text, "Interserver · SFTP");
    assert_eq!(summary.host, "Interserver");
    assert_eq!(summary.connection_status, "loading");
    assert_eq!(summary.connection_status_label, "Loading");
    assert!(
        summary.tooltip_text.contains("Path: /srv/app/releases"),
        "expected live SFTP path in tooltip, got: {}",
        summary.tooltip_text
    );
    assert!(
        summary.tooltip_text.contains("Binding: Locked / Manual"),
        "expected live SFTP binding in tooltip, got: {}",
        summary.tooltip_text
    );
    assert!(
        summary.tooltip_text.contains("Status: Loading"),
        "expected live SFTP status in tooltip, got: {}",
        summary.tooltip_text
    );
}

#[test]
fn workspace_sftp_projection_contract_threads_richer_rows_and_incremental_sync_into_slint() {
    let view_model = fs::read_to_string("src/shell/view_model.rs").expect("read shell view model");
    let sftp_view_model =
        fs::read_to_string("src/shell/view_model/sftp.rs").expect("read sftp view model");
    let right_panel = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");
    let bootstrap = fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read sftp bootstrap");

    for contract in [
        "pub permissions_label: String,",
        "pub owner_label: String,",
        "pub group_label: String,",
        "pub icon_kind: String,",
    ] {
        assert!(
            view_model.contains(contract),
            "workspace SFTP projection should expose `{contract}` for the productized host row and summary contract"
        );
    }

    for contract in [
        "pub fn workspace_sftp_total_row_count(&self) -> usize {",
        "pub fn workspace_sftp_selected_row_count(&self) -> usize {",
    ] {
        assert!(
            sftp_view_model.contains(contract),
            "workspace SFTP view-model helpers should expose `{contract}` for the productized host status summary"
        );
    }

    for contract in [
        "permissions_label: string,",
        "owner_label: string,",
        "group_label: string,",
        "icon_kind: string,",
    ] {
        assert!(
            right_panel.contains(contract),
            "Slint-facing SFTP item contract should expose `{contract}` so quick browser and workspace host can share one richer row projection"
        );
    }

    for contract in [
        "permissions_label: row.permissions_label.as_str().into(),",
        "owner_label: row.owner_label.as_str().into(),",
        "group_label: row.group_label.as_str().into(),",
        "icon_kind: row.icon_kind.as_str().into(),",
        "sync_sftp_panel_items_model(",
        "state.workspace_sftp_render_dirty_indices()",
        "state.mark_workspace_sftp_render_clean()",
    ] {
        assert!(
            bootstrap.contains(contract),
            "workspace SFTP bootstrap sync should project `{contract}` instead of replacing the whole workspace file model on every tick"
        );
    }
}

#[test]
fn workspace_sftp_row_height_contract_matches_the_slint_host() {
    let sftp_view_model =
        fs::read_to_string("src/shell/view_model/sftp.rs").expect("read sftp view model");
    let host =
        fs::read_to_string("ui/shell/sftp-workspace-host.slint").expect("read sftp workspace host");

    assert!(
        sftp_view_model.contains("const SFTP_PANEL_ROW_HEIGHT_PX: u32 = 40;")
            && host.contains("height: 40px;"),
        "workspace virtualization must share one 40px row-height contract across Rust and Slint so scrolling math does not drift between visible rows and spacer heights"
    );
}

#[test]
fn workspace_sftp_submitting_root_path_resets_the_virtual_viewport_to_the_top() {
    let session = root_scroll_fixture_session("/srv/app/releases");
    let sftp_tab = WorkspaceTab::sftp(
        "tab-files-root",
        session.file_browser_session_id.clone(),
        "Files: Root",
    );
    let mut view_model = ShellViewModel::default();
    view_model.set_file_browser_session(session);
    view_model.set_workspace_tabs(vec![sftp_tab]);

    assert!(
        view_model.update_workspace_sftp_viewport(-14.0 * 44.0, 160.0),
        "scroll fixture should move the virtual workspace viewport away from the top before navigation resets are asserted"
    );
    assert!(
        view_model.workspace_sftp_top_spacer_height_px() > 0.0,
        "scroll fixture should start from a non-zero spacer height"
    );

    assert!(view_model.submit_workspace_sftp_path("/"));

    assert_eq!(
        view_model.workspace_sftp_top_spacer_height_px(),
        0.0,
        "navigating the workspace to `/` should reset the controlled viewport to the top instead of preserving a stale scrolled window from the previous directory"
    );
    assert!(
        view_model
            .workspace_sftp_render_rows()
            .iter()
            .take(8)
            .any(|row| row.name == "home"),
        "once `/` is projected from the top, the first visible workspace rows should include early root entries such as `home`"
    );
}
