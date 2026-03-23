# SSH Create Connect Tabs TDD Spec

日期: 2026-03-23
阶段: implementation complete -> TDD handoff
状态: 计划内 Task 已完成，workspace 编译、clippy、完整回归集均通过

## 范围摘要

本轮实现完成了 `SSH create / connect / tabs` 的首轮可用链路：

- `SSH modal` 的 `Standard` 字段、四个动作和基础校验
- `ConnectionProfile` 归一化、凭据存储接口、`known_hosts` TOFU service
- `SessionManager`、`SshSessionRuntime`、`WorkspaceTab` 的基础 session/tab 模型
- `assets tree -> modal -> session open -> tab strip -> workspace host` 的端到端接线
- `close-connection` 从“直接删 tab”收敛为“先断开为 disconnected，再由用户显式关 tab”
- UI contract、smoke、workspace 回归测试补齐

当前边界也需要明确：

- `TerminalSessionHost` 仍是 terminal surface 预留宿主，还没有真实 renderer 嵌入
- `KnownHostsService` 与 host key confirm modal 已有模块和回归覆盖，但尚未接入真实 runtime 握手回调
- `Reconnect` 目前只有 tab 状态与文案占位，没有独立 callback/command

## 核心 Struct

### Modal / shell state

- `src/shell/view_model.rs`
  - `ShellViewModel`
    - shell/UI 真相源
    - 持有 `AssetTree`、workspace tabs、modal state、context menu state
  - `AssetModalState`
    - `NewFolder`
    - `NewSshConnection`
    - `RenameAsset`
    - `DeleteAssetConfirm`
  - `AssetSshConnectionDraft`
    - modal 内 SSH draft
  - `AssetSshModalTab`
    - `Standard`
    - `Tunnel`
    - `Proxy`
    - `Environment`
    - `Advanced`
  - `SshModalAction`
    - `Connect`
    - `TestConnection`
    - `SaveAndConnect`
  - `PendingSshModalAction`
    - 保存“已校验、待 bootstrap 消费”的 modal submit intent
  - `SshHostKeyPromptState`
    - host
    - fingerprint

### SSH domain / runtime

- `src/app/ssh/profile.rs`
  - `ConnectionProfile`
    - `asset_id`
    - `name`
    - `host`
    - `user`
    - `port`
    - `auth_method`
    - `credential_ref`
    - `private_key_path`
    - `remark`
  - `SshAuthMethod`
    - `Password`
    - `PrivateKeyPath`
    - `PrivateKeyContent`

- `src/app/ssh/credentials.rs`
  - `MemoryCredentialStore`
  - `SystemCredentialStore`

- `src/app/ssh/known_hosts.rs`
  - `KnownHostsService`
  - `KnownHostCheck`
    - `Trusted`
    - `Unknown { fingerprint }`
    - `Changed { expected, actual }`

- `src/app/ssh/runtime.rs`
  - `SshSessionRuntime`
  - `TerminalSession`
  - `SessionRuntimeEvent`
    - `Connected`
    - `Output(Vec<u8>)`
    - `Disconnected`
    - `Error(String)`

- `src/app/ssh/session_manager.rs`
  - `SessionManager`
  - `SessionHandle`
  - `SessionState`
    - `Connecting`
    - `Connected`
    - `Disconnected`
    - `Error(String)`
  - `OpenSessionMode`
    - `ActivateExisting`
    - `ForceNewTab`
  - `SessionRuntimeLauncher`

### Workspace tab projection

- `src/shell/tabs.rs`
  - `WorkspaceTab`
    - `session_id`
    - `asset_id`
    - `title`
    - `subtitle`
    - `state`
    - `active`

### Bootstrap bridge

- `src/app/bootstrap.rs`
  - `ShellSessionBridge`
    - `AppAsyncRuntime`
    - `SessionManager`
  - `LiveSessionRuntimeLauncher`
    - 负责把 `ConnectionProfile` 启动为 `SshSessionRuntime`

