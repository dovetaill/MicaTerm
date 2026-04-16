# SFTP Browser Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Restore broken SFTP quick-browser interactions, make refresh and new-file actions real, preserve folder-upload semantics, and let the transfer center close on outside click.

**Architecture:** Keep the existing async SFTP browser/controller flow and repair the broken edges instead of replacing it. UI interaction fixes stay in Slint and bootstrap wiring, while real behavior fixes land in the SFTP queue/domain layer so toolbar actions, context-menu actions, drag-and-drop, and transfer-center refreshes all share the same backend paths.

**Tech Stack:** Rust, Slint, Tokio, `russh-sftp`, `cargo test`

---

### Task 1: Lock the regressions with failing tests

**Files:**
- Modify: `tests/sftp_right_panel_render_spec.rs`
- Modify: `tests/sftp_context_menu_spec.rs`
- Modify: `tests/sftp_transfer_flow_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add or update tests that assert:
- the compact quick-browser source uses a named metadata text row that matches the actual two-line layout contract
- SFTP context refresh dispatches through a real refresh path, not only local loading state
- empty remote files can be created through the SFTP modal confirmation flow
- folder upload preserves the selected folder root and empty directories
- transfer center outside-click dismissal is exposed in app-window source or smoke coverage

**Step 2: Run the targeted tests to verify they fail**

Run:
```bash
cargo test --test sftp_right_panel_render_spec --test sftp_context_menu_spec --test sftp_transfer_flow_spec --test bootstrap_smoke -q
```
Expected: FAIL on the newly added assertions.

**Step 3: Do not change production code yet**

Confirm the failure messages point to the intended missing behavior and not typos in the tests.

**Step 4: Commit the red test snapshot**

```bash
git add tests/sftp_right_panel_render_spec.rs tests/sftp_context_menu_spec.rs tests/sftp_transfer_flow_spec.rs tests/bootstrap_smoke.rs
git commit -m "test: lock sftp browser regressions"
```

### Task 2: Repair quick-browser hit testing and refresh behavior

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Use a source-contract test or smoke assertion that proves:
- row click/double-click handlers are not covered by a full-list blank overlay
- the compact row includes a named metadata text block
- context refresh triggers a real queued refresh path

**Step 2: Run just those tests to verify they fail**

Run:
```bash
cargo test --test sftp_right_panel_render_spec --test bootstrap_smoke -q
```
Expected: FAIL on list-hit-testing / metadata-row / refresh assertions.

**Step 3: Write the minimal implementation**

- rename the second-line metadata `Text` node in `ui/shell/right-panel.slint` to match the contract (`meta-text := Text`)
- remove or constrain the ready-state blank-area `TouchArea` so it does not cover list rows
- change the loading copy/headline so cached refreshes do not surface `Refreshing remote directory`
- wire `refresh-sftp` through the same bootstrap/controller refresh path used by the toolbar button

**Step 4: Run the focused tests to verify they pass**

Run:
```bash
cargo test --test sftp_right_panel_render_spec --test bootstrap_smoke -q
```
Expected: PASS.

**Step 5: Commit**

```bash
git add ui/shell/right-panel.slint src/app/bootstrap/sftp.rs src/shell/view_model/context_menu_dispatcher.rs src/shell/view_model/sftp.rs tests/sftp_right_panel_render_spec.rs tests/bootstrap_smoke.rs
git commit -m "fix: restore sftp browser hit testing and refresh wiring"
```

### Task 3: Implement real SFTP new-file creation

**Files:**
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model/asset_modal_executor.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/assets.rs` (if modal entry points need adjustment)
- Test: `tests/sftp_context_menu_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add tests that assert:
- `new-file` opens the SFTP create modal instead of falling through to a placeholder path
- confirming the modal queues a real async empty-file creation and refreshes the directory

**Step 2: Run the tests to verify they fail**

Run:
```bash
cargo test --test sftp_context_menu_spec --test bootstrap_smoke -q
```
Expected: FAIL because `new-file` is not implemented.

**Step 3: Write the minimal implementation**

- add a new `PendingSftpContextAction::CreateFile { path, refresh_path }`
- route `new-file` to an SFTP create-file modal flow
- on confirm, dispatch `sftp_upload_file_async(..., Vec::new())`
- send a refresh message only after the remote create succeeds

**Step 4: Run the tests to verify they pass**

Run:
```bash
cargo test --test sftp_context_menu_spec --test bootstrap_smoke -q
```
Expected: PASS.

**Step 5: Commit**

```bash
git add src/shell/view_model/context_menu_dispatcher.rs src/shell/view_model/asset_modal_executor.rs src/app/bootstrap/sftp.rs src/shell/view_model/assets.rs tests/sftp_context_menu_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: implement async sftp new-file creation"
```

### Task 4: Preserve folder-upload roots and empty directories

**Files:**
- Modify: `src/app/sftp/local_ops.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/app/sftp/session_binding.rs`
- Modify: `src/app/sftp/mod.rs` if exports change
- Test: `tests/sftp_transfer_flow_spec.rs`
- Test: `tests/sftp_queue_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add tests that prove:
- selecting a folder to upload keeps the picked folder name in the remote target path
- empty local directories are represented and created remotely
- drag-and-drop folder uploads keep the same semantics

