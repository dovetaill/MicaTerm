# Windows Console 资产列表 Explorer 行为优化 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把 `Windows Console` 资产 Explorer 的展开图标、同父级唯一命名、统一 `Rename Modal`、递归删除确认与焦点回落完整接线到现有 `Rust state -> bootstrap bridge -> Slint` 壳层。

**Architecture:** Rust 继续作为单一真相源，负责树投影、命名校验、modal state、context menu 动作分发与删除后的 focus 恢复；Slint 只消费投影结果，渲染 disclosure、rename/delete modal 与错误态。实现时先锁测试，再收敛领域逻辑，最后替换 UI 契约，避免同时维护 inline rename 与 modal rename 两条路径。

**Tech Stack:** Rust, Slint, `winit + femtovg-wgpu`, cargo test, shell smoke scripts

---

## 执行前提

- 在独立 worktree 中执行，避免污染当前已确认的设计文档状态。
- 严格按 TDD 顺序推进：先补失败测试，再做最小实现，再跑回归。
- 不要在实现阶段保留“双路径 rename”：
  - 不能同时保留 `inline rename` 与 `Rename Modal`
  - 不能同时保留“按类型避重名”和“同父级统一唯一”

## Task 1: 锁定树投影与命名新契约

**Files:**
- Modify: `tests/assets_explorer_projection.rs:1-149`
- Modify: `tests/shell_view_model.rs:369-556`
- Modify: `src/shell/assets.rs:108-504`

**Step 1: Write the failing tests**

在 `tests/assets_explorer_projection.rs` 添加或改写以下测试：

```rust
#[test]
fn tree_projection_exposes_disclosure_state_for_folder_rows() {
    // collapsed folder => "collapsed"
    // expanded folder => "expanded"
    // leaf ssh row => "none"
}

#[test]
fn parent_scope_uniqueness_blocks_cross_kind_duplicates() {
    // same parent: Folder("Prod") + SSH("Prod") should conflict
}

#[test]
fn next_default_folder_name_uses_dash_suffix_after_base_collision() {
    // Folder 1 exists => next default is Folder 1-1
}
```

在 `tests/shell_view_model.rs` 添加或改写以下测试：

```rust
#[test]
fn create_validation_rejects_duplicate_name_across_kinds_within_same_parent() {}

#[test]
fn unchanged_rename_value_is_treated_as_valid() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_projection --test shell_view_model
```

Expected:

- FAIL because `VisibleAssetRow` 还没有 disclosure state 字段；
- FAIL because 当前命名逻辑仍允许“按类型分别补号”，而不是统一父级唯一；
- FAIL because view model 还没有“校验结果”概念。

**Step 3: Write the minimal implementation**

在 `src/shell/assets.rs` 做最小领域收敛：

```rust
pub enum AssetDisclosureState {
    None,
    Collapsed,
    Expanded,
}

pub enum AssetNameValidation {
    Valid,
    Empty,
    Duplicate,
}

pub fn validate_name_in_parent(
    parent_id: Option<&str>,
    candidate: &str,
    exclude_id: Option<&str>,
) -> AssetNameValidation

pub fn next_default_name_from_base(
    base: &str,
    siblings: &[MockConsoleAssetItem],
) -> String
```

同时调整：

- `VisibleAssetRow` 新增 `disclosure_state`
- `row_from_node()` 不再只输出 `show_disclosure + expanded`
- 默认命名改成：
  - `Folder 1`
  - `Folder 1-1`
  - `Folder 1-2`
- 唯一性判断改成同父级统一唯一，不再区分 `folder` / `ssh`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_explorer_projection --test shell_view_model
```

Expected: PASS

**Step 5: Commit**

```bash
git add tests/assets_explorer_projection.rs tests/shell_view_model.rs src/shell/assets.rs
git commit -m "feat: lock explorer disclosure and naming rules"
```

## Task 2: 为 `AssetTree` 增加递归删除与删除后焦点恢复辅助能力

**Files:**
- Modify: `src/shell/assets.rs:137-432`
- Modify: `tests/assets_explorer_projection.rs:1-149`
- Modify: `tests/shell_view_model.rs:500-556`

**Step 1: Write the failing tests**

在 `tests/assets_explorer_projection.rs` 添加：

```rust
#[test]
fn removing_folder_subtree_removes_all_descendants() {}

