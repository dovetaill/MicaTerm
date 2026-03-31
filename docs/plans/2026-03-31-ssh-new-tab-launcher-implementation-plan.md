# SSH New Tab Launcher Implementation Plan

日期: 2026-03-31
执行者: Codex
状态: 方案已确认，待进入实现

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current quick-launch dashboard with a lightweight launcher tab, add a Fluent new-tab button to the workspace tab strip, and let launcher actions open saved SSH connections by replacing the launcher tab with the real SSH session.

**Architecture:** Keep the existing `SessionManager` responsible only for real SSH sessions and model the launcher as a lightweight workspace tab projection owned by `ShellViewModel`. Reuse the existing saved-SSH asset tree and quick-launch recent preferences, simplify the Slint welcome surface into a launcher view, and add a dedicated saved-SSH picker modal that reuses `ConsoleAssetItem` / `AssetNodeRow` so modal browsing stays aligned with the existing assets tree.

**Tech Stack:** Rust, Slint, existing asset tree and session manager, Fluent icon assets, cargo tests, shell UI contract smoke scripts

---

### Task 1: Add Launcher Tab and SSH Picker State to the View Model

**Files:**
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/quick_launch_projection_spec.rs`

**Step 1: Write the failing tests**

Add tab and picker coverage that locks the new state model before touching UI:

```rust
#[test]
fn workspace_launcher_tab_projects_welcome_mode_without_runtime_session() {
    let mut view_model = ShellViewModel::default();

    view_model.open_workspace_launcher_tab();

    assert_eq!(view_model.workspace_tabs().len(), 1);
    assert!(view_model.workspace_tabs()[0].is_launcher());
    assert_eq!(view_model.workspace_session_host_mode(), "welcome");
}

#[test]
fn workspace_launcher_tab_is_singleton_when_opened_repeatedly() {
    let mut view_model = ShellViewModel::default();

    view_model.open_workspace_launcher_tab();
    view_model.open_workspace_launcher_tab();

    assert_eq!(view_model.workspace_tabs().len(), 1);
}

