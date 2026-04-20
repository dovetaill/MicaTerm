//! Session-bound SFTP binding snapshots projected from SSH runtime state.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::app::sftp::{
    DownloadTransferEntry, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpPanelMode,
    SftpRuntimeHandle, SftpSessionBindingState, TransferConflictPolicy, TransferQueue,
    TransferTask, TransferTaskAction,
};

use super::transfer_engine::{execute_download_task, execute_upload_task};

#[derive(Clone)]
pub struct SftpSessionBinding {
    session_id: Uuid,
    binding_id: Uuid,
    mode: SftpPanelMode,
    runtime: Option<SftpRuntimeHandle>,
}

impl SftpSessionBinding {
    pub fn connecting(session_id: Uuid, runtime: SftpRuntimeHandle) -> Self {
        Self {
            session_id,
            binding_id: runtime.binding_id(),
            mode: SftpPanelMode::Connecting,
            runtime: Some(runtime),
        }
    }

    pub fn disconnected(session_id: Uuid, binding_id: Uuid) -> Self {
        Self {
            session_id,
            binding_id,
            mode: SftpPanelMode::Disconnected,
            runtime: None,
        }
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn binding_id(&self) -> Uuid {
        self.binding_id
    }

    pub fn mode(&self) -> SftpPanelMode {
        self.mode
    }

    pub fn runtime(&self) -> Option<SftpRuntimeHandle> {
        self.runtime.clone()
    }

    pub fn mark_disconnected(&mut self) {
        self.mode = SftpPanelMode::Disconnected;
        self.runtime = None;
    }
}

pub async fn execute_queued_transfers(
    runtime: &SftpRuntimeHandle,
    queue: &mut TransferQueue,
) -> Result<()> {
    execute_queued_transfers_with_progress(runtime, queue, |_| true).await
}

pub async fn execute_queued_transfers_with_progress<F>(
    runtime: &SftpRuntimeHandle,
    queue: &mut TransferQueue,
    mut on_queue_updated: F,
) -> Result<()>
where
    F: FnMut(&TransferQueue) -> bool,
{
    let task_ids = queue.queued_task_ids();
    for task_id in task_ids {
        if let Err(err) = execute_transfer(runtime, queue, &task_id, &mut on_queue_updated).await {
            queue.mark_failed(&task_id, err.to_string());
            on_queue_updated(queue);
            return Err(err);
        }
    }

    Ok(())
}

pub async fn collect_download_targets(
    runtime: &SftpRuntimeHandle,
    local_root: &Path,
    entries: &[SftpDirectoryEntry],
) -> Result<Vec<DownloadTransferEntry>> {
    let mut targets = Vec::new();
    let mut pending_directories = Vec::new();
    for entry in entries {
        match entry.kind {
            SftpDirectoryEntryKind::Directory => {
                let directory_root = local_root.join(entry.name.as_str());
                targets.push(DownloadTransferEntry {
                    remote_path: entry.path.clone(),
                    local_path: directory_root.clone(),
                    entry_kind: SftpDirectoryEntryKind::Directory,
                    bytes_total: 0,
                });
                pending_directories.push((entry.path.clone(), directory_root));
            }
            _ => targets.push(DownloadTransferEntry {
                remote_path: entry.path.clone(),
                local_path: local_root.join(entry.name.as_str()),
                entry_kind: entry.kind,
                bytes_total: entry.size_bytes.unwrap_or(0),
            }),
        }
    }

    while let Some((remote_dir, local_dir)) = pending_directories.pop() {
        let entries = runtime
            .read_dir(remote_dir.as_str())
            .await
            .with_context(|| format!("failed to read remote directory `{remote_dir}`"))?;
        for entry in entries {
            let local_path = local_dir.join(entry.name.as_str());
            if entry.kind == SftpDirectoryEntryKind::Directory {
                targets.push(DownloadTransferEntry {
                    remote_path: entry.path.clone(),
                    local_path: local_path.clone(),
                    entry_kind: SftpDirectoryEntryKind::Directory,
                    bytes_total: 0,
                });
                pending_directories.push((entry.path.clone(), local_path));
            } else {
                targets.push(DownloadTransferEntry {
                    remote_path: entry.path.clone(),
                    local_path,
                    entry_kind: entry.kind,
                    bytes_total: entry.size_bytes.unwrap_or(0),
                });
            }
        }
    }

    targets.sort_by(|left, right| left.local_path.cmp(&right.local_path));
    Ok(targets)
}

pub async fn move_entry_between_directories(
    runtime: &SftpRuntimeHandle,
    state: &mut SftpSessionBindingState,
    entry_id: &str,
    destination_dir: &str,
) -> Result<bool> {
    let Some(entry) = state
        .entries
        .iter()
        .find(|entry| entry.id == entry_id)
        .cloned()
    else {
        return Ok(false);
    };

    let target_path = remote_child_path(destination_dir, &entry.name);
    if target_path == entry.path {
        return Ok(false);
    }

    runtime
        .move_entry(&entry.path, &target_path)
        .await
        .with_context(|| {
            format!(
                "failed to move remote entry `{}` into `{destination_dir}`",
                entry.path
            )
        })?;

    let viewing_destination = same_remote_dir(state.current_path.as_str(), destination_dir);
    if viewing_destination {
        if let Some(item) = state.entries.iter_mut().find(|item| item.id == entry_id) {
            item.path = target_path;
            item.name = item
                .path
                .rsplit('/')
                .next()
                .unwrap_or(item.name.as_str())
                .to_string();
            state.selected_entry_ids = vec![item.id.clone()];
        }
    } else {
        state.entries.retain(|item| item.id != entry_id);
        state
            .selected_entry_ids
            .retain(|selected_id| selected_id != entry_id);
    }

    Ok(true)
}

pub async fn delete_entries(
    runtime: &SftpRuntimeHandle,
    queue: &mut TransferQueue,
    session_id: &str,
    state: &mut SftpSessionBindingState,
    entry_ids: &[String],
) -> Result<usize> {
    let selected_entries = state
        .entries
        .iter()
        .filter(|entry| entry_ids.iter().any(|id| id == &entry.id))
        .cloned()
        .collect::<Vec<_>>();
    if selected_entries.is_empty() {
        return Ok(0);
    }

    let remote_paths = selected_entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<Vec<_>>();
    queue.cancel_conflicting_paths(session_id, &remote_paths);
    queue.enqueue_delete(session_id, &selected_entries);
    execute_queued_transfers(runtime, queue).await?;

    state
        .entries
        .retain(|entry| !entry_ids.iter().any(|entry_id| entry_id == &entry.id));
    state
        .selected_entry_ids
        .retain(|selected_id| !entry_ids.iter().any(|entry_id| entry_id == selected_id));

    Ok(selected_entries.len())
}

async fn execute_transfer(
    runtime: &SftpRuntimeHandle,
    queue: &mut TransferQueue,
    task_id: &str,
    on_queue_updated: &mut impl FnMut(&TransferQueue) -> bool,
) -> Result<()> {
    let Some(task) = queue.task(task_id).cloned() else {
        return Ok(());
    };
    if has_blocking_download_directory_conflict(queue, &task) {
        return Ok(());
    }

    match task.action {
        TransferTaskAction::Upload { local_path } => {
            let remote_exists = runtime.path_exists(&task.target_path).await?;
            if remote_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Remote path already exists");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing remote path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::AutoRename)
                    | Some(TransferConflictPolicy::CancelCurrent) => {
                        queue.mark_conflict(task_id, "Unsupported conflict policy for remote path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
            on_queue_updated(queue);
            ensure_remote_parent_dirs(runtime, &task.target_path).await?;
            let Some(mut queued_task) = queue.task(task_id).cloned() else {
                return Ok(());
            };
            if !matches!(queued_task.action, TransferTaskAction::Upload { .. }) {
                return Ok(());
            }
            if queued_task.source_path != local_path.to_string_lossy() {
                queued_task.source_path = local_path.to_string_lossy().to_string();
            }

            let mut sync_progress = |updated_task: &TransferTask| {
                let _ = queue.replace_task(updated_task.clone());
                on_queue_updated(queue)
            };

            let result = execute_upload_task(runtime, &mut queued_task, &mut sync_progress).await;
            let _ = queue.replace_task(queued_task);

            match result {
                Ok(()) => {
                    let _ = on_queue_updated(queue);
                }
                Err(err) => return Err(err),
            }
        }
        TransferTaskAction::UploadDirectory {
            local_path: _local_path,
        } => {
            let remote_exists = runtime.path_exists(&task.target_path).await?;
            if remote_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Remote path already exists");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing remote path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::AutoRename)
                    | Some(TransferConflictPolicy::CancelCurrent) => {
                        queue.mark_conflict(task_id, "Unsupported conflict policy for remote path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {
                        if runtime.read_dir(&task.target_path).await.is_err() {
                            runtime
                                .delete_file(&task.target_path)
                                .await
                                .with_context(|| {
                                    format!(
                                        "failed to replace conflicting remote file `{}`",
                                        task.target_path
                                    )
                                })?;
                        }
                    }
                }
            }

            queue.mark_running(task_id);
            on_queue_updated(queue);
            ensure_remote_parent_dirs(runtime, &task.target_path).await?;
            if !runtime.path_exists(&task.target_path).await? {
                runtime.mkdir(&task.target_path).await.with_context(|| {
                    format!("failed to create remote directory `{}`", task.target_path)
                })?;
            }
            queue.mark_completed(task_id, 0);
            on_queue_updated(queue);
        }
        TransferTaskAction::Download { local_path } => {
            let mut local_path = local_path;
            let local_exists = local_path.exists();
            if local_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Local path already exists");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing local path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::CancelCurrent) => {
                        queue.cancel_task(task_id, "Cancelled conflicting local download");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::AutoRename) => {
                        if !queue.apply_download_auto_rename(task_id) {
                            queue.mark_failed(task_id, "Failed to auto rename local download path");
                            on_queue_updated(queue);
                            return Ok(());
                        }
                        on_queue_updated(queue);
                        let Some(updated_task) = queue.task(task_id).cloned() else {
                            return Ok(());
                        };
                        local_path = match updated_task.action {
                            TransferTaskAction::Download { local_path } => local_path,
                            _ => local_path,
                        };
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
            on_queue_updated(queue);
            let Some(mut queued_task) = queue.task(task_id).cloned() else {
                return Ok(());
            };
            if !matches!(queued_task.action, TransferTaskAction::Download { .. }) {
                return Ok(());
            }
            if queued_task.target_path != local_path.to_string_lossy() {
                queued_task.target_path = local_path.to_string_lossy().to_string();
            }

            let mut sync_progress = |updated_task: &TransferTask| {
                let _ = queue.replace_task(updated_task.clone());
                on_queue_updated(queue)
            };

            let result = execute_download_task(runtime, &mut queued_task, &mut sync_progress).await;
            let _ = queue.replace_task(queued_task);

            match result {
                Ok(()) => {
                    let _ = on_queue_updated(queue);
                }
                Err(err) => return Err(err),
            }
        }
        TransferTaskAction::DownloadDirectory { local_path } => {
            let mut local_path = local_path;
            if local_path.exists() {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Local path already exists");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing local path");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::CancelCurrent) => {
                        queue.cancel_task(task_id, "Cancelled conflicting local download");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::AutoRename) => {
                        if !queue.apply_download_auto_rename(task_id) {
                            queue.mark_failed(task_id, "Failed to auto rename local download path");
                            on_queue_updated(queue);
                            return Ok(());
                        }
                        on_queue_updated(queue);
                        let Some(updated_task) = queue.task(task_id).cloned() else {
                            return Ok(());
                        };
                        local_path = match updated_task.action {
                            TransferTaskAction::DownloadDirectory { local_path } => local_path,
                            _ => local_path,
                        };
                    }
                    Some(TransferConflictPolicy::Overwrite) => {
                        if !local_path.is_dir() {
                            fs::remove_file(&local_path).with_context(|| {
                                format!(
                                    "failed to remove conflicting local file `{}`",
                                    local_path.display()
                                )
                            })?;
                        }
                    }
                }
            }

            queue.mark_running(task_id);
            on_queue_updated(queue);
            fs::create_dir_all(&local_path).with_context(|| {
                format!(
                    "failed to create local download directory `{}`",
                    local_path.display()
                )
            })?;
            queue.mark_completed(task_id, 0);
            on_queue_updated(queue);
        }
        TransferTaskAction::Delete { entry_kind } => {
            queue.mark_running(task_id);
            on_queue_updated(queue);
            delete_remote_entry(runtime, &task.source_path, entry_kind).await?;
            queue.mark_completed(task_id, 0);
            on_queue_updated(queue);
        }
        TransferTaskAction::Move => {
            let remote_exists = runtime.path_exists(&task.target_path).await?;
            if remote_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Remote target already exists");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing remote target");
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::AutoRename)
                    | Some(TransferConflictPolicy::CancelCurrent) => {
                        queue.mark_conflict(
                            task_id,
                            "Unsupported conflict policy for remote target",
                        );
                        on_queue_updated(queue);
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
            on_queue_updated(queue);
            ensure_remote_parent_dirs(runtime, &task.target_path).await?;
            runtime
                .move_entry(&task.source_path, &task.target_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to move remote entry `{}` -> `{}`",
                        task.source_path, task.target_path
                    )
                })?;
            queue.mark_completed(task_id, 0);
            on_queue_updated(queue);
        }
    }

    Ok(())
}

