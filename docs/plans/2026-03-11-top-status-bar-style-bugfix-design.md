# Mica Term Top Status Bar Style Bugfix Design

日期: 2026-03-11  
执行者: Codex  
状态: 已完成方案确认

## 背景

当前仓库已在 `fb7ab7b feat: implement top status bar shell chrome` 中引入第一版顶部状态栏实现，并在 `84d8a7b` 合并到 `master`。现有实现已经具备以下基础能力：

- `AppWindow` 维持 `no-frame: true` 的自绘标题栏路线
- `WindowController` 已具备 `minimize / maximize-toggle / close / drag` 能力
- `TitlebarMenu` 已采用 `PopupWindow`
- 顶栏已经具备品牌区、拖拽区、动作区、窗口控制区的初步分段

但当前 UI 仍存在明显产品级缺陷：

- 右侧 `SSH / R+ / S` 区域过窄，未悬浮时视觉粘连，读感近似 `SSR+`
- 左侧 `M` 与右侧 `S` 都触发同一个设置菜单，语义重复
- 菜单锚点错误，导致弹层从最右侧弹出，而不是从左侧 `M` 下方展开
- 最小化按钮虽然底层能力已接通，但纯文本 `-` 表现过弱，用户感知为“缺少”
- 顶栏全部采用单字母或符号按钮，缺乏专业桌面工具应有的图标语义
- 所有图标按钮缺少 hover 提示，功能可发现性较差

结合竞品截图观察，本轮问题的本质不是单纯视觉美化，而是 `titlebar` 的信息架构、按钮语义、图标系统与弹层锚定策略需要一起校正。

## 目标

- 修复顶部状态栏的堆叠与错误分组问题
- 让左侧 `M` 成为唯一全局菜单入口
- 让菜单从 `M` 下方展开，而不是从右侧错误锚点展开
- 保持右侧工具簇与最右系统窗口控制的清晰边界
- 为窗口控制按钮引入符合 Windows 11 Fluent 语义的图标
- 为所有图标按钮补齐 tooltip 机制
- 保持当前 `Rust + Slint + Tokio` 和 `frameless shell` 路线不变
- 为后续扩展更多右侧工具入口保留稳定结构

## 边界

### 本文档覆盖

- 顶部状态栏的信息架构重排
- `M` 菜单与右侧工具簇的职责边界
- `PopupWindow` 锚定策略
- 自绘窗口控制按钮的图标与状态语言
- tooltip 呈现策略
- Fluent SVG 资产来源与使用策略
- 高层实施步骤、风险与回滚策略、验证清单

### 本文档不覆盖

- SSH / SFTP 会话逻辑
- 终端渲染区接入和 wezterm-term 集成
- 设置页或右侧功能面板的完整产品设计
- 详细到逐文件 diff 的 implementation plan
- 本轮之外的新功能定义

## 当前现状与根因判断

### 1. 分组拥挤导致视觉堆叠

当前 `ui/shell/titlebar.slint` 中的 `actions-zone` 固定为 `120px`，同时承载：

- `SSH` 状态 pill
- `R+` 右侧面板切换
- `S` 设置入口

这会导致文本 glyph 之间的视觉边界不清晰，hover 前几乎只能靠字形间距辨认。

### 2. 菜单入口语义重复

左侧 `M` 与右侧 `S` 都绑定到 `toggle-settings-menu-requested`，说明当前并未区分：

- `global app menu`
- `session/context settings`

这会让用户难以形成稳定心智模型。

### 3. 弹层锚点选错

当前 `settings-menu` 的 `x/y` 由 `actions-zone.absolute-position` 计算，因此菜单天然从右侧区域弹出。这与“左侧 M 是主菜单入口”的产品语义直接冲突。

### 4. 窗口控制按钮虽有能力，但没有正确表达

`WindowController` 已经支持最小化、最大化/还原和关闭，因此这不是底层命令缺失，而是窗口控制按钮的视觉表达过于原始：

- `-`
- `+`
- `R`
- `X`

这种纯文本 glyph 不符合 Fluent 标题栏质感，也会在高 DPI 下削弱可发现性。

### 5. 缺少统一的可发现性反馈

顶栏按钮目前只有 hover 背景，没有 tooltip 文案，因此新用户难以快速理解 `M / R+ / S / + / X` 的语义。

## 设计要点与方案对比

### 1. 顶栏整体分组

#### 方案 A: 保留五区架构，仅修补当前分区内容

结构:

- 左侧品牌区
- 中间拖拽区
- 右侧动作区
- 拖拽安全区
- 窗口控制区

优点:

- 改动最小
- 可复用现有布局骨架
- 风险较低

缺点:

- 容易停留在“修补版”
- 产品层级感提升有限
- 无法充分吸收竞品的分组优点

#### 方案 B: 重构为更接近竞品的清晰层级

结构:

- 左侧 `brand + global menu`
- 中间 `workspace / context`
- 右侧 `status + utility icons`
- 最右 `caption controls`

优点:

- 分组语义更清楚
- 更符合成熟桌面终端工具的观察路径
- 对未来增加更多 utility icons 更友好

