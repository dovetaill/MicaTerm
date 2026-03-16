# Assets Sidebar Toolbar Bugfix TDD Spec

日期: 2026-03-16
阶段: 实现完成后的测试交接
适用分支: `feature/assets-sidebar-toolbar-bugfix`

## 目标

为 `AssetsSidebar` 顶部工具区 bugfix 提供下一阶段 TDD 输入，覆盖以下已落地契约：

- `AssetsSidebar` 宽度预算提升到 `288px`
- Search 从 sidebar 内联行迁移为根窗口 anchored overlay
- outside click 通过共享 dismiss layer 统一分发
- Search 保持 `S2B` 语义
  - 空 query: 可收起
  - 非空 query: 保持展开
- `Create` 改为纯 icon trigger
- `Create` 菜单改为右缘对齐、固定 `216px` 宽度、`PopupClosePolicy.no-auto-close`

## 本轮涉及的核心对象

### Rust struct

#### `ShellViewModel`
路径: `src/shell/view_model.rs`

关键字段：

- `asset_search_expanded: bool`
- `asset_search_query: String`
- `asset_create_menu_open: bool`
- `asset_tree_fully_expanded: bool`
- `asset_view_mode: AssetViewMode`

关键方法：

- `toggle_asset_search()`
  - 当前语义是“只打开，不切换关闭”
- `set_asset_search_query(query: String)`
- `collapse_asset_search_if_empty()`
  - 当前是 Search 收起规则的唯一业务入口
- `toggle_asset_create_menu()`
- `close_asset_create_menu()`

#### `ShellMetrics`
路径: `src/shell/metrics.rs`

关键常量：

- `ASSETS_SIDEBAR_WIDTH = 288`
- `FULL_LAYOUT_MIN_WIDTH` 通过 `ASSETS_SIDEBAR_WIDTH` 间接放大

### Trait / interface 变化

本次 feature 没有新增或修改 Rust trait interface。

需要测试的“接口面”主要是：

- Slint 导出的 root/window properties
- Slint callbacks 到 `ShellViewModel` 的绑定链
- root overlay 与 sidebar anchor 的几何契约

## Slint 组件与职责

### `AssetsSidebar`
路径: `ui/shell/assets-sidebar.slint`

职责：

- 提供 Search / tree expansion / view mode / Create 的顶部工具按钮
- 暴露 Search 与 Create trigger 的 anchor 几何

关键导出属性：

- `search-anchor-x`
- `search-anchor-y`
- `search-anchor-width`
- `search-anchor-height`
- `create-menu-anchor-x`
- `create-menu-anchor-y`
- `create-menu-anchor-width`
- `create-menu-anchor-height`

关键回调：

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `collapse-assets-search-requested()`
- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

### `Sidebar`
路径: `ui/shell/sidebar.slint`

职责：

- 透传 `AssetsSidebar` 的 search/create anchor
- 作为 `AppWindow` 根层 overlay 的几何中介

### `AssetsSearchPopover`
路径: `ui/components/assets-search-popover.slint`

职责：

- 作为唯一 Search overlay 宿主
- 展示 `TextInput`
- 将 query 编辑事件透传给根窗口

关键接口：

- `in property <string> query`
- `callback query-changed(string)`
- `public function focus-input()`

### `AssetsCreateMenu`
路径: `ui/components/assets-create-menu.slint`

职责：

- 作为根窗口 `PopupWindow` 菜单
- 保留 action item 的 icon + text 行布局

关键契约：

- `width: 216px`
- `close-policy: PopupClosePolicy.no-auto-close`
- `close-requested`
- `new-folder-selected`
- `new-ssh-connection-selected`

### `AppWindow`
路径: `ui/app-window.slint`

职责：

- 挂载根层 `assets-search-overlay`
- 挂载根层 `assets-create-menu-overlay`
- 挂载共享 `overlay-dismiss-layer`
- 负责 Search 展开后的输入聚焦

关键导出属性：

- `layout-assets-search-anchor-x`
- `layout-assets-search-anchor-y`
- `layout-assets-search-anchor-width`
- `layout-assets-search-anchor-height`
- `layout-assets-create-menu-anchor-x`
- `layout-assets-create-menu-anchor-y`
- `layout-assets-create-menu-anchor-width`
- `layout-assets-create-menu-anchor-height`

关键 UI 规则：

- `assets-search-overlay.x = sidebar.search-anchor-x`
- `assets-search-overlay.y = sidebar.search-anchor-y + sidebar.search-anchor-height + 6px`
- `assets-create-menu-overlay.x = sidebar.create-menu-anchor-x + sidebar.create-menu-anchor-width - self.width`
- `assets-create-menu-overlay.y = sidebar.create-menu-anchor-y + sidebar.create-menu-anchor-height + 6px`
- `overlay-dismiss-layer` 只覆盖 `titlebar` 之下的 body 区域

