# Windows Console Assets Context Menu Bugfix2 TDD Spec

Date: 2026-03-18
Status: implementation complete, ready for test-driven follow-up

## Source Inputs

- Design: `docs/plans/2026-03-18-windows-console-assets-context-menu-bugfix2-design.md`
- Implementation Plan: `docs/plans/2026-03-18-windows-console-assets-context-menu-bugfix2-implementation-plan.md`
- Verification: `verification.md` -> `2026-03-18 - Windows Console assets context menu bugfix2`

## Scope

本轮已经把 `Windows Console` 资产区收敛到第二轮 bugfix 目标：

- context menu action metadata 统一为英文 label + icon id，并在 Rust 侧集中解析
- blank-area / item menu 共用紧凑、无标题、单层包裹的 Slint surface
- assets toolbar 切换为 panel-aware descriptor，`Create` 主动作与 tooltip 随 sidebar destination 变化
- 空查询搜索的 click-away 收敛到 shell-level dismiss policy
- inline rename 改为显式 rename session，不再依赖 `TextInput.has-focus` 隐式提交
- create 默认名改为同类型独立编号，使用最小缺失正整数

本轮仍未覆盖：

- 真实 SSH/SFTP runtime、Tokio actor、channel 或持久化资产模型
- snippet / keychain 的真实 create flow，只保留 descriptor 与 UI shell
- rename 冲突提示 UI、批量编辑、上传下载等 planned action 的真实业务实现

## Core Rust Surfaces

### `src/shell/assets.rs`

- `AssetViewMode`
  - `id() -> &'static str`
  - `toggle() -> Self`
- `ConsoleAssetKind`
  - `id() -> &'static str`
  - `from_id(&str) -> Option<Self>`
  - `from_create_action_id(&str) -> Option<Self>`
  - `default_name_prefix() -> &'static str`
- `MockConsoleAssetItem`
  - 仍是当前 shell contract 的资产真源，字段为 `id` / `kind` / `label`
- `next_default_name(kind, items)`
  - 只识别 trim 后严格匹配 `Folder {n}` / `SSH Connection {n}` 的同类型标签
  - 返回同类型最小缺失正整数编号

### `src/shell/context_menu.rs`

- `ContextTargetKind`
  - `BlankArea`
  - `SshConnection`
  - `Folder`
- `ContextMenuActionState`
  - `Enabled`
  - `Disabled`
  - `Planned`
- `ContextMenuActionNode`
  - `id`
  - `label`
  - `icon_id`
  - `state`
  - `children`
  - `divider_before`
- `SelectionContext`
  - `selected_ids`
  - `clipboard_has_asset_payload`
  - `target_mutable`
  - `target_has_active_connection`
- `resolve_action_tree(target, selection)`
  - Rust 侧菜单真源，blank-area / ssh / folder 三套 action tree 都从这里派生
- `visible_columns_for_path(...)`
- `context_menu_column_height(...)`
- `resolve_root_menu_origin(...)`
- `should_keep_corridor_open(...)`

### `src/shell/sidebar.rs`

- `SidebarDestination`
  - `Console`
  - `Snippets`
  - `Keychain`
  - `id() -> &'static str`
  - `title() -> &'static str`
  - `from_id(&str) -> Option<Self>`
- `AssetsToolbarDescriptor`
  - `primary_create_action_id`
  - `primary_create_tooltip`
  - `search_tooltip`
  - `view_mode_tooltip`
  - `tree_expansion_tooltip`
  - `show_tree_controls`
- `toolbar_descriptor_for(destination, view_model)`
  - 让 toolbar 的 create id / tooltip / search copy / tree control 可见性都由 Rust 决定

### `src/shell/view_model.rs`

关键状态：

- `asset_view_mode: AssetViewMode`
- `asset_search_expanded: bool`
- `asset_search_query: String`
- `asset_tree_fully_expanded: bool`
- `console_asset_items: Vec<MockConsoleAssetItem>`
- `selected_asset_ids: Vec<String>`
- `renaming_asset_id: Option<String>`
- `renaming_asset_text: String`
- `context_menu_open: bool`
- `context_menu_target_kind: Option<ContextTargetKind>`
- `context_menu_open_path: Vec<usize>`
- `context_menu_feedback_text: String`
- `next_console_asset_serial: u64`

