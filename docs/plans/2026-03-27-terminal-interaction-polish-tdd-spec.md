# Terminal Interaction Polish TDD Spec

日期: 2026-03-27
方案名: `terminal-interaction-polish`
关联设计: `docs/plans/2026-03-26-terminal-interaction-polish-design.md`
关联计划: `docs/plans/2026-03-26-terminal-interaction-polish-implementation-plan.md`

## 范围概述

本轮实现围绕现有 `Rust + Slint + Tokio + wezterm-term` 架构，补齐了成熟桌面终端需要的三类基础契约：

- runtime 侧 scrollback 元数据投影、输入后回底语义、主题 palette 可读性；
- Slint `TerminalSessionHost` 的本地快捷键、滚动条显示、thumb drag / jump 交互；
- `SessionManager` 与 `bootstrap` 对 active session surface 的同步、滚动命令转发与窗口属性投影。

## 核心 Struct

- `TerminalSurfaceState`  
  路径: `src/app/ssh/runtime.rs`  
  终端 surface 对外快照。现在除了 `cells / visible_lines / cursor / mouse_grabbed` 外，还包含：
  - `viewport_offset_lines`
  - `viewport_max_offset_lines`
  - `viewport_at_bottom`

- `TerminalSession`  
  路径: `src/app/ssh/runtime.rs`  
  `wezterm-term` 的本地包装层，负责：
  - 远端字节流写入与过滤
  - 键盘 / 鼠标 / paste 编码
  - 本地 scrollback 变更
  - surface snapshot 生成
  - 输入或新输出到来时的 `snap_viewport_to_bottom()`

- `SshSessionRuntime`  
  路径: `src/app/ssh/runtime.rs`  
  异步 SSH runtime 实例，持有：
  - `terminal: Arc<Mutex<TerminalSession>>`
  - `command_tx: mpsc::UnboundedSender<RuntimeCommand>`

- `SessionManager`  
  路径: `src/app/ssh/session_manager.rs`  
  runtime 与 UI 之间的会话编排层，负责：
  - session registry
  - runtime control attach / detach
  - terminal surface 缓存
  - `scroll_session_viewport`
  - `scroll_session_to_top`
  - `scroll_session_to_bottom`
  - `scroll_session_to_ratio`

- `ShellSessionBridge`  
  路径: `src/app/bootstrap.rs`  
  当前只封装 `SessionManager`，用于 bootstrap 内回调桥接。

- `ShellViewModel`  
  路径: `src/shell/view_model.rs`  
  UI 层当前 workspace tab 与 active terminal surface 的本地状态源。

- `TerminalSessionHost`  
  路径: `ui/shell/terminal-session-host.slint`  
  终端宿主组件，只负责：
  - 渲染 cells / cursor / selection
  - 捕获键盘、鼠标、滚轮、context menu
  - 计算滚动条 thumb 尺寸与位置
  - 发出本地 terminal callbacks

## Trait 与接口契约

- `SessionRuntimeControl`  
  路径: `src/app/ssh/session_manager.rs`  
  当前实现依赖以下接口：
  - `disconnect() -> Result<()>`
  - `send_text_input(text: String) -> Result<()>`
  - `send_key_input(event: TerminalKeyEvent) -> Result<()>`
  - `send_mouse_input(event: TerminalMouseInput) -> Result<()>`
  - `send_paste(text: String) -> Result<()>`
  - `resize(rows: u32, cols: u32) -> Result<()>`
  - `update_theme_mode(mode: ThemeMode) -> Result<Option<TerminalSurfaceState>>`
  - `scroll_viewport_lines(delta: i32) -> Result<TerminalSurfaceState>`

- `SessionRuntimeLauncher`  
  路径: `src/app/ssh/session_manager.rs`  
  会话生命周期入口：
  - `launch(profile, session_id, event_tx)` 返回 runtime control
  - `probe(profile)` 只做连接探测，不注册 session

- `TerminalSessionHost` 的本地快捷键契约  
  路径: `ui/shell/terminal-session-host.slint`  
  已落地的本地拦截规则：
  - `Ctrl+Shift+C` / `Ctrl+Insert` 复制 selection
  - `Ctrl+Shift+V` / `Shift+Insert` 触发本地 paste
  - `Shift+PageUp` / `Shift+PageDown` 本地滚动 viewport
  - `Ctrl+Shift+Home` / `Ctrl+Shift+End` 跳到 top / bottom
  未命中的组合键继续交给 runtime 键盘编码层。