## Trait 与接口契约

### SSH profile / secret / host key

- `ConnectionProfile::from_draft(&AssetSshConnectionDraft) -> Result<ConnectionProfile>`
  - 归一化 modal draft
  - 负责 auth mode 分流和基础输入约束
  - 当前只支持 `password`、`private-key(path)`、`private-key(content)`

- `CredentialStore`
  - `put_secret(&self, key: &str, value: &str) -> Result<()>`
  - `get_secret(&self, key: &str) -> Result<Option<String>>`
  - `delete_secret(&self, key: &str) -> Result<()>`

- `KnownHostsService`
  - `check(&self, host: &str, port: u16, key: &PublicKey) -> Result<KnownHostCheck>`
  - `accept_unknown(&self, host: &str, port: u16, key: &PublicKey) -> Result<()>`

### Session lifecycle

- `SessionRuntimeLauncher::launch(...) -> Future<Result<()>>`
  - `SessionManager` 不直接依赖具体 SSH runtime
  - launcher 负责把 profile 与 `event_tx` 绑定起来

- `SessionManager`
  - `open_session(profile, mode) -> Result<SessionHandle>`
  - `session(session_id) -> Option<SessionHandle>`
  - `disconnect_session(session_id) -> Option<SessionHandle>`
  - `close_session(session_id) -> Option<SessionHandle>`

约束如下：

- `ActivateExisting` 默认按 `asset_id` 复用已存在 session
- `ForceNewTab` 为同一 asset 创建第二个 session
- `disconnect_session` 只改状态，不移除 tab/session handle
- `close_session` 才是显式释放 registry 项

### ViewModel / bootstrap 边界

- `ShellViewModel::begin_ssh_modal_action(&mut self, action_id: &str) -> bool`
  - 只负责校验和记录 submit intent
  - 不直接启动 runtime

- `ShellViewModel::take_pending_ssh_modal_action(&mut self) -> Option<PendingSshModalAction>`
  - 由 bootstrap 消费

- `ShellViewModel::confirm_asset_modal(&mut self) -> bool`
  - 负责资产树 mutation
  - `Save` 路径走这里

- `bootstrap` 中的 `open_session_with_profile(...)`
  - 把 runtime handle 合并回 `WorkspaceTab`
  - 当前仍由 UI 线程同步调用，不做后台 UI 推送

## Slint callbacks / global state / bindings

### AppWindow 关键 properties

- `asset-ssh-modal-*`
  - `name`
  - `host`
  - `user`
  - `port`
  - `auth-method`
  - `private-key-source`
  - `password`
  - `private-key-content`
  - `private-key-path`
  - `passphrase`
  - `remark`
  - `environment`
  - `proxy-method`
- `workspace-tab-items`
- `active-workspace-session-id`
- `workspace-session-host-mode`
- `workspace-session-title`
- `workspace-session-subtitle`
- `workspace-session-state`
- `workspace-session-can-reconnect`
- `ssh-host-key-modal-open`
- `ssh-host-key-modal-host`
- `ssh-host-key-modal-fingerprint`

### AppWindow callbacks

- `asset-ssh-modal-tab-selected(string)`
- `asset-ssh-modal-draft-changed(string, string)`
- `asset-ssh-modal-action-requested(string)`
- `workspace-tab-selected(string)`
- `workspace-tab-close-requested(string)`
- `asset-selected(string)`
- `asset-context-menu-requested(string, string, length, length)`
- `assets-context-menu-action-invoked(string)`
- `ssh-host-key-modal-accept-requested()`
- `ssh-host-key-modal-reject-requested()`

### Slint component contract

- `ui/components/assets-ssh-connection-modal.slint`
  - 暴露四个动作按钮
  - `Standard` 页承载本轮真实连接字段
- `ui/shell/tabbar.slint`
  - 用 `[WorkspaceTabItem]` 驱动，不再是 placeholder tab
