//! Bootstrap SFTP binder module.

use super::*;
use crate::SftpBreadcrumbItem;
use crate::app::sftp::{SftpDirectoryEntry, normalize_remote_dir};
use crate::shell::view_model::PendingSftpContextAction;
use std::collections::HashSet;
use std::sync::{Arc, Mutex, OnceLock};

const SFTP_PARENT_ITEM_ID: &str = "__sftp_parent__";

#[derive(Clone, Default)]
struct TransferPauseRegistry {
    task_ids: Arc<Mutex<HashSet<String>>>,
}

impl TransferPauseRegistry {
    fn request_pause(&self, task_id: &str) -> bool {
        let mut task_ids = self.task_ids.lock().expect("lock transfer pause registry");
        task_ids.insert(task_id.to_string())
    }

    fn consume_for_queue(&self, queue: &crate::app::sftp::TransferQueue) -> bool {
        let Some(active_task_id) = queue
            .tasks
            .iter()
            .find(|task| {
                matches!(
                    task.state,
                    crate::app::sftp::TransferTaskState::Running
                        | crate::app::sftp::TransferTaskState::VerifyingResume
                )
            })
            .map(|task| task.id.as_str())
        else {
            return false;
        };

        let mut task_ids = self.task_ids.lock().expect("lock transfer pause registry");
        task_ids.remove(active_task_id)
    }
}

fn transfer_pause_registry() -> TransferPauseRegistry {
    static REGISTRY: OnceLock<TransferPauseRegistry> = OnceLock::new();
    REGISTRY.get_or_init(TransferPauseRegistry::default).clone()
}

#[derive(Clone, Copy)]
enum TransferReplayAction {
    Resume,
    Restart,
}

fn project_sftp_panel_item(row: &crate::shell::view_model::SftpPanelRenderRow) -> SftpPanelItem {
    SftpPanelItem {
        id: row.id.as_str().into(),
        name: row.name.as_str().into(),
        meta_label: row.meta_label.as_str().into(),
        type_label: row.type_label.as_str().into(),
        modified_label: row.modified_label.as_str().into(),
        size_label: row.size_label.as_str().into(),
        permissions_label: row.permissions_label.as_str().into(),
        owner_label: row.owner_label.as_str().into(),
        group_label: row.group_label.as_str().into(),
        icon_kind: row.icon_kind.as_str().into(),
        kind: row.kind.as_str().into(),
        selected: row.selected,
    }
}

fn replace_sftp_panel_items_model(
    rows: &[crate::shell::view_model::SftpPanelRenderRow],
    replace: impl FnOnce(ModelRc<SftpPanelItem>),
) {
    let rows = rows.iter().map(project_sftp_panel_item).collect::<Vec<_>>();
    replace(ModelRc::from(Rc::new(VecModel::from(rows))));
}

fn sync_sftp_panel_items_model(
    current: ModelRc<SftpPanelItem>,
    rows: &[crate::shell::view_model::SftpPanelRenderRow],
    dirty_indices: &[usize],
    force_full_resync: bool,
    replace: impl FnOnce(ModelRc<SftpPanelItem>),
) {
    if force_full_resync {
        replace_sftp_panel_items_model(rows, replace);
        return;
    }

    let Some(model) = current.as_any().downcast_ref::<VecModel<SftpPanelItem>>() else {
        replace_sftp_panel_items_model(rows, replace);
        return;
    };
    if model.row_count() != rows.len() {
        replace_sftp_panel_items_model(rows, replace);
        return;
    }

    for &index in dirty_indices {
        let Some(row) = rows.get(index) else {
            replace_sftp_panel_items_model(rows, replace);
            return;
        };
        model.set_row_data(index, project_sftp_panel_item(row));
    }
}

fn sftp_panel_items_model_matches_rows(
    current: &ModelRc<SftpPanelItem>,
    rows: &[crate::shell::view_model::SftpPanelRenderRow],
) -> bool {
    if current.row_count() != rows.len() {
        return false;
    }

    rows.iter().enumerate().all(|(index, row)| {
        current
            .row_data(index)
            .is_some_and(|item| item.id.as_str() == row.id && item.selected == row.selected)
    })
}

fn project_sftp_breadcrumb_items(path: &str) -> Vec<SftpBreadcrumbItem> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut crumbs = vec![SftpBreadcrumbItem {
        label: "/".into(),
        path: "/".into(),
        active: trimmed == "/",
    }];
    if trimmed == "/" {
        return crumbs;
    }

    let mut current = String::new();
    for segment in trimmed
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
    {
        current.push('/');
        current.push_str(segment);
        crumbs.push(SftpBreadcrumbItem {
            label: segment.into(),
            path: current.as_str().into(),
            active: current == trimmed,
        });
    }
    crumbs
}

