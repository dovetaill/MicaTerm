# Windows Frameless Resize Drag Design

日期: 2026-03-12
执行者: Codex
状态: 方案已确认，待决定是否继续补 implementation plan

## 背景

当前仓库已经完成 `frameless + transparent + self-drawn shell` 的窗口壳层，并在最近两轮提交中先后补了：

- `04d1ef3 fix: restore shell body height on resize`
- `a151351 Implement Windows drag restore and snap recovery`

但针对 Windows 11 首发场景，窗口壳层仍存在三类几何交互问题：

1. 窗口可以继续被缩小到明显失真的尺寸，主布局出现异常压缩。
2. 边框 resize 目前只稳定表现为左右方向；上下方向与右下角整体拖动不可靠。
3. 在双击顶栏或点击最大化按钮后，顶部状态栏拖拽偶发失效，但窗口控制按钮仍然正常。

结合当前源码，问题核心仍然不是 terminal renderer，而是 `window shell / frame geometry contract`：

- 主区仍是 `WelcomeView` 占位，不是真实 terminal host。
- `ShellMetrics` 已定义最小宽高预算，但当前没有成为原生窗口硬约束。
- `AppWindow` 已启用 `resize-border-width: 6px`，但当前交互表现说明仅依赖默认 frameless resize 热区还不够稳。
- `Titlebar.drag-zone` 当前是在 `TouchArea.moved` 且 `pressed` 时才调用 `drag_window()`，与 `winit` 对拖拽启动时机的契约并不完全一致。

## 目标

- 为窗口定义稳定的最小尺寸，避免 shell 布局在极小尺寸下失真。
- 修复 Windows frameless 窗口的上下边、角落以及右下角 resize 体验。
- 修复最大化切换后顶栏拖拽偶发失效的问题。
- 保持当前 `no-frame: true`、`Mica`、自绘标题栏路线，不回退到系统标题栏。
- 保持跨平台边界清晰，允许 Windows 侧做适度专用适配，但不把全部窗口行为沉到底层 Win32。

## 边界

### 本文档覆盖

- `AppWindow` 的窗口最小尺寸契约
- frameless resize 交互策略
- Windows frame adapter 的职责边界
- 顶栏拖拽启动策略
- 验证矩阵、风险与回滚

### 本文档不覆盖

- `wezterm-term` / `termwiz` 接入
- `russh` / SFTP 功能
- Welcome / Sidebar / TabBar 的视觉重做
- 持久化“上次窗口位置与尺寸”产品策略
- 逐任务 TDD 级 implementation plan

## 当前实现观察

### 1. 最小尺寸常量已经存在，但未形成单一真源

当前 `ShellMetrics` 已定义：

- `WINDOW_MIN_HEIGHT = 640`
- `WINDOW_MIN_WIDTH = ACTIVITY_BAR_WIDTH + MAIN_WORKSPACE_MIN_WIDTH = 688`

但实际运行路径里：

- 它们只在布局决策里被消费，用于避免响应式折叠逻辑继续向下推宽度。
- `AppWindow` 本身没有声明 `min-width` / `min-height`。
- 也没有显式看到运行时把该预算下发成宿主窗口层的强约束。

这意味着当前“最小尺寸”只是一组 layout 常量，还不是 native window contract。

### 2. Slint 1.15 / winit 0.30 已具备可复用的 resize 与 min-size 能力

本仓库当前依赖：

- `slint 1.15.1`
- `winit 0.30.13`

本地依赖源码显示：

- Slint `Window` 的 `min-width / min-height` 会经由 winit backend 下发到 `set_min_inner_size(...)`
- `resize-border-width` 会在无边框窗口上计算八方向热区
- 鼠标按下后会调用 `drag_resize_window(ResizeDirection)`

因此，本轮不需要重新发明一套跨平台 resize 基础设施，但需要把它接入得更可控。

### 3. 当前 Windows frame adapter 仍然很轻

`windows_frame.rs` 目前的 `WM_NCHITTEST` 处理主要只覆盖：

- maximize button 的命中区桥接
- `HTMAXBUTTON`

它还没有成为完整的 non-client resize / caption bridge。当前策略更像“补一点 Windows 特化能力”，而不是完全接管 frame hit-test。

### 4. 顶栏拖拽链路存在时机不稳的问题

当前顶栏空白拖拽区是：

- Slint `TouchArea`
- 在 `moved` 且 `pressed` 时触发 `drag-requested`
- Rust 侧再调用 `window.drag_window()`

这条链路的弱点是：

- 用户已经发生了一次 move 之后才真正尝试进入系统拖拽
- 最大化 / 还原切换后，窗口状态、输入节奏和命中区切换都更敏感
- 容易演化成“按钮都正常，但拖拽偶发失效”的不稳定体验

## 设计要点与方案对比

### 1. 最小尺寸预算

#### 方案 1A：沿用现有内容预算 `688 x 640`

