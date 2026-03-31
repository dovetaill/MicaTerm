//! Session-bound SFTP binding snapshots projected from SSH runtime state.

use std::fs;

use anyhow::{Context, Result};
use uuid::Uuid;

use crate::app::sftp::{
    SftpDirectoryEntryKind, SftpPanelMode, SftpRuntimeHandle, SftpSessionBindingState,
    TransferConflictPolicy, TransferQueue, TransferTaskAction,
};

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
    let task_ids = queue.queued_task_ids();
    for task_id in task_ids {
        execute_transfer(runtime, queue, &task_id).await?;
    }

    Ok(())
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
) -> Result<()> {
    let Some(task) = queue.task(task_id).cloned() else {
        return Ok(());
    };

    match task.action {
        TransferTaskAction::Upload { local_path } => {
            let remote_exists = runtime.path_exists(&task.target_path).await?;
            if remote_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Remote path already exists");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing remote path");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
            ensure_remote_parent_dirs(runtime, &task.target_path).await?;
            let bytes = fs::read(&local_path)
                .with_context(|| format!("failed to read local upload source `{}`", local_path.display()))?;
            let transferred = runtime
                .upload_file(&task.target_path, bytes)
                .await
                .with_context(|| format!("failed to upload `{}`", local_path.display()))?;
            queue.mark_completed(task_id, transferred);
        }
        TransferTaskAction::Download { local_path } => {
            let local_exists = local_path.exists();
            if local_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Local path already exists");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing local path");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
            let bytes = runtime
                .download_file(&task.source_path)
                .await
                .with_context(|| format!("failed to download `{}`", task.source_path))?;
            if let Some(parent) = local_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("failed to create local download directory `{}`", parent.display())
                })?;
            }
            fs::write(&local_path, &bytes)
                .with_context(|| format!("failed to write local download `{}`", local_path.display()))?;
            queue.mark_completed(task_id, bytes.len() as u64);
        }
        TransferTaskAction::Delete { entry_kind } => {
            queue.mark_running(task_id);
            delete_remote_entry(runtime, &task.source_path, entry_kind).await?;
            queue.mark_completed(task_id, 0);
        }
        TransferTaskAction::Move => {
            let remote_exists = runtime.path_exists(&task.target_path).await?;
            if remote_exists {
                match task.conflict_policy {
                    None => {
                        queue.mark_conflict(task_id, "Remote target already exists");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Skip) => {
                        queue.cancel_task(task_id, "Skipped existing remote target");
                        return Ok(());
                    }
                    Some(TransferConflictPolicy::Overwrite) => {}
                }
            }

            queue.mark_running(task_id);
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
        }
    }

    Ok(())
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
