# Sidebar Layout Shell Bugfix TDD Spec

日期: 2026-03-11

## 目标

为 `sidebar layout shell bugfix` 的下一阶段测试提供稳定输入，覆盖默认恢复尺寸、请求态与有效显示态分离、`ShellFrame / ShellBody` 几何契约，以及窄窗折叠顺序回归。

## 本轮实现范围

- `default_window_size()` 与 `ShellMetrics` 统一为 `1440x900`
- 新增纯 Rust `shell layout policy`
- 将 `show_*` 保留为用户请求态，并新增 `effective-show-*` 作为 UI 有效显示态
- `AppWindow` 重构为 `shell-frame -> titlebar + body-host -> shell-body`
- `body-host` 显式承接 `window height - titlebar height`
- 通过几何 getter 暴露 `titlebar/body/sidebar/right-panel` 的关键尺寸
- 顶部状态栏按钮继续使用 Fluent SVG 资源

## 核心 Rust 结构

### `ShellMetrics`

文件:
[src/shell/metrics.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/src/shell/metrics.rs)

关键常量:

- `WINDOW_DEFAULT_WIDTH`
- `WINDOW_DEFAULT_HEIGHT`
- `WINDOW_MIN_WIDTH`
- `WINDOW_MIN_HEIGHT`
- `MAIN_WORKSPACE_MIN_WIDTH`
- `FULL_LAYOUT_MIN_WIDTH`
- `RIGHT_PANEL_ONLY_MIN_WIDTH`
- `TITLEBAR_HEIGHT`
- `ACTIVITY_BAR_WIDTH`
- `ASSETS_SIDEBAR_WIDTH`
- `RIGHT_PANEL_WIDTH`

当前契约:

- 默认恢复尺寸固定为 `1440x900`
- 响应式折叠顺序固定为 `Assets Sidebar -> RightPanel`
- 主工作区最低预算固定为 `640px`

### `ShellLayoutInput` / `ShellLayoutDecision`

文件:
[src/shell/layout.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/src/shell/layout.rs)

输入:

- `window_width: u32`
- `request_assets_sidebar: bool`
- `request_right_panel: bool`

输出:

- `show_assets_sidebar: bool`
- `show_right_panel: bool`
- `main_workspace_width: u32`

当前契约:

- 宽度足够时保留完整三列布局
- 宽度不足时优先折叠 `Assets Sidebar`
- 再不足时收起 `RightPanel`
- 不会在缩窗时抹掉用户请求态

### `ShellViewModel`

文件:
[src/shell/view_model.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/src/shell/view_model.rs)

与本轮布局修复直接相关的方法:

- `requested_assets_sidebar()`
- `requested_right_panel()`
- `toggle_assets_sidebar()`
- `toggle_right_panel()`
- `select_sidebar_destination(destination)`

当前契约:

- `show_assets_sidebar` / `show_right_panel` 只表达用户 intent
- 有效显示态由 Rust 布局策略统一计算后再下发到 Slint

### `apply_restored_window_size()`

文件:
[src/app/windowing.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/src/app/windowing.rs)

职责:

- 在 bind 阶段显式应用 restored size
- 让运行时窗口尺寸与 `ShellMetrics` 保持单一真源

### `bind_top_status_bar_with_store_and_effects(...)`

文件:
[src/app/bootstrap.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/src/app/bootstrap.rs)

与本轮布局相关的关键逻辑:

- `apply_restored_window_size(window, default_window_size())`
- `sync_shell_layout(window, state, logical_width)`
- `current_window_width(window)`

当前契约:

- 绑定阶段立即应用 restored size
- 初始化时同步一次有效布局态
- `toggle_right_panel_requested`
- `toggle_assets_sidebar_requested`
- `sidebar_destination_selected`
- `shell_layout_invalidated`

以上回调在更新请求态后都会重新计算有效布局态

## 核心 Slint 接口

### `AppWindow`

文件:
[ui/app-window.slint](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/ui/app-window.slint)

布局结构:

- `shell-frame := Rectangle`
- `titlebar := Titlebar`
- `body-host := Rectangle`
- `shell-body := HorizontalLayout`
- `main-workspace := Rectangle`
- `right-panel := RightPanel`

与本轮布局直接相关的属性:

- `show-right-panel: bool`
- `show-assets-sidebar: bool`
- `effective-show-assets-sidebar: bool`
- `effective-show-right-panel: bool`

新增诊断属性:

- `layout-titlebar-height`
- `layout-shell-body-height`
- `layout-main-workspace-width`
- `layout-right-panel-width`
- `layout-activity-bar-width`
- `layout-assets-sidebar-width`

新增回调:

- `shell-layout-invalidated(length, length)`

当前契约:

- `titlebar` 固定顶部壳层
- `body-host.height == root.height - titlebar.height`
- `Sidebar` 消费 `effective-show-assets-sidebar`
- `RightPanel` 消费 `effective-show-right-panel`

### `Sidebar`

文件:
[ui/shell/sidebar.slint](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/ui/shell/sidebar.slint)

关键输入属性:

- `show-assets-sidebar: bool`
- `active-sidebar-destination: string`

新增诊断属性:

- `activity-bar-width`
- `assets-sidebar-width`

当前契约:

- `activity-bar` 固定 `48px`
- `assets-sidebar-width` 绑定真实 `AssetsSidebar.width`
- tooltip 状态机保留原实现，不与 titlebar overlay 串态

### `RightPanel`

文件:
[ui/shell/right-panel.slint](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/ui/shell/right-panel.slint)

关键契约:

- `expanded: bool`
- `width: expanded ? 392px : 0px`
- `visible: expanded`
- `clip: true`

### `WelcomeView`

文件:
[ui/welcome/welcome-view.slint](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/ui/welcome/welcome-view.slint)

当前契约:

- 使用声明式 `VerticalLayout`
- 不再依赖绝对定位文本
- 通过底部 `Rectangle { vertical-stretch: 1; }` 吃满剩余空间

## 几何契约测试入口

Rust tests:

- [tests/shell_layout_policy.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/shell_layout_policy.rs)
- [tests/top_status_bar_smoke.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/top_status_bar_smoke.rs)
- [tests/sidebar_navigation_smoke.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/sidebar_navigation_smoke.rs)
- [tests/window_geometry_spec.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/window_geometry_spec.rs)
- [tests/window_shell.rs](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/window_shell.rs)

Shell smoke:

- [tests/shell_layout_ui_contract_smoke.sh](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/shell_layout_ui_contract_smoke.sh)
- [tests/sidebar_ui_contract_smoke.sh](/home/wwwroot/mica-term/.worktrees/sidebar-layout-shell-bugfix/tests/sidebar_ui_contract_smoke.sh)

当前覆盖重点:

- 默认恢复尺寸
- 默认窗口下 `ShellBody` 高度
- 较大窗口下 `ShellBody` 跟随增长
- `1335px` 时先折叠 `Assets Sidebar`
- `1079px` 时再收起 `RightPanel`

## 后续测试建议

### Rust 行为测试

- `shell_layout_invalidated` 在连续 resize 下是否保持幂等
- `requested_*` 与 `effective_*` 在多次 toggle + resize 交错后是否始终分离
- `layout-main-workspace-width` 在边界宽度附近是否稳定不抖动

### Slint / UI 契约测试

- `titlebar.height` 是否始终固定为 `48px`
- `body-host` 是否在极窄窗口和最大化状态下仍紧贴 `titlebar`
- `right-panel` 展开/收起时是否影响 tooltip overlay 位置
- 顶部状态栏按钮 SVG 来源是否始终来自 `assets/icons/fluent/*.svg`

## 边缘情况

- `show_*` 是请求态，`effective-show-*` 是有效态；测试不能混用二者
- `window_geometry_spec` 在 `i-slint-backend-testing` 下读取几何前需要 `show()`，否则可能只拿到隐式内容高度
- `body-host` 目前依赖显式几何承接剩余高度；后续若再改回顶层布局容器，必须防止 Slint `layoutinfo` 绑定环
- 本轮没有引入新的 Tokio task、channel、actor 或共享可变状态；当前不存在新的数据竞争面
- 后续如果 terminal runtime / SSH / SFTP 异步事件要驱动这些布局属性，必须通过 `slint::invoke_from_event_loop` 回到 UI 线程，避免跨线程直接写 Slint 状态
- 若未来引入 Tokio channel 推送窗口态或 sidebar 内容，需补 `channel backlog`、事件乱序和窗口已销毁时 `Weak` 升级失败的测试

## 本轮最终验证记录

已通过:

- `cargo fmt --check`
- `cargo test --test shell_layout_policy --test top_status_bar_smoke --test sidebar_navigation_smoke --test window_geometry_spec --test window_shell -q`
- `bash tests/shell_layout_ui_contract_smoke.sh`
- `bash tests/sidebar_ui_contract_smoke.sh`
- `cargo check --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --workspace -- -D warnings`
