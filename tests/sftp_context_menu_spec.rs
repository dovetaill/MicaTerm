use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::sftp::{
    SftpDirectoryEntry, SftpDirectoryEntryKind, SftpPanelMode, SftpSessionBindingState,
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
        ContextMenuActionState::Planned
    );
    assert_eq!(
        blank_actions
            .iter()
            .find(|node| node.id == "show-hidden-sftp")
            .expect("show-hidden-sftp action")
            .state,
        ContextMenuActionState::Planned
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
            "show-hidden-sftp",
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
            "rename-sftp-entry",
            "delete-sftp-entry",
            "copy-sftp-entry",
            "cut-sftp-entry",
            "paste-sftp",
            "copy-folder-path",
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
            "rename-sftp-entry",
            "delete-sftp-entry",
            "copy-sftp-entry",
            "cut-sftp-entry",
            "paste-sftp",
            "copy-file-path",
            "copy-file-name",
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
        "Open"
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
    assert_eq!(
        file_actions
            .iter()
            .find(|node| node.id == "copy-sftp-entry")
            .expect("copy-sftp-entry action")
            .state,
        ContextMenuActionState::Planned
    );

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
        ContextMenuActionState::Planned
    );
}

#[test]
fn sftp_file_context_menu_contract_exposes_open_and_edit_locally_actions() {
    let dispatcher_source = fs::read_to_string("src/shell/view_model/context_menu_dispatcher.rs")
        .expect("read context menu dispatcher");
    let context_menu_source =
        fs::read_to_string("src/shell/context_menu.rs").expect("read context menu source");

    assert!(
        context_menu_source.contains("\"open-local\"") && context_menu_source.contains("\"edit-locally\""),
        "the file context menu should expose separate action ids for local open and edit-locally"
    );
    assert!(
        dispatcher_source.contains("\"open-local\"") && dispatcher_source.contains("\"edit-locally\""),
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
        },
        SftpDirectoryEntry {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            path: "/srv/app/release.tar.gz".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: None,
            size_bytes: Some(14 * 1024),
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
fn sftp_create_rename_and_delete_flows_mutate_projected_state_only_after_confirmation() {
    let mut state = active_sftp_view_model(vec![
        SftpDirectoryEntry {
            id: "entry-app".into(),
            name: "app".into(),
            path: "/srv/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: None,
            size_bytes: None,
        },
        SftpDirectoryEntry {
            id: "entry-release".into(),
            name: "release.tar.gz".into(),
            path: "/srv/app/release.tar.gz".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: None,
            size_bytes: Some(14 * 1024),
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
            .any(|entry| entry.name == "shared" && entry.kind == SftpDirectoryEntryKind::Directory)
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
    assert!(
        state
            .active_sftp_session_state()
            .expect("active sftp state")
            .entries
            .iter()
            .any(|entry| entry.id == "entry-release" && entry.name == "release.tar.gz")
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
            .all(|entry| entry.id != "entry-app" && entry.id != "entry-release")
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