- `ui/shell/terminal-session-host.slint`
  - `mode = welcome | terminal | session-error`
  - disconnected / error 状态留在宿主中，不强制立刻删 tab

当前没有额外 Slint `global` singleton。窗口级状态仍由 Rust `ShellViewModel` -> `AppWindow` property 单向同步。

## Tokio task / channel / actor 交互关系

当前实现里的异步交互链路是：

1. `bootstrap` 创建 `AppAsyncRuntime`
2. `ShellSessionBridge` 用 runtime handle 构造 `SessionManager`
3. `SessionManager::open_session(...)` 创建 `mpsc::unbounded_channel()`
4. `LiveSessionRuntimeLauncher::launch(...)` 在 tokio runtime 中启动 `SshSessionRuntime`
5. runtime 通过 `SessionRuntimeEvent` 把 `Connected / Output / Disconnected / Error` 回送给 `SessionManager`
6. `SessionManager` 更新内部 registry

当前状态的限制：

- registry 更新尚未自动反向投影回 Slint
- 还没有在 runtime 事件消费处使用 `slint::invoke_from_event_loop`
- `Output(Vec<u8>)` 事件已经定义，但 UI terminal surface 尚未接入
- 没有单独 actor mailbox；`SessionManager` 当前承担“session registry + launcher coordinator”的职责

这意味着：

- 当前 UI 可在 session open / disconnect / close 的同步路径上刷新 tab
- 真实的长期异步状态投影仍是下一阶段工作

## 状态流转说明

### Save

1. `asset-ssh-modal-draft-changed` 更新 draft
2. `begin_ssh_modal_action("save")`
3. `confirm_asset_modal()`
4. runtime `AssetTree` 创建/更新 SSH asset
5. bootstrap 持久化 catalog
6. 不创建 workspace tab

### Connect

1. draft 校验通过
2. `ShellViewModel` 记录 `PendingSshModalAction { action: Connect, draft }`
3. bootstrap 消费 pending action
4. 先 `confirm_asset_modal()` 落资产
5. 归一化为 `ConnectionProfile`
6. `SessionManager::open_session(..., ActivateExisting)`
7. `SessionHandle -> WorkspaceTab`
8. `workspace-tab-items` 与 active session 同步到 UI

### Test Connection

1. draft 校验通过
2. `PendingSshModalAction { action: TestConnection, draft }`
3. bootstrap 消费后只更新 modal feedback
4. 不创建 tab

当前这是“连接校验占位接线”，不是完整的后台短生命周期握手流程。

### Save and Connect

1. draft 校验通过
2. `PendingSshModalAction { action: SaveAndConnect, draft }`
3. bootstrap 按 `Save + Connect` 顺序执行
4. 最终创建或激活 tab

### Asset select

1. 用户点击 SSH asset row
2. `asset-selected(string)` -> `ShellViewModel::select_asset(...)`
3. bootstrap 从 `AssetTree` 读取 SSH spec
4. `SessionManager::open_session(..., ActivateExisting)`
5. 已有 tab 则激活；没有则新建

### Open in New Tab

1. 用户从 SSH context menu 触发 `open-in-new-tab`
2. bootstrap 读取目标 asset profile
3. `SessionManager::open_session(..., ForceNewTab)`
4. 同一 asset 可出现第二个 tab

### Close connection

1. 用户从 SSH context menu 触发 `close-connection`
2. bootstrap 调 `SessionManager::disconnect_session(...)`
3. 对应 `WorkspaceTab.state` 变为 `disconnected`
4. tab 保持可见
5. 只有 `workspace-tab-close-requested` 才真正移除 tab

## 关键错误处理策略

- modal draft 非法
  - 通过 `validation_message` 就地反馈
  - 不创建 asset/tab

- `ConnectionProfile::from_draft(...)` 失败
  - bootstrap 把错误写回 modal validation text

- session open 失败
  - `SessionManager` 把 handle 状态收敛为 `Error(String)`
  - tab 保留，可用于后续 reconnect 语义

