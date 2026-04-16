# Async SFTP Browser And Transfer Center Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove UI-thread SFTP blocking, replace fake SFTP actions with honest asynchronous behavior, and rebuild the transfer center into an actionable task hub.

**Architecture:** Keep the existing `SftpBrowserController` and session-scoped browser state, but route all user-facing SFTP work through a unified async operation dispatcher. Reuse the real runtime/session-binding methods for mkdir/rename/delete/upload/download, add generation-aware result handling for browser and file-operation tasks, and project richer transfer/task state into the UI so the quick browser stays lightweight while the transfer center owns long-running work and recovery actions.

**Tech Stack:** Rust, Slint, Tokio runtime handles, existing `SessionManager` / `SftpRuntime` / transfer-queue model, cargo test, cargo build

---

### Task 1: Lock the new async/Open/Edit UI contract with failing tests

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/sftp_context_menu_spec.rs`
- Modify: `tests/transfer_center_smoke.rs`
- Modify: `tests/top_status_bar_smoke.rs`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/shell/transfer-center.slint`
- Modify: `ui/shell/titlebar.slint`

**Step 1: Write the failing tests**

Add or extend tests to prove the intended contract before implementation:

```rust
#[test]
fn sftp_ready_state_exposes_blank_area_context_menu_hook() {
    let source = fs::read_to_string("ui/shell/right-panel.slint").unwrap();
    assert!(source.contains("sftp-panel-context-menu-requested(\n                                \"\",\n                                \"sftp-blank\""));
}

#[test]
fn transfer_center_contract_includes_completed_file_actions() {
    let source = fs::read_to_string("ui/shell/transfer-center.slint").unwrap();
    assert!(source.contains("callback open-file-requested(string);"));
    assert!(source.contains("callback open-folder-requested(string);"));
    assert!(source.contains("callback remove-requested(string);"));
    assert!(source.contains("callback clear-completed-requested();"));
}

#[test]
fn titlebar_no_longer_renders_numeric_transfer_badge() {
    let source = fs::read_to_string("ui/shell/titlebar.slint").unwrap();
    assert!(!source.contains("transfer-badge := Rectangle"));
}
```

Also extend bootstrap coverage so a completed download row is expected to surface `Open File`, `Open Containing Folder`, and `Remove`, while failed rows still surface `Retry` and `Show Error`.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --test sftp_context_menu_spec --test transfer_center_smoke --test top_status_bar_smoke --test bootstrap_smoke -q`

Expected: FAIL because the ready-state blank-area hook, completed-row actions, and titlebar badge removal are not implemented yet.

**Step 3: Write the minimal UI contract changes**

Make the smallest source-level updates needed so the contract exists but behavior can still fail:

```slint
callback open-file-requested(string);
callback open-folder-requested(string);
callback remove-requested(string);
callback clear-completed-requested();
```

Add a ready-state blank-area `TouchArea` or equivalent host-level pointer handler in `right-panel.slint`, and remove the raw numeric badge block from `titlebar.slint`.

**Step 4: Run the tests to verify they now fail for behavior, not missing contract**

Run: `cargo test --test sftp_context_menu_spec --test transfer_center_smoke --test top_status_bar_smoke --test bootstrap_smoke -q`

Expected: FAIL on missing Rust/bootstrap behavior, not on missing Slint callbacks/markup.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/sftp_context_menu_spec.rs tests/transfer_center_smoke.rs tests/top_status_bar_smoke.rs ui/shell/right-panel.slint ui/shell/transfer-center.slint ui/shell/titlebar.slint
git commit -m "test: lock async sftp browser ui contract"
```

### Task 2: Introduce a unified async SFTP operation dispatcher and generation-aware result model

**Files:**
- Create: `src/app/sftp/operation_dispatch.rs`
- Modify: `src/app/sftp/mod.rs`
- Modify: `src/app/sftp/browser_state.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add focused tests for the dispatcher/generation behavior:

```rust
#[test]
fn stale_sftp_browser_results_do_not_override_newer_navigation() {
    let mut controller = SftpBrowserController::default();
    let first = controller.open(session_id, "/etc");
    let second = controller.navigate(session_id, "/var/log");
    controller.apply_loaded_directory_for_browser_session(first.file_browser_session_id.as_str(), first.request_id, "/etc", vec![]);
    assert_eq!(controller.browser_session_state(first.file_browser_session_id.as_str()).unwrap().current_path, "/var/log");
}
```

Add bootstrap coverage that follow-mode refresh only requeues on real path/session changes instead of every projection tick.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --test sftp_follow_cwd_spec --test bootstrap_smoke -q`

