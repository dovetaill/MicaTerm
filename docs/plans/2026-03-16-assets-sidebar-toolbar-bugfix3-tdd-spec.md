# Assets Sidebar Toolbar Bugfix3 TDD Spec

日期: 2026-03-16
分支工作目录: `/home/wwwroot/mica-term/.worktrees/assets-sidebar-toolbar-bugfix3-exec`
对应计划: `docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix3-implementation-plan.md`
对应设计: `docs/plans/2026-03-16-assets-sidebar-toolbar-bugfix3-design.md`

## 1. 变更目标

本轮已完成 `AssetsSidebar` 顶部工具区 bugfix3 的 4 个实现任务：

- Search 从 `AppWindow` 根层 overlay 回收为 `AssetsSidebar` 内部的 inline search row
- Search 增加真实占位高度暴露 `search-row-height`
- Search 输入壳层改为双层 frame，并修正 caret/文字区度量
- `Create` 菜单行改为显式几何布局，移除 `HorizontalLayout` 基线依赖

## 2. 核心 Struct 与接口

### Rust 侧核心 Struct

- `ShellViewModel`
  - 字段:
    - `asset_search_expanded: bool`
    - `asset_search_query: String`
    - `asset_create_menu_open: bool`
    - `asset_tree_fully_expanded: bool`
    - `asset_view_mode: AssetViewMode`
  - 关键方法:
    - `activate_asset_search()`
    - `close_asset_search()`
    - `set_asset_search_query(String)`
    - `collapse_asset_search_if_empty()`
    - `toggle_asset_create_menu()`
    - `close_asset_create_menu()`

### Rust 侧绑定入口

- `src/app/bootstrap.rs`
  - `sync_assets_toolbar_state(window, state)` 仍然是 Search/Create/ViewMode/TreeExpansion 的唯一 UI 同步入口
  - `window.on_*` 回调继续在 UI 线程上同步更新 `ShellViewModel`

### Trait 接口

- 本轮没有新增或修改 Rust trait
- `PlatformWindowEffects`、窗口控制、Tokio/Channel 接口均未介入本次改动

## 3. Slint 组件与契约

### `ui/app-window.slint`

- 删除了 root `assets-search-overlay := AssetsSearchPopover`
- 新增:
  - `out property <length> layout-assets-search-row-height: sidebar.search-row-height;`
- 保留:
  - `assets-create-menu-overlay := AssetsCreateMenu`
- 语义变化:
  - `overlay-dismiss-layer` 现在只负责关闭 Create 菜单
  - Search 不再依赖 root anchor 几何

### `ui/shell/sidebar.slint`

- 删除 search anchor 透传
- 新增:
  - `out property <length> search-row-height: assets-sidebar.search-row-height;`
- 回调链路:
  - `toggle-assets-search-requested()`
  - `assets-search-query-changed(string)`
  - `close-assets-search-requested()`
  - `collapse-assets-search-requested()`

### `ui/shell/assets-sidebar.slint`

- 在 header 下方新增 `search-row-host`
- 新增:
  - `out property <length> search-row-height: search-row-host.height;`
- `search-row-host` 当前契约:
  - `height: root.asset-search-expanded ? 44px : 0px;`
  - `clip: true`
- `inline-search := AssetsSearchPopover` 当前契约:
  - `x: 12px`
  - `y: 6px`
  - `width: parent.width - 24px`
  - `visible: parent.height > 0px`

### `ui/components/assets-search-popover.slint`

- Search 当前为双层壳：
  - `glow-frame`
  - `field-frame`
- 关键几何契约:
  - 外层高度 `32px`
  - `search-input.y = 5px`
  - `search-input.height = 22px`
  - `font-size = 13px`
- 关键 callback:
  - `query-changed(string)`
  - `collapse-requested()`
  - `close-requested()`
- 关键 public function:
  - `focus-input()`

### `ui/components/assets-toolbar-menu-row.slint`

