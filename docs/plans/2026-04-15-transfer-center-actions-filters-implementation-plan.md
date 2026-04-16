# Transfer Center Actions And Filters Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add lightweight row actions and real status filtering to the transfer center without turning it into a heavy workspace.

**Architecture:** Keep the transfer center backed by the real transfer-task list. Add local filter state and row action callbacks in Slint, then route actions through bootstrap into real SFTP queue/session operations. `Retry` only applies to failed tasks; conflict rows stay honest and expose `Open in SFTP Workspace` instead of a fake retry.

**Tech Stack:** Rust, Slint, existing SFTP transfer queue/session model, bootstrap callback wiring.

---

### Task 1: Lock UI contracts with failing tests

**Files:**
- Modify: `tests/transfer_center_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**
- Require transfer-center source to expose row-action callback(s) and filter state contract.
- Add bootstrap tests for:
  - failed transfer row exposes `Retry`
  - conflict row exposes `Open in SFTP Workspace`
  - failed filter includes failed/conflict rows and hides completed rows

**Step 2: Run test to verify it fails**
Run: `cargo test --test transfer_center_smoke --test bootstrap_smoke -q`
Expected: FAIL because filters/actions are not wired yet.

**Step 3: Write minimal implementation**
- None yet.

**Step 4: Run test to verify it still fails for the right reason**
Run: `cargo test --test transfer_center_smoke --test bootstrap_smoke -q`
Expected: FAIL on missing transfer-center filter/action behavior, not syntax/setup errors.

### Task 2: Add real transfer-center filters and row actions

**Files:**
- Modify: `ui/shell/transfer-center.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/sftp.rs`

**Step 1: Implement lightweight filter state**
- Add local filter state in `TransferCenter`.
- Make the existing status chips toggle real filtering; clicking the active chip resets to `All`.
- Define `Failed` filter as `Failed + Conflict`.

**Step 2: Implement row action contract**
- Extend `TransferCenterItem` with `can_retry` and `can_open_workspace`.
- Add row action callback(s) from Slint to AppWindow/bootstrap.

**Step 3: Implement real action backends**
- `Retry`: rebuild a single-task queue from the failed task, reset it to queued, and execute through the real SFTP transfer worker path.
- `Open in SFTP Workspace`: open or activate an SFTP workspace session for the task’s linked SSH session and navigate it to the relevant remote directory.

**Step 4: Run targeted tests**
Run: `cargo test --test transfer_center_smoke --test bootstrap_smoke -q`
Expected: PASS

### Task 3: Focused verification

**Files:**
- Verify only

**Step 1: Run focused verification**
Run:
`cargo test --test transfer_center_smoke --test bootstrap_smoke --test sftp_transfer_flow_spec --test top_status_bar_smoke -q`
`cargo build -q`
Expected: PASS

**Step 2: Re-read requirements**
- `Retry` exists only where semantics are honest.
- `Open in SFTP Workspace` reuses the existing heavy workspace path.
- transfer-center chips are real filters, not decorative counts.
- quick browser / terminal-first behavior remains unchanged.