- close / disconnect 时 session id 非法
  - bootstrap 安静忽略，不做破坏性回退

- `known_hosts` 文件缺失
  - 视为 `Unknown`
- `known_hosts` 文件损坏或不可写
  - 直接返回 `Result::Err`
  - 不静默吞掉

- credential store 失败
  - 通过 `Result` 向上传递
  - 当前还没有接到 modal 层的完整用户提示链路

## 潜在边缘情况

### Tokio channel 阻塞或消息堆积

- 当前使用 `mpsc::unbounded_channel`
- 优点是不会因为短时背压阻塞 runtime 启动
- 风险是长期 `Output` 洪泛时可能堆积
- 下一阶段更适合改为有界 channel 或 batched projection

### UI 线程更新时机不正确

- 当前大部分 tab 更新发生在同步 UI callback 内
- 一旦开始消费 runtime 长连接事件，必须通过 `slint::invoke_from_event_loop`
- 否则会把后台线程状态直接写入 Slint model，触发线程安全问题

### 数据竞争或共享状态不一致

- `SessionManager` registry 由 `Arc<Mutex<_>>` 保护
- `ShellViewModel` 仍在 UI 线程的 `Rc<RefCell<_>>` 中
- 两边目前通过同步 helper 合并，不直接共享内部引用
- 风险点在于“registry 已更新但 tab model 尚未投影”的短暂不一致

### 资源释放时序问题

- `disconnect_session` 不释放 registry 项
- `close_session` 才会移除 registry
- 如果未来 runtime 真正持有 socket/channel，disconnect 与 close 的释放语义必须进一步拆清

### 异步任务取消或界面关闭后的悬挂回调

- 当前 bootstrap 没有在 window close 时统一取消 runtime session
- 一旦接入真实 SSH 长连接，必须处理 window drop 后的后台任务清理
- 否则会出现 session 还在、UI 已销毁的悬挂状态

### Slint model 更新与实际数据源不同步

- `WorkspaceTab` 当前是 `SessionHandle` 的快照投影
- runtime 后续状态变化不会自动刷新现有 `VecModel`
- 下一阶段需要显式把 runtime 事件映射回 `workspace-tab-items`

### Host key TOFU 集成缺口

- `KnownHostsService`、host key modal、smoke 回归都已存在
- 但真实 SSH 握手还没有把未知 key -> modal -> accept -> persist -> reconnect 这条链路接通
- 这是下一轮最重要的补线点之一

## 后续适合编写的测试建议

### 单元测试

- `SessionManager::disconnect_session()` 与 `close_session()` 的 registry 行为
- `ConnectionProfile::from_draft()` 对非法 port、非法 auth source 的错误分支
- `KnownHostsService` 的多 host、多 port 交叉回归
- `WorkspaceTab::from_session()` 在 `Disconnected` / `Error` 状态下的 title/subtitle/state 投影

### 集成测试

- bootstrap 消费 `PendingSshModalAction::SaveAndConnect` 的完整资产落盘 + tab 打开流程
- close-connection 后再次 asset select 只激活 disconnected tab，而不是静默创建新 tab
- workspace tab close 后再打开同 asset，验证 registry 与 tab model 一致
- credential store 接入后，验证 password / inline key secret 的写入与删除

### UI 交互测试

- host key modal 在 accept / reject 下的焦点与关闭行为
- disconnected tab 的视觉状态与 close 按钮交互
- `Test Connection` 成功/失败时 modal feedback 文案
- `TerminalSessionHost` 在 `welcome / terminal / session-error` 三种模式下的布局切换

### 下一阶段最优先的 TDD 顺序

1. runtime 事件通过 `slint::invoke_from_event_loop` 投影到 `workspace-tab-items`
2. host key TOFU 真正接入 session open 流程
3. `Reconnect` callback 和 command contract
4. terminal surface 渲染与 `Output(Vec<u8>)` 投影
