# SFTP Quick Browser Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rebuild the right-side SFTP quick browser into a true narrow quick browser, move directory sync off the UI blocking path, add context-menu-first file actions plus drag-and-drop upload, and disambiguate duplicate SSH tab titles with reusable numeric suffixes.

**Architecture:** Keep `ShellViewModel` as the quick-browser/session projection source of truth, but stop treating the right rail as a multi-column file table. Add a background SFTP directory request path that runs on the Tokio runtime and returns results to the Slint event loop using request IDs to drop stale responses. Extend the existing context-menu and transfer-queue model instead of inventing a second file-ops path, and hook desktop file drag/drop through Slint's winit custom application handler so dropped paths become queued uploads to the current quick-browser directory.

**Tech Stack:** Rust 2024, Slint 1.15.1, Tokio, winit 0.30 via Slint's `unstable-winit-030`, `russh-sftp`, existing bootstrap/session-manager/workspace projection pipeline, `cargo test`, `cargo build`

---

**Execution Rules:**

- 每个任务先使用 `@superpowers:test-driven-development`：先写失败测试，再写最小实现，再跑通过。
- 如果异步请求或 UI 状态回投出现竞态、重复更新或结果乱序，立刻切 `@superpowers:systematic-debugging`，不要叠补丁猜修。
- 改动完成后必须执行 `@superpowers:verification-before-completion`，以新鲜测试输出为准。

### Task 1: Freeze the quick browser header, state badge, and list render contract

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Test: `tests/sftp_quick_browser_render_spec.rs`

**Step 1: Write the failing test**

Add assertions that the quick browser:

- exposes icon-first header actions for follow / refresh / expand
- forwards tooltip open/close callbacks from the right panel through `AppWindow`
- renders the connection shell as a badge/status chip instead of an input-like field
- renders an explicit follow on/off visual state
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

### Task 2: Add quick browser cache/state semantics and richer row projection

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/sftp/browser_state.rs`
- Modify: `src/app/sftp/browser_controller.rs`
- Modify: `src/app/sftp/model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Cover:

- quick browser row projection emits inline meta text instead of four visible table columns
- file kinds can project richer visual categories (directory, file, symlink, archive, image, config, executable)
- parent row remains available and distinguishable
- browser state can retain the last successful directory snapshot while a refresh is in flight
- stale-while-revalidate state projects light syncing/stale affordances without clearing interaction

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

### Task 3: Move quick browser refresh work off the tab-switch hot path

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/sftp_follow_cwd_spec.rs`

**Step 1: Write the failing tests**

Add coverage proving:

- tab switching updates the terminal immediately without synchronously forcing a quick-browser activation refresh
- quick browser directory requests execute asynchronously without forcing immediate blocking projection work
- stale request IDs are dropped when follow/refresh/tab-switch requests race
- switching tabs can show previous snapshot state before the newest async refresh completes
- follow mode only refreshes when connection or cwd actually changes
- the projection timer no longer spins/yields in order to wait for SFTP results after queueing work

**Step 2: Run tests to verify they fail**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test sftp_follow_cwd_spec -q`
Expected: FAIL because quick-browser reads still rely on synchronous `block_on` calls in the projection path.

**Step 3: Write minimal implementation**

- Add an async/background-friendly SFTP read-dir entrypoint in `SessionManager`
- Add a bootstrap worker/result channel path for SFTP browser requests
- Schedule SFTP loads on the Tokio runtime and re-enter the Slint loop with `slint::invoke_from_event_loop`
- Keep `request_id` authoritative and only project the latest accepted result
- Preserve last successful entries while a refresh is in flight
- Remove eager SFTP refresh from the workspace-tab selection path and let the timer/background request path perform revalidation
- Drop the short busy-wait/yield loop after queuing SFTP work

**Step 4: Run tests to verify they pass**

Run: `cargo test --test ssh_session_manager_spec --test bootstrap_smoke --test sftp_follow_cwd_spec -q`
Expected: PASS

### Task 4: Expand SFTP context menus without overpromising backend support

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `ui/components/assets-context-menu-column.slint`
- Test: `tests/sftp_context_menu_spec.rs`

**Step 1: Write the failing test**

Cover:

- blank/file/folder/multi-selection menus expose the requested grouped actions in stable order
- disabled/planned entries remain visibly disabled and can explain unsupported actions
- right-clicking an already multi-selected SFTP entry can resolve to the multi-selection menu target
- copy/paste labels do not imply server-side copy when only relay/planned flow exists

**Step 2: Run test to verify it fails**

Run: `cargo test --test sftp_context_menu_spec -q`
Expected: FAIL because the current SFTP menus are still partial and do not cover the expanded IA.

**Step 3: Write minimal implementation**

- Extend `ContextMenuActionNode`/selection context with enough metadata for disabled reasons or planned relay actions
- Add the requested blank/file/folder/multi-selection action groups with separators and icons
- Wire leaf actions that already have real behavior (`open`, `refresh`, `new-folder`, `rename`, `delete`, `expand to workspace`, path copy)
- Keep unsupported actions disabled/planned instead of pretending they work

**Step 4: Run test to verify it passes**

Run: `cargo test --test sftp_context_menu_spec -q`
Expected: PASS

### Task 5: Add drag-and-drop upload into the quick browser content area

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Modify: `src/app/sftp/queue.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Cover:

- desktop file drops over the right-panel content area queue uploads to the current SFTP path
- dragging over the quick browser toggles a visible drop-target state only when the current session/path can accept uploads
- dropping files does not block the UI thread and does not steal terminal focus
- dropping while the browser is disconnected/empty/manual-invalid does nothing

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke -q`
Expected: FAIL because the app does not yet bridge OS drag/drop events into quick-browser upload queue actions.

**Step 3: Write minimal implementation**

- Install a winit custom application handler through `BackendSelector::with_winit_custom_application_handler(...)`
- Capture `HoveredFile`, `HoveredFileCancelled`, and `DroppedFile` events and forward them into bootstrap
- Track a lightweight quick-browser drop-target state in the view model
- Reuse local scan + transfer queue helpers to enqueue uploads into the current remote directory
- Refresh/revalidate the current directory after queueing or after queue completion when needed

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke -q`
Expected: PASS

### Task 6: Add duplicate SSH tab title numbering with gap reuse

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

### Task 7: Run focused verification and close the loop

**Files:**
- Modify: `docs/plans/2026-04-15-sftp-quick-browser-polish-design.md`
- Modify: `docs/plans/2026-04-15-sftp-quick-browser-polish-implementation-plan.md`

**Step 1: Run focused verification**

Run:

```bash
cargo test --test sftp_quick_browser_render_spec --test sftp_panel_state_spec --test ssh_session_manager_spec --test workspace_tabs_spec --test sftp_follow_cwd_spec --test bootstrap_smoke -q
cargo test --test sftp_context_menu_spec -q
cargo build
```

Expected: PASS

**Step 2: Re-read requirements**

Check that the implementation actually satisfies:

- icon-based follow/refresh/expand actions
- right-panel tooltip wiring and connection badge semantics
- two-row header with breadcrumb
- no quick-browser horizontal table layout
- async quick-browser refresh path
- context-menu-first action set with honest disabled/planned semantics
- drag-and-drop upload into the quick browser content area
- duplicate SSH tab numbering with gap reuse

**Step 3: Update docs if implementation details changed**

Record any deviations or follow-ups in the design/plan docs.

**Step 4: Report completion with evidence**

Do not claim success until the fresh commands above pass.
