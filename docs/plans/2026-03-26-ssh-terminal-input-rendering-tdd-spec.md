# SSH Terminal Input Rendering TDD Spec

日期: 2026-03-26
方案名: `ssh-terminal-input-rendering`
状态: 已完成实现后的测试与交接规格

## 核心 Struct

- `TerminalSession`
  负责封装 `wezterm-term` 实例、keyboard mode 观察、parser 前横幅过滤、本地 viewport offset、theme-aware palette config，以及 surface projection。
- `SshSessionRuntime`
  负责 SSH channel 生命周期、`RuntimeCommand` 下行命令入口、`SessionRuntimeEvent` 上行事件输出，以及对 `TerminalSession` 的线程安全封装。
- `TerminalSurfaceState`
  作为 runtime 到 UI 的稳定投影快照，包含 `visible_rows`、`visible_lines`、`cells`、`cursor`、`mouse_grabbed`、`bracketed_paste_enabled`。
- `TerminalKeyEvent`
  用于表达 live encoder 的结构化键盘输入，支持 `Named`、`Function`、`Char`。
- `TerminalMouseInput`
  用于表达鼠标/滚轮输入，包含 `Down`、`Up`、`Move`、`Scroll` 与 `WheelUp`、`WheelDown`。
- `SessionTerminalConfig`
  负责基于 `ThemeMode` 生成终端 palette，并通过 generation 变化驱动 `wezterm-term` 刷新配置。
- `SessionManager`
  负责 session registry、runtime control 挂载、terminal surface 缓存、pending resize/disconnect、theme mode 广播。

## Trait 与接口契约

- `SessionRuntimeControl`
  当前稳定接口包括：
  - `disconnect()`
  - `send_text_input(text: String)`
  - `send_key_input(event: TerminalKeyEvent)`
  - `send_mouse_input(event: TerminalMouseInput)`
  - `send_paste(text: String)`
  - `resize(rows: u32, cols: u32)`
  - `update_theme_mode(mode: ThemeMode) -> Result<Option<TerminalSurfaceState>>`
  - `scroll_viewport_lines(delta: i32) -> Result<TerminalSurfaceState>`
- `SessionRuntimeLauncher`
  负责异步创建 runtime control，并把 `SessionRuntimeEvent` 绑定回 manager。

## Slint 回调 / Global State / Bindings

### 关键回调

- `workspace-session-text-input(string)`
- `workspace-session-key-input(string, bool, bool, bool)`
- `workspace-session-resize-requested(int, int)`
- `workspace-session-copy-selection-requested(int, int, int, int)`
- `workspace-session-paste-requested()`
- `workspace-session-scroll-requested(int, int, int, bool, bool, bool)`
- `workspace-session-mouse-input(string, string, int, int, bool, bool, bool)`

### 关键 UI 状态投影

- `workspace-session-cells`
- `workspace-session-rows`
- `workspace-session-cols`
- `workspace-session-cursor-row`
- `workspace-session-cursor-col`
- `workspace-session-cursor-visible`
- `workspace-session-cursor-blinking`
- `workspace-session-cursor-shape`
- `workspace-session-cursor-fg`
- `workspace-session-cursor-bg`
- `workspace-session-mouse-grabbed`
- `workspace-session-visible-lines`
- `workspace-session-surface-seqno`

### TerminalSessionHost 的共享 metrics contract

- `terminal-font-family`
- `terminal-font-size`
- `terminal-cell-width`
- `terminal-cell-height`
- `terminal-cursor-thickness`
- `terminal-cell-x() / terminal-cell-y()`
- `terminal-hit-row() / terminal-hit-col()`
- `terminal-content-width() / terminal-content-height()`

这些属性和 helper 统一驱动：

- resize rows/cols 计算
- pointer hit-testing
- cell rectangle 布局
- cursor 几何
- blank surface 尺寸

## Tokio Task / Channel / Actor 交互关系

- UI -> bootstrap:
  Slint callbacks 进入 Rust 闭包，解析成结构化输入。
- bootstrap -> `SessionManager`:
  通过同步方法转发到当前 active session。
- `SessionManager` -> runtime control:
  调用 `SessionRuntimeControl` trait 方法。
- `SshSessionRuntime`:
  对 SSH channel pump 使用 `mpsc::UnboundedSender<RuntimeCommand>` 下发命令。
- runtime -> manager:
  使用 `mpsc::UnboundedSender<SessionRuntimeEvent>` 回推 `Connected`、`SurfaceChanged`、`Disconnected`、`Error`。
- manager -> view model:
  `sync_workspace_projection_from_manager()` 从 registry 读取 surface/tab 状态。
- view model -> Slint:
  通过 `VecModel` / `ModelRc` 和属性 setter 推送给 `AppWindow`。

当前模型更接近 command/event loop，而不是完全独立 actor：

- `RuntimeCommand` 是 runtime 的命令面。
- `SessionRuntimeEvent` 是 runtime 的事件面。
- `SessionManager` 充当 registry + coordination layer。

## 状态流转说明