pub(super) fn sync_sftp_panel_state(window: &AppWindow, state: &mut ShellViewModel) {
    window.set_sftp_panel_mode(state.sftp_panel_mode_id().into());
    window.set_sftp_panel_host_label(state.sftp_panel_host_label().into());
    window.set_sftp_panel_path(state.sftp_panel_path().into());
    window.set_sftp_panel_follow_mode(state.sftp_panel_follow_mode_id().into());
    window.set_sftp_panel_connection_badge(state.quick_browser_connection_badge().into());
    window.set_sftp_panel_binding_mode_label(state.quick_browser_binding_mode_label().into());
    window.set_sftp_panel_binding_mode_active(state.quick_browser_follows_active_terminal());
    window.set_sftp_panel_path_editing(state.quick_browser_path_editing());
    window.set_sftp_panel_can_go_back(state.sftp_panel_can_go_back());
    window.set_sftp_panel_can_go_forward(state.sftp_panel_can_go_forward());
    window.set_sftp_panel_can_go_up(state.sftp_panel_can_go_up());
    window.set_sftp_panel_actions_enabled(state.sftp_panel_actions_enabled());
    window.set_sftp_panel_sort_column(state.sftp_panel_sort_column_id().into());
    window.set_sftp_panel_sort_direction(state.sftp_panel_sort_direction_id().into());
    window.set_sftp_panel_name_column_width(state.sftp_panel_name_column_width_px());
    window.set_sftp_panel_type_column_width(state.sftp_panel_type_column_width_px());
    window.set_sftp_panel_modified_column_width(state.sftp_panel_modified_column_width_px());
    window.set_sftp_panel_size_column_width(state.sftp_panel_size_column_width_px());
    window.set_sftp_panel_total_content_height(state.sftp_panel_total_content_height_px());
    window.set_sftp_panel_top_spacer_height(state.sftp_panel_top_spacer_height_px());
    window.set_sftp_panel_bottom_spacer_height(state.sftp_panel_bottom_spacer_height_px());
    window.set_sftp_queue_drawer_open(state.sftp_queue_drawer_open());
    window.set_sftp_panel_drop_target_active(state.quick_browser_drop_target_active());

    let active_session_id = state.quick_browser_session_id().map(str::to_owned);
    let active_rows = state.active_sftp_panel_render_rows();
    let current_items = window.get_sftp_panel_items();
    let force_full_resync = active_session_id != state.sftp_panel_last_rendered_session_id
        || state.active_sftp_panel_render_requires_full_resync()
        || !sftp_panel_items_model_matches_rows(&current_items, active_rows);
    let dirty_row_indices = if force_full_resync {
        Vec::new()
    } else {
        state.active_sftp_panel_render_dirty_indices()
    };
    if force_full_resync || !dirty_row_indices.is_empty() {
        sync_sftp_panel_items_model(
            current_items,
            active_rows,
            dirty_row_indices.as_slice(),
            force_full_resync,
            |model| window.set_sftp_panel_items(model),
        );
        let _ = state.mark_active_sftp_panel_render_clean();
    }

    let selected_ids = state
        .sftp_panel_selected_entry_ids()
        .iter()
        .map(|entry_id| SharedString::from(entry_id.as_str()))
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_sftp_panel_selected_entry_ids(),
        selected_ids,
        |model| window.set_sftp_panel_selected_entry_ids(model),
    );

    let queue = &state.sftp_queue_summary;
    window.set_sftp_panel_queue_active(i32::try_from(queue.active_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_failed(i32::try_from(queue.failed_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_current_session(
        i32::try_from(queue.current_session_count).unwrap_or(i32::MAX),
    );
}

pub(super) fn sync_right_panel_state(window: &AppWindow, state: &mut ShellViewModel) {
    window.set_right_panel_view(state.right_panel_view_id().into());
    window.set_right_panel_expanded_width(state.right_panel_expanded_width_px());
    sync_sftp_panel_state(window, state);
}

pub(super) fn sync_workspace_sftp_state(window: &AppWindow, state: &mut ShellViewModel) {
    window.set_workspace_sftp_host_label(state.workspace_sftp_host_label().into());
    window.set_workspace_sftp_connection_label(state.workspace_sftp_connection_label().into());
    window.set_workspace_sftp_binding_label(state.workspace_sftp_binding_label().into());
    window.set_workspace_sftp_path(state.workspace_sftp_path().into());
    window.set_workspace_sftp_path_editing(state.workspace_sftp_path_editing());
    window.set_workspace_sftp_focus_sequence(state.workspace_sftp_focus_sequence());
    window.set_workspace_sftp_can_go_back(state.workspace_sftp_can_go_back());
    window.set_workspace_sftp_can_go_forward(state.workspace_sftp_can_go_forward());
    window.set_workspace_sftp_can_go_up(state.workspace_sftp_can_go_up());
    window.set_workspace_sftp_can_go_home(state.workspace_sftp_can_go_home());
    window.set_workspace_sftp_actions_enabled(state.workspace_sftp_actions_enabled());
    window.set_workspace_sftp_toolbar_disabled_reason(
        state.workspace_sftp_toolbar_disabled_reason().into(),
    );
    window.set_workspace_sftp_viewport_y(state.workspace_sftp_viewport_y());
    window.set_workspace_sftp_row_height(state.workspace_sftp_row_height_px());
    window.set_workspace_sftp_total_row_count(state.workspace_sftp_total_row_count_i32());
    window.set_workspace_sftp_total_content_height(state.workspace_sftp_total_content_height_px());
    window.set_workspace_sftp_top_spacer_height(state.workspace_sftp_top_spacer_height_px());
    window.set_workspace_sftp_bottom_spacer_height(state.workspace_sftp_bottom_spacer_height_px());
    window.set_workspace_sftp_sort_column(state.workspace_sftp_sort_column_id().into());
    window.set_workspace_sftp_sort_direction(state.workspace_sftp_sort_direction_id().into());

    let breadcrumb_items = project_sftp_breadcrumb_items(state.workspace_sftp_path().as_str());
    sync_vec_model(
        window.get_workspace_sftp_breadcrumb_items(),
        breadcrumb_items,
        |model| window.set_workspace_sftp_breadcrumb_items(model),
    );

    let selected_ids = state
        .workspace_sftp_selected_entry_ids()
        .iter()
        .map(|entry_id| SharedString::from(entry_id.as_str()))
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_workspace_sftp_selected_entry_ids(),
        selected_ids,
        |model| window.set_workspace_sftp_selected_entry_ids(model),
    );

    let active_session_id = state
        .active_workspace_sftp_session()
        .map(|session| session.file_browser_session_id.clone());
    let workspace_rows = state.workspace_sftp_render_rows();
    let current_items = window.get_workspace_sftp_items();
    let force_full_resync = active_session_id != state.workspace_sftp_last_rendered_session_id
        || state.workspace_sftp_render_requires_full_resync()
        || !sftp_panel_items_model_matches_rows(&current_items, workspace_rows);
    let dirty_row_indices = if force_full_resync {
        Vec::new()
    } else {
        state.workspace_sftp_render_dirty_indices()
    };
    if force_full_resync || !dirty_row_indices.is_empty() {
        sync_sftp_panel_items_model(
            current_items,
            workspace_rows,
            dirty_row_indices.as_slice(),
            force_full_resync,
            |model| window.set_workspace_sftp_items(model),
        );
        let _ = state.mark_workspace_sftp_render_clean();
    }
}

pub(super) fn sync_sftp_remote_file_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let editor = state.sftp_remote_file_editor_state();
    window.set_sftp_remote_file_modal_open(editor.open);
    window.set_sftp_remote_file_modal_title(editor.title.clone().into());
    window.set_sftp_remote_file_modal_path(editor.remote_path.clone().into());
    window.set_sftp_remote_file_modal_content(editor.content.clone().into());
    window.set_sftp_remote_file_modal_status_text(editor.status_text.clone().into());
    window.set_sftp_remote_file_modal_error_text(editor.error_text.clone().into());
    window.set_sftp_remote_file_modal_can_save(state.sftp_remote_file_editor_can_save());
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn sync_sftp_conflict_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let conflict = state.sftp_conflict_modal_state();
    window.set_sftp_conflict_modal_open(conflict.open);
    window.set_sftp_conflict_modal_kind(state.sftp_conflict_modal_kind_id().into());
    window.set_sftp_conflict_modal_source_path(conflict.source_path.clone().into());
    window.set_sftp_conflict_modal_target_path(conflict.target_path.clone().into());
    window.set_sftp_conflict_modal_batch_conflict_count(
        state.sftp_conflict_modal_batch_conflict_count(),
    );
    window.set_sftp_conflict_modal_apply_to_batch(state.sftp_conflict_modal_apply_to_batch());
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) type SftpBrowserBackgroundMessage = crate::app::sftp::SftpBrowserOperationResult;

#[derive(Debug)]
pub(super) struct SftpTransferBackgroundMessage {
    pub session_id: String,
    pub tasks: Vec<crate::app::sftp::TransferTask>,
    pub refresh_remote_path: Option<String>,
    pub error: Option<String>,
    pub open_queue_drawer: bool,
    pub open_transfer_center: bool,
}

#[derive(Debug)]
pub(super) enum SftpLocalActionBackgroundMessage {
    OpenFileFailed {
        error: String,
    },
    OpenFolderFailed {
        error: String,
    },
    RemoveFinished {
        task_id: String,
        error: Option<String>,
        missing_after_remove: bool,
    },
}

fn persist_transfer_tasks_snapshot(
    transfer_store: Option<&Arc<crate::app::sftp::RedbTransferStore>>,
    tasks: &[crate::app::sftp::TransferTask],
) {
    let Some(transfer_store) = transfer_store.cloned() else {
        return;
    };
    let tasks = tasks.to_vec();
    std::thread::spawn(move || {
        if let Err(err) = transfer_store.save_tasks(&tasks) {
            tracing::error!(
                target: "config.sftp_transfer_store",
                error = %err,
                "failed to persist SFTP transfer snapshot"
            );
        }
    });
}

fn apply_download_conflict_default(
    tasks: &mut [crate::app::sftp::TransferTask],
    default: crate::app::ui_preferences::DownloadConflictDefault,
) {
    let policy = match default {
        crate::app::ui_preferences::DownloadConflictDefault::Ask => None,
        crate::app::ui_preferences::DownloadConflictDefault::Overwrite => {
            Some(crate::app::sftp::TransferConflictPolicy::Overwrite)
        }
        crate::app::ui_preferences::DownloadConflictDefault::AutoRename => {
            Some(crate::app::sftp::TransferConflictPolicy::AutoRename)
        }
    };

    for task in tasks {
        if matches!(
            task.action,
            crate::app::sftp::TransferTaskAction::Download { .. }
                | crate::app::sftp::TransferTaskAction::DownloadDirectory { .. }
        ) {
            task.conflict_policy = policy;
        }
    }
}

fn project_transfer_queue_update(
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    session_id_text: &str,
    queue: &crate::app::sftp::TransferQueue,
) {
    let _ = result_tx.send(SftpTransferBackgroundMessage {
        session_id: session_id_text.into(),
        tasks: queue.tasks.clone(),
        refresh_remote_path: None,
        error: None,
        open_queue_drawer: false,
        open_transfer_center: false,
    });
}

fn run_transfer_queue_in_background(
    manager: SessionManager,
    session_id: Uuid,
    mut queue: crate::app::sftp::TransferQueue,
    refresh_remote_path: Option<String>,
    result_tx: std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    pause_registry: TransferPauseRegistry,
) {
    let session_id_text = session_id.to_string();
    std::thread::spawn(move || {
        project_transfer_queue_update(&result_tx, session_id_text.as_str(), &queue);
        let error = manager
            .sftp_execute_queued_transfers_with_progress(session_id, &mut queue, {
                let result_tx = result_tx.clone();
                let session_id_text = session_id_text.clone();
                let pause_registry = pause_registry.clone();
                move |queue| {
                    project_transfer_queue_update(&result_tx, session_id_text.as_str(), queue);
                    !pause_registry.consume_for_queue(queue)
                }
            })
            .err()
            .map(|err| err.to_string());
        let _ = result_tx.send(SftpTransferBackgroundMessage {
            session_id: session_id_text,
            tasks: queue.tasks.clone(),
            refresh_remote_path,
            error,
            open_queue_drawer: false,
            open_transfer_center: false,
        });
    });
}

pub(super) fn schedule_sftp_upload_paths(
    manager: &SessionManager,
    session_id: Uuid,
    target_dir: &str,
    local_paths: Vec<PathBuf>,
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    if local_paths.is_empty() {
        return false;
    }

    let manager = manager.clone();
    let result_tx = result_tx.clone();
    let target_dir = target_dir.to_string();
    let session_id_text = session_id.to_string();
    std::thread::spawn(move || {
        let mut queue = crate::app::sftp::TransferQueue::default();
        match crate::app::sftp::scan_local_sources(&local_paths) {
            Ok(sources) if !sources.is_empty() => {
                queue.enqueue_upload(session_id_text.as_str(), target_dir.as_str(), &sources);
                run_transfer_queue_in_background(
                    manager,
                    session_id,
                    queue,
                    Some(target_dir),
                    result_tx,
                    transfer_pause_registry(),
                );
            }
            Ok(_) => {}
            Err(err) => {
                let _ = result_tx.send(SftpTransferBackgroundMessage {
                    session_id: session_id_text,
                    tasks: Vec::new(),
                    refresh_remote_path: None,
                    error: Some(err.to_string()),
                    open_queue_drawer: false,
                    open_transfer_center: false,
                });
            }
        }
    });
    true
}

pub(super) fn schedule_sftp_download_entries(
    manager: &SessionManager,
    session_id: Uuid,
    local_root: PathBuf,
    entries: Vec<SftpDirectoryEntry>,
    download_conflict_default: crate::app::ui_preferences::DownloadConflictDefault,
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    if entries.is_empty() {
        return false;
    }

    let manager = manager.clone();
    let result_tx = result_tx.clone();
    let session_id_text = session_id.to_string();
    std::thread::spawn(move || {
        let mut queue = crate::app::sftp::TransferQueue::default();
        let download_targets =
            match manager.sftp_collect_download_targets(session_id, local_root.as_path(), &entries)
            {
                Ok(targets) if !targets.is_empty() => targets,
                Ok(_) => return,
                Err(err) => {
                    let _ = result_tx.send(SftpTransferBackgroundMessage {
                        session_id: session_id_text,
                        tasks: Vec::new(),
                        refresh_remote_path: None,
                        error: Some(err.to_string()),
                        open_queue_drawer: false,
                        open_transfer_center: false,
                    });
                    return;
                }
            };
        queue.enqueue_download_targets(session_id_text.as_str(), &download_targets);
        apply_download_conflict_default(&mut queue.tasks, download_conflict_default);
        run_transfer_queue_in_background(
            manager,
            session_id,
            queue,
            None,
            result_tx,
            transfer_pause_registry(),
        );
    });
    true
}

fn transfer_task_refresh_remote_path(task: &crate::app::sftp::TransferTask) -> Option<String> {
    match task.direction {
        crate::app::sftp::TransferDirection::Upload | crate::app::sftp::TransferDirection::Move => {
            remote_parent_dir(task.target_path.as_str())
        }
        crate::app::sftp::TransferDirection::Download
        | crate::app::sftp::TransferDirection::Delete => {
            remote_parent_dir(task.source_path.as_str())
        }
    }
}

fn remote_parent_dir(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "/" {
        return Some("/".into());
    }

    let normalized = trimmed.trim_end_matches('/');
    if normalized.is_empty() {
        return Some("/".into());
    }
    match normalized.rsplit_once('/') {
        Some(("", _)) => Some("/".into()),
        Some((parent, _)) => Some(parent.to_string()),
        None => Some("/".into()),
    }
}

fn prepare_task_for_replay(
    task: &crate::app::sftp::TransferTask,
    action: TransferReplayAction,
) -> Option<crate::app::sftp::TransferTask> {
    let eligible_for_resume = matches!(
        task.state,
        crate::app::sftp::TransferTaskState::Paused
            | crate::app::sftp::TransferTaskState::Interrupted
            | crate::app::sftp::TransferTaskState::Failed
    ) && task.resume_mode
        == crate::app::sftp::TransferResumeMode::ResumeIfPossible;
    let eligible_for_restart = matches!(
        task.state,
        crate::app::sftp::TransferTaskState::Paused
            | crate::app::sftp::TransferTaskState::Interrupted
            | crate::app::sftp::TransferTaskState::Failed
    );

    match action {
        TransferReplayAction::Resume if !eligible_for_resume => None,
        TransferReplayAction::Restart if !eligible_for_restart => None,
        _ => {
            let mut replay_task = task.clone();
            replay_task.state = crate::app::sftp::TransferTaskState::Queued;
            replay_task.error_message = None;
            if matches!(action, TransferReplayAction::Restart) {
                replay_task.bytes_transferred = 0;
                replay_task.bytes_confirmed = 0;
                replay_task.temp_target_path = None;
                replay_task.resume_mode = crate::app::sftp::TransferResumeMode::ResumeIfPossible;
            }
            Some(replay_task)
        }
    }
}

fn replay_transfer_task(
    manager: &SessionManager,
    task: &crate::app::sftp::TransferTask,
    action: TransferReplayAction,
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    let Ok(session_id) = Uuid::parse_str(task.session_id.as_str()) else {
        return false;
    };
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }
    let Some(replay_task) = prepare_task_for_replay(task, action) else {
        return false;
    };

    let manager = manager.clone();
    let result_tx = result_tx.clone();
    let refresh_remote_path = transfer_task_refresh_remote_path(task);
    let queue = crate::app::sftp::TransferQueue {
        tasks: vec![replay_task],
    };
    run_transfer_queue_in_background(
        manager,
        session_id,
        queue,
        refresh_remote_path,
        result_tx,
        transfer_pause_registry(),
    );
    true
}

