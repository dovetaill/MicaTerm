# SFTP Workspace Usability Hardening Design

日期: 2026-05-20
执行者: Codex
状态: 已整理，待在新 worktree 中执行

## 设计目标

在当前 `46fc0bf` 已 merge 的 `SFTP Workspace Productized UI` 基线上，不重写 SFTP backend，也不推翻 two-layer architecture，而是把 SFTP workspace 从“有真实数据、但交互 contract 不受控”的状态，收口为一个受控的 MicaTerm shell surface。

这份设计是对 `docs/plans/2026-05-19-sftp-workspace-productized-ui-design.md` 的后续硬化，不重复定义 why / product positioning，而是专门解决本轮暴露出来的 contract gap、假绿测试和 UI state routing 问题。

## 设计输入

### 本地事实

- 当前真实 `HEAD`: `46fc0bf (HEAD -> master, origin/master) Merge branch 'feat/sftp-workspace-productized-ui'`
- 当前测试虽然全绿，但用户报告 scroll、path edit、tooltip、context menu、disconnected state 仍不可靠。
- 设计图路径：`/home/images/13357dd2-cbcd-4fdc-aea9-2c27864f9192.png`

### 已确认根因线索

- `src/shell/view_model/sftp.rs` 用 `44px` 作为 workspace SFTP 虚拟列表行高；`ui/shell/sftp-workspace-host.slint` 的实际 row height 为 `40px`。
- workspace host 内部维护私有 `list-viewport-y`，但整个 projection/binding 链没有把 viewport 做成完全受控状态。
- path edit 目前是 submit-only 结构，`Esc` 语义和 `Enter` 混用，`Ctrl+L` 没有完整入口。
- workspace toolbar 的 tooltip 文本虽然部分已写入 Slint，但没有像 titlebar/right-panel 那样把 tooltip owner/anchor/scheduler 真正接到共享 overlay。
- SFTP workspace context menu 仍通过 assets overlay plumbing 打开，selection/mutable/path 还夹带 quick browser accessor。
- disconnected/error/empty 当前仍以 overlay-card 形式盖在列表上，不是内容区状态面。

### 外部参考

- Slint `ScrollView` / `Flickable` 官方文档说明：`viewport-y` 通常为负值，`scrolled()` / `flicked()` 用于回传用户滚动，且 `for` 循环场景下 viewport size 不能完全指望自动计算。
- Microsoft BreadcrumbBar 指南强调：path 应持久可见、上层 segment 可直接返回、窄宽度下允许折叠而不是失真。
- GNOME Files/keyboard navigation 文档提供了 location bar 与键盘菜单的成熟惯例：键盘应能进入路径编辑，`Esc` 应该是 cancel / close 当前 transient，不应伪装成 submit。
- VS Code theme color reference 提供了成熟的 list/tree state 语义拆分：active selection、inactive selection、hover、focus outline 应分离，而不是用一个硬边框吃掉所有状态。

参考链接：

- <https://docs.slint.dev/latest/docs/slint/reference/std-widgets/views/scrollview/>
- <https://releases.slint.dev/1.16.0/docs/slint/reference/gestures/flickable>
- <https://learn.microsoft.com/en-us/windows/apps/design/controls/breadcrumbbar>
- <https://help.gnome.org/gnome-help/keyboard-nav.html>
- <https://code.visualstudio.com/api/references/theme-color>

## 现状问题不是孤立 UI 小 bug，而是 contract 没收口

本轮问题的共同模式不是“某几个按钮没配对”，而是：workspace SFTP 复用了 quick browser 的 render/projection 基础，但以下状态没有被设计成单一真源：

- viewport
- row metrics
- path edit state machine
- tooltip ownership
- context menu surface routing
- content-state rendering mode

结果就是：静态字符串可以绿，行为仍然漂移。

## 方案比较

## 方案 A：在现有架构上把 workspace host 收口成受控 shell

### 做法

- 保留 `FileBrowserSession`、`WorkspaceTab::Sftp`、现有 bootstrap/controller/backend 路径。
- 补齐 controlled contract：
  - 单一 row-height 来源
  - 受控 viewport-y + reset policy
  - `visible_rows` 与 `total_row_count` 分离
  - path edit 状态机
  - tooltip owner / disabled reason
  - surface-aware context menu
  - unified content-state mode

