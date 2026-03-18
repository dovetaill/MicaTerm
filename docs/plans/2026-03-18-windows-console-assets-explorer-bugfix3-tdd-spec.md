# Windows Console Assets Explorer Bugfix3 TDD Spec

日期: 2026-03-18
阶段: implementation -> test-driven-development handoff
状态: 待进入下一轮测试补强

## 范围摘要

本轮已完成 `Windows Console` 资产区从扁平 placeholder 交互到 Explorer shell 的最小落地：

- Rust 真源改为 `AssetTree`
- `ShellViewModel` 已拆出 `selection / focus / editing / context target`
- Slint 已能消费 `visible_console_asset_rows()` 并渲染 Explorer row metadata
- Console 顶部 `+` 恢复为 create popover
- folder target context-menu create 已支持 child create + parent auto-expand

下一轮 TDD 重点应转向：更细粒度的交互回归、搜索/展开边界、以及完整 context-menu overlay 行为。

## 核心 Struct 与状态

### Rust domain

- `src/shell/assets.rs`
  - `AssetNode`
    - `id: String`
    - `kind: ConsoleAssetKind`
    - `title: String`
    - `parent_id: Option<String>`
    - `children: Vec<String>`
    - `expanded: bool`
  - `VisibleAssetRow`
    - `id: String`
    - `kind: ConsoleAssetKind`
    - `label: String`
    - `depth: usize`
    - `has_children: bool`
    - `expanded: bool`
  - `AssetTree`
    - 负责 canonical tree 存储、root/child 插入、default naming、tree/flat/search projection

- `src/shell/view_model.rs`
  - `ShellViewModel`
    - Explorer 状态字段：
      - `selected_asset_ids: Vec<String>`
      - `focused_asset_id: Option<String>`
      - `editing_asset_id: Option<String>`
      - `editing_asset_text: String`
      - `context_menu_open: bool`
      - `context_target_kind: Option<ContextTargetKind>`
      - `context_target_asset_id: Option<String>`
      - `console_asset_tree: AssetTree`

- `src/shell/context_menu.rs`
  - `ContextTargetKind`
    - `BlankArea`
    - `SshConnection`
    - `Folder`
  - `ContextMenuActionNode`
    - `id`
    - `label`
    - `children`

### Slint window model

- `ui/shell/assets-sidebar.slint`
  - `ConsoleAssetItem`
    - `id`
    - `kind`
    - `label`
    - `depth`
    - `has_children`
    - `expanded`
    - `selected`
    - `focused`
    - `renaming`
    - `rename_text`

## 已落地的关键函数契约

### Tree / projection

- `AssetTree::insert_root(kind, title) -> String`
- `AssetTree::insert_child(parent_id, kind, title) -> String`
- `AssetTree::set_expanded(node_id, expanded)`
- `AssetTree::project_visible_rows(view_mode, search_query) -> Vec<VisibleAssetRow>`
- `AssetTree::next_default_name_for_parent(parent_id, kind) -> String`

### View-model

- `ShellViewModel::visible_console_asset_rows() -> Vec<VisibleAssetRow>`
- `ShellViewModel::handle_assets_create_action(action_id)`
  - toolbar / create-popover create，当前始终写入 root
- `ShellViewModel::open_context_menu_for_target(target_kind, target_id, anchor_x, anchor_y)`
- `ShellViewModel::handle_context_menu_leaf_action(action_id)`
  - folder target -> child create
  - blank target -> root create
- `ShellViewModel::handle_blank_area_click()`
  - commit rename
  - clear selection
  - clear focus
- `ShellViewModel::select_asset(asset_id)`
- `ShellViewModel::toggle_folder_expanded(asset_id)`

## 当前没有新增 Trait

本轮没有引入新的 trait interface。下一轮若引入持久化、异步 actor 或命令总线，建议先定义明确 trait，再进入测试与实现。

## Slint callbacks / bridge contracts

### Create popover

- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

### Explorer rows

- `asset-selected(string)`
- `toggle-expanded-requested(string)`
- `asset-context-menu-requested(string, string, length, length)`

### Context menu action bridge

- `assets-context-menu-action-invoked(string)`

### Search

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`

## 下一轮测试建议

### 1. AssetTree 单测补强

- 更细的 search matrix
  - folder label match
  - child label match
  - mixed-case query
  - empty query reset
- 同父级重复命名冲突
  - folder / ssh 分作用域验证
  - root / child 分作用域验证

### 2. ShellViewModel 状态机测试补强

- 重复右键切换 target 后的 selection/focus 演化
- child create 后 rename cancel / commit 路径
- root create 与 folder create 连续混合操作
- `toggle_folder_expanded()` 对非 folder 节点的 no-op 保护

### 3. Slint smoke 测试补强

- row icon / chevron / focus 边框 contract
- row selection 与 `console-asset-items` 同步
- blank-area right-click callback contract
- create popover 与 context target 混合使用时的 root-create 保证

### 4. 完整 context-menu overlay 测试

本轮只落地了 create routing，还没有恢复完整 overlay 视觉层。下一轮若补全 overlay，应增加：

- open/close state
- anchor geometry
- pointer corridor / hover intent
- keyboard navigation

## 需要重点盯防的边缘情况

- 搜索结果不应修改节点真实 `expanded` 状态
- `Flat` 投影必须忽略 `expanded`
- toolbar create 即使此前存在 folder context target，也必须继续写入 root
- folder context create 必须先展开 parent，再投影 child row
- blank-area click 不能残留 `selection` 或 `focus`
- `editing_asset_id` 与 `focused_asset_id` 不应永久绑定；rename 结束后要允许 focus 独立存在
- 当前实现仍是单线程 UI 状态，没有 Tokio channel / data race 风险；如果下一轮引入异步更新，必须通过 `slint::invoke_from_event_loop` 回到 UI 线程再写入窗口模型

## 建议的下一阶段入口

建议下一轮直接以本文件作为 `test-driven-development` 输入，优先补：

1. `AssetTree` 搜索与命名矩阵
2. `ShellViewModel` context target / rename 状态矩阵
3. Slint row / overlay contract smoke
