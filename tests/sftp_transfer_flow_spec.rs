use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mica_term::app::sftp::{
    LocalTransferEntry, SftpBackend, SftpDirectoryEntry, SftpDirectoryEntryKind, SftpPanelMode,
    SftpRuntimeHandle, SftpSessionBindingState, TransferConflictPolicy, TransferQueue,
    TransferTaskState, delete_entries, execute_queued_transfers, move_entry_between_directories,
    scan_local_sources,
};
use uuid::Uuid;

#[derive(Default)]
struct MemoryBackend {
    files: Mutex<HashMap<String, Vec<u8>>>,
    directories: Mutex<HashSet<String>>,
    mkdir_requests: Mutex<Vec<String>>,
    rename_requests: Mutex<Vec<(String, String)>>,
    delete_requests: Mutex<Vec<String>>,
}

impl MemoryBackend {
    fn with_directories(paths: &[&str]) -> Self {
        let backend = Self::default();
        {
            let mut directories = backend.directories.lock().expect("lock directories");
            for path in paths {
                directories.insert((*path).to_string());
            }
        }
        backend
    }

    fn insert_remote_file(&self, path: &str, bytes: &[u8]) {
        self.files
            .lock()
            .expect("lock files")
            .insert(path.into(), bytes.to_vec());
    }

    fn file_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().expect("lock files").get(path).cloned()
    }
}

impl SftpBackend for MemoryBackend {
    fn read_dir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn mkdir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.mkdir_requests
                .lock()
                .expect("lock mkdir requests")
                .push(path.to_string());
            self.directories
                .lock()
                .expect("lock directories")
                .insert(path.to_string());
            Ok(())
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a str,
        to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.rename_requests
                .lock()
                .expect("lock rename requests")
                .push((from.to_string(), to.to_string()));
            let mut files = self.files.lock().expect("lock files");
            if let Some(bytes) = files.remove(from) {
                files.insert(to.to_string(), bytes);
            }
            Ok(())
        })
    }

    fn path_exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            let has_file = self.files.lock().expect("lock files").contains_key(path);
            let has_dir = self
                .directories
                .lock()
                .expect("lock directories")
                .contains(path);
            Ok(has_file || has_dir)
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            self.files
                .lock()
                .expect("lock files")
                .insert(remote_path.to_string(), data.clone());
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move {
            self.file_bytes(remote_path)
                .ok_or_else(|| anyhow!("missing remote file: {remote_path}"))
        })
    }

    fn remove_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_requests
                .lock()
                .expect("lock delete requests")
                .push(remote_path.to_string());
            self.files.lock().expect("lock files").remove(remote_path);
            Ok(())
        })
    }

    fn remove_dir<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            self.delete_requests
                .lock()
                .expect("lock delete requests")
                .push(remote_path.to_string());
            self.directories
                .lock()
                .expect("lock directories")
                .remove(remote_path);
            Ok(())
        })
    }
}

fn temp_test_root(label: &str) -> PathBuf {
    let root = env::temp_dir().join(format!("mica-term-{label}-{}", Uuid::new_v4()));
    fs::create_dir_all(&root).expect("create temp test root");
    root
}

fn file_entry(id: &str, name: &str, path: &str) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        id: id.into(),
        name: name.into(),
        path: path.into(),
        kind: SftpDirectoryEntryKind::File,
        size_bytes: Some(128),
    }
}

#[tokio::test(flavor = "current_thread")]
async fn upload_and_download_tasks_reach_completed_and_update_session_summary() {
    let root = temp_test_root("transfer-flow");
    let upload_path = root.join("notes.txt");
    fs::write(&upload_path, b"hello from local").expect("write upload source");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app", "/srv/other"]));
    backend.insert_remote_file("/srv/app/config.yml", b"port=22");
    let runtime = SftpRuntimeHandle::new(backend.clone());
    let sources = scan_local_sources(&[upload_path]).expect("scan upload source");

    let mut queue = TransferQueue::default();
    let upload_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    assert_eq!(
        queue.task(upload_ids.first().expect("upload task id"))
            .expect("queued upload task")
            .state,
        TransferTaskState::Queued
    );

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued upload");

    assert_eq!(
        queue.task(upload_ids.first().expect("upload task id"))
            .expect("completed upload task")
            .state,
        TransferTaskState::Completed
    );
    assert_eq!(
        backend.file_bytes("/srv/app/notes.txt"),
        Some(b"hello from local".to_vec())
    );

    let session_b_source = LocalTransferEntry {
        local_path: root.join("other.txt"),
        relative_path: PathBuf::from("other.txt"),
        bytes_total: 5,
    };
    fs::write(&session_b_source.local_path, b"other").expect("write secondary upload source");
    queue.enqueue_upload("session-b", "/srv/other", &[session_b_source]);
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run secondary upload");

    let download_ids = queue.enqueue_download(
        "session-a",
        root.as_path(),
        &[file_entry("remote-config", "config.yml", "/srv/app/config.yml")],
    );
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued download");

    assert_eq!(
        queue.task(download_ids.first().expect("download task id"))
            .expect("completed download task")
            .state,
        TransferTaskState::Completed
    );
    assert_eq!(
        fs::read(root.join("config.yml")).expect("read downloaded file"),
        b"port=22".to_vec()
    );

    let summary = queue.summary(Some("session-a"));
    assert_eq!(summary.total_count, 3);
    assert_eq!(summary.active_count, 0);
    assert_eq!(summary.failed_count, 0);
    assert_eq!(summary.current_session_count, 2);
}

