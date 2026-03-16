# Assets Sidebar Toolbar Bugfix3 TDD Spec

日期: 2026-03-16
状态: 实现完成，待进入 test-driven-development 阶段
分支: `feature/assets-sidebar-toolbar-bugfix3-exec-2`

## 本轮变更范围

本轮只覆盖 `AssetsSidebar` 顶部 Search 的两个问题：

- 空搜索框点击外部区域时要稳定收起
- 暗色模式下 `TextInput` 输入文字与选区颜色要显式绑定主题 token

本轮没有改动 terminal runtime、renderer、SSH/SFTP、`Create` 菜单结构，也没有新增 Rust 状态字段。

## 核心 Rust 结构与状态契约

### `ShellViewModel`

文件: [src/shell/view_model.rs](/home/wwwroot/mica-term/.worktrees/assets-sidebar-toolbar-bugfix3-exec-2/src/shell/view_model.rs)

关键字段：

- `asset_search_expanded: bool`
- `asset_search_query: String`
- `asset_create_menu_open: bool`

关键方法：

- `activate_asset_search()`
  - 展开 Search
  - 同时关闭 `asset_create_menu_open`
- `collapse_asset_search_if_empty()`
  - 仅当 `asset_search_query.is_empty()` 时收起 Search
- `close_asset_search()`
  - 无条件收起 Search
  - 不清空 `asset_search_query`

当前没有新增或修改 trait 接口。本轮 Rust 侧仍以既有 `ShellViewModel` 状态机为唯一真相来源。

## 关键 Slint 组件与回调链

### `AppWindow`

文件: [ui/app-window.slint](/home/wwwroot/mica-term/.worktrees/assets-sidebar-toolbar-bugfix3-exec-2/ui/app-window.slint)

关键回调：

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`

关键宿主：

- `workspace-search-dismiss-layer := TouchArea`
  - 区域从 `sidebar.width` 开始，只覆盖 workspace
  - `enabled` 条件为 `root.asset-search-expanded && root.assets-search-query == ""`
  - `clicked` 只转发 `root.collapse-assets-search-requested()`
- `overlay-dismiss-layer := TouchArea`
  - 仍只服务 `asset-create-menu-open`
  - 不复用为 Search dismiss

### `AssetsSidebar`

文件: [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/.worktrees/assets-sidebar-toolbar-bugfix3-exec-2/ui/shell/assets-sidebar.slint)

关键回调：

- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`

关键宿主：

- `header-search-dismiss-touch := TouchArea`
  - 覆盖 header 背景
  - 仅在空 query 且 Search 展开时启用
- `panel-search-dismiss-touch := TouchArea`
  - 覆盖面板正文背景
  - 仅在空 query 且 Search 展开时启用

交互要求：

- 这两个 TouchArea 必须处于背景层，不能拦截 toolbar button 与 inline search 本体点击
- `Sidebar` 仍只是透传层，不持有新的搜索状态

### `AssetsSearchPopover`

文件: [ui/components/assets-search-popover.slint](/home/wwwroot/mica-term/.worktrees/assets-sidebar-toolbar-bugfix3-exec-2/ui/components/assets-search-popover.slint)

关键回调与函数：

- `query-changed(string)`
- `collapse-requested()`
- `close-requested()`
- `public function focus-input()`

关键输入样式绑定：

- `color: ThemeTokens.text-primary`
- `selection-background-color: ThemeTokens.accent`
- `selection-foreground-color: ThemeTokens.text-primary`

当前仍保留：

- `changed has-focus => { if !self.has-focus { root.collapse-requested(); } }`

这条路径现在是 blur fallback，不再是唯一 dismiss 机制。

## 已确认的行为契约

1. Search 保持 inline row 结构，不回退 root overlay。
2. 点击外部区域只触发 `collapse_asset_search_if_empty()` 语义。
3. 非空 query 在 blur 或 click-away 后必须保持展开。
4. `Esc` 仍走 `close_asset_search()` 语义，并保留现有 query。
5. Search 与 `Create` 菜单继续互斥，但 dismiss 层逻辑彼此独立。
6. 输入颜色必须来自 `ThemeTokens`，不允许在组件内写死 dark/light 十六进制颜色。

## 下一阶段 TDD 重点

### 1. Click-away 路径测试

- 覆盖 `header-search-dismiss-touch`
- 覆盖 `panel-search-dismiss-touch`
- 覆盖 `workspace-search-dismiss-layer`
- 断言空 query 时触发 collapse，非空 query 时不触发关闭

### 2. 关闭语义测试

- 断言 click-away 不会走 force close
- 断言 `Esc` 会走 `close-requested()`
- 断言 close 后 query 仍然保留

### 3. 互斥与层级测试

- Search 展开时 `Create` 菜单仍能按既有规则关闭
- `workspace-search-dismiss-layer` 不能覆盖 sidebar 区域
- 背景 TouchArea 不能吞掉 toolbar button 和 search input 的点击

### 4. 主题契约测试

- 暗色模式下输入文本使用 `ThemeTokens.text-primary`
- 选区背景使用 `ThemeTokens.accent`
- 选区前景使用 `ThemeTokens.text-primary`
- 不允许回归到局部硬编码颜色

## 潜在边缘情况

- Slint `TouchArea` 命中顺序若变化，背景 dismiss host 可能误拦截前景控件点击。
- `workspace-search-dismiss-layer` 如果范围扩得过宽，可能误覆盖 sidebar 内部交互。
- 如果后续有人把 click-away 改成统一 `close_asset_search()`，会破坏非空 query 保持展开的契约。
- 如果只保留 `color` 而遗漏选区颜色，暗色模式选区可见性会再次退化。
- 当前 query 保留策略依赖 `close_asset_search()` 不清空文本；后续若变更此方法，需要同步更新契约测试。

## 当前验证基线

本轮实现完成后，已通过以下验证：

- `bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `cargo test --test assets_sidebar_toolbar_spec --test assets_sidebar_toolbar_smoke --test shell_view_model -q`
- `cargo check -q`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`

建议下一阶段继续以这组命令为回归基线，再补充更细的交互测试。
