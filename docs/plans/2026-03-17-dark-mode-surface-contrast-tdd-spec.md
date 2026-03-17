# Dark Mode Surface Contrast TDD Spec

日期: 2026-03-17
状态: 实现完成，待后续测试深化
关联设计: `docs/plans/2026-03-17-dark-mode-surface-contrast-design.md`
关联计划: `docs/plans/2026-03-17-dark-mode-surface-contrast-implementation-plan.md`

## 1. 本轮实现摘要

本轮实现完成了暗色/亮色模式下的 semantic surface ladder 重构，并把主壳层与共享控件统一迁移到新的语义 token：

- 主区域层级已固定为：`Titlebar` 最亮、`RightPanel` 次亮、`AssetsSidebar` 居中、`Activity Bar` 更深、`Workspace` 最深
- `ThemeTokens` 已提供最终语义 token：
  - `window-surface`
  - `titlebar-surface`
  - `activity-surface`
  - `assets-surface`
  - `workspace-surface`
  - `inspector-surface`
  - `divider-subtle`
  - `divider-strong`
  - `control-hover-surface`
  - `control-active-surface`
  - `accent`
  - `text-primary`
- 旧的泛化 alias 已删除：
  - `shell-surface`
  - `shell-stroke`
  - `command-tint`
  - `panel-tint`
  - `terminal-surface`
- 共享控件 hover/active、menu、tooltip、popover、segmented container、status pill 已全部切到新的 semantic mapping

## 2. 主要改动文件

### 2.1 Theme 与主区域

- `ui/theme/tokens.slint`
- `ui/shell/titlebar.slint`
- `ui/shell/sidebar.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/app-window.slint`
- `ui/shell/right-panel.slint`
- `ui/shell/tabbar.slint`

### 2.2 共享控件与浮层

- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`
- `ui/components/titlebar-menu.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/components/assets-create-menu.slint`
- `ui/components/assets-toolbar-menu-row.slint`
- `ui/components/assets-search-popover.slint`
- `ui/components/segmented-control.slint`
- `ui/components/command-entry.slint`
- `ui/components/command-palette.slint`
- `ui/components/status-pill.slint`

### 2.3 合同/回归测试

- `tests/theme_surface_contract_smoke.sh`
- `tests/top_status_bar_ui_contract_smoke.sh`
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `tests/sidebar_ui_contract_smoke.sh`
- `tests/window_shell.rs`

## 3. 核心 Rust 接口与结构

本轮没有新增 Rust `trait`，也没有引入新的业务 `struct`。语义 surface ladder 主要通过 Slint `global` token 暴露，Rust 侧继续复用既有窗口与主题接口。

### 3.1 保持不变但与本轮强相关的 Rust 接口

#### `mica_term::theme::ThemeMode`

位置：`src/theme/spec.rs`

用途：
- 表示 `Dark` / `Light` 主题模式
- 与 Slint `ThemeTokens.dark-mode`、原生窗口外观同步链路绑定

关键点：
- 本轮没有改 `ThemeMode` 定义
- 后续测试需要继续确认 theme toggle 不破坏 semantic token 映射

#### `mica_term::shell::metrics::ShellMetrics`

位置：`src/shell/metrics.rs`

用途：
- 固定 titlebar/activity bar/assets sidebar/right panel/tab bar 等布局预算
- 本轮所有 surface 调整都建立在现有几何不变的前提上

关键点：
- 颜色层级已重构，但几何约束没有变化
- 后续测试不应把颜色回归和布局尺寸回归混在一起

#### `mica_term::app::windowing`

相关函数：
- `window_appearance()`
- `window_command_spec()`
- `next_maximize_state()`

用途：
- 维持 frameless + `MicaAlt` + 自绘窗口控制链路
- 与本轮 surface token 改动共同构成窗口壳层 contract

关键点：
- 本轮不改 native appearance / renderer / geometry strategy
- 主题 surface 重构不能影响 `MicaAlt`、maximize、drag/resize 行为

## 4. 核心 Slint 接口与 callbacks

### 4.1 `ThemeTokens`

位置：`ui/theme/tokens.slint`

这是本轮最核心的 UI contract。后续 TDD 阶段应视为最终单一真源（single source of truth）。

最终 token 列表：

```slint
window-surface
titlebar-surface
activity-surface
assets-surface
workspace-surface
inspector-surface
divider-subtle
divider-strong
control-hover-surface
control-active-surface
accent
text-primary
```

必须保持的事实：
- dark/light 主题共用同一套语义命名，只替换值
- 不允许重新引入旧 alias
- 不允许把品牌蓝重新用于大面板背景

### 4.2 `AppWindow`

位置：`ui/app-window.slint`

关键属性：
- `in-out property <bool> dark-mode`
- `in-out property <bool> show-right-panel`
- `in-out property <bool> show-global-menu`
- `in-out property <bool> show-assets-sidebar`
- `in-out property <bool> effective-show-assets-sidebar`
- `in-out property <bool> effective-show-right-panel`
- `in-out property <string> assets-search-query`
- `in-out property <bool> asset-search-expanded`
- `in-out property <bool> asset-create-menu-open`
- `in-out property <bool> asset-tree-fully-expanded`
- `in-out property <string> asset-view-mode`

关键 callbacks：
- `toggle-theme-mode-requested()`
- `toggle-right-panel-requested()`
- `toggle-global-menu-requested()`
- `toggle-assets-sidebar-requested()`
- `toggle-assets-search-requested()`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`
- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

