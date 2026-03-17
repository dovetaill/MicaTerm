# Mica Term Dark Mode Surface Contrast Design

日期: 2026-03-17  
执行者: Codex  
状态: 方案已确认，待进入实现阶段

## 背景

当前窗口壳层已经完成 `frameless + custom titlebar + left shell + right inspector` 的主骨架，但暗色模式下各模块之间的层级感仍然不足，尤其是：

- 最左侧 `Activity Bar` 与 `AssetsSidebar` 使用了同一层大面背景色，边界主要依赖 1px 描边；
- 中间主工作区与左侧/右侧面板同属冷灰蓝家族，但明度台阶不够，视觉重心不稳定；
- 顶部 `Titlebar`、右侧 `RightPanel`、左侧双栏之间没有形成明确的 chrome hierarchy；
- 亮色模式虽然相对更容易分辨，但也没有建立与暗色模式一致的语义化分层。

用户已确认本轮目标不是重做布局，也不是接入真正的 terminal runtime，而是先把 `surface hierarchy` 做对，让窗口各模块在暗色/亮色模式下都具备稳定、克制、符合 Windows 11 Fluent / Mica 气质的层级关系。

## 目标

- 建立一套语义化 `surface token mapping`，替代当前过少且职责混杂的颜色 token。
- 让暗色模式下的模块对比来自“同色系不同台阶”，而不是只有描边。
- 保留 Windows 11 的 `Mica / Mica Alt` 质感，但把它限制在真正合适的基础层，不让内部面板透明度破坏可读性。
- 让亮色模式跟随同一套 semantic mapping 校正，而不是额外拼一套无关的浅色值。
- 保持现有布局、交互状态机、窗口几何策略与 renderer 路线不变。

## 边界

### 本文档覆盖

- `ui/theme/tokens.slint` 的 surface token 重构方向
- `Titlebar / Activity Bar / AssetsSidebar / Main Workspace / RightPanel` 的层级关系
- 模块边界表达方式（色差、divider、可选 highlight）
- 暗色模式与亮色模式的统一映射原则
- 风险、回滚与验证清单

### 本文档不覆盖

- `wezterm-term` / `termwiz` / `russh` / `russh-sftp` 接入
- 真实 terminal renderer、ANSI/VT 状态绘制
- 新的布局结构、窗口几何、面板尺寸变更
- 新交互模型（例如新增浮层、导航重组、拖拽式面板）
- 业务数据模型或持久化 schema

## 调研摘要

### 1. 最近 Git 历史

和当前问题直接相关的近期提交如下：

- `4f9fc9e feat: flatten shell chrome — Titlebar/RightPanel 收敛为 flat internal chrome`
- `53c4b1d feat: implement assets sidebar toolbar shell`
- `2958754 fix: theme assets search input for dark mode`
- `6648867 test: align shell layout contract with current UI`

结论：

- 当前 shell 几何 ownership 已基本收敛为 flat internal chrome；
- 左侧 toolbar / search / create menu 的交互已经建立；
- 但主题系统仍停留在“少量全局色值 + 局部修补”的阶段，还没有形成系统化的 surface ladder。

### 2. 当前源码证据

- `ui/theme/tokens.slint:6` 到 `ui/theme/tokens.slint:10` 当前只有少量核心 surface token：
  - `shell-surface`
  - `command-tint`
  - `panel-tint`
  - `terminal-surface`
  - `shell-stroke`
- `ui/shell/sidebar.slint:95` 中 `Activity Bar` 直接使用 `ThemeTokens.shell-surface`。
- `ui/shell/assets-sidebar.slint:36` 中 `AssetsSidebar` 也直接使用 `ThemeTokens.shell-surface`。
- `ui/app-window.slint:251` 中主工作区使用 `ThemeTokens.terminal-surface`。
- `ui/shell/right-panel.slint:11` 中 `RightPanel` 使用 `ThemeTokens.panel-tint`。
- `ui/shell/titlebar.slint:59` 中 `Titlebar` 使用 `ThemeTokens.command-tint`。

这说明当前模块分层主要是：

- `Titlebar = command-tint`
- `RightPanel = panel-tint`
- `Workspace = terminal-surface`
- `Activity Bar + AssetsSidebar = 同一个 shell-surface`

问题核心不是“颜色不统一”，而是“多个大模块共用同层 surface，导致没有台阶”。

### 3. 当前问题量化

基于当前 token 粗估，暗色大面板之间的相对对比非常低：

- `command-tint` vs `panel-tint` 约 `1.02`
- `shell-surface` vs `terminal-surface` 约 `1.04`
- `panel-tint` vs `terminal-surface` 约 `1.13`

这类对比更适合按钮 hover/pressed 差异，不适合承担“大模块分区”的视觉职责。

### 4. 外部参考结论

#### Fluent 2

Fluent 2 官方明确把界面色彩拆为 `neutral / shared / brand` 三类，其中 neutral palette 用于 surface、text、layout 元素，并强调用不同 neutral 级别建立 hierarchy。  
参考：<https://fluent2.microsoft.design/color>

Fluent 2 的 alias tokens 也明确提供了多级 `Neutral Background` 与 `Neutral Stroke`，例如：

