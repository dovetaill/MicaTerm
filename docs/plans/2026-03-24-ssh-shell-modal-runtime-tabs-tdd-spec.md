# SSH Shell Modal Runtime Tabs TDD Spec

日期: 2026-03-24
阶段: implementation complete -> TDD handoff
状态: Task 1 到 Task 10 已完成；`cargo test`、`cargo check --workspace`、`cargo clippy --workspace -- -D warnings` 均通过

## 范围摘要

本轮实现完成了 `SSH shell modal / runtime / tabs` 的首轮闭环：

- `SSH modal` 四个动作已按设计分流
- `ConnectionProfile`、secret、`known_hosts`、真实 SSH runtime 已接通
- `SessionManager`、`TerminalSurfaceState`、`WorkspaceTab`、`ShellViewModel` 已形成 session 到 UI 的稳定投影
- `TerminalSessionHost` 已消费 terminal snapshot，不再停留在 placeholder host
- 同资产默认复用 session，`Open in New Tab` 明确创建第二个 session
- `Disconnected / Error` tab 保留，close tab 才释放 session

当前仍需明确的边界：

- Win11 真机视觉与交互体验仍需人工最终确认
- terminal renderer 当前是 text snapshot projection，不是高性能增量渲染器
- `Reconnect` 还没有独立 command/callback，只保留状态与可重连语义
- 内存基线已从结构上减少重复 runtime，但仍需人工量测

## 核心 Struct

### SSH domain / runtime

- `src/app/ssh/profile.rs`
  - `ConnectionProfile`
  - `SshAuthMethod`

- `src/app/ssh/known_hosts.rs`
  - `KnownHostsService`
  - `KnownHostCheck`

- `src/app/ssh/runtime.rs`
  - `SshSessionRuntime`
  - `TerminalSession`
  - `TerminalSurfaceState`
  - `SessionRuntimeEvent`

- `src/app/ssh/session_manager.rs`
  - `SessionManager`
  - `SessionHandle`
  - `SessionState`
  - `OpenSessionMode`

### Shell / UI projection

- `src/shell/tabs.rs`
  - `WorkspaceTab`

- `src/shell/view_model.rs`
  - `ShellViewModel`
  - `AssetModalState`
  - `AssetSshConnectionDraft`
  - `SshModalAction`
  - `PendingSshModalAction`
  - `SshModalActionState`
  - `SshHostKeyPromptState`

- `src/app/bootstrap.rs`
  - `ShellSessionBridge`
  - `LiveSessionRuntimeLauncher`

## Trait 与接口契约

### Secret / profile / host key

- `ConnectionProfile::from_draft(&AssetSshConnectionDraft) -> Result<ConnectionProfile>`
  - 负责把 modal draft 归一化为 runtime 可消费 profile
  - 支持 `password`、`private-key(path)`、`private-key(content)`

- `ConnectionProfile::from_saved_asset(asset_id, title, spec) -> Result<ConnectionProfile>`
  - 负责把 persisted asset 恢复为可重连 profile

- `CredentialStore`
  - `put_secret(&self, key: &str, value: &str) -> Result<()>`
  - `get_secret(&self, key: &str) -> Result<Option<String>>`
  - `delete_secret(&self, key: &str) -> Result<()>`

- `KnownHostsService`
  - `check(&self, host: &str, port: u16, key: &PublicKey) -> Result<KnownHostCheck>`
  - `accept_unknown(&self, host: &str, port: u16, key: &PublicKey) -> Result<()>`
  - `ensure_trusted(&self, host: &str, port: u16, key: &PublicKey) -> Result<()>`

### Session lifecycle

- `SessionRuntimeLauncher`
  - `launch(profile, session_id, event_tx) -> Future<Result<()>>`
  - `probe(profile) -> Future<Result<()>>`

- `SessionManager`
  - `open_session(profile, mode) -> Result<SessionHandle>`
  - `probe_connection(profile) -> Result<()>`
  - `session(session_id) -> Option<SessionHandle>`
  - `ordered_sessions() -> Vec<SessionHandle>`
  - `terminal_surface(session_id) -> Option<TerminalSurfaceState>`
  - `disconnect_session(session_id) -> Option<SessionHandle>`
  - `close_session(session_id) -> Option<SessionHandle>`