### 优点

- 最符合“本轮不是重做 backend”的约束。
- 修改范围聚焦在 projection、Slint host、bootstrap routing、测试。
- 能保住 05-19 productized UI 已经落下来的大部分结构。

### 缺点

- 仍然继续维护手写 virtualization 与部分自定义 Slint 交互逻辑。

## 方案 B：抽 shared component，把 quick browser 与 workspace 全量统一

### 做法

- 把 path bar、toolbar、table row、tooltip wiring、viewport policy 抽为共享组件。
- quick browser 与 workspace 只在 density / columns / actions 上做层级差异。

### 优点

- 长期最干净，可避免两套 SFTP UI 再漂移。

### 缺点

- 本轮改动面太大，容易把“hardening”变成“大重构”。
- 需要更长的实现窗口与更重的回归测试。

## 方案 C：workspace 暂时退化为非虚拟全量列表

### 做法

- workspace 保留受控 path/menu/state contract，但不立即继续使用现有 virtualization。
- 在一个有上限的 full-list 模式下先修可用性。

### 优点

- 最快恢复滚动正确性与 viewport 可预测性。

### 缺点

- 大目录性能有风险。
- quick browser / workspace 的技术路径暂时分叉。

## 推荐结论

采用 **方案 A** 作为主方案，并把 **方案 C** 作为明确的 fallback gate：

- 第一优先级是把 workspace SFTP 收口成受控 shell，而不是继续追加视觉 patch。
- 若在实现期内无法稳定修好 virtualization，则允许在受控 contract 下临时切换到 bounded full-list fallback。
- 方案 B 留作后续统一 quick browser/workspace 的独立工作，不在本轮混入。

## 最终设计

## 1. 受控状态模型

workspace SFTP 后续必须把以下状态视为 contract，而不是 host 私有实现细节：

- `workspace_sftp_total_row_count`
- `workspace_sftp_visible_rows`
- `workspace_sftp_row_height_px`
- `workspace_sftp_viewport_y`
- `workspace_sftp_visible_height`
- `workspace_sftp_path_mode = viewing | editing`
- `workspace_sftp_path_draft`
- `workspace_sftp_focus_request_token` 或等价的 input focus/select-all 触发器
- `workspace_sftp_content_mode = ready | loading | connecting | empty | disconnected | error`
- `workspace_sftp_toolbar_tooltips`
- `workspace_sftp_disabled_reasons`
- `workspace_sftp_context_surface = quick_browser | workspace`

这组状态中，UI 不应再偷偷持有一个与 Rust 脱节的第二真源。

## 2. viewport 与 row metrics 设计

### 2.1 单一行高真源

- row height 必须来自一个地方，推荐由 Rust view-model 常量统一，并投影到 Slint 使用；或反过来由 Slint 常量输入 Rust，但不能双写。
- `total_content_height`、`top_spacer`、`bottom_spacer`、可见窗口计算都依赖同一值。

### 2.2 受控 viewport

- `workspace_sftp_viewport_y` 必须被视为 per-browser-session 的 view state，而不是 host 生命周期内的私有缓存。
- UI 的 `ScrollView.viewport-y` 与 view-model 必须双向受控同步。
- 必须显式考虑 Slint 中 `viewport-y` 通常为负值。

### 2.3 viewport reset / restore policy

以下事件必须 reset 到 top：

- path submit
- breadcrumb navigate
- Back / Forward / Up / Home
- 打开目录进入新 path
- explicit Refresh 完成
- Expand to Workspace

以下事件应 restore 该 tab 的已知稳定滚动位置：

- 切换到另一个 workspace tab 后再切回
- 仅焦点变化，不涉及重新加载目录

以下行为必须禁止：

- selection change 让列表莫名跳到中间
- render cache rebuild 无条件沿用旧 viewport

## 3. path bar 状态机设计

### 3.1 状态定义

- `Viewing`: 默认 breadcrumb 模式
- `Editing`: 文本输入模式

### 3.2 进入 Editing 的入口

