# Terminal Interaction Polish Follow-up TDD Spec

日期: 2026-03-27
方案名: `terminal-interaction-polish-followup`
状态: 已完成实现后的测试与交接规格

## 核心 Struct

- `TerminalSurfaceState`
  作为 runtime 到 UI 的稳定终端快照，当前包含 `default_fg_rgba`、`default_bg_rgba`、viewport offset、visible rows、cells、cursor、`mouse_grabbed`、`bracketed_paste_enabled`。
- `TerminalSurfaceSignature`
  作为轻量签名，用于比较 surface 是否发生了需要重投影的关键变化，补充了默认前景/背景和 viewport 元数据。
- `TerminalSession`
  封装 `wezterm-term`，负责 palette、cursor、cells、visible rows、local scrollback、keyboard encoding、mouse encoding 与 paste encoding。
- `SshSessionRuntime`
  封装 SSH channel 生命周期和 `RuntimeCommand` 命令入口，通过内部 `TerminalSession` 生成 `SessionRuntimeEvent::SurfaceChanged`。
- `SessionManager`
  负责 session registry、runtime control 挂载、surface 缓存、`pending_resizes` / `pending_disconnects`、theme mode 广播和本地 viewport 滚动。
- `ShellViewModel`
  作为 Slint shell 的聚合状态源，持有 `active_workspace_terminal_surface` 与当前 active tab/session 的投影结果。
- `ShellSessionBridge`
  作为 bootstrap 对 `SessionManager` 的轻量转发包装，负责把 Slint 回调转成 session 级调用。

## Trait 与接口契约

- `SessionRuntimeControl`
  当前与 follow-up 直接相关的稳定接口包括：
  - `send_text_input(text: String)`
  - `send_key_input(event: TerminalKeyEvent)`
  - `send_mouse_input(event: TerminalMouseInput)`
  - `send_paste(text: String)`
  - `resize(rows: u32, cols: u32)`
  - `update_theme_mode(mode: ThemeMode) -> Result<Option<TerminalSurfaceState>>`
  - `scroll_viewport_lines(delta: i32) -> Result<TerminalSurfaceState>`
  - `disconnect()`
- `SessionRuntimeLauncher`
  负责异步创建 runtime control，并把 `SessionRuntimeEvent` 绑定回 `SessionManager` 的事件侧。

## Slint Callbacks / Global State / Bindings

### 关键回调

- `workspace-session-text-input(string)`
- `workspace-session-key-input(string, bool, bool, bool)`
- `workspace-session-resize-requested(int, int)`
- `workspace-session-copy-selection-requested(int, int, int, int)`
- `workspace-session-paste-requested()`
- `workspace-session-scroll-requested(int, int, int, bool, bool, bool)`
- `workspace-session-scroll-thumb-drag-requested(float)`
- `workspace-session-scroll-jump-requested(float)`
- `workspace-session-mouse-input(string, string, int, int, bool, bool, bool)`

### 关键状态投影

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
- `workspace-session-default-fg`
- `workspace-session-default-bg`
- `workspace-session-mouse-grabbed`
- `workspace-session-visible-lines`
- `workspace-session-viewport-offset-lines`
- `workspace-session-viewport-max-offset-lines`
- `workspace-session-viewport-at-bottom`
- `workspace-session-surface-seqno`

### Binding 链路

- `src/app/bootstrap.rs` 中 `sync_workspace_session_state()` 把 `TerminalSurfaceState` 投影到 `AppWindow`。
- [ui/app-window.slint](/home/wwwroot/mica-term/.worktrees/terminal-interaction-polish-followup/ui/app-window.slint) 将 terminal properties 和 callbacks 继续透传给 [ui/shell/workspace-pane.slint](/home/wwwroot/mica-term/.worktrees/terminal-interaction-polish-followup/ui/shell/workspace-pane.slint)。
- [ui/shell/workspace-pane.slint](/home/wwwroot/mica-term/.worktrees/terminal-interaction-polish-followup/ui/shell/workspace-pane.slint) 再把它们透传给 [ui/shell/terminal-session-host.slint](/home/wwwroot/mica-term/.worktrees/terminal-interaction-polish-followup/ui/shell/terminal-session-host.slint)。
- `workspace-session-cells` 与 `workspace-session-visible-lines` 使用 `VecModel` / `ModelRc` 在 bootstrap 中做增量同步，避免每次全量替换造成 UI 抖动。

### 终端 Host 视觉契约

- `terminal-font-family`
- `terminal-font-size`
- `terminal-cell-width`
- `terminal-cell-height`
- `terminal-cursor-thickness`
- `terminal-padding-left/top/right/bottom`
- `terminal-cell-x()` / `terminal-cell-y()`
- `terminal-hit-row()` / `terminal-hit-col()`
- `scrollbar-thumb-drag-requested()` / `scroll-jump-requested()`

