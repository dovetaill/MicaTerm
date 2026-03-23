# Windows Console 资产列表右键菜单 Bugfix2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `Windows Console` 资产区收敛到 design doc 选定的最终形态：无标题紧凑菜单、统一 action metadata、动态 toolbar、统一空查询搜索 dismiss、显式 rename session，以及同类型资产唯一命名。

**Architecture:** 保持当前 `Rust state -> bootstrap bridge -> Slint shell` 架构不变。Rust 继续作为菜单动作、toolbar descriptor、rename session、唯一命名和 dismiss policy 的真相源；Slint 只负责表面层渲染、pointer/focus 事件上报和 tooltip 呈现。实现顺序按 TDD 拆分为菜单 metadata、菜单 surface、toolbar 语义、空查询搜索 dismiss、rename + naming、全量验证六个任务，避免多处状态同时漂移。

**Tech Stack:** Rust 2024, Slint 1.15.1, `winit + femtovg-wgpu`, `TouchArea`, `FocusScope`, `TextInput`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- Design source: `docs/plans/2026-03-18-windows-console-assets-context-menu-bugfix2-design.md`
- REQUIRED before coding: `@superpowers:test-driven-development`
- If a task produces event-order bugs or focus regressions, switch immediately to `@superpowers:systematic-debugging`
- Execute in a dedicated worktree if possible; if not, do not mix this work with unrelated edits
- Do not expand scope into SSH form editing, persistence, terminal runtime, or renderer work
- Keep commits small and task-scoped; each task below ends with a recommended commit message

## Target Files

### Rust

- Modify: `src/shell/assets.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/sidebar.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`

### Slint

- Modify: `ui/app-window.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `ui/components/assets-context-menu-row.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Delete: `ui/components/assets-create-menu.slint`

### Assets

- Create: `assets/icons/fluent/delete-20-regular.svg`
- Create: `assets/icons/fluent/edit-20-regular.svg`
- Create: `assets/icons/fluent/copy-20-regular.svg`
- Create: `assets/icons/fluent/cut-20-regular.svg`
- Create: `assets/icons/fluent/arrow-clockwise-20-regular.svg`
- Create: `assets/icons/fluent/arrow-upload-20-regular.svg`
- Create: `assets/icons/fluent/arrow-download-20-regular.svg`

### Tests

- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/sidebar_assets_smoke.sh`
- Modify: `tests/sidebar_tooltip_ui_contract_smoke.sh`

### Docs

- Modify: `verification.md`

## Task 1: Introduce Context-Menu Action Metadata And Icon Coverage

**Files:**
- Create: `assets/icons/fluent/delete-20-regular.svg`
- Create: `assets/icons/fluent/edit-20-regular.svg`
- Create: `assets/icons/fluent/copy-20-regular.svg`
- Create: `assets/icons/fluent/cut-20-regular.svg`
- Create: `assets/icons/fluent/arrow-clockwise-20-regular.svg`
- Create: `assets/icons/fluent/arrow-upload-20-regular.svg`
- Create: `assets/icons/fluent/arrow-download-20-regular.svg`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-context-menu-row.slint`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/sidebar_assets_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_context_menu_spec.rs`, add a metadata-level test that proves visible actions now carry concrete labels and icons:

```rust
#[test]
fn blank_area_actions_expose_label_and_icon_metadata() {
    let roots = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            selected_ids: Vec::new(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            target_has_active_connection: false,
        },
    );

    assert_eq!(roots[0].label, "New Folder");
    assert_eq!(roots[0].icon_id, "folder");
    assert_eq!(roots[1].label, "New SSH Connection");
    assert_eq!(roots[1].icon_id, "window-console");
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, replace the legacy title-only check with icon-row checks:

```bash
grep -F 'in property <image> icon-source;' "$MENU_ROW" >/dev/null
grep -F 'icon-slot := Rectangle {' "$MENU_ROW" >/dev/null
! grep -F 'Text { text: "操作";' "$MENU_COLUMN" >/dev/null
```

