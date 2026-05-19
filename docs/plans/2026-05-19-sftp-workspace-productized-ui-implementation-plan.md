# SFTP Workspace Productized UI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Productize the real SFTP workspace tab into a mature MicaTerm/Ayu file workspace with a compact header, icon toolbar, breadcrumb-first path bar, responsive table, bottom status bar, consistent state views, and an automatic right-panel collapse policy that prevents duplicate SFTP file lists.

**Architecture:** Keep `FileBrowserSession`, `WorkspaceTab::Sftp`, and the existing `workspace_sftp_*` projection chain as the single source of truth, but extend the projection layer with right-panel display policy semantics, richer render rows, and live SFTP summary state. Rebuild `ui/shell/sftp-workspace-host.slint` around existing callbacks, context-menu dispatch, modal flows, and transfer-center plumbing while continuing to consume runtime-projected Ayu shell/session properties instead of inventing a second palette in Slint.

**Tech Stack:** Rust, Slint, existing ShellViewModel projection loop, existing SFTP browser/controller/queue pipeline, existing Ayu runtime theme projection, existing transfer center and modal stack.

---

## Input Design

- Design baseline: `docs/plans/2026-05-19-sftp-workspace-productized-ui-design.md`
- Screenshot target: `/home/images/20260518.png`
- Current gap reference:
  - `/home/images/Snipaste_2026-05-19_15-14-15.png`
  - `/home/images/Snipaste_2026-05-19_15-15-13.png`
- Must preserve:
  - terminal-first product positioning
  - lightweight right-side quick browser
  - full-featured center SFTP workspace
  - runtime-projected Ayu palette chain
  - subtle fill + accent rail selection treatment
  - existing real SFTP session binding and async/background SFTP work model

## Execution Notes

- 实现必须在新的 `.worktrees` 工作目录中执行，不在当前文档会话直接动现有工作树的功能代码。
- 每个 task 先走 `@superpowers:test-driven-development`：先写失败测试 / contract smoke，再做最小实现，再跑通过。
- 如果 UI 合同、Slint callback、projection policy 或 context menu 行为出现偏差，立刻切 `@superpowers:systematic-debugging`，不要叠补丁猜修。
- `workspace_sftp_*` 现有 projection 是主干，不允许重造第二套 workspace-only backend path。
- quick browser 与 workspace SFTP 仍必须基于不同 `FileBrowserSession` snapshot；禁止退回共享同一个活动会话。
- `ui/theme/tokens.slint` 只有在现有 semantic token 明确不足时才允许补最小 token；禁止在 `ui/shell/sftp-workspace-host.slint` 内写新 hex palette。

## Task Sequence Overview

1. Freeze the new SFTP workspace UI and projection contracts in tests.
2. Add right-panel display policy semantics for active SFTP workspace tabs.
3. Extend workspace SFTP projection rows and summary state without breaking real bindings.
4. Rebuild `SftpWorkspaceHost` into a compact Ayu workspace surface.
5. Rework right-panel / shell chrome integration so duplicate SFTP lists disappear while transfer center remains reachable.
6. Expand workspace SFTP context/menu/modal contracts onto the productized host without creating a fake action path.
7. Verify the focused test suite and compile checks, then hand off to an execution session.

### Task 1: Freeze the new SFTP workspace UI and projection contracts in tests

