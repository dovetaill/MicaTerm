# Bootstrap ViewModel Runtime Refactor TDD Handoff

Date: 2026-04-04

## Scope

本轮实现完成了 `bootstrap` / `view_model` / `ssh::runtime` 三个超大根文件的结构化收窄，目标不是改业务，而是把“稳定入口”和“域内实现”重新分层：

- `src/app/bootstrap.rs`
  - 保持 `bind_top_status_bar_with_*`、`run*` 等稳定入口
  - 内部职责下沉到：
    - `src/app/bootstrap/assets_keychain.rs`
    - `src/app/bootstrap/workspace_terminal.rs`
    - `src/app/bootstrap/sftp.rs`
    - `src/app/bootstrap/shell_chrome.rs`
    - `src/app/bootstrap/windowing.rs`
    - `src/app/bootstrap/vault_sync.rs`
- `src/shell/view_model.rs`
  - 保持 `ShellViewModel` 作为唯一 owner
  - 行为按域拆到：
    - `projection.rs`
    - `workspace.rs`
    - `quick_launch.rs`
    - `assets.rs`
    - `keychain.rs`
    - `sftp.rs`
    - `validation.rs`
    - `ssh_modal.rs`
    - `asset_modal_executor.rs`
    - `context_menu_dispatcher.rs`
- `src/app/ssh/runtime.rs`
  - 保持 `crate::app::ssh::runtime` 作为稳定 facade
  - 内部职责拆到：
    - `contracts.rs`
    - `transport.rs`
    - `auth.rs`
    - `terminal.rs`
    - `pump.rs`
    - `sftp_backend.rs`

本轮最后收尾还额外增加了 `tests/refactor_root_thinness_smoke.sh`，用于约束三大 root 文件继续维持 thin facade 形态。

## Architecture Boundaries

### `bootstrap` 边界

- `src/app/bootstrap.rs`
  - 负责应用启动装配、依赖注入、Slint callback 注册总入口、跨域 orchestrator glue
- `src/app/bootstrap/assets_keychain.rs`
  - 负责 assets / keychain / context menu / asset modal 的 UI 同步与 callback 注册
- `src/app/bootstrap/workspace_terminal.rs`
  - 负责 workspace tabs、terminal 输入输出、滚动、复制粘贴、native terminal surface projection
- `src/app/bootstrap/sftp.rs`
  - 负责 right panel、SFTP browser request、remote file editor、transfer queue 绑定
- `src/app/bootstrap/shell_chrome.rs`
  - 负责 title bar、theme、window chrome、layout 顶层投影
- `src/app/bootstrap/windowing.rs`
  - 负责 window 生命周期、Windows placement 跟踪、sync modal / host key modal / paste warning modal
- `src/app/bootstrap/vault_sync.rs`
  - 负责 vault sync 编排辅助逻辑与后台结果投影

### `ShellViewModel` 边界

- `src/shell/view_model.rs`
  - 只保留共享状态定义、公共类型、少量全局 helper
- 子模块只扩展 `impl ShellViewModel`
  - 不引入第二个 owner
  - 不改变对外调用路径
  - 只按域拆行为，不复制状态

### `ssh::runtime` 边界

- `src/app/ssh/runtime.rs`
  - 保留 `SshSessionRuntime` facade、公共 re-export、最薄的 connect flow glue
- `contracts.rs`
  - 承载 terminal DTO / input contract / surface signature
- `transport.rs`
  - 承载 direct / SOCKS5 / HTTP CONNECT / SSH proxy chain transport 建连
- `auth.rs`
  - 承载 known-hosts 校验、认证、progress reporter
- `terminal.rs`
  - 承载 terminal engine、surface projection、input encoding、env negotiation
- `pump.rs`
  - 承载 channel pump、surface dirty coalescing、working set trim
- `sftp_backend.rs`
  - 承载 `russh-sftp` 到 `SftpBackend` 的适配

## Core Structs

- `ShellViewModel`
  - UI 全局状态唯一 owner；集中承载 assets、keychain、workspace、quick launch、SFTP、modal、context menu、vault panel、window placement
- `ShellSessionBridge`
  - `bootstrap` 里对 `SessionManager` 的轻量桥接；让 binder 层不直接散落 session runtime 细节
- `SessionManager`
  - SSH session registry + actor-like coordinator；维护 session handle、connection attempt、terminal surface、cwd、SFTP binding