pub(super) fn retry_transfer_task(
    manager: &SessionManager,
    task: &crate::app::sftp::TransferTask,
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    let action = if task.resume_mode == crate::app::sftp::TransferResumeMode::RestartOnly {
        TransferReplayAction::Restart
    } else {
        TransferReplayAction::Resume
    };
    replay_transfer_task(manager, task, action, result_tx)
}

pub(super) fn resolve_conflict_transfer_tasks(
    manager: &SessionManager,
    tasks: &[crate::app::sftp::TransferTask],
    policy: crate::app::sftp::TransferConflictPolicy,
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    let Some(first_task) = tasks.first() else {
        return false;
    };
    let Ok(session_id) = Uuid::parse_str(first_task.session_id.as_str()) else {
        return false;
    };
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    let resumed_tasks = tasks
        .iter()
        .filter(|task| {
            task.state == crate::app::sftp::TransferTaskState::Conflict
                && task.session_id == first_task.session_id
        })
        .cloned()
        .map(|mut task| {
            task.state = crate::app::sftp::TransferTaskState::Queued;
            task.bytes_transferred = 0;
            task.conflict_policy = Some(policy);
            task.error_message = None;
            task
        })
        .collect::<Vec<_>>();
    if resumed_tasks.is_empty() {
        return false;
    }

    let manager = manager.clone();
    let result_tx = result_tx.clone();
    let refresh_remote_path = transfer_task_refresh_remote_path(first_task);
    let queue = crate::app::sftp::TransferQueue {
        tasks: resumed_tasks,
    };
    run_transfer_queue_in_background(
        manager,
        session_id,
        queue,
        refresh_remote_path,
        result_tx,
        TransferPauseRegistry::default(),
    );
    true
}

pub(super) fn open_transfer_task_in_workspace(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
    task_id: &str,
) -> bool {
    let opened = state.open_transfer_task_in_sftp_workspace(task_id);
    if !opened {
        return false;
    }
    let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
        return opened;
    };
    let request = controller.open_file_browser_session(browser_session);
    queue_sftp_browser_request(
        state,
        controller,
        manager,
        request,
        async_runtime,
        result_tx,
    ) || opened
}

pub(super) fn sync_active_sftp_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> bool {
    let Some(active_session_id_text) = state
        .active_workspace_terminal_session_id()
        .map(str::to_string)
    else {
        return false;
    };
    if !state.quick_browser_follows_active_terminal() && state.quick_browser_session().is_some() {
        return false;
    }

    let Some(session_id) = Uuid::parse_str(&active_session_id_text).ok() else {
        return false;
    };

    let binding = manager.sftp_binding(session_id);
    let cwd = manager.current_working_directory(session_id);
    let Some(binding) = binding else {
        return false;
    };

    let previous_quick_browser_session_id = state.quick_browser_session_id.clone();
    let previous_pending_terminal_session_id = state
        .quick_browser_state
        .pending_terminal_session_id
        .clone();
    let defer_display_switch = state
        .quick_browser_state
        .pending_terminal_session_id
        .as_deref()
        == Some(active_session_id_text.as_str())
        && previous_quick_browser_session_id.as_deref() != Some(active_session_id_text.as_str());
    let host_profile_ref = state
        .active_workspace_tab()
        .map(|tab| {
            crate::app::sftp::HostProfileRef::with_label(tab.asset_id.clone(), tab.title.clone())
        })
        .unwrap_or_else(|| crate::app::sftp::HostProfileRef::new("active-session"));
    let initial_path = cwd.clone().unwrap_or_else(|| "/".to_string());
    let (session_changed, can_promote_display_target) = {
        let session_state = state
            .file_browser_sessions
            .entry(active_session_id_text.clone())
            .or_insert_with(|| {
                let mut session = crate::app::sftp::FileBrowserSession::quick_browser(
                    host_profile_ref.clone(),
                    initial_path,
                );
                session.file_browser_session_id = active_session_id_text.clone();
                session.attach_terminal_session_id(active_session_id_text.clone());
                session
            });
        session_state.attach_terminal_session_id(active_session_id_text.as_str());
        let before = session_state.clone();

        match binding.mode() {
            SftpPanelMode::Disconnected => session_state.mark_disconnected(),
            _ if matches!(
                session_state.mode,
                SftpPanelMode::Empty | SftpPanelMode::Disconnected
            ) =>
            {
                session_state.mark_connecting()
            }
            _ => {}
        }

        if let Some(cwd) = cwd {
            if session_state.current_path.is_empty() {
                session_state.reenable_follow(cwd);
            } else if session_state.follow_mode == SftpFollowMode::FollowCwd {
                session_state.follow_terminal_path(cwd);
            }
        }

        let can_promote_display_target = !session_state.entries.is_empty()
            || matches!(
                session_state.mode,
                SftpPanelMode::Ready | SftpPanelMode::Error | SftpPanelMode::Disconnected
            );
        (before != *session_state, can_promote_display_target)
    };

    if !defer_display_switch || can_promote_display_target {
        state.quick_browser_session_id = Some(active_session_id_text.clone());
        if state
            .quick_browser_state
            .pending_terminal_session_id
            .as_deref()
            == Some(active_session_id_text.as_str())
        {
            state.quick_browser_state.pending_terminal_session_id = None;
        }
    }

    previous_quick_browser_session_id != state.quick_browser_session_id
        || previous_pending_terminal_session_id
            != state.quick_browser_state.pending_terminal_session_id
        || session_changed
}

pub(super) fn project_sftp_browser_state_into_view_model(
    state: &mut ShellViewModel,
    browser_session_id: &str,
    browser_state: &SftpBrowserSessionState,
) -> bool {
    let mut next = state
        .file_browser_sessions
        .get(browser_session_id)
        .cloned()
        .unwrap_or_else(|| {
            let mut session = crate::app::sftp::FileBrowserSession::quick_browser(
                crate::app::sftp::HostProfileRef::new("active-session"),
                browser_state.current_path.clone(),
            );
            session.file_browser_session_id = browser_session_id.to_string();
            session
        });
    next.mode = browser_state.mode;
    next.follow_mode = browser_state.follow_mode;
    next.current_path = browser_state.current_path.clone();
    next.history = browser_state.history.clone();
    next.entries = browser_state.entries.clone();
    next.selected_entry_ids = browser_state.selected_entry_ids.clone();
    next.last_error = browser_state.last_error.clone();
    next.active_request_id = browser_state.active_request_id;
    let promote_pending_terminal = state.quick_browser_follows_active_terminal()
        && state
            .quick_browser_state
            .pending_terminal_session_id
            .as_deref()
            == Some(browser_session_id)
        && matches!(
            browser_state.mode,
            SftpPanelMode::Ready | SftpPanelMode::Error | SftpPanelMode::Disconnected
        );
    if state.file_browser_sessions.get(browser_session_id) == Some(&next)
        && !promote_pending_terminal
    {
        return false;
    }
    state.set_file_browser_session(next);
    if promote_pending_terminal {
        state.quick_browser_session_id = Some(browser_session_id.to_string());
        state.quick_browser_state.pending_terminal_session_id = None;
    }
    true
}

#[allow(dead_code)]
pub(super) fn execute_sftp_browser_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    request: SftpBrowserLoadRequest,
) -> bool {
    let (result_tx, result_rx) = std::sync::mpsc::channel();
    crate::app::sftp::dispatch_sftp_load_dir_operation(
        &manager.runtime_handle(),
        manager.clone(),
        request,
        result_tx,
    );
    let Ok(message) = result_rx.recv() else {
        return false;
    };
    apply_sftp_browser_background_message(state, controller, message)
}

fn project_pending_sftp_browser_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    browser_session_id: &str,
) -> bool {
    controller
        .browser_session_state(browser_session_id)
        .is_some_and(|browser_state| {
            project_sftp_browser_state_into_view_model(state, browser_session_id, browser_state)
        })
}

fn queue_sftp_browser_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    request: SftpBrowserLoadRequest,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let pending_changed = project_pending_sftp_browser_request(
        state,
        controller,
        request.file_browser_session_id.as_str(),
    );

    if !controller.mark_request_in_flight(request.request_id) {
        return pending_changed;
    }

    let runtime_handle = async_runtime
        .cloned()
        .unwrap_or_else(|| manager.runtime_handle());
    crate::app::sftp::dispatch_sftp_load_dir_operation(
        &runtime_handle,
        manager.clone(),
        request,
        result_tx.clone(),
    );

    pending_changed
}

pub(super) fn apply_sftp_browser_background_message(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    message: SftpBrowserBackgroundMessage,
) -> bool {
    controller.complete_request(message.request.request_id);
    if message.kind != crate::app::sftp::SftpOperationKind::LoadDir {
        return false;
    }
    match message.result {
        Ok(entries) => controller.apply_loaded_directory_for_browser_session(
            message.request.file_browser_session_id.as_str(),
            message.request.generation,
            message.request.request_id,
            message.request.path.as_str(),
            entries,
        ),
        Err(error) => {
            if message.disconnected {
                controller.apply_disconnected_for_browser_session(
                    message.request.file_browser_session_id.as_str(),
                    message.request.generation,
                    message.request.request_id,
                );
            } else {
                controller.apply_load_error_for_browser_session(
                    message.request.file_browser_session_id.as_str(),
                    message.request.generation,
                    message.request.request_id,
                    message.request.path.as_str(),
                    error,
                );
            }
        }
    }

    project_pending_sftp_browser_request(
        state,
        controller,
        message.request.file_browser_session_id.as_str(),
    )
}

pub(super) fn drain_sftp_browser_background_messages(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    result_rx: &std::sync::mpsc::Receiver<SftpBrowserBackgroundMessage>,
) -> bool {
    let mut changed = false;
    loop {
        let Ok(message) = result_rx.try_recv() else {
            break;
        };
        changed |= apply_sftp_browser_background_message(state, controller, message);
    }
    changed
}

pub(super) fn drain_sftp_transfer_background_messages(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    transfer_store: Option<&Arc<crate::app::sftp::RedbTransferStore>>,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    browser_result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
    result_rx: &std::sync::mpsc::Receiver<SftpTransferBackgroundMessage>,
) -> bool {
    let mut changed = false;
    let mut tasks_changed = false;
    loop {
        let Ok(message) = result_rx.try_recv() else {
            break;
        };

        let merged = state.merge_sftp_transfer_tasks(&message.tasks);
        changed |= merged;
        tasks_changed |= merged;
        changed |= state.recompute_sftp_queue_summary();
        if message.open_queue_drawer && !state.sftp_queue_drawer_open {
            state.sftp_queue_drawer_open = true;
            changed = true;
        }
        if message.open_transfer_center && !state.transfer_center_open() {
            state.toggle_transfer_center();
            changed = true;
        }
        if state.settings_modal_download_conflict_default()
            == crate::app::ui_preferences::DownloadConflictDefault::Ask
            && !state.sftp_conflict_modal_state().open
            && let Some(conflict_task) = message.tasks.iter().find(|task| {
                task.state == crate::app::sftp::TransferTaskState::Conflict
                    && matches!(
                        task.action,
                        crate::app::sftp::TransferTaskAction::Download { .. }
                            | crate::app::sftp::TransferTaskAction::DownloadDirectory { .. }
                    )
            })
        {
            changed |= state.open_transfer_conflict_modal(conflict_task.id.as_str());
        }

        if let Some(error) = message.error.as_deref() {
            tracing::error!(
                target: "app.sftp",
                session_id = message.session_id.as_str(),
                error,
                "background SFTP transfer finished with an error"
            );
            let prefix = if message.tasks.is_empty() && message.refresh_remote_path.is_none() {
                "Operation failed"
            } else {
                "Transfer failed"
            };
            state.show_transfer_center_feedback("error", format!("{prefix}: {error}"));
            changed = true;
        }

        if let Some(refresh_remote_path) = message.refresh_remote_path.as_deref()
            && let Ok(session_id) = Uuid::parse_str(message.session_id.as_str())
        {
            let matching_browser_session_ids = state
                .file_browser_sessions
                .values()
                .filter(|browser_session| {
                    browser_session.linked_terminal_session_id.as_deref()
                        == Some(message.session_id.as_str())
                        && browser_session.current_path == refresh_remote_path
                })
                .map(|browser_session| browser_session.file_browser_session_id.clone())
                .collect::<Vec<_>>();

            for browser_session_id in matching_browser_session_ids {
                if let Some(request) =
                    controller.refresh_browser_session(browser_session_id.as_str(), session_id)
                {
                    changed |= queue_sftp_browser_request(
                        state,
                        controller,
                        manager,
                        request,
                        async_runtime,
                        browser_result_tx,
                    );
                }
            }
        }
    }

    if tasks_changed {
        persist_transfer_tasks_snapshot(transfer_store, state.sftp_transfer_tasks());
    }

    changed
}