**Files:**
- Modify: `tests/workspace_sftp_tab_contract_smoke.sh`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`
- Modify: `tests/workspace_sftp_projection_spec.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`

**Step 1: Replace the old smoke script with a workspace UI contract smoke**

The shell script should stop checking only `WorkspaceTabId` / `browser_session.rs` existence and instead assert source contracts for:

- compact workspace header markers
- icon toolbar markers
- breadcrumb/path region markers
- file table markers
- bottom status bar markers
- absence of `New Folde`
- absence of duplicate right-side SFTP file list behavior when workspace SFTP is active

```bash
grep -F 'workspace-header :=' ui/shell/sftp-workspace-host.slint >/dev/null
grep -F 'workspace-toolbar :=' ui/shell/sftp-workspace-host.slint >/dev/null
grep -F 'workspace-breadcrumb-shell :=' ui/shell/sftp-workspace-host.slint >/dev/null
grep -F 'workspace-file-table :=' ui/shell/sftp-workspace-host.slint >/dev/null
grep -F 'workspace-statusbar :=' ui/shell/sftp-workspace-host.slint >/dev/null
! grep -F 'New Folde' ui/shell/sftp-workspace-host.slint >/dev/null
```

**Step 2: Add render/source assertions for the productized host**

`tests/sftp_workspace_tab_render_spec.rs` should fail until the source includes:

- compact header structure
- icon-first toolbar assets
- breadcrumb root handling for `/`
- selected subtle fill + accent rail token usage
- responsive optional-column hide logic

Add assertions for contracts such as:

```rust
assert!(source.contains("workspace-header :="));
assert!(source.contains("workspace-statusbar :="));
assert!(source.contains("sidebar-item-selected-background"));
assert!(source.contains("sidebar-item-selected-border"));
assert!(source.contains("root.workspace-width-tier()"));
```

**Step 3: Extend projection tests for right-panel display policy**

`tests/workspace_sftp_projection_spec.rs` should lock:

- active workspace kind `SFTP` => right panel display policy hides duplicate SFTP quick browser
- leaving the SFTP tab restores the previous quick-browser request state
- workspace SFTP still resolves a distinct `file_browser_session_id`

**Step 4: Extend theme-token contract coverage**

`tests/theme_semantic_token_contract_spec.rs` should fail if:

- `ui/shell/sftp-workspace-host.slint` contains raw Ayu hex colors
- SFTP row selection stops using semantic selected/focus/accent tokens
- the productized workspace starts reading a detached second palette ladder

**Step 5: Run the new tests and confirm they fail first**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test workspace_tabs_spec -q
cargo test --test theme_semantic_token_contract_spec -q
```

Expected: failing assertions that describe the missing compact host, policy-hidden right panel semantics, and token contracts.

### Task 2: Add right-panel display policy semantics for active SFTP workspace tabs

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`

**Step 1: Confirm the current layout only knows visible vs hidden**

Run:

```bash
sed -n '1,120p' src/shell/layout.rs
sed -n '100,190p' src/shell/view_model/projection.rs
```

Expected: only `show_right_panel` and `effective_show_right_panel` exist; there is no reason code for policy-hidden vs user-collapsed.

**Step 2: Introduce a right-panel hidden-reason / display-policy projection**

Add a small enum or string id in the view model that can distinguish:

- `visible`
- `user-collapsed`
- `policy-hidden-sftp-workspace`

For example:

```rust
pub enum RightPanelDisplayPolicy {
    Visible,
    UserCollapsed,
    PolicyHiddenForSftpWorkspace,
}
```

**Step 3: Teach the policy to hide duplicate SFTP quick browser lists**

The computed policy should treat the right panel as `policy-hidden-sftp-workspace` when:

- `active_workspace_tab().kind == WorkspaceTabKind::Sftp`
- `right_panel_view == RightPanelView::Sftp`

Keep the user's requested visibility state in memory so leaving the SFTP tab can restore the quick browser automatically.

**Step 4: Expose the policy to Slint**

Thread new properties into `ui/app-window.slint`, such as:

- `right-panel-display-policy`
- `right-panel-can-revive`

`right-panel-can-revive` must be `false` for policy-hidden SFTP workspace state so the UI does not offer a misleading revive strip for the duplicate browser.

**Step 5: Keep the titlebar / active summary SFTP-aware**

Update summary projection so an active SFTP tab can surface host-first metadata from the live `FileBrowserSession`, rather than only the static `WorkspaceTab::sftp(..., title)` shell:

```rust
ActiveWorkspaceTabSummary {
    primary_summary_text: format!("{host} · SFTP"),
    connection_status_label: workspace_sftp_connection_label().to_string(),
    // ...
}
```

**Step 6: Re-run projection tests**

Run:

```bash
cargo test --test workspace_sftp_projection_spec --test workspace_tabs_spec -q
```

Expected: the right-panel policy tests now pass while terminal-tab behavior remains unchanged.

### Task 3: Extend workspace SFTP projection rows and summary state without breaking real bindings

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`

**Step 1: Expand `SftpPanelRenderRow` with productized table data**

Add the minimum new fields needed by the workspace host:

```rust
pub struct SftpPanelRenderRow {
    pub id: String,
    pub name: String,
    pub meta_label: String,
    pub type_label: String,
    pub modified_label: String,
    pub size_label: String,
    pub permissions_label: String,
    pub owner_label: String,
    pub group_label: String,
    pub icon_kind: String,
    pub kind: String,
    pub selected: bool,
}
```

Do not drop or rename the current fields used by quick browser and workspace.

**Step 2: Populate the new fields from real `SftpDirectoryEntry` data**

Update `build_sftp_panel_render_row(...)` so:

- `icon_kind` matches the existing entry classification
- `permissions_label` comes from remote mode bits if present
- `owner_label` / `group_label` come from entry metadata if present
- parent row gets an explicit parent icon kind

**Step 3: Extend the Slint-facing `SftpPanelItem` contract**

