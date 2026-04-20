# SFTP Resumable Transfer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build persisted resumable SFTP upload/download with chunked transfer, pause/resume/restart semantics, and transfer-center recovery across reconnects and app restarts.

**Architecture:** Extend the existing SFTP queue into a durable transfer system backed by `redb`, then replace the current whole-file runtime calls with chunked reader/writer flows that can seek to an offset and continue from `.part` artifacts. Keep the existing Transfer Center surface and `ShellViewModel` as the projection layer, but add richer task states (`Interrupted`, `VerifyingResume`) plus startup recovery and explicit `Resume`/`Restart` actions.

**Tech Stack:** Rust, Slint, Tokio, russh-sftp 2.1.1, redb, cargo test, cargo check

---

### Task 1: Freeze the resumable transfer domain contract

**Files:**
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/app/app_paths.rs`
- Create: `tests/sftp_resume_domain_spec.rs`

**Step 1: Write the failing test**

Add domain-level tests that lock the new states and helper paths before any production code changes:

```rust
#[test]
fn transfer_task_state_reports_resume_related_states() {
    assert!(TransferTaskState::Interrupted.needs_attention());
    assert!(!TransferTaskState::VerifyingResume.needs_attention());
}

#[test]
fn app_root_paths_exposes_transfer_database_path() {
    let paths = sample_app_root_paths();
    assert_eq!(
        paths.transfer_database_path(),
        paths.data_dir.join("transfers.redb")
    );
}

#[test]
fn download_task_uses_part_file_path() {
    let part_path = download_part_path(Path::new("/tmp/report.zip"));
    assert_eq!(part_path, PathBuf::from("/tmp/report.zip.part"));
}
```

Also add a task-model test that proves a resumable task can carry `temp_target_path`, `bytes_confirmed`, and `resume_mode`.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_resume_domain_spec -q
```

Expected:

- FAIL because `Interrupted` / `VerifyingResume` / `transfer_database_path` / resumable helpers do not exist yet.

**Step 3: Write minimal implementation**

Add the narrowest possible contract in `src/app/sftp/queue.rs` and `src/app/app_paths.rs`:

```rust
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

pub enum TransferResumeMode {
    ResumeIfPossible,
    RestartOnly,
}

impl AppRootPaths {
    pub fn transfer_database_path(&self) -> PathBuf {
        self.data_dir.join("transfers.redb")
    }
}
```

Add simple helper functions for `.part` naming in the queue module or a small sibling helper module, but do not implement persistence or runtime behavior yet.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test sftp_resume_domain_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/queue.rs src/app/app_paths.rs tests/sftp_resume_domain_spec.rs
git commit -m "test: freeze resumable transfer domain contract"
```

### Task 2: Add the persisted transfer store

**Files:**
- Create: `src/app/sftp/transfer_store.rs`
- Modify: `src/app/sftp/mod.rs`
- Modify: `src/app/sftp/queue.rs`
- Create: `tests/sftp_transfer_store_spec.rs`

**Step 1: Write the failing test**

Create storage-focused tests that prove resumable tasks round-trip through `redb`:

```rust
#[test]
fn persisted_transfer_store_roundtrips_interrupted_task() {
    let store = test_store();
    let task = sample_interrupted_download_task();

    store.save_tasks(&[task.clone()]).expect("save tasks");
    let loaded = store.load_tasks().expect("load tasks");

    assert_eq!(loaded, vec![task]);
}

#[test]
fn persisted_transfer_store_returns_empty_when_database_does_not_exist() {
    let store = test_store();
    assert!(store.load_tasks().expect("load empty").is_empty());
}
```

Add one more test that proves `clear_completed()` only removes completed / cancelled rows and preserves interrupted rows.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_transfer_store_spec -q
```

Expected:

- FAIL because `transfer_store` and persistence record types do not exist.

**Step 3: Write minimal implementation**

Create `src/app/sftp/transfer_store.rs` modeled after the existing asset/keychain `redb` stores:

```rust
pub struct RedbTransferStore {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl RedbTransferStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            database_path: data_dir.join("transfers.redb"),
            data_dir,
        }
    }

    pub fn load_tasks(&self) -> Result<Vec<TransferTask>> { /* ... */ }
    pub fn save_tasks(&self, tasks: &[TransferTask]) -> Result<()> { /* ... */ }
    pub fn clear_completed(&self) -> Result<()> { /* ... */ }
}
```

Use one metadata table for schema version and one task table keyed by `task.id`. Serialize task rows with `bincode`, matching the local-store style already used in the repo.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test sftp_transfer_store_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/transfer_store.rs src/app/sftp/mod.rs src/app/sftp/queue.rs tests/sftp_transfer_store_spec.rs
git commit -m "feat: add persisted sftp transfer store"
```

### Task 3: Add streamable SFTP runtime contracts

**Files:**
- Modify: `src/app/sftp/runtime.rs`
- Modify: `src/app/ssh/runtime/sftp_backend.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `tests/sftp_runtime_spec.rs`

**Step 1: Write the failing test**

Extend `tests/sftp_runtime_spec.rs` with runtime contract tests for seekable remote handles:

```rust
#[tokio::test]
async fn runtime_can_open_seekable_reader_and_writer() {
    let runtime = test_runtime();
    let mut writer = runtime
        .open_file_writer("/srv/app/report.zip.part", SftpWriteMode::CreateOrAppend)
        .await
        .expect("open writer");
    writer.seek(SeekFrom::Start(3)).await.expect("seek writer");

    let mut reader = runtime
        .open_file_reader("/srv/app/report.zip.part")
        .await
        .expect("open reader");
    reader.seek(SeekFrom::Start(3)).await.expect("seek reader");
}
```

Also add a metadata test that expects remote size and `mtime` retrieval without downloading the full file.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_runtime_spec -q
```

Expected:

- FAIL because the runtime only exposes whole-file `upload_file` / `download_file` helpers.

**Step 3: Write minimal implementation**

Introduce boxed handle traits in `src/app/sftp/runtime.rs`:

```rust
pub trait SftpAsyncReader: tokio::io::AsyncRead + tokio::io::AsyncSeek + Send + Unpin {}
impl<T> SftpAsyncReader for T where T: tokio::io::AsyncRead + tokio::io::AsyncSeek + Send + Unpin {}

pub trait SftpAsyncWriter: tokio::io::AsyncWrite + tokio::io::AsyncSeek + Send + Unpin {}
impl<T> SftpAsyncWriter for T where T: tokio::io::AsyncWrite + tokio::io::AsyncSeek + Send + Unpin {}

pub type BoxedSftpReader = Pin<Box<dyn SftpAsyncReader>>;
pub type BoxedSftpWriter = Pin<Box<dyn SftpAsyncWriter>>;
```

Extend `SftpBackend` and `SftpRuntimeHandle` with:

```rust
fn stat<'a>(&'a self, path: &'a str) -> SftpOperationFuture<'a, SftpRemoteMetadata>;
fn open_file_reader<'a>(&'a self, path: &'a str) -> SftpReaderFuture<'a>;
fn open_file_writer<'a>(&'a self, path: &'a str, mode: SftpWriteMode) -> SftpWriterFuture<'a>;
```

Implement them in `src/app/ssh/runtime/sftp_backend.rs` using `russh_sftp::client::SftpSession::open`, `open_with_flags`, and file handle `AsyncSeek` support.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test sftp_runtime_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/runtime.rs src/app/ssh/runtime/sftp_backend.rs src/app/ssh/session_manager.rs tests/sftp_runtime_spec.rs
git commit -m "feat: add streamable sftp runtime contracts"
```

### Task 4: Implement resumable chunked download execution