#[test]
fn ssh_picker_projection_filters_to_saved_ssh_assets_in_tree_order() {
    let (mut view_model, ids) = seeded_view_model();

    view_model.set_saved_ssh_picker_query("db".into());

    let items = view_model.saved_ssh_picker_items();
    assert_eq!(items.len(), 2); // folder + matching ssh asset
    assert_eq!(items[1].id, ids.db);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test workspace_tabs_spec --test quick_launch_projection_spec -q`

Expected: FAIL because launcher-tab helpers and saved-SSH picker projection APIs do not exist yet.

**Step 3: Write the minimal implementation**

Extend the workspace-tab projection so `ShellViewModel` can own one pseudo-tab that is not backed by a runtime session:

```rust
pub struct WorkspaceTab {
    pub session_id: String,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: String,
    pub error_detail: String,
    pub active: bool,
    pub kind: WorkspaceTabKind,
}

pub enum WorkspaceTabKind {
    Session,
    Launcher,
}
```

Add `ShellViewModel` state and helpers:

```rust
saved_ssh_picker_open: bool,
saved_ssh_picker_query: String,
saved_ssh_picker_selected_asset_id: Option<String>,

pub fn open_workspace_launcher_tab(&mut self) { /* singleton launcher tab */ }
pub fn close_workspace_launcher_tab(&mut self) { /* remove launcher tab if present */ }
pub fn active_workspace_launcher_tab(&self) -> Option<&WorkspaceTab> { /* ... */ }
pub fn open_saved_ssh_picker(&mut self) { /* open + seed selection */ }
pub fn close_saved_ssh_picker(&mut self) { /* reset query, preserve tree state */ }
pub fn set_saved_ssh_picker_query(&mut self, query: String) { /* search */ }
pub fn saved_ssh_picker_items(&self) -> Vec<ConsoleAssetItem> { /* ssh-only tree projection */ }
```

Key rules:

- launcher tab uses a stable synthetic id like `"workspace-launcher"`
- `workspace_session_host_mode()` returns `"welcome"` for launcher tabs
- real session behavior remains unchanged for `WorkspaceTabKind::Session`
- picker projection includes folders plus saved SSH assets only
- snippet and keychain nodes must not leak into the picker

**Step 4: Run tests to verify they pass**

Run: `cargo test --test workspace_tabs_spec --test quick_launch_projection_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/tabs.rs src/shell/view_model.rs tests/workspace_tabs_spec.rs tests/quick_launch_projection_spec.rs
git commit -m "feat: add launcher tab and ssh picker state"
```

### Task 2: Add the Fluent New-Tab Button to the Workspace Tab Strip

**Files:**
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing contract checks**

Extend the tab-strip smoke contract so it requires the new button contract:

```bash
grep -F 'callback new-tab-requested();' ui/shell/tabbar.slint >/dev/null
grep -F '@image-url("../../assets/icons/fluent/add-20-regular.svg")' ui/shell/tabbar.slint >/dev/null
grep -F 'callback workspace-new-tab-requested();' ui/app-window.slint >/dev/null
grep -F 'new-tab-requested => {' ui/shell/workspace-pane.slint >/dev/null
```

**Step 2: Run the smoke test to verify it fails**

Run: `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

Expected: FAIL because the tab strip does not yet expose the Fluent new-tab button or callback chain.

**Step 3: Write the minimal implementation**

Update the Slint contract:

```slint
export component TabBar inherits Rectangle {
    callback tab-selected(string);
    callback tab-close-requested(string);
    callback new-tab-requested();

    private property <image> new-tab-icon: @image-url("../../assets/icons/fluent/add-20-regular.svg");

    new-tab-button := Rectangle {
        width: 32px;
        height: 30px;
        // hover / pressed surface using existing ThemeTokens

        TouchArea {
            clicked => { root.new-tab-requested(); }
        }
    }
}
```

Forward the callback:

- `TabBar.new-tab-requested()` -> `WorkspacePane.workspace-new-tab-requested()`
- `WorkspacePane.workspace-new-tab-requested()` -> `AppWindow.workspace-new-tab-requested()`

Do not remove any existing tab-select or tab-close contracts.

**Step 4: Run the smoke test to verify it passes**

Run: `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/tabbar.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: add workspace new tab button"
```

### Task 3: Collapse WelcomeView into the New Tab Launcher

**Files:**
- Modify: `ui/welcome/welcome-view.slint`
- Modify: `ui/welcome/quick-launch-card.slint`
- Modify: `ui/welcome/quick-launch-section.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/quick_launch_ui_contract_smoke.sh`

**Step 1: Write the failing contract checks**

Replace the old dashboard assertions with launcher assertions:

```bash
grep -F 'text: "New Tab"' ui/welcome/welcome-view.slint >/dev/null
grep -F 'Open Saved SSH Connections' ui/welcome/welcome-view.slint >/dev/null
! grep -F 'QuickLaunchDetailPane' ui/welcome/welcome-view.slint
! grep -F 'search-query' ui/welcome/welcome-view.slint
grep -F 'callback open-saved-ssh-requested();' ui/welcome/welcome-view.slint >/dev/null
```

**Step 2: Run the smoke test to verify it fails**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: FAIL because the welcome view still renders the old multi-section dashboard.

**Step 3: Write the minimal implementation**

Refactor `WelcomeView` into a lightweight launcher surface:

```slint
export component WelcomeView inherits Rectangle {
    in property <[QuickLaunchCardRow]> recent-items: [];
    callback connect-requested(string);
    callback open-saved-ssh-requested();
}
```

Concrete UI changes:

- replace `Quick Start` with `New Tab`
- remove search shell entirely
- remove `Favorites`, `Groups`, `Group Focus`, and `QuickLaunchDetailPane`
- keep only one section: `Recent Connections`
- add a primary button: `Open Saved SSH Connections`
- change `QuickLaunchCard` so the primary click path triggers `activated(...)` directly
- remove selected-state-only styling from launcher cards

Forward the new callback chain:

- `WelcomeView.open-saved-ssh-requested()` -> `TerminalSessionHost`
- `TerminalSessionHost` -> `WorkspacePane`
- `WorkspacePane` -> `AppWindow`

Retain the recent-items data contract so the existing preference-backed MRU list still works.

**Step 4: Run the smoke test to verify it passes**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/welcome/welcome-view.slint ui/welcome/quick-launch-card.slint ui/welcome/quick-launch-section.slint ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/quick_launch_ui_contract_smoke.sh
git commit -m "feat: simplify welcome view into launcher"
```

### Task 4: Add the Saved SSH Picker Modal

**Files:**
- Create: `ui/components/open-saved-ssh-modal.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/quick_launch_ui_contract_smoke.sh`

**Step 1: Write the failing contract checks**

Add smoke assertions for the modal contract:

```bash
grep -F 'OpenSavedSshModal' ui/app-window.slint >/dev/null
grep -F 'callback open-saved-ssh-modal-close-requested();' ui/app-window.slint >/dev/null
grep -F 'callback open-saved-ssh-modal-query-changed(string);' ui/app-window.slint >/dev/null
grep -F 'callback open-saved-ssh-modal-asset-activated(string);' ui/app-window.slint >/dev/null
grep -F 'AssetNodeRow' ui/components/open-saved-ssh-modal.slint >/dev/null
```

**Step 2: Run the smoke test to verify it fails**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: FAIL because the saved-SSH picker modal does not exist yet.

**Step 3: Write the minimal implementation**

Create a focused modal that reuses existing asset-row rendering:

```slint
export component OpenSavedSshModal inherits Rectangle {
    in property <string> query: "";
    in property <[ConsoleAssetItem]> items: [];
    callback close-requested();
    callback query-changed(string);
    callback asset-selected(string);
    callback asset-activated(string);
    callback toggle-expanded-requested(string);
}
```

Implementation rules:

- wrap it in the existing `BlockingModalShell`
- title is `Open Saved SSH Connections`
- top search field filters the SSH tree
- list body uses `AssetNodeRow`
- single click selects
- double click activates
- footer only needs `Cancel`

Add modal properties and callbacks to `AppWindow`:

```slint
in-out property <bool> open-saved-ssh-modal-open: false;
in-out property <string> open-saved-ssh-modal-query: "";
in-out property <[ConsoleAssetItem]> open-saved-ssh-modal-items: [];
```

Render the modal only when `open-saved-ssh-modal-open` is true.

**Step 4: Run the smoke test to verify it passes**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/open-saved-ssh-modal.slint ui/app-window.slint tests/quick_launch_ui_contract_smoke.sh
git commit -m "feat: add saved ssh picker modal"
```

### Task 5: Wire Launcher Actions to Open SSH Sessions and Replace the Launcher Tab

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing integration tests**

Add end-to-end coverage for the new launcher flows:

```rust
#[test]
fn workspace_new_tab_request_opens_single_launcher_tab() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    app.invoke_workspace_new_tab_requested();
    app.invoke_workspace_new_tab_requested();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode(), "welcome");
}

#[test]
fn launcher_recent_connection_replaces_launcher_tab_with_real_session_tab() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "prod.example.com");

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_quick_launch_connect_requested(ssh_id.clone().into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let item = app.get_workspace_tab_items().row_data(0).unwrap();
    assert_ne!(item.session_id, "workspace-launcher");
    assert_eq!(item.title, "Prod Bastion");
}

#[test]
fn launcher_picker_activation_replaces_launcher_tab_and_closes_modal() {
    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);
    let ssh_id = create_root_ssh(&app, "DB Admin", "db.example.com");

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_open_saved_ssh_requested();
    app.invoke_open_saved_ssh_modal_asset_activated(ssh_id.clone().into());

    assert!(!app.get_open_saved_ssh_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test bootstrap_smoke --test workspace_tabs_spec -q`

Expected: FAIL because the bootstrap layer does not yet manage launcher tabs, picker modal state, or replacement semantics.

**Step 3: Write the minimal implementation**

Wire the new callbacks in `bootstrap.rs`:

```rust
window.on_workspace_new_tab_requested(move || {
    state.open_workspace_launcher_tab();
    sync_workspace_tabs(...);
});

window.on_welcome_open_saved_ssh_requested(move || {
    state.open_saved_ssh_picker();
    sync_saved_ssh_picker_state(...);
});

window.on_open_saved_ssh_modal_asset_activated(move |asset_id| {
    state.close_saved_ssh_picker();
    state.close_workspace_launcher_tab();
    open_saved_ssh_asset_from_quick_launch(..., asset_id.as_str(), OpenSessionMode::ForceNewTab);
    sync_workspace_tabs_with_manager(...);
});
```

Critical behavioral rules:

- opening from a launcher always uses `ForceNewTab`
- the launcher tab is removed immediately before or immediately after a successful open attempt, but never left behind on success
- the picker modal closes on activation and cancel
- a repeated `workspace-new-tab-requested` activates the existing launcher instead of duplicating it
- existing session-tab selection, close, reconnect, and follow logic must remain unchanged

Update state sync so `AppWindow` receives:

- launcher-aware `workspace-tab-items`
- saved-SSH picker modal `open/query/items`

**Step 4: Run tests to verify they pass**

Run: `cargo test --test bootstrap_smoke --test workspace_tabs_spec -q`

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "feat: wire launcher tab ssh open flow"
```

### Task 6: Run Full Verification for the Launcher Flow

**Files:**
- Modify as needed: any files touched by Tasks 1-5
- Test: `tests/quick_launch_ui_contract_smoke.sh`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/quick_launch_projection_spec.rs`

**Step 1: Run targeted Rust tests**

Run: `cargo test --test workspace_tabs_spec --test quick_launch_projection_spec --test bootstrap_smoke -q`

Expected: PASS

**Step 2: Run Slint/UI smoke contracts**

Run: `bash tests/quick_launch_ui_contract_smoke.sh`

Expected: PASS

Run: `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

Expected: PASS

**Step 3: Run formatter**

Run: `cargo fmt --all`

Expected: formatting completes with no diff-producing errors

**Step 4: Re-run the targeted suite after formatting**

Run: `cargo test --test workspace_tabs_spec --test quick_launch_projection_spec --test bootstrap_smoke -q`

Expected: PASS

**Step 5: Commit**

```bash
git add ui src tests docs/plans/2026-03-31-ssh-new-tab-launcher-design.md docs/plans/2026-03-31-ssh-new-tab-launcher-implementation-plan.md
git commit -m "feat: add ssh new tab launcher flow"
```