做法：

- 直接复用 `ShellMetrics::WINDOW_MIN_WIDTH`
- 直接复用 `ShellMetrics::WINDOW_MIN_HEIGHT`

优点：

- 与当前响应式设计一致
- 改动最小
- 为小窗口使用保留余地

缺点：

- 视觉安全边界偏紧
- 真实 terminal host 接入后可能还要继续上调

#### 方案 1B：上调到更宽的视觉安全预算

优点：

- 界面更稳，不容易拥挤

缺点：

- 与当前布局折叠策略不完全一致
- 会牺牲小窗使用场景

#### 方案 1C：用 full layout 预算作为最小尺寸

优点：

- 所有面板长期都能展开

缺点：

- 过于保守
- 不符合终端工具的桌面窗口使用习惯

**最终选择：1A**

### 2. 最小尺寸约束落点

#### 方案 2A：只在 Slint `Window` 上声明 `min-width / min-height`

优点：

- 声明式最干净
- 跨平台最好

缺点：

- 若 Rust 侧常量继续独立存在，后续容易漂移

#### 方案 2B：只在 Rust / winit 侧设置 native min size

优点：

- 原生窗口行为最直接

缺点：

- 会产生双真源问题
- `.slint` 与 Rust 常量可能脱节

#### 方案 2C：统一契约，`ShellMetrics` 为数值真源，Slint 声明约束，Rust 负责同步与校验

做法：

- `ShellMetrics` 保持唯一数值真源
- `AppWindow` 显式声明 `min-width` / `min-height`
- Rust bootstrap 保持对默认尺寸与实际窗口状态的同步校验

优点：

- 单一真源
- 复用官方能力
- 后续跨平台迁移边界清晰

缺点：

- 比纯声明式多一层同步约束

**最终选择：2C**

### 3. Frameless resize 输入策略

#### 方案 3A：继续依赖 `resize-border-width` 默认热区

做法：

- 保留当前机制
- 只调整 `resize-border-width` 数值

优点：

- 改动最小

缺点：

- 当前实际表现已经证明默认热区在本项目窗口壳层上不够稳
- 不足以解释和收敛上下、右下角 resize 的问题

#### 方案 3B：增加显式 edge grips，主动调用 `drag_resize_window`

做法：

- 保留 `resize-border-width` 作为后备能力
- 在窗口四边与四角定义清晰的 invisible resize interaction layer
- 将不同边与角映射成 `ResizeDirection::{North, South, East, West, NorthEast, ...}`
- 由运行时显式调用 `drag_resize_window`

优点：

- 交互更稳定
- 能对上下边与角落行为做明确建模
- 仍然复用官方 `winit` 能力，不需要重写整套原生 resize

缺点：

- 需要细致处理 titlebar、shell frame 与热区之间的关系

#### 方案 3C：Windows 下完整接管 `WM_NCHITTEST` 的 resize

优点：

- Windows 原生感最强

缺点：

- Win32 耦合过重
- 超出当前“中度适配层”边界

**最终选择：3B**

### 4. Windows frame adapter 的深度

#### 方案 4A：保持极轻 adapter

优点：

- 平台代码最少

缺点：

- 当前问题很难完全收敛
- 后续 Windows 壳层 bug 容易继续漏出

#### 方案 4B：中度 adapter，聚焦 placement / maximize / caption 语义同步

做法：

- 保留当前 placement 查询与 maximize button hit-test 方向
- 补充 frame adapter 与显式 resize layer 之间的职责边界
- 避免 adapter 误吞 edge resize 区域
- 在 Windows 下增加专用 contract / smoke 验证

优点：

- 能覆盖当前问题
- 平台特化仍然局限在 adapter 层

缺点：

- 不是完整 Win32 接管方案

#### 方案 4C：重型 adapter，完整接管 non-client caption / resize 行为

优点：

- Windows 11 原生程度最高

缺点：

- 实现成本与维护成本显著提高

**最终选择：4B**

### 5. 顶栏拖拽启动策略

#### 方案 5A：保留当前 `moved -> drag_window()` 链路

优点：

- 改动最小

缺点：

- 触发时机不稳定
- 与 `drag_window()` 所需的输入时机契约不够匹配
- 已被实际现象证明存在偶发失效

#### 方案 5B：改为“按下即启动拖拽”，双击最大化单独仲裁

做法：

- 顶栏空白区在按下阶段就进入 drag 启动逻辑
- 双击最大化保持单独判断
- 将“拖拽”和“双击还原/最大化”视为两条并行但有优先级的输入路径

优点：

- 与 `winit` 拖拽时机契约更一致
- 能显著降低“按钮正常但拖拽偶发失效”的概率
- 仍然保留跨平台边界

缺点：

- 需要处理单击按下、双击、轻微移动三者的仲裁顺序

#### 方案 5C：Windows 下把拖拽区映射成 `HTCAPTION`

优点：

- 更原生

