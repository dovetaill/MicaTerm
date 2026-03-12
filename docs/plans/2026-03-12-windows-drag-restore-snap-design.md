# Mica Term Windows Drag Restore Snap Design

日期: 2026-03-12  
执行者: Codex  
状态: 已确认方案，待进入实现规划

## 背景

当前仓库已经完成 `frameless + transparent + self-drawn shell` 的窗口壳层，并且顶部状态栏、侧边栏和主题切换基础能力已经落地。但在 Windows 10 / 11 上，窗口经过 `maximize -> drag restore` 或 `snap -> drag unsnap` 后，出现了明显的壳层异常：

- 最大化后再拖拽顶部状态栏还原，顶部圆角与框架形态异常
- 标题栏看起来没有完整渲染，顶部壳层像被裁掉一部分
- 贴边分屏后再次拖拽，窗口出现严重的可见区域和形态错乱
- 问题发生在窗口壳层级别，不是 terminal 内容区逻辑错误

结合当前源码与截图，本轮问题的核心不是 SSH / SFTP / terminal stack，而是 `Windows frameless window` 在系统窗口状态变化时，缺少一层可靠的“真实状态同步 + frame 适配 + 恢复重绘”机制。

相关实现主要位于：

- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `src/app/windowing.rs`
- `src/app/window_effects.rs`
- `src/app/bootstrap.rs`
- `src/shell/view_model.rs`

最近相关提交主要是：

- `280600d fix: restore titlebar rendering in shell frame`
- `04d1ef3 fix: restore shell body height on resize`
- `991d032 修复 Light / Dark 切换时窗口超出屏幕区域出现不完全切换的问题`

这些提交已经修复了布局和部分重绘问题，但仍未建立 `maximize / snap / restore / unsnap` 的正式窗口状态机。

## 目标

- 修复 Windows 10 / 11 下 `maximize -> restore-drag` 与 `snap -> unsnap-drag` 的窗口壳层异常
- 保持当前 `no-frame: true` 与自绘标题栏路线，不回退系统默认标题栏
- 明确区分“系统设计行为”和“真实 bug”
- 为后续 `Fluent + Mica` 深化、终端控件接入以及跨平台迁移保留清晰架构边界
- 将当前零散的窗口恢复 workaround 收敛为可解释、可验证、可扩展的状态管理方案

## 边界

### 本文档覆盖

- Windows 窗口壳层形态策略
- 窗口真实状态来源设计
- `restore-drag / unsnap-drag` 恢复链路
- Windows 非客户区消息适配深度
- 高层实施步骤
- 风险、回滚与验证清单

### 本文档不覆盖

- `wezterm-term` / `termwiz` 接入
- `russh` / SFTP 功能
- Welcome / Sidebar / TabBar 的视觉重做
- 数据持久化结构调整
- 逐文件命令级 implementation plan

## 当前现状与调研结论

### 1. 当前窗口路线是完整自绘壳层

当前 `AppWindow` 明确采用：

- `no-frame: true`
- `background: transparent`
- 内部 `shell-frame` 自绘边框、圆角和背景
- 顶栏拖拽通过 `winit::window::Window::drag_window()`

这说明当前产品路线已经锁定在“自绘标题栏 + 原生窗口能力桥接”，而不是“原生标题栏微定制”。

### 2. 当前最大化状态只覆盖自家交互路径

目前 `is_window_maximized` 主要在以下路径更新：

- 顶栏最大化按钮
- 顶栏双击

但 Windows 自己触发的状态变化，例如：

- 贴边分屏
- 从最大化窗口直接拖拽还原
- Win + 方向键
- 系统菜单最大化 / 还原

并没有成为统一的真状态源。

### 3. 仓库已有 redraw workaround，但只服务另一类问题

`src/app/bootstrap.rs` 中已经存在针对“主题切换离屏恢复”的 Windows 专用恢复逻辑：

- `render-revision`
- `request_redraw()`
- `request_inner_size(+1px -> restore)`

这套机制已经证明窗口壳层在 Windows 状态切换边界上是脆弱的，但当前并未扩展到 `maximize / snap / restore / unsnap` 这条链路。

### 4. 外部资料确认了两个关键事实

#### 4.1 Windows 11 下 snapped / maximized 无圆角是系统设计

微软官方文档明确指出，Windows 11 顶层窗口在以下状态下默认不显示圆角：

