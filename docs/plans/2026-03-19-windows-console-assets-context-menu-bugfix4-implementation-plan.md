# Windows Console 资产列表右键菜单 Bugfix4 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `Windows Console` 资产区的新建流程升级为 modal 驱动，并将 Explorer 列表收敛到 VS Code 风格的 `Tree / Flat` 双视图，修复展开/收缩点击异常与 `Flat` 目录显示错误。

**Architecture:** 保持现有 `Rust state -> bootstrap bridge -> Slint shell` 路线不变。Rust 继续持有 `AssetTree`、context menu、selection/focus/rename 真源，并新增 `asset modal draft state`；Slint 继续只承担 root overlay、`ListView` 渲染、row hit-testing 和视觉层。create 流程改为“open modal -> edit draft -> save -> insert node”，inline rename 只保留给现有节点的 `Rename` 动作。

**Tech Stack:** Rust 2024, Slint 1.15.1, `ListView`, `TouchArea`, `FocusScope`, `TextInput`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- Design source: `docs/plans/2026-03-19-windows-console-assets-context-menu-bugfix4-design.md`
- I'm using the writing-plans skill to create the implementation plan.
- REQUIRED before coding: `@superpowers:test-driven-development`
- If any task hits pointer routing, focus trap, or event-order regressions, switch immediately to `@superpowers:systematic-debugging`
- Execution should happen in a dedicated worktree even though this plan was authored from the current workspace
- Do not expand scope into terminal runtime, SSH runtime, SFTP, persistence, or renderer internals
- Keep each commit task-scoped and reversible
- Existing tests such as `tests/assets_explorer_projection.rs`, `tests/assets_sidebar_toolbar_spec.rs`, and `tests/assets_context_menu_smoke.rs` encode the current behavior; changing them is part of the feature, not a regression

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
- Modify: `ui/components/asset-node-row.slint`
- Create: `ui/components/assets-folder-create-modal.slint`
- Create: `ui/components/assets-ssh-connection-modal.slint`

### Tests

- Modify: `tests/assets_explorer_projection.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`
- Create: `tests/assets_modal_smoke.rs`
- Create: `tests/assets_modal_ui_contract_smoke.sh`

### Docs

- Modify: `verification.md`

## Task 1: Add Modal Draft State And Save-On-Confirm Create Semantics

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

Extend `tests/shell_view_model.rs` with modal-state coverage that proves create no longer inserts placeholder rows immediately:

```rust
#[test]
fn opening_new_folder_modal_does_not_insert_placeholder_row() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_folder_modal(None);

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(view_model.asset_modal_state.is_some());
}

#[test]
fn confirming_new_folder_modal_inserts_root_node_and_selects_it() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_folder_modal(None);
    view_model.update_new_folder_modal_name("Infra".into());

    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "Infra");
    assert_eq!(view_model.focused_asset_id.as_deref(), Some(rows[0].id.as_str()));
    assert!(view_model.asset_modal_state.is_none());
}

#[test]
fn folder_targeted_create_modal_inserts_child_on_confirm() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_new_folder_modal(Some(folder_id.clone()));
    view_model.update_new_folder_modal_name("Bastions".into());
    view_model.confirm_asset_modal();

    view_model.toggle_folder_expanded(&folder_id);
    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].label, "Bastions");
    assert_eq!(rows[1].depth, 1);
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
```

Expected:

- FAIL because modal state, modal open/update/confirm APIs, or save-on-confirm create behavior do not exist yet

**Step 3: Write minimal implementation**

In `src/shell/view_model.rs`, add an explicit modal draft enum and helper methods:

```rust
pub enum AssetModalState {
    NewFolder {
        parent_id: Option<String>,
        draft_name: String,
    },
    NewSshConnection {
        parent_id: Option<String>,
        active_tab: AssetSshModalTab,
        draft: AssetSshConnectionDraft,
    },
}

pub fn open_new_folder_modal(&mut self, parent_id: Option<String>) { ... }
pub fn update_new_folder_modal_name(&mut self, value: String) { ... }
pub fn confirm_asset_modal(&mut self) { ... }
pub fn cancel_asset_modal(&mut self) { ... }
```

Required behavior:

- opening create modal must close context menu and create popover
- opening modal must end any active inline rename session
- modal confirm is the only path that inserts a new node
- after confirm, select + focus the new node and clear modal state
- inline rename remains unchanged for existing nodes

