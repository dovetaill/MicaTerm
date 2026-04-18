# SFTP Download Behavior Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Update SFTP downloads so completed items open with native desktop semantics, downloaded artifacts move to Trash on remove, and download conflicts support overwrite, auto rename, and cancel-current with a persisted default strategy.

**Architecture:** Extend the existing transfer-queue and transfer-center pipeline instead of replacing it. Keep remote-conflict handling intact, but add a download-specific conflict preference and execution path, then layer the new settings/modal behavior on top of the current Slint + view-model projection architecture.

**Tech Stack:** Rust, Slint, existing SFTP queue/session-binding code, platform shell helpers, UI preference persistence, Rust test suite (`cargo test`) and existing shell-based UI contract smoke tests.

---

### Task 1: Persist the download conflict default in settings

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `ui/components/settings-modal.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/ui_preferences.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add tests that expect a persisted `download_conflict_default` field and a settings-modal projection for:

```rust
assert_eq!(prefs.download_conflict_default, DownloadConflictDefault::Ask);
assert_eq!(app.get_settings_modal_download_conflict_default(), "ask");
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test ui_preferences_defaults_to_ask_for_download_conflicts -- --exact
cargo test settings_modal_download_conflict_preference_updates_window_state_and_persist -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected: failures because the new preference field, callbacks, and UI contract do not exist yet.

**Step 3: Write the minimal implementation**

Introduce a small enum/string-backed setting and wire it through the existing settings modal state:

```rust
pub enum DownloadConflictDefault {
    Ask,
    Overwrite,
    AutoRename,
}
```

Persist it in `UiPreferences`, expose getter/setter methods on `ShellViewModel`, bind it in `shell_chrome`, and add a single-choice control in `settings-modal.slint` / `app-window.slint`.

**Step 4: Run tests to verify they pass**

Run:
```bash
cargo test ui_preferences_defaults_to_ask_for_download_conflicts -- --exact
cargo test settings_modal_download_conflict_preference_updates_window_state_and_persist -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/ui_preferences.rs src/shell/view_model.rs src/shell/view_model/projection.rs src/app/bootstrap/shell_chrome.rs ui/components/settings-modal.slint ui/app-window.slint tests/ui_preferences.rs tests/vault_settings_smoke.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: persist sftp download conflict defaults"
```

### Task 2: Add download auto-rename resolution and cancel-current behavior

**Files:**
- Modify: `src/app/sftp/local_ops.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Test: `tests/sftp_queue_spec.rs`
- Test: `tests/sftp_transfer_flow_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests that describe:
- file auto rename (`report.txt` -> `report (1).txt`)
- directory-root auto rename cascading to nested paths
- cancel-current affecting only the current conflicting download task

Example expectations:

```rust
assert_eq!(resolved_path, root.join("report (1).txt"));
assert_eq!(renamed_child.local_path, root.join("logs (1)").join("app.log"));
assert_eq!(cancelled_task.state, TransferTaskState::Cancelled);
assert_eq!(later_conflict_task.state, TransferTaskState::Conflict);
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test download_auto_rename_uses_numeric_suffixes -- --exact
cargo test directory_download_auto_rename_rewrites_nested_targets -- --exact
cargo test download_conflict_cancel_only_cancels_current_task -- --exact
```

Expected: FAIL because the queue only knows overwrite/skip and does not rewrite download targets.

**Step 3: Write the minimal implementation**

Add local-path conflict helpers and a download-specific resolution path:

```rust
fn next_auto_rename_path(path: &Path) -> PathBuf { /* report -> report (1) */ }
fn rewrite_download_subtree_root(tasks: &mut [TransferTask], old_root: &Path, new_root: &Path)
```

Then extend the queue/session-binding/bootstrap flow so download conflicts can resolve to:
- overwrite
- auto rename
- cancel current

without changing the existing non-download remote-conflict behavior.

**Step 4: Run tests to verify they pass**

Run:
```bash
cargo test download_auto_rename_uses_numeric_suffixes -- --exact
cargo test directory_download_auto_rename_rewrites_nested_targets -- --exact
cargo test download_conflict_cancel_only_cancels_current_task -- --exact
cargo test conflict_task_can_resume_and_complete_with_selected_policy -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/sftp/local_ops.rs src/app/sftp/queue.rs src/app/sftp/session_binding.rs src/app/bootstrap/sftp.rs src/shell/view_model/sftp.rs tests/sftp_queue_spec.rs tests/sftp_transfer_flow_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add download auto-rename conflict handling"
```

### Task 3: Project the new download conflict modal behavior

**Files:**
- Modify: `ui/components/sftp-conflict-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Test: `tests/transfer_center_smoke.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add UI contract and flow tests that expect download conflicts to expose:
- `Overwrite`
- `Auto Rename`
- `Cancel Download`

and to keep “apply to batch” inactive when cancelling.

Example assertions:

```rust
assert!(component.contains("Auto Rename"));
assert!(component.contains("Cancel Download"));
assert!(!app.get_sftp_conflict_modal_apply_to_batch());
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test transfer_center_conflict_modal_exposes_download_actions -- --exact
cargo test transfer_center_conflict_modal_cancel_keeps_batch_toggle_inactive -- --exact
```

Expected: FAIL because the modal still only exposes replace/skip semantics.

**Step 3: Write the minimal implementation**

Project conflict actions from task type/state instead of hard-coding one modal mode:

```rust
enum SftpConflictModalKind {
    Download,
    Remote,
}
```

Wire new callbacks through `app-window.slint` and `bootstrap/sftp.rs`, and keep the existing replace/skip route for non-download conflicts.

