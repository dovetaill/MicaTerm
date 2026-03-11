# Sidebar Navigation TDD Spec

日期: 2026-03-11

## 目标

为 `sidebar navigation shell` 的下一阶段测试补齐稳定输入，覆盖左侧双层导航骨架、Rust 状态同步、tooltip 交互和回归风险。

## 本轮实现范围

- 固定 `48px Activity Bar + 256px Assets Sidebar`
- 一级导航固定为 `Window Console`、`Snippets`、`Keychain`
- `Folder / Folder Open` 仅负责展开与折叠 `Assets Sidebar`
- `ShellViewModel` 作为 sidebar 唯一状态源
- `bootstrap` 将 sidebar 状态同步到 Slint 属性与 `ModelRc`
- sidebar 图标 hover 时复用 overlay tooltip 机制

## 核心 Rust 结构

### `SidebarDestination`

文件:
[src/shell/sidebar.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/src/shell/sidebar.rs)

职责:

- 定义一级导航目标枚举
- 提供 `id()` 供 Slint 字符串属性同步
- 提供 `title()` 供导航标签与 tooltip 文案复用
- 提供 `from_id()` 供 `AppWindow` callback 回写 Rust 状态

当前值:

- `Console`
- `Snippets`
- `Keychain`

### `ShellViewModel`

文件:
[src/shell/view_model.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/src/shell/view_model.rs)

与 sidebar 直接相关的字段:

- `show_assets_sidebar: bool`
- `active_sidebar_destination: SidebarDestination`

与 sidebar 直接相关的方法:

- `toggle_assets_sidebar()`
- `select_sidebar_destination(destination)`

当前契约:

- 默认 `show_assets_sidebar == true`
- 默认 `active_sidebar_destination == SidebarDestination::Console`
- 折叠 sidebar 不丢当前 destination
- 选择 destination 会强制展开 `Assets Sidebar`

### `sidebar_items_for(state)`

文件:
[src/shell/sidebar.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/src/shell/sidebar.rs)

职责:

- 将 `ShellViewModel` 转成 `Vec<SidebarNavItem>`
- 为 Slint 提供 `id`、`label`、`active`
- 当前实现每次状态变化重建 `VecModel`

### `bootstrap` 同步入口

文件:
[src/app/bootstrap.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/src/app/bootstrap.rs)

关键函数:

- `sync_sidebar_state(window, state)`
- `sync_shell_state(window, state, effects)`
- `bind_top_status_bar_with_store_and_effects(...)`

当前行为:

- 初始化时同步 `show_assets_sidebar`
- 初始化时同步 `active_sidebar_destination`
- 初始化时同步 `sidebar_items`
- 响应 `toggle_assets_sidebar_requested`
- 响应 `sidebar_destination_selected`

## 核心 Slint 接口

### `SidebarNavItem`

文件:
[ui/shell/sidebar.slint](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/ui/shell/sidebar.slint)

字段:

- `id: string`
- `label: string`
- `active: bool`

### `Sidebar`

文件:
[ui/shell/sidebar.slint](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/ui/shell/sidebar.slint)

输入属性:

- `items: [SidebarNavItem]`
- `show-assets-sidebar: bool`
- `active-sidebar-destination: string`

输出属性:

- `tooltip-text: string`
- `tooltip-visible: bool`
- `tooltip-anchor-x: length`
- `tooltip-anchor-y: length`
- `tooltip-anchor-width: length`

回调:

- `toggle-assets-sidebar-requested()`
- `destination-selected(string)`

内部职责:

- 维护 `Activity Bar` 与 `Assets Sidebar`
- 维护 sidebar tooltip 的 delay / debounce 状态机
- 将 toggle 按钮 tooltip 文案在 `Collapse sidebar` 与 `Expand sidebar` 间切换

### `SidebarNavButton`

文件:
[ui/components/sidebar-nav-button.slint](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/ui/components/sidebar-nav-button.slint)

输入属性:

- `item-id: string`
- `label: string`
- `active: bool`
- `sidebar-open: bool`
- `tooltip-text: string`
- `tooltip-source-id: string`

