# SFTP Workspace Usability Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在当前已 merge 的 SFTP workspace 基线上，把主工作区收口成真实可用、可滚动、可导航、可右键、可断线重连、符合 Ayu/MicaTerm 语义的受控文件工作区。

**Architecture:** 保留 `FileBrowserSession`、`WorkspaceTab::Sftp`、现有 `SftpBrowserController` / `browser_session` / `session_binding` / transfer center 后端链路，不另起第二套 backend；把问题聚焦在 projection、Slint host、bootstrap 路由和 transient UI controller 的 contract 收口。优先修复 row metrics、viewport control、path edit state machine、context menu surface routing、content-state rendering，并继续只消费 runtime-projected Ayu theme properties。

**Tech Stack:** Rust, Slint, existing ShellViewModel, existing bootstrap glue, existing SFTP browser/controller/session binding, `cargo test`, `cargo check`, optional `cargo clippy`

---

## Input Docs

- `docs/plans/2026-05-20-sftp-workspace-usability-hardening-requirements.md`
- `docs/plans/2026-05-20-sftp-workspace-usability-hardening-design.md`
- `docs/plans/2026-05-19-sftp-workspace-productized-ui-design.md`
- `docs/plans/2026-05-19-sftp-workspace-productized-ui-implementation-plan.md`
- 历史两层 SFTP、transfer center、Ayu refinement 文档全部继续有效

## Execution Notes

- 必须在新的 `.worktrees` 目录中执行，不在当前文档会话直接改功能代码。
- 每个 task 都先用 `@superpowers:test-driven-development`：先写失败测试，再做最小实现，再跑通过。
- 对 scroll/viewport、右键菜单串路由、overlay 错位、tooltip 不显示这类问题，先用 `@superpowers:systematic-debugging` 做 root-cause tracing，不要猜修。
- 只有在受控 virtualization 无法在本轮稳定收口时，才允许进入 bounded full-list fallback；若走 fallback，必须补清楚性能门禁测试。
- 不允许引入 fake rows、demo-only workaround、或在 Slint 中硬编码第二套 Ayu palette。

## Task Sequence Overview

1. 先把当前假绿测试改成真实红灯 contract。
2. 收口 workspace viewport / row metrics / total-row-count contract。
3. 收口 breadcrumb-first path bar 与 `Ctrl+L` / `Enter` / `Esc` 状态机。
4. 接通 workspace toolbar tooltip、disabled reason、action width tiers。
5. 修复 surface-aware context menu routing 与 close-before-route transient 行为。
6. 把 disconnected/loading/empty/error 改成统一 content-state surface。
7. 收尾 Ayu runtime-token contract、响应式列与状态条，并做完整验证。

### Task 1: Rewrite the tests so the current regressions fail for the right reasons

**Files:**
- Modify: `tests/workspace_sftp_tab_contract_smoke.sh`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`
- Modify: `tests/workspace_sftp_projection_spec.rs`
- Modify: `tests/sftp_context_menu_spec.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Strengthen the shell smoke for real workspace contracts**

Replace overly static checks with source contracts that would fail on the current regressions:

- tooltip wiring must reference the shared tooltip path, not just local `tooltip-text`
- workspace host must not contain truncated `New Folde` / `New Fold...`
- workspace context menu contracts must exclude `New SSH Connection`
- disconnected state must not be expressed as `state-overlay` + `overlay-card`

Example anchors to add:

```bash
! rg -n "New Folde|New Fold\.\.\." ui/shell/sftp-workspace-host.slint >/dev/null
! rg -n "overlay-card := Rectangle" ui/shell/sftp-workspace-host.slint >/dev/null
rg -n "workspace-sftp-context-menu-requested" ui/app-window.slint >/dev/null
```

**Step 2: Add real render-contract tests for path editing and breadcrumb behavior**

In `tests/sftp_workspace_tab_render_spec.rs`, add failing assertions for:

- breadcrumb segment rendering for `/ > home > wwwroot`
- `Ctrl+L` routing into path editing
- `Esc` cancel path editing without resubmitting current path
- path shell click entering edit mode
- tooltip source/owner wiring for workspace toolbar

Prefer upgrading this file from pure source-string checks into a mixed `render + element contract` test. Reuse the existing `ElementHandle` interaction style already used elsewhere in the repo instead of adding more `source.contains(...)`-only coverage.

**Step 3: Add failing projection tests for viewport control**

In `tests/workspace_sftp_projection_spec.rs`, lock these behaviors first:

- root path `/` reset viewport to top
- explicit refresh completion resets viewport to top
- expand-to-workspace resets viewport to top
- selected row does not jump viewport into the middle
- total row count is distinct from the visible row slice

**Step 4: Add a true workspace-SFTP context-menu fixture**

In `tests/sftp_context_menu_spec.rs`, stop relying only on quick-browser fixtures.