测试关注点：
- 主题变化时 surface 映射与原生窗口外观保持一致
- search/create overlay 状态互斥
- layout 预算与 exported geometry 不受本轮色彩改动影响

### 4.3 `Titlebar`

位置：`ui/shell/titlebar.slint`

关键 callbacks：
- `drag-requested`
- `drag-double-clicked`
- `toggle-theme-mode-requested`
- `toggle-right-panel-requested`
- `toggle-global-menu-requested`
- `close-global-menu-requested`
- `toggle-window-always-on-top-requested`
- `minimize-requested`
- `maximize-toggle-requested`
- `close-requested`

相关子组件：
- `TitlebarIconButton`
- `WindowControlButton`
- `TitlebarMenu`
- `TitlebarTooltip`

测试关注点：
- hover/active surface 正确使用 neutral token
- close button 的 danger 路径继续保留红色语义，不能被 neutral token 覆盖
- tooltip/menu 的 surface 与 divider contract 稳定

### 4.4 `Sidebar` / `AssetsSidebar`

位置：
- `ui/shell/sidebar.slint`
- `ui/shell/assets-sidebar.slint`

关键 callbacks：
- `toggle-assets-sidebar-requested()`
- `destination-selected(string)`
- `toggle-assets-search-requested()`
- `assets-search-query-changed(string)`
- `close-assets-search-requested()`
- `collapse-assets-search-requested()`
- `toggle-assets-view-mode-requested()`
- `toggle-assets-tree-expansion-requested()`
- `toggle-assets-create-menu-requested()`
- `close-assets-create-menu-requested()`
- `assets-create-action-selected(string)`

相关子组件：
- `SidebarNavButton`
- `SidebarToolbarIconButton`
- `AssetsSearchPopover`
- `AssetsCreateMenu`
- `AssetsToolbarMenuRow`

测试关注点：
- activity rail 与 assets panel 的 surface 层级不能混回同一层
- search row 只在展开时占位
- create menu 锚点、overlay dismiss、search focus 行为不能退化

### 4.5 `RightPanel`

位置：`ui/shell/right-panel.slint`

关键约束：
- `background` 必须是 `ThemeTokens.inspector-surface`
- `left-divider.background` 必须是 `ThemeTokens.divider-strong`

测试关注点：
- inspector 必须可感知为高于 assets/activity、但不能抢过 workspace 视觉焦点

## 5. 已锁定的源码合同

### 5.1 Source contract

- `tests/theme_surface_contract_smoke.sh`
  - 锁定语义 token 存在
  - 锁定主区域 surface/divider 映射
  - 锁定 `ui/` 下不再出现旧 generic token 引用
  - 锁定 `ui/theme/tokens.slint` 不再定义旧 alias

- `tests/window_shell.rs`
  - `semantic_surface_tokens_define_dual_theme_ladder()`
  - `semantic_surface_tokens_lock_the_approved_dual_theme_values()`
  - `semantic_surface_tokens_remove_legacy_surface_aliases()`

### 5.2 UI contract

- `tests/top_status_bar_ui_contract_smoke.sh`
- `tests/assets_sidebar_toolbar_ui_contract_smoke.sh`
- `tests/sidebar_ui_contract_smoke.sh`

