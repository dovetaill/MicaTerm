# Assets Sidebar Toolbar Bugfix2 TDD Spec

日期: 2026-03-16
来源实现计划: `docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix2-implementation-plan.md`
来源设计文档: `docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix2-design.md`
适用工作树: `feature/assets-sidebar-toolbar-bugfix2`

## 目标摘要

本轮实现完成了 `A1 + B1 + C2 + D2` 方案落地：

- Search anchor 已从 `search-button` 切换为 `AssetsSidebar` 顶部 `toolbar-content` 内容矩形。
- Search overlay 已具备 `Esc` 强制关闭、重复点击 Search 只重新聚焦、不抖动重建的行为。
- `AssetsCreateMenu` 已从 `PopupWindow` 收敛为 `AppWindow` 根层 overlay。
- `Assets` 工具区菜单项已收敛为统一的 `AssetsToolbarMenuRow` 行度量。
- Search / Create 已在 Rust 状态层收敛为互斥关系。

## 关键 Rust 结构与状态接口

### `ShellViewModel`

文件: `src/shell/view_model.rs`

本轮相关字段：

- `asset_search_expanded: bool`
- `asset_search_query: String`
- `asset_create_menu_open: bool`
- `asset_tree_fully_expanded: bool`
- `asset_view_mode: AssetViewMode`

本轮相关方法：

- `toggle_asset_search()`
  - 当前仅作为兼容入口，内部委托给 `activate_asset_search()`
- `activate_asset_search()`
  - 强制打开 Search
  - 同时关闭 Create menu
- `close_asset_search()`
  - 无条件关闭 Search
  - 不清空 `asset_search_query`
- `collapse_asset_search_if_empty()`
  - 仅在 query 为空时关闭 Search
  - 继续服务 outside-click 的“空值才关闭”规则
- `toggle_asset_create_menu()`
  - 若当前打开则关闭
  - 若当前关闭则打开，并同时关闭 Search
- `close_asset_create_menu()`
  - 无条件关闭 Create menu

### 状态语义

- Search 与 Create 必须互斥，任何时刻只允许一个 toolbar overlay 处于打开状态。
- Search query 是独立业务状态，关闭 Search 时不应被清空。
- Create menu 打开时必须压掉 Search，即使 Search query 非空也不能保留展开。

## 关键 UI 组件与回调契约

### `AssetsSidebar`

文件: `ui/shell/assets-sidebar.slint`

关键导出属性：

- `search-anchor-x`
- `search-anchor-y`
- `search-anchor-width`
- `search-anchor-height`
- `create-menu-anchor-x`
- `create-menu-anchor-y`
- `create-menu-anchor-width`
- `create-menu-anchor-height`

关键行为：

- Search anchor 来源是 `toolbar-content.absolute-position` 与 `toolbar-content.width/height`
- Search 已展开时点击 Search button：
  - 不切换业务状态
  - 触发 `focus-assets-search-requested()`
- Search 未展开时点击 Search button：
  - 触发 `toggle-assets-search-requested()`

关键 callback：

- `toggle-assets-search-requested()`
- `focus-assets-search-requested()`
- `assets-search-query-changed(string)`
- `collapse-assets-search-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`

### `Sidebar`

文件: `ui/shell/sidebar.slint`

职责：

- 继续作为 `AssetsSidebar` 与 `AppWindow` 的透传层
- 透传 Search / Create anchor
- 透传 `focus-assets-search-requested()`

### `AssetsSearchPopover`

文件: `ui/components/assets-search-popover.slint`

关键接口：

- `in property <string> query`
- `callback query-changed(string)`
- `callback close-requested()`
- `public function focus-input()`

当前度量约束：

- 高度 `34px`
- 方角 `border-radius: 0px`
- `TextInput.height: 20px`
- `font-size: 13px`

键盘行为：

- `Key.Escape` 触发 `close-requested()`

### `AssetsToolbarMenuRow`

文件: `ui/components/assets-toolbar-menu-row.slint`

职责：

- 统一 `Assets` 工具区菜单行布局
- 固定 `icon-slot + text-slot + trailing stretch` 结构
- 统一 hover / pressed 背景响应

关键接口：

- `in property <string> label`
- `in property <image> icon-source`
- `callback invoked`

### `AssetsCreateMenu`

文件: `ui/components/assets-create-menu.slint`

当前宿主语义：

- `inherits Rectangle`
- 不再使用 `PopupWindow`
- 由 `AppWindow` 根层 overlay 宿主控制 `visible`

关键接口：

- `callback new-folder-selected`
- `callback new-ssh-connection-selected`
- `callback close-requested`
- `public function focus-menu()`