Create a fixture where:

- quick browser path != workspace path
- quick browser mode != workspace mode
- workspace selected ids != quick browser selected ids

Then assert:

- workspace row menu gets SFTP file actions
- workspace blank-area menu gets workspace path
- `New SSH Connection` and assets actions are absent
- opening workspace menu closes assets create popup first

**Step 5: Extend bootstrap smoke around reconnect and transients**

In `tests/bootstrap_smoke.rs`, add failing integration assertions for:

- disconnected workspace `Reconnect` callback
- `Ctrl+L` -> begin path edit callback chain
- workspace right-click closes create popup and opens context menu
- disconnected/error render as content state, not overlay-card

**Step 6: Run the strengthened tests and verify they fail first**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test sftp_context_menu_spec -q
cargo test --test theme_semantic_token_contract_spec --test bootstrap_smoke -q
```

Expected: FAIL, and the failures must point at missing runtime behavior, not typos or missing imports.

### Task 2: Fix row metrics and make workspace viewport a controlled contract

**Files:**
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `tests/workspace_sftp_projection_spec.rs`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`

**Step 1: Investigate the current row-height split before changing code**

Read and confirm the mismatch:

```bash
rg -n "SFTP_PANEL_ROW_HEIGHT_PX|height: 40px" src/shell/view_model/sftp.rs ui/shell/sftp-workspace-host.slint
```

Expected: Rust and Slint disagree today.

**Step 2: Write the smallest failing unit around row-height math**

Add a focused test that proves the current first visible row / total height math is wrong when UI and VM row heights diverge.

**Step 3: Introduce a single row-height source of truth**

Use one constant or one projected property that both VM and Slint consume.

Example target shape:

```rust
const WORKSPACE_SFTP_ROW_HEIGHT_PX: u32 = 40;
```

Or project from one place only; do not double-author.

**Step 4: Make viewport-y controlled instead of host-private drift**

Ensure the active workspace SFTP session stores the authoritative viewport state, and the host binds to it.

At minimum:

- Rust updates viewport on user scroll
- bootstrap pushes viewport back to Slint
- render cache rebuild differentiates reset events from restore events

**Step 5: Separate `visible_rows` from `total_row_count`**

Add or expose a dedicated total-count projection so the status bar and empty-state logic stop using the visible slice length as the full item count.

**Step 6: Implement reset policy exactly as required**

Reset viewport to top on:

- path submit
- breadcrumb navigation
- Back / Forward / Up / Home
- explicit Refresh completion
- Expand to Workspace

Restore prior stable viewport on tab switch only.

**Step 7: Re-run viewport-focused tests**

Run:

```bash
cargo test --test workspace_sftp_projection_spec --test sftp_workspace_tab_render_spec -q
```

Expected: PASS, including root `/` showing first rows such as `home`.

### Task 3: Rebuild the workspace path bar around a real editing state machine

**Files:**
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Freeze the failing interaction contracts**

Add tests that prove:

- `Ctrl+L` enters editing
- clicking path shell enters editing
- `Enter` submits a draft path
- `Esc` cancels editing without resubmitting current path
- entering edit focuses and selects all text

Where feasible, mount the real `AppWindow` test fixture and exercise the workspace path bar through UI-level handles or callback entry points, rather than only asserting that callback names exist in source.

**Step 2: Introduce explicit begin / cancel / submit callbacks**

Today only submit exists. Add explicit cancel semantics.

Target shape:

```slint
callback workspace-sftp-path-edit-requested();
callback workspace-sftp-path-cancelled();
callback workspace-sftp-path-submitted(string);
```

And matching Rust handlers.

**Step 3: Model path editing as `viewing | editing` with a draft**

The VM should distinguish the canonical current path from the edit draft.

**Step 4: Route `Ctrl+L` through the workspace SFTP chain**

Wire it through the correct focused workspace path rather than terminal-only shortcuts.

**Step 5: Ensure focus/select-all happens deterministically**

Use the existing focus handoff pattern where possible; do not rely on implicit text-input focus.

**Step 6: Re-run path-edit and bootstrap tests**

Run:

```bash
cargo test --test sftp_workspace_tab_render_spec --test bootstrap_smoke -q
```

Expected: PASS.

### Task 4: Wire workspace toolbar tooltips, disabled reasons, and action width tiers