Update the `SftpPanelItem` struct in `ui/shell/right-panel.slint` and its Rust projection helper so workspace and quick browser can both read the richer row contract.

**Step 4: Add workspace summary accessors for bottom status and state views**

Add minimal getters on `ShellViewModel` for:

- total row count
- selected row count
- current host/status labels
- optional read-only / last-path / transfer count strings if the UI needs them

Keep them derived from the active workspace SFTP session; do not create fake demo counters.

**Step 5: Stop full model churn in `sync_workspace_sftp_state()`**

Replace unconditional `replace_sftp_panel_items_model(...)` with the same dirty-index / full-resync helper already used by the quick browser when feasible. The goal is to reduce jitter during selection and refresh while preserving the existing virtualization contract.

**Step 6: Re-run the render and projection tests**

Run:

```bash
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec -q
```

Expected: row/icon/summary contracts compile and the tests now see the richer productized projection fields.

### Task 4: Rebuild `SftpWorkspaceHost` into a compact Ayu workspace surface

**Files:**
- Modify: `ui/shell/sftp-workspace-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/app-window.slint`
- Reference: `/home/images/20260518.png`

**Step 1: Replace the title-card stack with a compact workspace header**

Add stable source markers so the smoke tests can assert them:

- `workspace-header :=`
- `workspace-toolbar :=`
- `workspace-breadcrumb-shell :=`
- `workspace-file-table :=`
- `workspace-statusbar :=`

The header should show:

- primary host label
- secondary `SFTP · Locked · Manual` style copy
- compact dot+label status
- optional lock badge / transfer badge

**Step 2: Convert the toolbar to Fluent icon actions**

Use existing icon assets such as:

- `arrow-hook-up-left-20-regular.svg` or equivalent back arrow asset already in repo
- `arrow-sync-20-regular.svg`
- `folder-20-regular.svg`
- `arrow-upload-20-regular.svg`
- `edit-20-regular.svg`

If a required icon is missing, add it under `assets/icons/fluent` instead of falling back to text-only buttons.

**Step 3: Merge toolbar and path bar into one chrome band**

The default state is breadcrumb-first. Editable path input only appears after:

- click on the edit affordance, or
- explicit callback trigger

The root path `/` must render as a stable breadcrumb shell with a root crumb.

**Step 4: Rebuild the file table with responsive optional columns**

Implement width-tier helpers in Slint, for example:

```slint
function workspace-width-tier() -> string {
    if root.width < 980px { return "compact"; }
    if root.width < 1120px { return "medium"; }
    return "wide";
}
```

Use that tier to hide:

- `Group` first
- then `Owner`
- then `Permissions`

Never hide `Name`, `Type`, `Modified`, or `Size`.

**Step 5: Add productized row states**

Rows must use:

- file/folder icon cell
- subtle hover fill
- subtle selected fill
- left accent rail
- weak separator hairlines

Do not reintroduce thick orange outlines or card-like row boxes.

**Step 6: Add explicit centered state views and bottom status bar**

Handle:

- loading
- empty directory
- error
- disconnected
- read-only

The bottom status bar should show item count, selected count, host/status, and a transfer entry.

**Step 7: Re-run the render/source tests**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test sftp_workspace_tab_render_spec -q
```

Expected: the productized host source now satisfies the compact-header, icon-toolbar, responsive-table, and bottom-status contracts.

### Task 5: Rework right-panel / shell chrome integration so duplicate SFTP lists disappear while transfer center remains reachable

**Files:**
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/shell/view_model/projection.rs`

**Step 1: Make the right panel distinguish policy-hidden from user-collapsed**

When `right-panel-display-policy == "policy-hidden-sftp-workspace"`:

- do not reserve right-panel width
- do not render the revive strip
- do not expose the duplicate list host

But still allow transfer center to open and place itself correctly.

**Step 2: Keep the quick browser behavior unchanged for terminal tabs**

Switching back to a terminal tab should restore:

- the user's requested right-panel visibility
- quick-browser width
- current quick-browser session

This must not accidentally clear the user's remembered right-panel preference.

**Step 3: Make the transfer-center anchor independent of duplicate quick-browser visibility**

Verify the transfer center still places itself correctly when:

- right panel is visible
- right panel is user-collapsed
- right panel is policy-hidden for active SFTP workspace