In `tests/sidebar_assets_smoke.sh`, add all newly visible icon assets to the required file list.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_spec -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/sidebar_assets_smoke.sh
```

Expected:

- Rust test fails because `ContextMenuActionNode` does not yet expose `label` / `icon_id`
- UI contract smoke fails because `AssetsContextMenuRow` still renders only text
- Asset smoke fails because the new icon files do not exist yet

**Step 3: Write the minimal implementation**

In `src/shell/context_menu.rs`, extend the action model and helper constructors:

```rust
pub struct ContextMenuActionNode {
    pub id: &'static str,
    pub label: &'static str,
    pub icon_id: &'static str,
    pub state: ContextMenuActionState,
    pub children: Vec<ContextMenuActionNode>,
    pub divider_before: bool,
}

fn action_with_state(
    id: &'static str,
    label: &'static str,
    icon_id: &'static str,
    state: ContextMenuActionState,
    divider_before: bool,
) -> ContextMenuActionNode {
    ContextMenuActionNode { id, label, icon_id, state, children: Vec::new(), divider_before }
}
```

Use concrete icon ids for every visible action. Reuse existing icons where possible:

```rust
action_with_state("new-folder", "New Folder", "folder", ContextMenuActionState::Enabled, divider_before)
action_with_state("new-ssh-connection", "New SSH Connection", "window-console", ContextMenuActionState::Enabled, false)
action_with_state("rename-asset", "Rename", "edit", mutable_selection_state(selection), false)
action_with_state("copy-asset", "Copy", "copy", selection_state(selection), false)
```

In `src/app/bootstrap.rs`, extend `AssetsContextMenuItem` projection to carry icon identity through the bridge.

In `ui/components/assets-context-menu-row.slint`, render a dedicated icon slot before the label:

```slint
in property <image> icon-source;

icon-slot := Rectangle {
    x: 12px;
    y: (parent.height - 20px) / 2;
    width: 20px;
    height: 20px;

    Image {
        width: 20px;
        height: 20px;
        source: root.icon-source;
        image-fit: contain;
        colorize: ThemeTokens.text-primary;
    }
}

label-text := Text {
    x: 38px;
    width: parent.width - 50px;
    text: root.label;
}
```

Vendor the missing Fluent SVGs into the exact file paths listed above. Do not leave TODO placeholders.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_context_menu_spec -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/sidebar_assets_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add assets/icons/fluent/delete-20-regular.svg \
  assets/icons/fluent/edit-20-regular.svg \
  assets/icons/fluent/copy-20-regular.svg \
  assets/icons/fluent/cut-20-regular.svg \
  assets/icons/fluent/arrow-clockwise-20-regular.svg \
  assets/icons/fluent/arrow-upload-20-regular.svg \
  assets/icons/fluent/arrow-download-20-regular.svg \
  src/shell/context_menu.rs src/app/bootstrap.rs \
  ui/components/assets-context-menu-row.slint \
  tests/assets_context_menu_spec.rs tests/assets_context_menu_ui_contract_smoke.sh \
  tests/sidebar_assets_smoke.sh

git commit -m "feat: add icon metadata for assets context menus"
```

## Task 2: Compact The Menu Surface And Remove The Legacy Chinese Header

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_context_menu_spec.rs`, add compact-height tests:

```rust
#[test]
fn blank_area_menu_height_is_compact() {
    let roots = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            selected_ids: Vec::new(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            target_has_active_connection: false,
        },
    );

    let height = context_menu_column_height(&roots);
    assert!(height < 160.0);
}
```

In `tests/assets_context_menu_smoke.rs`, assert the root overlay height is no longer the old fixed tall panel for the blank-area menu:

```rust
#[test]
fn blank_area_menu_projects_compact_overlay_height() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let overlay_height = app.get_layout_assets_context_menu_height();
    assert!(overlay_height > 0.0);
    assert!(overlay_height < 160.0);
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, remove fixed-header expectations and add compact-surface guards:

```bash
! grep -F 'in property <string> title: "操作";' "$MENU_COLUMN" >/dev/null
! grep -F 'height: 320px;' "$MENU_OVERLAY" >/dev/null
! grep -F 'height: parent.height;' "$MENU_COLUMN" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- Height spec fails because menu height is still effectively fixed
- Smoke fails because `layout_assets_context_menu_height` still reports the old fixed height
- UI contract smoke fails because the old title / height assumptions remain in Slint

**Step 3: Write the minimal implementation**

In `src/shell/context_menu.rs`, replace the fixed menu-height constant with explicit sizing helpers:

```rust
pub const CONTEXT_MENU_ROW_HEIGHT: f32 = 32.0;
pub const CONTEXT_MENU_ROW_GAP: f32 = 4.0;
pub const CONTEXT_MENU_VERTICAL_PADDING: f32 = 8.0;
pub const CONTEXT_MENU_DIVIDER_HEIGHT: f32 = 1.0;

pub fn context_menu_column_height(items: &[ContextMenuActionNode]) -> f32 {
    if items.is_empty() {
        return 0.0;
    }

    let dividers = items.iter().filter(|item| item.divider_before).count() as f32;
    let rows = items.len() as f32;
    CONTEXT_MENU_VERTICAL_PADDING * 2.0
        + rows * CONTEXT_MENU_ROW_HEIGHT
        + (rows - 1.0).max(0.0) * CONTEXT_MENU_ROW_GAP
        + dividers * CONTEXT_MENU_DIVIDER_HEIGHT
}
```

Update root placement to use `context_menu_column_height(&roots)` instead of the legacy fixed height.

In `ui/components/assets-context-menu-column.slint`, delete the title area entirely and make the column height depend on its own content rather than `parent.height`:

```slint
height: 8px + menu-body.preferred-height + 8px;

menu-body := VerticalLayout {
    padding: 8px;
    spacing: 4px;
    // rows only
}
```

In `ui/components/assets-context-menu-overlay.slint`, drop `primary-title` / `secondary-title` / `tertiary-title` and compute overlay height as the max visible column height.

In `ui/app-window.slint` and `src/app/bootstrap.rs`, remove the title projection properties that were only there to feed `"操作"`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/app/bootstrap.rs ui/components/assets-context-menu-overlay.slint \
  ui/components/assets-context-menu-column.slint ui/app-window.slint \
  tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs \
  tests/assets_context_menu_ui_contract_smoke.sh

git commit -m "fix: compact the assets context menu surface"
```

## Task 3: Replace The Old Create Menu With Panel-Aware Toolbar Descriptors

**Files:**
- Delete: `ui/components/assets-create-menu.slint`
- Modify: `src/shell/sidebar.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/sidebar_tooltip_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_sidebar_toolbar_spec.rs`, replace the create-menu state contract with panel-aware descriptor tests:

```rust
#[test]
fn console_destination_exposes_new_ssh_as_primary_create_action() {
    let view_model = ShellViewModel::default();

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert_eq!(descriptor.primary_create_action_id, Some("new-ssh-connection"));
    assert_eq!(descriptor.primary_create_tooltip, "New SSH Connection");
}
```

Add a second test for destination switching:

```rust
#[test]
fn switching_sidebar_destination_updates_primary_create_descriptor() {
    let mut view_model = ShellViewModel::default();
    view_model.select_sidebar_destination(SidebarDestination::Snippets);

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert_eq!(descriptor.primary_create_action_id, Some("new-snippet"));
    assert_eq!(descriptor.primary_create_tooltip, "New Snippet");
}
```

In `tests/assets_sidebar_toolbar_smoke.rs`, assert bootstrap projection at the window level:

```rust
#[test]
fn switching_destination_updates_toolbar_descriptor_projection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "new-ssh-connection");

    app.invoke_sidebar_destination_selected("snippets".into());
    assert_eq!(app.get_asset_primary_create_action_id().as_str(), "new-snippet");
}
```

In `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`, rewrite the old create-menu assertions:

```bash
! grep -F 'AssetsCreateMenu' "$APP_WINDOW" >/dev/null
! grep -F 'asset_create_menu_open' "$ROOT_DIR/src/shell/view_model.rs" >/dev/null
grep -F 'callback tooltip-open-requested(' "$BUTTON" >/dev/null
grep -F 'in property <string> tooltip-text;' "$BUTTON" >/dev/null
! grep -F 'Console Tree —' "$ASSETS" >/dev/null
! grep -F 'Hosts, recent sessions, favorites' "$ASSETS" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_tooltip_ui_contract_smoke.sh
```

Expected:

- Spec fails because no toolbar descriptor helper exists
- Smoke fails because the app window does not yet project panel-aware create action ids
- UI contract smoke fails because the old `AssetsCreateMenu` path still exists and toolbar buttons have no tooltip API

**Step 3: Write the minimal implementation**

In `src/shell/sidebar.rs`, introduce a concrete descriptor type and helpers:

```rust
pub struct AssetsToolbarDescriptor {
    pub primary_create_action_id: Option<&'static str>,
    pub primary_create_tooltip: &'static str,
    pub search_tooltip: &'static str,
    pub view_mode_tooltip: &'static str,
    pub tree_expansion_tooltip: &'static str,
    pub show_tree_controls: bool,
}
```

Use explicit mappings:

```rust
SidebarDestination::Console  -> Some("new-ssh-connection"), "New SSH Connection"
SidebarDestination::Snippets -> Some("new-snippet"), "New Snippet"
SidebarDestination::Keychain -> Some("new-keychain"), "New Keychain"
```

In `src/shell/view_model.rs`, delete the legacy `asset_create_menu_open` state and related open/close methods. Keep `handle_assets_create_action(&str)` as the execution entry point, but make it tolerate `new-snippet` / `new-keychain` as planned feedback actions if those placeholder flows are not implemented in this round.

In `ui/components/sidebar-toolbar-icon-button.slint`, mirror the tooltip callback shape already used by `titlebar-icon-button.slint`:

```slint
in property <string> tooltip-text;
in property <string> tooltip-source-id;
callback tooltip-open-requested(string, string, length, length, length);
callback tooltip-close-requested(string);
```

In `ui/shell/assets-sidebar.slint`, bind the new descriptor properties, remove the static helper copy under the toolbar, and change the `create-button` click path from “toggle menu” to “fire current primary action id immediately”.

In `ui/shell/sidebar.slint` and `ui/app-window.slint`, plumb toolbar button tooltip events into the existing `sidebar-tooltip-overlay` pipeline.

Delete `ui/components/assets-create-menu.slint` and remove all imports, properties, callbacks, layout exports, and tests that only existed to support the old overlay.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_tooltip_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/sidebar.rs src/shell/view_model.rs src/app/bootstrap.rs \
  ui/components/sidebar-toolbar-icon-button.slint ui/shell/assets-sidebar.slint \
  ui/shell/sidebar.slint ui/app-window.slint \
  tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs \
  tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/sidebar_tooltip_ui_contract_smoke.sh

git rm ui/components/assets-create-menu.slint

git commit -m "refactor: make the assets toolbar panel-aware"
```

## Task 4: Unify Empty-Search Dismiss Across The Shell Body

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_sidebar_toolbar_spec.rs`, add a pure view-model helper test:

```rust
#[test]
fn dismissing_empty_search_on_shell_interaction_only_closes_blank_queries() {
    let mut view_model = ShellViewModel::default();

    view_model.activate_asset_search();
    assert!(view_model.dismiss_empty_asset_search_on_shell_interaction());
    assert!(!view_model.asset_search_expanded);

    view_model.activate_asset_search();
    view_model.set_asset_search_query("prod".into());
    assert!(!view_model.dismiss_empty_asset_search_on_shell_interaction());
    assert!(view_model.asset_search_expanded);
}
```

In `tests/assets_sidebar_toolbar_smoke.rs`, add shell-interaction cases:

```rust
#[test]
fn sidebar_destination_click_collapses_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    assert!(app.get_asset_search_expanded());

    app.invoke_sidebar_destination_selected("snippets".into());
    assert!(!app.get_asset_search_expanded());
}

#[test]
fn context_menu_request_collapses_empty_search_but_keeps_non_empty_search() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_search_requested();
    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    assert!(!app.get_asset_search_expanded());

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    assert!(app.get_asset_search_expanded());
}
```