pub(super) fn drain_sftp_local_action_background_messages(
    state: &mut ShellViewModel,
    transfer_store: Option<&Arc<crate::app::sftp::RedbTransferStore>>,
    result_rx: &std::sync::mpsc::Receiver<SftpLocalActionBackgroundMessage>,
) -> bool {
    let mut changed = false;
    let mut tasks_changed = false;
    loop {
        let Ok(message) = result_rx.try_recv() else {
            break;
        };

        match message {
            SftpLocalActionBackgroundMessage::OpenFileFailed { error } => {
                state.show_transfer_center_feedback("error", format!("Open File failed: {error}"));
                changed = true;
            }
            SftpLocalActionBackgroundMessage::OpenFolderFailed { error } => {
                state
                    .show_transfer_center_feedback("error", format!("Open Folder failed: {error}"));
                changed = true;
            }
            SftpLocalActionBackgroundMessage::RemoveFinished {
                task_id,
                error,
                missing_after_remove,
            } => {
                if let Some(error) = error {
                    state.show_transfer_center_feedback("error", format!("Remove failed: {error}"));
                    changed = true;
                    continue;
                }

                if state.remove_transfer_task(task_id.as_str()) {
                    if missing_after_remove {
                        state.show_transfer_center_feedback(
                            "neutral",
                            "The local file is already missing, so only the transfer record was removed.",
                        );
                    }
                    changed = true;
                    tasks_changed = true;
                }
            }
        }
    }

    if tasks_changed {
        persist_transfer_tasks_snapshot(transfer_store, state.sftp_transfer_tasks());
    }
    changed
}

#[allow(dead_code)]
fn sftp_remote_file_title(remote_path: &str) -> String {
    remote_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Remote File")
        .to_string()
}

#[allow(dead_code)]
// Legacy modal fallback kept around while the default SFTP file actions use local files.
fn open_sftp_remote_file_editor_for_entry(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    session_id: Uuid,
    remote_path: &str,
) {
    match manager.sftp_download_file(session_id, remote_path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                text,
                "Editing remote text file".to_string(),
                String::new(),
            ),
            Err(err) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                String::from_utf8_lossy(err.as_bytes()).into_owned(),
                "View only".to_string(),
                "Only UTF-8 text files can be edited online right now.".to_string(),
            ),
        },
        Err(err) => state.open_sftp_remote_file_editor(
            session_id.to_string(),
            remote_path.to_string(),
            sftp_remote_file_title(remote_path),
            String::new(),
            "Open failed".to_string(),
            format!("Failed to open remote file: {err}"),
        ),
    }
}

fn start_sftp_working_copy_upload_monitor(
    manager: SessionManager,
    working_copy: crate::app::sftp::SftpWorkingCopy,
) {
    if !working_copy.upload_on_save {
        return;
    }

    let runtime_handle = manager.runtime_handle();
    std::thread::spawn(move || {
        let mut last_snapshot =
            crate::app::sftp::snapshot_working_copy(working_copy.local_path.as_path());
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            let next_snapshot =
                crate::app::sftp::snapshot_working_copy(working_copy.local_path.as_path());
            if !crate::app::sftp::working_copy_has_changed(&last_snapshot, &next_snapshot) {
                continue;
            }
            last_snapshot = next_snapshot;

            let Ok(bytes) = std::fs::read(working_copy.local_path.as_path()) else {
                continue;
            };
            let manager = manager.clone();
            let session_id = working_copy.session_id;
            let remote_path = working_copy.remote_path.clone();
            runtime_handle.spawn(async move {
                if let Err(err) = manager
                    .sftp_upload_file_async(session_id, remote_path.as_str(), bytes)
                    .await
                {
                    tracing::error!(
                        target: "app.sftp",
                        remote_path,
                        error = %err,
                        "failed to upload a saved local SFTP working copy"
                    );
                }
            });
        }
    });
}

fn queue_sftp_local_file_action(
    manager: &SessionManager,
    session_id: Uuid,
    remote_path: &str,
    action: crate::app::sftp::SftpOpenAction,
) -> bool {
    let Ok(local_path) = crate::app::sftp::prepare_local_open_path(session_id, remote_path, action)
    else {
        return false;
    };

    let manager = manager.clone();
    let remote_path = remote_path.to_string();
    manager.runtime_handle().spawn(async move {
        let bytes = match manager
            .sftp_download_file_async(session_id, remote_path.as_str())
            .await
        {
            Ok(bytes) => bytes,
            Err(err) => {
                tracing::error!(
                    target: "app.sftp",
                    remote_path,
                    error = %err,
                    "failed to download a remote SFTP file for local open"
                );
                return;
            }
        };

        if let Some(parent) = local_path.parent()
            && let Err(err) = std::fs::create_dir_all(parent)
        {
            tracing::error!(
                target: "app.sftp",
                local_path = %local_path.display(),
                error = %err,
                "failed to create the local SFTP staging directory"
            );
            return;
        }
        if let Err(err) = std::fs::write(local_path.as_path(), &bytes) {
            tracing::error!(
                target: "app.sftp",
                local_path = %local_path.display(),
                error = %err,
                "failed to persist the downloaded SFTP file locally"
            );
            return;
        }

        if action == crate::app::sftp::SftpOpenAction::EditLocally {
            let working_copy = crate::app::sftp::SftpWorkingCopy::new(
                session_id,
                remote_path.clone(),
                local_path.clone(),
                true,
            );
            start_sftp_working_copy_upload_monitor(manager.clone(), working_copy);
        }

        if let Err(err) = crate::app::sftp::open_path_locally(local_path.as_path()) {
            tracing::warn!(
                target: "app.sftp",
                local_path = %local_path.display(),
                error = %err,
                "failed to hand off downloaded SFTP file to the local platform opener"
            );
        }
    });
    true
}

fn queue_transfer_center_open_file_action(
    task_id: &str,
    local_path: Option<PathBuf>,
    result_tx: &std::sync::mpsc::Sender<SftpLocalActionBackgroundMessage>,
) {
    let task_id = task_id.to_string();
    let result_tx = result_tx.clone();
    std::thread::spawn(move || {
        let outcome = match local_path {
            Some(local_path) => crate::app::sftp::open_path_locally(local_path.as_path())
                .map_err(|err| err.to_string()),
            None => Err("The local file is no longer available.".into()),
        };

        if let Err(error) = outcome {
            tracing::warn!(
                target: "app.sftp",
                task_id,
                error,
                "failed to open a transfer-center file action locally"
            );
            let _ = result_tx.send(SftpLocalActionBackgroundMessage::OpenFileFailed { error });
        }
    });
}

fn queue_transfer_center_open_folder_action(
    task_id: &str,
    local_path: Option<PathBuf>,
    result_tx: &std::sync::mpsc::Sender<SftpLocalActionBackgroundMessage>,
) {
    let task_id = task_id.to_string();
    let result_tx = result_tx.clone();
    std::thread::spawn(move || {
        let outcome = match local_path {
            Some(local_path) => crate::app::sftp::reveal_path_locally(local_path.as_path())
                .map_err(|err| err.to_string()),
            None => Err("The local folder is no longer available.".into()),
        };

        if let Err(error) = outcome {
            tracing::warn!(
                target: "app.sftp",
                task_id,
                error,
                "failed to open a transfer-center folder action locally"
            );
            let _ = result_tx.send(SftpLocalActionBackgroundMessage::OpenFolderFailed { error });
        }
    });
}

fn queue_transfer_center_remove_action(
    task_id: &str,
    local_path: Option<PathBuf>,
    missing_download: bool,
    result_tx: &std::sync::mpsc::Sender<SftpLocalActionBackgroundMessage>,
) {
    let task_id = task_id.to_string();
    let result_tx = result_tx.clone();
    std::thread::spawn(move || {
        let trash_result = local_path
            .as_ref()
            .map(|local_path| crate::app::sftp::trash_path_locally(local_path.as_path()));
        let missing_after_trash = trash_result
            .as_ref()
            .and_then(|result| result.as_ref().err())
            .is_some_and(|err| err.to_string().contains("Local file already missing."))
            || missing_download;
        let error = if let Some(Err(err)) = trash_result {
            if err.to_string().contains("Local file already missing.") {
                None
            } else {
                tracing::warn!(
                    target: "app.sftp",
                    task_id,
                    error = %err,
                    "failed to move a downloaded transfer artifact to Trash"
                );
                Some(err.to_string())
            }
        } else {
            None
        };

        let _ = result_tx.send(SftpLocalActionBackgroundMessage::RemoveFinished {
            task_id,
            error,
            missing_after_remove: missing_after_trash,
        });
    });
}

pub(super) fn initial_sftp_browser_path(
    manager: &SessionManager,
    session_id: Uuid,
) -> Option<String> {
    if let Some(cwd) = manager.current_working_directory(session_id) {
        return Some(cwd);
    }

    manager
        .sftp_binding(session_id)
        .filter(|binding| binding.mode() != SftpPanelMode::Disconnected)
        .map(|_| "/".to_string())
}

fn quick_browser_terminal_session_uuid(state: &ShellViewModel) -> Option<Uuid> {
    state
        .quick_browser_linked_terminal_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

fn active_sftp_terminal_session_uuid(state: &ShellViewModel) -> Option<Uuid> {
    state
        .active_sftp_linked_terminal_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

fn active_workspace_sftp_request_identity(state: &ShellViewModel) -> Option<(String, Uuid)> {
    let browser_session = state.active_workspace_sftp_session()?;
    let session_id = browser_session
        .linked_terminal_session_id
        .as_deref()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())?;
    Some((browser_session.file_browser_session_id.clone(), session_id))
}

fn schedule_active_sftp_upload_from_paths(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    local_paths: Vec<PathBuf>,
) -> bool {
    let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
        return false;
    };
    let target_dir = state.active_sftp_path();
    if target_dir.trim().is_empty() {
        return false;
    }

    let scheduled = schedule_sftp_upload_paths(
        manager,
        session_id,
        target_dir.as_str(),
        local_paths,
        transfer_result_tx,
    );
    if scheduled {
        state.sftp_queue_drawer_open = true;
    }
    scheduled
}