- maximized
- snapped
- VM / WVD / WDAG 等特殊环境

因此：

- “贴边后圆角变方”本身不是 bug
- “恢复拖拽后依然保持错误形态”才是本轮要修的 bug

#### 4.2 自绘标题栏若想获得更完整原生行为，通常需要参与非客户区消息

winit 与 Windows 社区实践都表明：

- 单靠 `drag_window()` 可以触发系统移动
- 但若需要更完整的 `snap layout`、`maximize hover`、caption hit-test 行为，通常要参与 `WM_NCHITTEST`
- Windows 11 的 maximize hover `snap layout` 依赖 `HTMAXBUTTON`

这意味着当前只靠 Slint UI 回调和 `drag_window()` 的做法，在 Windows 上是功能不完整的。

### 5. Slint 1.15 已提供可复用的无边框 resize 基础能力

当前仓库使用 `slint 1.15.1`。这版 Slint/winit 后端已内建：

- `resize-border-width`
- `drag_resize_window`

仓库目前尚未启用该能力。这说明后续边框 resize 不需要再从零自研。

## 设计原则

- `State First`
  先建立真实窗口状态模型，再讨论外形和重绘。
- `Windows Truth, Cross-Platform Boundary`
  Windows 首发平台允许使用 Win32 作为真状态源，但必须封装在平台适配层中。
- `System-Consistent Geometry`
  恢复态是圆角，最大化和贴边态是方角，遵循 Windows 11 规则。
- `Explicit Recovery`
  恢复拖拽和贴边恢复后，不依赖“系统也许会重绘正确”，必须设计显式同步和 fallback。
- `Keep Frameless Route`
  不为了修 bug 而回退系统标题栏。

## 设计要点与方案对比

### 1. 窗口外形策略

#### 方案 1A：始终保持自绘圆角

做法：

- `shell-frame` 永远维持圆角
- 不区分 restored / maximized / snapped

优点：

- 视觉上始终统一
- 实现最简单

缺点：

- 与 Windows 11 官方几何规则冲突
- maximized / snapped 时更容易暴露透明角或裁剪异常
- 不利于排查“系统状态切换”和“渲染异常”之间的责任边界

#### 方案 1B：状态感知外形策略

做法：

- `Restored` 使用圆角
- `Maximized` 和 `Snapped` 使用方角
- `Restore / Unsnap` 后再回到圆角

优点：

- 符合 Windows 11 行为
- 能把“贴边方角”从 bug 列表中剥离
- 最有利于形成稳定状态机

缺点：

- 依赖准确的真实窗口状态识别

#### 方案 1C：尽量把圆角交给系统边框

优点：

- 理论上更原生

缺点：

- 与当前 `transparent + self-drawn shell` 路线冲突大
- 基本等同于重做窗口外壳

**最终决策：选择 `1B`**

### 2. 窗口真实状态来源

#### 方案 2A：继续使用 UI 内部布尔状态

做法：

- 保持 `is_window_maximized`
- 只在自家按钮和双击路径更新

优点：

- 改动小

缺点：

- 无法覆盖系统触发的 maximize / snap / restore / unsnap
- 不足以修复本轮问题

#### 方案 2B：基于 winit event 的推导状态机

做法：

- 监听 `Moved / Resized / ScaleFactorChanged`
- 结合 `is_maximized()`、窗口矩形、monitor work area 推导 `Restored / Maximized / Snapped`

优点：

- 平台层较轻
- 架构相对跨平台

缺点：

- `Snapped` 的判定需要 heuristic
- Windows 边角条件较多

#### 方案 2C：Windows 真状态源适配层

做法：

- Windows 下使用 Win32 placement / rect / work area 信息作为真状态源
- 对外统一输出平台无关状态，例如：
  - `Restored`
  - `Maximized`
  - `SnappedLeft`
  - `SnappedRight`
  - `SnappedTop`
  - `SnappedBottom`

优点：

- Windows 上最稳
- 对系统触发的状态变化感知最完整
- 最适合支撑恢复拖拽与贴边恢复

缺点：

- 需要新增 Windows 专用适配层

**最终决策：选择 `2C`**

### 3. `restore-drag / unsnap-drag` 恢复链路

#### 方案 3A：扩展现有 workaround

做法：

