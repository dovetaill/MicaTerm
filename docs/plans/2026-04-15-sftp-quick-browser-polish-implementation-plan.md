# SFTP Quick Browser Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the right-side SFTP quick browser into a true narrow quick browser, move directory sync off the UI blocking path, and disambiguate duplicate SSH tab titles with reusable numeric suffixes.

**Architecture:** Keep `ShellViewModel` as the quick-browser/session projection source of truth, but stop treating the right rail as a multi-column file table. Add a background SFTP directory request path that runs on the Tokio runtime and returns results to the Slint event loop using request IDs to drop stale responses. Keep tab identity stable while adding a display-only duplicate-title resolver in workspace tab projection.

**Tech Stack:** Rust 2024, Slint 1.15.1, Tokio, `russh-sftp`, existing bootstrap/session-manager/workspace projection pipeline, `cargo test`, `cargo build`

---

**Execution Rules:**

- 每个任务先使用 `@superpowers:test-driven-development`：先写失败测试，再写最小实现，再跑通过。
- 如果异步请求或 UI 状态回投出现竞态、重复更新或结果乱序，立刻切 `@superpowers:systematic-debugging`，不要叠补丁猜修。
- 改动完成后必须执行 `@superpowers:verification-before-completion`，以新鲜测试输出为准。

### Task 1: Freeze the new quick browser render contract

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Test: `tests/sftp_quick_browser_render_spec.rs`

**Step 1: Write the failing test**

Add assertions that the quick browser:

- exposes icon-first header actions for follow / refresh / expand
- renders a two-line header structure with breadcrumb row
- no longer renders table header cells for `Type`, `Modified`, `Size`
- no longer keeps the list host in horizontal-scroll mode
- renders row meta inline instead of separate columns

**Step 2: Run test to verify it fails**

Run: `cargo test --test sftp_quick_browser_render_spec -q`
Expected: FAIL because the current Slint still renders the old table layout.

**Step 3: Write minimal implementation**

- Replace the current toolbar/table structure in `ui/shell/right-panel.slint`
- Add icon assets/properties needed for the new actions
- Keep `SftpPanelItem` contract temporarily compatible while moving visual rendering to `name + meta`
- Remove quick-browser-only horizontal scroll behavior

**Step 4: Run test to verify it passes**

Run: `cargo test --test sftp_quick_browser_render_spec -q`
Expected: PASS

### Task 2: Add quick browser row semantics and file-type presentation helpers

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/sftp/model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Cover:

- quick browser row projection emits inline meta text instead of four visible table columns
- file kinds can project richer visual categories (directory, file, symlink, archive, image, config, executable)
- parent row remains available and distinguishable

**Step 2: Run tests to verify they fail**

Run: `cargo test --test sftp_panel_state_spec --test bootstrap_smoke -q`
Expected: FAIL because row/meta projection and richer file-type mapping do not exist yet.

**Step 3: Write minimal implementation**

- Extend `SftpPanelItem` projection with display-friendly row metadata
- Add helper functions in `src/app/bootstrap/sftp.rs` to derive `meta`, `icon kind`, and compact status text
- Keep domain data model simple; classification can be based on kind + extension/name heuristics

**Step 4: Run tests to verify they pass**

Run: `cargo test --test sftp_panel_state_spec --test bootstrap_smoke -q`
Expected: PASS

### Task 3: Move quick browser directory loading off the UI blocking path

**Files:**
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/sftp/browser_controller.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`

**Step 1: Write the failing tests**

Add coverage proving:

- quick browser directory requests can be executed asynchronously without forcing immediate blocking projection work
- stale request IDs are dropped when follow/refresh/tab-switch requests race
- switching tabs can show previous snapshot state before the newest async refresh completes

**Step 2: Run tests to verify they fail**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test sftp_follow_cwd_spec -q`
Expected: FAIL because quick-browser reads still rely on synchronous `block_on` calls in the projection path.

**Step 3: Write minimal implementation**

- Add an async/background-friendly SFTP read-dir entrypoint in `SessionManager`
- Add a bootstrap worker/result channel path for SFTP browser requests
- Schedule SFTP loads on the Tokio runtime and re-enter the Slint loop with `slint::invoke_from_event_loop`
- Keep `request_id` authoritative and only project the latest accepted result
- Preserve last successful entries while a refresh is in flight

**Step 4: Run tests to verify they pass**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test sftp_follow_cwd_spec -q`
Expected: PASS

### Task 4: Add duplicate SSH tab title numbering with gap reuse

**Files:**
- Modify: `src/shell/tabs.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing test**

Cover:

- same-name SSH tabs render `name`, `name(2)`, `name(3)`
- closing `name(2)` and reopening yields `name(2)` again
- numbering is display-only and does not alter session IDs

**Step 2: Run test to verify it fails**

Run: `cargo test --test workspace_tabs_spec --test ssh_session_manager_spec -q`
Expected: FAIL because duplicate titles are not disambiguated yet.

**Step 3: Write minimal implementation**

- Add a projection-time duplicate-title resolver in workspace tab sync
- Keep raw session handle titles unchanged
- Optionally reuse the same resolved base title for SFTP workspace tabs where appropriate

**Step 4: Run test to verify it passes**

Run: `cargo test --test workspace_tabs_spec --test ssh_session_manager_spec -q`
Expected: PASS

### Task 5: Run focused verification and close the loop

**Files:**
- Modify: `docs/plans/2026-04-15-sftp-quick-browser-polish-design.md`
- Modify: `docs/plans/2026-04-15-sftp-quick-browser-polish-implementation-plan.md`

**Step 1: Run focused verification**

Run:

```bash
cargo test --test sftp_quick_browser_render_spec --test sftp_panel_state_spec --test ssh_session_manager_spec --test workspace_tabs_spec --test sftp_follow_cwd_spec --test bootstrap_smoke -q
cargo build
```

Expected: PASS

**Step 2: Re-read requirements**

Check that the implementation actually satisfies:

- icon-based follow/refresh/expand actions
- two-row header with breadcrumb
- no quick-browser horizontal table layout
- async quick-browser refresh path
- duplicate SSH tab numbering with gap reuse

**Step 3: Update docs if implementation details changed**

Record any deviations or follow-ups in the design/plan docs.

**Step 4: Report completion with evidence**

Do not claim success until the fresh commands above pass.
