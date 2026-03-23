# Windows Console Assets Style Optimization Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 `Windows Console` 资产区重构为更接近 `VS Code Explorer` 的高密度视觉与稳定交互层，完成比例重做、modal 英文化、Flat 模式 toolbar 语义修正，以及 context menu hover / click 稳定性治理。

**Architecture:** 保持现有 `Rust state -> bootstrap bridge -> Slint root overlay` 路线不变。Rust 继续作为 `AssetTree`、toolbar descriptor、context menu action tree、placement 与 open-path 的结构真源；Slint 负责 explorer row、toolbar、modal、context menu 的视觉与局部瞬时 hover 态。核心改动是把 explorer 尺寸契约收敛到单一 dense row 模型，并把 context menu 的“瞬时 hover 视觉态”从 Rust round-trip 中拆出来，只在语义变化时同步 `open_path`。

**Tech Stack:** Rust 2024, Slint 1.15.1, `ListView`, `TouchArea`, `FocusScope`, `TextInput`, `i-slint-backend-testing`, shell smoke scripts, `cargo test`, `cargo check`

---

## Execution Notes

- Design source: `docs/plans/2026-03-19-windows-console-assets-style-optimization-design.md`
- I'm using the writing-plans skill to create the implementation plan.
- REQUIRED before coding: `@superpowers:test-driven-development`
- If any task hits pointer-routing, focus loss, hover flicker, or event-order regressions, switch immediately to `@superpowers:systematic-debugging`
- Execute in a dedicated worktree even though this document was authored from the current workspace
- Do not expand scope into terminal core, SSH runtime, SFTP, persistence, or renderer internals
- Existing user/unrelated workspace changes must be ignored; only touch files listed in this plan
- Keep commits task-scoped and reversible

## Target Files

### Rust

- Modify: `src/shell/sidebar.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`

### Slint

- Modify: `ui/theme/tokens.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `ui/components/assets-context-menu-row.slint`
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`

### Tests

- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_smoke.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

### Docs

- Modify: `verification.md`

## Task 1: Lock Toolbar Semantics And Stable Explorer Width Contract

**Files:**
- Modify: `src/shell/sidebar.rs`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `tests/assets_sidebar_toolbar_spec.rs`
- Modify: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_sidebar_toolbar_spec.rs`, add descriptor-level coverage for Flat mode and stable control slots:

```rust
#[test]
fn flat_mode_keeps_tree_controls_slot_but_disables_interaction() {
    let mut view_model = ShellViewModel::default();
    view_model.toggle_asset_view_mode();

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert!(descriptor.show_tree_controls);
    assert!(!descriptor.tree_controls_enabled);
    assert_eq!(descriptor.tree_expansion_tooltip, "Switch to Tree View to expand folders");
}

#[test]
fn tree_mode_keeps_tree_controls_enabled() {
    let view_model = ShellViewModel::default();

    let descriptor = toolbar_descriptor_for(view_model.active_sidebar_destination, &view_model);
    assert!(descriptor.show_tree_controls);
    assert!(descriptor.tree_controls_enabled);
}
```

In `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`, assert the new width/slot contract exists:

```bash
SIDEBAR="$ROOT_DIR/ui/shell/sidebar.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"

grep -F 'width: 44px + (root.show-assets-sidebar ? 320px : 0px);' "$SIDEBAR" >/dev/null
grep -F 'width: expanded ? 320px : 0px;' "$ASSETS" >/dev/null
grep -F 'enabled: root.asset-show-tree-controls && root.asset-tree-controls-enabled;' "$ASSETS" >/dev/null
! grep -F 'visible: root.asset-show-tree-controls && root.asset-view-mode == "tree";' "$ASSETS" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- FAIL because `tree_controls_enabled` does not exist yet
- FAIL because sidebar width and Flat-mode button visibility rules still match the old contract

**Step 3: Write minimal implementation**

In `src/shell/sidebar.rs`, extend the descriptor:

```rust
pub struct AssetsToolbarDescriptor {
    pub uses_create_popover: bool,
    pub primary_create_action_id: Option<&'static str>,
    pub primary_create_tooltip: &'static str,
    pub search_tooltip: &'static str,
    pub view_mode_tooltip: &'static str,
    pub tree_expansion_tooltip: &'static str,
    pub show_tree_controls: bool,
    pub tree_controls_enabled: bool,
}
```

Required behavior:

- `show_tree_controls` remains `true` for `Console` in both `Tree` and `Flat`
- `tree_controls_enabled` is `true` only in `Tree`
- `tree_expansion_tooltip` changes to explanatory copy in `Flat`

In `ui/shell/sidebar.slint` and `ui/shell/assets-sidebar.slint`:

- reduce activity rail footprint and widen explorer pane
- keep tree control slot visible in `Flat`
- bind `enabled` to `asset-tree-controls-enabled`
- use a ghost / disabled visual state instead of hiding the button

Do not refactor row visuals yet; this task only establishes the toolbar semantics and shell width contract.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/shell/sidebar.rs ui/shell/sidebar.slint ui/shell/assets-sidebar.slint tests/assets_sidebar_toolbar_spec.rs tests/assets_sidebar_toolbar_ui_contract_smoke.sh
git commit -m "feat: stabilize assets toolbar semantics"
```

## Task 2: Rebuild Explorer Density And Single-Line Row Contract

**Files:**
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_explorer_smoke.rs`, add a smoke that proves Flat projection still surfaces path hints but the Slint row contract no longer depends on a taller second line:

```rust
#[test]
fn flat_projection_rows_keep_path_hints_without_multiline_row_contract() {
    let app = AppWindow::new().unwrap();
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();

    let folder_id = app.get_console_asset_items().row_data(0).unwrap().id.to_string();
    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_confirm_asset_modal_requested();
    app.invoke_toggle_assets_view_mode_requested();

    let row = app.get_console_asset_items().row_data(0).unwrap();
    assert_eq!(row.label.as_str(), "Prod Bastion");
    assert_eq!(row.path_hint.as_str(), "Prod");
    assert!(row.compact_flat_mode);
}
```

In `tests/assets_explorer_ui_contract_smoke.sh`, assert the new row metrics:

```bash
ROW="$ROOT_DIR/ui/components/asset-node-row.slint"
ASSETS="$ROOT_DIR/ui/shell/assets-sidebar.slint"

grep -F 'private property <length> row-height: 28px;' "$ROW" >/dev/null
! grep -F 'root.path-hint == "" ? 36px : 48px' "$ROW" >/dev/null
grep -F 'x: 12px;' "$ASSETS" >/dev/null
grep -F 'width: parent.width - 24px;' "$ASSETS" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_smoke -- --nocapture
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- FAIL because row metrics and list host spacing still follow the old contract

**Step 3: Write minimal implementation**

In `ui/components/asset-node-row.slint`, replace the current two-height contract with a dense single-line Explorer row:

```slint
private property <length> indent: root.compact-flat-mode ? 10px : 10px + (root.depth * 12px);
private property <length> disclosure-width: root.show-disclosure ? 14px : 0px;
private property <length> row-height: 28px;
```

Required behavior:

- `Tree` and `Flat` both use one row height
- `path_hint` remains visible in `Flat`, but renders inline in a secondary text slot instead of a second line
- `ListView` host should tighten outer gutters from `16px` to `12px`
- `AssetNodeRow` should feel like Explorer chrome, not a settings button

In `src/app/bootstrap.rs`, keep emitting `compact_flat_mode` and `path_hint` exactly as before so data semantics do not change.

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
git add ui/components/asset-node-row.slint ui/shell/assets-sidebar.slint src/app/bootstrap.rs tests/assets_explorer_smoke.rs tests/assets_explorer_ui_contract_smoke.sh
git commit -m "feat: densify assets explorer rows"
```

## Task 3: Convert Asset Create Modals To Full English Copy And Reliable Focus

**Files:**
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_modal_smoke.rs`, add copy/focus reset coverage:

```rust
#[test]
fn ssh_modal_resets_to_standard_english_shell_when_reopened() {
    let app = AppWindow::new().unwrap();

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_tab_selected("proxy".into());
    app.invoke_close_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
}
```

In `tests/assets_modal_ui_contract_smoke.sh`, assert there is no Chinese left in the two modal files:

```bash
FOLDER_MODAL="$ROOT_DIR/ui/components/assets-folder-create-modal.slint"
SSH_MODAL="$ROOT_DIR/ui/components/assets-ssh-connection-modal.slint"