Expected: FAIL because the current browser/result model is still too narrow for all operation types and follow refresh still depends on heavier projection churn.

**Step 3: Write the minimal implementation**

Create an operation module with typed requests/results and a shared token model:

```rust
pub enum SftpOperationKind {
    LoadDir,
    DownloadAndOpen,
    PrepareEditWorkingCopy,
    UploadWorkingCopy,
    RenameEntry,
    DeleteEntries,
    CreateFolder,
}

pub struct SftpOperationToken {
    pub browser_session_id: String,
    pub generation: u64,
    pub operation_id: u64,
}
```

- Extend browser state with explicit stale/loading metadata and stronger generation acceptance checks.
- In `bootstrap/sftp.rs`, route directory work and non-directory work through the dispatcher instead of directly calling synchronous manager wrappers from UI callbacks.
- Keep `SessionManager` async methods as the real runtime boundary; stop using synchronous `block_on(...)` wrappers from user-facing SFTP UI paths.

**Step 4: Run the tests to verify they pass**

Run: `cargo test --test sftp_follow_cwd_spec --test bootstrap_smoke -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/sftp/operation_dispatch.rs src/app/sftp/mod.rs src/app/sftp/browser_state.rs src/app/bootstrap/sftp.rs src/app/bootstrap.rs src/app/ssh/session_manager.rs tests/sftp_follow_cwd_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add async sftp operation dispatcher"
```

### Task 3: Replace synchronous remote open/save with `Open` and `Edit Locally` working-copy flows

**Files:**
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Create: `src/app/sftp/working_copy.rs`
- Create: `src/app/sftp/local_open.rs`
- Modify: `ui/app-window.slint`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sftp_context_menu_spec.rs`

**Step 1: Write the failing tests**

Add bootstrap tests that prove:

```rust
#[test]
fn open_remote_file_queues_background_download_instead_of_modal_editor() {}

#[test]
fn edit_locally_tracks_working_copy_and_queues_async_upload_on_save() {}
```

Add source/UI tests that the file context menu exposes `Open` and `Edit Locally` distinctly, and that `Open` no longer depends on the old remote-file modal to function.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --test bootstrap_smoke --test sftp_context_menu_spec -q`

Expected: FAIL because `Open` still hits the synchronous download path and the old remote-file modal save path still uploads synchronously.

**Step 3: Write the minimal implementation**

Implement a working-copy model and local-open helper:

```rust
pub struct SftpWorkingCopy {
    pub task_id: String,
    pub session_id: Uuid,
    pub remote_path: String,
    pub local_path: PathBuf,
    pub upload_on_save: bool,
}

pub enum SftpOpenAction {
    DownloadAndOpen,
    EditLocally,
}
```

- `Open` queues a background download, persists to temp/cache, then invokes the platform opener asynchronously.
- `Edit Locally` queues a background download into a managed working-copy path and registers follow-up upload-back behavior.
- Remove `manager.sftp_download_file(...)` and `manager.sftp_upload_file(...)` from direct UI callbacks.
- If the old remote-file modal remains temporarily, mark it as non-default/legacy and stop routing default `Open` through it.

**Step 4: Run the tests to verify they pass**

Run: `cargo test --test bootstrap_smoke --test sftp_context_menu_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap/sftp.rs src/app/bootstrap/assets_keychain.rs src/shell/view_model/context_menu_dispatcher.rs src/shell/view_model/sftp.rs src/app/sftp/working_copy.rs src/app/sftp/local_open.rs ui/app-window.slint tests/bootstrap_smoke.rs tests/sftp_context_menu_spec.rs
git commit -m "feat: split sftp open and edit locally flows"
```

### Task 4: Make quick-browser mutations real and fix ready-state blank-area menus

**Files:**
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/asset_modal_executor.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Test: `tests/sftp_context_menu_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests that prove `Rename`, `Delete`, and `New Folder` stop being local-only mutations:

```rust
#[test]
fn sftp_new_folder_dispatches_backend_mkdir_instead_of_local_push() {}

#[test]
fn sftp_delete_dispatches_backend_remove_and_requires_confirmation() {}

#[test]
fn unsupported_sftp_actions_render_disabled_reasons() {}
```

Add a ready-state blank-area right-click test in `sftp_context_menu_spec.rs` to catch regressions.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --test sftp_context_menu_spec --test bootstrap_smoke -q`

Expected: FAIL because the current implementation still mutates in-memory entry lists for several operations and the ready-state blank area still lacks a context trigger.

**Step 3: Write the minimal implementation**