关键方法：

- toolbar / search / panel state
  - `toggle_asset_view_mode()`
  - `activate_asset_search()`
  - `collapse_asset_search_if_empty()`
  - `dismiss_empty_asset_search_on_shell_interaction() -> bool`
  - `toggle_asset_tree_expansion()`
- create / context menu
  - `handle_assets_create_action(&str)`
  - `open_context_menu_for_target(...)`
  - `close_context_menu()`
  - `handle_context_menu_leaf_action(&str)`
  - `handle_context_menu_escape()`
  - `navigate_context_menu_left()`
  - `navigate_context_menu_right()`
  - `invoke_current_context_menu_item()`
- rename session
  - `begin_asset_rename_session(asset_id, initial_text)`
  - `update_active_asset_rename_draft(text)`
  - `commit_active_asset_rename()`
  - `cancel_active_asset_rename()`
  - `dismiss_active_asset_rename()`
  - compatibility wrappers:
    - `update_asset_rename_draft(asset_id, text)`
    - `commit_asset_rename(asset_id, text)`
    - `cancel_asset_rename(asset_id)`
- test helper
  - `seed_test_asset(kind, label)`

当前行为契约：

- `new-folder` 创建时默认进入 `Folder {n}` rename session
- `new-ssh-connection` 创建时默认进入 `SSH Connection {n}` rename session
- 空白 rename draft 在提交时会回落到该类型的下一个默认编号
- `Esc` 只结束 session，不删除 placeholder 资产
- planned action 保持可点击，但只写入 feedback pill，不执行业务动作

- 本轮没有新增 Rust trait。

## Core Slint Surfaces

### `ui/components/assets-context-menu-row.slint`

- 输入契约：
  - `action-id`
  - `label`
  - `icon-source`
  - `enabled`
  - `planned`
  - `has-children`
  - `divider-before`
- 回调：
  - `invoked(string)`
  - `hovered()`
- 行为：
  - `planned == true` 时仍允许点击，以便把 feedback 回传到 Rust

### `ui/components/assets-context-menu-column.slint`

- `AssetsContextMenuItem` struct:
  - `id`
  - `label`
  - `icon_id`
  - `enabled`
  - `planned`
  - `has_children`
  - `divider_before`
- icon 映射在组件内部完成，当前已覆盖 `add` / `edit` / `copy` / `cut` / `delete` / `arrow-*` / `dismiss` / `window-console` / `folder(-open)`
- 回调：
  - `item-invoked(string)`
  - `item-hovered(int)`

### `ui/components/assets-context-menu-overlay.slint`

- 输入契约：
  - `primary-items`
  - `secondary-items`
  - `tertiary-items`
  - `flow-left`
- 回调：
  - `close-requested()`
  - `item-invoked(string)`
  - `row-hovered(int, int)`
  - `pointer-moved(length, length)`
  - `key-command(string)`
- 行为：
  - `FocusScope` 接收 `Escape` / `LeftArrow` / `RightArrow` / `Return`
  - `hover-open-delay` 与 `corridor-close-delay` 维持多列 hover corridor 语义

### `ui/components/sidebar-toolbar-icon-button.slint`

- 输入契约：
  - `icon-source`
  - `active-icon-source`
  - `active`
  - `enabled`
  - `tooltip-text`
  - `tooltip-source-id`
- 回调：
  - `clicked`
  - `tooltip-open-requested(...)`
  - `tooltip-close-requested(...)`

### `ui/components/sidebar-nav-button.slint`

- 关键回调：
  - `clicked(string)`
  - `pointer-activity-requested()`
  - tooltip open/close callbacks
- 作用：
  - activity bar pointer activity 会先上抛到 shell-level dismiss policy

### `ui/components/asset-node-row.slint`

- 输入契约：
  - `item-id`
  - `item-kind`
  - `label`
  - `selected`
  - `renaming`
  - `rename-text`
