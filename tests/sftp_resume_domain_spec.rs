use std::path::{Path, PathBuf};

use mica_term::app::app_paths::{AppRootPaths, AppRootSource};
use mica_term::app::sftp::{
    download_part_path, TransferDirection, TransferResumeMode, TransferTask, TransferTaskAction,
    TransferTaskState,
};

fn sample_app_root_paths() -> AppRootPaths {
    AppRootPaths {
        root_source: AppRootSource::StandardLocalData,
        root_dir: PathBuf::from("/tmp/mica-term"),
        data_dir: PathBuf::from("/tmp/mica-term/data"),
        logs_dir: PathBuf::from("/tmp/mica-term/logs"),
        crash_dir: PathBuf::from("/tmp/mica-term/crash"),
    }
}

#[test]
fn transfer_task_state_reports_resume_related_states() {
    assert!(TransferTaskState::Interrupted.needs_attention());
    assert!(!TransferTaskState::VerifyingResume.needs_attention());
}

#[test]
fn app_root_paths_exposes_transfer_database_path() {
    let paths = sample_app_root_paths();
    assert_eq!(paths.transfer_database_path(), paths.data_dir.join("transfers.redb"));
}

#[test]
fn download_task_uses_part_file_path() {
    let part_path = download_part_path(Path::new("/tmp/report.zip"));
    assert_eq!(part_path, PathBuf::from("/tmp/report.zip.part"));
}

#[test]
fn resumable_task_carries_temp_target_checkpoint_and_mode() {
    let task = TransferTask {
        id: "download-1".into(),
        session_id: "session-a".into(),
        source_path: "/srv/report.zip".into(),
        target_path: "/tmp/report.zip".into(),
        direction: TransferDirection::Download,
        action: TransferTaskAction::Download {
            local_path: PathBuf::from("/tmp/report.zip"),
        },
        state: TransferTaskState::Queued,
        bytes_total: 1024,
        bytes_transferred: 512,
        bytes_confirmed: 512,
        temp_target_path: Some(PathBuf::from("/tmp/report.zip.part")),
        resume_mode: TransferResumeMode::ResumeIfPossible,
        conflict_policy: None,
        error_message: None,
    };

    assert_eq!(task.bytes_confirmed, 512);
    assert_eq!(
        task.temp_target_path.as_deref(),
        Some(Path::new("/tmp/report.zip.part"))
    );
    assert_eq!(task.resume_mode, TransferResumeMode::ResumeIfPossible);
}