**Files:**
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `ui/app-window.slint`
- Modify: existing shared tooltip component files as needed
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `tests/workspace_sftp_tab_contract_smoke.sh`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`

**Step 1: Make the tests fail on non-wired tooltips**

Add assertions that distinguish between plain `tooltip-text` literals and actual overlay wiring / anchor export.

If the repo already has a software-render or hover-driven tooltip test harness, reuse it here so the test can fail when the tooltip text exists but never becomes visible.

**Step 2: Reuse the existing tooltip ownership model**

Make workspace toolbar emit tooltip state the same way titlebar/right-panel buttons already do.

Do not add a local ad-hoc tooltip popup.

**Step 3: Allow disabled controls to explain themselves**

Separate “cannot click” from “cannot hover/focus”.

At minimum, disconnected actions should surface `Reconnect to browse files`.

**Step 4: Add width-tier logic for the action cluster**

Make `Upload` / `New Folder` / `Transfer Center` choose between:

- icon + label
- icon-only + tooltip

based on available width, without truncating the label into gibberish.

**Step 5: Re-run tooltip and width smoke tests**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test sftp_workspace_tab_render_spec -q
```

Expected: PASS.

### Task 5: Make SFTP context menus surface-aware and transient-safe

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `tests/sftp_context_menu_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Confirm the current split-brain before fixing**

Read the current routing points:

```bash
rg -n "sync_assets_context_menu_state|sync_assets_toolbar_state|open_context_menu_for_target|copy-current-path|open-sftp-workspace" src ui
```

Expected: workspace SFTP still passes through assets overlay plumbing and quick-browser-only helpers.

**Step 2: Add a surface dimension to the SFTP context-menu contract**

The resolver/dispatcher must know whether the request comes from quick browser or workspace.

**Step 3: Snapshot selection/path/mutable state at open time**

Do not let render-time reads mix quick browser state into workspace menus.

**Step 4: Implement close-before-route behavior**

Any workspace click/right-click/tab switch should dismiss create popup and stale menus before continuing.

**Step 5: Fix blank-area menu contents**

- quick browser blank-area may keep `open-sftp-workspace`
- workspace blank-area must not include it
- workspace `copy-current-path` must use workspace path

**Step 6: Re-run context-menu and bootstrap tests**

Run:

```bash
cargo test --test sftp_context_menu_spec --test bootstrap_smoke -q
```

Expected: PASS.

### Task 6: Replace overlay-card states with a unified workspace content-state surface

**Files:**
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `tests/workspace_sftp_tab_contract_smoke.sh`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Make the disconnected-state tests fail for the current overlay-card model**

Lock out the current `state-overlay` + `overlay-card` approach.

**Step 2: Introduce an explicit content-mode projection**

Target shape:

```rust
enum WorkspaceSftpContentMode {
    Ready,
    Loading,
    Connecting,
    Empty,
    Disconnected,
    Error,
}
```

**Step 3: Render state views in-flow inside the file-area surface**

Keep header/toolbar/status bar visible. Only the content body swaps modes.

**Step 4: Wire `Reconnect` to the existing real reconnect path**

No terminal auto-open, no fake retry.

**Step 5: Re-run the relevant smoke/integration tests**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test bootstrap_smoke -q
```

Expected: PASS.

### Task 7: Finish responsive columns, status bar semantics, and theme-token enforcement

**Files:**
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`

**Step 1: Add failing tests for responsive column visibility**

At minimum, verify required columns remain legible and optional columns hide cleanly under narrow tiers.

Prefer software-render sampling or geometry assertions here, because a simple string grep cannot prove that `New Folder` is still visible or that a required column has not been clipped off-screen.

**Step 2: Extend the status bar projection**

Expose and render:

- connection state
- item count
- selected count
- current path
- binding label
- transfer center badge/count

**Step 3: Enforce theme-token-only styling**

The test should fail if raw Ayu hex values are introduced into `ui/shell/sftp-workspace-host.slint`.

**Step 4: Re-run visual contract tests**

Run:

```bash
cargo test --test sftp_workspace_tab_render_spec --test theme_semantic_token_contract_spec -q
```

Expected: PASS.

### Task 8: Run full verification before claiming completion

**Files:**
- No new files

**Step 1: Run the required command set fresh**

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test sftp_context_menu_spec -q
cargo test --test theme_semantic_token_contract_spec --test bootstrap_smoke -q
cargo check --workspace
```

**Step 2: If theme or interaction-sensitive code triggered warnings, run clippy too**

```bash
cargo clippy --workspace -- -D warnings
```

**Step 3: Re-read the requirements checklist**

Verify every A-G behavior from `docs/plans/2026-05-20-sftp-workspace-usability-hardening-requirements.md` has explicit evidence.

**Step 4: Run the manual acceptance scenarios**

Manually verify the 10 required user flows from the requirements doc.

**Step 5: Only then hand the work back**

If any verification fails, do not claim completion; loop back through TDD + debugging for that gap.

---

Plan complete and saved to `docs/plans/2026-05-20-sftp-workspace-usability-hardening-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - 我分 task 派发 fresh subagent，在当前会话逐个 review 落地。

**2. Parallel Session (separate)** - 在新的 worktree / 新窗口中，用 `superpowers:executing-plans` 按这个计划分批实现。

这轮按你的要求先停在文档，不进入正式代码修改。