约束：

- `ActivateExisting` 默认按 `asset_id` 复用 session
- `ForceNewTab` 为同一 asset 创建第二个 session
- 临时 `Connect` 使用 `session:<uuid>` 风格 asset id，避免错误复用旧 draft session
- `probe_connection()` 不得注册 workspace session

### ViewModel / bootstrap 边界

- `ShellViewModel::begin_ssh_modal_action(action_id) -> bool`
  - 只负责校验与记录 submit intent
  - 不直接触发 SSH runtime

- `ShellViewModel::confirm_asset_modal() -> bool`
  - `Save` / `SaveAndConnect` 的持久化入口

- `bootstrap::open_session_with_profile(...)`
  - 负责把 profile 启动成 session，并合并为 tab/state 投影

- `bootstrap::sync_workspace_projection_from_manager(...)`
  - 负责把 `SessionManager` registry + surface snapshot 拉平为 `ShellViewModel` 当前可见态

## Slint Callbacks / Global State / Bindings

### AppWindow 关键 properties

- `workspace-tab-items`
- `active-workspace-session-id`
- `workspace-session-host-mode`
- `workspace-session-title`
- `workspace-session-subtitle`
- `workspace-session-state`
- `workspace-session-can-reconnect`
- `workspace-session-screen-text`
- `workspace-session-surface-seqno`
- `asset-ssh-modal-*`
- `ssh-host-key-modal-*`

### AppWindow callbacks

- `asset-ssh-modal-action-requested(string)`
- `asset-ssh-modal-draft-changed(string, string)`
- `asset-ssh-modal-tab-selected(string)`
- `workspace-tab-selected(string)`
- `workspace-tab-close-requested(string)`
- `asset-selected(string)`
- `asset-context-menu-requested(string, string, length, length)`
- `assets-context-menu-action-invoked(string)`
- `ssh-host-key-modal-accept-requested()`
- `ssh-host-key-modal-reject-requested()`
- `blocking-modal-drag-requested(length, length)`
- `blocking-modal-drag-moved(length, length)`
- `blocking-modal-drag-ended()`

### Slint 组件职责

- `ui/shell/workspace-pane.slint`
  - 统一拥有 tab strip 和 terminal host 的宽高契约

- `ui/shell/terminal-session-host.slint`
  - 只消费 snapshot 文本与 session state
  - 不接触 `russh` transport

- `ui/shell/tabbar.slint`
  - 只消费 `WorkspaceTabItem`
  - 通过状态字符串驱动 `connecting / connected / disconnected / error`

## Tokio Task / Channel / Actor 相关交互

### Runtime task

- `AppAsyncRuntime`
  - 当前作为共享 Tokio runtime，供 session bridge 复用

- `SshSessionRuntime::connect(...)`
  - 建立 `russh` client 连接
  - 完成 auth / channel / PTY / shell
  - 创建 `command_tx`
  - `tokio::spawn(run_channel_pump(...))`

### Channel

- `mpsc::UnboundedSender<SessionRuntimeEvent>`
  - runtime -> session manager
  - 传递 `Connected / SurfaceChanged / Disconnected / Error`

- `mpsc::UnboundedSender<RuntimeCommand>`
  - UI/holder -> runtime pump
  - 传递 `Input / Resize / Disconnect`

### UI thread 切换

- `bootstrap` 不从 Tokio worker 线程直接写 Slint window
- `SessionManager` 只维护 registry
- `bootstrap` 通过 Slint `Timer` 在 UI 线程周期性拉取 manager 状态并同步 window properties
- 延迟聚焦逻辑仍通过 `slint::invoke_from_event_loop(...)` 回到 UI 线程

## 状态流转说明

### Modal action

- `TestConnection`
  - `draft -> ConnectionProfile -> SessionManager::probe_connection()`
  - 成功只更新 modal feedback
  - 不创建 tab，不保存 asset

- `Connect`
  - `draft -> ConnectionProfile`
  - 注入临时 `asset_id = session:<uuid>`
  - 打开临时 session tab
  - 不保存 asset