回调:

- `clicked(string)`
- `tooltip-open-requested(string, string, length, length, length)`
- `tooltip-close-requested(string)`

当前行为:

- hover 时发出 tooltip open
- 离开时发出 tooltip close
- 点击时先关闭 tooltip 再转发 click

### `TitlebarTooltip`

文件:
[ui/components/titlebar-tooltip.slint](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/ui/components/titlebar-tooltip.slint)

新增兼容属性:

- `anchor-width: length`
- `host-height: length`
- `place-right: bool`

当前用法:

- titlebar 继续使用默认下方定位
- sidebar tooltip 使用 `place-right: true`

### `AppWindow`

文件:
[ui/app-window.slint](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/ui/app-window.slint)

与 sidebar 直接相关的属性:

- `show-assets-sidebar: bool`
- `active-sidebar-destination: string`
- `sidebar-items: [SidebarNavItem]`

与 sidebar 直接相关的回调:

- `toggle-assets-sidebar-requested()`
- `sidebar-destination-selected(string)`

overlay:

- `tooltip-overlay` 供 titlebar 使用
- `sidebar-tooltip-overlay` 供 sidebar 使用

## 现有验证入口

Rust tests:

- [tests/sidebar_navigation_spec.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/sidebar_navigation_spec.rs)
- [tests/sidebar_navigation_smoke.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/sidebar_navigation_smoke.rs)
- [tests/shell_view_model.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/shell_view_model.rs)
- [tests/top_status_bar_smoke.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/top_status_bar_smoke.rs)
- [tests/window_shell.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/window_shell.rs)
- [tests/titlebar_layout_spec.rs](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/titlebar_layout_spec.rs)

Shell smoke:

- [tests/sidebar_assets_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/sidebar_assets_smoke.sh)
- [tests/sidebar_ui_contract_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/sidebar_ui_contract_smoke.sh)
- [tests/sidebar_tooltip_ui_contract_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-sidebar-navigation/tests/sidebar_tooltip_ui_contract_smoke.sh)

## 下一阶段建议测试主题

### Rust 行为测试

- 非法 destination id 是否回退到 `console`
- 连续重复选择同一 destination 时，`sidebar_items_for` 的 active 态是否稳定
- toggle 后再次 toggle，`show_assets_sidebar` 是否正确往返

### Slint / UI 契约测试

- `SidebarNavButton` 的 toggle tooltip 文案是否随 `show-assets-sidebar` 切换
- `sidebar-tooltip-overlay` 是否固定为右侧定位
- `sidebar` 与 `titlebar` tooltip overlay 是否不会共享状态

### 视觉与交互测试

- hover `Window Console` / `Snippets` / `Keychain` 时 tooltip 文案是否等于标签文本
- 展开态 hover toggle 显示 `Collapse sidebar`
- 折叠态 hover toggle 显示 `Expand sidebar`
- 鼠标离开与点击后 tooltip 是否按 debounce 关闭

## 边缘情况

- sidebar tooltip 使用独立状态机，不应与 titlebar tooltip 串态
- toggle 按钮在点击瞬间会触发 close，再触发 sidebar 切换；测试需覆盖快速 hover-click 场景
- `sidebar_items_for` 当前按状态变化全量重建 `VecModel`；列表只有 3 项，当前成本可接受
- `show_assets_sidebar` 当前不持久化；应用重启后应恢复默认展开
- 本轮没有引入新的 Tokio channel、actor 或并发共享状态；若未来 sidebar 接会话流或 keychain 异步加载，再补 channel 阻塞与事件顺序测试

## 本轮最终验证记录

已通过:

- `cargo fmt --all`
- `cargo test --test sidebar_navigation_spec --test shell_view_model --test sidebar_navigation_smoke --test top_status_bar_smoke --test window_shell -q`
- `bash tests/sidebar_assets_smoke.sh`
- `bash tests/sidebar_ui_contract_smoke.sh`
- `bash tests/sidebar_tooltip_ui_contract_smoke.sh`
- `cargo check -q`
- `cargo clippy --all-targets -- -D warnings`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
