use std::fs;

use i_slint_backend_selector::with_platform;
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::sftp::{
    FileBrowserSession, HostProfileRef, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode,
    SftpPanelMode, SftpPathHistory, SftpSessionBindingState,
};
use mica_term::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
use mica_term::shell::context_menu::{
    ContextMenuActionState, ContextTargetKind, SelectionContext, resolve_action_tree,
};
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::{AssetModalState, ShellViewModel};
use uuid::Uuid;

fn active_sftp_view_model(entries: Vec<SftpDirectoryEntry>) -> ShellViewModel {
    let session_id = Uuid::new_v4();
    let handle = SessionHandle {
        session_id,
        asset_id: "asset-prod".into(),
        title: "Prod Bastion".into(),
        subtitle: "ops@10.0.0.12:22".into(),
        state: SessionState::Connected,
        can_reconnect: false,
        enhanced_session_state: EnhancedSessionState::Plain,
    };
    let mut tab = WorkspaceTab::from_session(&handle);
    tab.active = true;

    let mut sftp = SftpSessionBindingState::follow("/srv/app");
    sftp.mode = SftpPanelMode::Ready;
    sftp.entries = entries;

    let mut state = ShellViewModel::default();
    state.set_workspace_tabs(vec![tab]);
    state.set_sftp_session_state(session_id.to_string(), sftp);
    state.open_sftp_panel();
    state
}

fn workspace_and_quick_browser_view_model(
    workspace_path: &str,
    quick_browser_path: &str,
) -> ShellViewModel {
    workspace_and_quick_browser_modes_view_model(
        workspace_path,
        SftpPanelMode::Ready,
        quick_browser_path,
        SftpPanelMode::Ready,
    )
}

fn workspace_and_quick_browser_modes_view_model(
    workspace_path: &str,
    workspace_mode: SftpPanelMode,
    quick_browser_path: &str,
    quick_browser_mode: SftpPanelMode,
) -> ShellViewModel {
    workspace_and_quick_browser_entries_view_model(
        workspace_path,
        workspace_mode,
        vec![SftpDirectoryEntry {
            id: "entry-wwwroot".into(),
            name: "wwwroot".into(),
            path: format!("{workspace_path}/wwwroot"),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        }],
        quick_browser_path,
        quick_browser_mode,
        vec![SftpDirectoryEntry {
            id: "entry-app".into(),
            name: "app".into(),
            path: format!("{quick_browser_path}/app"),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        }],
    )
}

fn workspace_and_quick_browser_entries_view_model(
    workspace_path: &str,
    workspace_mode: SftpPanelMode,
    workspace_entries: Vec<SftpDirectoryEntry>,
    quick_browser_path: &str,
    quick_browser_mode: SftpPanelMode,
    quick_browser_entries: Vec<SftpDirectoryEntry>,
) -> ShellViewModel {
    let workspace_session = FileBrowserSession {
        file_browser_session_id: "browser-workspace".into(),
        host_profile_ref: HostProfileRef::with_label("asset-prod", "Interserver"),
        linked_terminal_session_id: Some(Uuid::new_v4().to_string()),
        mode: workspace_mode,
        follow_mode: SftpFollowMode::ManualBrowse,
        current_path: workspace_path.into(),
        history: SftpPathHistory::with_initial(workspace_path),
        entries: workspace_entries,
        selected_entry_ids: vec![],
        selection_anchor_entry_id: None,
        last_error: None,
        active_request_id: None,
        sort_state: Default::default(),
        column_layout: Default::default(),
    };
    let quick_browser_session = FileBrowserSession {
        file_browser_session_id: "browser-quick".into(),
        host_profile_ref: HostProfileRef::with_label("asset-prod", "Interserver"),
        linked_terminal_session_id: Some(Uuid::new_v4().to_string()),
        mode: quick_browser_mode,
        follow_mode: SftpFollowMode::FollowCwd,
        current_path: quick_browser_path.into(),
        history: SftpPathHistory::with_initial(quick_browser_path),
        selected_entry_ids: quick_browser_entries
            .first()
            .map(|entry| vec![entry.id.clone()])
            .unwrap_or_default(),
        selection_anchor_entry_id: quick_browser_entries.first().map(|entry| entry.id.clone()),
        entries: quick_browser_entries,
        last_error: None,
        active_request_id: None,
        sort_state: Default::default(),
        column_layout: Default::default(),
    };

    let sftp_tab = WorkspaceTab::sftp(
        "tab-files-1",
        workspace_session.file_browser_session_id.clone(),
        "Files: Prod",
    );
    let mut state = ShellViewModel::default();
    state.set_file_browser_session(workspace_session);
    state.set_file_browser_session(quick_browser_session);
    state.quick_browser_session_id = Some("browser-quick".into());
    state.set_workspace_tabs(vec![sftp_tab]);
    state.open_sftp_panel();
    state
}