#[tokio::test(flavor = "current_thread")]
async fn moving_remote_entry_into_directory_updates_listing_and_clears_stale_selection() {
    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app", "/srv/archive"]));
    backend.insert_remote_file("/srv/app/release.tar.gz", b"release");
    let runtime = SftpRuntimeHandle::new(backend.clone());

    let mut state = SftpSessionBindingState::follow("/srv/app");
    state.mode = SftpPanelMode::Ready;
    state.entries = vec![file_entry(
        "release-id",
        "release.tar.gz",
        "/srv/app/release.tar.gz",
    )];
    state.selected_entry_ids = vec!["release-id".into()];

    let did_move = move_entry_between_directories(&runtime, &mut state, "release-id", "/srv/archive")
        .await
        .expect("move remote entry");

    assert!(did_move);
    assert!(state.entries.is_empty());
    assert!(state.selected_entry_ids.is_empty());
    assert_eq!(
        backend
            .rename_requests
            .lock()
            .expect("lock rename requests")
            .as_slice(),
        &[(
            "/srv/app/release.tar.gz".to_string(),
            "/srv/archive/release.tar.gz".to_string(),
        )]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn deleting_entries_cancels_conflicting_tasks_and_updates_listing() {
    let root = temp_test_root("delete-flow");
    let local_path = root.join("release.tar.gz");
    fs::write(&local_path, b"release").expect("write upload source");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app"]));
    backend.insert_remote_file("/srv/app/release.tar.gz", b"remote");
    let runtime = SftpRuntimeHandle::new(backend.clone());
    let sources = scan_local_sources(&[local_path]).expect("scan upload source");

    let mut queue = TransferQueue::default();
    let queued_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let queued_id = queued_ids.first().expect("queued upload task").clone();
    assert!(queue.mark_running(&queued_id));

    let mut state = SftpSessionBindingState::follow("/srv/app");
    state.mode = SftpPanelMode::Ready;
    state.entries = vec![file_entry(
        "release-id",
        "release.tar.gz",
        "/srv/app/release.tar.gz",
    )];
    state.selected_entry_ids = vec!["release-id".into()];

    let removed = delete_entries(
        &runtime,
        &mut queue,
        "session-a",
        &mut state,
        &["release-id".to_string()],
    )
    .await
    .expect("delete remote entry");

    assert_eq!(removed, 1);
    assert_eq!(
        queue.task(&queued_id).expect("cancelled upload task").state,
        TransferTaskState::Cancelled
    );
    assert!(state.entries.is_empty());
    assert!(state.selected_entry_ids.is_empty());
    assert_eq!(
        backend
            .delete_requests
            .lock()
            .expect("lock delete requests")
            .as_slice(),
        &["/srv/app/release.tar.gz".to_string()]
    );
}

#[tokio::test(flavor = "current_thread")]
async fn conflict_task_can_resume_and_complete_with_selected_policy() {
    let root = temp_test_root("resume-conflict");
    let local_path = root.join("config.yml");
    fs::write(&local_path, b"port=2200").expect("write upload source");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app"]));
    backend.insert_remote_file("/srv/app/config.yml", b"port=22");
    let runtime = SftpRuntimeHandle::new(backend.clone());
    let sources = scan_local_sources(&[local_path]).expect("scan upload source");

    let mut queue = TransferQueue::default();
    let task_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    let task_id = task_ids.first().expect("upload task id").clone();

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued upload to conflict");

    assert_eq!(
        queue.task(&task_id).expect("conflicted task").state,
        TransferTaskState::Conflict
    );

    assert!(queue.resume_conflict(&task_id, TransferConflictPolicy::Overwrite));
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("rerun upload with overwrite policy");

    assert_eq!(
        queue.task(&task_id).expect("completed task").state,
        TransferTaskState::Completed
    );
    assert_eq!(
        backend.file_bytes("/srv/app/config.yml"),
        Some(b"port=2200".to_vec())
    );
}
