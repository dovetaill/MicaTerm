# SFTP Right Panel Virtualization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the right-side SFTP panel's full-directory UI model with a bounded viewport window so large directories stop synchronizing every row into Slint.

**Architecture:** Keep the full SFTP render cache in Rust, add viewport/window state per browser session, expose only a visible slice plus spacer heights to the UI, and swap the SFTP row host from `ListView` to `ScrollView` so the scrollbar still represents the full directory height while the UI only renders nearby rows.

**Tech Stack:** Rust, Slint `ScrollView`, `VecModel`, existing `ShellViewModel` and SFTP browser session state.

---

### Task 1: Lock the virtualization contract with failing tests

**Files:**
- Modify: `tests/sftp_panel_state_spec.rs`
- Modify: `tests/sftp_right_panel_render_spec.rs`
- Reference: `src/shell/view_model/sftp.rs`
- Reference: `src/app/bootstrap/sftp.rs`
- Reference: `ui/shell/right-panel.slint`

**Step 1: Write the failing tests**
- Add a state test that creates a large SFTP directory and asserts the active UI-facing row slice is bounded below the full row count after viewport sizing is applied.
- Add a state test that changes viewport offset and asserts the visible row range and spacer heights move accordingly.
- Add a source-level test that the SFTP row host uses `ScrollView` plus viewport callback/spacer bindings instead of a plain `ListView` over the full item model.

**Step 2: Run test to verify it fails**
Run: `cargo test --test sftp_panel_state_spec --test sftp_right_panel_render_spec -- --nocapture`
Expected: FAIL because viewport/window state and the new UI contract do not exist yet.

### Task 2: Add viewport-window state to the SFTP render cache

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`

**Step 1: Add virtualization fields**
- Extend `SftpPanelRenderCache` with viewport offset/height, active visible row range, spacer heights, and visible-window dirty indices.

**Step 2: Implement window derivation helpers**
- Add helpers that derive a bounded visible window from total rows, row height, visible height, and overscan.
- Default to a safe initial viewport so the first open already uses a bounded row slice.

**Step 3: Thread selection updates through the visible window**
- Keep mutating the full row cache for selection changes.
- Only mark visible-window indices dirty when changed rows are inside the active visible range.

**Step 4: Add viewport update entrypoints**
- Expose helpers so bootstrap can push `viewport_y` and `visible_height` into the active SFTP panel session and recompute the visible window when needed.

### Task 3: Switch the right-panel UI to a spacer-backed `ScrollView`

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`

**Step 1: Add new panel properties and callback**
- Add spacer-height and total-content-height properties.
- Add a `sftp-panel-viewport-changed(length, length)` callback and forward it through `AppWindow`.

**Step 2: Replace the SFTP `ListView` row host**
- Use `ScrollView` with a full-height scroll body.
- Render top spacer, visible rows, and bottom spacer.
- Preserve row click, double-click, and context-menu behavior.

**Step 3: Emit viewport changes**
- Send viewport offset and visible height back to Rust on init, scroll, and visible-height changes.

### Task 4: Teach bootstrap to sync the bounded window model

**Files:**
- Modify: `src/app/bootstrap/sftp.rs`

**Step 1: Sync spacer properties every pass**
- Push total content height and spacer heights into Slint.

**Step 2: Bind viewport callback**
- Handle the new viewport callback by updating the active render-cache window and re-syncing the right panel only when the window actually changes.

**Step 3: Keep item model incremental**
- Continue to use incremental `VecModel` patching, but now against the visible window row slice rather than the full directory row set.

### Task 5: Verify and document

**Files:**
- Modify: `docs/plans/2026-04-20-sftp-right-panel-virtualization-design.md`
- Modify: `docs/plans/2026-04-20-sftp-right-panel-virtualization-implementation-plan.md`

**Step 1: Run focused tests**
Run: `cargo test --test sftp_panel_state_spec --test sftp_right_panel_render_spec -- --nocapture`
Expected: PASS

**Step 2: Run regression coverage**
Run: `cargo test --test async_latency_contract_spec -- --nocapture`
Run: `cargo test --test sftp_panel_state_spec -- --nocapture`
Run: `cargo test --test sftp_right_panel_render_spec -- --nocapture`
Run: `cargo test --test bootstrap_smoke -- --nocapture`
Expected: PASS

**Step 3: Commit**
Run: `git add docs/plans/2026-04-20-sftp-right-panel-virtualization-design.md docs/plans/2026-04-20-sftp-right-panel-virtualization-implementation-plan.md ui/shell/right-panel.slint ui/app-window.slint src/shell/view_model.rs src/shell/view_model/sftp.rs src/app/bootstrap/sftp.rs tests/sftp_panel_state_spec.rs tests/sftp_right_panel_render_spec.rs`
Run: `git commit -m "feat: virtualize sftp right panel rows"`
Expected: new commit with the virtualization implementation and docs.