- click path shell 空白
- click path text
- click pencil affordance
- `Ctrl+L`

### 3.3 Viewing -> Editing 转换副作用

- draft path = current canonical path
- input focus
- select all text
- 关闭任何与 path shell 冲突的 tooltip / menu transient

### 3.4 Editing -> Viewing 转换

- `Enter`: 验证并提交 path，再退出 editing
- `Esc`: 丢弃 draft，恢复 canonical path，退出 editing
- path submit reject: 保持 editing 并显示错误反馈，或回到 viewing 但保留 message；两者二选一，不能静默失败

### 3.5 breadcrumb 行为

- root 永远可点击
- 每个 segment 都有对应 canonical path
- 窄宽度时允许 segment 折叠或 host label/path label elide，但不能打断实际导航语义

## 4. tooltip 设计

### 4.1 复用现有 overlay

workspace SFTP tooltip 不单独做局部弹层，统一走 `AppWindow` 级 tooltip overlay，与 titlebar/right-panel/tabbar 共享样式与 z-order 规则。

### 4.2 button contract

现有 `WorkspaceIconButton` / `WorkspaceActionButton` 后续需要具备和 `sidebar-toolbar-icon-button` 同等级的 tooltip contract：

- tooltip text
- tooltip source id / anchor
- hover/focus 激活
- close on leave / blur / tab switch / popup open

### 4.3 disabled tooltip

- disabled 状态不能再用 `TouchArea.enabled: false` 直接切断 hover 信息。
- 点击禁用与可 hover 解释要分离。
- 文案来源可以是 view-model 直接投影的 reason string，也可以在 UI 基于状态拼装，但必须只有一个规则来源。

## 5. context menu 与 transient UI 设计

## 5.1 问题本质

当前问题不只是 action tree 错，而是 transient UI state 被切成多份：

- assets create popup
- assets context menu overlay
- workspace tab context menu
- workspace SFTP context menu request

但各自的 open/close/sync 没有统一 controller，导致“菜单串路由、create popup 不会先关、workspace 还在读取 quick browser path/mode”。

## 5.2 推荐状态模型

建立统一的 transient shell UI controller，至少统一管理：

- create popup
- context menu
- workspace tab context menu
- tooltip close-on-open 关系

即便视觉层暂时仍可复用 `AssetsContextMenuOverlay`，状态模型也必须先从 `assets-*` 命名里解耦出来。

### 建议最小抽象

- `TransientShellUiState`
- `ContextMenuSurface`
- `ContextMenuSnapshot`

`ContextMenuSnapshot` 在 open 时就固定：

- surface
- target kind
- selected ids
- mutable state
- active path
- anchor

避免 render 阶段再跨 surface 混读 quick browser / workspace 状态。

## 5.3 surface-aware action routing

- `SftpQuickBrowser` blank-area 才允许 `open-sftp-workspace`
- `SftpWorkspace` blank-area 不允许出现 `open-sftp-workspace`
- `copy-current-path` 必须基于 active workspace path 或 quick browser path 的 surface 区分
- dispatcher 不再直接把 workspace blank-area action 路由到 quick-browser-only helper

## 5.4 close-before-route 规则

以下所有入口都必须先执行 `dismiss_transients(...)`，再继续真实动作：

- workspace click
- workspace right-click
- workspace tab select
- workspace tab close
- sidebar destination switch
- 新 context menu 打开

不要再依赖整屏 dismiss layer 吞事件作为唯一关闭机制。

## 6. 内容区状态面设计

## 6.1 统一 content mode

workspace SFTP 内容区应以 `content_mode` 驱动：

- `ready`
- `loading`
- `connecting`
- `empty`
- `disconnected`
- `error`

### ready

- 显示文件表格与正常滚动区域

### empty

- 显示空目录状态面
- header / toolbar / status bar 保留

### disconnected

- 显示中心化 content state
- toolbar 保留但文件操作禁用
- `Reconnect` 按钮有效

### error

- 与 disconnected 共享版式语言，但文案和次级操作不同

## 6.2 视觉原则

- 是 content state，不是 modal
- 居中于文件列表区域
- 不应额外出现卡片叠层、错位边框、按钮压线
- 与 quick browser 的 status-row 语义一致，但允许 workspace 更完整