- 将当前 `render-revision + redraw + size nudge` 机制扩展到窗口状态恢复场景

优点：

- 见效快
- 可复用当前仓库已有思路

缺点：

- 仍属于补丁式修复
- 长期维护成本高

#### 方案 3B：显式窗口过渡状态机 + fallback redraw

做法：

- 引入显式过渡状态，例如：
  - `Restored`
  - `Maximized`
  - `Snapped*`
  - `RestoringFromMaximizedDrag`
  - `RestoringFromSnapDrag`
- 在状态迁移时按固定顺序执行：
  1. 更新真实窗口状态
  2. 更新 UI 壳层外形
  3. 更新布局缓存
  4. 请求重绘
  5. 仅在必要时触发 fallback `size nudge`

优点：

- 逻辑清晰，可记录日志，可测试
- 能解释并覆盖当前两类异常场景
- 后续维护成本最低

缺点：

- 设计和实现复杂度高于直接补 workaround

#### 方案 3C：优先转向 Skia renderer

优点：

- 若根因在 software renderer，可能一并解决更多问题

缺点：

- 当前仓库昨天已确认：现有 Linux -> Windows GNU 链路不适合作为短期主修路线
- 不适合作为本轮主方案

**最终决策：选择 `3B`，并保留 `3A` 作为 fallback**

### 4. Windows 原生集成深度

#### 方案 4A：只使用 Slint / winit 公共 API

做法：

- 保持 `drag_window()`
- 补 `resize-border-width`
- 不处理 Win32 非客户区消息

优点：

- 平台代码最少
- 架构最干净

缺点：

- Windows 11 maximize hover / snap layout 不完整
- caption hit-test 能力不足
- 长期仍然像“近似原生”而不是真原生

#### 方案 4B：增加 Windows frame adapter

做法：

- 在 Windows 下新增一层专用 frame adapter
- 参与必要的非客户区行为：
  - `WM_NCHITTEST`
  - `HTCAPTION`
  - `HTMAXBUTTON`
  - 必要的 client / frame rect 协调
- 与 Slint 自绘标题栏保持职责分离：
  - Slint 负责视觉与交互区定义
  - Windows adapter 负责把这些交互区映射为系统可识别的 caption 语义

优点：

- 最符合 Windows 11 产品目标
- 为 snap layout、caption hit-test、restore-drag 提供最完整支撑
- 平台差异被隔离在专用适配层

缺点：

- Windows 专用代码明显增加
- 需要良好抽象，避免污染其他平台

#### 方案 4C：回退系统标题栏

优点：

- 系统行为天然正确

缺点：

- 与当前产品和设计路线冲突
- 不可接受

**最终决策：选择 `4B`**

## 最终决策

本轮最终确认方案为：

- `1B` 状态感知外形策略
- `2C` Windows 真状态源适配层
- `3B` 显式窗口过渡状态机 + fallback redraw
- `4B` Windows frame adapter

### 决策摘要

1. `贴边 / 最大化时方角` 视为系统设计，不作为 bug 修复目标。
2. 真正修复目标是：窗口从 `Maximized / Snapped` 回到 `Restored` 时，壳层形态、标题栏、布局和重绘必须同步回到正确状态。
3. Windows 下不再依赖 UI 内部布尔值作为唯一状态源，而是引入平台真状态源。
4. 当前已有 redraw workaround 不删除，但从“主方案”降级为“fallback 兜底”。
5. 自绘标题栏路线保持不变，但要补一层 Windows 非客户区适配能力。

## 高层设计

### 1. 平台无关窗口状态模型

建议引入统一状态枚举，例如：

- `Restored`
- `Maximized`
- `SnappedLeft`
- `SnappedRight`
- `SnappedTop`
- `SnappedBottom`
- `Unknown`

该状态模型不直接暴露 Win32 细节，供：

- `shell-frame` 外形策略
- 标题栏布局与交互策略
- 恢复拖拽过渡逻辑
- 后续日志与测试

### 2. Windows 状态适配层

Windows 专用适配层负责：

- 读取真实窗口 placement / rect / work area
- 解析当前属于 `Restored / Maximized / Snapped*`
- 在事件变化时向上层发布稳定状态

该层不负责视觉绘制，不直接操作 Slint 组件细节。

### 3. 壳层外形策略层

壳层策略层只做两件事：