fn ready_selection(selected_ids: Vec<&str>) -> SelectionContext {
    ready_selection_with_clipboard(selected_ids, false)
}

fn ready_selection_with_clipboard(
    selected_ids: Vec<&str>,
    clipboard_has_payload: bool,
) -> SelectionContext {
    SelectionContext {
        selected_ids: selected_ids.into_iter().map(str::to_string).collect(),
        clipboard_has_asset_payload: clipboard_has_payload,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    }
}

fn ready_file_selection(selected_ids: Vec<&str>) -> SelectionContext {
    let count = selected_ids.len();
    SelectionContext {
        selected_ids: selected_ids.into_iter().map(str::to_string).collect(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        selected_file_count: count,
        selected_directory_count: 0,
    }
}

fn ready_directory_selection(selected_ids: Vec<&str>) -> SelectionContext {
    let count = selected_ids.len();
    SelectionContext {
        selected_ids: selected_ids.into_iter().map(str::to_string).collect(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: count,
    }
}

fn ready_mixed_selection(file_count: usize, directory_count: usize) -> SelectionContext {
    let total = file_count + directory_count;
    SelectionContext {
        selected_ids: (0..total).map(|index| format!("entry-{index}")).collect(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        selected_file_count: file_count,
        selected_directory_count: directory_count,
    }
}

#[test]
fn sftp_ready_state_exposes_blank_area_context_menu_hook() {
    let source = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");
    let ready_state_block = source
        .split(r#"if root.sftp-panel-mode != "empty" :"#)
        .nth(1)
        .and_then(|rest| rest.split("tooltip-delay := Timer {").next())
        .expect("ready-state quick browser block");

    assert!(
        ready_state_block.contains("sftp-panel-context-menu-requested(")
            && ready_state_block.contains("\"sftp-blank\""),
        "ready-state quick browser should expose a blank-area context-menu hook even when entries are already rendered"
    );
}
#[test]
fn sftp_targets_resolve_expected_action_sets() {
    let blank_actions = resolve_action_tree(
        ContextTargetKind::SftpBlankArea,
        &ready_selection_with_clipboard(vec![], true),
    );
    let blank_ids: Vec<_> = blank_actions.iter().map(|node| node.id).collect();
    assert_eq!(
        blank_actions
            .iter()
            .find(|node| node.id == "paste-sftp")
            .expect("paste-sftp action")
            .state,
        ContextMenuActionState::Disabled
    );
    assert_eq!(
        blank_ids,
        vec![
            "new-file",
            "new-folder",
            "upload-files",
            "upload-folder",
            "paste-sftp",
            "select-all-sftp",
            "refresh-sftp",
            "sort-name",
            "sort-size",
            "sort-modified",
            "copy-current-path",
            "open-sftp-workspace",
        ]
    );

    let folder_actions = resolve_action_tree(
        ContextTargetKind::SftpDirectory,
        &ready_directory_selection(vec!["entry-app"]),
    );
    let folder_ids: Vec<_> = folder_actions.iter().map(|node| node.id).collect();
    assert_eq!(
        folder_ids,
        vec![
            "open-remote",
            "open-in-new-sftp-tab",
            "download",
            "upload-files",
            "upload-folder",
            "new-folder",
            "new-file",
            "rename-sftp-entry",
            "delete-sftp-entry",
            "copy-sftp-entry",
            "cut-sftp-entry",
            "paste-sftp",
            "copy-folder-path",
            "refresh-sftp",
            "open-terminal-here",
            "permissions-sftp",
            "properties",
        ]
    );
    assert!(folder_ids.contains(&"open-terminal-here"));
    assert!(folder_ids.contains(&"rename-sftp-entry"));
    assert!(folder_ids.contains(&"delete-sftp-entry"));
    assert_eq!(
        folder_actions
            .iter()
            .find(|node| node.id == "download")
            .expect("download action")
            .label,
        "Download To..."
    );
    assert_eq!(
        folder_actions
            .iter()
            .find(|node| node.id == "open-remote")
            .expect("open-remote action")
            .label,
        "Open Folder"
    );
    assert_eq!(
        folder_actions
            .iter()
            .find(|node| node.id == "open-in-new-sftp-tab")
            .expect("open-in-new-sftp-tab action")
            .label,
        "Open Folder in New SFTP Tab"
    );
    assert_eq!(
        folder_actions
            .iter()
            .find(|node| node.id == "download")
            .expect("download action")
            .state,
        ContextMenuActionState::Enabled
    );
    assert_eq!(
        folder_actions
            .iter()
            .find(|node| node.id == "paste-sftp")
            .expect("paste-sftp action")
            .state,
        ContextMenuActionState::Disabled
    );

    let file_actions = resolve_action_tree(
        ContextTargetKind::SftpFile,
        &ready_file_selection(vec!["entry-release"]),
    );
    let file_ids: Vec<_> = file_actions.iter().map(|node| node.id).collect();
    assert_eq!(
        file_ids,
        vec![
            "open-local",
            "edit-locally",
            "download",
            "upload-files",
            "upload-folder",
            "new-folder",
            "new-file",
            "rename-sftp-entry",
            "delete-sftp-entry",
            "copy-sftp-entry",
            "cut-sftp-entry",
            "paste-sftp",
            "copy-file-path",
            "copy-file-name",
            "refresh-sftp",
            "permissions-sftp",
            "properties",
        ]
    );
    assert_eq!(
        file_actions
            .iter()
            .find(|node| node.id == "open-local")
            .expect("open-local action")
            .label,
        "Open File"
    );
    assert_eq!(
        file_actions
            .iter()
            .find(|node| node.id == "edit-locally")
            .expect("edit-locally action")
            .label,
        "Edit Locally"
    );
    assert!(file_ids.contains(&"download"));
    assert!(file_ids.contains(&"copy-file-path"));
    assert!(file_ids.contains(&"properties"));
    assert_eq!(
        file_actions
            .iter()
            .find(|node| node.id == "download")
            .expect("download action")
            .state,
        ContextMenuActionState::Enabled
    );
    for action_id in [
        "upload-files",
        "upload-folder",
        "new-folder",
        "new-file",
        "refresh-sftp",
    ] {
        assert_eq!(
            file_actions
                .iter()
                .find(|node| node.id == action_id)
                .unwrap_or_else(|| panic!("missing `{action_id}` action"))
                .state,
            ContextMenuActionState::Enabled,
            "workspace-style SFTP file menus should keep `{action_id}` available via the shared dispatcher instead of forcing users back to a blank-area-only action path"
        );
    }
    assert_eq!(
        file_actions
            .iter()
            .find(|node| node.id == "copy-sftp-entry")
            .expect("copy-sftp-entry action")
            .state,
        ContextMenuActionState::Disabled
    );
    for action_id in [
        "upload-files",
        "upload-folder",
        "new-folder",
        "new-file",
        "refresh-sftp",
    ] {
        assert_eq!(
            folder_actions
                .iter()
                .find(|node| node.id == action_id)
                .unwrap_or_else(|| panic!("missing `{action_id}` action"))
                .state,
            ContextMenuActionState::Enabled,
            "workspace-style SFTP directory menus should keep `{action_id}` close to the selected row while still routing through the existing pending-action and modal system"
        );
    }

    let multi_actions = resolve_action_tree(
        ContextTargetKind::SftpMultiSelection,
        &ready_mixed_selection(1, 1),
    );
    let multi_ids: Vec<_> = multi_actions.iter().map(|node| node.id).collect();
    assert_eq!(
        multi_ids,
        vec![
            "download-selected",
            "delete-selected",
            "copy-sftp-entry",
            "cut-sftp-entry",
            "permissions-sftp",
            "copy-paths",
            "refresh-sftp",
        ]
    );
    assert_eq!(
        multi_actions
            .iter()
            .find(|node| node.id == "download-selected")
            .expect("download-selected action")
            .state,
        ContextMenuActionState::Enabled
    );
    assert_eq!(
        multi_actions
            .iter()
            .find(|node| node.id == "copy-sftp-entry")
            .expect("copy-sftp-entry action")
            .state,
        ContextMenuActionState::Disabled
    );
}

#[test]
fn sftp_file_context_menu_contract_exposes_open_and_edit_locally_actions() {
    let dispatcher_source = fs::read_to_string("src/shell/view_model/context_menu_dispatcher.rs")
        .expect("read context menu dispatcher");
    let context_menu_source =
        fs::read_to_string("src/shell/context_menu.rs").expect("read context menu source");

    assert!(
        context_menu_source.contains("\"open-local\"")
            && context_menu_source.contains("\"edit-locally\""),
        "the file context menu should expose separate action ids for local open and edit-locally"
    );
    assert!(
        dispatcher_source.contains("\"open-local\"")
            && dispatcher_source.contains("\"edit-locally\""),
        "the context-menu dispatcher should route both local-open and edit-locally actions"
    );
    assert!(
        !context_menu_source.contains("\"open-with-remote\""),
        "the default SFTP file context menu should stop advertising the legacy remote editor action"
    );
}

#[test]
fn sftp_multi_select_download_supports_files_and_directories() {
    let files_only_actions = resolve_action_tree(
        ContextTargetKind::SftpMultiSelection,
        &ready_file_selection(vec!["entry-release", "entry-config"]),
    );
    assert_eq!(
        files_only_actions
            .iter()
            .find(|node| node.id == "download-selected")
            .expect("download-selected action")
            .state,
        ContextMenuActionState::Enabled
    );

    let directories_only_actions = resolve_action_tree(
        ContextTargetKind::SftpMultiSelection,
        &ready_directory_selection(vec!["entry-app", "entry-logs"]),
    );
    assert_eq!(
        directories_only_actions
            .iter()
            .find(|node| node.id == "download-selected")
            .expect("download-selected action")
            .state,
        ContextMenuActionState::Enabled
    );

    let mixed_actions = resolve_action_tree(
        ContextTargetKind::SftpMultiSelection,
        &ready_mixed_selection(1, 1),
    );
    assert_eq!(
        mixed_actions
            .iter()
            .find(|node| node.id == "download-selected")
            .expect("download-selected action")
            .state,
        ContextMenuActionState::Enabled
    );
}

#[test]
fn sftp_disconnected_or_loading_targets_disable_mutating_actions() {
    let disabled_selection = SelectionContext {
        target_mutable: false,
        selected_file_count: 0,
        selected_directory_count: 1,
        ..ready_selection(vec!["entry-app"])
    };

    let folder_actions = resolve_action_tree(ContextTargetKind::SftpDirectory, &disabled_selection);
    let rename = folder_actions
        .iter()
        .find(|node| node.id == "rename-sftp-entry")
        .expect("rename action should exist");
    let delete = folder_actions
        .iter()
        .find(|node| node.id == "delete-sftp-entry")
        .expect("delete action should exist");

    assert_eq!(rename.state, ContextMenuActionState::Disabled);
    assert_eq!(delete.state, ContextMenuActionState::Disabled);

    let blank_actions = resolve_action_tree(ContextTargetKind::SftpBlankArea, &disabled_selection);
    let new_folder = blank_actions
        .iter()
        .find(|node| node.id == "new-folder")
        .expect("new-folder action should exist");
    let paste = blank_actions
        .iter()
        .find(|node| node.id == "paste-sftp")
        .expect("paste-sftp action should exist");
    assert_eq!(new_folder.state, ContextMenuActionState::Disabled);
    assert_eq!(paste.state, ContextMenuActionState::Disabled);
}

#[test]
fn opening_sftp_context_menu_tracks_remote_selection_without_touching_asset_selection() {
    let mut state = active_sftp_view_model(vec![SftpDirectoryEntry {
        id: "entry-app".into(),
        name: "app".into(),
        path: "/srv/app".into(),
        kind: SftpDirectoryEntryKind::Directory,
        modified_unix_seconds: None,
        size_bytes: None,
        permissions_label: None,
        owner_label: None,
        group_label: None,
    }]);

    state.selected_asset_ids = vec!["asset-root".into()];
    state.focused_asset_id = Some("asset-root".into());

    state.open_context_menu_for_target(
        ContextTargetKind::SftpDirectory,
        Some("entry-app".into()),
        64.0,
        96.0,
    );

    assert_eq!(
        state.context_menu_selection().selected_ids,
        vec!["entry-app".to_string()]
    );
    assert_eq!(state.selected_asset_ids, vec!["asset-root".to_string()]);
    assert_eq!(state.focused_asset_id.as_deref(), Some("asset-root"));
}

#[test]
fn right_clicking_an_already_multi_selected_sftp_entry_keeps_the_multi_selection_menu() {
    let mut state = active_sftp_view_model(vec![
        SftpDirectoryEntry {
            id: "entry-app".into(),
            name: "app".into(),
            path: "/srv/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            path: "/srv/app/release.tar.gz".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: None,
            size_bytes: Some(14 * 1024),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
    ]);
    let active_session_id = state
        .active_file_browser_session_id()
        .expect("active browser session")
        .to_string();
    state
        .file_browser_sessions
        .get_mut(&active_session_id)
        .expect("session state")
        .selected_entry_ids = vec!["entry-app".into(), "entry-release".into()];

    state.open_context_menu_for_target(
        ContextTargetKind::SftpFile,
        Some("entry-release".into()),
        96.0,
        144.0,
    );

    assert_eq!(
        state.context_menu_target_kind,
        Some(ContextTargetKind::SftpMultiSelection)
    );
    assert_eq!(
        state.context_menu_selection().selected_ids,
        vec!["entry-app".to_string(), "entry-release".to_string()]
    );
}

#[test]
fn sftp_create_rename_and_delete_confirmations_do_not_mutate_projected_entries_locally() {
    let mut state = active_sftp_view_model(vec![
        SftpDirectoryEntry {
            id: "entry-app".into(),
            name: "app".into(),
            path: "/srv/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            path: "/srv/app/release.tar.gz".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: None,
            size_bytes: Some(14 * 1024),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
    ]);

    state.open_context_menu_for_target(ContextTargetKind::SftpBlankArea, None, 64.0, 96.0);
    state.handle_context_menu_leaf_action("new-folder");
    state.update_new_folder_modal_name("shared".into());
    assert!(state.confirm_asset_modal());
    assert!(
        state
            .active_sftp_session_state()
            .expect("active sftp state")
            .entries
            .iter()
            .all(|entry| entry.name != "shared"),
        "SFTP new-folder confirmation should wait for a real backend refresh instead of pushing a synthetic row into the projected list"
    );

    state.open_context_menu_for_target(
        ContextTargetKind::SftpFile,
        Some("entry-release".into()),
        80.0,
        120.0,
    );
    state.handle_context_menu_leaf_action("rename-sftp-entry");
    assert!(matches!(
        state.asset_modal_state,
        Some(AssetModalState::SftpRenameEntry { .. })
    ));

    state.update_rename_asset_modal_name("app".into());
    assert!(!state.confirm_asset_modal());
    assert_eq!(
        state.asset_rename_modal_validation_message(),
        "Name already exists in this folder."
    );

    state.update_rename_asset_modal_name("release-v2.tar.gz".into());
    assert!(state.confirm_asset_modal());
    assert!(
        state
            .active_sftp_session_state()
            .expect("active sftp state")
            .entries
            .iter()
            .any(|entry| {
                entry.id == "entry-release"
                    && entry.name == "release.tar.gz"
                    && entry.path == "/srv/app/release.tar.gz"
            }),
        "SFTP rename confirmation should leave the projected row alone until the remote refresh lands"
    );

    {
        let active_session_id = state
            .active_file_browser_session_id()
            .expect("active browser session")
            .to_string();
        let sftp_state = state
            .file_browser_sessions
            .get_mut(&active_session_id)
            .expect("session state");
        sftp_state.selected_entry_ids = vec!["entry-app".into(), "entry-release".into()];
    }
    state.open_context_menu_for_target(
        ContextTargetKind::SftpMultiSelection,
        Some("entry-app".into()),
        96.0,
        144.0,
    );
    state.handle_context_menu_leaf_action("delete-selected");
    assert!(matches!(
        state.asset_modal_state,
        Some(AssetModalState::SftpDeleteEntriesConfirm { .. })
    ));
    assert!(state.confirm_delete_asset());
    assert!(
        state
            .active_sftp_session_state()
            .expect("active sftp state")
            .entries
            .iter()
            .any(|entry| entry.id == "entry-app")
            && state
                .active_sftp_session_state()
                .expect("active sftp state")
                .entries
                .iter()
                .any(|entry| entry.id == "entry-release"),
        "SFTP delete confirmation should stop pruning projected rows locally before the remote delete finishes"
    );
}

#[test]
fn unsupported_sftp_actions_render_disabled_reasons() {
    let blank_actions = resolve_action_tree(
        ContextTargetKind::SftpBlankArea,
        &ready_selection_with_clipboard(vec![], true),
    );
    assert_eq!(
        blank_actions
            .iter()
            .find(|node| node.id == "paste-sftp")
            .expect("paste-sftp action")
            .state,
        ContextMenuActionState::Disabled
    );

    let file_actions = resolve_action_tree(
        ContextTargetKind::SftpFile,
        &ready_file_selection(vec!["entry-release"]),
    );
    for action_id in ["copy-sftp-entry", "cut-sftp-entry", "permissions-sftp"] {
        assert_eq!(
            file_actions
                .iter()
                .find(|node| node.id == action_id)
                .unwrap_or_else(|| panic!("{action_id} action"))
                .state,
            ContextMenuActionState::Disabled,
            "{action_id} should surface as disabled instead of planned"
        );
    }

    let mut state = active_sftp_view_model(vec![
        SftpDirectoryEntry {
            id: "entry-app".into(),
            name: "app".into(),
            path: "/srv/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            path: "/srv/app/release.tar.gz".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: None,
            size_bytes: Some(14 * 1024),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
    ]);

    state.open_context_menu_for_target(
        ContextTargetKind::SftpFile,
        Some("entry-release".into()),
        80.0,
        120.0,
    );
    let copy_index = resolve_action_tree(
        state.context_menu_target_kind.expect("copy target kind"),
        &state.context_menu_selection(),
    )
    .iter()
    .position(|node| node.id == "copy-sftp-entry")
    .expect("copy action index");
    state.set_context_menu_open_path(vec![copy_index]);
    state.invoke_current_context_menu_item();
    assert_eq!(
        state.context_menu_feedback_text,
        "Copy is not available for SFTP yet."
    );

    state.open_context_menu_for_target(
        ContextTargetKind::SftpFile,
        Some("entry-release".into()),
        80.0,
        120.0,
    );
    let cut_index = resolve_action_tree(
        state.context_menu_target_kind.expect("cut target kind"),
        &state.context_menu_selection(),
    )
    .iter()
    .position(|node| node.id == "cut-sftp-entry")
    .expect("cut action index");
    state.set_context_menu_open_path(vec![cut_index]);
    state.invoke_current_context_menu_item();
    assert_eq!(
        state.context_menu_feedback_text,
        "Cut is not available for SFTP yet."
    );

    state.open_context_menu_for_target(ContextTargetKind::SftpBlankArea, None, 64.0, 96.0);
    let paste_index = resolve_action_tree(
        state.context_menu_target_kind.expect("paste target kind"),
        &state.context_menu_selection(),
    )
    .iter()
    .position(|node| node.id == "paste-sftp")
    .expect("paste action index");
    state.set_context_menu_open_path(vec![paste_index]);
    state.invoke_current_context_menu_item();
    assert_eq!(
        state.context_menu_feedback_text,
        "Paste is not available for SFTP yet."
    );

    state.open_context_menu_for_target(
        ContextTargetKind::SftpFile,
        Some("entry-release".into()),
        80.0,
        120.0,
    );
    let permissions_index = resolve_action_tree(
        state
            .context_menu_target_kind
            .expect("permissions target kind"),
        &state.context_menu_selection(),
    )
    .iter()
    .position(|node| node.id == "permissions-sftp")
    .expect("permissions action index");
    state.set_context_menu_open_path(vec![permissions_index]);
    state.invoke_current_context_menu_item();
    assert_eq!(
        state.context_menu_feedback_text,
        "Permissions are not available for SFTP yet."
    );
}

#[test]
fn workspace_blank_menu_copy_current_path_prefers_the_workspace_session_path() {
    i_slint_backend_testing::init_no_event_loop();

    let mut state = workspace_and_quick_browser_view_model("/home/wwwroot", "/srv/app");
    with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    state.open_context_menu_for_target(ContextTargetKind::SftpBlankArea, None, 64.0, 96.0);
    state.handle_context_menu_leaf_action("copy-current-path");

    let copied = with_platform(|platform| {
        Ok(platform
            .clipboard_text(slint::platform::Clipboard::DefaultClipboard)
            .unwrap_or_default())
    })
    .expect("read clipboard");

    assert_eq!(
        copied, "/home/wwwroot",
        "workspace blank-area copy-current-path should copy the active workspace path, not the quick-browser path from a different surface"
    );
}

#[test]
fn workspace_context_menu_selection_prefers_the_workspace_surface_selection() {
    let mut state = workspace_and_quick_browser_view_model("/home/wwwroot", "/srv/app");

    state.open_context_menu_for_target(
        ContextTargetKind::SftpDirectory,
        Some("entry-wwwroot".into()),
        84.0,
        112.0,
    );

    assert_eq!(
        state.context_menu_selection().selected_ids,
        vec!["entry-wwwroot".to_string()],
        "workspace row context menus should read selected ids from the workspace SFTP surface instead of leaking the quick-browser selection"
    );
}

#[test]
fn workspace_context_menu_mutability_prefers_the_workspace_surface_mode() {
    let mut state = workspace_and_quick_browser_modes_view_model(
        "/home/wwwroot",
        SftpPanelMode::Ready,
        "/srv/app",
        SftpPanelMode::Disconnected,
    );

    state.open_context_menu_for_target(ContextTargetKind::SftpBlankArea, None, 64.0, 96.0);

    assert!(
        state.context_menu_selection().target_mutable,
        "workspace blank menus should stay mutable when the workspace surface is ready even if the quick browser happens to be disconnected"
    );
}

#[test]
fn quick_browser_rename_modal_stays_bound_to_the_originating_surface_session() {
    let mut state = workspace_and_quick_browser_entries_view_model(
        "/home",
        SftpPanelMode::Ready,
        vec![SftpDirectoryEntry {
            id: "entry-wwwroot".into(),
            name: "wwwroot".into(),
            path: "/home/wwwroot".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        }],
        "/srv/app",
        SftpPanelMode::Ready,
        vec![
            SftpDirectoryEntry {
                id: "entry-logs".into(),
                name: "logs".into(),
                path: "/srv/app/logs".into(),
                kind: SftpDirectoryEntryKind::Directory,
                modified_unix_seconds: None,
                size_bytes: None,
                permissions_label: None,
                owner_label: None,
                group_label: None,
            },
            SftpDirectoryEntry {
                id: "entry-logs-archive".into(),
                name: "logs-archive".into(),
                path: "/srv/app/logs-archive".into(),
                kind: SftpDirectoryEntryKind::Directory,
                modified_unix_seconds: None,
                size_bytes: None,
                permissions_label: None,
                owner_label: None,
                group_label: None,
            },
        ],
    );

    state.open_context_menu_for_surface(
        mica_term::shell::context_menu::ContextMenuSurface::QuickBrowserSftp,
        ContextTargetKind::SftpDirectory,
        Some("entry-logs".into()),
        84.0,
        112.0,
    );
    state.handle_context_menu_leaf_action("rename-sftp-entry");
    let quick_terminal_session_id = state
        .file_browser_sessions
        .get("browser-quick")
        .and_then(|session| session.linked_terminal_session_id.clone())
        .expect("quick browser linked terminal session id");
    state.update_rename_asset_modal_name("logs-archive".into());
    assert_eq!(
        state.asset_rename_modal_validation_message(),
        "Name already exists in this folder.",
        "SFTP rename validation should stay bound to the quick-browser session that opened the modal instead of switching to the active workspace session"
    );

    state.update_rename_asset_modal_name("logs-current".into());
    assert!(
        state.confirm_asset_modal(),
        "confirming a quick-browser rename should keep working even while a workspace SFTP tab is active elsewhere"
    );
    assert_eq!(
        state.take_pending_sftp_context_action(),
        Some(
            mica_term::shell::view_model::PendingSftpContextAction::RenameEntry {
                from: "/srv/app/logs".into(),
                to: "/srv/app/logs-current".into(),
                refresh_path: "/srv/app".into(),
                linked_terminal_session_id: quick_terminal_session_id,
            }
        )
    );
}

#[test]
fn sftp_rename_execution_keeps_the_originating_terminal_session_id() {
    let view_model =
        fs::read_to_string("src/shell/view_model/asset_modal_executor.rs").expect("read executor");
    let bootstrap = fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");

    assert!(
        view_model.contains("linked_terminal_session_id,")
            && view_model.contains("PendingSftpContextAction::RenameEntry {")
            && view_model.contains("linked_terminal_session_id,"),
        "SFTP rename confirmation should enqueue the terminal session id captured from the browser session that opened the rename modal"
    );
    assert!(
        bootstrap.contains("RenameEntry {\n            from,\n            to,\n            refresh_path,\n            linked_terminal_session_id,")
            && bootstrap.contains("Uuid::parse_str(linked_terminal_session_id.as_str())"),
        "SFTP rename execution should use the captured terminal session id instead of resolving whichever SFTP surface is active when the modal is confirmed"
    );
}

#[test]
fn bootstrap_routes_workspace_and_quick_browser_context_menus_through_distinct_sftp_surfaces() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");

    assert!(
        bootstrap_sftp.contains("ContextMenuSurface::QuickBrowserSftp")
            && bootstrap_sftp.contains("ContextMenuSurface::WorkspaceSftp"),
        "bootstrap should open quick-browser and workspace SFTP context menus through explicit surface ids so selection, mutability, and copied paths cannot bleed across surfaces"
    );
}

#[test]
fn opening_workspace_sftp_context_menu_dismisses_the_assets_create_popup_first() {
    let mut state = active_sftp_view_model(vec![SftpDirectoryEntry {
        id: "entry-app".into(),
        name: "app".into(),
        path: "/srv/app".into(),
        kind: SftpDirectoryEntryKind::Directory,
        modified_unix_seconds: None,
        size_bytes: None,
        permissions_label: None,
        owner_label: None,
        group_label: None,
    }]);
    state.asset_create_menu_open = true;

    state.open_context_menu_for_target(ContextTargetKind::SftpBlankArea, None, 64.0, 96.0);

    assert!(
        !state.asset_create_menu_open,
        "opening a workspace SFTP context menu should dismiss the assets create popup before the new menu takes over the transient surface"
    );
}

#[test]
fn queue_summary_callback_opens_queue_drawer_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sftp_panel_requested();
    app.invoke_sftp_panel_open_queue_requested();

    assert!(app.get_sftp_queue_drawer_open());
}