- 回调：
  - `clicked(string)`
  - `pointer-activity-requested()`
  - `context-menu-requested(string, string, length, length)`
  - `rename-text-changed(string, string)`
  - `rename-commit-requested(string, string)`
  - `rename-cancel-requested(string)`
- 当前编辑态行为：
  - `TextInput.edited` -> draft 更新
  - `Key.Return` -> commit
  - `Key.Escape` -> cancel
  - 已移除 `changed has-focus` 隐式提交

### `ui/shell/assets-sidebar.slint`

- `ConsoleAssetItem` struct:
  - `id`
  - `kind`
  - `label`
  - `selected`
  - `renaming`
  - `rename_text`
- 关键 callback：
  - `toggle-assets-search-requested()`
  - `assets-search-query-changed(string)`
  - `close-assets-search-requested()`
  - `collapse-assets-search-requested()`
  - `toggle-assets-view-mode-requested()`
  - `toggle-assets-tree-expansion-requested()`
  - `assets-create-action-selected(string)`
  - `asset-context-menu-requested(string, string, length, length)`
  - `asset-rename-text-changed(string, string)`
  - `asset-rename-commit-requested(string, string)`
  - `asset-rename-cancel-requested(string)`
  - `shell-interaction-requested()`
  - tooltip open/close callbacks
- 空白区与 row pointer activity 都会上抛 `shell-interaction-requested()`

### `ui/shell/sidebar.slint`

- 作为 pass-through shell：
  - 转发 toolbar / rename / context menu / tooltip / shell-interaction callbacks
  - 让 activity bar 与 assets sidebar 共用同一条 shell interaction 通道

### `ui/app-window.slint`

- 新增关键 property：
  - `asset-primary-create-action-id`
  - `asset-primary-create-tooltip`
  - `asset-search-tooltip`
  - `asset-view-mode-tooltip`
  - `asset-tree-expansion-tooltip`
  - `asset-show-tree-controls`
  - `asset-rename-active`
  - `assets-context-menu-open`
  - `assets-context-menu-primary-items`
  - `assets-context-menu-secondary-items`
  - `assets-context-menu-tertiary-items`
  - `context-menu-feedback-text`
- 关键 callback：
  - `assets-create-action-selected(string)`
  - `asset-context-menu-requested(...)`
  - `asset-rename-text-changed(...)`
  - `asset-rename-commit-requested(...)`
  - `asset-rename-cancel-requested(...)`
  - `dismiss-active-asset-rename-requested()`
  - `shell-interaction-requested()`
  - `assets-context-menu-action-invoked(string)`
  - `assets-context-menu-key-pressed(string)`
  - `assets-context-menu-row-hovered(int, int)`
  - `close-assets-context-menu-requested()`
- 根层 dismiss surface：
  - `shell-body-rename-dismiss-layer`
  - `shell-body-empty-search-dismiss-layer`
  - `assets-context-menu-dismiss-layer`

## Bootstrap Bridge

### `src/app/bootstrap.rs`

关键桥接职责：

- `sync_sidebar_state(window, state)`
  - 投影 `sidebar_items`、`console_asset_items` 与 toolbar state
- `sync_assets_toolbar_state(window, state)`
  - 投影 panel-aware descriptor、search state、tree state、`asset_rename_active`
- `sync_assets_context_menu_state(window, state)`
  - 投影三列菜单模型与 feedback pill 文本
- `console_asset_items_for(state)`
  - 把 `renaming_asset_id` / `renaming_asset_text` 转成 Slint `renaming` / `rename_text`

关键 callback 绑定顺序：

- panel / toolbar / create / context menu 打开前，先收束 rename session
- shell interaction 优先结束 active rename；没有 rename 时才尝试 dismiss 空查询搜索
- rename draft / commit / cancel 都回到 `ShellViewModel`，然后重新同步 sidebar 投影

这意味着当前优先级实际落地为：

1. rename dismiss
2. context menu dismiss / open transition
3. create action transition
4. empty-search dismiss

## Existing Automated Coverage