## Slint Callbacks / Global State / Bindings

本轮没有新增 Slint global singleton。状态仍然通过 `AppWindow` 的 properties 与 `ShellViewModel` 持有。

### AppWindow / WorkspacePane / TerminalSessionHost 绑定链路

- `AppWindow` 新增或接通的 property：
  - `workspace-session-viewport-offset-lines`
  - `workspace-session-viewport-max-offset-lines`
  - `workspace-session-viewport-at-bottom`

- 属性透传路径：
  - `AppWindow`
  - `WorkspacePane`
  - `TerminalSessionHost`

- `TerminalSessionHost` 发出的关键 callbacks：
  - `text-input(string)`
  - `key-input(string, bool, bool, bool)`
  - `paste-requested()`
  - `scroll-requested(int, int, int, bool, bool, bool)`
  - `scroll-thumb-drag-requested(float)`
  - `scroll-jump-requested(float)`
  - `copy-selection-requested(int, int, int, int)`
  - `mouse-input(string, string, int, int, bool, bool, bool)`

- `bootstrap` 里接到的窗口级 callbacks：
  - `workspace_session_text_input`
  - `workspace_session_key_input`
  - `workspace_session_paste_requested`
  - `workspace_session_scroll_requested`
  - `workspace_session_scroll_thumb_drag_requested`
  - `workspace_session_scroll_jump_requested`

### UI 状态投影要求

- 如果 active session 有 surface：
  - `cells / visible_lines / cursor / mouse_grabbed`
  - `viewport_offset_lines / viewport_max_offset_lines / viewport_at_bottom`
  必须来自同一份 `TerminalSurfaceState`

- 如果 active session 没有 surface：
  - viewport 必须回退到 `0 / 0 / true`
  - 避免滚动条残留旧状态

## Tokio Task / Channel / Actor 交互关系

- `SessionManager.open_session(...)` 会启动两类异步任务：
  - runtime launch task
  - runtime event receive task

- runtime command 通道：
  - `SshSessionRuntime.command_tx: mpsc::UnboundedSender<RuntimeCommand>`
  - UI / manager 调用 runtime control 后，将命令发送到 runtime actor

- runtime event 通道：
  - `mpsc::UnboundedSender<SessionRuntimeEvent>`
  - runtime 发送 `Connected / SurfaceChanged / Disconnected / Error`
  - `SessionManager` 在 receive task 内消费并更新 `SessionRegistry`

- UI 同步链路：
  - `bootstrap` 内定时器周期性执行 `sync_workspace_projection_from_manager(...)`
  - 文本输入、按键、paste、本地滚动条拖拽与跳转之后，还会同步调用 `refresh_active_workspace_projection(...)`
  - 这样可以在 timer tick 之前立即刷新窗口 projection

## 状态流转说明

1. `TerminalSessionHost` 捕获本地输入、快捷键或滚动条动作。
2. `AppWindow` callback 进入 `bootstrap`。
3. `bootstrap` 根据 active session：
   - 对 text / key / paste 先执行 `snap_active_workspace_viewport_to_bottom_if_needed(...)`
   - 对 scrollbar thumb / jump 调用 `SessionManager.scroll_session_to_ratio(...)`
   - 对 wheel 调用 `scroll_session_viewport(...)`
4. `SessionManager` 调用 runtime control，并把返回的 `TerminalSurfaceState` 缓存到 registry。
5. `refresh_active_workspace_projection(...)` 或定时器调用 `sync_workspace_projection_from_manager(...)`。
6. `ShellViewModel` 更新 active tab 与 active terminal surface。
7. `sync_workspace_session_state(...)` 把同一份 surface 投影到 `AppWindow` properties。
8. `TerminalSessionHost` 重新渲染 cells、cursor 与 scrollbar。

## 关键错误处理策略