- 菜单行不再使用 `HorizontalLayout`
- 关键几何契约:
  - `icon-slot.x = 12px`
  - `icon-slot.y = (parent.height - self.height) / 2`
  - `label-text.x = 38px`
  - `label-text.width = parent.width - 50px`
  - `label-text.height = parent.height`
  - `label-text.vertical-alignment = center`

## 4. Slint Callback 链路

### Search 打开

1. `search-button.clicked`
2. `toggle-assets-search-requested()`
3. `AppWindow.on_toggle_assets_search_requested`
4. `ShellViewModel::activate_asset_search()`
5. `sync_assets_toolbar_state()`

### Search 输入

1. `AssetsSearchPopover.query-changed(value)`
2. `AssetsSidebar.assets-search-query-changed(value)`
3. `AppWindow.on_assets_search_query_changed`
4. `ShellViewModel::set_asset_search_query()`
5. `sync_assets_toolbar_state()`

### Search 失焦折叠

1. `TextInput.changed has-focus`
2. `collapse-requested()`
3. `collapse-assets-search-requested()`
4. `AppWindow.on_collapse_assets_search_requested`
5. `ShellViewModel::collapse_asset_search_if_empty()`

### Search 强制关闭

1. `Esc` 命中 `close-requested()`
2. `close-assets-search-requested()`
3. `AppWindow.on_close_assets_search_requested`
4. `ShellViewModel::close_asset_search()`

### Create 菜单

1. `create-button.clicked`
2. `toggle-assets-create-menu-requested()`
3. `AppWindow.on_toggle_assets_create_menu_requested`
4. `ShellViewModel::toggle_asset_create_menu()`
5. `sync_assets_toolbar_state()`

## 5. 已确认的行为契约

- Search 展开时必须占据真实高度，`layout-assets-search-row-height >= 40.0`
- Search 收起后 `layout-assets-search-row-height == 0.0`
- 空 query 失焦时允许自动折叠
- 非空 query 失焦时不得自动折叠
- `Esc` 强制关闭 Search 时，query 保留
- Search 与 Create 菜单必须互斥
- Create 菜单仍由 root overlay 宿主承载
- Search 不再暴露 root anchor 几何

## 6. 边缘情况与风险

### 当前已处理

- 空搜索失焦折叠与非空搜索保留之间的状态分歧，交给 `collapse_asset_search_if_empty()` 统一处理
- 点击 Create 时，Search 由 `toggle_asset_create_menu()` 关闭，避免两个面板同时可见
- root dismiss layer 不再错误折叠 Search，避免和 inline row 语义冲突

### 后续 TDD 应重点覆盖

- Search 失焦后立即点回 Search 按钮时，焦点恢复与折叠顺序是否稳定
- 深浅色主题下 `glow-frame` 的可见度是否都在可接受范围
- 长文本菜单项是否在 `label-text.width = parent.width - 50px` 下出现不可接受裁切
- 不同 DPI 或字体渲染环境下，`32px / 22px / y: 5px` 的输入度量是否仍视觉居中
- Search query 非空时打开 Create，确认 query 保留且 Search 正确关闭

### 并发与线程说明

- 本轮没有引入新的 Tokio task、Actor 或 channel
- 本轮没有新增共享可变状态，因此不存在新的数据竞争面
- 若后续把 Search 接入异步搜索源，必须通过 `slint::invoke_from_event_loop` 回到 UI 线程，避免后台线程直接写 UI

## 7. 建议的测试入口

- UI contract:
  - `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- 状态语义:
  - `cargo test --test assets_sidebar_toolbar_spec -- --nocapture`
- 窗口级契约:
  - `cargo test --test assets_sidebar_toolbar_smoke -- --nocapture`
- 编译与 lint:
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

## 8. 最终验证快照

- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 通过
- `cargo test --test assets_sidebar_toolbar_spec -- --nocapture` 通过，9/9
- `cargo test --test assets_sidebar_toolbar_smoke -- --nocapture` 通过，8/8
- `cargo check --workspace` 通过
- `cargo clippy --workspace -- -D warnings` 通过
