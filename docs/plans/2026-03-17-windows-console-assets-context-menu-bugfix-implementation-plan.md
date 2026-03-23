# Windows Console 资产列表右键菜单 Bugfix Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `Windows Console` 资产区从 demo 原型收敛为可用壳层：移除无意义 demo 项、补齐 blank-area + item 的右键入口、把新建动作统一成 placeholder 创建，并在第一版只通过 inline rename 收一个名称。

**Architecture:** 保持当前 `Rust state -> bootstrap bridge -> Slint root overlay` 路线不变。Rust 继续作为真相源，负责资产列表、最小新建动作映射、context menu resolver 和 inline rename 状态；Slint 只负责 empty state、blank-area pointer 命中、资产行编辑态和 overlay 渲染。blank-area 右键不采用覆盖全列表的大 touch layer，而是拆成“empty state host”和“列表尾部剩余空白 fill”两个命中区，避免抢占 row 事件。

**Tech Stack:** Rust 2024, Slint 1.15.1, `winit + femtovg-wgpu`, `TouchArea.pointer-event`, `TextInput`, `FocusScope`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- Design source: `docs/plans/2026-03-17-windows-console-assets-context-menu-bugfix-design.md`
- 实施时必须走 `@superpowers:test-driven-development`
- 如遇到 blank-area 命中层抢事件、`TextInput` 焦点异常、右键坐标投影异常，立即切换 `@superpowers:systematic-debugging`
- 推荐在独立 worktree 中执行，再用 `@superpowers:executing-plans` 按任务逐个落地
- 本轮不允许顺手扩展到真实 SSH 配置、持久化、terminal runtime、额外协议入口

## Target Files

### Rust

- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`

### Slint

- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`

### Tests

- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

### Docs

- Modify: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`
- Modify: `verification.md`

## Task 1: Replace Demo Assets With Empty Console State

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing test**

In `tests/assets_context_menu_smoke.rs`, replace the current demo-data expectation with a true empty-state baseline:

```rust
#[test]
fn bootstrap_starts_with_empty_console_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_console_asset_items().row_count(), 0);
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, add greps for the empty-state copy:

```bash
grep -F 'text: "No assets yet";' "$ASSETS" >/dev/null
grep -F 'text: "Right-click or use Create to add a folder or SSH connection."; ' "$ASSETS" >/dev/null
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- Rust smoke fails because `console_asset_items` still contains 3 demo items
- Shell contract smoke fails because `AssetsSidebar` does not yet render empty-state copy

**Step 3: Write minimal implementation**

In `src/app/bootstrap.rs`, stop seeding the demo list:

```rust
fn default_console_asset_items() -> Vec<MockConsoleAssetItem> {
    Vec::new()
}
```

In `ui/shell/assets-sidebar.slint`, split the console panel into two branches:

```slint
if root.console-asset-items.length == 0 : Rectangle {
    vertical-stretch: 1;
    background: transparent;

    VerticalLayout {
        spacing: 6px;
        alignment: center;

        Text { text: "No assets yet"; color: ThemeTokens.text-primary; }
        Text {
            text: "Right-click or use Create to add a folder or SSH connection.";
            color: ThemeTokens.text-secondary;
        }
    }
}

if root.console-asset-items.length > 0 : VerticalLayout {
    // keep the existing row list here; blank-area fill comes in Task 2
}
```

Do not add placeholder rows yet in this task. The only goal is to make the default console panel truly empty and readable.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs ui/shell/assets-sidebar.slint tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "fix: replace demo console assets with empty state"
```

## Task 2: Add Blank-Area Right-Click And Shrink Create IA

**Files:**
- Modify: `src/shell/context_menu.rs`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`

**Step 1: Write the failing test**

In `tests/assets_context_menu_spec.rs`, replace the old blank-area menu expectation:

```rust
#[test]
fn blank_area_scene_only_exposes_minimal_create_actions() {
    let roots = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            selected_ids: Vec::new(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            target_has_active_connection: false,
        },
    );

    let ids: Vec<_> = roots.iter().map(|node| node.id).collect();
    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
}
```

In `tests/assets_context_menu_smoke.rs`, add a bootstrap smoke for blank-area requests:

```rust
#[test]
fn blank_area_right_click_opens_minimal_primary_menu() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let primary = app.get_assets_context_menu_primary_items();
    let ids: Vec<String> = (0..primary.row_count())
        .filter_map(|index| primary.row_data(index))
        .map(|item| item.id.to_string())
        .collect();

    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
    assert_eq!(app.get_assets_context_menu_secondary_items().row_count(), 0);
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, add blank-area touch markers:

```bash
grep -F 'empty-state-context-touch := TouchArea {' "$ASSETS" >/dev/null
grep -F 'list-blank-fill-context-touch := TouchArea {' "$ASSETS" >/dev/null
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- Spec fails because blank-area resolver still returns the old larger action set
- Smoke fails because `new-connection` submenu is still projected instead of the two flat create actions
- UI contract smoke fails because the two blank-area touch targets do not exist yet

**Step 3: Write minimal implementation**

In `src/shell/context_menu.rs`, make blank-area create actions flat and minimal:

```rust
fn resolve_blank_area_actions(_selection: &SelectionContext) -> Vec<ContextMenuActionNode> {
    vec![
        action("new-folder", "New Folder"),
        action("new-ssh-connection", "New SSH Connection"),
    ]
}
```

For item menus, remove `new_connection_submenu(...)` from any “create” branch and replace it with the same two flat actions where needed:

```rust
fn create_actions(divider_before: bool) -> Vec<ContextMenuActionNode> {
    vec![
        action_with_state("new-folder", "New Folder", ContextMenuActionState::Enabled, divider_before),
        action_with_state("new-ssh-connection", "New SSH Connection", ContextMenuActionState::Enabled, false),
    ]
}
```

In `ui/shell/assets-sidebar.slint`, add two non-overlapping blank-area targets:

```slint
empty-state-context-touch := TouchArea {
    width: parent.width;
    height: parent.height;

    pointer-event(event) => {
        if event.kind == PointerEventKind.down && event.button == PointerEventButton.right {
            root.asset-context-menu-requested(
                "",
                "blank",
                self.mouse-x + self.absolute-position.x,
                self.mouse-y + self.absolute-position.y,
            );
        }
    }
}

list-blank-fill := Rectangle {
    vertical-stretch: 1;
    background: transparent;

    list-blank-fill-context-touch := TouchArea {
        width: parent.width;
        height: parent.height;
        pointer-event(event) => {
            if event.kind == PointerEventKind.down && event.button == PointerEventButton.right {
                root.asset-context-menu-requested(
                    "",
                    "blank",
                    self.mouse-x + self.absolute-position.x,
                    self.mouse-y + self.absolute-position.y,
                );
            }
        }
    }
}
```

Do not place a full-surface `TouchArea` above the rows; the row hit-target must stay first-class.

In `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md`, update the notes so the hidden/deferred actions are documented as backlog references, not current shipped UI.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/context_menu.rs ui/shell/assets-sidebar.slint tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md
git commit -m "fix: add blank-area context menu and minimal create ia"
```

## Task 3: Route Create Actions Into Placeholder Asset Creation

**Files:**
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_smoke.rs`

**Step 1: Write the failing test**

In `tests/shell_view_model.rs`, add:

```rust
#[test]
fn toolbar_create_action_inserts_ssh_placeholder_and_starts_rename() {
    let mut view_model = ShellViewModel::default();

    view_model.handle_assets_create_action("new-ssh-connection");

    assert_eq!(view_model.console_asset_items.len(), 1);
    assert_eq!(view_model.console_asset_items[0].kind, ConsoleAssetKind::SshConnection);
    assert_eq!(view_model.console_asset_items[0].label, "New SSH Connection");
    assert_eq!(view_model.renaming_asset_id.as_deref(), Some(view_model.console_asset_items[0].id.as_str()));
    assert_eq!(view_model.renaming_asset_text, "New SSH Connection");
}

#[test]
fn context_menu_create_action_inserts_folder_placeholder_and_closes_menu() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(ContextTargetKind::BlankArea, None, 32.0, 48.0);

    view_model.handle_context_menu_leaf_action("new-folder");

    assert!(!view_model.context_menu_open);
    assert_eq!(view_model.console_asset_items.len(), 1);
    assert_eq!(view_model.console_asset_items[0].kind, ConsoleAssetKind::Folder);
}
```

In `tests/assets_context_menu_smoke.rs`, add a bootstrap round-trip:

```rust
#[test]
fn create_menu_action_projects_placeholder_item_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());

    let items = app.get_console_asset_items();
    assert_eq!(items.row_count(), 1);
    assert_eq!(items.row_data(0).unwrap().label.as_str(), "New Folder");
}
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
```

Expected:

- Compile or assertion failures because `ShellViewModel` does not yet expose rename state or create-action handlers
- Smoke test fails because `on_assets_create_action_selected` still only logs and never inserts items

**Step 3: Write minimal implementation**

In `src/shell/assets.rs`, add helpers that make action routing explicit:

```rust
impl ConsoleAssetKind {
    pub fn from_create_action_id(value: &str) -> Option<Self> {
        match value {
            "new-folder" => Some(Self::Folder),
            "new-ssh-connection" => Some(Self::SshConnection),
            _ => None,
        }
    }