! rg -n '[一-龥]' "$FOLDER_MODAL" >/dev/null
! rg -n '[一-龥]' "$SSH_MODAL" >/dev/null
grep -F 'text: "New Folder";' "$FOLDER_MODAL" >/dev/null
grep -F 'text: "New SSH Connection";' "$SSH_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- shell script FAILS because modal copy still contains Chinese

**Step 3: Write minimal implementation**

In `ui/components/assets-folder-create-modal.slint`:

- rename title/button copy to `New Folder`, `Cancel`, `Create`
- keep square Fluent shell
- ensure `focus-input()` still focuses the name field

In `ui/components/assets-ssh-connection-modal.slint`:

- rename title/tab/button/caption copy to English
- `Standard / Tunnel / Proxy / Environment / Advanced`
- placeholder text can stay absent; only shell copy must be English this round
- implement `focus-primary-field()` to focus the first meaningful field on open

In `ui/app-window.slint` and `src/app/bootstrap.rs`:

- when modal opens, call the appropriate focus helper after visibility flips true
- keep close / confirm / reset semantics unchanged

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
git add ui/components/assets-folder-create-modal.slint ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/app/bootstrap.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: translate asset create modals to english"
```

## Task 4: Decouple Context Menu Visual Hover From Structural Open Path And Wire Pointer Corridor

**Files:**
- Modify: `ui/components/assets-context-menu-overlay.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `ui/components/assets-context-menu-row.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_context_menu_smoke.rs`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

In `tests/assets_context_menu_spec.rs`, add pure-state coverage for corridor semantics:

```rust
#[test]
fn corridor_keeps_pointer_alive_between_parent_and_child_columns() {
    let parent = Rect { x: 100.0, y: 100.0, width: 224.0, height: 120.0 };
    let child = Rect { x: 332.0, y: 100.0, width: 224.0, height: 120.0 };

    assert!(should_keep_corridor_open((320.0, 140.0), parent, child));
    assert!(!should_keep_corridor_open((320.0, 40.0), parent, child));
}
```

In `tests/assets_context_menu_smoke.rs`, add a bridge-level expectation that pointer movement callback is wired:

```rust
#[test]
fn pointer_move_callback_exists_for_context_menu_corridor() {
    let app = AppWindow::new().unwrap();
    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_pointer_moved(120.0, 180.0);
    assert!(app.get_assets_context_menu_open());
}
```

In `tests/assets_context_menu_ui_contract_smoke.sh`, assert the callback is connected end-to-end:

```bash
APP_WINDOW="$ROOT_DIR/ui/app-window.slint"
BOOTSTRAP="$ROOT_DIR/src/app/bootstrap.rs"
OVERLAY="$ROOT_DIR/ui/components/assets-context-menu-overlay.slint"

grep -F 'callback assets-context-menu-pointer-moved(length, length);' "$APP_WINDOW" >/dev/null
grep -F 'window.on_assets_context_menu_pointer_moved(move |pointer_x, pointer_y| {' "$BOOTSTRAP" >/dev/null
grep -F 'callback pointer-moved(length, length);' "$OVERLAY" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_spec --test assets_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

Expected:

- FAIL because pointer-move is not handled in Rust yet
- FAIL because current menu interaction still rebuilds hover/open state too aggressively

**Step 3: Write minimal implementation**

In `ui/components/assets-context-menu-overlay.slint`, introduce local visual-hover state that does not immediately rewrite Rust structure on every hover:

```slint
property <int> visual-hover-column: -1;
property <int> visual-hover-row: -1;
```

Required behavior:

- moving over a row updates local hover visuals immediately
- only rows with children, or committed submenu transitions, call `row-hovered(...)` into Rust
- leaf rows should not cause full menu structure replacement just for hover

In `src/app/bootstrap.rs` and `src/shell/view_model.rs`:

- implement `window.on_assets_context_menu_pointer_moved(...)`
- keep `open_path` as the structural submenu truth
- use `should_keep_corridor_open(...)` before collapsing submenu state
- ensure leaf-action invoke still goes straight to `handle_context_menu_leaf_action(...)`

Do not tune colors yet; this task is about interaction stability and structure/visual responsibility split.

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
git add ui/components/assets-context-menu-overlay.slint ui/components/assets-context-menu-column.slint ui/components/assets-context-menu-row.slint ui/app-window.slint src/shell/context_menu.rs src/app/bootstrap.rs src/shell/view_model.rs tests/assets_context_menu_spec.rs tests/assets_context_menu_smoke.rs tests/assets_context_menu_ui_contract_smoke.sh
git commit -m "fix: stabilize assets context menu hover and corridor"
```