**Step 2: Run the tests to verify they fail**

Run:
```bash
cargo test --test sftp_transfer_flow_spec --test sftp_queue_spec --test bootstrap_smoke -q
```
Expected: FAIL because uploads currently only model files.

**Step 3: Write the minimal implementation**

- extend local scan results to distinguish file entries from directory placeholders
- add an upload-directory queue action
- create remote directories during queued-transfer execution without trying to upload bytes
- preserve existing file-upload behavior and conflict handling

**Step 4: Run the tests to verify they pass**

Run:
```bash
cargo test --test sftp_transfer_flow_spec --test sftp_queue_spec --test bootstrap_smoke -q
```
Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/sftp/local_ops.rs src/app/sftp/queue.rs src/app/sftp/session_binding.rs src/app/sftp/mod.rs tests/sftp_transfer_flow_spec.rs tests/sftp_queue_spec.rs tests/bootstrap_smoke.rs
git commit -m "fix: preserve folder roots and empty dirs in sftp uploads"
```

### Task 5: Add transfer-center outside-click dismissal

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model/projection.rs`
- Test: `tests/transfer_center_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing test**

Add smoke/source assertions for a dismiss layer behind the transfer center and a bootstrap callback that closes the panel when the outside layer is clicked.

**Step 2: Run the tests to verify they fail**

Run:
```bash
cargo test --test transfer_center_smoke --test top_status_bar_smoke -q
```
Expected: FAIL because no outside-dismiss path exists.

**Step 3: Write the minimal implementation**

- add a transparent full-content dismiss layer that is only visible when the transfer center is open
- place it behind the transfer center panel and above the rest of the shell content
- add a close callback in bootstrap/view-model instead of reusing toggle-only semantics

**Step 4: Run the tests to verify they pass**

Run:
```bash
cargo test --test transfer_center_smoke --test top_status_bar_smoke -q
```
Expected: PASS.

**Step 5: Commit**

```bash
git add ui/app-window.slint src/app/bootstrap.rs src/shell/view_model/projection.rs tests/transfer_center_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "fix: dismiss transfer center on outside click"
```

### Task 6: Run the final verification sweep

**Files:**
- Verify: `ui/shell/right-panel.slint`
- Verify: `src/app/bootstrap/sftp.rs`
- Verify: `src/app/sftp/local_ops.rs`
- Verify: `src/app/sftp/queue.rs`
- Verify: `src/app/sftp/session_binding.rs`
- Verify: `ui/app-window.slint`

**Step 1: Run the focused suite**

Run:
```bash
cargo test --test sftp_right_panel_render_spec --test sftp_context_menu_spec --test sftp_transfer_flow_spec --test sftp_queue_spec --test bootstrap_smoke --test transfer_center_smoke --test top_status_bar_smoke -q
```
Expected: PASS.

**Step 2: Run a broader compile/test sanity check**

Run:
```bash
cargo test -q
```
Expected: PASS, or if unrelated pre-existing failures remain, document them explicitly.

**Step 3: Review git diff**

Run:
```bash
git status --short
git diff --stat
```
Expected: only the intended SFTP and transfer-center files changed.

**Step 4: Commit the final integrated fix**

```bash
git add ui/shell/right-panel.slint ui/app-window.slint src/app/bootstrap.rs src/app/bootstrap/sftp.rs src/app/sftp/local_ops.rs src/app/sftp/queue.rs src/app/sftp/session_binding.rs src/app/sftp/mod.rs src/shell/view_model/context_menu_dispatcher.rs src/shell/view_model/asset_modal_executor.rs src/shell/view_model/assets.rs src/shell/view_model/sftp.rs tests/sftp_right_panel_render_spec.rs tests/sftp_context_menu_spec.rs tests/sftp_transfer_flow_spec.rs tests/sftp_queue_spec.rs tests/bootstrap_smoke.rs tests/transfer_center_smoke.rs tests/top_status_bar_smoke.rs
git commit -m "fix: restore sftp browser and transfer center workflows"
```