### Bundled Font 契约

- `ui/app-window.slint` 通过 `import "fonts/IosevkaTerm-Regular.ttf";` 将 `Iosevka Term` 纳入 Slint 编译输入。
- `build.rs` 通过 `cargo:rerun-if-changed=ui/fonts/IosevkaTerm-Regular.ttf` 保证字体变更触发重编译。
- `TerminalSessionHost` 默认字体栈以 `Iosevka Term` 为 primary face，仅保留少量 fallback。

## Tokio Task / Channel / Actor 交互关系

- UI -> bootstrap
  Slint callbacks 进入 Rust 闭包，先解析为结构化输入。
- bootstrap -> `SessionManager`
  通过 `ShellSessionBridge` 调用 active session 的同步接口。
- `SessionManager` -> runtime control
  调用 `SessionRuntimeControl` trait，把文本、按键、paste、resize、mouse、scroll 请求下发到 runtime。
- `SshSessionRuntime`
  内部用 `tokio::sync::mpsc::UnboundedSender<RuntimeCommand>` 作为命令面。
- SSH channel pump
  `run_channel_pump(...)` 在 Tokio task 中用 `tokio::select!` 同时处理 `RuntimeCommand` 与远端 channel 数据。
- runtime -> manager
  用 `mpsc::UnboundedSender<SessionRuntimeEvent>` 推送 `Connected`、`SurfaceChanged`、`Disconnected`、`Error`。
- manager event loop
  `SessionManager::open_session()` 会 spawn 事件消费任务，并对 `SurfaceChanged` backlog 做 coalesce，减少 UI 投影抖动。
- manager -> view model -> Slint
  bootstrap 从 manager 读取当前 registry / surface，更新 `ShellViewModel`，再通过 `VecModel` / property setter 推给 Slint。

当前模型是 command/event 分离的协调层，而不是完全独立 actor：

- `RuntimeCommand` 是下行命令面。
- `SessionRuntimeEvent` 是上行事件面。
- `SessionManager` 充当 registry + synchronization coordinator。

## 状态流转说明

### 1. 终端画布 palette 投影

1. runtime 在 `TerminalSession::surface_state()` 中读取 `wezterm-term` palette。
2. `default_fg_rgba` / `default_bg_rgba` 随 `TerminalSurfaceState` 一起上送。
3. bootstrap 的 `sync_workspace_session_state()` 把默认前景/背景写入 `AppWindow`。
4. `AppWindow -> WorkspacePane -> TerminalSessionHost` 逐层传递。
5. `TerminalSessionHost` 用 `session-default-bg` 驱动 `surface-frame` 与 `blank-surface`，保证空白画布与 cell 背景属于同一 palette。

### 2. 保留快捷键与远端输入分流

1. `TerminalSessionHost.key-pressed(event)` 先吞掉 bare `Ctrl` / `Shift` / `Alt`。
2. 已实现的本地动作优先处理：
   - `Ctrl+Shift+C`
   - `Ctrl+Shift+V`
   - `Ctrl+Insert`
   - `Shift+Insert`
   - `Shift+PageUp`
   - `Shift+PageDown`
   - `Ctrl+Shift+Home`
   - `Ctrl+Shift+End`
3. 保留但未实现动作的 `Ctrl+Shift+<letter>` 直接吞掉，不进入远端编码层。
4. 只有允许透传的组合才走 `workspace-session-key-input(...)` -> `SessionManager::send_session_key_input(...)` -> `TerminalSession::send_key_event(...)`。

### 3. Wheel accumulation 与本地 scrollback

1. `scroll-event` 把 `event.delta-y` 累加到 `wheel-delta-remainder`。
2. 累积值跨过 `wheel-delta-threshold` 后，转换成固定多行 `delta_lines`。
3. 若 `session-mouse-grabbed = true`，bootstrap 把滚轮作为 `TerminalMouseInput` 透传给远端 TUI。
4. 若 `session-mouse-grabbed = false`，bootstrap 调 `SessionManager::scroll_session_viewport(...)` 更新本地 viewport。
5. 新的 `viewport_offset_lines`、`viewport_max_offset_lines`、cells、visible rows/cursor 再次投影回 Slint。

### 4. Theme mode 切换

1. UI 切换 `ThemeMode`。
2. `SessionManager::set_theme_mode(mode)` 遍历所有 attached runtime。
3. runtime 调 `update_theme_mode(mode)` 重新生成带新 palette 的 `TerminalSurfaceState`。
4. active workspace surface 重新同步，blank canvas、默认前景/背景和 cell/cursor 一起更新。

### 5. 字体与 metrics 生效