In `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`, replace the old local-dismiss assumptions:

```bash
! grep -F 'header-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
! grep -F 'panel-search-dismiss-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'shell-body-empty-search-dismiss-layer := TouchArea {' "$APP_WINDOW" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- Spec fails because there is no unified `dismiss_empty_asset_search_on_shell_interaction()` helper
- Smoke fails because sidebar destination and context-menu callbacks currently do not collapse empty search consistently
- UI contract smoke fails because the old local dismiss touch areas are still present

**Step 3: Write the minimal implementation**

In `src/shell/view_model.rs`, add one explicit helper and make it side-effect free for non-empty searches:

```rust
pub fn dismiss_empty_asset_search_on_shell_interaction(&mut self) -> bool {
    if self.asset_search_expanded && self.asset_search_query.is_empty() {
        self.asset_search_expanded = false;
        true
    } else {
        false
    }
}
```

In `src/app/bootstrap.rs`, call this helper at the start of shell interactions that should participate in click-away behavior:

```rust
state.dismiss_empty_asset_search_on_shell_interaction();
```

Apply it before:

- `sidebar_destination_selected`
- `asset_context_menu_requested`
- `assets_create_action_selected`
- `toggle_assets_view_mode_requested`
- `toggle_assets_tree_expansion_requested`

In `ui/components/asset-node-row.slint` and `ui/components/sidebar-nav-button.slint`, emit lightweight “pointer activity” callbacks so active shell widgets can participate without relying on a giant swallow-all overlay.

In `ui/app-window.slint`, replace `workspace-search-dismiss-layer` with `shell-body-empty-search-dismiss-layer` for passive regions that do not already dispatch their own callbacks.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/components/asset-node-row.slint \
  ui/components/sidebar-nav-button.slint ui/shell/assets-sidebar.slint \
  ui/shell/sidebar.slint ui/app-window.slint \
  tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs \
  tests/assets_sidebar_toolbar_ui_contract_smoke.sh

git commit -m "fix: unify empty assets search dismiss"
```

## Task 5: Add Explicit Rename Sessions And Same-Type Unique Naming

**Files:**
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/shell_view_model.rs`, replace the old placeholder-name assumptions with numbered defaults and same-type uniqueness:

```rust
#[test]
fn toolbar_create_action_uses_first_missing_ssh_connection_name() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-ssh-connection");
    view_model.commit_active_asset_rename();
    view_model.handle_assets_create_action("new-ssh-connection");
    view_model.commit_active_asset_rename();

    assert_eq!(view_model.console_asset_items[0].label, "SSH Connection 1");
    assert_eq!(view_model.console_asset_items[1].label, "SSH Connection 2");
}

#[test]
fn folder_default_name_uses_smallest_missing_positive_index() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 2");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 3");

    view_model.handle_assets_create_action("new-folder");
    assert_eq!(view_model.renaming_asset_text, "Folder 1");
}
```

Add explicit rename-session tests:

```rust
#[test]
fn dismissing_active_rename_commits_current_draft() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.update_active_asset_rename_draft("Prod".into());

    view_model.commit_active_asset_rename();

    assert_eq!(view_model.console_asset_items[0].label, "Prod");
    assert_eq!(view_model.renaming_asset_id, None);
}
```

In `tests/assets_context_menu_smoke.rs`, add a window-level rename dismiss round-trip:

```rust
#[test]
fn dismiss_active_asset_rename_commits_through_window_callback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let asset_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();

    app.invoke_asset_rename_text_changed(asset_id.into(), "Infra".into());
    app.invoke_dismiss_active_asset_rename_requested();

    assert_eq!(app.get_console_asset_items().row_data(0).unwrap().label.as_str(), "Infra");
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, remove the old implicit blur-commit assumption:

```bash
! grep -F 'changed has-focus => {' "$ROW" >/dev/null
grep -F 'callback dismiss-active-asset-rename-requested();' "$APP_WINDOW" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- View-model tests fail because numbering still uses `New Folder` / `New SSH Connection`
- Smoke fails because no window callback exists to dismiss an active rename session
- UI contract smoke fails because `AssetNodeRow` still commits on `has-focus` change

**Step 3: Write the minimal implementation**

In `src/shell/assets.rs`, add strict same-type default-name helpers:

```rust
impl ConsoleAssetKind {
    pub fn default_name_prefix(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::SshConnection => "SSH Connection",
        }
    }
}

pub fn next_default_name(kind: ConsoleAssetKind, items: &[MockConsoleAssetItem]) -> String {
    // trim labels, collect only strict same-type "Prefix {n}" matches, choose the smallest missing positive integer
}
```

In `src/shell/view_model.rs`, move rename into explicit session helpers:

```rust
pub fn begin_asset_rename_session(&mut self, asset_id: String, initial_text: String) { ... }
pub fn update_active_asset_rename_draft(&mut self, text: String) { ... }
pub fn commit_active_asset_rename(&mut self) { ... }
pub fn cancel_active_asset_rename(&mut self) { ... }
pub fn dismiss_active_asset_rename(&mut self) { self.commit_active_asset_rename(); }
```

Update create flows to seed numbered defaults like `Folder 1` and `SSH Connection 1`.

In `ui/components/asset-node-row.slint`, delete the implicit `changed has-focus` auto-commit block. Keep only explicit `Enter` and `Escape` callbacks.

In `ui/app-window.slint`, add `callback dismiss-active-asset-rename-requested();` and route shell interactions that should end rename before opening another overlay.

In `src/app/bootstrap.rs`, wire the new callback to `state.dismiss_active_asset_rename()` and ensure handlers for panel-switch, context menu open, and primary create action call it first when appropriate.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/view_model.rs src/app/bootstrap.rs \
  ui/components/asset-node-row.slint ui/shell/assets-sidebar.slint \
  ui/shell/sidebar.slint ui/app-window.slint \
  tests/shell_view_model.rs tests/assets_context_menu_smoke.rs \
  tests/assets_context_menu_ui_contract_smoke.sh

git commit -m "feat: add explicit asset rename sessions"
```

## Task 6: Run The Full Targeted Verification Sweep And Record It

**Files:**
- Modify: `verification.md`

**Step 1: Run the full targeted suite**

Run:

```bash
cargo test --test assets_context_menu_spec \
  --test assets_context_menu_smoke \
  --test assets_sidebar_toolbar_spec \
  --test assets_sidebar_toolbar_smoke \
  --test shell_view_model \
  -- --nocapture

bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_tooltip_ui_contract_smoke.sh
bash tests/sidebar_assets_smoke.sh
cargo check
```

Expected:

- All targeted Rust tests pass
- All shell contract scripts pass
- `cargo check` passes cleanly

**Step 2: Record the verification results**

In `verification.md`, add a dated section for this bugfix round:

```md
## 2026-03-18 - Windows Console assets context menu bugfix2

- `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -- --nocapture`
- `bash tests/assets_context_menu_ui_contract_smoke.sh`
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `bash tests/sidebar_tooltip_ui_contract_smoke.sh`
- `bash tests/sidebar_assets_smoke.sh`
- `cargo check`
```

If any test had to be re-run, note the final successful run only.

**Step 3: Commit**

```bash
git add verification.md
git commit -m "docs: record assets bugfix2 verification"
```

## Done Criteria

The implementation is done only when all of the following are true:

- Context menus are compact, titleless, iconized, and fully wrapped in a single surface
- Toolbar behavior and tooltip copy change with `console`, `snippets`, and `keychain`
- The old create-menu overlay no longer exists in code or tests
- Empty search dismiss is consistent across activity bar, assets area, workspace, and passive shell regions
- Rename is explicit-session based, not implicit blur-based
- Default asset names are same-type unique and use the smallest missing positive integer
- The full targeted suite in Task 6 passes and is recorded in `verification.md`