- `Save`
  - `draft -> confirm_asset_modal()`
  - 只更新 asset tree / catalog / secret store
  - 不打开 tab

- `SaveAndConnect`
  - `draft -> confirm_asset_modal()`
  - 再通过保存后的 asset profile 打开 session

### Session state

- `Connecting`
  - `open_session()` 初始状态

- `Connected`
  - 仅在 SSH handshake/auth/channel/pty/shell 全部成功后进入

- `Disconnected`
  - runtime 断开或手动 disconnect 后进入
  - tab 保留，可重连

- `Error(String)`
  - probe / auth / transport / channel 路径出错后进入
  - tab 保留，可重连

### Workspace fallback

- close active tab 时：
  - 优先右侧
  - 再左侧
  - 都没有则回到 `welcome`

## 关键错误处理策略

- host key 未信任：
  - `KnownHostsService::ensure_trusted()` 直接返回错误
  - 不允许静默跳过 TOFU

- host key 变更：
  - 直接阻断连接

- auth secret 缺失或格式错误：
  - 在 runtime auth 阶段明确返回错误

- runtime launch/probe 失败：
  - `SessionManager` 将 session 标记为 `Error`
  - `probe_connection()` 不污染 registry

- PTY / shell 请求失败：
  - 不发送 `Connected`
  - 直接转为错误路径

- UI sync：
  - 只在 `sync_workspace_projection_from_manager()` 比较出差异后更新，避免无意义刷新

## 潜在边缘情况

### Tokio channel 阻塞或消息堆积

- 当前 `SessionRuntimeEvent` 与 `RuntimeCommand` 使用 unbounded channel
- 在首轮实现里能保证功能正确，但高吞吐输出下仍可能积压
- 下一轮若引入高频增量渲染，建议：
  - surface update 做 coalesce
  - 或切换为 bounded channel + drop/merge 策略

### UI 线程更新时机不正确

- 当前通过 Slint `Timer` 在 UI 线程拉取 manager 状态
- 避免了后台线程直接写 Slint property
- 若下一轮切到 push 模型，必须继续保证 `invoke_from_event_loop` 或等效 UI-thread handoff

### 数据竞争或共享状态不一致

- `SessionManager` registry 使用 `Mutex`
- terminal surface 以 clone snapshot 对外暴露
- `ShellViewModel` 只保存 active session surface，避免 transport 对象泄漏进 UI

### 资源释放时序问题

- `close_session()` 会同时移除 registry、open order、terminal surface
- `probe()` 完成后显式 `disconnect()`
- window 关闭后，timer closure 会通过 `handle.upgrade()` 失败而自然停止写 UI

### 异步任务取消或界面关闭后的悬挂回调

- runtime pump 依赖 command/event channel 和 SSH channel 生命周期结束
- UI 侧使用 `window.as_weak()` / `upgrade()` 防止已销毁窗口被回写

### Slint model 更新与实际数据源不同步

- `sync_workspace_projection_from_manager()` 同时比较 tab 列表与 active surface
- active tab 切换时会重新解析 manager 中对应 session 的 surface
- close/disconnect 后会同步清理 surface 或重算 active tab

## 后续适合编写的测试建议

### 单元测试

- `SessionManager` surface coalesce / replace 规则
- `temporary_session_asset_id_for_draft()` 的唯一性约束
- `probe_connection()` 在更多失败场景下不污染 registry

### 集成测试

- 真实 `russh` test server 下的 password auth probe
- host key 首次拒绝 / 接受后重试 / changed key 阻断
- `Disconnect -> reconnect` 的完整状态回流

### UI 交互测试

- Win11 真机 modal drag / focus restore / click-away blocking
- 多 tab 下 terminal host 的 surface 切换
- `Disconnected / Error` tab 上的 reconnect CTA 行为

### 性能 / 稳定性测试

- 高频输出下 `SessionRuntimeEvent::SurfaceChanged` 的刷新频率
- 多 session 并发打开时的 UI 卡顿与 registry 一致性
- Win11 常驻内存和多 session 增量内存基线
