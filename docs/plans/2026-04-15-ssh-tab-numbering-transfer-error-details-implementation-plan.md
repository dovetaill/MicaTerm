# Duplicate SSH Tab Titles And Transfer Error Details Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Verify duplicate SSH tab numbering all the way through the UI and add lightweight failed/conflict error details to the transfer center.

**Architecture:** Keep duplicate numbering in `SessionManager` as the only source of truth, and project transfer-task errors into the existing transfer center model without adding a second details surface. Reuse the shared tooltip overlay pattern already used by the titlebar and quick browser.

**Tech Stack:** Rust, Slint, bootstrap projection helpers, existing SFTP transfer queue model.

---

### Task 1: Lock duplicate SSH tab numbering at the UI layer

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `src/app/ssh/session_manager.rs`
- Reference: `src/app/bootstrap.rs`

**Step 1: Write the failing test**
- Add a bootstrap/UI regression that opens the same SSH asset in duplicate tabs, checks `Prod Bastion`, `Prod Bastion(2)`, `Prod Bastion(3)`, closes `(2)`, and confirms the next reopened tab reuses `(2)`.

**Step 2: Run test to verify it fails**
Run: `cargo test --test bootstrap_smoke duplicate_ssh_tabs_keep_resolved_titles_and_reuse_suffix_gaps -q`
Expected: FAIL if the UI path drops or rewrites the resolved titles.

**Step 3: Write minimal implementation**
- Only change projection code if the UI path is not preserving `SessionHandle.title`.

**Step 4: Run test to verify it passes**
Run: `cargo test --test bootstrap_smoke duplicate_ssh_tabs_keep_resolved_titles_and_reuse_suffix_gaps -q`
Expected: PASS

### Task 2: Add lightweight transfer error summaries and tooltips

**Files:**
- Modify: `ui/shell/transfer-center.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `tests/transfer_center_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**
- Add/extend tests that require `TransferCenterItem` to expose error-summary fields and verify failed/conflict rows surface inline text plus tooltip wiring.

**Step 2: Run tests to verify they fail**
Run: `cargo test --test transfer_center_smoke --test bootstrap_smoke transfer_center -q`
Expected: FAIL because the current projected rows do not carry error summary/tooltip fields.

**Step 3: Write minimal implementation**
- Extend `TransferCenterItem` projection with `error_summary`, `error_tooltip`, and a boolean gate.
- Render a compact inline error line only for failed/conflict rows.
- Reuse `TitlebarTooltip` through a transfer-center-specific overlay in `AppWindow`.

**Step 4: Run tests to verify they pass**
Run: `cargo test --test transfer_center_smoke --test bootstrap_smoke transfer_center -q`
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
- Duplicate SSH tab numbering stays single-sourced and UI-visible.
- Transfer center remains lightweight while showing actionable failed/conflict details.
- No new heavy drawer/modal/expanded row system appears.
