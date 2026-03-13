# Mica Term Titlebar / Right Panel Corner Design

日期: 2026-03-13  
执行者: Codex

## 背景

当前 shell 已经完成 `frameless + custom titlebar + docked side panes` 的第一轮壳层搭建，但顶部状态栏与右侧面板的几何 ownership 仍然混杂，直接导致了本轮视觉问题：

- 顶部状态栏虽然自身是圆角，但外面仍被更大的方形边框包住，形成明显的 `box-in-box` 观感
- 右侧面板当前作为 docked pane 挂在主工作区右边，却仍然使用完整圆角卡片语义，导致与主区接缝异常、边缘发怪
- 左侧 `Activity Bar / AssetsSidebar` 本身已经是方角，右侧却走圆角卡片，整窗几何语言不一致

用户已确认本轮目标不是做新的视觉探索，而是收敛现有壳层几何策略：

- `1A` 只让 `shell-frame` 拥有外轮廓圆角
- `2A` 顶部状态栏改为方形内层
- `3A` 右侧 `RightPanel` 改为方形 docked pane
- `4A` 采用单一外框 + 内部分隔线策略，避免重复描边

## 目标

- 统一窗口外轮廓、顶部栏、右侧面板三者的几何职责
- 消除顶部栏“圆角内层被外部方框包裹”的割裂感
- 将右侧面板从“独立卡片”语义收敛为“贴边 inspector pane”语义
- 保留 Windows 11 首发所需的 Fluent / Mica 质感，同时不把实现锁死在 Win32-only 样式技巧上
- 保持当前 `restored rounded / maximized flat / snapped flat` 的窗口级几何策略不变

## 边界

### 本文档覆盖

- `AppWindow`、`Titlebar`、`RightPanel` 的圆角与边框职责重分配
- 恢复态、最大化态、贴靠态下的壳层几何规则
- 顶部栏与右侧面板的视觉语义调整
- 风险、回滚策略与验证清单

### 本文档不覆盖

- `wezterm-term` / `termwiz` 终端接入
- SSH / SFTP 业务逻辑
- 新的导航结构、浮层交互模型或布局重排
- 新主题 token、大规模色板重做
- 实现级逐文件 diff 与提交策略

## 调研结论

### Git 历史

最近与本轮问题直接相关的提交链路如下：

- `fb7ab7b feat: implement top status bar shell chrome`
- `ae48182 fix: repair sidebar shell layout contracts`
- `280600d fix: restore titlebar rendering in shell frame`
- `c94d6a0 fix: harden frameless resize and titlebar drag`
- `2ef3979 fix: allow frameless window height to grow`
- `9266180 Stabilize Windows femtovg-wgpu mainline on DX12`

这说明当前问题出现在“shell chrome 已成型、窗口行为逐步稳定”之后，不是基础布局缺失，而是几何 ownership 与视觉层级没有收敛。

### 当前源码证据

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L84) 中 `shell-frame` 已负责整窗外轮廓：
  - `border-radius: root.use-flat-window-chrome ? 0px : 14px;`
  - `border-width: 1px;`
  - `border-color: ThemeTokens.shell-stroke;`
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint#L57) 中 `Titlebar` 又额外绘制了一层完整内框：
  - `border-radius: root.use-flat-window-chrome ? 0px : 12px;`
  - `border-width: 1px;`
  - `border-color: ThemeTokens.shell-stroke;`