- Replace local-only `entries.push(...)`, `entries.retain(...)`, and rename-only view-model edits with dispatcher-backed operations that call real runtime methods.
- Keep optimistic UI only if the task is tied to a real backend operation and can be rolled back on failure.
- Add disabled-reason strings for unsupported actions (`Copy`, `Cut`, `Paste`, `Permissions...`) instead of fake enabled entries.
- Add the ready-state blank-area context handler to the list host so the same menu appears in both empty and populated states.

Representative backend dispatch shape:

```rust
match action {
    PendingSftpContextAction::RenameEntry { from, to } => dispatch_rename(...),
    PendingSftpContextAction::DeleteEntries { entries } => dispatch_delete(...),
    PendingSftpContextAction::CreateFolder { path } => dispatch_mkdir(...),
    _ => {}
}
```

**Step 4: Run the tests to verify they pass**

Run: `cargo test --test sftp_context_menu_spec --test bootstrap_smoke -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap/sftp.rs src/shell/view_model/assets.rs src/shell/view_model/asset_modal_executor.rs src/shell/view_model/context_menu_dispatcher.rs src/shell/view_model/sftp.rs ui/shell/right-panel.slint src/app/bootstrap/assets_keychain.rs tests/sftp_context_menu_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: wire real async sftp browser actions"
```

### Task 5: Rebuild the transfer center row model and replace the titlebar badge with semantic summary

**Files:**
- Modify: `ui/shell/transfer-center.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Test: `tests/transfer_center_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests for the richer row/action model:

```rust
#[test]
fn completed_transfer_rows_expose_open_file_open_folder_and_remove() {}

#[test]
fn failed_transfer_rows_expose_retry_show_error_and_remove() {}

#[test]
fn titlebar_shows_semantic_transfer_summary_without_numeric_badge() {}
```

Add filter tests that `Failed` includes both failed and conflict rows, and `Clear Completed` only removes completed rows.

**Step 2: Run the tests to verify they fail**

Run: `cargo test --test transfer_center_smoke --test top_status_bar_smoke --test bootstrap_smoke -q`

Expected: FAIL because the current row contract still centers on `Retry / Resolve / Workspace`, the layout is still table-like, and the titlebar still renders the raw count badge.

**Step 3: Write the minimal implementation**

Extend `TransferCenterItem` and bootstrap projection:

```rust
pub struct TransferCenterItem {
    pub can_open_file: bool,
    pub can_open_folder: bool,
    pub can_remove: bool,
    pub can_clear_completed: bool,
    pub can_show_error: bool,
}
```

- Change the Slint layout from wide table columns to compact rows with primary/secondary text, status badge, progress summary, and inline actions.
- Project completed download tasks with local-path metadata so `Open File` and `Open Containing Folder` can work.
- Add toolbar wiring for `Clear Completed`.
- Replace the titlebar badge with semantic summary text or no summary when the count is not meaningful.

**Step 4: Run the tests to verify they pass**

Run: `cargo test --test transfer_center_smoke --test top_status_bar_smoke --test bootstrap_smoke -q`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/transfer-center.slint ui/shell/titlebar.slint ui/app-window.slint src/app/bootstrap/shell_chrome.rs src/app/bootstrap.rs src/shell/view_model/sftp.rs src/app/bootstrap/sftp.rs tests/transfer_center_smoke.rs tests/top_status_bar_smoke.rs tests/bootstrap_smoke.rs
git commit -m "feat: rebuild sftp transfer center actions"
```

### Task 6: Focused verification and cleanup

**Files:**
- Verify only

**Step 1: Run the focused regression suite**

Run:
`cargo test --test sftp_context_menu_spec --test sftp_follow_cwd_spec --test transfer_center_smoke --test sftp_transfer_flow_spec --test bootstrap_smoke --test top_status_bar_smoke -q`

Expected: PASS

**Step 2: Run build verification**

Run: `cargo build -q`

Expected: PASS

**Step 3: Re-read the requirements and spot-check the shipped behavior**

Confirm all of the following are true:
- No user-facing SFTP UI path calls synchronous `block_on(...)` wrappers.
- `Open` means download-and-open locally.
- `Edit Locally` owns upload-back behavior asynchronously.
- `Rename`, `Delete`, and `New Folder` are real or explicitly disabled.
- Ready-state blank-area context menus work.
- Transfer-center completed rows expose `Open File`, `Open Containing Folder`, and `Remove`.
- The raw numeric titlebar badge is gone.

**Step 4: Commit final cleanup if needed**

```bash
git add -A
git commit -m "test: verify async sftp browser redesign"
```