缺点：

- 会把当前方案整体推向重型 Win32 adapter

**最终选择：5B**

## 最终决策

本轮确认采用的组合为：

- `1A` 最小尺寸预算沿用现有 `688 x 640`
- `2C` 统一最小尺寸真源
- `3B` 显式 edge grips + `drag_resize_window`
- `4B` 中度 Windows frame adapter
- `5B` 顶栏按下即启动拖拽，双击最大化单独仲裁

### 决策摘要

1. 最小尺寸不再只是布局常量，而要成为窗口层真实约束。
2. Frameless resize 不继续完全依赖隐式热区，而要引入显式、可建模的边与角交互层。
3. Windows adapter 保持中度专用，不升级为全量 Win32 frame 接管。
4. 顶栏拖拽要修正为更符合 `winit` 契约的启动时机，优先解决最大化切换后的偶发失效。

## 高层实施步骤

1. 固化最小尺寸契约
   - 将 `ShellMetrics` 继续作为唯一数值真源。
   - 在 `AppWindow` 上补足 `min-width` / `min-height`。
   - 保持 Rust bootstrap 对默认尺寸、恢复尺寸和 layout sync 的一致性。

2. 重构 resize interaction layer
   - 在窗口四边与四角定义明确的 invisible resize grips。
   - 为每个 grip 建立稳定的方向映射。
   - 让 grip 成为 resize 主入口，`resize-border-width` 作为保底后备。

3. 收敛 Windows frame adapter 职责
   - 继续由 adapter 负责 maximize / placement 相关语义桥接。
   - 明确 adapter 不抢夺 edge grips 的交互区域。
   - 保证 `maximize hover`、状态同步和壳层圆角切换与 resize layer 不相互污染。

4. 调整顶栏 drag 启动链路
   - 将 drag 启动时机前移到按下阶段。
   - 为双击最大化保留独立输入判定。
   - 避免最大化切换后 titlebar drag zone 因输入时序而失效。

5. 补齐验证与回归守卫
   - 新增最小尺寸 contract 测试。
   - 新增 resize direction contract / smoke。
   - 新增最大化切换后 drag 可用性的交互契约验证。

## 风险与回滚

### 主要风险

- 显式 edge grips 可能与现有 titlebar、tooltip overlay、window controls 的命中区发生重叠。
- 顶栏按下即启动拖拽后，若双击判定不当，可能影响最大化体验。
- Windows frame adapter 与显式 resize layer 若职责划分不清，可能出现新的输入竞争。

### 风险缓解

- 先把 resize grips 与 titlebar drag zone 的几何边界文档化，再进入实现。
- 优先通过 contract test 验证不同区域只映射到单一交互语义。
- Windows 专用行为统一落在 adapter / bootstrap 边界内，避免污染跨平台 UI 结构。

### 回滚策略

- 若显式 resize grips 引入新回归，可先保留 `min-size` 修复与顶栏 drag 时机修复，暂时回退 grips 层。
- 若顶栏按下即拖拽影响双击最大化，可先切回当前双击路径，只保留最小尺寸与 resize contract 修复。
- 若 Windows adapter 与交互层耦合过深，可暂时收缩 adapter 责任，保留 `HTMAXBUTTON` 与 placement sync。

## 验证清单

### 自动化验证

- `ShellMetrics` 的最小宽高是否与 `AppWindow` 的 min-size 声明一致。
- 最小尺寸是否真正阻止窗口继续缩小。
- 上、下、左、右、四角是否都存在明确的 resize direction 映射。
- 最大化后恢复、双击顶栏后恢复、点击最大化后恢复三条链路下，drag zone 是否仍然有效。
- Windows adapter 的 maximize button hit-test 是否仍保持正确。

### 手工验证

- 默认 restored 状态下：
  - 从四边缩放
  - 从四角缩放
  - 缩小到最小值后继续拖动不会再压缩
- 最大化后：
  - 双击顶栏恢复
  - 点击最大化按钮恢复
  - 恢复后立即拖动顶栏，窗口应稳定进入 drag
- Snap / unsnap 后：
  - 壳层圆角和方角切换符合 Windows 11 预期
  - 顶栏按钮与拖拽区都保持正常

## 相关文件

- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/app/windows_frame.rs`
- `src/shell/metrics.rs`
- `tests/window_geometry_spec.rs`
- `tests/window_shell.rs`
- `tests/windows_frame_contract_smoke.sh`
- 后续新增的 resize / drag contract tests

## 结论

这次问题的本质不是单个交互控件坏掉，而是窗口壳层缺少稳定的“尺寸真源 + resize 主入口 + 拖拽时机契约”。采用 `1A + 2C + 3B + 4B + 5B` 之后，项目可以在不回退系统标题栏、不全面沉到 Win32 的前提下，把最小尺寸、frameless resize 和最大化后的 titlebar drag 稳定性一次性收敛到可验证的架构边界内。