fn has_blocking_download_directory_conflict(
    queue: &TransferQueue,
    task: &crate::app::sftp::TransferTask,
) -> bool {
    let task_path = match &task.action {
        TransferTaskAction::Download { local_path }
        | TransferTaskAction::DownloadDirectory { local_path } => local_path.as_path(),
        _ => return false,
    };

    queue.tasks.iter().any(|candidate| {
        candidate.id != task.id
            && candidate.session_id == task.session_id
            && candidate.state == crate::app::sftp::TransferTaskState::Conflict
            && matches!(
                &candidate.action,
                TransferTaskAction::DownloadDirectory { local_path }
                    if task_path.starts_with(local_path.as_path())
            )
    })
}

async fn delete_remote_entry(
    runtime: &SftpRuntimeHandle,
    path: &str,
    entry_kind: SftpDirectoryEntryKind,
) -> Result<()> {
    match entry_kind {
        SftpDirectoryEntryKind::Directory => runtime.delete_dir(path).await,
        _ => runtime.delete_file(path).await,
    }
    .with_context(|| format!("failed to delete remote entry `{path}`"))
}

async fn ensure_remote_parent_dirs(runtime: &SftpRuntimeHandle, remote_path: &str) -> Result<()> {
    let mut pending = Vec::new();
    let mut cursor = remote_parent_path(remote_path);
    while let Some(path) = cursor {
        if path == "/" {
            break;
        }
        pending.push(path.clone());
        cursor = remote_parent_path(&path);
    }

    pending.reverse();
    for path in pending {
        if !runtime.path_exists(&path).await? {
            runtime
                .mkdir(&path)
                .await
                .with_context(|| format!("failed to create remote directory `{path}`"))?;
        }
    }

    Ok(())
}

fn remote_parent_path(path: &str) -> Option<String> {
    let trimmed = path.trim().trim_end_matches('/');
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }

    match trimmed.rsplit_once('/') {
        Some(("", _)) => Some("/".into()),
        Some((parent, _)) if !parent.is_empty() => Some(parent.into()),
        _ => None,
    }
}

fn remote_child_path(parent: &str, name: &str) -> String {
    if parent == "/" {
        format!("/{}", name.trim_start_matches('/'))
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

fn same_remote_dir(left: &str, right: &str) -> bool {
    normalize_remote_dir(left) == normalize_remote_dir(right)
}

fn normalize_remote_dir(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        "/".into()
    } else {
        format!("/{}", trimmed.trim_matches('/'))
    }
}