- `SshSessionRuntime`
  - 单个 SSH 会话的 runtime facade；暴露 text/key/mouse/paste/resize/disconnect/surface/sftp 能力
- `TerminalSession`
  - 本地 terminal engine owner；负责 remote bytes、surface projection、keyboard/mouse/paste encoding、viewport scrollback
- `TransportChainGuard`
  - SSH proxy chain 保活 guard；确保上游 `client::Handle` 在下游 channel pump 生命周期内不提前释放
- `ConnectionProgressReporter`
  - 连接进度事件构造器；把 resolve/connect/auth/open-session/request-shell 等阶段投递成 `ConnectionProgressEvent`
- `UnknownHostKeyError`
  - known-hosts 的显式错误载体；不是普通字符串错误，而是可以继续驱动 host-key prompt 的结构化错误
- `RusshSftpBackend`
  - `SftpBackend` 的 `russh-sftp` 实现；把 session-bound handle 暴露给 right panel / remote file editor
- `SftpRuntimeHandle`
  - SFTP 会话句柄；给 UI/binder 层提供 `read_dir`、`rename`、`upload_file`、`download_file`、`delete_*` 等异步接口
- `WorkspaceFollowTracker`
  - workspace 视图跟随态辅助器；避免投影刷新时破坏用户当前滚动/选择体验
- `DeferredWorkspaceProjectionRefreshGate`
  - workspace projection refresh 去重门；防止滚动和 surface 更新造成重复刷新
- `DeferredWorkspaceScrollThumbDrag`
  - 滚动条拖拽节流状态；缓存最新 ratio，避免 pointer 高频事件直接冲击 runtime
- `VaultSessionState`
  - vault runtime 会话态；被 `bootstrap` 持有并通过 `vault_sync.rs` 投影回 `ShellViewModel`

## Traits And Interface Contracts

### Runtime Contracts

- `SessionRuntimeControl`
  - 单会话控制接口
  - 合同要求：
    - `disconnect`
    - `send_text_input`
    - `send_key_input`
    - `send_mouse_input`
    - `send_paste`
    - `resize`
    - `terminal_surface`
    - `update_theme_mode`
    - `scroll_viewport_lines`
    - `sftp_runtime`
- `SessionRuntimeLauncher`
  - session manager 启动 runtime 的抽象边界
  - 合同要求：
    - `launch(profile, session_id, attempt_id, event_tx)`
    - `probe(profile)`

### SFTP Contracts

- `SftpBackend`
  - session-bound SFTP 能力抽象
  - 合同要求：
    - `read_dir`
    - `mkdir`
    - `rename`
    - `path_exists`
    - `upload_file`
    - `download_file`
    - `remove_file`
    - `remove_dir`
- `SftpRuntimeHandle`
  - UI/binder 只依赖 handle，不直接依赖 `russh_sftp::client::SftpSession`

### Bootstrap Injection Contracts

- `CredentialStore`
  - SSH modal hydration、saved secret lookup、vault secret restore 共用
- `PlatformWindowEffects`
  - shell chrome 只依赖窗口效果抽象，不直接写平台特定逻辑
- `PrivateKeyImporter`
  - asset/keychain modal 的私钥导入边界
- `VaultProviderFactory` / `VaultProvider`
  - 由 `bind_top_status_bar_with_injected_services_and_vault_runtime` 注入，不在 root binder 里硬编码 provider 细节

### Root Facade Stability Contract

- `src/app/bootstrap.rs`
  - 必须继续保留 `bind_top_status_bar_with_*` 与 `run*` 入口
- `src/shell/view_model.rs`
  - 必须继续保留 `ShellViewModel` root owner
- `src/app/ssh/runtime.rs`
  - 必须继续保留 `SshSessionRuntime` 与公共 terminal contracts / `TerminalSession` / `UnknownHostKeyError` 的稳定导出

## Slint Callbacks, Global State, Bindings

### Root Binding Entry

- `bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge`
  - 当前 composition root
  - 负责：
    - 加载持久化 state
    - 初始化 `ShellViewModel`
    - 装配 vault/session/sftp/controller/runtime
    - 注册各域 Slint callback
    - 启动 Timer 与后台回流通道

### Global State Ownership

- UI 线程共享状态以 `Rc<RefCell<...>>` 持有：
  - `Rc<RefCell<ShellViewModel>>`
  - `Rc<RefCell<WorkspaceFollowTracker>>`
  - `Rc<RefCell<SftpBrowserController>>`
  - `Rc<RefCell<VaultSessionState>>`
  - `Rc<RefCell<Option<PendingHostKeyApproval>>>`
  - `Rc<RefCell<Option<PendingWorkspacePasteWarning>>>`
  - 以及其他短生命周期 UI 协调状态

