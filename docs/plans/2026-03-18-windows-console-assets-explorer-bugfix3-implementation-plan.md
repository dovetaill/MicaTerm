# Windows Console Assets Explorer Bugfix3 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `Window Console` 资产区从当前扁平 mock list 升级为真正的 Explorer 树形壳层，修复空白点击残留选中、恢复顶部双入口 `Create Popover`、补齐资产行图标与 folder 内创建能力。

**Architecture:** 保持现有 `Rust state -> bootstrap bridge -> Slint shell` 路线不变，但把 Rust 侧资产真源从扁平 `Vec<MockConsoleAssetItem>` 迁移为 canonical tree，并由 Rust 统一投影出 `visible_rows` 给 Slint。交互层拆分为 `selection / focus / editing / context target` 四类状态；顶部 `+` 恢复为专用 create popover，右键菜单继续沿用当前 overlay 体系，但 create 动作根据 blank-area 或 folder target 决定写入 root 或 child。

**Tech Stack:** Rust 2024, Slint 1.15.1, `TouchArea`, `FocusScope`, `TextInput`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- Design source: `docs/plans/2026-03-18-windows-console-assets-explorer-bugfix3-design.md`
- REQUIRED before coding: `@superpowers:test-driven-development`
- If any task hits event-order, focus, or pointer-routing bugs, switch immediately to `@superpowers:systematic-debugging`
- Preferred workflow is a dedicated worktree because `ui/*.slint`, `src/app/bootstrap.rs`, and multiple test files will all move together
- Do not expand scope into SSH forms, persistence, terminal runtime, or renderer internals
- Keep commits task-scoped and reversible
- Current tests `tests/assets_sidebar_toolbar_spec.rs` and `tests/assets_sidebar_toolbar_smoke.rs` encode the old single-action `+` behavior; updating those expectations is part of the plan, not a regression

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
- Create: `ui/components/assets-create-menu.slint`

### Tests

- Create: `tests/assets_explorer_projection.rs`
- Create: `tests/assets_explorer_smoke.rs`
- Create: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/sidebar_assets_smoke.sh`

### Docs

- Modify: `verification.md`

## Task 1: Introduce The Canonical Tree Model And Visible-Row Projection

**Files:**
- Modify: `src/shell/assets.rs`
- Create: `tests/assets_explorer_projection.rs`

**Step 1: Write the failing tests**

Create `tests/assets_explorer_projection.rs` with projection-focused tests that do not touch Slint:

```rust
use mica_term::shell::assets::{AssetTree, AssetViewMode, ConsoleAssetKind};

#[test]
fn tree_projection_hides_children_until_folder_is_expanded() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "SSH Connection 1");

    let collapsed = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(collapsed.len(), 1);
    assert_eq!(collapsed[0].depth, 0);

    tree.set_expanded(&folder_id, true);
    let expanded = tree.project_visible_rows(AssetViewMode::Tree, "");
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded[1].depth, 1);
}