**Step 4: Run tests to verify they pass**

Run:
```bash
cargo test transfer_center_conflict_modal_exposes_download_actions -- --exact
cargo test transfer_center_conflict_modal_cancel_keeps_batch_toggle_inactive -- --exact
cargo test transfer_center_conflict_rows_can_open_resolve_modal_and_replace -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add ui/components/sftp-conflict-modal.slint ui/app-window.slint src/shell/view_model.rs src/shell/view_model/sftp.rs src/app/bootstrap/sftp.rs tests/transfer_center_smoke.rs tests/bootstrap_smoke.rs
git commit -m "feat: tailor sftp conflict modal for downloads"
```

### Task 4: Use native platform semantics for open-file and reveal-folder actions

**Files:**
- Modify: `src/app/sftp/local_open.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests that assert the platform helper source now includes:
- shell-open behavior for files
- reveal/select behavior for file downloads
- directory-open behavior for downloaded folders

Example source assertions:

```rust
assert!(local_open_source.contains("org.freedesktop.FileManager1"));
assert!(local_open_source.contains("ShowItems"));
assert!(local_open_source.contains("shell-open"));
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test transfer_center_open_actions_use_native_shell_and_reveal_helpers -- --exact
```

Expected: FAIL because Linux currently only falls back to `xdg-open` on the directory path and Windows uses direct command spawning.

**Step 3: Write the minimal implementation**

Refactor `local_open.rs` into clearer helpers:

```rust
pub fn open_path_locally(path: &Path) -> Result<()>;
pub fn reveal_path_locally(path: &Path) -> Result<()>;
```

Implement:
- Windows: shell open + Explorer select
- macOS: `open` + `open -R`
- Linux: `xdg-open` for files/directories, `FileManager1.ShowItems` first for file reveal, then directory fallback

**Step 4: Run tests to verify they pass**

Run:
```bash
cargo test transfer_center_open_actions_use_native_shell_and_reveal_helpers -- --exact
cargo test transfer_center_open_actions_route_through_platform_helpers_and_visible_feedback -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/sftp/local_open.rs src/app/bootstrap/sftp.rs src/app/bootstrap/shell_chrome.rs tests/bootstrap_smoke.rs
git commit -m "feat: use native shell semantics for download open actions"
```

### Task 5: Move downloaded artifacts to Trash before removing transfer rows

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/sftp/local_open.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/transfer_center_smoke.rs`

**Step 1: Write the failing tests**

Add tests that expect completed download rows to distinguish “remove record only” from “trash downloaded artifact and remove record”, plus a flow test for missing local artifacts.

Example expectations:

```rust
assert!(row.can_remove);
assert!(tooltip.contains("Trash"));
assert!(feedback.contains("local file already missing"));
```

**Step 2: Run tests to verify they fail**

Run:
```bash
cargo test transfer_center_completed_download_remove_uses_trash_semantics -- --exact
cargo test transfer_center_remove_missing_download_only_clears_record -- --exact
```

Expected: FAIL because `remove_transfer_task` currently only drops the row from memory.

**Step 3: Write the minimal implementation**

Choose one path and keep it explicit in code:

```rust
pub fn trash_path_locally(path: &Path) -> Result<()>;
```

If a lightweight dependency is needed, add it in `Cargo.toml`; otherwise keep platform-specific trash helpers in `local_open.rs`. In the remove callback:
- completed download with existing local artifact -> trash it, then remove the row
- completed download with missing artifact -> remove the row and show mild feedback
- all other tasks -> remove the row only

**Step 4: Run tests to verify they pass**

Run:
```bash
cargo test transfer_center_completed_download_remove_uses_trash_semantics -- --exact
cargo test transfer_center_remove_missing_download_only_clears_record -- --exact
cargo test transfer_center_contract_includes_completed_file_actions -- --exact
```

Expected: PASS.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/sftp/local_open.rs src/shell/view_model/projection.rs src/app/bootstrap/sftp.rs tests/bootstrap_smoke.rs tests/transfer_center_smoke.rs
git commit -m "feat: trash downloaded items from transfer center"
```

### Task 6: Run focused regression coverage and final verification

**Files:**
- Reference: `tests/ui_preferences.rs`
- Reference: `tests/sftp_queue_spec.rs`
- Reference: `tests/sftp_transfer_flow_spec.rs`
- Reference: `tests/transfer_center_smoke.rs`
- Reference: `tests/bootstrap_smoke.rs`
- Reference: `tests/vault_settings_smoke.rs`
- Reference: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Run the focused Rust test suite**

Run:
```bash
cargo test ui_preferences -- --nocapture
cargo test sftp_queue_spec -- --nocapture
cargo test sftp_transfer_flow_spec -- --nocapture
cargo test transfer_center -- --nocapture
cargo test bootstrap_smoke -- --nocapture
cargo test vault_settings_smoke -- --nocapture
```

Expected: PASS with the updated download/open/remove/conflict behavior.

**Step 2: Run the shell UI contract smoke test**

Run:
```bash
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected: PASS.

**Step 3: Run `cargo fmt` if needed**

Run:
```bash
cargo fmt --all
```

Expected: no diff or formatting-only diff.

**Step 4: Re-run the most sensitive tests after formatting**

Run:
```bash
cargo test sftp_transfer_flow_spec -- --nocapture
cargo test bootstrap_smoke -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add docs/plans/2026-04-18-sftp-download-behavior-design.md docs/plans/2026-04-18-sftp-download-behavior.md
git commit -m "docs: add sftp download behavior design and plan"
```
