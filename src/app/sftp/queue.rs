//! Transfer queue primitives for SFTP operations.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::sftp::local_ops::{
    LocalTransferEntry, build_local_download_path, build_remote_upload_path, next_auto_rename_path,
};
use crate::app::sftp::model::{SftpDirectoryEntry, SftpDirectoryEntryKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferDirection {
    Upload,
    Download,
    Delete,
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferConflictPolicy {
    Overwrite,
    Skip,
    AutoRename,
    CancelCurrent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferTaskAction {
    Upload { local_path: PathBuf },
    UploadDirectory { local_path: PathBuf },
    Download { local_path: PathBuf },
    DownloadDirectory { local_path: PathBuf },
    Delete { entry_kind: SftpDirectoryEntryKind },
    Move,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferTaskState {
    Queued,
    Running,
    Paused,
    VerifyingResume,
    Interrupted,
    Completed,
    Failed,
    Cancelled,
    Conflict,
}

impl TransferTaskState {
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Running | Self::Paused | Self::VerifyingResume
        )
    }

    pub fn needs_attention(self) -> bool {
        matches!(self, Self::Interrupted | Self::Failed | Self::Conflict)
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::VerifyingResume => "verifying-resume",
            Self::Interrupted => "interrupted",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Conflict => "conflict",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferResumeMode {
    ResumeIfPossible,
    RestartOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadTransferEntry {
    pub remote_path: String,
    pub local_path: PathBuf,
    pub entry_kind: SftpDirectoryEntryKind,
    pub bytes_total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTask {
    pub id: String,
    pub session_id: String,
    pub source_path: String,
    pub target_path: String,
    pub direction: TransferDirection,
    pub action: TransferTaskAction,
    pub state: TransferTaskState,
    pub bytes_total: u64,
    pub bytes_transferred: u64,
    pub bytes_confirmed: u64,
    pub temp_target_path: Option<PathBuf>,
    pub resume_mode: TransferResumeMode,
    pub conflict_policy: Option<TransferConflictPolicy>,
    pub error_message: Option<String>,
}

pub fn download_part_path(path: &Path) -> PathBuf {
    let mut part_path = path.as_os_str().to_os_string();
    part_path.push(".part");
    PathBuf::from(part_path)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferQueueSummary {
    pub total_count: usize,
    pub active_count: usize,
    pub queued_count: usize,
    pub running_count: usize,
    pub paused_count: usize,
    pub completed_count: usize,
    pub failed_count: usize,
    pub current_session_count: usize,
}

impl TransferQueueSummary {
    pub fn from_tasks(tasks: &[TransferTask], current_session_id: Option<&str>) -> Self {
        let total_count = tasks.len();
        let active_count = tasks.iter().filter(|task| task.state.is_active()).count();
        let queued_count = tasks
            .iter()
            .filter(|task| task.state == TransferTaskState::Queued)
            .count();
        let running_count = tasks
            .iter()
            .filter(|task| task.state == TransferTaskState::Running)
            .count();
        let paused_count = tasks
            .iter()
            .filter(|task| task.state == TransferTaskState::Paused)
            .count();
        let completed_count = tasks
            .iter()
            .filter(|task| task.state == TransferTaskState::Completed)
            .count();
        let failed_count = tasks
            .iter()
            .filter(|task| task.state.needs_attention())
            .count();
        let current_session_count = current_session_id
            .map(|session_id| {
                tasks
                    .iter()
                    .filter(|task| task.session_id == session_id)
                    .count()
            })
            .unwrap_or(0);

        Self {
            total_count,
            active_count,
            queued_count,
            running_count,
            paused_count,
            completed_count,
            failed_count,
            current_session_count,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferQueue {
    pub tasks: Vec<TransferTask>,
}

impl TransferQueue {
    pub fn summary(&self, current_session_id: Option<&str>) -> TransferQueueSummary {
        TransferQueueSummary::from_tasks(&self.tasks, current_session_id)
    }

    pub fn task(&self, task_id: &str) -> Option<&TransferTask> {
        self.tasks.iter().find(|task| task.id == task_id)
    }

    pub fn task_mut(&mut self, task_id: &str) -> Option<&mut TransferTask> {
        self.tasks.iter_mut().find(|task| task.id == task_id)
    }

    pub fn replace_task(&mut self, updated_task: TransferTask) -> bool {
        let Some(task) = self.task_mut(&updated_task.id) else {
            return false;
        };
        *task = updated_task;
        true
    }

    pub fn mark_running(&mut self, task_id: &str) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        task.state = TransferTaskState::Running;
        task.error_message = None;
        true
    }

    pub fn mark_completed(&mut self, task_id: &str, bytes_transferred: u64) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        task.state = TransferTaskState::Completed;
        task.bytes_transferred = bytes_transferred;
        task.bytes_confirmed = bytes_transferred;
        if task.bytes_total == 0 {
            task.bytes_total = bytes_transferred;
        }
        task.error_message = None;
        true
    }

    pub fn mark_failed(&mut self, task_id: &str, message: impl Into<String>) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        task.state = TransferTaskState::Failed;
        task.error_message = Some(message.into());
        true
    }

    pub fn cancel_task(&mut self, task_id: &str, message: impl Into<String>) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        task.state = TransferTaskState::Cancelled;
        task.error_message = Some(message.into());
        true
    }

    pub fn mark_conflict(&mut self, task_id: &str, message: impl Into<String>) -> bool {
        let Some(task) = self.tasks.iter_mut().find(|task| task.id == task_id) else {
            return false;
        };
        task.state = TransferTaskState::Conflict;
        task.error_message = Some(message.into());
        true
    }

    pub fn resume_conflict(&mut self, task_id: &str, policy: TransferConflictPolicy) -> bool {
        let Some(task_index) = self.tasks.iter().position(|task| task.id == task_id) else {
            return false;
        };
        if self.tasks[task_index].state != TransferTaskState::Conflict {
            return false;
        }

        let is_download = transfer_task_local_path(&self.tasks[task_index]).is_some();
        if !is_download
            && matches!(
                policy,
                TransferConflictPolicy::AutoRename | TransferConflictPolicy::CancelCurrent
            )
        {
            return false;
        }
        if is_download && policy == TransferConflictPolicy::AutoRename {
            let _ = self.apply_download_auto_rename(task_id);
        }

        let task = &mut self.tasks[task_index];
        if policy == TransferConflictPolicy::CancelCurrent {
            task.state = TransferTaskState::Cancelled;
            task.conflict_policy = Some(policy);
            task.error_message = Some("Cancelled conflicting local download".into());
            task.bytes_transferred = 0;
            return true;
        }

        task.state = TransferTaskState::Queued;
        task.conflict_policy = Some(policy);
        task.bytes_transferred = 0;
        task.error_message = None;
        true
    }

    pub fn apply_download_auto_rename(&mut self, task_id: &str) -> bool {
        let Some(task_index) = self.tasks.iter().position(|task| task.id == task_id) else {
            return false;
        };
        let Some(old_root) =
            transfer_task_local_path(&self.tasks[task_index]).map(Path::to_path_buf)
        else {
            return false;
        };
        let session_id = self.tasks[task_index].session_id.clone();
        let new_root = next_auto_rename_path(&old_root);
        rewrite_download_subtree_root(
            &mut self.tasks,
            session_id.as_str(),
            old_root.as_path(),
            new_root.as_path(),
        );
        true
    }

    pub fn cancel_conflicting_paths(&mut self, session_id: &str, remote_paths: &[String]) -> usize {
        let mut cancelled = 0;
        for task in &mut self.tasks {
            if task.session_id != session_id || !task.state.is_active() {
                continue;
            }
            let matches_remote_path = remote_paths
                .iter()
                .any(|path| path == &task.source_path || path == &task.target_path);
            if !matches_remote_path {
                continue;
            }

            task.state = TransferTaskState::Cancelled;
            task.error_message = Some("Cancelled because the remote path is being deleted".into());
            cancelled += 1;
        }

        cancelled
    }

    pub fn queued_task_ids(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|task| task.state == TransferTaskState::Queued)
            .map(|task| task.id.clone())
            .collect()
    }

    pub fn enqueue_upload(
        &mut self,
        session_id: &str,
        target_dir: &str,
        sources: &[LocalTransferEntry],
    ) -> Vec<String> {
        sources
            .iter()
            .map(|source| {
                let action = if source.local_path.is_dir() {
                    TransferTaskAction::UploadDirectory {
                        local_path: source.local_path.clone(),
                    }
                } else {
                    TransferTaskAction::Upload {
                        local_path: source.local_path.clone(),
                    }
                };
                let task = TransferTask {
                    id: Uuid::new_v4().to_string(),
                    session_id: session_id.into(),
                    source_path: source.local_path.to_string_lossy().to_string(),
                    target_path: build_remote_upload_path(
                        target_dir,
                        source.relative_path.as_path(),
                    ),
                    direction: TransferDirection::Upload,
                    action,
                    state: TransferTaskState::Queued,
                    bytes_total: source.bytes_total,
                    bytes_transferred: 0,
                    bytes_confirmed: 0,
                    temp_target_path: None,
                    resume_mode: TransferResumeMode::ResumeIfPossible,
                    conflict_policy: None,
                    error_message: None,
                };
                let id = task.id.clone();
                self.tasks.push(task);
                id
            })
            .collect()
    }

    pub fn enqueue_download(
        &mut self,
        session_id: &str,
        local_root: &Path,
        entries: &[SftpDirectoryEntry],
    ) -> Vec<String> {
        let targets = entries
            .iter()
            .map(|entry| DownloadTransferEntry {
                remote_path: entry.path.clone(),
                local_path: build_local_download_path(local_root, &entry.path),
                entry_kind: entry.kind,
                bytes_total: entry.size_bytes.unwrap_or(0),
            })
            .collect::<Vec<_>>();
        self.enqueue_download_targets(session_id, &targets)
    }

    pub fn enqueue_download_targets(
        &mut self,
        session_id: &str,
        entries: &[DownloadTransferEntry],
    ) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                let action = if entry.entry_kind == SftpDirectoryEntryKind::Directory {
                    TransferTaskAction::DownloadDirectory {
                        local_path: entry.local_path.clone(),
                    }
                } else {
                    TransferTaskAction::Download {
                        local_path: entry.local_path.clone(),
                    }
                };
                let task = TransferTask {
                    id: Uuid::new_v4().to_string(),
                    session_id: session_id.into(),
                    source_path: entry.remote_path.clone(),
                    target_path: entry.local_path.to_string_lossy().to_string(),
                    direction: TransferDirection::Download,
                    action,
                    state: TransferTaskState::Queued,
                    bytes_total: entry.bytes_total,
                    bytes_transferred: 0,
                    bytes_confirmed: 0,
                    temp_target_path: None,
                    resume_mode: TransferResumeMode::ResumeIfPossible,
                    conflict_policy: None,
                    error_message: None,
                };
                let id = task.id.clone();
                self.tasks.push(task);
                id
            })
            .collect()
    }

    pub fn enqueue_delete(
        &mut self,
        session_id: &str,
        entries: &[SftpDirectoryEntry],
    ) -> Vec<String> {
        entries
            .iter()
            .map(|entry| {
                let task = TransferTask {
                    id: Uuid::new_v4().to_string(),
                    session_id: session_id.into(),
                    source_path: entry.path.clone(),
                    target_path: String::new(),
                    direction: TransferDirection::Delete,
                    action: TransferTaskAction::Delete {
                        entry_kind: entry.kind,
                    },
                    state: TransferTaskState::Queued,
                    bytes_total: 0,
                    bytes_transferred: 0,
                    bytes_confirmed: 0,
                    temp_target_path: None,
                    resume_mode: TransferResumeMode::RestartOnly,
                    conflict_policy: None,
                    error_message: None,
                };
                let id = task.id.clone();
                self.tasks.push(task);
                id
            })
            .collect()
    }

    pub fn enqueue_move(
        &mut self,
        session_id: &str,
        entry: &SftpDirectoryEntry,
        target_dir: &str,
    ) -> String {
        let target_path =
            build_remote_upload_path(target_dir, PathBuf::from(&entry.name).as_path());
        let task = TransferTask {
            id: Uuid::new_v4().to_string(),
            session_id: session_id.into(),
            source_path: entry.path.clone(),
            target_path,
            direction: TransferDirection::Move,
            action: TransferTaskAction::Move,
            state: TransferTaskState::Queued,
            bytes_total: 0,
            bytes_transferred: 0,
            bytes_confirmed: 0,
            temp_target_path: None,
            resume_mode: TransferResumeMode::RestartOnly,
            conflict_policy: None,
            error_message: None,
        };
        let id = task.id.clone();
        self.tasks.push(task);
        id
    }
}

fn transfer_task_local_path(task: &TransferTask) -> Option<&Path> {
    match &task.action {
        TransferTaskAction::Download { local_path }
        | TransferTaskAction::DownloadDirectory { local_path } => Some(local_path.as_path()),
        _ => None,
    }
}

fn set_transfer_task_local_path(task: &mut TransferTask, local_path: PathBuf) {
    match &mut task.action {
        TransferTaskAction::Download {
            local_path: task_local_path,
        }
        | TransferTaskAction::DownloadDirectory {
            local_path: task_local_path,
        } => {
            *task_local_path = local_path.clone();
            task.target_path = local_path.to_string_lossy().to_string();
        }
        _ => {}
    }
}

fn rewrite_download_subtree_root(
    tasks: &mut [TransferTask],
    session_id: &str,
    old_root: &Path,
    new_root: &Path,
) {
    for task in tasks {
        if task.session_id != session_id {
            continue;
        }
        let Some(local_path) = transfer_task_local_path(task).map(Path::to_path_buf) else {
            continue;
        };
        if local_path != old_root && !local_path.starts_with(old_root) {
            continue;
        }

        let suffix = local_path
            .strip_prefix(old_root)
            .unwrap_or_else(|_| Path::new(""));
        let rewritten = if suffix.as_os_str().is_empty() {
            new_root.to_path_buf()
        } else {
            new_root.join(suffix)
        };
        set_transfer_task_local_path(task, rewritten);
    }
}
