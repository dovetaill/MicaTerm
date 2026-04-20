# SFTP Right Panel Incremental Sync Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Keep the right-side SFTP panel responsive for large directories by avoiding full row rebuilding and full row reconciliation on every shell sync pass.

**Architecture:** Keep the existing async SFTP loading pipeline, but add a second cache layer for rendered right-panel rows. The view model will track a per-session render cache plus dirty-row metadata so bootstrap can patch only the rows that changed, and only do a full model rebuild when the active SFTP session changes or the directory snapshot structure changes.

**Tech Stack:** Rust, Slint `VecModel`, existing `ShellViewModel`/SFTP browser session state.

---

### Task 1: Lock the regression with tests

**Files:**
- Modify: `tests/sftp_panel_state_spec.rs`
- Reference: `src/shell/view_model/sftp.rs`
- Reference: `src/app/bootstrap/sftp.rs`

**Step 1: Write the failing test**
- Add a unit test that changes SFTP selection after the render cache has been marked clean and asserts only the affected row indices become dirty instead of forcing a full panel rebuild.
- Add a source-level guard that `src/app/bootstrap/sftp.rs` uses a dedicated incremental sync helper instead of generic `sync_vec_model()` for `sftp_panel_items`.

**Step 2: Run test to verify it fails**
Run: `cargo test --test sftp_panel_state_spec -- --nocapture`
Expected: FAIL because the new render-cache APIs / incremental sync helper do not exist yet.

**Step 3: Write minimal implementation**
- Add just enough render-cache state and bootstrap hook points to make the new tests compile and fail for the right reason.

**Step 4: Run test to verify it still fails for the intended gap**
Run: `cargo test --test sftp_panel_state_spec -- --nocapture`
Expected: FAIL because the cache is not yet updated incrementally.

### Task 2: Add per-session render-cache state in the view model

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Reference: `src/app/sftp/browser_session.rs`

**Step 1: Build the render-cache types**
- Add a per-session cache for rendered right-panel rows, including row data, row index lookup by entry id, dirty row indices, and a full-resync flag.
- Add a field that tracks which SFTP session was last rendered into the actual right-panel model.

**Step 2: Populate the cache from session snapshots**
- Extend `set_file_browser_session()` so every directory/sort/path update rebuilds the right-panel render cache once and marks it as needing a full sync.
- Keep the existing projection cache; the render cache should sit on top of it.

**Step 3: Update selection-only mutations incrementally**
- Update `select_sftp_panel_entry()` and `select_all_sftp_entries()` so they patch only the affected cached rows and dirty-row indices.
- Preserve correctness for parent-row offsets and select-all cases.

**Step 4: Add helper accessors for bootstrap**
- Expose helpers that return the active render rows, dirty indices, whether a full resync is needed, and a way to mark the active cache clean after sync.

### Task 3: Switch bootstrap to incremental right-panel syncing

**Files:**
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap.rs` (only if signatures / callers need `&mut ShellViewModel`)

**Step 1: Replace full item sync with incremental helper**
- Introduce a dedicated helper for `sftp_panel_items` that:
  - fully replaces the model when the session changed or the cache requests a full resync
  - patches only dirty indices when the row structure is unchanged
  - falls back to full replace if the current model is not a `VecModel`

**Step 2: Thread mutable state through right-panel sync**
- Update `sync_sftp_panel_state()` / `sync_right_panel_state()` callers to pass mutable state where needed so the cache can be marked clean after a successful sync.
- Leave other small panel properties on the existing fast path.

**Step 3: Keep non-row sync logic intact**
- Continue syncing selected-id chips, queue counts, and mode/path labels normally.
- Avoid changing SFTP async loading semantics in this task.

### Task 4: Verify no regressions in SSH/SFTP flows

**Files:**
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Run focused state tests**
Run: `cargo test --test sftp_panel_state_spec -- --nocapture`
Expected: PASS

**Step 2: Run broader integration coverage**
Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 3: Sanity-check diff scope**
Run: `git diff --stat`
Expected: only the SFTP right-panel/view-model/test files changed for this optimization layer.