### Binding Split

- `assets_keychain::bind_assets_keychain_callbacks`
  - 资产搜索、create action、asset click/context menu、rename/delete modal、SSH modal、keychain modal
- `sftp::bind_sftp_callbacks`
  - right panel 导航、context menu、remote file editor、queue drawer、follow/retry/request
- `shell_chrome::bind_shell_chrome_callbacks`
  - theme、window controls、top status bar、sidebar/right panel 顶层交互
- `windowing::bind_windowing_callbacks`
  - sync modal、host key prompt、workspace paste warning、modal drag/windowing 交互
- `bootstrap.rs` 内保留的 `window.on_workspace_*`
  - workspace tab、terminal text/key/mouse/resize/scroll/paste/copy 等 callback glue

### Slint Property Projection

- `sync_shell_state(...)`
  - 顶层统一投影入口
- 域内同步函数包括：
  - `assets_keychain::sync_sidebar_state`
  - `assets_keychain::sync_assets_toolbar_state`
  - `assets_keychain::sync_assets_context_menu_state`
  - `assets_keychain::sync_asset_modal_state`
  - `sftp::sync_right_panel_state`
  - `sftp::sync_sftp_remote_file_modal_state`
  - `shell_chrome::sync_top_status_bar_state`
  - `windowing::sync_sync_modal_state`
  - `windowing::sync_ssh_host_key_modal_state`
  - `windowing::sync_workspace_paste_warning_modal_state`

### Slint Model Rules

- 列表型 UI 继续通过 `ModelRc` + `VecModel` 投影
- context menu、workspace tabs、SFTP panel、quick launch、asset tree、keychain tree 都遵循“先更新 `ShellViewModel`，再同步到 Slint model”
- 当前只在需要强制 UI 焦点切换时使用 `slint::invoke_from_event_loop`
  - 典型位置：`assets_keychain::schedule_asset_modal_focus`

## Tokio Task / Channel / Actor Interactions

### SSH Session Runtime

- `SessionManager::spawn_session_attempt`
  - 为每次连接尝试创建 `tokio::sync::mpsc::unbounded_channel`
  - `event_tx` 交给 runtime
  - `event_rx` 由 session manager actor-like loop 消费
- `SshSessionRuntime::connect_with_credential_store`
  - 完成 transport/auth/pty/shell 握手后：
    - 创建 `command_tx` / `command_rx`
    - 创建 `SftpRuntimeHandle`
    - `tokio::spawn(run_channel_pump(...))`
- `run_channel_pump`
  - `tokio::select!` 同时处理：
    - UI -> runtime 的命令输入
    - remote SSH channel 输出
    - dirty notification timer
    - working set trim timer

### Event Coalescing

- `SessionManager` 对 `SessionRuntimeEvent::SurfaceChanged` / `SurfaceDirty` 做 backlog 合并
- 目标：
  - 避免 terminal 高频输出直接淹没 UI projection
  - 保证最新 surface 优先，而不是逐帧回放所有中间态

### Vault Sync Background Path

- `bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge`
  - 使用 `AppAsyncRuntime::handle`
  - 背景执行经 `tokio::task::spawn_blocking(...)`
- 结果通过 `std::sync::mpsc::channel<VaultSyncBackgroundMessage>` 回流
- UI 线程由 `Slint Timer` 定时 `try_recv()` 消费结果

### Actor-Like Responsibilities

- `SessionManager`
  - 管 registry、attempt、surface、cwd、runtime attach/detach
- `SshSessionRuntime`
  - 管 transport/auth/terminal/channel pump 生命周期
- `SftpBrowserController`
  - 管 right panel request/pending/follow coordination
- `ShellViewModel`
  - 管所有可见 UI 状态
- `bootstrap` binder
  - 管 callback wiring、跨域桥接与线程切换

## State Flow

### 1. 应用启动

1. `bootstrap.rs` 解析 app paths、stores、repositories、vault runtime 选项
2. 加载 asset catalog、keychain catalog、quick launch preferences、UI preferences
3. 组装 `ShellViewModel::default()` 并注入持久化快照
4. 注册各域 callback
5. 首次执行 `sync_shell_state(...)` + `sync_shell_layout(...)`

### 2. 打开 SSH Session

