# Mica Term Top Status Bar Style Bugfix3 Design

日期: 2026-03-11  
执行者: Codex  
状态: 已确认方案，未进入实现

## 背景

当前仓库的顶部状态栏相关实现主要来自以下提交：

- `fb7ab7b feat: implement top status bar shell chrome`
- `b832f42 feat: polish top status bar style`
- `5c5b95e feat: implement top status bar style bugfix2`

本轮问题聚焦在一个明确缺陷：

- 顶部状态栏图标按钮在鼠标悬停后，没有稳定出现位于图标底部的小型文字提示框

现有仓库并不是“没有 tooltip 实现”，而是已经具备一套 `shared tooltip` 原型：

- `TitlebarIconButton` / `WindowControlButton` 在 hover 时会发出 `tooltip-open-requested(...)`
- `Titlebar` 内部有统一的 `tooltip-delay := Timer`
- `TitlebarTooltip` 目前实现为 `PopupWindow`
- 所有顶栏按钮都已经定义了 `tooltip-text`

相关文件：

- `ui/shell/titlebar.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/titlebar-tooltip.slint`
- `src/app/bootstrap.rs`
- `verification.md`

## 现状判断

基于源码和 Git 历史，当前问题更可能出在“运行时呈现链路不稳定”，而不是“按钮没有发送 hover 事件”。

现有实现的关键链路如下：

1. 按钮 `TouchArea.has-hover` 变化
2. 按钮发出 `tooltip-open-requested(text, anchor-x, anchor-y)`
3. `Titlebar.schedule-tooltip(...)` 写入共享状态并重启 `Timer`
4. `Timer.triggered` 后调用 `tooltip-popup.show()`
5. `TitlebarTooltip` 作为 `PopupWindow` 显示

这说明源码契约已经存在，但仓库当前验证仍主要停留在静态层面：

- `tests/top_status_bar_ui_contract_smoke.sh` 只验证源码结构存在
- `verification.md` 明确记录本环境没有执行 GUI smoke

因此，本轮 `bugfix3` 的目标不是补充 tooltip 文案，而是让 tooltip 在真实窗口里稳定出现，并在失败时能留下可追踪证据。

## 目标

- 让所有顶栏图标按钮在 hover 延时后稳定显示 tooltip
- tooltip 位置保持在对应图标下方或近下方区域
- 多个图标之间快速切换 hover 时，tooltip 不明显闪烁、不残留
- 点击按钮、离开按钮、打开菜单时，tooltip 立即关闭
- 增加最小可用的文件日志，输出到程序当前目录 `logs/`
- 不改变现有顶栏整体信息架构，不扩散到 SSH/SFTP、Terminal 或其他业务模块

## 边界

### 本文档覆盖

- 顶栏 tooltip 的呈现载体
- hover 触发与延时调度策略
- tooltip 运行时日志与可观测性
- 高层实施步骤
- 风险、回滚与验证清单

### 本文档不覆盖

- 顶栏整体布局再次重构
- `global-menu` 行为重做
- 新的持久化配置项设计
- 更大范围的日志系统改造
- 逐文件命令级 implementation plan

## 设计要点与方案对比

### 1. Tooltip 呈现载体

#### 方案 A1：继续使用 `PopupWindow`

做法：

- 保留 `TitlebarTooltip inherits PopupWindow`
- 继续依赖 `show()` / `close()`
- 继续使用传入的 anchor 坐标定位

优点：

- 改动最小
- 与当前 `global-menu` 技术路径一致
- 复用现有 `tooltip-popup` 结构

缺点：

- 继续依赖当前已经可疑的 popup 呈现链路
- 对 frameless/transparent/software renderer 组合的稳定性更敏感
- 调试时较难快速判断是 hover 问题、坐标问题还是 popup 层级问题

#### 方案 A2：改为 `Titlebar/AppWindow` 内部 overlay

做法：

- tooltip 不再作为独立 `PopupWindow`
- 将 tooltip 作为顶栏或窗口内部的 overlay element
- 坐标统一落在当前窗口内容坐标系中

优点：

- 坐标系、层级和可见性都在同一窗口内，更可控
- 更容易稳定覆盖 Windows 11 首发场景
- 调试与日志记录更直接
- 为后续跨平台迁移保留更稳定的 UI 语义