键盘行为：

- `FocusScope` 接收焦点
- `Key.Escape` 触发 `close-requested()`

### `AppWindow`

文件: `ui/app-window.slint`

关键 callback：

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

关键 overlay 语义：

- `assets-search-overlay.visible <=> root.asset-search-expanded`
- `assets-create-menu-overlay.visible <=> root.asset-create-menu-open`
- `changed asset-search-expanded` 打开时调用 `assets-search-overlay.focus-input()`
- `changed asset-create-menu-open` 打开时调用 `assets-create-menu-overlay.focus-menu()`
- `overlay-dismiss-layer.clicked`
  - 调用 `collapse-assets-search-requested()`
  - 调用 `close-assets-create-menu-requested()`

## Bootstrap 绑定点

文件: `src/app/bootstrap.rs`

关键绑定：

- `on_toggle_assets_search_requested`
  - 调用 `state.activate_asset_search()`
- `on_close_assets_search_requested`
  - 调用 `state.close_asset_search()`
- `on_collapse_assets_search_requested`
  - 调用 `state.collapse_asset_search_if_empty()`
- `on_toggle_assets_create_menu_requested`
  - 调用 `state.toggle_asset_create_menu()`
- `on_close_assets_create_menu_requested`
  - 调用 `state.close_asset_create_menu()`

测试阶段应重点确认：

- 每次状态变更后都调用 `sync_assets_toolbar_state()`
- Search / Create 的互斥由 ViewModel 统一保证，而不是依赖单个 Slint callback 顺序偶然成立

## 已落地测试覆盖

### UI Contract

文件: `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`

当前覆盖：

- Search anchor 来自 `toolbar-content`
- `focus-assets-search-requested()` callback 透传存在
- Search overlay 暴露 `close-requested()` 和 `Esc` 处理
- `AssetsCreateMenu` 已改为 `Rectangle`
- `AssetsToolbarMenuRow` 已存在并包含 `icon-slot` / `text-slot`
- `AppWindow` 使用 `visible` 驱动 Create overlay，而不是 `.show()` / `.close()`

### Rust Spec

文件: `tests/assets_sidebar_toolbar_spec.rs`

当前覆盖：

- 空 query 时 collapse Search
- 非空 query 时 outside-click 不关闭 Search
- force close Search 不清空 query
- `activate_asset_search()` 会关闭 Create
- `toggle_asset_create_menu()` 在打开时会关闭 Search

### Window Smoke

文件: `tests/assets_sidebar_toolbar_smoke.rs`

当前覆盖：

- Search / Create 初始状态
- Search query round-trip
- `close-assets-search-requested` 的窗口契约
- Search anchor 宽度跟随 toolbar content rect
- Create / Search 在窗口契约中保持互斥

## 下一阶段 TDD 建议

建议下一轮优先补以下测试，而不是先继续改实现：

1. Slint backend 级别的 `Esc` 行为测试
   - 验证 Search 聚焦时按 `Esc` 会回调 `close-assets-search-requested`
   - 验证 Create menu 聚焦时按 `Esc` 会回调 `close-assets-create-menu-requested`

2. outside-click 细粒度测试
   - Search query 为空时点击 dismiss layer 会关闭
   - Search query 非空时点击 dismiss layer 仍保持展开
   - Create menu 打开时点击 dismiss layer 无条件关闭

3. overlay 几何测试
   - 窗口宽度变化后 Search width 与 `toolbar-content.width` 一致
   - Create overlay 的右边界与 trigger 对齐

4. focus 回流测试
   - Search 已展开时重复点击 Search button，只触发 focus，不触发状态变化

## 边缘情况与风险清单

- `overlay-dismiss-layer` 当前同时触发 Search collapse 和 Create close，若后续引入更多 overlay，需要重新梳理优先级与 hit-testing。
- Search 的 outside-click 行为依赖 `collapse_asset_search_if_empty()`，如果未来 query 被异步更新，需确认关闭时机不会与输入事件竞争。
- Create menu 已使用根层 overlay；若后续增加二级菜单，需重新确认 z-order 和 pointer routing。
- `focus-input()` 与 `focus-menu()` 依赖 `changed ... =>` 触发时机；若未来状态变更被合并或延迟，需确认焦点不会丢失。
- 当前没有 Tokio 运行时层面的直接改动，但若后续将 asset search 接入异步过滤：
  - 需要防止 channel 背压导致 UI 状态滞后
  - 需要避免后台结果回流时错误重开已关闭的 Search
  - 需要保证 UI 线程更新通过安全的 event-loop 入口完成

## 推荐下一步验证命令

```bash
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
