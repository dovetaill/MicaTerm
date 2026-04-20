use std::env;
use std::fs;
use std::path::PathBuf;

use mica_term::app::sftp::{
    DownloadTransferEntry, SftpDirectoryEntryKind, TransferConflictPolicy, TransferDirection,
    TransferQueue, TransferQueueSummary, TransferResumeMode, TransferTask, TransferTaskAction,
    TransferTaskState, scan_local_sources,
};
use uuid::Uuid;

fn task(id: &str, session_id: &str, state: TransferTaskState) -> TransferTask {
    TransferTask {
        id: id.into(),
        session_id: session_id.into(),
        source_path: format!("/local/{id}"),
        target_path: format!("/remote/{id}"),
        direction: TransferDirection::Upload,
        action: TransferTaskAction::Upload {
            local_path: PathBuf::from(format!("/local/{id}")),
        },
        state,
        bytes_total: 1_024,
        bytes_transferred: 512,
        bytes_confirmed: 256,
        temp_target_path: Some(PathBuf::from(format!("/tmp/{id}.part"))),
        resume_mode: TransferResumeMode::ResumeIfPossible,
        conflict_policy: None,
        error_message: None,
    }
}

fn temp_test_root(label: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("mica-term-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create temp test root");
    root
}

#[test]
fn queue_summary_counts_active_failed_and_current_session_tasks() {
    let tasks = vec![
        task("queued", "session-a", TransferTaskState::Queued),
        task("running", "session-a", TransferTaskState::Running),
        task("done", "session-a", TransferTaskState::Completed),
        task("failed", "session-b", TransferTaskState::Failed),
        task("cancelled", "session-c", TransferTaskState::Cancelled),
    ];

    let summary = TransferQueueSummary::from_tasks(&tasks, Some("session-a"));

    assert_eq!(summary.total_count, 5);
    assert_eq!(summary.active_count, 2);
    assert_eq!(summary.failed_count, 1);
    assert_eq!(summary.current_session_count, 3);
}

#[test]
fn queue_summary_treats_conflicts_as_attention_required() {
    let tasks = vec![task("conflict", "session-a", TransferTaskState::Conflict)];

    let summary = TransferQueueSummary::from_tasks(&tasks, Some("session-a"));

    assert_eq!(summary.active_count, 0);
    assert_eq!(summary.failed_count, 1);
    assert_eq!(summary.current_session_count, 1);
}

#[test]
fn transfer_task_state_reports_active_states() {
    assert!(TransferTaskState::Queued.is_active());
    assert!(TransferTaskState::Running.is_active());
    assert!(TransferTaskState::Paused.is_active());
    assert!(TransferTaskState::VerifyingResume.is_active());
    assert!(!TransferTaskState::Interrupted.is_active());
    assert!(!TransferTaskState::Completed.is_active());
    assert!(!TransferTaskState::Failed.is_active());
    assert!(!TransferTaskState::Cancelled.is_active());
    assert!(!TransferTaskState::Conflict.is_active());
}

#[test]
fn pause_task_preserves_confirmed_progress_checkpoint() {
    let mut queue = TransferQueue {
        tasks: vec![task(
            "paused-candidate",
            "session-a",
            TransferTaskState::Running,
        )],
    };

    assert!(queue.pause_task("paused-candidate"));

    let task = queue.task("paused-candidate").expect("paused task");
    assert_eq!(task.state, TransferTaskState::Paused);
    assert_eq!(task.bytes_transferred, 512);
    assert_eq!(task.bytes_confirmed, 256);
}

#[test]
fn interrupt_task_marks_task_interrupted_without_discarding_checkpoint() {
    let mut queue = TransferQueue {
        tasks: vec![task(
            "interrupt-candidate",
            "session-a",
            TransferTaskState::VerifyingResume,
        )],
    };

    assert!(queue.interrupt_task("interrupt-candidate", "network dropped"));

    let task = queue.task("interrupt-candidate").expect("interrupted task");
    assert_eq!(task.state, TransferTaskState::Interrupted);
    assert_eq!(task.bytes_transferred, 512);
    assert_eq!(task.bytes_confirmed, 256);
    assert_eq!(task.error_message.as_deref(), Some("network dropped"));
}

#[test]
fn restart_task_resets_progress_error_and_temp_target_path() {
    let mut queue = TransferQueue {
        tasks: vec![TransferTask {
            error_message: Some("resume failed".into()),
            state: TransferTaskState::Interrupted,
            resume_mode: TransferResumeMode::RestartOnly,
            ..task(
                "restart-candidate",
                "session-a",
                TransferTaskState::Interrupted,
            )
        }],
    };

    assert!(queue.restart_task("restart-candidate"));

    let task = queue.task("restart-candidate").expect("restarted task");
    assert_eq!(task.state, TransferTaskState::Queued);
    assert_eq!(task.bytes_transferred, 0);
    assert_eq!(task.bytes_confirmed, 0);
    assert!(task.temp_target_path.is_none());
    assert_eq!(task.resume_mode, TransferResumeMode::ResumeIfPossible);
    assert!(task.error_message.is_none());
}

