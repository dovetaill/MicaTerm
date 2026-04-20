use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mica_term::app::sftp::{
    RedbTransferStore, TransferDirection, TransferResumeMode, TransferTask, TransferTaskAction,
    TransferTaskState,
};

fn temp_data_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join(format!("{name}-{unique}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn test_store(name: &str) -> RedbTransferStore {
    RedbTransferStore::new(temp_data_dir(name))
}

fn sample_interrupted_download_task() -> TransferTask {
    TransferTask {
        id: "download-1".into(),
        session_id: "session-a".into(),
        source_path: "/srv/archive.zip".into(),
        target_path: "/tmp/archive.zip".into(),
        direction: TransferDirection::Download,
        action: TransferTaskAction::Download {
            local_path: PathBuf::from("/tmp/archive.zip"),
        },
        state: TransferTaskState::Interrupted,
        bytes_total: 1024,
        bytes_transferred: 512,
        bytes_confirmed: 512,
        temp_target_path: Some(PathBuf::from("/tmp/archive.zip.part")),
        resume_mode: TransferResumeMode::ResumeIfPossible,
        conflict_policy: None,
        error_message: Some("network interrupted".into()),
    }
}

#[test]
fn persisted_transfer_store_roundtrips_interrupted_task() {
    let store = test_store("sftp-transfer-store-roundtrip");
    let task = sample_interrupted_download_task();

    store.save_tasks(&[task.clone()]).expect("save tasks");
    let loaded = store.load_tasks().expect("load tasks");

    assert_eq!(loaded, vec![task]);
}

#[test]
fn persisted_transfer_store_returns_empty_when_database_does_not_exist() {
    let store = test_store("sftp-transfer-store-empty");
    fs::remove_file(&store.database_path).ok();

    assert!(store.load_tasks().expect("load empty").is_empty());
}

#[test]
fn clear_completed_only_removes_completed_and_cancelled_tasks() {
    let store = test_store("sftp-transfer-store-clear-completed");
    let interrupted = sample_interrupted_download_task();
    let completed = TransferTask {
        id: "download-2".into(),
        state: TransferTaskState::Completed,
        error_message: None,
        ..sample_interrupted_download_task()
    };
    let cancelled = TransferTask {
        id: "download-3".into(),
        state: TransferTaskState::Cancelled,
        error_message: Some("cancelled by user".into()),
        ..sample_interrupted_download_task()
    };

    store
        .save_tasks(&[interrupted.clone(), completed, cancelled])
        .expect("save tasks");

    store.clear_completed().expect("clear completed tasks");
    let loaded = store.load_tasks().expect("reload tasks");

    assert_eq!(loaded, vec![interrupted]);
}