**Files:**
- Create: `src/app/sftp/transfer_engine.rs`
- Modify: `src/app/sftp/mod.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `tests/sftp_transfer_flow_spec.rs`

**Step 1: Write the failing test**

Add download-resume tests in `tests/sftp_transfer_flow_spec.rs`:

```rust
#[tokio::test]
async fn interrupted_download_resumes_from_local_part_file() {
    let root = temp_test_root("resume-download");
    let queue = seeded_download_queue(root.join("archive.zip.part"), 3);
    let runtime = resumable_runtime_with_remote_bytes(b"abcdefghi");

    execute_queued_transfers_with_progress(&runtime, &mut queue, |_| {})
        .await
        .expect("resume download");

    assert_eq!(fs::read(root.join("archive.zip")).unwrap(), b"abcdefghi");
    assert_eq!(queue.task("download-1").unwrap().state, TransferTaskState::Completed);
}

#[tokio::test]
async fn download_resume_falls_back_to_restart_when_remote_shrinks() {
    // Expect Failed or RestartRequired projection, not silent corruption.
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_transfer_flow_spec -q
```

Expected:

- FAIL because downloads still read whole remote files and write final targets directly.

**Step 3: Write minimal implementation**

Create a narrow download engine in `src/app/sftp/transfer_engine.rs`:

```rust
pub async fn execute_download_task<F>(
    runtime: &SftpRuntimeHandle,
    task: &mut TransferTask,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(&TransferTask),
{
    let remote_meta = runtime.stat(&task.source_path).await?;
    let local_part = download_part_path(task.target_path.as_str());
    let local_offset = existing_file_len(&local_part)?;
    validate_download_resume(task, local_offset, &remote_meta)?;

    let mut reader = runtime.open_file_reader(&task.source_path).await?;
    let mut writer = open_local_part_writer(&local_part)?;
    reader.seek(SeekFrom::Start(local_offset)).await?;
    writer.seek(SeekFrom::Start(local_offset))?;

    copy_in_chunks(&mut reader, &mut writer, task, on_progress).await?;
    fs::rename(&local_part, &task.target_path)?;
    Ok(())
}
```

Then update `execute_transfer(...)` in `src/app/sftp/session_binding.rs` so `TransferTaskAction::Download` delegates to the new chunked engine rather than `runtime.download_file(...)`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test sftp_transfer_flow_spec -q
```

Expected: PASS, including existing download tests and the new resume test.

**Step 5: Commit**

```bash
git add src/app/sftp/transfer_engine.rs src/app/sftp/mod.rs src/app/sftp/session_binding.rs src/app/sftp/queue.rs tests/sftp_transfer_flow_spec.rs
git commit -m "feat: add resumable chunked sftp downloads"
```

### Task 5: Implement resumable chunked upload execution

**Files:**
- Modify: `src/app/sftp/transfer_engine.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `tests/sftp_transfer_flow_spec.rs`

**Step 1: Write the failing test**

Add upload-resume tests:

```rust
#[tokio::test]
async fn interrupted_upload_resumes_from_remote_part_file() {
    let local_path = write_local_source(b"abcdefghi");
    let runtime = resumable_runtime_with_remote_part("/srv/app/archive.zip.part", b"abc");
    let mut queue = seeded_upload_queue(local_path, 3);

    execute_queued_transfers_with_progress(&runtime, &mut queue, |_| {})
        .await
        .expect("resume upload");

    assert_remote_file_eq(&runtime, "/srv/app/archive.zip", b"abcdefghi");
}

#[tokio::test]
async fn upload_resume_requires_restart_when_local_source_changes() {
    // Change source mtime/len and expect resume rejection.
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_transfer_flow_spec -q
```

Expected:

- FAIL because uploads still read the entire local file into memory and write the final remote target directly.

**Step 3: Write minimal implementation**

Extend `src/app/sftp/transfer_engine.rs` with upload flow:

```rust
pub async fn execute_upload_task<F>(
    runtime: &SftpRuntimeHandle,
    task: &mut TransferTask,
    on_progress: &mut F,
) -> Result<()>
where
    F: FnMut(&TransferTask),
{
    let local_meta = fs::metadata(task.local_source_path()?)?;
    validate_upload_resume(task, &local_meta)?;

    let remote_part = upload_part_path(task.target_path.as_str());
    let remote_offset = remote_part_len(runtime, &remote_part).await?;

    let mut local_reader = tokio::fs::File::open(task.local_source_path()?).await?;
    let mut remote_writer = runtime
        .open_file_writer(&remote_part, SftpWriteMode::CreateOrAppend)
        .await?;

    local_reader.seek(SeekFrom::Start(remote_offset)).await?;
    remote_writer.seek(SeekFrom::Start(remote_offset)).await?;
    copy_in_chunks(&mut local_reader, &mut remote_writer, task, on_progress).await?;
    runtime.move_entry(&remote_part, &task.target_path).await?;
    Ok(())
}
```

Route `TransferTaskAction::Upload` through this new helper.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test sftp_transfer_flow_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/transfer_engine.rs src/app/sftp/session_binding.rs src/app/sftp/queue.rs tests/sftp_transfer_flow_spec.rs
git commit -m "feat: add resumable chunked sftp uploads"
```

### Task 6: Add pause, interrupt, and restart semantics to the queue and view model

**Files:**
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/sftp_queue_spec.rs`

**Step 1: Write the failing test**

Add queue/view-model tests:

```rust
#[test]
fn paused_task_projects_resume_action_in_transfer_center() {
    let mut vm = ShellViewModel::default();
    vm.sftp_transfer_tasks = vec![sample_paused_download_task()];

    let item = project_transfer_center_items(&vm).remove(0);
    assert!(item.can_retry);
    assert_eq!(item.status_label.as_str(), "Paused");
}

#[test]
fn interrupted_task_requires_resume_or_restart_not_conflict_resolution() {
    let task = sample_interrupted_upload_task();
    assert_eq!(task.state, TransferTaskState::Interrupted);
}
```

Add a queue unit test that `pause_task()` preserves `bytes_confirmed` and `restart_task()` resets it to zero and clears the `.part` path reference.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model --test sftp_queue_spec -q
```

Expected:

- FAIL because pause/restart/resume-related helpers and projections do not exist yet.

**Step 3: Write minimal implementation**

In `src/app/sftp/queue.rs`, add focused state helpers:

```rust
pub fn pause_task(&mut self, task_id: &str) -> bool { /* set Paused */ }
pub fn interrupt_task(&mut self, task_id: &str, message: impl Into<String>) -> bool { /* set Interrupted */ }
pub fn restart_task(&mut self, task_id: &str) -> bool {
    // reset bytes_confirmed, bytes_transferred, error, state, and resume-specific fields
}
```

Update `src/shell/view_model.rs` / `src/shell/view_model/sftp.rs` so transfer-center rows map:

- `Paused` -> `Resume`
- `Interrupted` -> `Resume`
- `RestartOnly` capability -> `Restart`

Do not wire bootstrap callbacks yet; only project the new state honestly.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test shell_view_model --test sftp_queue_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/queue.rs src/shell/view_model.rs src/shell/view_model/sftp.rs tests/shell_view_model.rs tests/sftp_queue_spec.rs
git commit -m "feat: add transfer pause interrupt restart semantics"
```

### Task 7: Persist task updates and recover them during bootstrap

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/sftp/transfer_store.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add bootstrap tests that prove persisted tasks rehydrate into the transfer center:

```rust
#[test]
fn bootstrap_loads_interrupted_transfer_tasks_from_store() {
    let harness = bootstrap_with_persisted_transfer(sample_interrupted_download_task());
    assert_eq!(harness.app.get_transfer_queue_total(), 1);
    assert!(harness.app.get_transfer_center_items().row_data(0).unwrap().can_retry);
}

#[test]
fn bootstrap_marks_invalid_resume_tasks_as_restart_required() {
    let harness = bootstrap_with_invalid_persisted_transfer();
    let row = harness.app.get_transfer_center_items().row_data(0).unwrap();
    assert!(row.status_label.contains("Restart"));
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke -q
```

Expected:

- FAIL because bootstrap never loads transfer records from disk and never validates restart-vs-resume capability.

**Step 3: Write minimal implementation**

Wire a `RedbTransferStore` into bootstrap startup and SFTP background updates:

```rust
let transfer_store = RedbTransferStore::new(app_paths.data_dir.clone());
let persisted_tasks = transfer_store.load_tasks()?;
state.replace_transfer_tasks(persisted_tasks);
```

On every task mutation that matters (`Queued`, `Running`, `Paused`, `Interrupted`, `Completed`, `Failed`), save the latest task snapshot back to the store on a background-safe path. Add a recovery pass that validates persisted tasks and downgrades invalid resume cases to `RestartOnly`.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test bootstrap_smoke -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/sftp.rs src/app/sftp/transfer_store.rs src/shell/view_model.rs tests/bootstrap_smoke.rs
git commit -m "feat: restore persisted sftp transfers during bootstrap"
```

### Task 8: Wire transfer-center callbacks for resume, restart, and pause

**Files:**
- Modify: `ui/shell/transfer-center.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `tests/transfer_center_smoke.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing test**

Add UI contract tests:

```rust
#[test]
fn transfer_center_exposes_resume_restart_and_pause_callbacks() {
    let content = fs::read_to_string("ui/shell/transfer-center.slint").unwrap();
    assert!(content.contains("callback transfer-row-resume-requested(string);"));
    assert!(content.contains("callback transfer-row-restart-requested(string);"));
    assert!(content.contains("callback transfer-row-pause-requested(string);"));
}

#[test]
fn interrupted_transfer_rows_use_resume_copy_instead_of_retry_copy() {
    // Assert projection contract and row labels.
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test transfer_center_smoke --test top_status_bar_smoke -q
```

Expected:

- FAIL because the UI still assumes `Retry`/`Resolve`/`Open in SFTP Workspace` are the only attention actions.

**Step 3: Write minimal implementation**

Update the Slint contract so the row callback surface matches the new states:

```slint
callback transfer-row-resume-requested(string);
callback transfer-row-restart-requested(string);
callback transfer-row-pause-requested(string);
```

Then project state-dependent copy from `src/app/bootstrap/shell_chrome.rs`:

- `Running` => button label `Pause`
- `Paused` / `Interrupted` => button label `Resume`
- `RestartOnly` => button label `Restart`

Finally wire the callbacks in `src/app/bootstrap.rs` to queue/view-model operations and background execution scheduling.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test transfer_center_smoke --test top_status_bar_smoke -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/transfer-center.slint ui/app-window.slint src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs tests/transfer_center_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "feat: add transfer center resume restart pause actions"
```

### Task 9: Focused verification and cleanup

**Files:**
- Verify only

**Step 1: Run focused transfer verification**

Run:

```bash
cargo test --test sftp_resume_domain_spec --test sftp_transfer_store_spec --test sftp_runtime_spec --test sftp_transfer_flow_spec --test sftp_queue_spec --test shell_view_model --test bootstrap_smoke --test transfer_center_smoke --test top_status_bar_smoke -q
```

Expected: PASS

**Step 2: Run compile verification**

Run:

```bash
cargo check -q
```

Expected: PASS

**Step 3: Re-read the design doc and compare behavior**

Check `docs/plans/2026-04-20-sftp-resumable-transfer-design.md` and confirm the implementation actually satisfies:

- persisted transfer store
- `.part` download/upload strategy
- resume vs restart semantics
- chunked transfers instead of whole-file memory buffering
- startup recovery behavior

**Step 4: Commit the final verification checkpoint**

```bash
git add -A
git commit -m "test: verify resumable sftp transfer flow"
```

**Step 5: Request review before merge**

Use `superpowers:requesting-code-review`, then resolve any findings before merging.
