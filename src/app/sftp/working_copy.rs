use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpWorkingCopy {
    pub task_id: String,
    pub session_id: Uuid,
    pub remote_path: String,
    pub local_path: PathBuf,
    pub upload_on_save: bool,
}

impl SftpWorkingCopy {
    pub fn new(
        session_id: Uuid,
        remote_path: impl Into<String>,
        local_path: PathBuf,
        upload_on_save: bool,
    ) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            session_id,
            remote_path: remote_path.into(),
            local_path,
            upload_on_save,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingCopySnapshot {
    pub modified_at: Option<SystemTime>,
    pub size_bytes: u64,
}

pub fn snapshot_working_copy(path: &Path) -> Option<WorkingCopySnapshot> {
    let metadata = fs::metadata(path).ok()?;
    Some(WorkingCopySnapshot {
        modified_at: metadata.modified().ok(),
        size_bytes: metadata.len(),
    })
}

pub fn working_copy_has_changed(
    previous: &Option<WorkingCopySnapshot>,
    next: &Option<WorkingCopySnapshot>,
) -> bool {
    previous != next
}