1. UI 事件触发 `SessionManager::open_session(...)`
2. `SessionManager::spawn_session_attempt(...)`
3. `SessionRuntimeLauncher::launch(...)` 进入 `SshSessionRuntime::connect*`
4. `transport.rs` 解析 direct / proxy chain
5. `auth.rs` 完成 host key 校验与认证
6. `terminal.rs` 请求 PTY、协商 env、请求 shell
7. `pump.rs` 开启 channel pump
8. `SessionRuntimeEvent` 回流到 `SessionManager`
9. `bootstrap` 的 projection timer 从 manager 拉取最新 tabs / terminal surface / SFTP binding

### 3. Workspace Terminal

1. Slint callback 走 `window.on_workspace_*`
2. `workspace_terminal.rs` 把 text/key/mouse/paste/scroll/resize 转发给 `SessionRuntimeControl`
3. runtime 产生 `SurfaceChanged` / `SurfaceDirty`
4. session manager 合并事件
5. projection timer 调 `sync_workspace_projection_from_manager(...)`
6. `sync_workspace_session_state_with_manager(...)` 把 surface / progress / enhanced state 投影到 `AppWindow`

### 4. Asset / Context Menu / Modal

1. Slint callback 更新 `ShellViewModel`
2. `assets.rs`、`keychain.rs`、`context_menu_dispatcher.rs`、`asset_modal_executor.rs` 完成域内变更
3. `assets_keychain.rs` 执行 toolbar/tree/context-menu/modal 的 UI 同步
4. 必要时通过 `schedule_asset_modal_focus(...)` 在 UI 事件循环里推进焦点序列

### 5. SFTP Right Panel

1. workspace active session 变化后，`sftp.rs` 检查并启动/刷新 active browser
2. `SftpBrowserController` 发起 request，结果回写 `ShellViewModel::SftpSessionBindingState`
3. `sftp::sync_right_panel_state(...)` 与 `sync_sftp_remote_file_modal_state(...)` 投影到 Slint

### 6. Sync Modal / Vault

1. UI callback 触发 sync intent
2. `vault_sync.rs` 在后台执行 push/refresh
3. completion message 通过 `std::sync::mpsc` 回 UI 线程
4. `ShellViewModel`、vault panel、sync modal 状态一起刷新

## Error Handling Strategy

- transport / proxy / auth 失败
  - 一律保留 `anyhow::Context`
  - 让错误消息保持“哪一跳失败、哪个请求失败”可定位
- unknown host key
  - 通过 `UnknownHostKeyError` 显式上浮
  - `bootstrap` / `SessionManager` 能把它转成 host-key prompt，而不是普通 toast
- runtime 命令通道关闭
  - `SshSessionRuntime::{send_text_input, send_key_input, ...}` 返回结构化错误
- terminal lock 失败或编码失败
  - `run_channel_pump` 发送 `SessionRuntimeEvent::Error(...)`，并结束当前 pump
- 后台 vault sync 失败
  - 不 panic
  - 转为 `vault_sync_background_failure(...)`
  - 回 UI 线程后只更新 modal/panel/error feedback
- repository / preferences / store 加载失败
  - 记录 `tracing::error!`
  - 尽量降级到空状态或默认配置，避免启动期崩溃
- UI 焦点和延后同步
  - 只允许在 UI 线程触发 Slint property 更新
  - 通过 `Timer` / `invoke_from_event_loop` 处理延后焦点和后台结果回写

## Edge Cases

### 1. Tokio channel 阻塞或消息堆积

- 当前 session runtime 和 manager 之间主要用 `mpsc::unbounded_channel`
- 它不会“阻塞发送端”，但会有消息堆积风险
- 已有缓解：
  - `SurfaceChanged` backlog coalescing
  - `SurfaceDirty` 去重
  - `WorkingSetTrimScheduler`
- 仍需关注：
  - 极端高吞吐输出时，非 surface 事件仍可能排队
  - UI 卡顿时 unbounded backlog 仍可能扩大

### 2. UI 线程更新时机不正确

- 任何 `AppWindow` 更新都必须留在 UI 线程
- 背景任务只能回 channel / timer，不能直接触碰 Slint handle
- `schedule_asset_modal_focus(...)` 已通过 `slint::invoke_from_event_loop` 规避非 UI 线程触发焦点更新

### 3. 数据竞争或共享状态不一致