- Rust
  - `tests/assets_context_menu_spec.rs`
    - action metadata
    - icon ids
    - planned actions
    - flat blank-area columns
    - menu placement / corridor geometry
  - `tests/assets_context_menu_smoke.rs`
    - bootstrap context menu projection
    - create action projection
    - rename dismiss window callback round-trip
  - `tests/assets_sidebar_toolbar_spec.rs`
    - toolbar descriptor projection
    - empty-search dismiss contract
  - `tests/assets_sidebar_toolbar_smoke.rs`
    - bootstrap toolbar defaults
    - shell-level empty-search dismiss on destination/context menu interaction
  - `tests/shell_view_model.rs`
    - create numbering
    - explicit rename session commit/cancel
    - context menu state and navigation
- Shell smoke
  - `tests/assets_context_menu_ui_contract_smoke.sh`
  - `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
  - `tests/sidebar_tooltip_ui_contract_smoke.sh`
  - `tests/sidebar_assets_smoke.sh`
- Verification sweep
  - `cargo test --test assets_context_menu_spec --test assets_context_menu_smoke --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -- --nocapture`
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

## Required Next-Stage TDD Focus

### 1. Rename Uniqueness And Conflict Resolution

- 当前只保证默认编号与空白 draft 回退路径稳定。
- 需要继续补测试覆盖：
  - 手工 rename 到已存在同类型名称时的最终策略
  - `Folder 01`、`Folder 1 extra`、大小写差异、前后空格的边界
  - rename 到空白字符串后是否始终回退到最小缺失编号

### 2. Dismiss Priority Regression Matrix

- 需要显式覆盖以下交互矩阵：
  - rename 中右键 row
  - rename 中右键 blank-area
  - rename 中切 `Snippets` / `Keychain`
  - rename 中点击 toolbar search / view-mode / tree-expansion / create
  - context menu 已开时点击 shell body 的收束顺序

### 3. Planned Action Delivery

- `Planned` action 目前只落 feedback pill。
- 下一阶段可补：
  - feedback 生命周期
  - planned action 与 enabled action 的视觉差异
  - planned action 后 menu 是否应保持打开的稳定契约

### 4. Snippets / Keychain Real Create Flows

- 当前 `toolbar_descriptor_for()` 已暴露 `new-snippet` / `new-keychain`，但 `ShellViewModel` 尚未实现真实创建。
- 下一阶段需要先写 failing tests，定义：
  - action id 是否进入 no-op / feedback / placeholder 路径
  - panel-specific item model 如何投影到 Slint

### 5. Geometry And Input Robustness

- 多列菜单 hover corridor 需要继续覆盖极端 host size、右下角 anchor、快速穿越列间空隙等场景。
- rename dismiss layer 与 context menu dismiss layer 的命中竞争，建议新增更细的 UI contract smoke 或 renderer-level regression。

## Edge Cases And Risks

### UI / Interaction

- `shell-body-rename-dismiss-layer` 与 `assets-context-menu-dismiss-layer` 都在根窗口层；若后续 overlay 层级变动，容易回归点击优先级。
- 当前 row 左键 `clicked(string)` 还没有真实 selection business logic；未来接入真实列表选择时，要避免与 rename / context menu 命中互相覆盖。
- `asset_rename_active` 只依据 `renaming_asset_id.is_some()` 投影；若后续引入异步删除或后台刷新，必须处理“session 指向已删除项”的异常路径。

### Data Integrity

- `draft-asset-{serial}` 仍只是进程内临时 id，不具备持久化语义。
- 目前默认编号池只看严格格式；自由命名项不会参与编号分配，这是有意限制，但也意味着后续若要“任意名称去重”，需要先更新规则再补测试。

### Concurrency / Async

- 本轮没有新增 Tokio task、actor mailbox 或 channel 通信。
- 一旦后续把 create / rename / delete 接入异步 runtime，必须保证：
  - 所有 Slint UI 更新都通过 `slint::invoke_from_event_loop(...)`
  - 后台 worker 不直接持有或操作 Slint component state
  - channel 回写不会覆盖用户尚未提交的 rename draft
  - actor / channel 在高频 context menu 或 rename 操作下不会出现阻塞、乱序或 stale-state overwrite

## Suggested Entry Commands For Next Phase

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
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
