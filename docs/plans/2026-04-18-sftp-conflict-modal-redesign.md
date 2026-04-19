# SFTP Conflict Modal Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the SFTP conflict modal into a clearer Fluent-style desktop dialog with unambiguous download actions and corrected footer/layout behavior.

**Architecture:** Keep the existing Rust conflict-resolution callbacks for skip / auto-rename / replace, but rebuild the Slint modal structure around a dedicated elevated shell, explicit download copy, and button-level focus semantics. Remove the ambiguous close/cancel path by routing dismiss actions into the existing skip flow and update transfer-center projection so skipped items read more honestly.

**Tech Stack:** Rust, Slint, shared theme tokens, existing blocking modal shell, cargo test.

---

### Task 1: Lock the redesigned modal contract with failing tests

**Files:**
- Modify: `tests/transfer_center_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**
- Assert the modal source exposes `Skip This Download`, `Auto Rename`, and `Replace Existing`.
- Assert the Fluent dismiss icon asset is used.
- Assert the old `Cancel Download` callback/label contract is gone.
- Assert the footer structure uses an independent footer container rather than a bare divider + absolutely placed button cluster.
- Assert invoking modal close now produces the same transfer outcome as skip for the current download.

**Step 2: Run tests to verify they fail**
Run:
```bash
cargo test --test transfer_center_smoke --test assets_modal_render_spec --test bootstrap_smoke -q
```
Expected: failures because the old modal still exposes ambiguous cancel/close behavior and old layout strings.

**Step 3: Commit the failing tests snapshot**
Do not commit unless explicitly requested by the user.

### Task 2: Rebuild the Slint modal structure and visuals

**Files:**
- Modify: `ui/components/sftp-conflict-modal.slint`
- Modify: `ui/components/blocking-modal-shell.slint`
- Modify: `ui/theme/tokens.slint`

**Step 1: Implement the new structure**
- replace absolute body/footer stacking with header + scrollable body + footer
- add Fluent dismiss icon
- add button focus helper so `Auto Rename` can receive initial focus
- remove modal-level Enter override and let focused buttons handle activation

**Step 2: Implement elevated visuals**
- add subtle conflict-dialog shell tokens and/or reuse utility-panel shadow/glow tokens
- give the conflict modal rounded corners, restrained glow, and separate footer band

**Step 3: Re-run targeted tests**
Run:
```bash
cargo test --test transfer_center_smoke --test assets_modal_render_spec -q
```
Expected: modal-contract tests pass; remaining behavior tests may still fail until bootstrap wiring is updated.

### Task 3: Rewire dismiss semantics and remove the obsolete cancel-download path

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/sftp.rs`

**Step 1: Update callback wiring**
- route top-right dismiss and `Esc` into `skip-requested`
- remove the obsolete `cancel-download-requested` UI callback from app-window/modal wiring
- keep Rust resolution connected to `skip`, `auto rename`, and `replace`

**Step 2: Re-run behavior tests**
Run:
```bash
cargo test --test bootstrap_smoke -q
```
Expected: close/skip behavior now resolves the current download instead of closing silently.

### Task 4: Improve projected transfer labels for skipped conflicts

**Files:**
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Update projection logic**
- if a cancelled transfer was cancelled by an explicit skip policy, surface `Skipped` in the transfer center labels / progress copy

**Step 2: Re-run tests**
Run:
```bash
cargo test --test bootstrap_smoke -q
```
Expected: skipped download rows read as `Skipped`.

### Task 5: Run final verification

**Files:**
- No new files beyond the above

**Step 1: Run focused verification**
Run:
```bash
cargo test --test transfer_center_smoke --test assets_modal_render_spec --test bootstrap_smoke -q
```

**Step 2: Run formatting if needed**
Run only on modified Rust files if formatting drift appears:
```bash
cargo fmt -- src/app/bootstrap/sftp.rs src/app/bootstrap/shell_chrome.rs
```

**Step 3: Inspect diff**
Run:
```bash
git diff -- ui/components/sftp-conflict-modal.slint ui/components/blocking-modal-shell.slint ui/theme/tokens.slint ui/app-window.slint src/app/bootstrap/sftp.rs src/app/bootstrap/shell_chrome.rs tests/transfer_center_smoke.rs tests/assets_modal_render_spec.rs tests/bootstrap_smoke.rs docs/plans/2026-04-18-sftp-conflict-modal-redesign-design.md docs/plans/2026-04-18-sftp-conflict-modal-redesign.md
```