## Bootstrap 绑定链

路径: `src/app/bootstrap.rs`

当前根窗口 callback 到状态层的绑定为：

- `on_toggle_assets_search_requested`
  - `ShellViewModel::toggle_asset_search()`
- `on_assets_search_query_changed`
  - `ShellViewModel::set_asset_search_query(...)`
- `on_collapse_assets_search_requested`
  - `ShellViewModel::collapse_asset_search_if_empty()`
- `on_toggle_assets_view_mode_requested`
  - `ShellViewModel::toggle_asset_view_mode()`
- `on_toggle_assets_tree_expansion_requested`
  - `ShellViewModel::toggle_asset_tree_expansion()`
- `on_toggle_assets_create_menu_requested`
  - `ShellViewModel::toggle_asset_create_menu()`
- `on_close_assets_create_menu_requested`
  - `ShellViewModel::close_asset_create_menu()`
- `on_assets_create_action_selected`
  - 先 `close_asset_create_menu()`，再记录 action log

测试重点：

- 这些 callback 仍然全都停留在 UI 线程的 `Rc<RefCell<ShellViewModel>>` 路径
- 当前 feature 没有引入后台线程共享状态

## 当前验证基线

已通过：

- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `cargo test --test assets_sidebar_toolbar_smoke -- --nocapture`
- `cargo test --test assets_sidebar_toolbar_spec -- --nocapture`
- `cargo test --test window_shell -- --nocapture`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

## 下一阶段建议补测项

### 1. Search overlay 行为

- 展开 Search 后，`AssetsSearchPopover::focus-input()` 是否总能把焦点给到 `TextInput`
- `asset-search-expanded = true` 且 query 为空时，点击 dismiss layer 后是否关闭
- `asset-search-expanded = true` 且 query 非空时，点击 dismiss layer 后是否保持展开
- Search overlay 出现后，query 编辑是否仍能正确回写到 `AppWindow.assets-search-query`

### 2. Overlay 几何

- Search overlay 的 `x/y` 是否稳定跟随 Search trigger
- Create menu 的右缘对齐在 `288px` sidebar 下是否稳定
- sidebar 收起或 layout threshold 切换时，search/create overlay 是否出现错误残留

### 3. Create 菜单交互

- `Create` icon trigger 的 active 状态是否与 `asset_create_menu_open` 双向一致
- `PopupClosePolicy.no-auto-close` 下，只有共享 dismiss layer 或 action selection 会关闭菜单
- 点击 menu item 后，菜单关闭与 action dispatch 顺序是否正确

### 4. 回归项

- `toggle_asset_tree_expansion()` 在 `flat` 模式下仍不得生效
- `asset_view_mode` 在 tree/flat 间切换时，不应影响 search/create 状态
- `FULL_LAYOUT_MIN_WIDTH` 增大后，窄窗口布局切换阈值应保持一致性

## 重点边缘情况

### UI 线程与并发

- 当前实现没有引入新的 Tokio task、channel 或跨线程共享状态
- 当前状态更新仍在 Slint UI 回调线程完成，因此本轮没有新增 data race 面
- 如果后续把资产搜索、树数据或创建向导接到 Tokio runtime：
  - 不要从后台线程直接触碰 Slint component
  - 统一使用 `slint::invoke_from_event_loop(...)` 回到 UI 线程
  - 若引入 channel，需补测 burst update、close-after-send、UI teardown 后消息迟到

### z-order 与点击穿透

- `overlay-dismiss-layer` 必须位于 body overlays 下方，否则会吞掉 Search 输入和 Create 菜单点击
- Search overlay 与 Create menu 同时打开时，dismiss layer 应同时发送两类关闭请求，但最终状态仍由 `ShellViewModel` 决定

### 状态语义

- `toggle_asset_search()` 当前不是 toggle，而是单向 open；测试不应错误假设“再次点击会关闭”
- Search 的真正关闭行为由 `collapse_asset_search_if_empty()` 决定，不应在 UI 层私自复制规则

## 建议的测试组织

- 继续保留 `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 作为静态结构契约
- 将交互行为优先放到 `tests/assets_sidebar_toolbar_smoke.rs`
- 将纯状态语义继续放在 `tests/assets_sidebar_toolbar_spec.rs`
- 宽度与布局预算继续放在 `tests/window_shell.rs`

## 结论

下一阶段 TDD 应围绕三条主线展开：

- Search overlay 的焦点、click-away 和 query 持久化
- Create menu 的 icon-only trigger 与 shared dismiss 协作
- `288px` sidebar 宽度对 layout threshold 和 anchor 几何的回归覆盖