这些脚本已经把 hover/active/menu/search/menu-row 的 semantic token 使用固定下来。

## 6. 已执行验证证据

本轮已实际运行并通过：

```bash
bash tests/theme_surface_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/assets_sidebar_toolbar_ui_contract_smoke.sh
bash tests/sidebar_ui_contract_smoke.sh
bash tests/window_theme_contract_smoke.sh
cargo test --test window_shell -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
cargo test --test window_effects -- --nocapture
cargo test --test ui_preferences -- --nocapture
cargo test --test assets_sidebar_toolbar_spec -- --nocapture
cargo test --test assets_sidebar_toolbar_smoke -- --nocapture
cargo test --test shell_layout_policy -- --nocapture
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

## 7. 下一阶段 TDD 建议

建议后续测试阶段按下面顺序推进：

### 7.1 Token contract tests

目标：继续把 semantic ladder 作为只读 contract 固化。

建议覆盖：
- dark/light 双主题最终值
- 旧 alias 永不回归
- `ThemeTokens` 只允许语义命名，不允许重新引入表现型命名

### 7.2 Main region visual hierarchy tests

目标：验证主区域 hierarchy 长期稳定。

建议覆盖：
- `Titlebar > RightPanel > AssetsSidebar > ActivityBar > Workspace`
- `divider-subtle` 和 `divider-strong` 的使用位置不串层
- `Workspace` 始终是主内容焦点，而不是最亮或最抬升的区域

### 7.3 Shared control interaction tests

目标：验证 hover/active/menu/tooltip/search field 的状态语义。

建议覆盖：
- titlebar buttons hover/active
- sidebar buttons hover/active
- close button danger path 不被 neutral token 覆盖
- titlebar menu / tooltip / assets create menu / search field 的 surface + border 对应关系

### 7.4 Theme toggle regression tests

目标：验证切换 dark/light 时视觉语义与 native appearance 同步链路仍正确。

建议覆盖：
- `ThemeMode::toggled()` 触发后的 Slint token 更新
- native `MicaAlt` 路线不被 surface token 重构影响
- 不出现 light mode 下 hierarchy 丢失、dark mode 下模块重新混层

## 8. 需要重点关注的边缘情况

### 8.1 Close button danger path

`WindowControlButton` 的 close button 仍保留红色 danger 行为：
- pressed: `#9f1239`
- hover: `#be123c`

后续不要把这一路径替换成 `control-hover-surface` / `control-active-surface`。

### 8.2 Search focus 与 click-away

`AssetsSearchPopover` 的 `has-focus` 变化会触发 `collapse-requested()`，但窗口/侧栏层还有额外的 click-away 约束：
- 空查询时允许点击空白区折叠 search
- 非空查询时不能因为一次失焦就错误清空用户上下文

这是现有 smoke/spec 已经覆盖的高风险点。

### 8.3 Search / Create overlay 互斥

`asset-search-expanded` 与 `asset-create-menu-open` 当前维持互斥语义。
后续如果扩展 toolbar 交互，必须继续保证：
- 打开 create menu 时 search 正确收敛
- 打开 search 时 create menu 正确关闭
- overlay dismiss 不会误关错层

### 8.4 Native window appearance 不得被 UI surface 重构影响

本轮没有修改：
- `MicaAlt`
- renderer 选择
- frameless drag/resize
- maximize/minimize/restore 策略

后续测试必须继续把这些 contract 和 UI theme contract 分开验证，避免把平台问题误归因到 surface token。

### 8.5 未来若接入异步主题同步

虽然本轮没有修改 Tokio/actor/channel 逻辑，但下一阶段若把主题切换联动到异步状态同步：
- UI 回写必须通过 `slint::invoke_from_event_loop`
- 不要在 UI 线程等待 channel 或阻塞 Tokio runtime
- 不要在主题切换时引入跨线程竞态，导致 `dark-mode` 与原生窗口外观短暂不一致

## 9. 下一步测试输入建议

如果下一轮直接进入 `test-driven-development` 阶段，建议优先把以下内容作为输入：

1. `ThemeTokens` 最终值与旧 alias 清理 contract
2. 主区域 hierarchy 的 source contract + smoke contract
3. shared control hover/active/menu/search surface contract
4. dark/light 切换下的 window appearance regression