缺点：

- 需要自己管理 overlay 的显隐、位置和边界避让
- 比 `A1` 略高一些的实现改动

**最终决策：选择 `A2`**

决策原因：

- 当前问题核心是“运行时能否稳定显示”，不是“组件是否存在”
- 在当前证据不足的情况下，优先选择更可控的同窗体 overlay 路线

### 2. Hover 触发与延时调度

#### 方案 B1：保留“按钮发 intent，Titlebar 统一调度”

做法：

- `TitlebarIconButton` / `WindowControlButton` 继续只负责发出 tooltip intent
- `Titlebar` 维护唯一 tooltip 状态机
- 状态机至少包含：
  - `idle`
  - `pending`
  - `visible`
- 每次 hover 变更时统一做 delay restart、close、切换 source

优点：

- 最大程度复用现有结构
- 所有按钮仍然是轻量组件
- tooltip 行为集中，便于统一调优

缺点：

- 需要明确 source 切换时的消抖策略
- 需要避免相邻按钮快速切换时出现闪烁

#### 方案 B2：改为 root 级 hover router

做法：

- 由 `Titlebar` 根层统一判断鼠标命中的按钮区域
- 按钮本身不再发送 tooltip request

优点：

- 所有 hover 逻辑在一个位置
- 理论上更适合未来做复杂命中区策略

缺点：

- 实现复杂度明显更高
- 与当前按钮封装方向相反
- 对本轮 bugfix 来说属于过度设计

**最终决策：选择 `B1`**

决策原因：

- 当前需要的是修复现有链路，而不是重写 hover 事件体系
- `B1` 既能保留当前架构分层，也足够支撑稳定 tooltip

### 3. 日志与可观测性

#### 方案 C1：只输出控制台日志

做法：

- 使用 `eprintln!` 或等价方式输出 hover/tooltip 生命周期日志

优点：

- 最快
- 对代码侵入最小

缺点：

- GUI 环境下不一定有稳定控制台
- 不满足“程序目录 `logs/` 下可回看”的排障诉求

#### 方案 C2：增加轻量文件日志到 `logs/`

做法：

- 在运行目录创建 `logs/`
- 记录 tooltip 相关关键事件到独立日志文件
- 仅覆盖本轮排障相关字段

建议记录内容：

- `hover-enter`
- `hover-leave`
- `schedule-tooltip`
- `cancel-tooltip`
- `show-tooltip`
- `close-tooltip`
- `source-id`
- `text`
- `anchor-x / anchor-y`

优点：

- 最符合当前排障需求
- 便于 Windows 11 手测后直接回看证据
- 不需要立刻把整个项目升级为完整日志框架

缺点：

- 需要补充最小日志目录管理与写入策略
- 需要注意避免日志噪音过大

#### 方案 C3：开发态可视化 debug HUD

做法：

- 在顶栏临时显示 tooltip 状态、坐标和 source

优点：

- 现场观察最直观

缺点：

- 更偏开发态工具
- 不如文件日志适合作为交付证据

**最终决策：选择 `C2`**

决策原因：

- 用户已经明确要求在必要时输出到 `logs/`
- 文件日志最适合当前“真实桌面行为异常”的排障闭环

## 最终方案

本轮 `top status bar style bugfix3` 采用组合方案：

- `A2`：tooltip 从 `PopupWindow` 调整为 `Titlebar/AppWindow` 内部 overlay
- `B1`：保留按钮发 intent，`Titlebar` 统一 delay 调度与状态管理
- `C2`：增加 `logs/` 文件日志，记录 tooltip 生命周期与坐标

对应的架构原则如下：

1. tooltip 只保留一个共享实例，不为每个按钮单独创建弹层
2. 按钮组件只负责表达“当前 hover 的 tooltip 意图”
3. `Titlebar` 统一决定：
   - 是否进入 `pending`
   - 是否切换 source
   - 是否真正显示
   - 是否立即关闭
4. tooltip 显示在当前窗口坐标系内，避免额外 popup 窗口语义
5. 日志只覆盖 tooltip 路径，不扩散为全局日志改造

## 建议的运行时行为

