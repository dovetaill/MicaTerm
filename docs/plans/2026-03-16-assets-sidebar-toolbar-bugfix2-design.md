# Assets Sidebar Toolbar Bugfix2 Design

日期: 2026-03-16
执行者: Codex
状态: 方案已确认，待进入实现规划

## 背景

`AssetsSidebar` 顶部工具区在 `2026-03-16` 已完成一轮 overlay 化修复，但当前实际效果与目标视觉仍存在明显偏差：

- Search 浮层没有贴在顶部工具区正下方，而是相对搜索按钮向右漂移，破坏了侧栏内聚感。
- Search 输入框内部的文字区域与 caret 垂直度量不协调，呈现出“光标贴底、下侧空隙偏大”的视觉问题。
- `Create` 下拉菜单中的图标和文字没有形成统一 baseline，对齐质量不达标。
- `Create` 菜单无法通过点击任意空白区域稳定关闭，当前关闭语义与根窗口 dismiss 层存在冲突。

本轮任务不是重做 `AssetsSidebar`，而是在不触碰 terminal runtime、SSH/SFTP、renderer 主路径的前提下，对顶部工具区的 overlay 架构、输入度量和菜单行系统进行一次收敛。

## 目标

- 让 Search 始终稳定地出现在 `AssetsSidebar` 顶部工具区正下方，而不是相对某个按钮漂移。
- 修正 Search 输入框的文字区、caret 区和上下内边距，使其更符合 Windows 11 Fluent 风格下的单行工具搜索栏观感。
- 为 `Assets` 工具区相关菜单建立统一的 `menu row` 度量规则，解决 `Create` 菜单内图标与文字错位问题。
- 统一 `Search` 与 `Create` 的 overlay 语义，使它们共享同一套根窗口 dismiss 模型，并稳定支持 outside-click 与 `Esc` 关闭。
- 保持现有 `AppWindow -> Sidebar -> AssetsSidebar` 架构，避免扩散到标题栏菜单、terminal 视图或业务状态层重构。

## 边界

### 本文档覆盖

- `AssetsSidebar` 顶部工具区的 Search 锚点策略
- Search 输入框的视觉度量与焦点行为
- `Create` 菜单的行布局系统与对齐规则
- `Search` / `Create` 的互斥、outside-click、`Esc` 关闭契约
- 根窗口 overlay 层级与 dismiss 策略

### 本文档不覆盖

- `wezterm-term` / `termwiz` / `russh` / `russh-sftp`
- terminal 渲染、SSH/SFTP 连接、资产树真实数据和搜索算法
- 标题栏 `TitlebarMenu` 的同步重构
- 全局主题 token 重命名或颜色系统重做
- 右侧面板、窗口 frame、动画系统的额外调整

## 调研摘要

### 相关提交

- `53c4b1d feat: implement assets sidebar toolbar shell`
- `1d4de33 fix: complete assets sidebar toolbar bugfix`
- `2470516 fix: finalize assets sidebar toolbar overlays`

结论：

- 当前问题不是业务状态机错误，而是 `Slint` 壳层几何、输入控件度量与 overlay 关闭语义没有完全收敛。
- `Search` 与 `Create` 目前分别处于两种不一致的实现语义中：`Search` 是根层可见组件，`Create` 仍是 `PopupWindow`，关闭却依赖根窗口 `TouchArea`。
- 这种“根层 overlay + PopupWindow 混用”的方案，正是当前关闭不稳定与视觉不一致的主要来源。

### 关键代码位置