- `SessionManager` 对未就绪 runtime 使用 `anyhow!` 返回显式错误，避免静默失败。
- `scroll_session_to_ratio(...)` 会先 clamp `ratio` 到 `0.0..=1.0`，再按 `max_offset` 计算目标 offset。
- `scroll_session_to_offset(...)` 对 delta 做 `i32::try_from(...)`，超界时返回带上下文的错误。
- `bootstrap` 中所有 terminal forwarding 失败路径都只记录 `tracing::error!`，不让 UI panic。
- `sync_workspace_session_state(...)` 在 surface 缺失时会清空 cells 并重置 viewport 相关属性，避免 UI 残影。
- 弱引用窗口句柄 `window.as_weak()` 升级失败时直接返回，避免窗口销毁后的悬挂 UI 更新。

## 潜在边缘情况

- Tokio channel 阻塞或消息堆积  
  当前 runtime command 与 event 都是 `UnboundedSender`。优点是低摩擦，缺点是远端高频输出时理论上可能堆积。当前缓解方式是 manager 只保留最新 `TerminalSurfaceState`，UI 轮询取最终快照；后续可考虑 coalescing 或 bounded channel。

- UI 线程更新时机不正确  
  如果只依赖 50ms projection timer，输入后滚动条会短暂滞后。本轮通过 `refresh_active_workspace_projection(...)` 在 text / key / paste / thumb drag / jump 后立即刷新，降低延迟窗口。

- 数据竞争或共享状态不一致  
  `TerminalSession` 与 `SessionRegistry` 都通过 `Mutex` 保护。需要继续避免在未来引入交叉锁顺序，否则有死锁风险。当前实现通过快照比较后整体替换，避免半更新状态。

- 资源释放时序问题  
  `close_session(...)` / `disconnect_session(...)` 会处理 `runtime_controls`、`pending_disconnects`、`pending_resizes`。如果窗口先销毁，UI 不再消费 projection，但 runtime 仍可能短时间继续发送事件，必须容忍这种尾部事件。

- 异步任务取消或界面关闭后的悬挂回调  
  `window.as_weak()` 升级失败后直接退出回调，避免访问已销毁窗口。未来如果增加更复杂的后台 actor，还需要显式取消 session projection timer 与 runtime task 的生命周期绑定。

- Slint model 更新与实际数据源不同步  
  `visible_lines`、`cells`、cursor、viewport 三元组必须从同一个 `TerminalSurfaceState` 投影；任何只更新部分字段的路径都会导致 scrollbar 与内容不同步。

- 鼠标抓取状态与本地滚动冲突  
  `mouse_grabbed == true` 时 wheel 事件必须转为远端 mouse input，而不是本地 scrollback；否则会破坏 TUI 应用的交互语义。

- ratio 计算边界  
  `0.0` 必须映射到底部，`1.0` 必须映射到顶部；`max_offset == 0` 时拖拽和跳转应保持稳定，不得产生负值或无意义 delta。

## 后续适合补充的测试

### 单元测试

- `SessionManager.scroll_session_to_ratio(...)` 的 rounding 与 clamp 边界测试
- `TerminalSession.snap_viewport_to_bottom()` 在 keyboard / paste / remote output 三条路径上的独立覆盖
- theme 切换后默认 palette 前景 / 背景对比度的快照测试

### 集成测试

- 远端高频 `SurfaceChanged` 事件下，`sync_workspace_projection_from_manager(...)` 只保留最新 surface 的一致性测试
- session close / disconnect 与 pending resize 并发交错时的 registry 清理测试
- UI 已关闭但 runtime 仍发送尾部事件时，不发生 panic 的测试

### UI 交互测试

- `Ctrl+Shift+C/V`、`Ctrl+Insert`、`Shift+Insert`、`Shift+PageUp/PageDown` 的 Slint callback contract smoke
- scrollbar thumb drag / jump 与 viewport property 的往返一致性测试
- selection、context menu、mouse grabbed 与 scrollbar 之间的互斥行为测试

### 手工验证建议

- Windows 11 下验证 Fluent 风格终端在浅色 / 深色主题切换时的可读性
- Windows 11 下验证滚动条 thumb hit target、拖拽流畅度与输入即回底观感
- 使用真实远端 shell / TUI 程序验证 mouse grabbed 模式下 wheel forwarding 是否符合预期