#[test]
fn descendant_count_reports_nested_item_total() {}
```

在 `tests/shell_view_model.rs` 添加：

```rust
#[test]
fn deleting_selected_row_focuses_next_sibling_then_previous_then_parent() {}

#[test]
fn deleting_last_root_row_clears_focus_and_selection() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_explorer_projection --test shell_view_model deleting_
```

Expected:

- FAIL because `AssetTree` 还没有 subtree remove / descendant count API；
- FAIL because view model 还没有删除后的 focus fallback 逻辑。

**Step 3: Write the minimal implementation**

在 `src/shell/assets.rs` 增加：

```rust
pub struct RemovedAssetSummary {
    pub removed_ids: Vec<String>,
    pub descendant_count: usize,
}

pub fn descendant_count(&self, node_id: &str) -> Option<usize>

pub fn remove_subtree(&mut self, node_id: &str) -> Option<RemovedAssetSummary>
```

实现要求：

- root 删除时要正确维护 `root_ids`
- child 删除时要从父节点 `children` 中移除
- 返回所有被删节点 id，供 view model 清理 selection / focus / target

在 `src/shell/view_model.rs` 预留 focus fallback helper：

```rust
fn next_focus_target_after_removal(&self, removed_root_id: &str) -> Option<String>
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_explorer_projection --test shell_view_model deleting_
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/assets.rs src/shell/view_model.rs tests/assets_explorer_projection.rs tests/shell_view_model.rs
git commit -m "feat: add recursive asset removal helpers"
```

## Task 3: 用 modal state 替换运行时 rename 路径，并接入 delete confirm

**Files:**
- Modify: `src/shell/view_model.rs:23-105`
- Modify: `src/shell/view_model.rs:264-684`
- Modify: `src/shell/context_menu.rs:239-360`
- Modify: `tests/shell_view_model.rs:115-556`
- Modify: `tests/assets_context_menu_spec.rs:1-290`

**Step 1: Write the failing tests**

在 `tests/shell_view_model.rs` 添加：

```rust
#[test]
fn rename_context_action_opens_single_field_rename_modal() {}

#[test]
fn rename_modal_commit_updates_label_and_closes_modal() {}

#[test]
fn rename_modal_duplicate_name_disables_confirm() {}

#[test]
fn delete_context_action_opens_destructive_confirm_modal() {}

#[test]
fn confirming_folder_delete_removes_descendants_and_restores_focus() {}
```

在 `tests/assets_context_menu_spec.rs` 添加：

```rust
#[test]
fn folder_and_ssh_context_menus_keep_rename_and_delete_as_enabled_leaf_actions() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec
```

Expected:

- FAIL because `AssetModalState` 只有 `NewFolder` / `NewSshConnection`
- FAIL because `handle_context_menu_leaf_action()` 还没有 rename / delete 业务分发
- FAIL because modal confirm 逻辑只覆盖 create

**Step 3: Write the minimal implementation**

扩展 `AssetModalState`：

```rust
pub enum AssetModalState {
    NewFolder { ... },
    NewSshConnection { ... },
    RenameAsset {
        asset_id: String,
        original_name: String,
        draft_name: String,
    },
    DeleteAssetConfirm {
        asset_id: String,
        label: String,
        descendant_count: usize,
    },
}
```

在 `ShellViewModel` 中新增最小方法：

```rust
pub fn open_rename_asset_modal(&mut self, asset_id: String)
pub fn update_rename_asset_modal_name(&mut self, value: String)
pub fn open_delete_asset_confirm(&mut self, asset_id: String)
pub fn confirm_delete_asset(&mut self)
```

实现要求：

- `can_confirm_asset_modal()` 同时支持 rename / delete
- rename 时：
  - 原值不算冲突
  - 同父级同名算冲突
- delete 时：
  - SSH / 空 folder / 非空 folder 都进入 confirm modal
  - 非空 folder 使用 `descendant_count`
- `handle_context_menu_leaf_action()` 把 `rename-asset`、`delete-asset` 接到以上方法

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/shell/context_menu.rs tests/shell_view_model.rs tests/assets_context_menu_spec.rs
git commit -m "feat: route rename and delete through asset modals"
```