- `colorNeutralBackground1` 到 `colorNeutralBackground6`
- `colorNeutralStroke1`、`colorNeutralStroke2`、`colorNeutralStroke3`

这说明官方推荐的是“中性色阶梯 + 分层描边”，而不是单一 surface。  
参考：<https://fluent2.microsoft.design/color-tokens>

#### Windows Mica

Microsoft Learn 对 Mica 的建议是：把它作为应用的 foundation layer，尤其适合 title bar 区域；不要在应用内部重复叠加 backdrop material。  
参考：<https://learn.microsoft.com/en-us/windows/apps/develop/ui/system-backdrops>

#### skillsmp.com 参考页

`dist/tmp/skillsmp.css` 的 dark theme 采用了更明确的 neutral ladder：

- `background`
- `card`
- `muted`
- `sidebar`
- `sidebar-accent`

这和 Fluent 2 的设计方向一致：同色系、多级 neutral、用台阶形成分层。

## 已确认的设计决策

用户已确认采用以下组合：

- `1B` 语义化 `surface token mapping`
- `2B` `hybrid` 边界表达：中等色差 + 两级 divider + 微弱 highlight
- `3B` 只让 outer chrome / titlebar 保留 Mica 气质，内部 pane 改为稳定近不透明 neutral surface
- `4A` `Titlebar` 最亮、`RightPanel` 次亮、`AssetsSidebar` 居中、`Activity Bar` 稍深、`Workspace` 最深
- `light mode` 跟随同一套 semantic mapping 校正，不拆出另一套随意浅色方案

## 最终设计

## 1. Surface 语义映射

本轮不再继续让少量 token 兼任多个模块，而是建立明确的语义层：

- `window-surface`：整窗基础层
- `titlebar-surface`：顶部 chrome 层
- `activity-surface`：最左侧窄工具栏
- `assets-surface`：左侧资产面板
- `workspace-surface`：中间主工作区
- `inspector-surface`：右侧 pane / inspector
- `divider-subtle`：常规模块分隔
- `divider-strong`：更强的区域边界
- `control-hover-surface`：按钮 hover
- `control-active-surface`：按钮 active / pressed

原则：

- token 命名表达“语义角色”，不表达“颜色长相”；
- 亮暗主题只替换值，不替换语义；
- 大模块与小控件分开建模，避免未来继续复用错误层级。

## 2. 模块层级地图

### 暗色模式

最终层级采用：

1. `Titlebar`：最亮的一档 chrome
2. `RightPanel`：第二亮的 inspector surface
3. `AssetsSidebar`：中间层，承接左侧信息结构
4. `Activity Bar`：比 `AssetsSidebar` 更深，形成 rail 感
5. `Workspace`：最深，作为视觉焦点承载区

这套顺序可以让用户在第一眼就理解：

- 顶部是 chrome；
- 右侧是辅助信息层；
- 左侧是导航与资产层；
- 中间才是主内容。

### 亮色模式

亮色模式保持同一语义顺序，不改成另一种逻辑：

1. `Titlebar`：略带冷色的最强 chrome
2. `RightPanel`：次一级的 inspector 背景
3. `AssetsSidebar`：略低于 inspector
4. `Activity Bar`：稍更克制、更接近工具栏
5. `Workspace`：最接近纯白的主内容层

换句话说，light mode 不是简单“全部一起变浅”，而是保留相同 hierarchy，只把值映射到浅色 neutral ladder。

## 3. Mica / 透明度策略

继续保留当前 Win32 + `MicaAlt` 的窗口基础层，但收敛使用范围：

- `window background` / `titlebar chrome` 保留 Mica 气质
- 内部 `Activity Bar / AssetsSidebar / Workspace / RightPanel` 改为稳定的近不透明 neutral surface
- 不再依赖多个内层半透明面板叠加出层级

原因：

- Mica 适合作为 foundation layer，不适合作为所有内部面板的主要分层机制；
- 内层面板越透明，越容易受到桌面壁纸和背板 tint 影响，导致边界不稳定；
- 模块对比应该由 token hierarchy 决定，而不是“今天壁纸比较浅/比较深”决定。

## 4. 边界表达策略

采用 `hybrid` 方案：

- 主要边界来自中等幅度的 surface 差异
- 常规相邻区域使用 `divider-subtle`
- 强分区（例如 `workspace` 与 `right panel` 的接缝）使用 `divider-strong`
- 对 `Titlebar` 和 `RightPanel` 允许一条非常弱的内侧 highlight，用来表达“raised chrome / inspector”感

明确不采用的方式：

- 只靠 1px 线切块
- 大量阴影
- 大幅色相偏移
- 重新引入圆角卡片语义

## 5. 建议的目标色阶

以下不是最终死锁的像素级数值，而是实现阶段建议采用的起始值区间；允许在实现时做小幅微调，但不能破坏已确认的层级关系。

### 暗色模式建议起始值