## Task 5: Rebuild Explorer And Menu Visual Contrast Ladder

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/sidebar-toolbar-icon-button.slint`
- Modify: `ui/components/assets-context-menu-column.slint`
- Modify: `ui/components/assets-context-menu-row.slint`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `verification.md`

**Step 1: Write the failing tests**

In `tests/assets_context_menu_ui_contract_smoke.sh`, add assertions for dedicated menu/explorer surface tokens:

```bash
TOKENS="$ROOT_DIR/ui/theme/tokens.slint"
MENU_ROW="$ROOT_DIR/ui/components/assets-context-menu-row.slint"
EXPLORER_ROW="$ROOT_DIR/ui/components/asset-node-row.slint"

grep -F 'out property <brush> explorer-row-hover-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> explorer-row-selected-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> menu-row-hover-surface:' "$TOKENS" >/dev/null
grep -F 'out property <brush> menu-row-open-surface:' "$TOKENS" >/dev/null
grep -F 'ThemeTokens.menu-row-hover-surface' "$MENU_ROW" >/dev/null
grep -F 'ThemeTokens.explorer-row-hover-surface' "$EXPLORER_ROW" >/dev/null
```

In `tests/assets_explorer_ui_contract_smoke.sh`, assert row components stopped borrowing the generic button hover token:

```bash
! grep -F 'ThemeTokens.control-hover-surface' "$ROOT_DIR/ui/components/asset-node-row.slint" >/dev/null
! grep -F 'ThemeTokens.control-hover-surface' "$ROOT_DIR/ui/components/assets-context-menu-row.slint" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- FAIL because dedicated explorer/menu tokens do not exist yet

**Step 3: Write minimal implementation**

In `ui/theme/tokens.slint`, add a dedicated surface ladder for explorer/menu interaction:

```slint
out property <brush> explorer-row-hover-surface: dark-mode ? #243041 : #e6edf8;
out property <brush> explorer-row-selected-surface: dark-mode ? #2b3850 : #dce7f7;
out property <brush> menu-row-hover-surface: dark-mode ? #2a3648 : #e2ebfa;
out property <brush> menu-row-open-surface: dark-mode ? #32425a : #d8e6fb;
```

Then update row/button components so:

- explorer rows use explorer-specific hover/selected surfaces
- context menu rows use menu-specific hover/open surfaces
- disabled / planned states remain visibly distinct
- activity rail and toolbar buttons stay on generic control tokens unless the new hierarchy proves they also need adjustment

Record the final visual verification in `verification.md` with a dedicated section such as `Windows Console Assets Style Optimization`.

**Step 4: Run tests to verify they pass**

Run:

```bash
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint ui/components/asset-node-row.slint ui/components/sidebar-nav-button.slint ui/components/sidebar-toolbar-icon-button.slint ui/components/assets-context-menu-column.slint ui/components/assets-context-menu-row.slint tests/assets_context_menu_ui_contract_smoke.sh tests/assets_explorer_ui_contract_smoke.sh verification.md
git commit -m "feat: tune explorer and menu contrast ladder"
```

## Final Verification

After all tasks are complete, run the full focused verification set:

```bash
cargo test --test assets_sidebar_toolbar_spec --test assets_explorer_smoke --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke --test shell_view_model -- --nocapture
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
cargo check
```

Expected:

- all targeted Rust tests PASS
- all UI contract smoke scripts PASS
- `cargo check` succeeds
- `verification.md` records the final results and any residual visual risks

## Done Checklist

- [ ] Flat 模式下 tree controls 保留位置，但为 disabled / ghost 状态
- [ ] explorer pane 比例与内容宽度比当前版本更接近 `VS Code Explorer`
- [ ] tree / flat row 都改为更致密的单行契约
- [ ] flat row 的 `path_hint` 仍然可见，但不再占第二行
- [ ] `New Folder` modal 全量英文
- [ ] `New SSH Connection` modal 全量英文
- [ ] context menu hover 不再闪烁
- [ ] context menu leaf action 不再偶发“点了没反应”
- [ ] submenu corridor 已真正接线并参与 keep-open 判定
- [ ] dark / light 主题下 explorer 与 menu 的 hover / selected 对比明显提升
- [ ] 本轮变更未扩散到 terminal core / SSH runtime / persistence 层
