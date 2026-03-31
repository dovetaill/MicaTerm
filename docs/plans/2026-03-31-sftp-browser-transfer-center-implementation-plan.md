# SFTP Browser And Transfer Center Implementation Plan

日期: 2026-03-31
执行者: Codex
状态: 方案已确认，待进入实现

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the SFTP experience into a usable remote file browser with a Windows-style editable breadcrumb bar, a multi-column remote file table, a separate transfer center, and a dedicated browser controller that fixes the stuck-connecting and session-follow bugs.

**Architecture:** Introduce a dedicated `SftpBrowserController` that owns per-session browser state and all directory loading events, while keeping `bootstrap` limited to callback wiring and UI projection. Remove queue rendering from the right panel, add a transfer-center surface reachable from the titlebar, and project only active-session browser state into the narrow right rail.

**Tech Stack:** Rust, Slint, Tokio, russh-sftp, cargo test, cargo check, cargo clippy

---

### Task 1: Freeze the SFTP controller contract with failing tests

**Files:**
- Create: `tests/sftp_browser_controller_spec.rs`
- Modify: `src/app/sftp/mod.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`

**Step 1: Write the failing test**

Add controller-focused tests that prove the future contract:

```rust
#[test]
fn open_loads_active_session_directory_and_marks_ready() {
    // Arrange controller + fake session manager/runtime
    // Act controller.open(active_session, "/srv/app")
    // Assert mode == Ready and entries are projected
}

#[test]
fn stale_directory_results_do_not_overwrite_newer_requests() {
    // Arrange two queued responses with different request ids
    // Assert older response is discarded
}
```

Also extend `tests/sftp_follow_cwd_spec.rs` with a failing case that tab switch must reproject the correct active-session path.

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_browser_controller_spec --test sftp_follow_cwd_spec -q
```

Expected:

- FAIL because `SftpBrowserController` does not exist
- FAIL because tab switch does not force a fresh SFTP projection

**Step 3: Write minimal implementation**

- Add the module export in `src/app/sftp/mod.rs`
- Create a minimal controller/state shape just sufficient for the tests to compile and fail for the intended reason

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/sftp/mod.rs tests/sftp_browser_controller_spec.rs tests/sftp_follow_cwd_spec.rs
git commit -m "test: freeze sftp browser controller contract"
```

### Task 2: Implement per-session SFTP browser state and request token protection

**Files:**
- Create: `src/app/sftp/browser_state.rs`
- Create: `src/app/sftp/browser_controller.rs`
- Modify: `src/app/sftp/mod.rs`
- Test: `tests/sftp_browser_controller_spec.rs`

**Step 1: Write the failing test**

Expand the controller tests to cover:

```rust
#[test]
fn navigate_switches_to_manual_browse_and_pushes_history() {}

#[test]
fn follow_cwd_only_updates_when_follow_mode_is_enabled() {}

#[test]
fn retry_moves_disconnected_session_back_to_connecting() {}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_browser_controller_spec -q
```

Expected:

- FAIL because state machine transitions and request token handling are not implemented yet

**Step 3: Write minimal implementation**

- Implement `SftpBrowserSessionState`
- Implement `SftpBrowserController`
- Add `active_request_id` protection so stale loads are discarded
- Keep the API narrow: `open`, `session_activated`, `follow_cwd`, `navigate`, `refresh`, `retry`

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/sftp/browser_state.rs src/app/sftp/browser_controller.rs src/app/sftp/mod.rs tests/sftp_browser_controller_spec.rs
git commit -m "feat: add session scoped sftp browser controller"
```

### Task 3: Route real directory loads through the controller and remove bootstrap-side fake state transitions

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`

**Step 1: Write the failing test**

Add or update failing tests for:

```rust
#[test]
fn opening_sftp_reads_the_active_session_directory_instead_of_staying_connecting() {}

#[test]
fn switching_workspace_tabs_reprojects_sftp_to_the_new_active_session() {}

#[test]
fn refresh_and_path_submit_trigger_real_directory_reads() {}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke --test sftp_follow_cwd_spec -q
```

Expected:

- FAIL because current callbacks only mutate `ShellViewModel` mode without a guaranteed `sftp_read_dir(...)` call path

**Step 3: Write minimal implementation**

- Add a single helper in `bootstrap` that hands SFTP events to the controller
- Ensure `open`, `refresh`, `retry`, path submit, follow-cwd, and tab switch all flow through controller-managed directory loads
- Stop using bootstrap-only `mark_loading` / `mark_connecting` as terminal behavior
- Keep `SessionManager::sftp_read_dir(...)` as the runtime boundary for now

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/app/ssh/session_manager.rs tests/bootstrap_smoke.rs tests/sftp_follow_cwd_spec.rs
git commit -m "fix: route sftp directory loads through browser controller"
```

### Task 4: Replace the current right panel with a single-line toolbar, editable breadcrumb bar, and multi-column file table

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/sftp_context_menu_spec.rs`

**Step 1: Write the failing test**

Add render and interaction coverage for:

```rust
#[test]
fn right_panel_renders_single_line_toolbar_and_path_bar() {}

#[test]
fn multi_column_remote_table_renders_name_modified_and_size_headers() {}

#[test]
fn queue_summary_is_no_longer_rendered_inside_the_right_panel() {}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_right_panel_render_spec --test sftp_context_menu_spec -q
```

Expected:

- FAIL because the panel still contains the session card, text buttons, and queue strip

**Step 3: Write minimal implementation**

- Replace text toolbar buttons with Fluent icon buttons
- Collapse secondary actions under a single `+` menu
- Add a breadcrumb/path control that can switch to editable text mode
- Replace the card-like list with a tighter multi-column table
- Remove queue-specific visuals from the right panel

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add ui/shell/right-panel.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/sftp_right_panel_render_spec.rs tests/sftp_context_menu_spec.rs
git commit -m "feat: rebuild sftp panel as compact file browser"
```

### Task 5: Add a top-status transfer icon and a separate transfer center surface

**Files:**
- Create: `ui/shell/transfer-center.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Create: `tests/transfer_center_smoke.rs`

**Step 1: Write the failing test**

Add tests that assert:

```rust
#[test]
fn titlebar_exposes_transfer_icon_with_queue_badge() {}

#[test]
fn clicking_transfer_icon_opens_transfer_center_surface() {}

#[test]
fn transfer_center_renders_running_queued_paused_failed_completed_tabs() {}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test top_status_bar_smoke --test transfer_center_smoke -q
```

Expected:

- FAIL because there is no transfer icon, no transfer-center surface, and queue still lives in the SFTP panel

**Step 3: Write minimal implementation**

- Add the titlebar transfer icon and badge projection
- Add `transfer-center.slint`
- Project shared queue summary and visible transfer rows into the new surface
- Keep the first iteration simple: tab strip + table + empty state

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add ui/shell/transfer-center.slint ui/shell/titlebar.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/top_status_bar_smoke.rs tests/transfer_center_smoke.rs
git commit -m "feat: move transfers into dedicated transfer center"
```

### Task 6: Finish the active-session follow model and error/disconnect UX

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/sftp/browser_controller.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/sftp_follow_cwd_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Add failing tests for:

```rust
#[test]
fn manual_navigation_pauses_follow_until_user_explicitly_reenables_it() {}

#[test]
fn disconnected_state_keeps_last_path_and_exposes_retry_without_fake_connecting_loop() {}

#[test]
fn path_errors_render_as_lightweight_status_rows_instead_of_full_height_empty_cards() {}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test sftp_follow_cwd_spec --test bootstrap_smoke -q
```

Expected:

- FAIL because follow/manual/disconnect UX still matches the legacy panel behavior

**Step 3: Write minimal implementation**

- Ensure `FollowCwd` only mutates browsing when follow mode is still active
- Preserve last path on error/disconnect
- Replace large empty-card feedback with compact status-row feedback in the panel

**Step 4: Run test to verify it passes**

Run the same command from Step 2.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/sftp/browser_controller.rs src/shell/view_model.rs ui/shell/right-panel.slint tests/sftp_follow_cwd_spec.rs tests/bootstrap_smoke.rs
git commit -m "fix: complete sftp follow and error state redesign"
```

### Task 7: Run full verification and document residual risks

**Files:**
- Modify: `docs/plans/2026-03-31-sftp-browser-transfer-center-design.md`
- Modify: `docs/plans/2026-03-31-sftp-browser-transfer-center-implementation-plan.md`

**Step 1: Run focused tests**

Run:

```bash
cargo test --test sftp_browser_controller_spec --test sftp_follow_cwd_spec --test sftp_right_panel_render_spec --test transfer_center_smoke --test top_status_bar_smoke -q
```

Expected:

- PASS

**Step 2: Run full workspace verification**

Run:

```bash
cargo test --workspace
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 3: Record any follow-up notes**

Update the design/plan docs only if the implementation revealed:

- deferred enhancements
- residual UX debt
- known protocol limitations

**Step 4: Commit**

```bash
git add docs/plans/2026-03-31-sftp-browser-transfer-center-design.md docs/plans/2026-03-31-sftp-browser-transfer-center-implementation-plan.md
git commit -m "docs: finalize sftp browser redesign plan notes"
```

## Task 7 Completion Notes

Verification completed on `2026-03-31`:

- Focused regression pass:
  - `cargo test --test sftp_browser_controller_spec --test sftp_follow_cwd_spec --test sftp_right_panel_render_spec --test transfer_center_smoke --test top_status_bar_smoke -q`
- Full workspace pass:
  - `cargo test --workspace`
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

Implementation follow-up notes:

- `Transfer Center` is intentionally shipped as a first-iteration shell. The titlebar entry, badge, tabs, and empty state are live, but tab-specific row projection is deferred.
- The SFTP browser now preserves manual browsing context across disconnect/retry, but reconnect completion is still finalized by the existing projection timer loop instead of a dedicated direct callback.
- The right-panel table contract is in place, but `Modified` still shows a placeholder kind label until remote metadata is added to `SftpDirectoryEntry`.