## Task 4: 更新 bootstrap 桥接，移除 inline rename 运行时依赖

**Files:**
- Modify: `src/app/bootstrap.rs:168-260`
- Modify: `src/app/bootstrap.rs:467-493`
- Modify: `src/app/bootstrap.rs:835-1099`
- Modify: `tests/assets_context_menu_smoke.rs:86-321`
- Modify: `tests/assets_modal_smoke.rs:1-55`

**Step 1: Write the failing tests**

在 `tests/assets_context_menu_smoke.rs` 添加：

```rust
#[test]
fn rename_action_opens_rename_modal_with_existing_name() {}

#[test]
fn rename_modal_confirm_round_trips_through_window_properties() {}

#[test]
fn delete_action_opens_delete_confirm_modal_with_nested_count() {}

#[test]
fn delete_confirm_round_trips_and_removes_window_rows() {}
```

在 `tests/assets_modal_smoke.rs` 添加：

```rust
#[test]
fn rename_modal_visibility_round_trips_through_window_properties() {}

#[test]
fn delete_modal_visibility_round_trips_through_window_properties() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_context_menu_smoke --test assets_modal_smoke
```

Expected:

- FAIL because `AppWindow` 还没有 rename / delete modal 属性；
- FAIL because bootstrap 还在同步 `editing_asset_id` 与 inline rename callbacks；
- FAIL because `sync_asset_modal_state()` 还只认识 create modals。

**Step 3: Write the minimal implementation**

在 `src/app/bootstrap.rs`：

- 扩展 `sync_asset_modal_state()`，投影：
  - rename modal props
  - delete confirm modal props
- 删除运行时不再需要的 inline rename 同步链：
  - `asset_rename_*` callbacks
  - `asset_rename_active`
  - `dismiss_active_asset_rename_requested`
- 保留 `toggle-expanded-requested` 与 context menu 桥接

建议使用如下桥接形式：

```rust
window.on_asset_rename_modal_name_changed(...)
window.on_confirm_asset_rename_requested(...)
window.on_confirm_delete_asset_requested(...)
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_context_menu_smoke --test assets_modal_smoke
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/assets_context_menu_smoke.rs tests/assets_modal_smoke.rs
git commit -m "refactor: bridge explorer rename and delete modals"
```

## Task 5: 更新 Slint Explorer 行契约与 modal 组件

**Files:**
- Create: `assets/icons/fluent/chevron-right-20-regular.svg`
- Create: `ui/components/assets-rename-modal.slint`
- Create: `ui/components/assets-delete-confirm-modal.slint`
- Modify: `ui/components/asset-node-row.slint:1-180`
- Modify: `ui/shell/assets-sidebar.slint:9-328`
- Modify: `ui/app-window.slint:6-139`
- Modify: `ui/app-window.slint:407-565`
- Modify: `tests/assets_context_menu_ui_contract_smoke.sh:1-68`
- Modify: `tests/assets_modal_ui_contract_smoke.sh:1-27`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh:1-16`

**Step 1: Write the failing UI contract checks**

先更新 shell smoke 断言，显式要求：

```bash
! grep -F 'rename-input := TextInput {' "$ROW" >/dev/null
! grep -F 'callback asset-rename-text-changed(string, string);' "$APP_WINDOW" >/dev/null
grep -F 'export component AssetsRenameModal inherits Rectangle {' "$RENAME_MODAL" >/dev/null
grep -F 'export component AssetsDeleteConfirmModal inherits Rectangle {' "$DELETE_MODAL" >/dev/null
grep -F 'private property <image> chevron-right-icon:' "$ROW" >/dev/null
grep -F 'in property <string> disclosure-state:' "$ROW" >/dev/null
```

**Step 2: Run UI contract scripts to verify they fail**

Run:

```bash
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because row 仍存在 `rename-input`
- FAIL because `AppWindow` / `AssetsSidebar` 仍暴露 inline rename callbacks
- FAIL because rename / delete modal 组件尚不存在
- FAIL because disclosure 仍只有 down chevron

