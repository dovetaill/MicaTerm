# SFTP Main-Thread Latency Probes Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add precise SFTP main-thread probe points so we can tell whether the remaining freeze happens before request dispatch, while applying the result into browser state, or while syncing the right panel into Slint.

**Architecture:** Keep the SFTP directory read fully async, but extend the existing `app.async_latency` flow ids (`sftp-panel-open` and `sftp-panel-switch`) with UI-thread stages after the background message arrives. Reuse the existing request latency seed in `ShellViewModel`, stage one pending UI-sync trace per SFTP result, and consume it during `sync_right_panel_state(...)` so the final log clearly separates browser-state projection from Slint model sync.

**Tech Stack:** Rust, Slint, tokio, tracing, cargo test

---

### Task 1: Lock the probe contract before code changes

**Files:**
- Modify: `tests/async_latency_contract_spec.rs`

**Step 1: Write the failing test**
- Extend the SFTP contract test to require `result-drained`, `browser-state-applied`, and `right-panel-sync-finished` markers in source/docs.

**Step 2: Run test to verify it fails**
- Run: `cargo test --test async_latency_contract_spec`
- Expected: FAIL because the new markers do not exist yet.

### Task 2: Stage and consume the new SFTP UI-sync trace

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`

**Step 1: Add minimal state support**
- Introduce a dedicated pending SFTP UI-sync trace alongside the existing request trace types in `ShellViewModel`.

**Step 2: Wire the result-application stages**
- Log `result-drained` when the UI thread first receives the async SFTP result.
- Log `browser-state-applied` once the controller + view-model projection finishes.
- Preserve the trace so `sync_right_panel_state(...)` can emit `right-panel-sync-start` and `right-panel-sync-finished` in the same flow.

**Step 3: Keep the async/sync fallback diagnosable**
- If the async runtime is absent, emit an explicit fallback stage before entering the blocking path so logs can prove whether a real sync fallback happened.

### Task 3: Document how to read the new probes

**Files:**
- Modify: `docs/plans/2026-04-20-async-latency-instrumentation.md`
- Create: `docs/plans/2026-04-20-sftp-main-thread-latency-probes-implementation-plan.md`

**Step 1: Update the central latency note**
- Describe each new SFTP stage and what a high value means.
- Clarify that `request-queued` isolates timer/dispatch delay, while `right-panel-sync-finished` isolates final Slint/model sync cost.

### Task 4: Verify and capture the conclusion

**Files:**
- Modify: `tests/async_latency_contract_spec.rs`
- Optional context: `src/app/bootstrap.rs`, `src/app/bootstrap/sftp.rs`

**Step 1: Run focused verification**
- Run: `cargo test --test async_latency_contract_spec`
- Run: `cargo test --lib sftp -- --nocapture`

**Step 2: Summarize the diagnostic reading**
- If `ui-return` and `request-queued` stay low while `browser-state-applied` or `right-panel-sync-finished` spikes, the remaining freeze is local main-thread apply/render work.
- If the new SFTP stages stay low too, then renderer/event-loop behavior becomes a stronger upstream suspect and should be compared against alternate Slint renderers on Windows.