- 根据真实状态决定 `shell-frame` 当前使用圆角还是方角
- 根据状态决定是否需要额外裁剪、边框和布局刷新

这样可以把“视觉外形”从“平台消息细节”中剥离出来。

### 4. 恢复链路控制器

恢复链路控制器负责管理：

- `maximize -> drag restore`
- `snap -> unsnap drag`
- 必要时的 `redraw / render-revision / size nudge`

它不猜测状态，而是消费“已经被确认”的真实窗口状态。

### 5. Windows frame adapter

该层负责把自绘标题栏与系统窗口语义对齐：

- 让拖拽区被系统识别为 caption
- 让 maximize 按钮区域具备 `HTMAXBUTTON` 能力
- 为后续 Windows 11 maximize hover `snap layout` 预留能力

## 实施步骤

1. 在 Windows 平台层补充真实窗口状态读取与归一化输出，不再只依赖 `is_window_maximized`。
2. 为 `AppWindow` 引入可绑定的壳层状态属性，使 `shell-frame` 能在 `Restored` 与 `Maximized / Snapped` 间切换外形。
3. 新增恢复过渡状态机，覆盖 `maximize -> restore-drag` 与 `snap -> unsnap-drag`。
4. 将现有 redraw workaround 收敛成 fallback 策略，只在恢复链路确认重绘不同步时触发。
5. 在 Windows 下增加 frame adapter，把拖拽区、maximize hit-test 与系统 caption 语义对齐。
6. 启用 Slint 现有 `resize-border-width` 路线，避免后续边框 resize 再次自研。
7. 补充 geometry / state / recovery 测试，确保这类回归可被自动识别。

## 风险

### 1. Win32 适配层复杂度上升

风险：

- 增加 Windows 专用代码
- 若抽象边界不清，容易污染跨平台结构

应对：

- 平台相关逻辑仅放在 `src/app/windows_*` 或等价平台模块
- 向上只暴露平台无关枚举和接口

### 2. 过渡状态机与现有 workaround 叠加后顺序错误

风险：

- 重绘顺序反而制造新的闪烁或尺寸抖动

应对：

- 先建立单一状态迁移顺序
- fallback workaround 只允许在最终步骤触发

### 3. `HTMAXBUTTON` 与自绘按钮区域不一致

风险：

- maximize hover 行为与实际按钮热区不一致

应对：

- 统一由标题栏布局导出按钮几何信息
- Windows frame adapter 消费同一份几何数据

### 4. Windows 10 与 Windows 11 表现存在差异

风险：

- Windows 10 无 snap layout hover
- Windows 11 有 maximize hover 与更明显的圆角规则

应对：

- 状态机与恢复链路保持共用
- 平台体验增强按 OS capability 分支处理

## 回滚策略

若 Windows frame adapter 第一阶段验证不稳定，可按以下顺序回滚：

1. 保留 `1B + 2C + 3B`，暂时关闭 `4B` 中的增强 hit-test，仅保留状态机和恢复链路。
2. 若恢复链路仍不稳定，保留真状态源，但回退到更保守的 fallback redraw 方案。
3. 不回退 `no-frame: true`，也不回退系统标题栏。

## 验证清单

- [ ] 默认恢复态窗口为圆角，顶部壳层完整渲染
- [ ] 最大化后窗口转为方角，且不出现异常透明角
- [ ] 从最大化状态拖拽标题栏还原后，圆角与标题栏完整恢复
- [ ] Windows 10 / 11 下左右贴边分屏后，窗口外形切换为方角
- [ ] 从贴边分屏状态再次拖拽后，窗口恢复正常 restored 外形
- [ ] 过程中不出现标题栏截断、壳层缺角、大片未重绘区域
- [ ] 顶栏按钮与拖拽热区仍然行为正确
- [ ] 现有主题切换离屏恢复逻辑不被破坏
- [ ] Windows 11 下 maximize 按钮区域为后续 `snap layout` 扩展保留接入点

## 参考资料

- Microsoft Learn: Apply rounded corners in desktop apps for Windows 11
- Microsoft Learn: Custom Window Frame Using DWM
- Microsoft Learn: `WM_NCHITTEST`
- winit issue `#1548` and related borderless snap discussions
- winit issue `#3884` about `HTMAXBUTTON` for Windows 11 snap layout
- Slint `WinitWindowAccessor` and `resize-border-width` support
