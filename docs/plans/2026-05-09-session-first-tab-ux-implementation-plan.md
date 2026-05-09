# Session-First Tab UX Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the workspace tab strip into a session-first tab management bar with stable tab identity, UI-only drag reorder, top active-session info, hover tooltips, and a safe tab context menu without rebuilding SSH sessions or terminal surfaces.

**Architecture:** Keep `SessionManager` as the source of truth for live session lifecycle and terminal surfaces, but add a separate tab presentation layer in `ShellViewModel` that owns `tab_id`, tab metadata, and UI order. Extend the Slint tab/titlebar contracts to display structured active-session info, tooltips, drag feedback, and a lightweight context menu, while routing close/reconnect/clone actions through explicit tab commands that preserve session semantics.

**Tech Stack:** Rust, Slint, existing SSH/session manager, existing workspace projection loop, existing tooltip/popup/clipboard patterns.

---

### Task 1: Stabilize the tab model and stop overloading session identity

**Files:**
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/tabbar.slint`

**Step 1: Verify the current tab/item identity mismatch still exists**
Run: `sed -n '1,120p' ui/shell/tabbar.slint && sed -n '2010,2025p' src/app/bootstrap.rs`
Expected: `WorkspaceTabItem.session_id` exists and `src/app/bootstrap.rs` feeds `tab.tab_id` into that field.

**Step 2: Expand `WorkspaceTab` into a session-first metadata container**
- Add structured fields for `display_name`, `host`, `username`, `port`, `connection_status`, and reconnect/clone metadata.
- Keep `tab_id` as the stable tab container id.
- Keep `session_id` as the current live SSH session id when one exists.
- Preserve `kind`, `asset_id`, `enhanced_session_state`, and `error_detail`.

```rust
pub struct WorkspaceTab {
    pub tab_id: WorkspaceTabId,
    pub session_id: String,
    pub asset_id: String,
    pub display_name: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    pub connection_status: String,
    // ... existing fields retained as needed
}
```

**Step 3: Rename the Slint tab item contract away from `session_id`**
- Replace the misleading `WorkspaceTabItem.session_id` field with `tab_id`.
- Update Rust projection code so UI callbacks always return `tab_id`.

**Step 4: Add tab metadata accessors on the view-model side**
- Add helper methods for `active_workspace_tab_summary`, `tab_index_by_id`, and `workspace_tab_by_id`.
- Keep these helpers keyed by `tab_id`, never by index.

**Step 5: Verify the UI contract is no longer mislabeled**
Run: `rg -n "WorkspaceTabItem.*session_id|tab-selected\(session-id\)|tab-close-requested\(session-id\)" src ui`
Expected: no remaining tab-strip identity wiring uses `session_id` as the UI key.

### Task 2: Add UI-only tab order state and preserve it across manager projection

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `src/app/bootstrap.rs`

**Step 1: Confirm the manager projection currently rebuilds visible tab order from session order**
Run: `sed -n '42,115p' src/app/bootstrap/workspace_terminal.rs`
Expected: `sync_workspace_projection_from_manager()` builds `next_tabs` directly from `manager.ordered_sessions()`.

**Step 2: Add a dedicated tab presentation order state**
- Introduce `workspace_tab_order: Vec<String>` in `ShellViewModel`.
- Normalize the order whenever tabs are added/removed so it contains every live `tab_id` exactly once.
- Keep launcher, SFTP, preserved error tabs, and manager-backed terminal tabs in the same order table.

**Step 3: Add reorder APIs to the view model**
- Implement `reorder_workspace_tab(tab_id, target_index)`.
- Implement helpers that compute left/right/others sets from the current UI order.

```rust
pub fn reorder_workspace_tab(&mut self, tab_id: &str, target_index: usize) -> bool {
    // remove tab_id from current order, clamp target, insert once, normalize, keep active_tab_id unchanged
}
```

**Step 4: Merge manager projection into existing tab order instead of overwriting it**
- Update `sync_workspace_projection_from_manager()` to merge new manager-backed session tabs into existing `workspace_tabs`, then sort by `workspace_tab_order`.
- Append truly new tabs to the end of `workspace_tab_order`.
- Do not write drag order back to `SessionManager.open_order`.

**Step 5: Add regression tests for order stability and active preservation**
- Add tests near the existing workspace projection tests in `src/app/bootstrap.rs`.
- Cover: drag reorder survives a projection tick; active tab id is unchanged after reorder; close fallback still picks right neighbor then left neighbor based on UI order.

**Step 6: Verify the new order API and tests are discoverable**
Run: `rg -n "reorder_workspace_tab|workspace_tab_order|right neighbor|projection tick" src/app/bootstrap.rs src/shell`
Expected: matches show the new order state, reorder method, and regression tests.

### Task 3: Surface structured active-session info in the titlebar and tab tooltips

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/titlebar-tooltip.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap.rs`