fn schedule_active_sftp_download_selection(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    entry_ids: &[String],
    local_root: PathBuf,
) -> bool {
    let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
        return false;
    };
    let entries = state
        .active_sftp_session_state()
        .map(|session| {
            session
                .entries
                .iter()
                .filter(|entry| entry_ids.iter().any(|entry_id| entry_id == &entry.id))
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if entries.is_empty() {
        return false;
    }

    let scheduled = schedule_sftp_download_entries(
        manager,
        session_id,
        local_root,
        entries,
        state.settings_modal_download_conflict_default(),
        transfer_result_tx,
    );
    if scheduled {
        state.sftp_queue_drawer_open = true;
    }
    scheduled
}

pub(super) fn schedule_quick_browser_upload_from_paths(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    local_paths: Vec<PathBuf>,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    let target_dir = state.sftp_panel_path();
    if target_dir.trim().is_empty() {
        return false;
    }

    let scheduled = schedule_sftp_upload_paths(
        manager,
        session_id,
        target_dir.as_str(),
        local_paths,
        transfer_result_tx,
    );
    if scheduled {
        state.sftp_queue_drawer_open = true;
        let _ = state.set_quick_browser_drop_target_active(false);
    }
    scheduled
}

fn send_terminal_drop_transfer_center_request(
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    session_id: Uuid,
) {
    let _ = result_tx.send(SftpTransferBackgroundMessage {
        session_id: session_id.to_string(),
        tasks: Vec::new(),
        refresh_remote_path: None,
        error: None,
        open_queue_drawer: true,
        open_transfer_center: true,
    });
}

fn send_terminal_drop_error(
    result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    session_id: Uuid,
    error: String,
) {
    let _ = result_tx.send(SftpTransferBackgroundMessage {
        session_id: session_id.to_string(),
        tasks: Vec::new(),
        refresh_remote_path: None,
        error: Some(error),
        open_queue_drawer: false,
        open_transfer_center: false,
    });
}

fn schedule_terminal_cwd_upload_from_paths(
    manager: &SessionManager,
    session_id: Uuid,
    target_dir: String,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    local_paths: Vec<PathBuf>,
) -> anyhow::Result<bool> {
    let path_count = local_paths.len();
    tracing::info!(
        target: "app.drop",
        target = "terminal",
        session_id = %session_id,
        remote_dir = target_dir.as_str(),
        path_count,
        "scheduling terminal external drop upload"
    );
    let _ = manager
        .sftp_read_dir(session_id, target_dir.as_str())
        .map_err(|err| {
            anyhow::anyhow!(
                "the remote working directory `{}` is not ready for upload: {err}",
                target_dir
            )
        })?;

    let scheduled = schedule_sftp_upload_paths(
        manager,
        session_id,
        target_dir.as_str(),
        local_paths,
        transfer_result_tx,
    );
    if scheduled {
        send_terminal_drop_transfer_center_request(transfer_result_tx, session_id);
    }
    tracing::info!(
        target: "app.drop",
        target = "terminal",
        scheduled,
        path_count,
        "terminal external drop upload scheduling finished"
    );
    Ok(scheduled)
}

fn terminal_surface_allows_interactive_zmodem_drop(state: &ShellViewModel) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface() else {
        return false;
    };
    if surface.alternate_screen_active || surface.mouse_grabbed {
        return false;
    }

    let shell = surface.shell_integration;
    !shell.has_markers || (shell.input_active && !shell.command_running)
}

fn local_paths_are_zmodem_files(local_paths: &[PathBuf]) -> bool {
    !local_paths.is_empty() && local_paths.iter().all(|path| path.is_file())
}

fn active_workspace_session_uuid_for_terminal(state: &ShellViewModel) -> anyhow::Result<Uuid> {
    state
        .active_workspace_terminal_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
        .ok_or_else(|| anyhow::anyhow!("the active workspace session is not a terminal"))
}

fn terminal_current_working_directory_for_drop(
    manager: &SessionManager,
    session_id: Uuid,
) -> anyhow::Result<Option<String>> {
    if let Some(cwd) = manager.current_working_directory(session_id) {
        return Ok(Some(cwd));
    }

    tracing::info!(
        target: "app.drop",
        target = "terminal",
        session_id = %session_id,
        "terminal external drop has no tracked cwd; probing remote shell cwd"
    );
    let Some(cwd) = manager.resolve_current_working_directory(session_id)? else {
        return Ok(None);
    };
    tracing::info!(
        target: "app.drop",
        target = "terminal",
        session_id = %session_id,
        remote_dir = cwd.as_str(),
        "terminal external drop resolved cwd via remote probe"
    );
    Ok(Some(cwd))
}

fn schedule_terminal_zmodem_drop_from_paths(
    manager: &SessionManager,
    session_id: Uuid,
    local_paths: Vec<PathBuf>,
    allow_interactive_fallback: bool,
) -> anyhow::Result<bool> {
    if !local_paths_are_zmodem_files(local_paths.as_slice()) {
        return Ok(false);
    }
    if let Some(zmodem_state) = manager.zmodem_state(session_id) {
        if zmodem_state.phase == ZmodemTransferPhase::AwaitingUploadSelection {
            tracing::info!(
                target: "app.drop",
                target = "terminal",
                method = "zmodem",
                session_id = %session_id,
                path_count = local_paths.len(),
                "terminal external drop reusing active rz receiver"
            );
            manager.start_zmodem_upload(session_id, local_paths)?;
            return Ok(true);
        }
    }

    let rz_available = manager.remote_command_exists(session_id, "rz")?;
    tracing::info!(
        target: "app.drop",
        target = "terminal",
        method = "zmodem",
        session_id = %session_id,
        rz_available,
        path_count = local_paths.len(),
        "terminal external drop probed remote rz"
    );
    if !rz_available {
        tracing::info!(
            target: "app.drop",
            target = "terminal",
            method = "sftp",
            session_id = %session_id,
            path_count = local_paths.len(),
            "terminal external drop did not find remote rz; falling back to sftp"
        );
        return Ok(false);
    }

    let Some(remote_dir) = terminal_current_working_directory_for_drop(manager, session_id)? else {
        if !allow_interactive_fallback {
            anyhow::bail!(
                "the active terminal has not reported its current working directory yet, and it is not safe to start interactive rz from the current terminal state"
            );
        }
        tracing::info!(
            target: "app.drop",
            target = "terminal",
            method = "zmodem",
            session_id = %session_id,
            path_count = local_paths.len(),
            "terminal external drop starting interactive rz fallback because cwd is unavailable"
        );
        manager.start_interactive_zmodem_upload(session_id, local_paths)?;
        return Ok(true);
    };

    tracing::info!(
        target: "app.drop",
        target = "terminal",
        method = "zmodem",
        session_id = %session_id,
        remote_dir = remote_dir.as_str(),
        path_count = local_paths.len(),
        "terminal external drop starting rz over dedicated exec channel"
    );
    manager.start_zmodem_upload_to_remote_dir(session_id, local_paths, remote_dir)?;
    Ok(true)
}

fn schedule_terminal_external_drop_from_paths(
    state: &ShellViewModel,
    manager: &SessionManager,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    local_paths: Vec<PathBuf>,
) -> anyhow::Result<bool> {
    if local_paths.is_empty() {
        return Ok(false);
    }
    let session_id = active_workspace_session_uuid_for_terminal(state)?;
    let allow_interactive_zmodem = terminal_surface_allows_interactive_zmodem_drop(state);
    let manager = manager.clone();
    let transfer_result_tx = transfer_result_tx.clone();
    std::thread::spawn(move || {
        let result = (|| -> anyhow::Result<()> {
            if local_paths_are_zmodem_files(local_paths.as_slice())
                && schedule_terminal_zmodem_drop_from_paths(
                    &manager,
                    session_id,
                    local_paths.clone(),
                    allow_interactive_zmodem,
                )?
            {
                return Ok(());
            }

            let target_dir = terminal_current_working_directory_for_drop(&manager, session_id)?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "the active terminal has not reported its current working directory yet, and rz is not available for ZMODEM upload"
                    )
                })?;
            schedule_terminal_cwd_upload_from_paths(
                &manager,
                session_id,
                target_dir,
                &transfer_result_tx,
                local_paths,
            )?;
            Ok(())
        })();

        if let Err(err) = result {
            send_terminal_drop_error(&transfer_result_tx, session_id, err.to_string());
        }
    });
    Ok(true)
}

pub(super) fn workspace_terminal_accepts_external_drop(
    state: &ShellViewModel,
    manager: Option<&SessionManager>,
) -> bool {
    let Some(_manager) = manager else {
        return false;
    };
    active_workspace_session_uuid_for_terminal(state).is_ok()
}

