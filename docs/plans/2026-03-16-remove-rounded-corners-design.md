# Mica Term Remove Rounded Corners Design

日期: 2026-03-16
执行者: Codex

## 背景

当前问题不是单一 `Titlebar` 组件的局部样式错误，而是 shell chrome 的几何 ownership 仍然把“圆角”当作恢复态默认策略，导致顶部左右角在视觉上持续异常。

基于当前源码与最近提交，关键事实如下：

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L87) 的 `shell-frame` 仍然在 restored 状态使用 `14px` 圆角，并通过 [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L95) 的 `chrome-host` 做统一裁剪。
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L57) 的 `Titlebar` 本体已经是方角，但外层壳仍然是圆角，因此顶部左右角仍然继承外层几何。
- [src/app/window_state.rs](/home/wwwroot/mica-term/src/app/window_state.rs#L37) 仍将 `Restored | Unknown` 归类为 `WindowChromeMode::Rounded`，并由 [tests/window_state_spec.rs](/home/wwwroot/mica-term/tests/window_state_spec.rs#L5) 与 [tests/window_geometry_spec.rs](/home/wwwroot/mica-term/tests/window_geometry_spec.rs#L105) 固化为测试契约。
- [src/app/window_effects.rs](/home/wwwroot/mica-term/src/app/window_effects.rs#L130) 当前只同步 Windows theme 与 Mica/backdrop，没有设置 Windows 11 原生的 `corner preference`。这意味着即使 Slint 层全部方角，顶层 `HWND` 仍可能被系统继续圆角化。

因此，本轮不再继续修补“顶部栏角落背景”这类局部现象，而是直接把“圆角”从当前 shell chrome 架构中整体撤出，收敛为统一方角体系。

## 目标

- 让窗口外轮廓、顶部状态栏、右侧面板、内部 shell chrome 全部采用方角。
- 删除最近几轮未解决实际问题、但扩大了复杂度的圆角相关状态机和样式路径。
- 在 Windows 11 上同时关闭 Slint 层与原生顶层窗口的圆角，避免只改一层导致残留。
- 保持当前 `frameless + custom titlebar + Mica` 路线不变，不引入新的 UI 框架或新渲染路径。
- 为后续迁移 macOS、Linux、Android、iOS 保留更简单的跨平台外观基线。

## 边界

### 本文档覆盖

- `AppWindow`、`Titlebar`、`RightPanel`、titlebar menu / tooltip / buttons / sidebar buttons / tabs / segmented control 等现有圆角组件的几何收敛
- Windows 11 原生顶层窗口的 `corner preference`
- 圆角状态机、相关测试契约、最近失败尝试残留代码的清理策略
- 风险、回滚方式、验证清单

### 本文档不覆盖

- `wezterm-term` / `termwiz` 接入
- SSH / SFTP 业务逻辑
- 新组件、新布局、新交互模型设计
- 主题色板的大规模重构
- 逐提交的 Git 历史重写

## 调研结论

### 最近提交链路

最近和该问题直接相关的提交为：

- `4f9fc9e feat: flatten shell chrome — Titlebar/RightPanel 收敛为 flat internal chrome`
- `2f99310 顶部状态栏圆角修复`
- `fb4933c feat: flatten shell chrome corner ownership`
- `c8eec37 docs: add top status bar corner background plan docs`

这些提交说明团队已经连续多次尝试修补顶部角问题，但方案都停留在“内部组件 flatten、外层窗口壳仍保持 rounded”这个框架内，因此没有根治。

### 当前源码证据

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L90) 仍保留 `root.use-flat-window-chrome ? 0px : 14px` 的外层圆角切换。
- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L15) 仍暴露 `use-flat-window-chrome`，说明“圆角/方角”仍是可切换架构，不是统一基线。
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L159)、[ui/components/titlebar-icon-button.slint](/home/wwwroot/mica-term/ui/components/titlebar-icon-button.slint#L20)、[ui/components/titlebar-menu.slint](/home/wwwroot/mica-term/ui/components/titlebar-menu.slint#L40) 等内部组件仍残留圆角语义。
- [ui/components/segmented-control.slint](/home/wwwroot/mica-term/ui/components/segmented-control.slint#L1)、[ui/components/sidebar-nav-button.slint](/home/wwwroot/mica-term/ui/components/sidebar-nav-button.slint#L30)、[ui/components/active-tab.slint](/home/wwwroot/mica-term/ui/components/active-tab.slint#L1) 也仍在使用圆角。

### 外部参考

- Windows 11 官方支持通过 `DWMWA_WINDOW_CORNER_PREFERENCE` 指定顶层窗口的圆角偏好，并可使用 `DWMWCP_DONOTROUND` 显式禁用圆角。
- 当前依赖的 `winit 0.30.13` 已提供 `WindowExtWindows::set_corner_preference(CornerPreference::DoNotRound)`，无需直接先上裸 `DwmSetWindowAttribute`。

参考：

- https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_window_corner_preference
- https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners
- https://smithay.github.io/smithay/winit/platform/windows/trait.WindowExtWindows.html

## 设计要点与方案对比

### 设计点 1：外轮廓策略

#### 方案 A：保留现有 `Rounded/Flat` 状态机，只把 restored 默认改成 flat

优点：

- 改动面较小
- 可以较快消除当前窗口壳的圆角

缺点：

- “圆角仍是有效模式”会继续存在
- 未来容易被重新接回 restored 分支
- 相关命名、测试、状态机语义仍然误导

#### 方案 B：删除 shell chrome 的圆角状态机，把 `shell-frame` 永久收敛为方角

优点：

- 与“整个软件去圆角”目标完全一致
- 架构最干净
- 可以同步清理 `use-flat-window-chrome` 及其相关测试契约

缺点：

- 需要更新状态机、绑定和测试，改动面大于方案 A

最终选择：`方案 B`

### 设计点 2：Windows 原生顶层圆角策略

#### 方案 A：只改 Slint，不处理原生 `HWND`

优点：

- 改动最少

缺点：

- Windows 11 仍可能继续给顶层窗口补圆角
- 无法保证截图中的顶角问题完全消失

#### 方案 B：使用 `winit` 的 `CornerPreference::DoNotRound`

优点：

- 走现有依赖能力
- 能同时关闭原生顶层窗口圆角
- Windows-specific 逻辑仍可被局部封装

缺点：

- 需要在现有 window effects / window bootstrap 路线上补一次平台同步

#### 方案 C：直接调用 `DwmSetWindowAttribute`

优点：

- 显式且底层

缺点：

- 引入额外 `unsafe` 和 Win32 维护面
- 对当前依赖条件而言没有必要

最终选择：`方案 B`

### 设计点 3：圆角清理范围

#### 方案 A：只处理顶部状态栏外框

优点：

- 风险最小

缺点：

- 内部仍会保留 button、menu、tooltip 等圆角
- 与“内部也不要圆角”不一致

#### 方案 B：只处理 shell chrome 范围

优点：

- 可以解决当前最直观的界面不一致
- 改动范围相对可控

缺点：

- 应用其他区域仍保留圆角
- 会留下设计语言不统一的尾巴

#### 方案 C：处理整个应用，把所有现有 `border-radius` 统一清零

优点：

- 视觉语言最统一
- 不再需要维护“有些组件圆、有些组件方”的例外
- 最符合本轮目标

缺点：

- 改动范围最大
- 需要复核 hover、active、focus 态在方角下是否仍然成立

最终选择：`方案 C`

### 设计点 4：失败尝试的清理方式

#### 方案 A：只清理当前 `HEAD` 中已证明无效或多余的代码与测试，不改 Git 历史

优点：

- 最安全
- 不影响现有分支协作
- 可以把当前树恢复到可理解状态

缺点：

- 失败提交仍保留在历史中

#### 方案 B：同步重写最近提交历史

优点：

- Git 历史更干净

缺点：

- 超出本轮“纯方案”边界
- 协作风险高

#### 方案 C：保留失败路径并加注释

优点：

- 最省事

缺点：

- 与“删除无用代码”目标冲突

最终选择：`方案 A`

## 最终决策

本轮确认采用以下组合：

- `1B` 删除 shell chrome 的圆角状态机，把 `shell-frame` 永久收敛为方角
- `2B` 在 Windows 上使用 `winit::platform::windows::WindowExtWindows::set_corner_preference(CornerPreference::DoNotRound)`
- `3C` 对整个应用现有圆角组件做统一清零
- `4A` 只清理当前工作树中已经证明无效或多余的代码、测试和文档路径，不重写 Git 历史

由此得到的统一设计原则如下：

- 顶层窗口外轮廓使用方角，不再存在 restored/flat 的圆角切换语义
- 内部 shell chrome 全部使用方角，不再使用 rounded card 作为层级表达手段
- Windows 原生窗口也显式禁用圆角，避免系统补角
- 区域层级改由 `background tint`、`border`、`divider`、`spacing` 与 `hover feedback` 表达，而不是 `border-radius`

## 实施步骤

以下步骤用于后续实现阶段的执行边界，仍属于设计层级，不展开为逐文件实现计划。

### 阶段 1：外层窗口壳收敛

- 从 `AppWindow` 移除圆角状态机入口，例如 `use-flat-window-chrome`
- 让 `shell-frame` 永久使用 `0px` 圆角
- 评估 `chrome-host` 是否仍需要继续承担 rounded clip 语义；若仅为圆角存在，则同步简化

### 阶段 2：Windows 原生 corner preference 同步

- 在 Windows-specific window sync 路线上加入 `CornerPreference::DoNotRound`
- 保持非 Windows 平台为 no-op
- 避免将此逻辑散落在多个窗口生命周期节点

### 阶段 3：全局组件圆角清零

- 清理 Titlebar、RightPanel、titlebar 内按钮、menu、tooltip、sidebar buttons、tab、segmented-control、command-palette、command-entry、status-pill 等现有 `border-radius`
- 复查清零后需要保留的局部分隔线、描边和 hover surface
- 统一使用方角 hover / active / selected 反馈

### 阶段 4：删除失败尝试残留

- 删除仅为 `Rounded/Flat` 状态切换服务的属性、导出 layout 诊断字段和辅助逻辑
- 清理已经不再成立的测试命名与断言，例如 restored 必须 rounded 的契约
- 清理与该方案冲突、但没有实际保留价值的设计文档或测试脚本引用

## 风险与回滚

### 风险

- 全局去圆角会让多个 hover / active 态的观感更硬，需要重新确认是否仍有足够的点击反馈
- 原生 `DoNotRound` 与 `MicaAlt` 组合在不同 Windows 11 build 上需要实际验证
- 一些测试当前直接把“恢复态应为 rounded”写死，修改后会连带触发多处失败
- 某些组件原本依赖圆角来隐藏边缘瑕疵，去掉后可能暴露新的接缝问题

### 回滚策略

- 回滚以“阶段”为单位，而不是把圆角状态机整体恢复
- 若 Windows 原生 corner preference 出现兼容性问题，只回滚原生同步，不回滚 Slint 方角基线
- 若某个局部组件在方角下可用性下降，优先补 hover / divider / spacing，不恢复圆角

## 验证清单

- 启动后 restored 状态下，窗口顶部左右角无任何圆角残留
- Windows 11 下顶层窗口不再被系统补圆角
- Titlebar、RightPanel、menu、tooltip、sidebar buttons、tab、segmented-control、command-entry、command-palette、status-pill 等组件均为方角
- 最大化、还原、贴靠、拖拽、resize 行为不因去圆角而回归
- 所有与 `Rounded/Flat` shell chrome 相关的旧测试契约已更新为方角基线
- 当前树中不再保留只为失败圆角修复尝试而存在的冗余逻辑

## 备注

本设计文档只确认架构方向与清理边界，不在本轮生成 `implementation-plan`。如后续需要执行级拆分，再单独输出 `docs/plans/2026-03-16-remove-rounded-corners-implementation-plan.md`。