| 语义 | 建议起始值 | 说明 |
| --- | --- | --- |
| `window-surface` | `#171a20` | 整窗底层，承接 MicaAlt |
| `titlebar-surface` | `#202734` | 最亮 chrome，不要过蓝 |
| `activity-surface` | `#14181f` | 比 assets 更深，形成 rail |
| `assets-surface` | `#1a2029` | 左侧信息面板中层 |
| `workspace-surface` | `#101419` | 最深主工作区 |
| `inspector-surface` | `#1e2632` | 次亮 inspector |
| `divider-subtle` | `#ffffff14` | 常规分隔 |
| `divider-strong` | `#ffffff22` | 强分隔 |

### 亮色模式建议起始值

| 语义 | 建议起始值 | 说明 |
| --- | --- | --- |
| `window-surface` | `#f4f6fa` | 整窗底层 |
| `titlebar-surface` | `#edf3fb` | 最强 chrome |
| `activity-surface` | `#eef2f7` | 稍克制的工具栏 |
| `assets-surface` | `#f7f9fc` | 左侧信息面板 |
| `workspace-surface` | `#ffffff` | 主内容层 |
| `inspector-surface` | `#e9eef7` | 次级 inspector |
| `divider-subtle` | `#0f172a12` | 常规分隔 |
| `divider-strong` | `#0f172a1e` | 强分隔 |

约束：

- 所有面板仍然保持同一冷中性色家族；
- `Titlebar` 与 `RightPanel` 可以略偏冷蓝灰，但不能偏成明显品牌蓝；
- `Workspace` 必须始终是主内容焦点，不允许被 `RightPanel` 抢成更深或更亮的第一视觉中心。

## 6. 对现有组件的影响范围

### 直接影响

- `ui/theme/tokens.slint`
- `ui/shell/titlebar.slint`
- `ui/shell/sidebar.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/app-window.slint`
- `ui/shell/right-panel.slint`

### 间接受影响

- `ui/components/titlebar-icon-button.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/assets-search-popover.slint`
- `ui/components/titlebar-menu.slint`
- `ui/components/titlebar-tooltip.slint`

原因：

- 一旦 surface token 语义调整，hover / active / popover / menu 的对比逻辑也要跟着校正；
- 否则大面板分层变清晰后，按钮态可能反而变得过亮或过弱。

## 实施步骤

以下是设计级实施顺序，不展开为实现级 patch 清单：

1. 在 `ui/theme/tokens.slint` 中扩展 surface ladder 与 divider ladder。
2. 先改大面板映射：`Titlebar / Activity Bar / AssetsSidebar / Workspace / RightPanel`。
3. 再调整按钮与浮层使用的 hover / active / popover surface，确保不和新大面板层级冲突。
4. 单独校正 dark mode 截图，再用同一语义映射映射到 light mode。
5. 补一轮 UI contract / smoke 断言，锁定 token 使用与主布局层级关系。

## 风险与回滚

### 风险

- token 扩容后，如果旧组件继续引用过于泛化的旧 token，可能出现局部过亮或过暗；
- `RightPanel` 抬升过多会抢占主工作区焦点；
- `Titlebar` 与 `RightPanel` 如果同时过亮，窗口会显得头重脚轻；
- light mode 如果只是机械反色，容易丢失暗色模式里已经建立的 hierarchy。

### 回滚策略

- 保留语义 token 结构，不回滚到“少量泛化 token”的旧模型；
- 若某一面板层级不理想，只回调该 token 的值，不撤销整体 semantic mapping；
- 若微弱 highlight 证明多余，只去掉 highlight，不退回到纯 divider-first 方案；
- 若 `RightPanel` 抬升感过强，优先降低其 surface 明度，而不是把 `Workspace` 再拉亮。

## 验证清单

- [ ] 暗色模式下，`Activity Bar` 与 `AssetsSidebar` 一眼可分，不再依赖细描边才能识别。
- [ ] 暗色模式下，`Workspace` 明确是最深的主内容区，视觉中心稳定。
- [ ] 暗色模式下，`RightPanel` 能感知为 inspector，但不会压过主工作区。
- [ ] `Titlebar` 比其他区域更像 chrome，而不是与内容面板混成一层。
- [ ] 亮色模式遵循同一语义层级，而不是另一套随意色板。
- [ ] 现有布局尺寸、窗口几何、Mica / MicaAlt 路线与 renderer 路线不发生变更。
- [ ] toolbar、popover、tooltip、search input 在新 surface ladder 下仍具备清晰 hover / active / focus 对比。

## 参考资料

### 项目内源码

- `ui/theme/tokens.slint:6`
- `ui/shell/sidebar.slint:95`
- `ui/shell/assets-sidebar.slint:36`
- `ui/app-window.slint:117`
- `ui/app-window.slint:251`
- `ui/shell/right-panel.slint:11`
- `ui/shell/titlebar.slint:59`
- `src/main.rs:26`

### 外部参考

- Fluent 2 Color: <https://fluent2.microsoft.design/color>
- Fluent 2 Color Tokens: <https://fluent2.microsoft.design/color-tokens>
- Microsoft Learn System Backdrops: <https://learn.microsoft.com/en-us/windows/apps/develop/ui/system-backdrops>
- 参考页面快照：`dist/tmp/skillsmp.html`、`dist/tmp/skillsmp.css`