### Hover 生命周期

1. 鼠标进入按钮
2. 按钮上报 `source-id + text + anchor`
3. `Titlebar` 记录当前请求，进入 `pending`
4. 延时结束后，如果当前 source 仍有效，则显示 tooltip
5. 鼠标离开按钮时立即关闭 tooltip，并记录 `hover-leave`

### 切换行为

当鼠标从一个按钮快速移动到另一个按钮时：

- 不保留旧 tooltip
- 不允许两个 tooltip 并存
- 新按钮重新触发 delay
- 如果希望减少闪烁，可允许 very-short handoff，但默认先以稳定为主，不做过度动画

### 关闭行为

以下情况 tooltip 必须立即关闭：

- 鼠标离开当前按钮
- 用户点击当前按钮
- 打开 `global-menu`
- 窗口失活
- 标题栏整体状态重建或隐藏

## 高层实施步骤

1. 将 `TitlebarTooltip` 从 `PopupWindow` 语义调整为窗口内部 overlay 语义
2. 在 `Titlebar` 中保留共享 tooltip 状态，但补全更明确的状态管理字段
3. 让按钮继续发出 tooltip intent，并补充可区分的 `source-id`
4. 将 tooltip 定位逻辑改到当前窗口内部坐标系
5. 增加最小文件日志能力，写入 `./logs/`
6. 更新源码契约 smoke 与验证文档，使其覆盖新 overlay 路线
7. 在 Windows 11 真机上做 hover / fast-switch / click-close / menu-open-close smoke

## 风险

### 风险 1：overlay 坐标偏移

说明：

- 从 `PopupWindow` 改为 overlay 后，坐标参考系会发生变化

缓解：

- 统一由 `Titlebar` 计算 anchor
- 首轮实现优先保证视觉正确，再做像素级微调

### 风险 2：快速切换 hover 时闪烁

说明：

- 相邻图标密集排列时，频繁 enter/leave 很容易造成 tooltip 抖动

缓解：

- 用统一状态机管理 `pending` 和 `visible`
- 先保证“旧 tooltip 不残留”，再调优手感

### 风险 3：日志过于噪音

说明：

- hover 事件频繁，文件日志可能快速膨胀

缓解：

- 只记录关键生命周期事件
- 不记录逐帧坐标
- 后续必要时增加 debug flag 或 rotate 策略

## 回滚策略

如果 overlay 路线在真实窗口中出现比当前更严重的问题，按以下顺序回滚：

1. 保留 `B1 + C2`
2. 将 tooltip 呈现层暂时退回 `PopupWindow`
3. 继续保留文件日志，优先查明 popup 呈现失败根因

即：

- 首先回滚呈现载体
- 不回滚统一调度结构
- 不回滚日志能力

这样可以避免再次失去排障证据。

## 验证清单

### 源码级验证

- [ ] tooltip 不再依赖独立 `PopupWindow`
- [ ] `Titlebar` 仍只维护一个共享 tooltip 实例
- [ ] 按钮 hover 仍走统一 intent 回调
- [ ] tooltip 状态包含清晰的 `pending / visible / close` 生命周期
- [ ] 日志目录与文件写入路径明确

### 行为级验证

- [ ] 所有顶栏图标 hover 后都能显示 tooltip
- [ ] tooltip 出现在对应图标底部或近下方区域
- [ ] 鼠标离开后 tooltip 立即消失
- [ ] 点击按钮前后 tooltip 不残留
- [ ] 打开 `global-menu` 时 tooltip 不遮挡交互
- [ ] 快速横向掠过多个图标时没有明显残影

### 排障级验证

- [ ] 程序当前目录自动生成 `logs/`
- [ ] tooltip 日志文件可读
- [ ] 日志能看出 hover enter、schedule、show、close 的先后顺序
- [ ] 日志中能定位到具体按钮 source

## 参考资料

- Slint `PopupWindow`:
  - https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow
- Slint `TouchArea`:
  - https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea/
- Slint positioning:
  - https://docs.slint.dev/latest/docs/slint/guide/language/coding/positioning-and-layouts
- Slint issue `#6446 Easy-to-use declarative tooltips`:
  - https://github.com/slint-ui/slint/issues/6446