缺点:

- 需要调整当前顶栏区内布局关系

最终选择: `B. 重构为更接近竞品的清晰层级`

选择原因:

- 当前问题的核心是信息架构失真，而不是单一控件样式错误
- 该方案能同时解决堆叠感、重复入口和工具区扩展性

### 2. 左侧 `M` 与右侧设置入口职责

#### 方案 A: 左侧 `M` 成为唯一全局菜单

结构:

- 左侧 `M` 负责 `global app menu`
- 右侧删除重复的 `S`
- 右侧只保留真正独立的工具入口，例如 `toggle-right-panel`

优点:

- 语义清晰
- 与用户预期完全一致
- 交互学习成本最低

缺点:

- 右侧少一个现成入口，需要重新定义右侧工具簇的最小集合

#### 方案 B: 保留双入口，但严格区分语义

结构:

- 左侧 `M` = `global app menu`
- 右侧 `S` = `session/context settings`

优点:

- 功能密度高
- 更利于未来功能扩展

缺点:

- 当前产品阶段不需要这么复杂
- 用户仍可能把两个入口视为重复

#### 方案 C: 左侧 `M` 退化为品牌动作，右侧 `S` 保留设置入口

优点:

- 改动最小

缺点:

- 与“菜单应在 M 下展开”的确认要求冲突

最终选择: `A. 左侧 M 成为唯一全局菜单`

选择原因:

- 先把顶栏语义变清楚，再考虑增加更多入口
- 这能直接消除当前最明显的重复功能问题

### 3. 菜单弹出方式与锚点

#### 方案 A: 继续使用 `PopupWindow`，但改为锚定左侧 `M`

结构:

- 保留 `TitlebarMenu inherits PopupWindow`
- 由左侧 menu button 的坐标决定弹层位置
- 菜单状态从“settings menu”提升为“global menu”

优点:

- 与现有 Slint 架构一致
- 最少改动即可修正错误锚点
- 继续复用点击外部关闭逻辑

缺点:

- 需要同步修正状态命名与回调语义

#### 方案 B: 改成窗口内 overlay/dropdown layer

结构:

- 不使用 `PopupWindow`
- 在 `AppWindow` 内直接绘制 overlay layer

优点:

- 动效、阴影和层级控制更自由

缺点:

- 侵入窗口层级更多
- 对当前问题来说过重

最终选择: `A. PopupWindow 锚定左侧 M`

选择原因:

- 问题在于锚点和语义，不在于控件类型本身
- `PopupWindow` 足以承载主菜单与 tooltip

### 4. 窗口控制按钮表现方式

#### 方案 A: 继续完全自绘 caption controls，但改成 Fluent 图标

结构:

- `Minimize`
- `Maximize / Restore`
- `Close`

全部由 Slint 绘制，点击后继续走 Rust `WindowController`。

优点:

- 与 `no-frame: true` 路线一致
- 可完全纳入 Fluent/Mica 视觉语言
- 对跨平台抽象最友好

缺点:

- 所有 hover / pressed / inactive / danger 状态都需要自己规范

#### 方案 B: 局部回退到系统 caption buttons

优点:

- 某些系统行为天然正确

缺点:

- 与 frameless shell 冲突
- 视觉割裂
- 跨平台抽象变差

最终选择: `A. 完全自绘 caption controls + Fluent 图标`

选择原因:

- 当前底层命令已经接通，只需提升表现层质量
- 这是最符合产品视觉方向的方案

### 5. Tooltip 策略

#### 方案 A: 统一 `TooltipPopup`

结构:

- 所有顶栏 icon button 统一走一个 tooltip 组件
- hover 延迟后显示
- pointer leave / click / focus loss 时关闭

优点:

- 符合桌面工具预期
- 功能可发现性最好
- 易于后续在其他 toolbar 中复用

缺点:

- 需要额外处理 hover 延迟与闪烁抑制

#### 方案 B: 固定说明区

结构:

- hover 时把文案显示在标题栏固定位置

优点:

- 更稳定
- 没有弹层抖动

缺点:

- 不符合用户对 icon toolbar 的直觉
- 会占用顶栏宝贵空间

最终选择: `A. 统一 TooltipPopup`

选择原因:

- 这是最直接、最符合桌面端经验的方案

### 6. 图标系统选型

#### 方案 A: 本地 vendoring `microsoft/fluentui-system-icons` 的 SVG 资产

来源:

- `https://github.com/microsoft/fluentui-system-icons`

策略:

- 以该仓库作为 `source of truth`
- 只挑选少量需要的 SVG 资产 vendoring 到项目本地
- 默认使用 `Regular`
- 激活态少量使用 `Filled`

优点:

- 视觉语言最接近 Windows 11 Fluent
- 不引入额外运行时依赖
- 与当前 Slint 自绘路线最契合

缺点:

- 需要手工维护少量本地资产

#### 方案 B: 引入 `lucide-slint`

优点:

- 集成方便
- 图标数量丰富
- 与 Slint 1.15 兼容良好