1. Slint 编译链加载 bundled `Iosevka Term`。
2. `TerminalSessionHost` 默认 `terminal-font-family` 选用该字体。
3. `terminal-font-size`、`terminal-cell-width`、`terminal-cell-height` 使用更紧凑的 IDE-like 默认值。
4. 同一套 metrics helper 统一驱动 hit-testing、grid geometry、cursor 几何、surface resize 推导。

## 关键错误处理策略

- runtime 尚未 ready 时，`SessionManager` 对输入调用返回明确错误；对 resize / disconnect 则分别走 `pending_resizes` / `pending_disconnects` 缓冲路径。
- bootstrap 解析未知 mouse kind / button 时只记录 warning，不把非法值送入 runtime。
- `TerminalSession::send_key_event()` 对不支持的 named key 返回显式错误，避免静默编码错误。
- `SshSessionRuntime` 在 key / mouse / paste 编码失败，或向 SSH channel 写入失败时，通过 `SessionRuntimeEvent::Error` 向上游报告。
- UI 回调中普遍使用 weak window handle，窗口销毁后不会强行升级为无效强引用。
- surface 投影基于单一 `TerminalSurfaceState` 快照，避免 cursor / cells / viewport 来自不同时间点。

## 潜在边缘情况（Edge Cases）

- Tokio channel 阻塞或消息堆积
  当前 command/event 两侧都使用 `mpsc::UnboundedSender`，不会反向施加背压，但高频输出、鼠标移动或滚轮事件可能导致消息堆积与内存增长。`SurfaceChanged` 事件已有 backlog coalesce，命令侧仍需持续观察。
- UI 线程更新时机不正确
  快速切换 active tab、theme toggle、scrollback 和窗口关闭同时发生时，可能出现旧 surface 短暂覆盖新 surface 的时序问题。
- 数据竞争或共享状态不一致
  `TerminalSession` 与 `SessionRegistry` 都依赖 `Mutex`；未来若增加更多跨线程回调，需避免锁嵌套、poison 传播和旧 surface 覆盖新 surface。
- 资源释放时序问题
  session 在 runtime control attach 前被关闭时，必须依赖 `pending_disconnects` 补发 disconnect，否则可能残留 SSH channel pump。
- 异步任务取消或界面关闭后的悬挂回调
  若 Tokio 任务晚于窗口销毁才回推 surface，必须继续依赖 weak handle / UI 线程切换策略避免悬挂更新。
- Slint model 更新与实际数据源不同步
  `workspace-session-cells`、`visible-lines`、cursor、viewport 和 `surface-seqno` 必须来自同一份 `TerminalSurfaceState`；后续若拆分为增量更新，更容易失步。
- wheel remainder 在不同输入设备之间切换
  触控板与离散鼠标滚轮混用时，`wheel-delta-remainder` 的遗留值可能影响下一次滚动手感。
- `mouse_grabbed` 状态切换边界
  TUI 程序切换 alternate screen 或 mouse tracking 时，滚轮事件可能在本地 scrollback 和远端 mouse input 之间切换，需要继续核对边界时机。
- 字体 fallback 与 glyph coverage
  `Iosevka Term` 作为 primary face 后，非 ASCII / box drawing / CJK 混排是否回退到预期 fallback，仍需要 Windows 11 实机观察。

## 后续适合编写的测试建议

### 单元测试

- `TerminalSurfaceState::signature()` 在 palette、viewport、cursor 变化时的最小必要变更集合。
- `TerminalSession::scroll_viewport_lines()` 在 resize、新输出和手动 jump 组合下的 clamp 逻辑。
- `TerminalSession::send_key_event()` 对保留键、功能键、带 modifier 字符的编码边界。
- `SessionManager::scroll_session_to_ratio()` 在极端 ratio 与大 viewport offset 下的取整行为。

### 集成测试

- theme toggle 发生在本地 scrollback 非底部时，surface palette 与 viewport 是否一起保持一致。
- runtime attach 晚于 resize / disconnect / theme update 时，`pending_resizes`、`pending_disconnects` 与 theme 同步是否正确回放。
- `mouse_grabbed` 在远端程序切换 mouse tracking 前后，wheel 路由是否稳定。
- surface backlog 高频更新下，coalesce 后的最后一帧是否仍与 registry 一致。

### UI 交互测试

- bare `Ctrl` / `Shift` / `Alt` 不触发远端输入，也不污染 selection / context menu 状态。
- `Ctrl+Shift+<letter>` 保留快捷键白名单与吞掉逻辑在 Slint 合同层保持稳定。
- wheel accumulation 在小 delta、多次 delta 和大 delta 下都产生预期多行滚动。
- scrollbar thumb drag / jump 与 viewport ratio 的映射精度。
- bundled `Iosevka Term` 加载后，cursor、selection 与 cell geometry 在 light / dark mode 下持续对齐。