**Step 1: Add titlebar properties for active session summary and tooltip text**
- Add properties for active display name, host, status, and summary tooltip.
- Keep this summary in the chrome/header area, not the terminal content host.

**Step 2: Extend `ActiveTab` and `TabBar` with hover tooltip plumbing**
- Add `tab-hovered(tab_id, anchor_x, anchor_y)` and `tab-hover-ended(tab_id)` callbacks.
- Keep `ActiveTab` single-line and ellipsized; do not add a second subtitle row.

**Step 3: Render the active-session summary lane beside the logo**
- Place the new text in `ui/shell/titlebar.slint` between the brand block and the drag/utility controls.
- Ensure narrow-width behavior prefers keeping the active tab name readable before host/status.

**Step 4: Reuse the lightweight tooltip surface for tabs and the active summary**
- Use the existing tooltip language and placement logic.
- Show: full tab name, host, username, port, connection status.
- Suppress tooltip while dragging or while the tab context menu is open.

**Step 5: Verify the summary/tooltip properties exist in the UI contract**
Run: `rg -n "active-session|tab-hovered|tab-hover-ended|tooltip" ui/shell/titlebar.slint ui/shell/tabbar.slint ui/components/active-tab.slint ui/app-window.slint`
Expected: the new summary lane and tab tooltip callbacks are wired through the Slint files.

### Task 4: Add a dedicated tab context menu overlay and session-safe tab commands