#[test]
fn default_names_are_unique_within_parent_scope() {
    let mut tree = AssetTree::new();
    let root_folder = tree.insert_root(ConsoleAssetKind::Folder, "Folder 1");
    tree.insert_root(ConsoleAssetKind::Folder, "Folder 2");
    tree.insert_child(&root_folder, ConsoleAssetKind::Folder, "Folder 1");

    assert_eq!(
        tree.next_default_name_for_parent(None, ConsoleAssetKind::Folder),
        "Folder 3"
    );
    assert_eq!(
        tree.next_default_name_for_parent(Some(&root_folder), ConsoleAssetKind::Folder),
        "Folder 2"
    );
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_explorer_projection -- --nocapture
```

Expected:

- FAIL because `AssetTree`, `VisibleAssetRow`, `insert_child`, or `project_visible_rows` do not exist yet

**Step 3: Write minimal implementation**

In `src/shell/assets.rs`, introduce the canonical tree primitives and projection helpers:

```rust
pub struct AssetNode {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub title: String,
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub expanded: bool,
}

pub struct VisibleAssetRow {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
}

pub struct AssetTree {
    // node storage + root ordering
}
```

Implement only the helpers needed by the tests:

- `AssetTree::new()`
- `insert_root(...)`
- `insert_child(...)`
- `set_expanded(...)`
- `project_visible_rows(view_mode, search_query)`
- `next_default_name_for_parent(parent_id, kind)`

Keep `MockConsoleAssetItem` only if needed as a temporary bridge; do not let it remain the canonical data source.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test assets_explorer_projection -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs tests/assets_explorer_projection.rs
git commit -m "feat: add canonical assets explorer tree model"
```

## Task 2: Migrate ShellViewModel To Explorer State Machine

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

In `tests/shell_view_model.rs`, add state-machine coverage for `selection / focus / editing / blank-click`:

```rust
#[test]
fn blank_area_click_commits_rename_and_clears_selection_and_focus() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.update_active_asset_rename_draft("Infra".into());

    view_model.handle_blank_area_click();

    assert!(view_model.selected_asset_ids.is_empty());
    assert_eq!(view_model.focused_asset_id, None);
    assert_eq!(view_model.editing_asset_id, None);
    assert_eq!(view_model.visible_console_asset_rows()[0].label, "Infra");
}

#[test]
fn selecting_an_asset_updates_focus_without_opening_context_menu() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.commit_active_asset_rename();
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.select_asset(&asset_id);

    assert_eq!(view_model.focused_asset_id.as_deref(), Some(asset_id.as_str()));
    assert_eq!(view_model.selected_asset_ids, vec![asset_id]);
    assert!(!view_model.context_menu_open);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
```

Expected:

- FAIL because `focused_asset_id`, `editing_asset_id`, `handle_blank_area_click`, or `visible_console_asset_rows()` do not exist yet

**Step 3: Write minimal implementation**

In `src/shell/view_model.rs`:

- Replace the asset truth source with the new tree from `src/shell/assets.rs`
- Add explicit explorer state:

```rust
pub focused_asset_id: Option<String>,
pub editing_asset_id: Option<String>,
pub editing_asset_text: String,
pub context_target_asset_id: Option<String>,
```

- Add methods:

```rust
pub fn visible_console_asset_rows(&self) -> Vec<VisibleAssetRow>;
pub fn select_asset(&mut self, asset_id: &str);
pub fn handle_blank_area_click(&mut self);
pub fn toggle_folder_expanded(&mut self, asset_id: &str);
```

In `src/app/bootstrap.rs`, stop projecting directly from `console_asset_items`; project from `visible_console_asset_rows()` instead.

Do not wire new Slint callbacks yet; this task only gets the Rust state machine and projection bridge compiling.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs tests/shell_view_model.rs
git commit -m "refactor: split explorer selection focus and editing state"
```

## Task 3: Restore The Console Create Popover

**Files:**
- Create: `ui/components/assets-create-menu.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/sidebar.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Update `tests/assets_sidebar_toolbar_spec.rs` to reflect the new Console create behavior:

```rust
#[test]
fn console_destination_uses_create_popover_instead_of_single_direct_action() {
    let view_model = ShellViewModel::default();
    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);

    assert!(descriptor.uses_create_popover);
    assert_eq!(descriptor.primary_create_tooltip, "Create Asset");
}
```

Add smoke coverage in `tests/assets_sidebar_toolbar_smoke.rs`:

```rust
#[test]
fn toggling_console_create_button_opens_and_closes_create_popover() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_toggle_assets_create_menu_requested();
    assert!(app.get_asset_create_menu_open());

    app.invoke_close_assets_create_menu_requested();
    assert!(!app.get_asset_create_menu_open());
}
```

Update `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` with contract checks:

```bash
grep -F 'callback toggle-assets-create-menu-requested();' "$ASSETS" >/dev/null
grep -F 'asset-create-menu-open' "$ASSETS" >/dev/null
grep -F 'AssetsCreateMenu' "$APP_WINDOW" >/dev/null
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- FAIL because Console still exposes direct `new-ssh-connection` semantics and no create-menu callbacks or overlay exist

**Step 3: Write minimal implementation**

Restore a dedicated create popover only for `Console`:

- Recreate `ui/components/assets-create-menu.slint` with two rows:
  - `New Folder`
  - `New SSH Connection`
- In `src/shell/sidebar.rs`, extend `AssetsToolbarDescriptor`:

```rust
pub struct AssetsToolbarDescriptor {
    pub uses_create_popover: bool,
    pub primary_create_action_id: Option<&'static str>,
    pub primary_create_tooltip: &'static str,
    // existing tooltip fields...
}
```

- For `Console`, set:

```rust
uses_create_popover: true,
primary_create_action_id: None,
primary_create_tooltip: "Create Asset",
```

- For `Snippets` and `Keychain`, keep the current direct-action path
- Reintroduce:
  - `asset_create_menu_open`
  - `toggle_assets_create_menu_requested`
  - `close_assets_create_menu_requested`
  - create-menu anchor geometry outputs
- In `AssetsSidebar`, route `+` click as:

```slint
if root.active-panel == "console" {
    root.toggle-assets-create-menu-requested();
} else if root.asset-primary-create-action-id != "" {
    root.assets-create-action-selected(root.asset-primary-create-action-id);
}
```

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/assets-create-menu.slint \
  ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint \
  src/shell/sidebar.rs src/shell/view_model.rs src/app/bootstrap.rs \
  tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_smoke.rs \
  tests/assets_sidebar_toolbar_ui_contract_smoke.sh

git commit -m "feat: restore console assets create popover"
```

## Task 4: Upgrade The Asset Row To Explorer Contract

**Files:**
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Create: `tests/assets_explorer_smoke.rs`
- Create: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `tests/sidebar_assets_smoke.sh`

**Step 1: Write the failing tests**

Create `tests/assets_explorer_smoke.rs`:

```rust
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::Model;

#[test]
fn created_folder_projects_depth_and_icon_metadata_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let row = app.get_console_asset_items().row_data(0).unwrap();

    assert_eq!(row.kind.as_str(), "folder");
    assert_eq!(row.depth, 0);
    assert!(!row.has_children);
}
```

Create `tests/assets_explorer_ui_contract_smoke.sh`:

```bash
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"

grep -F 'in property <int> depth;' "$ROW" >/dev/null
grep -F 'in property <bool> has-children;' "$ROW" >/dev/null
grep -F 'callback toggle-expanded-requested(string);' "$ROW" >/dev/null
grep -F 'callback asset-selected(string);' "$ASSETS" >/dev/null
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- FAIL because `ConsoleAssetItem` does not yet carry `depth` / `has_children`, and the row component does not expose Explorer props or callbacks

**Step 3: Write minimal implementation**

Extend `ConsoleAssetItem` in `ui/shell/assets-sidebar.slint` with Explorer row metadata:

```slint
export struct ConsoleAssetItem {
    id: string,
    kind: string,
    label: string,
    depth: int,
    has_children: bool,
    expanded: bool,
    selected: bool,
    focused: bool,
    renaming: bool,
    rename_text: string,
}
```

Upgrade `ui/components/asset-node-row.slint`:

- add props: `depth`, `has-children`, `expanded`, `focused`
- render indentation using `depth`
- render chevron for folders with children
- render kind icon for every row
- add callbacks:

```slint
callback selected(string);
callback toggle-expanded-requested(string);
```

Bridge these callbacks upward through:

- `ui/shell/assets-sidebar.slint`
- `ui/shell/sidebar.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`

Map them in Rust to:

- `select_asset(...)`
- `toggle_folder_expanded(...)`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/sidebar_assets_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/asset-node-row.slint \
  ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint \
  src/app/bootstrap.rs src/shell/view_model.rs \
  tests/assets_explorer_smoke.rs tests/assets_explorer_ui_contract_smoke.sh \
  tests/sidebar_assets_smoke.sh

git commit -m "feat: add explorer row contract for console assets"
```

## Task 5: Route Context-Menu Create Actions Into Root Or Folder Targets

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add child-create coverage in `tests/shell_view_model.rs`:

```rust
#[test]
fn folder_context_create_inserts_child_and_expands_parent() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.commit_active_asset_rename();
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(folder_id.clone()),
        48.0,
        64.0,
    );
    view_model.handle_context_menu_leaf_action("new-ssh-connection");

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, folder_id);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].kind, ConsoleAssetKind::SshConnection);
}
```

Add bootstrap coverage in `tests/assets_context_menu_smoke.rs`:

```rust
#[test]
fn folder_target_create_action_projects_child_row_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    let folder_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();
    app.invoke_asset_rename_commit_requested(folder_id.clone().into(), "Infra".into());
    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(1).unwrap().depth, 1);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
```

Expected:

- FAIL because create actions still write only root-level nodes and do not expand folder parents

**Step 3: Write minimal implementation**

In `src/shell/view_model.rs`, introduce an explicit create target resolver:

```rust
fn active_create_parent_id(&self, action_source: CreateActionSource) -> Option<String> {
    match action_source {
        CreateActionSource::Toolbar => None,
        CreateActionSource::ContextMenuFolder(id) => Some(id),
        CreateActionSource::ContextMenuBlank => None,
    }
}
```

Update `handle_context_menu_leaf_action(...)` so `new-folder` and `new-ssh-connection`:

- create child nodes when the context target is a folder
- create root nodes for blank-area targets
- auto-expand the parent folder before projecting visible rows
- select and focus the newly created child row

In `src/shell/context_menu.rs`, keep the `Folder` scene create actions flat and first-class; do not hide them behind a submenu.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/view_model.rs src/app/bootstrap.rs \
  tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs \
  tests/shell_view_model.rs tests/assets_context_menu_ui_contract_smoke.sh

git commit -m "feat: support folder-targeted assets create actions"
```

## Task 6: Final Verification, Search/Flat Regressions, And Evidence Logging

**Files:**
- Modify: `tests/assets_explorer_projection.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `verification.md`

**Step 1: Write the last failing regression tests**

Add final regression coverage:

```rust
#[test]
fn flat_projection_keeps_all_nodes_visible_regardless_of_expanded_state() {
    // build tree with collapsed folder + child
    // assert flat mode still shows both rows
}

#[test]
fn search_filters_visible_rows_without_destroying_tree_state() {
    // collapsed folder + child
    // assert search returns matching rows but folder expansion state is preserved after clearing search
}
```

Add a smoke test ensuring the Console create popover still creates root-level assets while folder context creates child assets.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_projection --test assets_explorer_smoke --test assets_sidebar_toolbar_smoke -- --nocapture
```

Expected:

- FAIL until flat-mode projection and search behavior are stabilized

**Step 3: Write minimal implementation**

Finalize the remaining projection and search behavior:

- ensure `AssetViewMode::Flat` ignores `expanded`
- ensure clearing search does not mutate expansion state
- ensure toolbar create always targets root
- ensure blank-area click after rename does not leave stale selection or focus

Record the executed commands and results in `verification.md` under a new dated section:

```md
## 2026-03-18 - Windows Console assets explorer bugfix3

- `cargo test --test assets_explorer_projection --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test assets_explorer_smoke -- --nocapture`
- `bash tests/assets_context_menu_ui_contract_smoke.sh`
- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `bash tests/assets_explorer_ui_contract_smoke.sh`
- `bash tests/sidebar_assets_smoke.sh`
- `cargo check --workspace`
```

**Step 4: Run the full verification suite**

Run:

```bash
cargo test --test assets_explorer_projection \
  --test shell_view_model \
  --test assets_context_menu_spec \
  --test assets_context_menu_smoke \
  --test assets_sidebar_toolbar_spec \
  --test assets_sidebar_toolbar_smoke \
  --test assets_explorer_smoke \
  -- --nocapture

bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/sidebar_assets_smoke.sh

cargo check --workspace
```

Expected:

- PASS for all tests and smoke scripts
- `cargo check --workspace` exits `0`

**Step 5: Commit**

```bash
git add tests/assets_explorer_projection.rs tests/assets_explorer_smoke.rs \
  tests/assets_sidebar_toolbar_smoke.rs verification.md

git commit -m "test: verify console assets explorer bugfix3"
```