Do not wire Slint yet; this task is Rust-side state only.

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs tests/shell_view_model.rs
git commit -m "feat: add asset create modal state"
```

## Task 2: Add Root Modal Overlay Plumbing To AppWindow

**Files:**
- Create: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/assets_modal_smoke.rs`
- Create: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Create `tests/assets_modal_smoke.rs` with bridge-level coverage for folder modal plumbing:

```rust
#[test]
fn folder_modal_visibility_round_trips_through_window_properties() {
    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-folder".into());
    app.set_asset_folder_modal_name("Infra".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Infra");
}
```

Create `tests/assets_modal_ui_contract_smoke.sh` and assert the new root overlay contract exists:

```bash
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"

grep -F 'in-out property <bool> asset-modal-open: false;' "$APP_WINDOW" >/dev/null
grep -F 'in-out property <string> asset-modal-kind: "";' "$APP_WINDOW" >/dev/null
grep -F 'callback close-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'callback confirm-asset-modal-requested();' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsFolderCreateModal inherits Rectangle {' "$FOLDER_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- Rust test fails because modal properties are not generated yet
- shell script fails because the modal component and callbacks do not exist

**Step 3: Write minimal implementation**

In `ui/app-window.slint`, add root-level modal bridge properties and callbacks:

```slint
in-out property <bool> asset-modal-open: false;
in-out property <string> asset-modal-kind: "";
in-out property <string> asset-folder-modal-name: "";
callback close-asset-modal-requested();
callback confirm-asset-modal-requested();
callback asset-folder-modal-name-changed(string);
```

Create `ui/components/assets-folder-create-modal.slint` as a compact custom modal surface with:

- title text
- one `TextInput`
- `取消` / `确定` buttons
- auto focus API
- `Esc` to close
- `Enter` to confirm
- `确定` disabled when the trimmed name is empty

In `src/app/bootstrap.rs`, add sync helpers between `ShellViewModel` and the new modal properties, but only for the folder modal at this task.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/assets-folder-create-modal.slint ui/app-window.slint src/app/bootstrap.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: add root asset folder modal overlay"
```

## Task 3: Add The SSH Connection Modal Shell And Draft Tabs

**Files:**
- Create: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Extend `tests/shell_view_model.rs` with SSH modal draft behavior:

```rust
#[test]
fn opening_new_ssh_modal_does_not_insert_placeholder_row() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(matches!(view_model.asset_modal_state, Some(AssetModalState::NewSshConnection { .. })));
}

#[test]
fn confirming_new_ssh_modal_requires_name_and_host() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    assert!(view_model.can_confirm_asset_modal());
}
```

Extend `tests/assets_modal_ui_contract_smoke.sh` with SSH modal contract checks:

```bash
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"

grep -F 'export component AssetsSshConnectionModal inherits Rectangle {' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> active-tab: "standard";' "$SSH_MODAL" >/dev/null
grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'callback draft-changed(string, string);' "$SSH_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because SSH modal draft types, confirm gating, or UI component do not exist yet

**Step 3: Write minimal implementation**

In `src/shell/view_model.rs`, define a minimal SSH modal draft:

```rust
pub struct AssetSshConnectionDraft {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: String,
    pub environment: String,
    pub proxy_method: String,
}
```

Add helpers:

```rust
pub fn open_new_ssh_modal(&mut self, parent_id: Option<String>) { ... }
pub fn update_ssh_modal_field(&mut self, field: &str, value: String) { ... }
pub fn select_ssh_modal_tab(&mut self, tab: &str) { ... }
pub fn can_confirm_asset_modal(&self) -> bool { ... }
```

Create `ui/components/assets-ssh-connection-modal.slint` with a custom large modal shell that includes:

- title and close button
- tab strip for `standard`, `tunnel`, `proxy`, `environment`, `advanced`
- required fields in the standard tab (`name`, `host`, `user`, `port`)
- placeholder sections for tunnel / proxy / environment / advanced
- bottom actions `测试连接` and `保存`

Do not wire any real SSH runtime. This task is shell-only and local-validation-only.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/shell_view_model.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: add ssh connection modal shell"
```

## Task 4: Migrate Console Assets Rendering To ListView And Refresh Row Contract

**Files:**
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Update `tests/assets_explorer_ui_contract_smoke.sh` to lock the new host structure:

```bash
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"

grep -F 'import { ListView } from "std-widgets.slint";' "$ASSETS" >/dev/null
grep -F 'for item in root.console-asset-items : AssetNodeRow' "$ASSETS" >/dev/null
grep -F 'in property <string> path-hint: "";' "$ROW" >/dev/null
grep -F 'private property <image> chevron-icon:' "$ROW" >/dev/null
```

Extend `tests/assets_explorer_smoke.rs` with a smoke test that proves row path hints round-trip:

```rust
#[test]
fn flat_projection_rows_can_surface_path_hints() {
    let app = AppWindow::new().unwrap();
    let rows = app.get_console_asset_items();
    assert!(rows.row_count() >= 0);
}
```

The smoke is intentionally shallow; the contract shell script is the real guard for this task.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- FAIL because `ListView`, path-hint row API, or chevron icon wiring do not exist yet

**Step 3: Write minimal implementation**

In `ui/shell/assets-sidebar.slint`:

- import `ListView` from `std-widgets.slint`
- replace the current console `VerticalLayout` list host with a `ListView`
- keep blank-area hit targets outside the row items
- preserve the existing empty-state branch

In `ui/components/asset-node-row.slint`, rework the row layout:

```slint
in property <string> path-hint: "";
in property <bool> show-disclosure: false;
in property <bool> compact-flat-mode: false;
```

Visual contract requirements:

- use real chevron icon instead of text `v` / `>`
- full-width flat selection surface
- separate title and optional path hint text blocks
- no button-like pill styling

Do not finish hit-testing or `Flat` semantics here; this task is about host migration and row structure.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/shell/assets-sidebar.slint ui/components/asset-node-row.slint src/app/bootstrap.rs tests/assets_explorer_smoke.rs tests/assets_explorer_ui_contract_smoke.sh
git commit -m "feat: migrate assets explorer to listview shell"
```

## Task 5: Fix Expand/Collapse Hit-Testing And Redefine Flat Projection

**Files:**
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/sidebar.rs`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `tests/assets_explorer_projection.rs`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_explorer_smoke.rs`

**Step 1: Write the failing tests**

Extend `tests/assets_explorer_projection.rs` with the new `Flat` contract:

```rust
#[test]
fn flat_projection_only_returns_ssh_rows_and_adds_path_hints() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Prod");
    tree.insert_child(&folder_id, ConsoleAssetKind::SshConnection, "Bastion");

    let rows = tree.project_visible_rows(AssetViewMode::Flat, "");

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "Bastion");
    assert_eq!(rows[0].path_hint.as_deref(), Some("Prod"));
    assert!(!rows[0].show_disclosure);
}
```

Extend `tests/assets_sidebar_toolbar_spec.rs`:

```rust
#[test]
fn flat_mode_hides_tree_controls_in_toolbar_descriptor() {
    let mut view_model = ShellViewModel::default();
    view_model.toggle_asset_view_mode();

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert!(!descriptor.show_tree_controls);
}
```

Update `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` to expect the tree control to be hidden in flat mode rather than only disabled.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_projection --test assets_sidebar_toolbar_spec --test assets_explorer_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- FAIL because `VisibleAssetRow` has no `path_hint` / `show_disclosure`
- FAIL because toolbar descriptor still exposes tree controls in flat mode

**Step 3: Write minimal implementation**

In `src/shell/assets.rs`, extend `VisibleAssetRow`:

```rust
pub struct VisibleAssetRow {
    pub id: String,
    pub kind: ConsoleAssetKind,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub path_hint: Option<String>,
    pub show_disclosure: bool,
}
```

Implement `Flat` projection as:

- only emit `ConsoleAssetKind::SshConnection`
- compute `path_hint` from ancestor folder titles
- set `show_disclosure = false`
- keep the canonical tree unchanged

In `src/shell/sidebar.rs`, make `show_tree_controls` false whenever `asset_view_mode == Flat`.

In `ui/components/asset-node-row.slint`, split row hit targets so disclosure and row-body are non-overlapping touch regions.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_explorer_projection --test assets_sidebar_toolbar_spec --test assets_explorer_smoke -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/view_model.rs src/shell/sidebar.rs ui/components/asset-node-row.slint ui/shell/assets-sidebar.slint tests/assets_explorer_projection.rs tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh tests/assets_explorer_smoke.rs
git commit -m "feat: redefine flat explorer projection for ssh-only rows"
```