    pub fn placeholder_label(self) -> &'static str {
        match self {
            Self::Folder => "New Folder",
            Self::SshConnection => "New SSH Connection",
        }
    }
}
```

In `src/shell/view_model.rs`, add the minimum new state:

```rust
pub renaming_asset_id: Option<String>,
pub renaming_asset_text: String,
next_console_asset_serial: u64,
```

Add explicit helpers:

```rust
pub fn handle_assets_create_action(&mut self, action_id: &str) {
    if let Some(kind) = ConsoleAssetKind::from_create_action_id(action_id) {
        self.create_placeholder_asset(kind);
        self.close_asset_create_menu();
    }
}

pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {
    if let Some(kind) = ConsoleAssetKind::from_create_action_id(action_id) {
        self.create_placeholder_asset(kind);
        self.close_context_menu();
        return;
    }

    // keep the existing planned/enabled/disabled behavior for non-create leaves
}

fn create_placeholder_asset(&mut self, kind: ConsoleAssetKind) {
    let id = format!("draft-asset-{}", self.next_console_asset_serial);
    self.next_console_asset_serial += 1;

    let label = kind.placeholder_label().to_string();
    self.console_asset_items.push(MockConsoleAssetItem::new(id.clone(), kind, label.clone()));
    self.selected_asset_ids = vec![id.clone()];
    self.renaming_asset_id = Some(id);
    self.renaming_asset_text = label;
}
```

In `src/app/bootstrap.rs`, replace the current create-menu logging path:

```rust
window.on_assets_create_action_selected(move |action_id| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.handle_assets_create_action(action_id.as_str());
    sync_sidebar_state(&window, &state);
});
```

Also route context-menu leaf actions into the same create flow before falling back to the old planned-action behavior.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/view_model.rs src/app/bootstrap.rs tests/shell_view_model.rs tests/assets_context_menu_smoke.rs
git commit -m "feat: create placeholder assets from console actions"
```

## Task 4: Add Inline Rename UI Bridge And Final Verification

**Files:**
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `verification.md`

**Step 1: Write the failing test**

In `tests/shell_view_model.rs`, add:

```rust
#[test]
fn committing_inline_rename_updates_label_and_clears_editing_state() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-ssh-connection");
    let asset_id = view_model.console_asset_items[0].id.clone();

    view_model.update_asset_rename_draft(&asset_id, "Prod Bastion".into());
    view_model.commit_asset_rename(&asset_id, "Prod Bastion".into());

    assert_eq!(view_model.console_asset_items[0].label, "Prod Bastion");
    assert_eq!(view_model.renaming_asset_id, None);
    assert!(view_model.renaming_asset_text.is_empty());
}

#[test]
fn cancelling_inline_rename_keeps_default_label_and_exits_editing() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    let asset_id = view_model.console_asset_items[0].id.clone();

    view_model.cancel_asset_rename(&asset_id);

    assert_eq!(view_model.console_asset_items[0].label, "New Folder");
    assert_eq!(view_model.renaming_asset_id, None);
}
```

In `tests/assets_context_menu_smoke.rs`, add:

```rust
#[test]
fn rename_commit_round_trips_through_window_callbacks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    let asset_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();

    app.invoke_asset_rename_text_changed(asset_id.clone().into(), "Prod Bastion".into());
    app.invoke_asset_rename_commit_requested(asset_id.into(), "Prod Bastion".into());

    assert_eq!(app.get_console_asset_items().row_data(0).unwrap().label.as_str(), "Prod Bastion");
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, add:

```bash
grep -F 'renaming: bool,' "$ASSETS" >/dev/null
grep -F 'rename_text: string,' "$ASSETS" >/dev/null
grep -F 'callback asset-rename-text-changed(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-commit-requested(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'callback asset-rename-cancel-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'rename-input := TextInput {' "$ROW" >/dev/null
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- Rust tests fail because rename draft/commit/cancel methods and projected fields do not exist yet
- UI contract smoke fails because `AssetNodeRow` still only renders static text and no rename callbacks are bridged through `AppWindow`

**Step 3: Write minimal implementation**