pub(super) fn apply_pending_sftp_context_action(
    state: &mut ShellViewModel,
    session_bridge: Option<&ShellSessionBridge>,
    controller: &mut SftpBrowserController,
    async_runtime: Option<&tokio::runtime::Handle>,
    browser_result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
    transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
) -> bool {
    let Some(action) = state.take_pending_sftp_context_action() else {
        return false;
    };
    let Some(session_bridge) = session_bridge else {
        return false;
    };

    match action {
        PendingSftpContextAction::OpenRemote { entry_id } => {
            let Some(entry) = state.active_sftp_entry(entry_id.as_str()).cloned() else {
                return false;
            };
            if entry.kind != SftpDirectoryEntryKind::Directory {
                return false;
            }
            let Some((browser_session_id, session_id)) = state
                .active_file_browser_session_id()
                .map(str::to_owned)
                .zip(active_sftp_terminal_session_uuid(state))
            else {
                return false;
            };
            let Some(request) = controller.navigate_browser_session(
                browser_session_id.as_str(),
                session_id,
                entry.path.as_str(),
            ) else {
                return false;
            };
            queue_sftp_browser_request(
                state,
                controller,
                &session_bridge.manager,
                request,
                async_runtime,
                browser_result_tx,
            )
        }
        PendingSftpContextAction::EditLocally { entry_id } => {
            let Some(entry) = state.active_sftp_entry(entry_id.as_str()).cloned() else {
                return false;
            };
            let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
                return false;
            };
            entry.kind == SftpDirectoryEntryKind::File
                && queue_sftp_local_file_action(
                    &session_bridge.manager,
                    session_id,
                    entry.path.as_str(),
                    crate::app::sftp::SftpOpenAction::EditLocally,
                )
        }
        PendingSftpContextAction::Refresh => {
            let Some((browser_session_id, session_id)) = state
                .active_file_browser_session_id()
                .map(str::to_owned)
                .zip(active_sftp_terminal_session_uuid(state))
            else {
                return false;
            };
            controller
                .refresh_browser_session(browser_session_id.as_str(), session_id)
                .is_some_and(|request| {
                    queue_sftp_browser_request(
                        state,
                        controller,
                        &session_bridge.manager,
                        request,
                        async_runtime,
                        browser_result_tx,
                    )
                })
        }
        PendingSftpContextAction::CreateFile { path, refresh_path } => {
            let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
                return false;
            };
            let runtime_handle = async_runtime
                .cloned()
                .unwrap_or_else(|| session_bridge.manager.runtime_handle());
            let manager = session_bridge.manager.clone();
            let result_tx = transfer_result_tx.clone();
            runtime_handle.spawn(async move {
                let error = manager
                    .sftp_upload_file_async(session_id, path.as_str(), Vec::new())
                    .await
                    .err()
                    .map(|err| err.to_string());
                let _ = result_tx.send(SftpTransferBackgroundMessage {
                    session_id: session_id.to_string(),
                    tasks: Vec::new(),
                    refresh_remote_path: error.is_none().then_some(refresh_path),
                    error,
                    open_queue_drawer: false,
                    open_transfer_center: false,
                });
            });
            true
        }
        PendingSftpContextAction::CreateFolder { path, refresh_path } => {
            let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
                return false;
            };
            let runtime_handle = async_runtime
                .cloned()
                .unwrap_or_else(|| session_bridge.manager.runtime_handle());
            let manager = session_bridge.manager.clone();
            let result_tx = transfer_result_tx.clone();
            runtime_handle.spawn(async move {
                let error = manager
                    .sftp_create_directory_async(session_id, path.as_str())
                    .await
                    .err()
                    .map(|err| err.to_string());
                let _ = result_tx.send(SftpTransferBackgroundMessage {
                    session_id: session_id.to_string(),
                    tasks: Vec::new(),
                    refresh_remote_path: error.is_none().then_some(refresh_path),
                    error,
                    open_queue_drawer: false,
                    open_transfer_center: false,
                });
            });
            true
        }
        PendingSftpContextAction::RenameEntry {
            from,
            to,
            refresh_path,
            linked_terminal_session_id,
        } => {
            let Some(session_id) = Uuid::parse_str(linked_terminal_session_id.as_str()).ok() else {
                return false;
            };
            let runtime_handle = async_runtime
                .cloned()
                .unwrap_or_else(|| session_bridge.manager.runtime_handle());
            let manager = session_bridge.manager.clone();
            let result_tx = transfer_result_tx.clone();
            runtime_handle.spawn(async move {
                let error = manager
                    .sftp_rename_entry_async(session_id, from.as_str(), to.as_str())
                    .await
                    .err()
                    .map(|err| err.to_string());
                let _ = result_tx.send(SftpTransferBackgroundMessage {
                    session_id: session_id.to_string(),
                    tasks: Vec::new(),
                    refresh_remote_path: error.is_none().then_some(refresh_path),
                    error,
                    open_queue_drawer: false,
                    open_transfer_center: false,
                });
            });
            true
        }
        PendingSftpContextAction::DeleteEntries {
            entries,
            refresh_path,
        } => {
            let Some(session_id) = active_sftp_terminal_session_uuid(state) else {
                return false;
            };
            let runtime_handle = async_runtime
                .cloned()
                .unwrap_or_else(|| session_bridge.manager.runtime_handle());
            let manager = session_bridge.manager.clone();
            let result_tx = transfer_result_tx.clone();
            runtime_handle.spawn(async move {
                let error = manager
                    .sftp_delete_entries_async(session_id, entries)
                    .await
                    .err()
                    .map(|err| err.to_string());
                let _ = result_tx.send(SftpTransferBackgroundMessage {
                    session_id: session_id.to_string(),
                    tasks: Vec::new(),
                    refresh_remote_path: error.is_none().then_some(refresh_path),
                    error,
                    open_queue_drawer: false,
                    open_transfer_center: false,
                });
            });
            true
        }
        PendingSftpContextAction::UploadFiles => rfd::FileDialog::new()
            .set_title("Upload Files to SFTP")
            .pick_files()
            .is_some_and(|local_paths| {
                schedule_active_sftp_upload_from_paths(
                    state,
                    &session_bridge.manager,
                    transfer_result_tx,
                    local_paths,
                )
            }),
        PendingSftpContextAction::UploadFolder => rfd::FileDialog::new()
            .set_title("Upload Folder to SFTP")
            .pick_folder()
            .is_some_and(|local_path| {
                schedule_active_sftp_upload_from_paths(
                    state,
                    &session_bridge.manager,
                    transfer_result_tx,
                    vec![local_path],
                )
            }),
        PendingSftpContextAction::DownloadSelection { entry_ids } => rfd::FileDialog::new()
            .set_title("Download To")
            .pick_folder()
            .is_some_and(|local_root| {
                schedule_active_sftp_download_selection(
                    state,
                    &session_bridge.manager,
                    transfer_result_tx,
                    &entry_ids,
                    local_root,
                )
            }),
    }
}

pub(super) fn ensure_active_sftp_browser_started(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_some() {
        return false;
    }

    initial_sftp_browser_path(manager, session_id).is_some_and(|path| {
        let request = controller.open(session_id, path.as_str());
        queue_sftp_browser_request(
            state,
            controller,
            manager,
            request,
            async_runtime,
            result_tx,
        )
    })
}

pub(super) fn open_active_sftp_browser_for_current_session(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    if !state.quick_browser_follows_active_terminal() && state.quick_browser_session().is_some() {
        return false;
    }

    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_none() {
        return ensure_active_sftp_browser_started(
            state,
            controller,
            manager,
            async_runtime,
            result_tx,
        );
    }

    let request = if controller.session_state(session_id).is_some() {
        controller.session_activated(session_id)
    } else {
        None
    };
    request.is_some_and(|request| {
        queue_sftp_browser_request(
            state,
            controller,
            manager,
            request,
            async_runtime,
            result_tx,
        )
    })
}

pub(super) fn sync_active_sftp_browser_follow_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };

    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        controller.mark_disconnected(session_id);
        let browser_session_id = session_id.to_string();
        return controller
            .session_state(session_id)
            .is_some_and(|browser_state| {
                project_sftp_browser_state_into_view_model(
                    state,
                    browser_session_id.as_str(),
                    browser_state,
                )
            });
    }

    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.follow_mode != SftpFollowMode::FollowCwd {
        return false;
    }

    let Some(cwd) = manager.current_working_directory(session_id) else {
        return false;
    };
    if browser_state.current_path == cwd {
        return false;
    }

    controller
        .follow_cwd(session_id, cwd.as_str())
        .is_some_and(|request| {
            queue_sftp_browser_request(
                state,
                controller,
                manager,
                request,
                async_runtime,
                result_tx,
            )
        })
}

pub(super) fn sync_active_sftp_browser_pending_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.mode != SftpPanelMode::Connecting {
        return false;
    }
    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    controller
        .pending_request(session_id)
        .is_some_and(|request| {
            queue_sftp_browser_request(
                state,
                controller,
                manager,
                request,
                async_runtime,
                result_tx,
            )
        })
}

pub(super) fn ensure_active_workspace_sftp_browser_started(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
        return false;
    };
    let needs_restart = matches!(
        browser_session.mode,
        SftpPanelMode::Connecting | SftpPanelMode::Disconnected
    );
    if controller
        .browser_session_state(browser_session.file_browser_session_id.as_str())
        .is_some_and(|browser_state| {
            browser_state.mode != SftpPanelMode::Disconnected && !needs_restart
        })
    {
        return false;
    }
    let Some(session_id) = browser_session
        .linked_terminal_session_id
        .as_deref()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
    else {
        return false;
    };
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    let request = if needs_restart {
        controller
            .retry_browser_session(browser_session.file_browser_session_id.as_str(), session_id)
            .unwrap_or_else(|| controller.open_file_browser_session(browser_session))
    } else {
        controller.open_file_browser_session(browser_session)
    };
    queue_sftp_browser_request(
        state,
        controller,
        manager,
        request,
        async_runtime,
        result_tx,
    )
}

pub(super) fn sync_active_workspace_sftp_browser_pending_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    async_runtime: Option<&tokio::runtime::Handle>,
    result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
) -> bool {
    let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
        return false;
    };
    let Some(session_id) = browser_session
        .linked_terminal_session_id
        .as_deref()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
    else {
        return false;
    };
    let Some(browser_state) =
        controller.browser_session_state(browser_session.file_browser_session_id.as_str())
    else {
        return false;
    };
    if browser_state.mode != SftpPanelMode::Connecting {
        return false;
    }
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    controller
        .pending_request_for_browser_session(
            browser_session.file_browser_session_id.as_str(),
            session_id,
        )
        .is_some_and(|request| {
            queue_sftp_browser_request(
                state,
                controller,
                manager,
                request,
                async_runtime,
                result_tx,
            )
        })
}

