# SSH Create / Connect / Tabs TDD Handoff

日期: 2026-03-24
范围: `Task 1` 到 `Task 7`
状态: implementation complete, workspace verification passed

## Core Structs

- `ShellViewModel`
  - 持有 asset modal state、workspace tabs、active session id、active terminal surface。
  - 负责把 UI 输入整理成可同步到 Slint 的纯状态。
- `SessionManager`
  - 维护 session registry、open order、asset-to-session reuse mapping、runtime controls、terminal surfaces。
  - 对外暴露 `open_session`、`probe_connection`、`disconnect_session`、`close_session`、`send_session_input`、`resize_session`。
- `SessionHandle`
  - 作为 workspace tab/session projection 的最小业务对象。
  - 字段包括 `session_id`、`asset_id`、`title`、`subtitle`、`state`、`can_reconnect`。
- `TerminalSurfaceState`
  - 当前 terminal host 使用的 renderer snapshot。
  - 字段包括 `session_id`、`seqno`、`rows`、`cols`、`visible_lines`。
- `SshSessionRuntime`
  - 封装 `russh` transport、PTY/shell 协商、runtime command channel、terminal snapshot 更新。
- `TerminalSession`
  - `wezterm-term` 的 thin wrapper。
  - 负责 `apply_remote_bytes`、`send_key_down`、`resize`、`surface_state`。
- `WorkspaceTab`
  - `SessionHandle` 到 Slint tab model 的投影对象。
  - 固化 title/subtitle/state/active/can_reconnect 等显示语义。
- `AssetSshConnectionDraft`
  - `New/Edit SSH Connection` modal 的单页分组表单草稿状态。

## Traits And Contracts

- `SessionRuntimeControl`
  - `disconnect(&self) -> Result<()>`
  - `send_input(&self, bytes: Vec<u8>) -> Result<()>`
  - `resize(&self, rows: u32, cols: u32) -> Result<()>`
  - 约束: 所有 session runtime 必须支持断开、输入转发、窗口大小同步。
- `SessionRuntimeLauncher`
  - `launch(profile, session_id, event_tx) -> LaunchFuture`
  - `probe(profile) -> ProbeFuture`
  - 约束: `launch` 负责创建可长期运行的 runtime control；`probe` 只做验证，不登记 workspace session。

## Slint Callbacks, Global State, Bindings

- `AppWindow`
  - workspace properties
    - `workspace-tab-items`
    - `active-workspace-session-id`
    - `workspace-session-host-mode`
    - `workspace-session-title`
    - `workspace-session-subtitle`
    - `workspace-session-state`
    - `workspace-session-error-detail`
    - `workspace-session-can-reconnect`
    - `workspace-session-visible-lines`
    - `workspace-session-surface-seqno`
  - workspace callbacks
    - `workspace-tab-selected(string)`
    - `workspace-tab-close-requested(string)`
    - `workspace-session-text-input(string)`
    - `workspace-session-key-input(string, bool, bool, bool)`
    - `workspace-session-resize-requested(int, int)`
- `WorkspacePane`
  - 负责把 `AppWindow` 的 workspace properties 传入 `TerminalSessionHost`。
  - 负责把 `TerminalSessionHost` 的 `text-input` / `key-input` / `surface-resize-requested` 回调回抛到 `AppWindow`。
- `TerminalSessionHost`
  - 负责 `welcome` / `terminal` / `session-error` 三态切换。
  - `terminal` 模式下用 `session-visible-lines` 进行 line-model 渲染。
  - 通过隐藏 `TextInput` 捕获 printable text。
  - 通过 `key-pressed(event)` 捕获 named key 和 modifier。
  - 尺寸变化时通过 `surface-resize-requested(rows, cols)` 请求 runtime resize。
- `BlockingModalShell`
  - 重新成为 blocking asset modal 的统一 chrome owner。
  - 统一承载 title、close、drag、frame 和 focus restore 语义。

## Tokio Tasks, Channels, Actor-Like Flow

- launch path
  - `bootstrap` 调用 `SessionManager::open_session`
  - `SessionManager` 创建 `event_tx/event_rx`
  - `SessionManager` spawn runtime event consumer task
  - `SessionManager` spawn launcher task
  - `launcher.launch(...)` 返回 `Box<dyn SessionRuntimeControl>`
- runtime path
  - `SshSessionRuntime` 内部持有 `command_tx`
  - `run_channel_pump(...)` 同时监听
    - SSH channel output
    - runtime command channel
  - command 类型
    - `Input(Vec<u8>)`
    - `Resize { rows, cols }`
    - `Disconnect`
- projection path
  - runtime 发送 `SessionRuntimeEvent`
    - `Connected`
    - `SurfaceChanged(TerminalSurfaceState)`
    - `Disconnected`
    - `Error(String)`
  - `SessionManager` 更新 registry
  - `bootstrap` 的定时 projection timer 把 registry 同步到 `ShellViewModel`
  - `sync_workspace_tabs` 把 view model 同步到 Slint properties

## State Flow

