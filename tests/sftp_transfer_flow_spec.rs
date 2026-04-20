use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::future::Future;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{Result, anyhow};
use mica_term::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, LocalTransferEntry, SftpBackend, SftpDirectoryEntry,
    SftpDirectoryEntryKind, SftpPanelMode, SftpRemoteMetadata, SftpRuntimeHandle,
    SftpSessionBindingState, SftpWriteMode, TransferConflictPolicy, TransferQueue,
    TransferResumeMode, TransferTask, TransferTaskAction, TransferTaskState,
    collect_download_targets, delete_entries, execute_queued_transfers,
    execute_queued_transfers_with_progress, move_entry_between_directories, scan_local_sources,
};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use uuid::Uuid;

struct MemoryFileReader {
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFileReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }
}

impl AsyncRead for MemoryFileReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut chunk = vec![0; buf.remaining()];
        let read = Read::read(&mut self.cursor, &mut chunk)?;
        buf.put_slice(&chunk[..read]);
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for MemoryFileReader {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

struct MemoryFileWriter {
    path: String,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFileWriter {
    fn new(path: String, files: Arc<Mutex<HashMap<String, Vec<u8>>>>, bytes: Vec<u8>) -> Self {
        Self {
            path,
            files,
            cursor: Cursor::new(bytes),
        }
    }

    fn persist(&self) {
        self.files
            .lock()
            .expect("lock files")
            .insert(self.path.clone(), self.cursor.get_ref().clone());
    }
}

impl Drop for MemoryFileWriter {
    fn drop(&mut self) {
        self.persist();
    }
}

impl AsyncWrite for MemoryFileWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let written = Write::write(&mut self.cursor, buf)?;
        self.persist();
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.persist();
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.persist();
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for MemoryFileWriter {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

#[derive(Default)]
struct MemoryBackend {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    directories: Arc<Mutex<HashSet<String>>>,
    directory_entries: Arc<Mutex<HashMap<String, Vec<SftpDirectoryEntry>>>>,
    mkdir_requests: Arc<Mutex<Vec<String>>>,
    rename_requests: Arc<Mutex<Vec<(String, String)>>>,
    delete_requests: Arc<Mutex<Vec<String>>>,
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

    fn set_directory_entries(&self, path: &str, entries: Vec<SftpDirectoryEntry>) {
        self.directory_entries
            .lock()
            .expect("lock directory entries")
            .insert(path.into(), entries);
    }

    fn file_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().expect("lock files").get(path).cloned()
    }
}

impl SftpBackend for MemoryBackend {
    fn read_dir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        Box::pin(async move {
            Ok(self
                .directory_entries
                .lock()
                .expect("lock directory entries")
                .get(path)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
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

    fn stat<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        Box::pin(async move {
            let size_bytes = self
                .files
                .lock()
                .expect("lock files")
                .get(path)
                .map(|bytes| bytes.len() as u64);
            if size_bytes.is_none() {
                return Err(anyhow!("missing remote file: {path}"));
            }

            Ok(SftpRemoteMetadata {
                size_bytes,
                modified_unix_seconds: Some(1_710_000_000),
            })
        })
    }

    fn open_file_reader<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        Box::pin(async move {
            let bytes = self
                .file_bytes(remote_path)
                .ok_or_else(|| anyhow!("missing remote file: {remote_path}"))?;
            Ok(Box::pin(MemoryFileReader::new(bytes)) as BoxedSftpReader)
        })
    }

    fn open_file_writer<'a>(
        &'a self,
        remote_path: &'a str,
        mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        Box::pin(async move {
            let initial = match mode {
                SftpWriteMode::CreateOrTruncate => Vec::new(),
                SftpWriteMode::CreateOrAppend => self.file_bytes(remote_path).unwrap_or_default(),
            };
            Ok(Box::pin(MemoryFileWriter::new(
                remote_path.to_string(),
                Arc::clone(&self.files),
                initial,
            )) as BoxedSftpWriter)
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
        modified_unix_seconds: None,
        size_bytes: Some(128),
    }
}

fn directory_entry(id: &str, name: &str, path: &str) -> SftpDirectoryEntry {
    SftpDirectoryEntry {
        id: id.into(),
        name: name.into(),
        path: path.into(),
        kind: SftpDirectoryEntryKind::Directory,
        modified_unix_seconds: None,
        size_bytes: None,
    }
}

fn task_local_download_path(task: &TransferTask) -> Option<PathBuf> {
    match &task.action {
        TransferTaskAction::Download { local_path }
        | TransferTaskAction::DownloadDirectory { local_path } => Some(local_path.clone()),
        _ => None,
    }
}

fn resumable_runtime_with_remote_bytes(bytes: &[u8]) -> SftpRuntimeHandle {
    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app"]));
    backend.insert_remote_file("/srv/app/archive.zip", bytes);
    SftpRuntimeHandle::new(backend)
}

fn seeded_download_queue(part_path: PathBuf, confirmed: u64) -> TransferQueue {
    let final_path = PathBuf::from(format!(
        "{}{}",
        part_path
            .to_string_lossy()
            .strip_suffix(".part")
            .expect("part file suffix"),
        ""
    ));

    let mut queue = TransferQueue::default();
    queue.tasks.push(TransferTask {
        id: "download-1".into(),
        session_id: "session-a".into(),
        source_path: "/srv/app/archive.zip".into(),
        target_path: final_path.to_string_lossy().to_string(),
        direction: mica_term::app::sftp::TransferDirection::Download,
        action: TransferTaskAction::Download {
            local_path: final_path,
        },
        state: TransferTaskState::Queued,
        bytes_total: confirmed,
        bytes_transferred: confirmed,
        bytes_confirmed: confirmed,
        temp_target_path: Some(part_path),
        resume_mode: TransferResumeMode::ResumeIfPossible,
        conflict_policy: None,
        error_message: None,
    });
    queue
}

#[tokio::test(flavor = "current_thread")]
async fn upload_and_download_tasks_reach_completed_and_update_session_summary() {
    let root = temp_test_root("transfer-flow");
    let upload_path = root.join("notes.txt");
    fs::write(&upload_path, b"hello from local").expect("write upload source");

    let backend = Arc::new(MemoryBackend::with_directories(&[
        "/srv",
        "/srv/app",
        "/srv/other",
    ]));
    backend.insert_remote_file("/srv/app/config.yml", b"port=22");
    let runtime = SftpRuntimeHandle::new(backend.clone());
    let sources = scan_local_sources(&[upload_path]).expect("scan upload source");

    let mut queue = TransferQueue::default();
    let upload_ids = queue.enqueue_upload("session-a", "/srv/app", &sources);
    assert_eq!(
        queue
            .task(upload_ids.first().expect("upload task id"))
            .expect("queued upload task")
            .state,
        TransferTaskState::Queued
    );

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued upload");

    assert_eq!(
        queue
            .task(upload_ids.first().expect("upload task id"))
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
        &[file_entry(
            "remote-config",
            "config.yml",
            "/srv/app/config.yml",
        )],
    );
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued download");

    assert_eq!(
        queue
            .task(download_ids.first().expect("download task id"))
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
async fn interrupted_download_resumes_from_local_part_file() {
    let root = temp_test_root("resume-download");
    let part_path = root.join("archive.zip.part");
    fs::write(&part_path, b"abc").expect("seed local part file");
    let mut queue = seeded_download_queue(part_path.clone(), 3);
    let runtime = resumable_runtime_with_remote_bytes(b"abcdefghi");

    execute_queued_transfers_with_progress(&runtime, &mut queue, |_| {})
        .await
        .expect("resume download");

    assert_eq!(
        fs::read(root.join("archive.zip")).expect("read completed download"),
        b"abcdefghi".to_vec()
    );
    assert!(
        !part_path.exists(),
        "completed resumable download should rename the part file into the final target"
    );
    let task = queue.task("download-1").expect("resumed download task");
    assert_eq!(task.state, TransferTaskState::Completed);
    assert_eq!(task.bytes_confirmed, 9);
    assert!(task.temp_target_path.is_none());
}

#[tokio::test(flavor = "current_thread")]
async fn download_resume_falls_back_to_restart_when_remote_shrinks() {
    let root = temp_test_root("resume-download-shrunk");
    let part_path = root.join("archive.zip.part");
    fs::write(&part_path, b"abcde").expect("seed oversized local part file");
    let mut queue = seeded_download_queue(part_path.clone(), 5);
    let runtime = resumable_runtime_with_remote_bytes(b"abc");

    let error = execute_queued_transfers_with_progress(&runtime, &mut queue, |_| {})
        .await
        .expect_err("shrunk remote should reject resume");

    assert!(error.to_string().contains("restart required"));
    let task = queue.task("download-1").expect("failed download task");
    assert_eq!(task.state, TransferTaskState::Failed);
    assert_eq!(task.resume_mode, TransferResumeMode::RestartOnly);
    assert!(part_path.exists());
    assert!(
        !root.join("archive.zip").exists(),
        "invalid resume should not silently overwrite the final target"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn moving_remote_entry_into_directory_updates_listing_and_clears_stale_selection() {
    let backend = Arc::new(MemoryBackend::with_directories(&[
        "/srv",
        "/srv/app",
        "/srv/archive",
    ]));
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

    let did_move =
        move_entry_between_directories(&runtime, &mut state, "release-id", "/srv/archive")
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

#[tokio::test(flavor = "current_thread")]
async fn recursive_folder_download_preserves_nested_files_and_empty_directories() {
    let root = temp_test_root("download-folder");
    let backend = Arc::new(MemoryBackend::with_directories(&[
        "/srv",
        "/srv/releases",
        "/srv/releases/current",
        "/srv/releases/empty-dir",
    ]));
    backend.set_directory_entries(
        "/srv/releases",
        vec![
            directory_entry("current-dir", "current", "/srv/releases/current"),
            directory_entry("empty-dir", "empty-dir", "/srv/releases/empty-dir"),
        ],
    );
    backend.set_directory_entries(
        "/srv/releases/current",
        vec![file_entry(
            "app-yml",
            "app.yml",
            "/srv/releases/current/app.yml",
        )],
    );
    backend.set_directory_entries("/srv/releases/empty-dir", vec![]);
    backend.insert_remote_file("/srv/releases/current/app.yml", b"port=22");

    let runtime = SftpRuntimeHandle::new(backend.clone());
    let download_targets = collect_download_targets(
        &runtime,
        root.as_path(),
        &[directory_entry("releases-dir", "releases", "/srv/releases")],
    )
    .await
    .expect("collect recursive folder download targets");

    let mut queue = TransferQueue::default();
    queue.enqueue_download_targets("session-a", &download_targets);
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run recursive folder download");

    assert_eq!(
        fs::read(root.join("releases/current/app.yml")).expect("read nested downloaded file"),
        b"port=22".to_vec()
    );
    assert!(
        root.join("releases/empty-dir").is_dir(),
        "recursive folder download should preserve empty directories instead of dropping them"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn download_auto_rename_uses_numeric_suffixes() {
    let root = temp_test_root("download-auto-rename");
    let existing_path = root.join("report.txt");
    fs::write(&existing_path, b"existing report").expect("seed conflicting local file");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv"]));
    backend.insert_remote_file("/srv/report.txt", b"fresh report");
    let runtime = SftpRuntimeHandle::new(backend);

    let mut queue = TransferQueue::default();
    let task_id = queue
        .enqueue_download(
            "session-a",
            root.as_path(),
            &[file_entry("report", "report.txt", "/srv/report.txt")],
        )
        .into_iter()
        .next()
        .expect("download task id");

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued download to conflict");

    assert_eq!(
        queue.task(&task_id).expect("conflicted task").state,
        TransferTaskState::Conflict
    );
    assert!(queue.resume_conflict(&task_id, TransferConflictPolicy::AutoRename));
    assert_eq!(
        task_local_download_path(queue.task(&task_id).expect("renamed task")).as_deref(),
        Some(root.join("report (1).txt").as_path())
    );

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("rerun auto-renamed download");

    assert_eq!(
        queue.task(&task_id).expect("completed task").state,
        TransferTaskState::Completed
    );
    assert_eq!(
        fs::read(root.join("report (1).txt")).expect("read renamed download"),
        b"fresh report".to_vec()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn directory_download_auto_rename_rewrites_nested_targets() {
    let root = temp_test_root("download-directory-auto-rename");
    fs::create_dir_all(root.join("logs")).expect("seed conflicting download root");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/logs"]));
    backend.set_directory_entries(
        "/srv/logs",
        vec![file_entry("app-log", "app.log", "/srv/logs/app.log")],
    );
    backend.insert_remote_file("/srv/logs/app.log", b"log-bytes");
    let runtime = SftpRuntimeHandle::new(backend);

    let download_targets = collect_download_targets(
        &runtime,
        root.as_path(),
        &[directory_entry("logs-dir", "logs", "/srv/logs")],
    )
    .await
    .expect("collect recursive folder download targets");

    let mut queue = TransferQueue::default();
    let task_ids = queue.enqueue_download_targets("session-a", &download_targets);
    let root_task_id = task_ids.first().expect("root task id").clone();
    let child_task_id = task_ids.get(1).expect("child task id").clone();

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run queued directory download to conflict");

    assert_eq!(
        queue
            .task(&root_task_id)
            .expect("conflicted root task")
            .state,
        TransferTaskState::Conflict
    );
    assert_eq!(
        queue.task(&child_task_id).expect("queued child task").state,
        TransferTaskState::Queued
    );

    assert!(queue.resume_conflict(&root_task_id, TransferConflictPolicy::AutoRename));
    assert_eq!(
        task_local_download_path(queue.task(&child_task_id).expect("renamed child")).as_deref(),
        Some(root.join("logs (1)").join("app.log").as_path())
    );

    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("rerun auto-renamed directory download");

    assert_eq!(
        queue
            .task(&root_task_id)
            .expect("completed root task")
            .state,
        TransferTaskState::Completed
    );
    assert_eq!(
        queue
            .task(&child_task_id)
            .expect("completed child task")
            .state,
        TransferTaskState::Completed
    );
    assert_eq!(
        fs::read(root.join("logs (1)").join("app.log")).expect("read renamed nested download"),
        b"log-bytes".to_vec()
    );
}

#[tokio::test(flavor = "current_thread")]
async fn recursive_folder_upload_preserves_root_directory_and_empty_directories() {
    let root = temp_test_root("upload-folder");
    let folder_path = root.join("releases");
    let nested_file = folder_path.join("current").join("app.yml");
    fs::create_dir_all(folder_path.join("current")).expect("create nested upload dir");
    fs::create_dir_all(folder_path.join("empty-dir")).expect("create empty upload dir");
    fs::write(&nested_file, b"port=22").expect("write nested upload file");

    let backend = Arc::new(MemoryBackend::with_directories(&["/srv", "/srv/app"]));
    let runtime = SftpRuntimeHandle::new(backend.clone());
    let sources = scan_local_sources(&[folder_path]).expect("scan recursive folder upload");

    let mut queue = TransferQueue::default();
    queue.enqueue_upload("session-a", "/srv/app", &sources);
    execute_queued_transfers(&runtime, &mut queue)
        .await
        .expect("run recursive folder upload");

    assert_eq!(
        backend.file_bytes("/srv/app/releases/current/app.yml"),
        Some(b"port=22".to_vec())
    );
    assert!(
        backend
            .directories
            .lock()
            .expect("lock directories")
            .contains("/srv/app/releases/empty-dir"),
        "recursive folder upload should create empty remote directories instead of dropping them"
    );
}