缺点:

- 视觉语气更通用，不够 Fluent 原生

#### 方案 C: 手写 Path 图标

优点:

- 控制力最高

缺点:

- 维护成本过高
- 没有必要

最终选择: `A. 本地 vendoring Fluent SVG 资产`

选择原因:

- 首发平台是 Windows 11
- 顶栏属于品牌和系统感最强的位置，图标语言应优先贴近 Fluent

## 最终决策

本轮正式确认以下组合：

- `1B`: 顶栏整体分组改为更接近竞品的清晰层级
- `2A`: 左侧 `M` 成为唯一全局菜单
- `3A`: 继续使用 `PopupWindow`，但锚定左侧 `M`
- `4A`: 窗口控制继续完全自绘，但换成 Fluent 图标
- `5A`: 顶栏所有图标按钮接入统一 tooltip
- `6A`: 图标来源采用 `microsoft/fluentui-system-icons`

补充确认的图标策略：

- 默认: `Regular`
- 激活态: 少量使用 `Filled`
- 不采用: 全部按钮都使用 `Filled`

## 目标态结构

### 顶栏分组

- 左侧: `brand chip + app name + M(global menu)`
- 中间: `workspace/context` 文案与主要拖拽区
- 右侧: `status pill + utility icons`
- 最右: `minimize / maximize-restore / close`

### 右侧工具簇最小集合

本轮建议最小集合如下：

- 连接或模式状态 pill，例如 `SSH`
- `toggle-right-panel`

本轮不保留右侧重复的 `S` 设置按钮。

### 菜单状态模型

当前 `show_settings_menu` 在语义上已不准确。设计上建议后续改为更中性的主菜单状态命名，例如：

- `show_global_menu`
- `toggle_global_menu_requested`
- `close_global_menu_requested`

该调整的目的不是增加功能，而是消除“主菜单”和“设置菜单”混用带来的长期语义漂移。

## 实施步骤

1. 调整顶栏信息架构
   - 保留当前五区大结构
   - 重新划分左侧主菜单、右侧工具簇和窗口控制区职责

2. 去除重复入口
   - 让左侧 `M` 成为唯一 `global menu`
   - 删除右侧重复的 `S`

3. 修正弹层锚点与状态语义
   - 让 `PopupWindow` 跟随左侧 `M`
   - 把相关状态和 callback 名称转为主菜单语义

4. 替换纯文本 glyph
   - 顶栏功能按钮改为图标按钮
   - caption controls 改为 Fluent 图标表达

5. 接入 tooltip
   - 为所有 icon button 定义 tooltip 文案
   - 统一 hover delay、关闭条件与层级

6. 引入本地 Fluent SVG 资产
   - 从 `microsoft/fluentui-system-icons` 选取最小必需图标集
   - 明确 `Regular` 与 `Filled` 的使用边界

7. 完成视觉回归与交互验证
   - 验证窄窗口下仍无堆叠
   - 验证菜单锚点、tooltip 和 caption controls 状态

## 风险与回滚

### 主要风险

- `PopupWindow` 锚点修改后，如未同步修正状态命名，仍可能在后续扩展中再次发生语义混乱
- 顶栏切换为图标后，如果 icon size、stroke weight 和 padding 控制不当，可能出现“更精致但更难点”的反效果
- `Regular` / `Filled` 混用如果没有明确规则，可能导致 active 态过度跳跃
- tooltip 如果没有 hover delay，可能在密集图标区造成频繁闪烁

### 回滚策略

- 保留现有 `WindowController` 与 `PopupWindow` 技术路线，不回退系统标题栏
- 若图标替换效果不理想，可先回退到统一 `Regular`，不回退到文本 glyph
- 若右侧工具簇在首轮重排后仍拥挤，可先减少 utility icons 数量，而不是再次塞回重复入口

## 验证清单

- [ ] 顶栏在默认窗口宽度下不再出现 `SSH / R+ / S` 视觉粘连
- [ ] 左侧 `M` 成为唯一主菜单入口
- [ ] 菜单从 `M` 下方展开，不再从右侧区域弹出
- [ ] 右侧不再存在与 `M` 重复的设置入口
- [ ] 最小化按钮具有清晰图标表达与 hover/pressed 反馈
- [ ] `maximize / restore / close` 的图标语义明确
- [ ] 所有顶栏图标在 hover 时都有 tooltip
- [ ] 默认图标风格为 `Regular`
- [ ] 激活态仅对少量需要强调的按钮使用 `Filled`
- [ ] 整体观感接近 Windows 11 Fluent，而非通用 web toolbar

## 参考依据

- Git commit: `fb7ab7b feat: implement top status bar shell chrome`
- Git commit: `84d8a7b Merge branch 'feature/top-status-bar'`
- Slint `PopupWindow` 参考: `https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/`
- Slint 标准按钮 icon 能力: `https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/button/`
- Fluent 图标来源: `https://github.com/microsoft/fluentui-system-icons`
- 竞品视觉参考: `dist/images/Snipaste_2026-03-11_09-11-57.png`