- [ui/shell/right-panel.slint](/home/wwwroot/mica-term/ui/shell/right-panel.slint#L7) 当前是 docked pane，但仍然使用：
  - `border-radius: 14px;`
  - `border-width: 1px;`
  - `clip: true;`
- [ui/shell/sidebar.slint](/home/wwwroot/mica-term/ui/shell/sidebar.slint#L74) 与 [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint#L7) 本身已经是方角面板，说明当前只有右侧 pane 仍在使用“卡片式圆角”。
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L135)、[src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L84)、[src/app/window_state.rs](/home/wwwroot/mica-term/src/app/window_state.rs#L37) 已把窗口级几何定义为：
  - `Restored => Rounded`
  - `Maximized / Snapped => Flat`

### 外部参考

- Windows 11 官方几何指导明确把圆角优先定义在 top-level app window，而不是要求每个 docked pane 都使用自己的完整圆角。
- Windows 11 官方说明：最大化、贴靠等状态下，窗口默认不使用圆角。
- Microsoft 的 title bar customization 指导强调：自定义标题栏时应保留清晰的拖拽区、窗口控制区与整体几何一致性。
- Slint `Rectangle` 文档确认支持 `border-radius` 和 `clip`，但本项目当前实现仍需通过实际 renderer 验证子元素是否严格服从外轮廓裁剪，不能在设计上假定“父层圆角一定自动约束所有子层视觉”。

参考：

- https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/ui/apply-rounded-corners
- https://learn.microsoft.com/en-us/windows/apps/develop/title-bar?tabs=wasdk
- https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry
- https://docs.slint.dev/latest/docs/slint/reference/elements/rectangle

## 设计要点与方案对比

### 1. 外轮廓圆角 ownership

#### 方案 A：只让 `shell-frame` 拥有外轮廓圆角

优点：

- 外轮廓职责单一，最容易消除双边框
- 最符合 Windows 11 的 top-level window 几何语义
- 最大化 / 贴靠态已有 `use-flat-window-chrome` 逻辑可以直接复用

缺点：

- 内部区域会更偏 docked chrome，而不是独立卡片

#### 方案 B：窗口和子面板都保留自己的完整圆角

优点：

- 每个区域单独看更“柔和”

缺点：

- 容易继续出现 `box-in-box`
- 接缝与描边 ownership 永远不清晰

#### 方案 C：混合 ownership，仅保留局部圆角

优点：

- 可以局部兼顾柔和感

缺点：

- 规则复杂
- 更依赖 renderer 细节，风险高

最终选择：`方案 A`

### 2. 顶部状态栏几何语义

#### 方案 A：顶部状态栏改为方形内层，外轮廓圆角只属于窗口

优点：

- 最符合成熟桌面终端 / 工具窗口的标题栏习惯
- 可以直接消除“内层 rounded header 被更大矩形框包住”的观感
- 与当前 `restored rounded / maximized flat` 的窗口级语义天然一致

缺点：

- 相比全圆角标题栏，局部个性略弱

#### 方案 B：只保留标题栏顶部圆角，底部改方角

优点：

- 能保留一点柔和感

缺点：

- 仍然需要处理标题栏完整边框与外框的重叠问题
- 视觉 ownership 仍不够干净

#### 方案 C：继续保留标题栏完整圆角卡片

优点：

- 改动最少

缺点：

- 当前问题不会根治

最终选择：`方案 A`

### 3. 右侧 `RightPanel` 几何语义

#### 方案 A：右侧 pane 改为全方角 docked inspector

优点：

- 与主工作区接缝最自然
- 与左侧 `Activity Bar / AssetsSidebar` 保持一致语言
- 避免右侧面板左边缘圆角“咬入主区”的怪异观感
- 降低 `clip + border-radius + docked seam` 的实现复杂度

缺点：

- 视觉更克制、更硬朗

#### 方案 B：仅保留靠窗口外侧的圆角，接缝侧方角

优点：

- 能保留一部分柔和感

缺点：

- 角半径、边框拼接、裁剪验证都更复杂

#### 方案 C：改为浮层卡片

优点：

- 高级感更强

缺点：

- 已经改变交互模型，超出本轮范围

最终选择：`方案 A`

### 4. 边框策略

#### 方案 A：单一外框 + 内部分隔线

规则：

- `shell-frame` 保留整窗唯一完整外框
- `Titlebar` 不再绘制完整外框，只保留需要的局部分隔
- `RightPanel` 不再绘制完整外框，只保留与主区之间的分隔线
- 通过局部 divider、surface tint 和层次色差区分区域，而不是重复描边

优点：

- 最能根治双边框和灰边堆叠
- 在暗色主题下更干净
- 与 docked pane 的结构关系一致

缺点：

- 需要重新定义哪些地方使用 divider、哪些地方完全依赖底色分层

#### 方案 B：每个区域继续保留完整边框

优点：

- 组件独立性强

缺点：

- 当前截图已经证明视觉效果不理想

#### 方案 C：大幅取消边框，只靠底色区分

优点：

- 现代感强

缺点：

- 当前暗色主题下层次可能不够稳

最终选择：`方案 A`

## 最终决策

本轮确认方案为：`1A + 2A + 3A + 4A`

即：

1. 外轮廓圆角只由 `shell-frame` 持有，恢复态继续保留圆角，最大化/贴靠态继续走平角
2. `Titlebar` 改为方形内层，不再作为独立 rounded card 出现
3. `RightPanel` 改为方形 docked pane，语义与左侧 side panes 对齐
4. 采用单一外框策略，内部改为分隔线与 surface 差异，不再叠加完整边框

## 具体设计定义

### A. 窗口级几何规则

- `shell-frame` 仍然是整窗唯一的外轮廓容器
- `restored` 状态保留当前圆角窗口语义
- `maximized / snapped` 状态继续通过 `use-flat-window-chrome` 切换到平角
- `Titlebar` 与 `RightPanel` 的几何变化不得影响现有 `WindowPlacementKind -> WindowChromeMode` 映射

### B. 顶部状态栏规则

- 顶部状态栏在视觉上定义为“贴在窗口顶部的 flat chrome strip”，不是“嵌在窗口里的卡片”
- 标题栏不再拥有完整四边边框
- 标题栏需要保留与主区之间的横向分隔关系
- 标题栏中的按钮、分组、tooltip、menu 不在本轮改变交互模型
- 若方形标题栏会触碰窗口圆角区域，则实现阶段必须显式验证并处理顶部角裁剪问题

### C. 右侧面板规则

- 右侧面板定义为 docked inspector pane，不再表现为独立 floating card
- 面板本体改为方角，优先通过左侧 divider 与主区分界
- 面板内部的 `SegmentedControl`、内容区域是否保留自身小圆角，不属于本轮外轮廓问题，可维持现状
- 面板展开/收起逻辑与宽度契约不在本轮重做

### D. 视觉层级规则

- 第一层：窗口外轮廓，由 `shell-frame` 唯一负责
- 第二层：区域分层，靠 `command-tint / panel-tint / terminal-surface` 差异区分
- 第三层：局部分隔，靠 `1px` divider 或必要的单侧描边建立结构
- 禁止再出现“外框 + 内框 + 局部边框”三层同时描边的情况

## 建议实施步骤

### 阶段 1：统一几何 ownership

- 审查 `AppWindow / Titlebar / RightPanel` 的圆角、描边、裁剪责任
- 以 `shell-frame` 为唯一外轮廓真源，删掉内部不再需要的完整外框语义
- 保持 `use-flat-window-chrome` 只作用于窗口级 outer geometry，不向右侧 pane 引入额外状态分支

### 阶段 2：顶部状态栏收敛为 flat chrome

- 将 `Titlebar` 从 rounded card 语义改为 flat strip
- 重新定义标题栏与主区的分隔方式
- 检查软件渲染器与 `femtovg-wgpu` 渲染器下，标题栏背景是否在窗口圆角处产生溢出或锯齿

### 阶段 3：右侧 pane 收敛为 square docked pane

- 移除 `RightPanel` 的完整圆角卡片语义
- 将右侧 pane 与主区接缝改为单一 divider 逻辑
- 验证收起、展开、主题切换时边缘没有重复描边或闪烁

### 阶段 4：补齐验证

- 扩充已有壳层 geometry / render contract，确保外轮廓、标题栏、右侧 pane 三者职责清晰
- 覆盖 restored / maximized / snapped 三类窗口状态
- 覆盖 light / dark 两种主题与当前主线 renderer 组合

## 风险与回滚

### 主要风险

- 如果 `shell-frame` 对子元素的裁剪表现与预期不一致，方形标题栏可能在顶部圆角区域产生溢色
- `femtovg-wgpu` 与软件渲染器在边界像素处理上可能存在微小差异，需要避免只在单一 renderer 上成立
- 右侧 pane 从圆角卡片切到 docked flat pane 后，若 divider 太弱，可能出现层级不够清晰的问题

### 风险缓解

- 实现阶段优先验证“恢复态顶部圆角区域是否干净”，再继续清理局部分隔线
- 不把本轮任务扩展为新的 hover、shadow、animation 重构，控制变量
- 保留现有颜色 token，只收敛几何与描边 ownership，避免把问题扩大为全面视觉改版

### 回滚策略

- 若 flat titlebar 在恢复态出现稳定的圆角裁剪问题，可临时回退为“标题栏本体方角，但通过上层 clip/mask 容器约束到窗口外轮廓”的中间态，而不是退回完整 rounded card
- 若右侧 pane 改方角后层次过弱，可先只补单侧 divider 或局部背景差，而不是恢复完整圆角卡片
- 若 renderer 差异导致单一方案不稳定，优先回滚局部分隔实现，不回滚整体 ownership 设计

## 验证清单

- `shell-frame` 仍然是唯一完整外框来源
- `Titlebar` 不再表现为独立 rounded card
- 顶部状态栏不再出现被更大矩形框包裹的观感
- `RightPanel` 改为方角 docked pane，与主区接缝自然
- 左右侧 pane 的几何语言一致
- `restored` 状态下窗口外轮廓仍然保持圆角
- `maximized / snapped` 状态下窗口继续保持平角
- light / dark 模式切换后不出现重复描边、发灰边或边缘闪烁
- 软件渲染器与当前主线 renderer 下，顶部圆角区域与右侧接缝区域都无明显异常

## 相关文件

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint)
- [ui/shell/titlebar.slint](/home/wwwroot/mica-term/ui/shell/titlebar.slint)
- [ui/shell/right-panel.slint](/home/wwwroot/mica-term/ui/shell/right-panel.slint)
- [ui/shell/sidebar.slint](/home/wwwroot/mica-term/ui/shell/sidebar.slint)
- [ui/shell/assets-sidebar.slint](/home/wwwroot/mica-term/ui/shell/assets-sidebar.slint)
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs)
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)
- [src/app/window_state.rs](/home/wwwroot/mica-term/src/app/window_state.rs)