**Files:**
- Create: `ui/components/workspace-tab-context-menu.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `src/shell/view_model/workspace.rs`

**Step 1: Verify the project already has reusable overlay/menu patterns**
Run: `sed -n '1,220p' ui/components/assets-context-menu-overlay.slint && sed -n '1,120p' ui/components/titlebar-tooltip.slint`
Expected: FocusScope/Esc/outside-click menu patterns are available to reuse visually.

**Step 2: Add tab context menu state to the view model**
- Create a dedicated menu state with `open`, `anchor_tab_id`, `anchor_x`, `anchor_y`, and computed enabled/disabled flags.
- Do not reuse the assets/sidebar context menu state.

**Step 3: Create the tab context menu Slint component**
- Add rows for `Reconnect`, `Clone Connection`, `Close`, `Copy Name`, `Copy Host`, `Close Others`, `Close All`, `Close Tabs to the Right`, and `Close Tabs to the Left`.
- Support hover, disabled rows, pointer close-out, and `Escape`.

**Step 4: Add explicit tab commands in Rust**
- Route all tab menu actions by `tab_id`.
- Implement `close_tab`, `close_others`, `close_left`, `close_right`, `close_all`, `copy_name`, `copy_host`, `reconnect_tab`, and `clone_connection` entry points.
- Ensure victim sets are frozen from the current UI order before mutating tabs.

```rust
enum WorkspaceTabCommand {
    Close { tab_id: String },
    CloseOthers { tab_id: String },
    CloseLeft { tab_id: String },
    CloseRight { tab_id: String },
    CloseAll,
    CopyName { tab_id: String },
    CopyHost { tab_id: String },
    Reconnect { tab_id: String },
    CloneConnection { tab_id: String },
}
```

**Step 5: Keep reconnect and close semantics session-safe**
- Manager-backed terminal tabs should use `retry_session(session_id)` for reconnect.
- Error/synthetic tabs should reopen from saved connection metadata into the same `tab_id` slot.
- Close flows must continue through `close_workspace_tab_by_id()` so SFTP-linked tabs and hidden terminal sessions keep their existing cleanup semantics.

**Step 6: Add unit tests for menu enablement and batch target calculation**
- Cover one-tab disablement.
- Cover left/right disablement on edge tabs.
- Cover active fallback after `Close Others`, `Close Left`, `Close Right`, and `Close All`.

### Task 5: Add drag-and-drop reorder feedback without changing session lifetime

**Files:**
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Test: `src/app/bootstrap.rs`

**Step 1: Add drag state and callbacks to the Slint tab strip**
- Track pointer-down origin, drag threshold, dragged tab id, and current insertion target.
- Emit callbacks such as `workspace-tab-reorder-previewed(tab_id, target_index)` only for UI preview if needed, and `workspace-tab-reorder-requested(tab_id, target_index)` on drop.

**Step 2: Add restrained drag visuals**
- Use a slightly raised active surface and reduced opacity on the dragged tab.
- Add a thin insertion line between tabs.
- Keep inactive/active sizing, close button affordances, and separators aligned with the current Mica/Fluent language.

**Step 3: Prevent click-to-select from firing once drag threshold is crossed**
- Normal click still selects the tab.
- Once dragging starts, releasing should only reorder.

**Step 4: Persist the final drop through `workspace_tab_order`**
- On drop, call `reorder_workspace_tab(tab_id, target_index)`.
- Re-sync the tab list after reorder without touching session lifetime or active session binding.

**Step 5: Add regression coverage for drag semantics**
- Add tests that verify reorder changes UI order only.
- Verify `active_workspace_tab_id` remains unchanged after reorder.
- Verify manager projection does not immediately snap the order back.

### Task 6: Verify end-to-end behavior and prepare the implementation handoff

**Files:**
- Modify: any touched files above as needed
- Test: `src/app/bootstrap.rs`
- Test: `src/shell/view_model/workspace.rs`

**Step 1: Run formatting**
Run: `cargo fmt --all --manifest-path Cargo.toml`
Expected: formatting completes cleanly.

**Step 2: Run targeted compile verification**
Run: `cargo test -q --manifest-path Cargo.toml --no-run`
Expected: the workspace compiles.

**Step 3: Run focused tab/workspace tests**
Run: `cargo test -q --manifest-path Cargo.toml workspace_tab -- --nocapture`
Expected: new tab-order, close-fallback, and reconnect-related tests pass.

**Step 4: Manual verification checklist**
- Open 10+ SSH tabs and confirm active-summary info shows full name/host/status.
- Hover any tab and confirm tooltip shows structured details.
- Right-click an inactive tab and confirm the menu opens at the cursor without switching active tabs.
- Verify `Copy Name` and `Copy Host` copy the correct data.
- Verify `Reconnect` reuses the same tab instead of creating a duplicate.
- Verify `Close Others`, `Close Left`, `Close Right`, and `Close All` operate on the correct UI-order ranges.
- Verify `Close All` lands in empty state instead of leaving an invalid active tab.
- Verify drag reorder preserves terminal content, session identity, and active session.

**Step 5: Commit the implementation when complete**
```bash
git add docs/plans/2026-05-09-session-first-tab-ux-design.md \
        docs/plans/2026-05-09-session-first-tab-ux-implementation-plan.md \
        src/shell/tabs.rs \
        src/shell/view_model.rs \
        src/shell/view_model/workspace.rs \
        src/app/bootstrap.rs \
        src/app/bootstrap/workspace_terminal.rs \
        src/app/bootstrap/shell_chrome.rs \
        ui/app-window.slint \
        ui/shell/titlebar.slint \
        ui/shell/workspace-pane.slint \
        ui/shell/tabbar.slint \
        ui/components/active-tab.slint \
        ui/components/titlebar-tooltip.slint \
        ui/components/workspace-tab-context-menu.slint

git commit -m "feat: upgrade workspace tabs to session-first UX"
```