- `Rc<RefCell<_>>` 不是线程安全容器，只能在 UI 线程内使用
- 若 callback / timer / background completion 在错误时机嵌套 borrow，容易触发运行时 borrow panic
- `Arc<Mutex<TerminalSession>>` 与 `Rc<RefCell<ShellViewModel>>` 的职责必须继续分开：
  - terminal engine 可跨异步任务使用
  - view model 只能 UI 线程独占

### 4. 资源释放时序问题

- `TransportChainGuard` 必须覆盖 `run_channel_pump` 生命周期，否则多跳 SSH upstream 可能提前 drop
- `Disconnect` 时序必须保持：
  - `channel.eof()`
  - `channel.close()`
  - `handle.disconnect(...)`
  - 发送 `SessionRuntimeEvent::Disconnected`

### 5. 异步任务取消或界面关闭后的悬挂回调

- Timer / callback 内普遍使用 `window.as_weak()`
- 必须保持“upgrade 失败就直接 return”的模式
- 否则窗口关闭后继续更新 Slint state，会产生悬挂回调风险

### 6. Slint model 更新与实际数据源不同步

- 若只更新 `ShellViewModel` 而忘记立即调用对应 `sync_*`，UI 会停留在旧 projection
- 高风险区域：
  - assets context menu
  - asset modal / delete confirm
  - workspace tabs / terminal surface
  - SFTP right panel
- `tests/refactor_root_thinness_smoke.sh` 只能约束结构，不能替代 projection 行为测试

### 7. Session 切换时的 SFTP binding 漂移

- active workspace session 改变后，旧的 SFTP binding 可能已经失效
- `sync_active_sftp_projection_from_manager(...)`、`ensure_active_sftp_browser_started(...)`、`sync_active_sftp_browser_follow_request(...)` 必须继续一起工作

### 8. Context Menu Corridor 与 Hover Path 不一致

- context menu 多列展示依赖：
  - open path
  - hover path
  - placement rect
  - pointer corridor
- 如果其中一项更新顺序错误，容易出现：
  - 子菜单闪烁
  - corridor 提前关闭
  - hover row 与 open path 不匹配

## Suggested Tests

### Unit Tests

- `src/shell/view_model/assets.rs`
  - rename/delete modal flow 的纯状态测试
  - keychain 与 console/snippet 共享删除分支测试
- `src/app/ssh/runtime/terminal.rs`
  - `await_channel_success`
  - negotiated env helper
  - key/mouse/paste encoding
  - viewport / scrollback / visible rows 投影
- `src/app/bootstrap/assets_keychain.rs`
  - context menu column/path/rect/placement helper 的纯函数测试

### Integration Tests

- session manager + runtime event coalescing
  - 高频 `SurfaceChanged` / `SurfaceDirty` 下最终 surface 是否正确
- SSH runtime transport/auth/pump
  - proxy chain / unknown host key / auth failure / disconnect cleanup
- vault sync background result 回流
  - `spawn_blocking -> std::sync::mpsc -> Slint Timer` 整链路
- SFTP browser 跟随 active workspace session 切换

### UI Interaction Tests

- assets context menu
  - keyboard left/right/enter/escape
  - pointer corridor 保持与关闭
- asset modal / rename / delete confirm
  - focus sequence 与 validation message 更新
- workspace terminal
  - copy/paste warning modal
  - native terminal scroll thumb drag
  - tab close/fallback selection
- SFTP right panel
  - path submit / back / forward / up / retry / reenable follow
  - remote file editor open/edit/save/error

### Structural Smoke Tests

- 保留并继续扩展：
  - `tests/bootstrap_module_contract_smoke.sh`
  - `tests/view_model_module_contract_smoke.sh`
  - `tests/ssh_runtime_module_contract_smoke.sh`
  - `tests/refactor_root_thinness_smoke.sh`
- 这些 smoke 负责约束：
  - root 路径稳定
  - 已拆模块仍存在
  - facade 不回流成巨型实现文件

## Verification Baseline

本轮最终实现完成后，已通过以下基线：

- `bash tests/refactor_root_thinness_smoke.sh`
- `cargo test --test bootstrap_smoke --test shell_view_model --test ssh_session_manager_spec --test terminal_session_spec --test sftp_runtime_spec --test top_status_bar_smoke --test assets_modal_smoke --test keychain_modal_smoke --test ssh_connection_timeline_spec -q`
- `bash tests/bootstrap_module_contract_smoke.sh`
- `bash tests/view_model_module_contract_smoke.sh`
- `bash tests/ssh_runtime_module_contract_smoke.sh`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