## 7. Header、table、status bar 的产品化语言

## 7.1 header

- host-first，不重复 tab title
- 超长值 ellipsis + tooltip
- secondary info 压缩成 `SFTP · Locked · Manual` 这类短摘要

## 7.2 toolbar 与 path band

- 单一紧凑 chrome band
- icon-first
- 响应式 width tier
- 不换行

## 7.3 table

- `Name` 为主列
- `Type / Size / Modified / Permissions / Owner / Group` 为可折叠次级列
- selected 使用 subtle fill + accent rail
- hover/focus 与 selected 分离

## 7.4 status bar

保留并强化：

- connection state
- item count
- selected count
- current path
- binding mode
- transfer badge

## 8. 主题与语义 token 设计

- 继续只使用 runtime-projected shell/session theme properties。
- 不在 Slint 重新 authored Ayu color ladder。
- selected / hover / focus 的表达优先复用既有语义字段：
  - selected fill
  - selected accent rail
  - hover fill
  - focus ring
- 若当前 token 不足，只允许最小增量扩展，并且必须从 Rust theme spec 端到 Slint 端全链路投影。

## 9. 测试设计

## 9.1 原则

- 这轮的关键不是再加字符串 grep，而是把用户真实操作路径变成行为测试。
- 先红后绿。
- 对 scroll/menu/popup 这类路由问题，测试前先做 root-cause tracing。

## 9.2 分层策略

- `L0: 薄 source smoke`
  - 只负责防误删关键 callback / property / anti-legacy 标记。
  - 不承担“布局未截断、交互可用、主题会刷新”的主回归职责。
- `L1: projection / state 单测`
  - 负责 `ShellViewModel`、`FileBrowserSession`、right-panel policy、workspace/quick-browser 会话隔离、virtualization dirty indices 等状态正确性。
- `L2: bootstrap / binder 集成`
  - 负责验证 `AppWindow` callback -> bootstrap -> view model -> controller/backend -> projection 的整条链路。
- `L3: render / element contract`
  - 负责验证用户真的看得到、点得到、不会被裁切、状态真的 repaint。
- `L4: 异步恢复与跨层回归`
  - 负责延迟 read-dir、断连重连、theme toggle、active tab 切换、policy-hidden 恢复等长期稳定性。

## 9.3 可复用的现有测试手法

- 复用 `tests/assets_explorer_smoke.rs` 一类 `ElementHandle` 交互模式来做 workspace SFTP 的真实 UI 操作。
- 复用 `tests/sftp_right_panel_render_spec.rs`、`tests/titlebar_render_spec.rs` 一类软件渲染采样方法，验证 header、breadcrumb、selected row、status bar、宽度断点下的可见结果。
- 保留少量 `read_to_string(...).contains(...)` 静态检查，但只把它们当作 `L0` 防误删 guard。

## 9.4 必须新增或重写的能力点

- viewport reset / restore
- root path 首行可见
- breadcrumb click -> navigate path
- `Ctrl+L` / `Enter` / `Esc`
- tooltip overlay wiring，不只是 tooltip text 常量
- disabled reason tooltip
- workspace row menu / blank menu 真 workspace fixture
- create popup close-before-route
- disconnected content-state rendering，不再是 overlay-card
- no raw hex / no second palette

## 10. 实施边界

### 本轮必须做

- requirements 中列出的 A-G 行为 contract
- 强化后的自动化测试与人工验收路径
- 在现有 architecture 上收口受控 shell

### 本轮不做

- quick browser / workspace 的大规模共享组件重构
- 全新 transfer center 架构
- 完整 inspector side panel redesign
- SFTP backend 重做

## 设计结论

本轮最佳解不是继续堆视觉 patch，而是把 workspace SFTP 明确重构为“受控 shell surface”：

- 受控 viewport
- 单一 row metric
- 正式 path edit 状态机
- 共享 tooltip overlay
- surface-aware context menu
- 统一 content-state mode
- runtime Ayu semantic token-only styling

这样既能解释为什么当前测试会假绿，也能给下一轮 worktree 实现提供清晰的 contract 和 fallback gate。