In `ui/shell/assets-sidebar.slint`, extend `ConsoleAssetItem`:

```slint
export struct ConsoleAssetItem {
    id: string,
    kind: string,
    label: string,
    selected: bool,
    renaming: bool,
    rename_text: string,
}
```

Add callback pass-throughs:

```slint
callback asset-rename-text-changed(string, string);
callback asset-rename-commit-requested(string, string);
callback asset-rename-cancel-requested(string);
```

In `ui/components/asset-node-row.slint`, add an editing branch:

```slint
in property <bool> renaming: false;
in property <string> rename-text: "";
callback rename-text-changed(string, string);
callback rename-commit-requested(string, string);
callback rename-cancel-requested(string);

if !root.renaming : Text {
    text: root.label;
}

if root.renaming : Rectangle {
    rename-input := TextInput {
        text: root.rename-text;

        changed has-focus => {
            if !self.has-focus {
                root.rename-commit-requested(root.item-id, self.text);
            }
        }

        edited => {
            root.rename-text-changed(root.item-id, self.text);
        }

        key-pressed(event) => {
            if (event.text == Key.Return) {
                root.rename-commit-requested(root.item-id, self.text);
                accept
            } else if (event.text == Key.Escape) {
                root.rename-cancel-requested(root.item-id);
                accept
            } else {
                reject
            }
        }
    }
}

changed renaming => {
    if root.renaming {
        rename-input.focus();
    }
}
```

In `src/shell/view_model.rs`, add:

```rust
pub fn update_asset_rename_draft(&mut self, asset_id: &str, text: String) {
    if self.renaming_asset_id.as_deref() == Some(asset_id) {
        self.renaming_asset_text = text;
    }
}

pub fn commit_asset_rename(&mut self, asset_id: &str, text: String) {
    if let Some(item) = self.console_asset_items.iter_mut().find(|item| item.id == asset_id) {
        item.label = text;
    }
    self.renaming_asset_id = None;
    self.renaming_asset_text.clear();
}

pub fn cancel_asset_rename(&mut self, asset_id: &str) {
    if self.renaming_asset_id.as_deref() == Some(asset_id) {
        self.renaming_asset_id = None;
        self.renaming_asset_text.clear();
    }
}
```

In `src/app/bootstrap.rs`, project rename state into the Slint model and wire the new callbacks:

```rust
window.on_asset_rename_text_changed(move |asset_id, text| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.update_asset_rename_draft(asset_id.as_str(), text.to_string());
    sync_sidebar_state(&window, &state);
});

window.on_asset_rename_commit_requested(move |asset_id, text| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.commit_asset_rename(asset_id.as_str(), text.to_string());
    sync_sidebar_state(&window, &state);
});

window.on_asset_rename_cancel_requested(move |asset_id| {
    let window = handle.unwrap();
    let mut state = state.borrow_mut();
    state.cancel_asset_rename(asset_id.as_str());
    sync_sidebar_state(&window, &state);
});
```

Update `console_asset_items_for(...)` so each row carries `renaming` and `rename_text`.

Finally, append the actual verification commands and output summary to `verification.md`.

**Step 4: Run tests to verify it passes**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo check
```

Expected:

- All Rust tests pass
- UI contract smoke passes
- `cargo check` succeeds with no compile errors

**Step 5: Commit**

```bash
git add ui/components/asset-node-row.slint ui/shell/assets-sidebar.slint ui/shell/sidebar.slint ui/app-window.slint src/app/bootstrap.rs src/shell/view_model.rs tests/shell_view_model.rs tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh verification.md
git commit -m "feat: add inline rename flow for console placeholders"
```

## Final Verification Checklist

- [ ] `tests/assets_context_menu_spec.rs` reflects the new flat create IA for blank area
- [ ] `tests/assets_context_menu_smoke.rs` no longer expects 3 demo rows
- [ ] `tests/shell_view_model.rs` covers create + rename draft + rename commit/cancel
- [ ] `tests/assets_context_menu_ui_contract_smoke.sh` covers empty state, blank-area touch targets, and rename bridge callbacks
- [ ] `docs/plans/2026-03-17-windows-console-assets-context-menu-unimplemented-actions.md` no longer reads like those hidden actions are in the current shipped menu
- [ ] `verification.md` records the exact verification commands and outcomes

## Suggested Full Command Sequence

Run this full sequence after Task 4:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test shell_view_model -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
cargo check
```

If any of those fail because of pointer hit-testing, rename focus, or Slint callback wiring, stop and switch to `@superpowers:systematic-debugging` before changing scope.
