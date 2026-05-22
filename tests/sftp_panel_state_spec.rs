use std::fs;

use mica_term::app::sftp::{
    FileBrowserSession, FileBrowserSortColumn, FileBrowserSortDirection, HostProfileRef,
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

    assert!(view_model.file_browser_sessions.is_empty());
    assert!(view_model.quick_browser_session_id.is_none());
    assert!(view_model.quick_browser_state.follows_active_terminal);
    assert_eq!(view_model.sftp_queue_summary.active_count, 0);
    assert_eq!(view_model.sftp_queue_summary.failed_count, 0);
    assert_eq!(view_model.sftp_queue_summary.current_session_count, 0);
}

#[test]
fn file_browser_sort_cycle_is_session_local_state() {
    let mut quick = mica_term::app::sftp::FileBrowserSession::quick_browser(
        mica_term::app::sftp::HostProfileRef::new("asset-prod"),
        "/srv/app",
    );
    quick.sort_state.column = Some(FileBrowserSortColumn::Name);
    quick.sort_state.direction = Some(FileBrowserSortDirection::Asc);
    let workspace = quick.clone_for_workspace();

    assert_eq!(
        workspace.sort_state.column,
        Some(FileBrowserSortColumn::Name)
    );
    assert_eq!(
        workspace.sort_state.direction,
        Some(FileBrowserSortDirection::Asc)
    );
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
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "dir-app".into(),
            name: "app".into(),
            path: "/srv/app/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: Some(2),
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "file-alpha".into(),
            name: "alpha.log".into(),
            path: "/srv/app/alpha.log".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(1),
            size_bytes: Some(10),
            permissions_label: None,
            owner_label: None,
            group_label: None,
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

#[test]
fn sftp_panel_render_cache_only_marks_changed_selection_rows_dirty() {
    let mut view_model = ShellViewModel::default();
    let mut session =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let session_id = session.file_browser_session_id.clone();
    session.entries = vec![
        SftpDirectoryEntry {
            id: "file-zeta".into(),
            name: "zeta.log".into(),
            path: "/srv/app/zeta.log".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(3),
            size_bytes: Some(100),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "dir-app".into(),
            name: "app".into(),
            path: "/srv/app/app".into(),
            kind: SftpDirectoryEntryKind::Directory,
            modified_unix_seconds: Some(2),
            size_bytes: None,
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
        SftpDirectoryEntry {
            id: "file-alpha".into(),
            name: "alpha.log".into(),
            path: "/srv/app/alpha.log".into(),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(1),
            size_bytes: Some(10),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        },
    ];
    session.selected_entry_ids = vec!["file-alpha".into()];

    view_model.quick_browser_session_id = Some(session_id);
    view_model.set_file_browser_session(session);

    assert!(
        view_model.active_sftp_panel_render_requires_full_resync(),
        "a freshly rebuilt directory snapshot should request one full right-panel sync"
    );
    assert!(view_model.mark_active_sftp_panel_render_clean());
    assert!(!view_model.active_sftp_panel_render_requires_full_resync());
    assert!(
        view_model
            .active_sftp_panel_render_dirty_indices()
            .is_empty(),
        "once the panel sync consumes the fresh snapshot, the render cache should go clean until another real change lands"
    );

    assert!(view_model.select_sftp_panel_entry("file-zeta"));

    assert!(
        !view_model.active_sftp_panel_render_requires_full_resync(),
        "changing the selected row should not force a full panel rebuild when the directory snapshot itself is unchanged"
    );
    assert_eq!(
        view_model.active_sftp_panel_render_dirty_indices(),
        &[2, 3],
        "switching selection between two file rows should only dirty the previously selected row and the newly selected row; the parent row and unaffected directory row should stay clean"
    );
    assert_eq!(
        view_model
            .active_sftp_panel_render_rows()
            .iter()
            .map(|row| (row.id.as_str(), row.selected))
            .collect::<Vec<_>>(),
        vec![
            ("__sftp_parent__", false),
            ("dir-app", false),
            ("file-alpha", false),
            ("file-zeta", true),
        ]
    );
}

#[test]
fn sftp_panel_projection_stays_cached_between_shell_sync_passes() {
    let view_model_source =
        fs::read_to_string("src/shell/view_model.rs").expect("read shell view model source");
    let sftp_view_model_source =
        fs::read_to_string("src/shell/view_model/sftp.rs").expect("read sftp view model source");
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp source");

    assert!(
        view_model_source.contains("sftp_panel_projection_cache"),
        "shell view model should keep a cached SFTP panel projection so reopening or switching the right sidebar can reuse the last sorted snapshot instead of rebuilding it synchronously on every UI sync"
    );
    assert!(
        sftp_view_model_source.contains("refresh_sftp_panel_projection_cache(")
            && sftp_view_model_source.contains("sftp_panel_projected_entries("),
        "SFTP view-model helpers should expose explicit projection-cache refresh/read paths for the quick browser"
    );
    assert!(
        view_model_source.contains("sftp_panel_render_cache")
            && sftp_view_model_source.contains("active_sftp_panel_render_rows("),
        "shell view model should keep a second-stage render cache so the right panel can reuse preformatted rows instead of rebuilding them on every shell sync"
    );
    assert!(
        !bootstrap_sftp.contains(".project_sftp_panel_entries(state.sftp_panel_entries())"),
        "bootstrap SFTP sync should stop rebuilding the sorted entry projection inline on every right-panel refresh"
    );
    assert!(
        bootstrap_sftp.contains("state.active_sftp_panel_render_rows()"),
        "bootstrap SFTP sync should consume the cached render rows instead of mapping raw directory entries into panel rows on every sync pass"
    );
}

#[test]
fn sftp_panel_sync_uses_incremental_row_updates_instead_of_generic_full_reconcile() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp source");

    assert!(
        bootstrap_sftp.contains("sync_sftp_panel_items_model("),
        "right-panel bootstrap should route SFTP row updates through a dedicated incremental sync helper so large directories can patch dirty rows without paying a full generic reconcile on every shell sync"
    );
    assert!(
        !bootstrap_sftp.contains("sync_vec_model(window.get_sftp_panel_items(), items"),
        "right-panel bootstrap should stop feeding SFTP rows through the generic full-list reconcile path once incremental row patching exists"
    );
}

#[test]
fn sftp_panel_virtualization_exposes_bounded_visible_window_for_large_directories() {
    let mut view_model = ShellViewModel::default();
    let mut session =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let session_id = session.file_browser_session_id.clone();
    session.entries = (0..200)
        .map(|index| SftpDirectoryEntry {
            id: format!("file-{index:03}"),
            name: format!("file-{index:03}.log"),
            path: format!("/srv/app/file-{index:03}.log"),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(index as u64),
            size_bytes: Some(index as u64),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        })
        .collect::<Vec<_>>();

    view_model.quick_browser_session_id = Some(session_id);
    view_model.set_file_browser_session(session);

    let _ = view_model.update_active_sftp_panel_viewport(0.0, 44.0 * 8.0);
    assert_eq!(
        view_model.active_sftp_panel_total_row_count(),
        201,
        "the virtualized panel should still know the full directory row count, including the parent row"
    );
    assert!(
        view_model.active_sftp_panel_render_rows().len()
            < view_model.active_sftp_panel_total_row_count(),
        "large directories should only expose a bounded visible window to Slint instead of the full row set"
    );
    assert_eq!(
        view_model.sftp_panel_top_spacer_height_px(),
        0.0,
        "the first viewport should start at the top of the full content range"
    );
    assert_eq!(
        view_model.sftp_panel_total_content_height_px(),
        201.0 * 44.0,
        "total content height should continue to represent the full directory so the scrollbar range stays correct"
    );
}

#[test]
fn sftp_panel_virtualization_updates_visible_window_and_spacers_when_scrolled() {
    let mut view_model = ShellViewModel::default();
    let mut session =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let session_id = session.file_browser_session_id.clone();
    session.entries = (0..200)
        .map(|index| SftpDirectoryEntry {
            id: format!("file-{index:03}"),
            name: format!("file-{index:03}.log"),
            path: format!("/srv/app/file-{index:03}.log"),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(index as u64),
            size_bytes: Some(index as u64),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        })
        .collect::<Vec<_>>();

    view_model.quick_browser_session_id = Some(session_id);
    view_model.set_file_browser_session(session);

    let _ = view_model.update_active_sftp_panel_viewport(0.0, 44.0 * 8.0);
    let initial_range = view_model.active_sftp_panel_visible_row_range();

    let _ = view_model.update_active_sftp_panel_viewport(-(44.0 * 60.0), 44.0 * 8.0);

    let next_range = view_model.active_sftp_panel_visible_row_range();
    assert!(
        next_range.start > initial_range.start,
        "scrolling down should move the visible window forward through the full row cache"
    );
    assert_eq!(
        view_model.sftp_panel_top_spacer_height_px(),
        next_range.start as f32 * 44.0,
        "top spacer height should match the number of skipped full-cache rows above the current window"
    );
    assert_eq!(
        view_model.sftp_panel_bottom_spacer_height_px(),
        (view_model.active_sftp_panel_total_row_count() - next_range.end) as f32 * 44.0,
        "bottom spacer height should preserve the remaining scroll range below the visible window"
    );
}

#[test]
fn sftp_panel_virtualization_keeps_non_visible_selection_changes_out_of_visible_dirty_patch_set() {
    let mut view_model = ShellViewModel::default();
    let mut session =
        FileBrowserSession::quick_browser(HostProfileRef::new("asset-prod"), "/srv/app");
    let session_id = session.file_browser_session_id.clone();
    session.entries = (0..200)
        .map(|index| SftpDirectoryEntry {
            id: format!("file-{index:03}"),
            name: format!("file-{index:03}.log"),
            path: format!("/srv/app/file-{index:03}.log"),
            kind: SftpDirectoryEntryKind::File,
            modified_unix_seconds: Some(index as u64),
            size_bytes: Some(index as u64),
            permissions_label: None,
            owner_label: None,
            group_label: None,
        })
        .collect::<Vec<_>>();

    view_model.quick_browser_session_id = Some(session_id);
    view_model.set_file_browser_session(session);
    let _ = view_model.update_active_sftp_panel_viewport(0.0, 44.0 * 8.0);
    assert!(view_model.mark_active_sftp_panel_render_clean());

    assert!(view_model.select_sftp_panel_entry("file-180"));
    assert!(
        view_model
            .active_sftp_panel_render_dirty_indices()
            .is_empty(),
        "selection changes outside the current visible window should not force row patches into the bounded UI model"
    );
    assert!(
        !view_model.active_sftp_panel_render_requires_full_resync(),
        "non-visible selection changes should stay incremental instead of forcing a full visible-window rebuild"
    );
}

#[test]
fn sftp_panel_virtualization_contract_is_threaded_through_bootstrap() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp source");

    assert!(
        bootstrap_sftp.contains("window.set_sftp_panel_total_content_height(")
            && bootstrap_sftp.contains("window.set_sftp_panel_top_spacer_height(")
            && bootstrap_sftp.contains("window.set_sftp_panel_bottom_spacer_height("),
        "bootstrap should sync total height and spacer heights so the right-panel scrollbar still represents the full directory while the row model stays windowed"
    );
    assert!(
        bootstrap_sftp
            .contains("window.on_sftp_panel_viewport_changed(move |viewport_y, visible_height| {"),
        "bootstrap should listen to viewport changes from the right panel so Rust can retarget the visible window instead of keeping a full directory-sized UI model"
    );
}