pub(super) fn bind_sftp_callbacks(
    window: &AppWindow,
    view_model: &Rc<RefCell<ShellViewModel>>,
    store: &Option<Rc<UiPreferencesStore>>,
    effects: &Rc<dyn PlatformWindowEffects>,
    transfer_store: Option<&Arc<crate::app::sftp::RedbTransferStore>>,
    session_bridge: &Option<Rc<ShellSessionBridge>>,
    async_runtime: Option<&tokio::runtime::Handle>,
    sftp_result_tx: &std::sync::mpsc::Sender<SftpBrowserBackgroundMessage>,
    sftp_transfer_result_tx: &std::sync::mpsc::Sender<SftpTransferBackgroundMessage>,
    sftp_local_action_result_tx: &std::sync::mpsc::Sender<SftpLocalActionBackgroundMessage>,
    workspace_follow_tracker: &Rc<RefCell<WorkspaceFollowTracker>>,
    sftp_browser_controller: &Rc<RefCell<SftpBrowserController>>,
) {
    let async_runtime_handle = async_runtime.cloned();
    let sftp_result_tx = sftp_result_tx.clone();
    let sftp_transfer_result_tx = sftp_transfer_result_tx.clone();
    let sftp_local_action_result_tx = sftp_local_action_result_tx.clone();
    let transfer_store = transfer_store.cloned();
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let open_runtime_handle = async_runtime_handle.clone();
    let open_result_tx = sftp_result_tx.clone();
    window.on_open_sftp_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.open_sftp_panel();
        if let Some(session_bridge) = session_bridge_ref.as_ref() {
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            open_active_sftp_browser_for_current_session(
                &mut state,
                &mut controller,
                &session_bridge.manager,
                open_runtime_handle.as_ref(),
                &open_result_tx,
            );
        }
        super::shell_chrome::sync_shell_side_regions(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        super::sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        super::schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    window.on_transfer_center_filter_toggle_requested(move |filter_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.toggle_transfer_center_filter(filter_id.as_str()) {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_resume_result_tx = sftp_transfer_result_tx.clone();
    window.on_transfer_center_resume_requested(move |task_id| {
        let window = handle.unwrap();
        let state = state.borrow();
        let resumed = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            state
                .transfer_task_by_id(task_id.as_str())
                .cloned()
                .is_some_and(|task| {
                    replay_transfer_task(
                        &session_bridge.manager,
                        &task,
                        TransferReplayAction::Resume,
                        &transfer_resume_result_tx,
                    )
                })
        });
        if resumed {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_restart_result_tx = sftp_transfer_result_tx.clone();
    window.on_transfer_center_restart_requested(move |task_id| {
        let window = handle.unwrap();
        let state = state.borrow();
        let restarted = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            state
                .transfer_task_by_id(task_id.as_str())
                .cloned()
                .is_some_and(|task| {
                    replay_transfer_task(
                        &session_bridge.manager,
                        &task,
                        TransferReplayAction::Restart,
                        &transfer_restart_result_tx,
                    )
                })
        });
        if restarted {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(view_model);
    let session_bridge_ref = session_bridge.clone();
    window.on_transfer_center_pause_requested(move |task_id| {
        let state = state.borrow();
        let can_pause = session_bridge_ref.as_ref().is_some()
            && state
                .transfer_task_by_id(task_id.as_str())
                .is_some_and(|task| task.state == crate::app::sftp::TransferTaskState::Running);
        if can_pause {
            let _ = transfer_pause_registry().request_pause(task_id.as_str());
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_retry_result_tx = sftp_transfer_result_tx.clone();
    window.on_transfer_center_retry_requested(move |task_id| {
        let window = handle.unwrap();
        let state = state.borrow();
        let retried = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            state
                .transfer_task_by_id(task_id.as_str())
                .cloned()
                .is_some_and(|task| {
                    retry_transfer_task(&session_bridge.manager, &task, &transfer_retry_result_tx)
                })
        });
        if retried {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    window.on_transfer_center_resolve_conflict_requested(move |task_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.open_transfer_conflict_modal(task_id.as_str()) {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            sync_sftp_conflict_modal_state(&window, &state);
            window.set_sftp_conflict_modal_focus_sequence(
                window.get_sftp_conflict_modal_focus_sequence() + 1,
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let open_workspace_runtime_handle = async_runtime_handle.clone();
    let open_workspace_result_tx = sftp_result_tx.clone();
    window.on_transfer_center_open_workspace_requested(move |task_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let opened = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            open_transfer_task_in_workspace(
                &mut state,
                &mut controller,
                &session_bridge.manager,
                open_workspace_runtime_handle.as_ref(),
                &open_workspace_result_tx,
                task_id.as_str(),
            )
        });
        if opened {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            sync_right_panel_state(&window, &mut state);
            super::sync_workspace_tabs_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            super::sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
            super::schedule_workspace_terminal_runtime_defaults_sync(
                &window,
                session_bridge_ref
                    .as_ref()
                    .map(|bridge| bridge.terminal_defaults.clone()),
            );
        }
    });

    let state = Rc::clone(view_model);
    let local_action_result_tx_ref = sftp_local_action_result_tx.clone();
    window.on_transfer_center_open_file_requested(move |task_id| {
        let local_path = {
            let state = state.borrow();
            state.transfer_task_local_open_file_path(task_id.as_str())
        };
        queue_transfer_center_open_file_action(
            task_id.as_str(),
            local_path,
            &local_action_result_tx_ref,
        );
    });

    let state = Rc::clone(view_model);
    let local_action_result_tx_ref = sftp_local_action_result_tx.clone();
    window.on_transfer_center_open_folder_requested(move |task_id| {
        let local_path = {
            let state = state.borrow();
            state.transfer_task_local_open_folder_path(task_id.as_str())
        };
        queue_transfer_center_open_folder_action(
            task_id.as_str(),
            local_path,
            &local_action_result_tx_ref,
        );
    });

    let state = Rc::clone(view_model);
    let local_action_result_tx_ref = sftp_local_action_result_tx.clone();
    window.on_transfer_center_remove_requested(move |task_id| {
        let local_path = {
            let state = state.borrow();
            state.transfer_task_local_remove_path(task_id.as_str())
        };
        let missing_download = {
            let state = state.borrow();
            state.transfer_task_remove_missing_download(task_id.as_str())
        };
        queue_transfer_center_remove_action(
            task_id.as_str(),
            local_path,
            missing_download,
            &local_action_result_tx_ref,
        );
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let transfer_store_ref = transfer_store.clone();
    window.on_transfer_center_clear_completed_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.clear_completed_transfer_tasks() {
            persist_transfer_tasks_snapshot(
                transfer_store_ref.as_ref(),
                state.sftp_transfer_tasks(),
            );
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            sync_sftp_conflict_modal_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_sftp_conflict_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let resolved = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            let tasks = state
                .current_sftp_conflict_task()
                .into_iter()
                .collect::<Vec<_>>();
            resolve_conflict_transfer_tasks(
                &session_bridge.manager,
                tasks.as_slice(),
                crate::app::sftp::TransferConflictPolicy::Skip,
                &transfer_result_tx_ref,
            )
        });
        let _ = state.close_sftp_conflict_modal();
        if resolved {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
        sync_sftp_conflict_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_conflict_modal_apply_to_batch_toggled(move |apply_to_batch| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.set_sftp_conflict_modal_apply_to_batch(apply_to_batch) {
            sync_sftp_conflict_modal_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_sftp_conflict_modal_replace_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let resolved = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            let tasks = state.active_sftp_conflict_tasks();
            resolve_conflict_transfer_tasks(
                &session_bridge.manager,
                tasks.as_slice(),
                crate::app::sftp::TransferConflictPolicy::Overwrite,
                &transfer_result_tx_ref,
            )
        });
        let _ = state.close_sftp_conflict_modal();
        if resolved {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
        sync_sftp_conflict_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_sftp_conflict_modal_skip_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let resolved = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            let tasks = state
                .current_sftp_conflict_task()
                .into_iter()
                .collect::<Vec<_>>();
            resolve_conflict_transfer_tasks(
                &session_bridge.manager,
                tasks.as_slice(),
                crate::app::sftp::TransferConflictPolicy::Skip,
                &transfer_result_tx_ref,
            )
        });
        let _ = state.close_sftp_conflict_modal();
        if resolved {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
        sync_sftp_conflict_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_sftp_conflict_modal_auto_rename_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let resolved = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            let tasks = state.active_sftp_conflict_tasks();
            resolve_conflict_transfer_tasks(
                &session_bridge.manager,
                tasks.as_slice(),
                crate::app::sftp::TransferConflictPolicy::AutoRename,
                &transfer_result_tx_ref,
            )
        });
        let _ = state.close_sftp_conflict_modal();
        if resolved {
            super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
        sync_sftp_conflict_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let expand_runtime_handle = async_runtime_handle.clone();
    let expand_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_expand_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.expand_quick_browser_to_workspace().is_none() {
            return;
        }
        if let Some(session_bridge) = session_bridge_ref.as_ref()
            && let Some(browser_session) = state.active_workspace_sftp_session().cloned()
        {
            let request = {
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                controller.open_file_browser_session(browser_session)
            };
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            let _ = queue_sftp_browser_request(
                &mut state,
                &mut controller,
                &session_bridge.manager,
                request,
                expand_runtime_handle.as_ref(),
                &expand_result_tx,
            );
        }
        super::sync_workspace_tabs_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        super::sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        super::schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_right_panel_state(&window, &mut state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let binding_toggle_runtime_handle = async_runtime_handle.clone();
    let binding_toggle_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_binding_mode_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.toggle_quick_browser_binding_mode() {
            return;
        }

        if state.quick_browser_follows_active_terminal()
            && let Some(session_bridge) = session_bridge_ref.as_ref()
        {
            let _ = sync_active_sftp_projection_from_manager(&mut state, &session_bridge.manager);
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            let _ = open_active_sftp_browser_for_current_session(
                &mut state,
                &mut controller,
                &session_bridge.manager,
                binding_toggle_runtime_handle.as_ref(),
                &binding_toggle_result_tx,
            );
        }

        sync_right_panel_state(&window, &mut state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_path_edit_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.begin_quick_browser_path_edit() {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_context_menu_requested(
        move |target_id, target_kind, anchor_x, anchor_y| {
            let window = handle.unwrap();
            let mut state = state.borrow_mut();
            state.open_context_menu_for_surface(
                crate::shell::context_menu::ContextMenuSurface::QuickBrowserSftp,
                parse_context_target_kind(target_kind.as_str(), SidebarDestination::Console),
                if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                },
                anchor_x,
                anchor_y,
            );
            sync_right_panel_state(&window, &mut state);
            super::assets_keychain::sync_assets_toolbar_state(&window, &state);
            super::assets_keychain::update_context_menu_placement(&window, &mut state);
            super::assets_keychain::sync_assets_context_menu_state(&window, &state);
        },
    );

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_external_drop_hover_changed(move |active| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.set_quick_browser_drop_target_active(active) {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_sftp_panel_external_drop_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let drop_paths = window.get_sftp_panel_external_drop_paths();
        let local_paths = (0..drop_paths.row_count())
            .filter_map(|index| drop_paths.row_data(index))
            .map(|path| PathBuf::from(path.to_string()))
            .collect::<Vec<_>>();
        let scheduled = session_bridge_ref.as_ref().is_some_and(|session_bridge| {
            schedule_quick_browser_upload_from_paths(
                &mut state,
                &session_bridge.manager,
                &transfer_result_tx_ref,
                local_paths,
            )
        });
        let _ = state.set_quick_browser_drop_target_active(false);
        window.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![])));
        let _ = scheduled;
        sync_right_panel_state(&window, &mut state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    window.on_workspace_terminal_external_drop_hover_changed(move |active| {
        let window = handle.unwrap();
        let state = state.borrow();
        let accepts_drop = active
            && window.get_workspace_session_host_mode() == "terminal"
            && workspace_terminal_accepts_external_drop(
                &state,
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        tracing::debug!(
            target: "app.drop",
            hover_active = active,
            accepts_drop,
            host_mode = window.get_workspace_session_host_mode().as_str(),
            "terminal external drop hover changed"
        );
        window.set_workspace_terminal_drop_target_active(accepts_drop);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_workspace_terminal_external_drop_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        window.set_workspace_terminal_drop_target_active(false);
        let drop_paths = window.get_workspace_terminal_external_drop_paths();
        let local_paths = (0..drop_paths.row_count())
            .filter_map(|index| drop_paths.row_data(index))
            .map(|path| PathBuf::from(path.to_string()))
            .collect::<Vec<_>>();
        let path_count = local_paths.len();
        tracing::info!(
            target: "app.drop",
            target = "terminal",
            path_count,
            "terminal external drop requested"
        );
        let result = session_bridge_ref.as_ref().map(|session_bridge| {
            schedule_terminal_external_drop_from_paths(
                &state,
                &session_bridge.manager,
                &transfer_result_tx_ref,
                local_paths,
            )
        });
        window.set_workspace_terminal_external_drop_paths(ModelRc::new(VecModel::from(vec![])));

        match result {
            Some(Ok(_)) => {
                super::shell_chrome::sync_shell_side_regions(
                    &window,
                    &mut state,
                    effects_ref.as_ref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    None,
                );
            }
            Some(Err(err)) => {
                tracing::warn!(
                    target: "app.drop",
                    target = "terminal",
                    path_count,
                    error = %err,
                    "terminal external drop failed"
                );
                state.show_transfer_center_feedback(
                    "error",
                    format!("Upload to current directory failed: {err}"),
                );
                super::shell_chrome::sync_shell_side_regions(
                    &window,
                    &mut state,
                    effects_ref.as_ref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    None,
                );
            }
            None => {}
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_viewport_changed(move |viewport_y, visible_height| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.update_active_sftp_panel_viewport(viewport_y, visible_height) {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_item_selected(move |entry_id| {
        if entry_id.as_str() == SFTP_PARENT_ITEM_ID {
            return;
        }
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.select_sftp_panel_entry(entry_id.as_str()) {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let item_activated_runtime_handle = async_runtime_handle.clone();
    let item_activated_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_item_activated(move |entry_id, item_kind| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let is_parent_row =
            entry_id.as_str() == SFTP_PARENT_ITEM_ID || item_kind.as_str() == "parent-directory";
        let selection_changed = if is_parent_row {
            false
        } else {
            state.select_sftp_panel_entry(entry_id.as_str())
        };
        let entry = state.active_sftp_entry(entry_id.as_str()).cloned();
        let mut panel_changed = selection_changed;
        let was_modal_open = state.sftp_remote_file_editor_state().open;

        if is_parent_row {
            if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
            {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    panel_changed |= queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        item_activated_runtime_handle.as_ref(),
                        &item_activated_result_tx,
                    );
                }
            } else {
                panel_changed |= state.navigate_sftp_panel_up();
            }
        } else if let Some(entry) = entry {
            if item_kind.as_str() == "directory" || entry.kind == SftpDirectoryEntryKind::Directory
            {
                if let Some(session_bridge) = session_bridge_ref.as_ref()
                    && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
                {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, entry.path.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    panel_changed |= queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        item_activated_runtime_handle.as_ref(),
                        &item_activated_result_tx,
                    );
                }
            } else if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
            {
                panel_changed |= queue_sftp_local_file_action(
                    &session_bridge.manager,
                    session_id,
                    entry.path.as_str(),
                    crate::app::sftp::SftpOpenAction::DownloadAndOpen,
                );
            }
        }

        if panel_changed {
            sync_right_panel_state(&window, &mut state);
        }
        sync_sftp_remote_file_modal_state(&window, &state);
        if !was_modal_open && state.sftp_remote_file_editor_state().open {
            super::assets_keychain::schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_open_queue_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_sftp_queue_drawer();
        sync_right_panel_state(&window, &mut state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let path_submit_runtime_handle = async_runtime_handle.clone();
    let path_submit_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_path_submitted(move |path| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    false
                } else {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, trimmed)
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        path_submit_runtime_handle.as_ref(),
                        &path_submit_result_tx,
                    )
                }
            } else {
                false
            }
        } else {
            state.submit_sftp_panel_path(path.to_string())
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let back_runtime_handle = async_runtime_handle.clone();
    let back_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_back_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_back(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        back_runtime_handle.as_ref(),
                        &back_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_back()
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let forward_runtime_handle = async_runtime_handle.clone();
    let forward_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_forward_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_forward(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        forward_runtime_handle.as_ref(),
                        &forward_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_forward()
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let up_runtime_handle = async_runtime_handle.clone();
    let up_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_up_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        up_runtime_handle.as_ref(),
                        &up_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_up()
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let refresh_runtime_handle = async_runtime_handle.clone();
    let refresh_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_refresh_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.refresh(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        refresh_runtime_handle.as_ref(),
                        &refresh_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.refresh_sftp_panel()
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let retry_runtime_handle = async_runtime_handle.clone();
    let retry_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_retry_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let retried = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                if let Err(err) = session_bridge.manager.retry_session(session_id) {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to retry active SSH session from SFTP panel"
                    );
                    false
                } else {
                    let projection = workspace_terminal::sync_workspace_projection_from_manager(
                        &mut state,
                        &session_bridge.manager,
                    );
                    let browser_changed = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        if let Some(request) = controller.retry(session_id) {
                            queue_sftp_browser_request(
                                &mut state,
                                &mut controller,
                                &session_bridge.manager,
                                request,
                                retry_runtime_handle.as_ref(),
                                &retry_result_tx,
                            )
                        } else {
                            false
                        }
                    };
                    browser_changed
                        || projection.sftp_changed
                        || projection.tabs_changed
                        || projection.surface_changed
                }
            } else {
                false
            }
        } else {
            state.retry_sftp_panel()
        };
        if retried {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let follow_runtime_handle = async_runtime_handle.clone();
    let follow_result_tx = sftp_result_tx.clone();
    window.on_sftp_panel_reenable_follow_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                if let Some(cwd) = session_bridge.manager.current_working_directory(session_id) {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.open(session_id, cwd.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        follow_runtime_handle.as_ref(),
                        &follow_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.reenable_sftp_follow()
        };
        if changed {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_sort_requested(move |column_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.cycle_sftp_panel_sort(column_id.as_str()) {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_column_width_change_requested(move |column_id, width| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.set_sftp_panel_column_width(column_id.as_str(), width) {
            sync_right_panel_state(&window, &mut state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_path_edit_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.begin_workspace_sftp_path_edit() {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_path_cancelled(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.finish_workspace_sftp_path_edit() {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_context_menu_requested(
        move |target_id, target_kind, anchor_x, anchor_y| {
            let window = handle.unwrap();
            let mut state = state.borrow_mut();
            state.open_context_menu_for_surface(
                crate::shell::context_menu::ContextMenuSurface::WorkspaceSftp,
                parse_context_target_kind(target_kind.as_str(), SidebarDestination::Console),
                if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                },
                anchor_x,
                anchor_y,
            );
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            super::assets_keychain::sync_assets_toolbar_state(&window, &state);
            super::assets_keychain::update_context_menu_placement(&window, &mut state);
            super::assets_keychain::sync_assets_context_menu_state(&window, &state);
        },
    );

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_sort_requested(move |column_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.toggle_workspace_sftp_sort_column(column_id.as_str()) {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_viewport_changed(move |viewport_y, visible_height| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.update_workspace_sftp_viewport(viewport_y, visible_height) {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_workspace_sftp_item_selected(move |entry_id, ctrl, shift| {
        if entry_id.as_str() == SFTP_PARENT_ITEM_ID {
            return;
        }
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.select_sftp_panel_entry_with_modifiers(entry_id.as_str(), ctrl, shift) {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_activate_runtime_handle = async_runtime_handle.clone();
    let workspace_activate_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_item_activated(move |entry_id, item_kind| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let is_parent_row =
            entry_id.as_str() == SFTP_PARENT_ITEM_ID || item_kind.as_str() == "parent-directory";
        let selection_changed = if is_parent_row {
            false
        } else {
            state.select_sftp_panel_entry(entry_id.as_str())
        };
        let entry = state.active_sftp_entry(entry_id.as_str()).cloned();
        let mut changed = selection_changed;

        if is_parent_row {
            if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some((browser_session_id, session_id)) =
                    active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    changed |= queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_activate_runtime_handle.as_ref(),
                        &workspace_activate_result_tx,
                    );
                }
            } else {
                changed |= state.navigate_workspace_sftp_up();
            }
        } else if let Some(entry) = entry {
            if item_kind.as_str() == "directory" || entry.kind == SftpDirectoryEntryKind::Directory
            {
                if let Some(session_bridge) = session_bridge_ref.as_ref()
                    && let Some((browser_session_id, session_id)) =
                        active_workspace_sftp_request_identity(&state)
                {
                    let _ = state.schedule_active_workspace_sftp_viewport_reset();
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate_browser_session(
                            browser_session_id.as_str(),
                            session_id,
                            entry.path.as_str(),
                        )
                    };
                    if let Some(request) = request {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        changed |= queue_sftp_browser_request(
                            &mut state,
                            &mut controller,
                            &session_bridge.manager,
                            request,
                            workspace_activate_runtime_handle.as_ref(),
                            &workspace_activate_result_tx,
                        );
                    }
                }
            } else if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some((_, session_id)) = active_workspace_sftp_request_identity(&state)
            {
                changed |= queue_sftp_local_file_action(
                    &session_bridge.manager,
                    session_id,
                    entry.path.as_str(),
                    crate::app::sftp::SftpOpenAction::DownloadAndOpen,
                );
            }
        }

        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_breadcrumb_runtime_handle = async_runtime_handle.clone();
    let workspace_breadcrumb_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_breadcrumb_clicked(move |path| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let target_path = path.trim();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if target_path.is_empty() {
                false
            } else if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                state.finish_workspace_sftp_path_edit();
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_browser_session(
                        browser_session_id.as_str(),
                        session_id,
                        target_path,
                    )
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_breadcrumb_runtime_handle.as_ref(),
                        &workspace_breadcrumb_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.submit_workspace_sftp_path(target_path.to_string())
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_path_runtime_handle = async_runtime_handle.clone();
    let workspace_path_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_path_submitted(move |path| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            let trimmed = path.trim();
            if trimmed.is_empty() {
                false
            } else if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let normalized_path = normalize_remote_dir(trimmed);
                state.finish_workspace_sftp_path_edit();
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_browser_session(
                        browser_session_id.as_str(),
                        session_id,
                        normalized_path.as_str(),
                    )
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_path_runtime_handle.as_ref(),
                        &workspace_path_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.submit_workspace_sftp_path(path.to_string())
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_back_runtime_handle = async_runtime_handle.clone();
    let workspace_back_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_back_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller
                        .navigate_back_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_back_runtime_handle.as_ref(),
                        &workspace_back_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_workspace_sftp_back()
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_forward_runtime_handle = async_runtime_handle.clone();
    let workspace_forward_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_forward_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller
                        .navigate_forward_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_forward_runtime_handle.as_ref(),
                        &workspace_forward_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_workspace_sftp_forward()
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_up_runtime_handle = async_runtime_handle.clone();
    let workspace_up_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_up_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_up_runtime_handle.as_ref(),
                        &workspace_up_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_workspace_sftp_up()
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_refresh_runtime_handle = async_runtime_handle.clone();
    let workspace_refresh_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_refresh_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.refresh_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_refresh_runtime_handle.as_ref(),
                        &workspace_refresh_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.refresh_workspace_sftp()
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_retry_runtime_handle = async_runtime_handle.clone();
    let workspace_retry_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_retry_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some((browser_session_id, session_id)) =
                active_workspace_sftp_request_identity(&state)
            {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.retry_browser_session(browser_session_id.as_str(), session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_retry_runtime_handle.as_ref(),
                        &workspace_retry_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.retry_workspace_sftp()
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let workspace_home_runtime_handle = async_runtime_handle.clone();
    let workspace_home_result_tx = sftp_result_tx.clone();
    window.on_workspace_sftp_home_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let home_path = state.workspace_sftp_home_path();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(home_path) = home_path
                && let Some((browser_session_id, session_id)) =
                    active_workspace_sftp_request_identity(&state)
            {
                let _ = state.schedule_active_workspace_sftp_viewport_reset();
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_browser_session(
                        browser_session_id.as_str(),
                        session_id,
                        home_path.as_str(),
                    )
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    queue_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                        workspace_home_runtime_handle.as_ref(),
                        &workspace_home_result_tx,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else if home_path.is_some() {
            state.submit_workspace_sftp_path(home_path.unwrap_or_default())
        } else {
            false
        };
        if changed {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_workspace_sftp_upload_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let uploaded = rfd::FileDialog::new()
            .set_title("Upload Files to SFTP")
            .pick_files()
            .is_some_and(|local_paths| {
                session_bridge_ref.as_ref().is_some_and(|session_bridge| {
                    schedule_active_sftp_upload_from_paths(
                        &mut state,
                        &session_bridge.manager,
                        &transfer_result_tx_ref,
                        local_paths,
                    )
                })
            });
        if uploaded {
            super::sync_workspace_session_state_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_workspace_sftp_new_folder_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.open_sftp_new_folder_modal();
        super::assets_keychain::sync_asset_modal_state(&window, &state);
        super::assets_keychain::schedule_asset_modal_focus(&window);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_sftp_remote_file_editor();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_content_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sftp_remote_file_editor_content(value.to_string());
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    window.on_sftp_remote_file_modal_save_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if let Some((session_id, remote_path, content)) =
            state.sftp_remote_file_editor_save_payload()
            && let Some(session_bridge) = session_bridge_ref.as_ref()
        {
            match Uuid::parse_str(session_id.as_str())
                .map_err(anyhow::Error::from)
                .and_then(|session_id| {
                    session_bridge.manager.sftp_upload_file(
                        session_id,
                        remote_path.as_str(),
                        content.into_bytes(),
                    )
                }) {
                Ok(_) => state.mark_sftp_remote_file_editor_saved(),
                Err(err) => state.set_sftp_remote_file_editor_error(format!(
                    "Failed to save remote file: {err}"
                )),
            }
        }
        sync_sftp_remote_file_modal_state(&window, &state);
    });
}