**Step 4: Re-run policy and smoke tests**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
cargo test --test workspace_sftp_projection_spec --test workspace_tabs_spec -q
```

Expected: no duplicate right-side file list is rendered for active SFTP workspace tabs, and terminal tabs still keep the quick browser.

### Task 6: Expand workspace SFTP menu / modal / transfer contracts onto the productized host

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `ui/app-window.slint`
- Modify: `tests/sftp_workspace_tab_render_spec.rs`

**Step 1: Verify the current dispatcher already carries real SFTP actions**

Run:

```bash
sed -n '574,620p' src/shell/view_model.rs
sed -n '252,340p' src/shell/view_model/context_menu_dispatcher.rs
```

Expected: `PendingSftpContextAction` already supports open, edit, refresh, create file/folder, rename, delete, upload, and download flows.

**Step 2: Fill the requested workspace context-menu shape**

Update `resolve_sftp_*_actions(...)` so the workspace surface can expose stable groupings for:

- Open
- Open With
- Edit
- Download
- Upload Here
- New Folder
- New File
- Rename
- Delete
- Copy Path
- Refresh
- Properties

Unsupported actions must stay disabled/planned, not fake-enabled.

**Step 3: Keep SFTP modals routed through the shared modal system**

Continue using:

- `AssetModalState::SftpNewFolder`
- `AssetModalState::SftpNewFile`
- `AssetModalState::SftpRenameEntry`
- `AssetModalState::SftpDeleteEntriesConfirm`
- `SftpRemoteFileModal`

The workspace host only triggers existing callbacks; it must not introduce a second modal state machine.

**Step 4: Make workspace upload/download actions visibly feed the transfer center**

After queueing an upload/download from workspace actions:

- open or badge the transfer center using existing state
- keep the main table interactive

**Step 5: Re-run the focused workspace tests**

Run:

```bash
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec -q
```

Expected: context-menu structure and transfer-entry wiring now satisfy the workspace contract.

### Task 7: Verify the focused test suite and compile checks, then hand off

**Files:**
- Modify: any touched files above as needed

**Step 1: Run the smoke contract**

Run:

```bash
bash tests/workspace_sftp_tab_contract_smoke.sh
```

Expected: PASS

**Step 2: Run focused Rust tests**

Run:

```bash
cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test workspace_tabs_spec -q
cargo test --test theme_semantic_token_contract_spec -q
```

Expected: PASS

**Step 3: Run workspace compile verification**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 4: Manual verification checklist**

- Open an SFTP workspace tab and confirm the right-side SFTP quick browser auto-hides.
- Switch back to a terminal tab and confirm the quick browser visibility is restored if the user had it open before.
- Confirm the workspace header is compact and does not repeat `Files: Interserver` inside the main content area.
- Confirm toolbar buttons are icon-first and no label is truncated.
- Confirm `/` breadcrumb layout stays stable and editable path mode does not clip.
- Confirm `Size` no longer sticks to the right edge and optional columns hide before required columns clip.
- Confirm selected rows use subtle fill + accent rail, not a heavy orange box.
- Confirm upload/download actions enter the global transfer center rather than embedding a queue in the table.
- Confirm disconnect/error/read-only states use the new centered state view plus compact header status language.

**Step 5: Capture implementation screenshots in the execution session**

- Compare against `/home/images/20260518.png`
- Save updated proof screenshots in a temporary review location
- Validate the final result looks like MicaTerm/Ayu shell chrome, not a temporary Slint form

**Step 6: Commit when the execution worktree is green**

```bash
git add docs/plans/2026-05-19-sftp-workspace-productized-ui-design.md \
        docs/plans/2026-05-19-sftp-workspace-productized-ui-implementation-plan.md \
        ui/shell/sftp-workspace-host.slint \
        ui/shell/workspace-pane.slint \
        ui/shell/right-panel.slint \
        ui/shell/tabbar.slint \
        ui/app-window.slint \
        ui/theme/tokens.slint \
        src/app/bootstrap/sftp.rs \
        src/app/bootstrap/shell_chrome.rs \
        src/shell/view_model.rs \
        src/shell/view_model/projection.rs \
        src/shell/view_model/sftp.rs \
        src/shell/view_model/workspace.rs \
        src/shell/context_menu.rs \
        src/shell/view_model/context_menu_dispatcher.rs \
        src/shell/tabs.rs \
        tests/workspace_sftp_tab_contract_smoke.sh \
        tests/sftp_workspace_tab_render_spec.rs \
        tests/workspace_sftp_projection_spec.rs \
        tests/workspace_tabs_spec.rs \
        tests/theme_semantic_token_contract_spec.rs
git commit -m "feat: productize sftp workspace ui"
```

Plan complete and saved to `docs/plans/2026-05-19-sftp-workspace-productized-ui-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session in the target `.worktrees` directory and use `superpowers:executing-plans` to implement this plan task-by-task

Which approach?