#[test]
fn upload_to_folder_creates_queue_task_bound_to_session() {
    let root = temp_test_root("queue-upload");
    let local_path = root.join("release.tar.gz");
    fs::write(&local_path, b"release-bytes").expect("write temp upload source");

    let sources = scan_local_sources(&[local_path]).expect("scan local sources");
    let mut queue = TransferQueue::default();

    let task_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let task = queue
        .task(task_ids.first().expect("upload task id"))
        .expect("queued upload task");

    assert_eq!(task.state, TransferTaskState::Queued);
    assert_eq!(task.session_id, "session-a");
    assert_eq!(task.direction, TransferDirection::Upload);
    assert_eq!(task.target_path, "/srv/app/release.tar.gz");
    assert_eq!(task.bytes_total, b"release-bytes".len() as u64);
}

#[test]
fn deletion_cancels_conflicting_transfer_tasks() {
    let root = temp_test_root("queue-delete-conflict");
    let local_path = root.join("release.tar.gz");
    fs::write(&local_path, b"release-bytes").expect("write temp upload source");
    let sources = scan_local_sources(&[local_path]).expect("scan local sources");

    let mut queue = TransferQueue::default();
    let task_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let task_id = task_ids.first().expect("upload task id").clone();

    assert!(queue.mark_running(&task_id));
    assert_eq!(
        queue.cancel_conflicting_paths("session-a", &["/srv/app/release.tar.gz".into()]),
        1
    );
    assert_eq!(
        queue.task(&task_id).expect("cancelled task").state,
        TransferTaskState::Cancelled
    );
}

#[test]
fn conflict_task_can_resume_with_selected_policy() {
    let root = temp_test_root("queue-resume-conflict");
    let local_path = root.join("config.toml");
    fs::write(&local_path, b"port=22").expect("write temp upload source");
    let sources = scan_local_sources(&[local_path]).expect("scan local sources");

    let mut queue = TransferQueue::default();
    let task_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let task_id = task_ids.first().expect("upload task id").clone();

    assert!(queue.mark_conflict(&task_id, "Remote path already exists"));
    assert!(queue.resume_conflict(&task_id, TransferConflictPolicy::Overwrite));

    let task = queue.task(&task_id).expect("resumed task");
    assert_eq!(task.state, TransferTaskState::Queued);
    assert_eq!(
        task.conflict_policy,
        Some(TransferConflictPolicy::Overwrite)
    );
}

#[test]
fn folder_upload_tasks_preserve_selected_root_directory_name() {
    let root = temp_test_root("queue-upload-folder-root");
    let folder_path = root.join("releases");
    let nested_path = folder_path.join("app.yml");
    fs::create_dir_all(folder_path.join("empty-dir")).expect("create nested empty dir");
    fs::write(&nested_path, b"port=22").expect("write nested upload source");

    let sources = scan_local_sources(&[folder_path]).expect("scan local folder source");
    let mut queue = TransferQueue::default();

    let task_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let target_paths = task_ids
        .iter()
        .filter_map(|task_id| queue.task(task_id))
        .map(|task| task.target_path.clone())
        .collect::<Vec<_>>();

    assert!(
        target_paths
            .iter()
            .any(|path| path == "/srv/app/releases/app.yml"),
        "folder upload should preserve the selected folder name when projecting remote upload targets"
    );
    assert!(
        target_paths
            .iter()
            .any(|path| path == "/srv/app/releases/empty-dir"),
        "folder upload should keep empty nested directories in the transfer queue"
    );
}

#[test]
fn download_conflict_cancel_only_cancels_current_task() {
    let root = temp_test_root("queue-download-conflict-cancel-current");
    let current_path = root.join("report.txt");
    let later_path = root.join("notes.txt");
    fs::write(&current_path, b"existing report").expect("write current download target");
    fs::write(&later_path, b"existing notes").expect("write later download target");

    let mut queue = TransferQueue::default();
    let task_ids = queue.enqueue_download_targets(
        "session-a",
        &[
            DownloadTransferEntry {
                remote_path: "/srv/report.txt".into(),
                local_path: current_path,
                entry_kind: SftpDirectoryEntryKind::File,
                bytes_total: 12,
            },
            DownloadTransferEntry {
                remote_path: "/srv/notes.txt".into(),
                local_path: later_path,
                entry_kind: SftpDirectoryEntryKind::File,
                bytes_total: 12,
            },
        ],
    );

    let cancelled_id = task_ids.first().expect("current conflict task id").clone();
    let later_conflict_id = task_ids.get(1).expect("later conflict task id").clone();

    assert!(queue.mark_conflict(&cancelled_id, "Local path already exists"));
    assert!(queue.mark_conflict(&later_conflict_id, "Local path already exists"));
    assert!(queue.resume_conflict(&cancelled_id, TransferConflictPolicy::CancelCurrent));

    assert_eq!(
        queue.task(&cancelled_id).expect("cancelled task").state,
        TransferTaskState::Cancelled
    );
    assert_eq!(
        queue
            .task(&later_conflict_id)
            .expect("later conflict task")
            .state,
        TransferTaskState::Conflict
    );
}