1. 用户在 modal 点击 `Connect` 或 `Save and Connect`
2. `bootstrap` 把 draft 转为 `ConnectionProfile`
3. `SessionManager::open_session` 创建 `SessionHandle`
4. `SessionRuntimeLauncher::launch` 建立 runtime
5. runtime 完成 PTY/shell 协商后发送 `Connected`
6. runtime 把 `wezterm-term` snapshot 转为 `TerminalSurfaceState`
7. `SessionManager` 更新 `terminal_surfaces`
8. `bootstrap` projection timer 把 active session 的 surface 放入 `ShellViewModel`
9. `AppWindow` 更新 `workspace-session-visible-lines` 等 properties
10. `TerminalSessionHost` 渲染当前 surface
11. 用户继续输入文本 / 按键 / resize
12. Slint callback 回到 `bootstrap`
13. `bootstrap` 调用 `SessionManager::send_session_input` / `resize_session`
14. runtime command channel 把输入和 resize 发回 SSH channel

## Key Error Handling Strategy

- modal validation
  - `ShellViewModel` 对 name/host/user/auth payload 进行同步验证。
  - 无效 draft 会禁用 `Connect` / `Test Connection` / `Save and Connect`。
- secret merge
  - edit mode 下 secret 留空表示保留旧值。
  - 显式 clear 才会删除保存的 secret bundle。
- unknown host key
  - 通过 typed error `UnknownHostKeyError` 触发 host key confirm modal。
  - 用户 accept 后写入 known hosts 并重试原操作。
- runtime launch / probe failure
  - probe 失败只更新 modal feedback，不创建 workspace tab。
  - open session 失败会保留 error tab，并允许用户关闭。
- runtime control missing
  - `send_session_input` / `resize_session` 在 runtime 尚未 ready 时返回 error，不静默吞掉。
- channel close
  - runtime command channel 关闭时返回明确错误。
  - SSH channel 关闭时发出 `Disconnected` event，session 转为 reconnectable。

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 当前使用 `mpsc::UnboundedSender`，不会直接背压调用方，但存在高频输入或 surface event 堆积风险。
  - 后续可以在 runtime 或 projection 层加入 coalescing / throttling。
- UI 线程更新时机不正确
  - 当前依赖 `bootstrap` 的 projection timer，把后台 runtime event 安全地投影到 Slint state。
  - 如果未来减少 timer，需要继续保证 UI 更新发生在 Slint 线程安全边界内。
- 数据竞争或共享状态不一致
  - `SessionManager` 使用 `Arc<Mutex<SessionRegistry>>` 统一保护 session、runtime control、surface registry。
  - 风险点在于长时间持锁调用外部逻辑；当前 `disconnect` / `send_input` / `resize` 已避免在锁内做复杂工作。
- 资源释放时序问题
  - `close_session` 和 `disconnect_session` 都会尝试移除 runtime control。
  - runtime 迟到注册时使用 `pending_disconnects` 避免 orphan runtime 悬挂。
- 异步任务取消或界面关闭后的悬挂回调
  - `window.as_weak()` 避免 UI 对象被后台闭包强持有。
  - runtime projection timer 在窗口释放后不会继续更新 UI。
- Slint model 更新与实际数据源不同步
  - `workspace-session-visible-lines` 完全来源于 active session 的 `TerminalSurfaceState`。
  - 切换 tab 后会重新从 `SessionManager` 拉取 active surface，避免旧 tab 内容残留。
- connecting 状态下先发生 resize/input
  - 可能早于首轮 surface ready。
  - 当前 contract 允许调用，但 runtime 若未 attach control 会返回 error；UI 不会伪造 ready 状态。
- temporary connect 与 saved asset connect 混用
  - 临时连接使用 `session:` 前缀 asset id，避免污染已保存资产和复用逻辑。

## Recommended Tests

- unit tests
  - `encode_named_key_input` 对 `Enter`、方向键、`Backspace`、modifier 组合的编码测试。
  - `TerminalSession::resize` 后 `surface_state.rows/cols` 投影测试。
  - `SessionManager::send_session_input` / `resize_session` 在 missing runtime control 场景下的错误测试。
- integration tests
  - active tab 切换后，输入只会发往当前 active session。
  - runtime disconnect 后，再触发输入/resize 时的 UI 和 error 行为。
  - `Save and Connect` 在 host key confirm、probe success、runtime open 三阶段下的完整状态流测试。
- UI interaction tests
  - `TerminalSessionHost` focus 后 printable text、named key、resize 回调顺序测试。
  - `New Folder` / `New SSH` / `Edit SSH` modal 的 drag/close/footer geometry snapshot 测试。
  - failed SSH tab 的 close hit target 与 normal tab 一致性测试。

## Residual Risk Boundary

- 当前 terminal host 已具备 input / key / resize / visible line update contract，但仍是 line-model host，不是完整 glyph/cursor renderer。
- 当前 resize 行数列数为 UI 估算值，足以建立 contract，但后续若引入真实 terminal renderer，应改为更精确的 cell metrics。
- 当前 projection 仍基于 timer，同步时延可接受，但不是最终低延迟架构。