- `ui/app-window.slint`
- `ui/shell/sidebar.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/components/assets-search-popover.slint`
- `ui/components/assets-create-menu.slint`
- `ui/components/titlebar-menu.slint`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`

### 官方能力确认

已确认以下官方能力可支撑本轮方案：

- `PopupWindow` 具备外部点击关闭与手动关闭两种策略
- `TextInput` 支持焦点管理，但属于更底层输入控件，必须自己收敛文本区域与 caret 度量
- `absolute-position` 可用于 anchor 参考，但不应被简单当作最终布局语义的唯一来源

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput/>
- <https://docs.slint.dev/latest/docs/slint/guide/development/focus/>

## 方案对比

### 设计点 1：Search 锚点模型

#### 方案 A1：锚定顶部工具区内容矩形

描述：

- Search 不再锚定 `search-button` 本身。
- 改为锚定 `AssetsSidebar` header 内部的“工具区内容矩形”。
- Search 左边界与工具区内容左内边距对齐，宽度跟随工具区内容区。

优点：

- 最符合目标视觉，搜索条真正像“挂在工具区下沿”。
- 不会随着按钮位置变化而向右漂移。
- 更利于后续 toolbar 内部按钮顺序调整。

缺点：

- 需要额外暴露 header content rect 的几何信息。

#### 方案 A2：继续锚定搜索按钮并做 sidebar 内部 clamp

优点：

- 改动更小。
- 保留“由搜索图标弹出”的语义。

缺点：

- 视觉上仍然更像“按钮 popover”，不够像整条工具搜索栏。
- 本质上没有解决“漂移感”。

最终选择：`A1`

### 设计点 2：Search 输入框实现

#### 方案 B1：保留自绘搜索壳，并显式收敛输入度量

描述：

- 保留自绘 `Search` 外壳。
- 显式定义单行输入的高度、上下内边距、文字基线、caret 区与水平留白。

优点：

- 能继续保持当前项目的自绘 Fluent 视觉语言。
- 能精准解决当前 caret 与输入区域不适配的问题。

缺点：

- 输入控件的细节要自己收敛，不能依赖标准控件默认观感。

#### 方案 B2：自绘外壳，内部切换到标准单行输入控件

优点：

- 默认输入行为更稳。
- caret 与文字度量通常更省心。

缺点：

- 容易与现有自绘 UI 的视觉语言脱节。
- 为了压样式，最终仍可能回到较多自定义。

最终选择：`B1`

### 设计点 3：Create 菜单内容对齐方式

#### 方案 C1：仅修 `Create` 当前两行布局

优点：

- 变更范围最小。
- 直接针对当前截图问题。

缺点：

- 只修这一处，后续新增菜单项仍可能重复出现对齐问题。

#### 方案 C2：抽统一 `menu row` 度量规则

描述：

- 抽出统一的 `icon slot + text slot + trailing stretch` 行结构。
- 固定 icon 列宽、行高、上下 padding 和文本 baseline。
- 本轮仅服务于 `Assets` 工具区相关菜单，不扩散到标题栏菜单。

优点：

- 这次修完以后，`Assets` 工具区菜单项的视觉规则会更稳定。
- 既解决当前错位问题，也降低后续局部返工概率。

缺点：

- 比单点修复多一个局部抽象步骤。

最终选择：`C2`

### 设计点 4：Create 关闭语义

#### 方案 D1：保留 `PopupWindow`，回到原生 outside-close

优点：

- 更接近 `Slint` 原生 popup 语义。
- outside-click 行为较自然。

缺点：

- 关闭事件与现有 Rust 侧布尔状态同步仍需额外收敛。
- 仍然与 Search 的根层 overlay 语义不一致。

#### 方案 D2：放弃 `PopupWindow`，改成根窗口同层 overlay

描述：

- `Create` 与 `Search` 都作为 `AppWindow` 中的根层 anchored overlay。
- outside-click 和 `Esc` 统一由根层 dismiss 模型处理。

优点：

- `Search` 与 `Create` 语义完全统一。
- 开关状态、z-order、hit-testing 都在同一坐标系下收敛。
- 最适合当前 bugfix2 的“统一壳层行为”目标。

缺点：

- 需要重做 `Create` 的宿主方式。

#### 方案 D3：保留当前 `PopupWindow + no-auto-close + dismiss layer`

优点：

- 表面改动最小。

缺点：

- 当前问题已经证明该混合策略不稳定。
- 后续仍容易出现点击穿透、关闭时机不一致等边角 bug。

最终选择：`D2`

## 最终决策

本轮最终确认组合为：`A1 + B1 + C2 + D2`。

### 架构决策

- `Search` 与 `Create` 都挂在 `AppWindow` 根层，不再混用 `PopupWindow` 与根层 overlay。
- `AssetsSidebar` 负责输出工具区内容区与触发按钮的 anchor 数据。
- `Sidebar` 继续作为中间透传层。
- `AppWindow` 负责最终 overlay 的布局、显示、互斥和 dismiss。

### Search 决策

- Search 的 anchor 从 `search-button` 改为顶部工具区内容矩形。
- Search 条整体贴在工具区正下方。
- Search 采用自绘单行输入样式，显式收敛视觉度量，不依赖默认输入观感。

### Create 决策

- `Create` 按钮仍保持纯 icon trigger，不恢复文字按钮。
- `Create` 菜单改为根层 overlay。
- 菜单项采用统一的 `menu row` 度量规则，仅覆盖 `Assets` 工具区相关菜单。

### 状态与交互决策

- `Search` 与 `Create` 严格互斥。
- 根窗口任意时刻只允许存在一个 toolbar overlay。
- `Search` 空值 outside-click 关闭，非空 outside-click 保持展开。
- `Create` outside-click 无条件关闭。
- `Esc` 对 `Search` 与 `Create` 都执行无条件关闭。

## 交互契约

### Search

- 点击 `Search` 按钮，若当前关闭，则打开 `Search`，并关闭 `Create`。
- 点击 `Search` 按钮，若当前已打开，则仅重新聚焦输入框，不做闪烁式重建。
- Search 打开后立即聚焦输入框。
- Search query 为空时，点击外部区域关闭。
- Search query 非空时，点击外部区域保持展开。
- 按下 `Esc` 时无条件关闭 Search。

### Create

- 点击 `Create` 按钮，若当前关闭，则打开 `Create`，并关闭 `Search`。
- 点击 `Create` 按钮，若当前已打开，则关闭 `Create`。
- 点击外部区域时无条件关闭 `Create`。
- 按下 `Esc` 时无条件关闭 `Create`。

### 互斥规则

- `Search` 打开时再点击 `Create`，必须先关闭 `Search`，再打开 `Create`。
- `Create` 打开时再点击 `Search`，必须先关闭 `Create`，再打开 `Search`。

## 实施步骤

1. 在 `AssetsSidebar` 中暴露顶部工具区内容矩形的 anchor 数据，而不是只暴露 `search-button` 的 anchor。
2. 在 `Sidebar` 与 `AppWindow` 中透传并消费新的 toolbar anchor，形成统一的 overlay 布局输入。
3. 重做 Search overlay 的几何规则，使其左边界、宽度和纵向偏移都由工具区内容矩形驱动。
4. 调整 Search 输入框的度量规则，显式定义单行输入的高度、上下留白、文字区与 caret 区。
5. 将 `Create` 从 `PopupWindow` 收敛为根层 overlay，并接入统一 dismiss 逻辑。
6. 提取 `Assets` 工具区专用的统一 `menu row` 组件或等价度量规则，替换当前 `Create` 菜单项实现。
7. 在 Rust 侧保持现有 `ShellViewModel` 的业务状态源，只对 UI 层的开关顺序和互斥行为做同步收敛。
8. 增补 UI contract smoke 与状态 smoke，验证互斥、outside-click、`Esc`、对齐与锚点行为。

## 风险与回滚

### 风险

- `Slint` 根层 overlay 的坐标参考若仍绑定到错误锚点，Search 可能继续出现偏移。
- 输入框即使在视觉上收敛，Windows 下的 caret 实际渲染仍可能存在后端差异，需要实测确认。
- `Create` 从 `PopupWindow` 改为根层 overlay 后，如果 hit-testing 或层级顺序处理不当，可能影响空白点击关闭行为。
- `menu row` 抽象如果过度泛化，可能超出本轮 bugfix2 的合理范围。

### 回滚策略

- 若 Search 锚点方案不稳定，优先回滚到“仍为根层 overlay，但仅缩小到已验证的几何输入”，避免直接退回旧的内联展开行。
- 若 `Create` 根层 overlay 命中行为异常，可单独回退 `Create` 的宿主实现，不影响 Search 新方案。
- 若统一 `menu row` 抽象带来范围膨胀，可保留度量规则，缩回到 `Assets` 工具区私有组件，不扩展为全局通用组件。

## 验证清单

- Search 始终贴在 `AssetsSidebar` 顶部工具区正下方，窗口尺寸变化后仍不漂移。
- Search 输入框的 caret 不再贴底，文字区与上下留白协调。
- 点击 `Search` 按钮时，重复点击只重置焦点，不出现抖动或二次展开。
- Search query 为空时，outside-click 会关闭 Search。
- Search query 非空时，outside-click 不会关闭 Search。
- `Esc` 能无条件关闭 Search。
- `Create` 菜单中的 icon 与 text 具备稳定一致的基线和行内对齐。
- 点击 `Create` 按钮可以开关菜单。
- 点击任意外部区域可以稳定关闭 `Create`。
- `Esc` 能无条件关闭 `Create`。
- `Search` 与 `Create` 始终互斥，不会同时打开。
- 现有标题栏菜单和 sidebar tooltip 不因本轮调整而产生明显回归。

## 后续

- 若确认进入实现阶段，下一步应基于本文档生成 `implementation-plan`，并将验证项映射为具体 smoke/test 契约。