**Step 3: Write the minimal implementation**

在 `ui/components/asset-node-row.slint`：

- 新增 `disclosure-state`
- 引入 `chevron-right-icon`
- 删除 inline rename `TextInput`
- 保持 row 只负责：
  - selection
  - disclosure toggle
  - context menu anchor

在 `ui/shell/assets-sidebar.slint`：

- `ConsoleAssetItem` 改成承接 `disclosure_state`
- 删除 inline rename callback 透传

在 `ui/app-window.slint`：

- 引入 `AssetsRenameModal` 与 `AssetsDeleteConfirmModal`
- 增加相应 property / callback
- 删除 `asset-rename-active`、`asset-rename-*` callback、`dismiss-active-asset-rename-requested()` 契约

rename modal 结构要求：

```slint
export component AssetsRenameModal inherits Rectangle {
    in property <string> item-name;
    in property <string> validation-message;
    in property <bool> can-confirm;
    callback name-changed(string);
    callback confirm-requested();
    callback close-requested();
}
```

delete confirm modal 结构要求：

```slint
export component AssetsDeleteConfirmModal inherits Rectangle {
    in property <string> target-label;
    in property <int> descendant-count;
    callback confirm-requested();
    callback close-requested();
}
```

**Step 4: Run UI contract scripts to verify they pass**

Run:

```bash
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add assets/icons/fluent/chevron-right-20-regular.svg ui/components/asset-node-row.slint ui/components/assets-rename-modal.slint ui/components/assets-delete-confirm-modal.slint ui/shell/assets-sidebar.slint ui/app-window.slint tests/assets_explorer_ui_contract_smoke.sh tests/assets_context_menu_ui_contract_smoke.sh tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: replace inline rename with explorer modals"
```

## Task 6: 跑完整回归并补 verification 记录

**Files:**
- Modify: `verification.md`

**Step 1: Run focused Rust tests**

Run:

```bash
cargo test --test assets_explorer_projection --test shell_view_model --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_modal_smoke
```

Expected: PASS

**Step 2: Run UI contract smoke scripts**

Run:

```bash
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/assets_context_menu_ui_contract_smoke.sh
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 3: Run broader explorer smoke coverage**

Run:

```bash
cargo test --test assets_explorer_smoke --test assets_sidebar_toolbar_smoke
```

Expected: PASS

**Step 4: Update verification record**

在 `verification.md` 追加：

```md
## 2026-03-20 Windows Console Assets Explorer Behavior

- disclosure state projection
- parent-scope uniqueness
- rename modal workflow
- recursive delete confirm
- focus fallback after removal
- UI contract cleanup for inline rename removal
```

**Step 5: Commit**

```bash
git add verification.md
git commit -m "test: verify explorer rename and delete workflow"
```

## 额外实现注意事项

- 不要保留旧的 `begin_asset_rename_session()` 运行时入口；如果短期保留 helper，也必须确保生产链路不再调用。
- `tests/assets_context_menu_ui_contract_smoke.sh` 需要从“保证 inline rename 存在”改成“保证 inline rename 已移除”。
- `asset_tree_fully_expanded` 的现有 toolbar 行为不要顺手改语义；本计划只修 row-level disclosure 投影，不扩散到 header policy。
- 若实现时发现 `asset-modal-kind` 已经不适合承载四类 modal，可拆分为更明确的 modal property，但不要在 `AppWindow` 同时挂两套互相重叠的 modal 路径。

## 最终验收标准

- 右键 `Rename` 打开的不是 inline row，而是统一的单字段 modal
- 右键 `Delete` 总会进入确认流程
- 非空 folder 删除确认文案明确体现递归影响
- 同父级 folder / SSH 不允许重名
- 默认命名初版为英文 `Folder 1`、`Folder 1-1`
- disclosure 图标在 `none / collapsed / expanded` 三态下正确渲染
- 现有 create modal 与新的 rename/delete modal 都能通过 bootstrap 正确 round-trip 到 Slint