### 文本输入

1. `TextInput.edited` 触发 `text-input(string)`
2. bootstrap 调 `send_session_text_input`
3. runtime 写入 SSH channel
4. 远端输出回流到 `apply_remote_bytes()`
5. parser 产出新 terminal state
6. `surface_state()` 投影回 Slint

### 结构化键盘输入

1. Slint `key-pressed` 归一化为 key 名 + modifiers
2. bootstrap 转为 `TerminalKeyEvent`
3. `TerminalSession::send_key_event()` 基于 live terminal mode 编码
4. bytes 写入 SSH channel

### Paste

1. 本地 clipboard 进入 `send_session_paste`
2. `TerminalSession::encode_paste()` 按 `bracketed_paste_enabled` 决定是否包裹
3. payload 写入 SSH channel

### Wheel / Scrollback

1. Slint `scroll-event` 触发 `scroll-requested`
2. 若 `mouse_grabbed = true`，bootstrap 转成 `TerminalMouseInput { kind: Scroll }`
3. 若 `mouse_grabbed = false`，bootstrap 调 `scroll_session_viewport(delta)`
4. `TerminalSession` 更新 `viewport_offset_lines`
5. `visible_rows/cells/cursor` 按新 viewport 重投影

### Theme Toggle

1. UI 触发 `toggle_theme_mode`
2. bootstrap 更新 `ShellViewModel.theme_mode`
3. `SessionManager::set_theme_mode()` 广播给 runtime controls
4. `SessionTerminalConfig` generation 增加
5. `surface_state()` 重新按新 palette 投影
6. Slint canvas 与终端 cell/cursor 背景同步变化

## 关键错误处理策略

- SSH runtime channel 关闭时，返回明确 `anyhow!` 错误，不静默吞掉。
- `SessionManager` 在 runtime 尚未 ready 时：
  - resize 走 `pending_resizes`
  - disconnect 走 `pending_disconnects`
- host key 未知时，使用 typed error `UnknownHostKeyError` 向上层暴露。
- parser 前横幅过滤只做 exact match，避免误吞相似输出。
- theme update 不再使用 runtime 内阻塞等待，避免 Tokio runtime 线程 panic。
- Slint window callbacks 使用 weak handle 路径，降低窗口关闭后的悬挂 UI 更新风险。

## 潜在边缘情况

- Tokio channel 消息堆积
  当前 runtime command/event 都是 `mpsc::UnboundedSender`，不会阻塞发送端，但高频 mouse move / wheel / output burst 可能导致消息堆积与内存增长。
- UI 线程更新时机不正确
  若 theme toggle、session surface polling、window close 同时发生，可能出现旧 surface 短暂覆盖新 surface，需要继续关注投影同步顺序。
- 数据竞争或共享状态不一致
  `TerminalSession` 和 `SessionRegistry` 依赖 `Mutex`；若未来引入更复杂回调，需避免锁嵌套和 poison 传播。
- 资源释放时序问题
  session 在 runtime control attach 前被关闭时，必须继续依赖 `pending_disconnects` 路径，否则会残留 SSH runtime。
- 异步任务取消或界面关闭后的悬挂回调
  bootstrap 闭包若持有过时强引用，可能在窗口已销毁时仍尝试同步 UI；当前已用 weak handle 降低风险，但仍建议补回归测试。
- Slint model 更新与实际数据源不同步
  `workspace-session-cells`、`visible-lines`、`surface-seqno` 必须来自同一份 `TerminalSurfaceState` 快照；未来若拆分增量更新，容易出现 cursor 与 cells 不一致。
- viewport 与新输出竞争
  本地 scrollback 状态下接收新远端输出时，当前实现只做 clamp，不会自动强制跳回底部，后续若 UX 要求变化需明确设计。
- palette override 与动态 OSC 颜色序列交互
  若远端应用主动修改 palette，需继续验证与 `ThemeMode` 切换的优先级关系。

## 后续适合补充的测试建议

### 单元测试

- `TerminalSession` 在 theme toggle 前后保留 ANSI 前景色但更新默认背景。
- `viewport_offset_lines` 在 resize 和新输出后的 clamp 行为。
- `encode_paste()` 对嵌套 bracketed markers 的净化行为。
- `SessionTerminalConfig` generation 仅在 theme 真实变化时递增。

### 集成测试

- `SessionManager::set_theme_mode()` 对已 attach 和延迟 attach runtime 的一致性。
- active session 切换时，surface projection 与 `workspace-session-surface-seqno` 的同步。
- `mouse_grabbed` 在 alternate screen/TUI 模式切换下的 wheel 路由。

### UI 交互测试

- `TerminalSessionHost` metrics helper 变化后，cursor 与 hit-test 仍命中同一 cell。
- `Ctrl+Shift+C/V`、`Shift+Insert`、右键 Paste 的 Slint callback contract。
- theme toggle 后 blank surface、cell 背景、cursor 颜色是否一起刷新。
- selection drag 与 remote mouse forwarding 在 `mouse_grabbed` 切换边界上的行为。