## Task 6: Route Toolbar And Context Menu Create Actions Through The Modal Flow

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`

**Step 1: Write the failing tests**

In `tests/assets_context_menu_smoke.rs`, replace the old placeholder-create expectation:

```rust
#[test]
fn toolbar_create_action_opens_modal_instead_of_inserting_placeholder() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(view_model.asset_modal_state.is_some());
}

#[test]
fn folder_context_create_opens_child_targeted_ssh_modal() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_context_menu_for_target(ContextTargetKind::Folder, Some(folder_id.clone()), 10.0, 10.0);
    view_model.handle_context_menu_leaf_action("new-ssh-connection");

    assert!(matches!(view_model.asset_modal_state, Some(AssetModalState::NewSshConnection { parent_id: Some(id), .. }) if id == folder_id));
}
```

Update `tests/assets_sidebar_toolbar_smoke.rs` so toolbar create no longer assumes immediate node insertion.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_smoke --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- FAIL because context menu create actions still call direct insertion + inline rename

**Step 3: Write minimal implementation**

In `src/shell/view_model.rs`, change create action handlers so they dispatch to modal open helpers:

```rust
if action_id == "new-folder" {
    self.open_new_folder_modal(parent_id);
    return;
}
if action_id == "new-ssh-connection" {
    self.open_new_ssh_modal(parent_id);
    return;
}
```

Required behavior:

- toolbar `+` opens modal, does not insert rows
- blank-area menu opens root-targeted modal
- folder menu opens child-targeted modal
- existing `Rename` still uses inline rename on committed nodes
- existing `Delete`, `Refresh`, `Import`, `Export` behavior stays unchanged

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_context_menu_smoke --test assets_sidebar_toolbar_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs src/shell/view_model.rs src/app/bootstrap.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh tests/assets_sidebar_toolbar_smoke.rs
git commit -m "feat: route asset create actions through modal flow"
```

## Task 7: Final Regression Sweep And Verification Notes

**Files:**
- Modify: `verification.md`
- Verify only: `src/app/bootstrap.rs`
- Verify only: `src/shell/view_model.rs`
- Verify only: `src/shell/assets.rs`
- Verify only: `ui/app-window.slint`
- Verify only: `ui/shell/assets-sidebar.slint`
- Verify only: `ui/components/asset-node-row.slint`
- Verify only: `ui/components/assets-folder-create-modal.slint`
- Verify only: `ui/components/assets-ssh-connection-modal.slint`

**Step 1: Update verification checklist**

In `verification.md`, add a new section `Windows Console Assets Context Menu Bugfix4 Verification` covering:

- folder modal opens and saves correctly
- SSH modal opens and saves correctly
- create does not insert placeholder nodes before confirm
- disclosure click expands/collapses reliably
- `Flat` only shows SSH rows
- path hint appears in `Flat`
- context menu / modal / inline rename do not overlap

**Step 2: Run the targeted regression suite**

Run:

```bash
cargo test \
  --test shell_view_model \
  --test assets_modal_smoke \
  --test assets_context_menu_smoke \
  --test assets_explorer_projection \
  --test assets_explorer_smoke \
  --test assets_sidebar_toolbar_spec \
  --test assets_sidebar_toolbar_smoke \
  -q

bash tests/assets_modal_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- all Rust tests PASS
- all smoke scripts exit `0`

**Step 3: Run compile-level verification**

Run:

```bash
cargo check
```

Expected:

- PASS with no compile errors

**Step 4: Commit**

```bash
git add verification.md
git commit -m "test: verify assets explorer modal and flat-mode bugfix4"
```

## Manual QA Notes For The Implementer

Run a local desktop session after automated verification and confirm:

- `Tree` mode visually resembles VS Code Explorer more than the old prototype
- row selection is full-width and flat, not pill-like
- folder disclosure does not require pixel-perfect clicking
- folder modal and SSH modal appear above all shell content and trap focus correctly
- large SSH modal does not close on accidental background click
- `Flat` mode hides directory rows entirely but still shows meaningful path context for SSH items

## Out-Of-Scope Reminders

Do not add in this plan:

- real SSH connection tests
- persistence writes to sqlite / JSON
- actual `测试连接` network calls
- drag/drop tree reordering
- multi-select or bulk-edit
- terminal runtime integration
