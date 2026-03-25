# SSH Create / Connect Tabs TDD Spec

日期: 2026-03-25

相关文档:
- `docs/plans/2026-03-24-ssh-create-connect-tabs-design.md`
- `docs/plans/2026-03-24-ssh-create-connect-tabs-implementation-plan.md`

## Scope

本轮实现覆盖以下闭环:
- blocking asset modal chrome ownership 收拢
- `New SSH Connection` / `Edit SSH Connection` 单页分组表单
- saved secret 的 retain / clear 语义
- `Save` / `Connect` / `Test` / `Save and Connect` 四动作模型
- workspace SSH tabs 的 title / close / reuse / fallback 规则
- terminal host 的 active-session input / key / resize / surface update contract

## Core Structs

### `AssetSshConnectionDraft`

位置:
- `src/shell/view_model.rs`

职责:
- 作为 SSH modal 的单一 draft source of truth
- 承载基础连接字段、认证字段、可视状态和 inline feedback 相关字段
- 在 edit-mode 下承载 `secret_retention_message`、`can_clear_saved_secret`、`clear_saved_secret_requested`

关键字段:
- `name` / `host` / `user` / `port`
- `auth_method` / `private_key_source`
- `password` / `private_key_content` / `private_key_path` / `passphrase`
- `validation_message`
- `secret_retention_message`
- `can_clear_saved_secret`
- `clear_saved_secret_requested`

### `ConnectionProfile`

位置:
- `src/app/ssh/profile.rs`

职责:
- 把 modal draft 或 saved asset payload 归一化为 SSH runtime 可消费的 profile
- 统一 saved secret ref、inline secret、private key path 认证差异
- 为 ephemeral connect 生成稳定的 temporary session identity

关键接口:
- `from_draft()`
- `from_modal_draft()`
- `from_saved_asset()`
- `temporary_session_asset_id()`

### `ShellViewModel`

位置:
- `src/shell/view_model.rs`

职责:
- 作为 UI thread 上的集中状态容器
- 维护 asset modal、SSH modal action state、workspace tabs、active terminal surface、context menu 等状态
- 对 Slint 只暴露可投影的数据，不直接持有 runtime control

与本轮最相关的字段:
- `asset_modal_state`
- `pending_ssh_modal_action`
- `ssh_modal_action_state`
- `workspace_tabs`
- `active_workspace_session_id`
- `active_workspace_terminal_surface`

### `WorkspaceTab`

位置:
- `src/shell/tabs.rs`

职责:
- 从 `SessionHandle` 投影出 UI tab model
- 统一 title / subtitle / state / error detail / reconnect 能力
- 保证 title 优先使用 asset name，为空时回退到 host

### `SessionHandle`

位置:
- `src/app/ssh/session_manager.rs`

职责:
- 作为 session registry 中的轻量投影
- 维护 `session_id`、`asset_id`、`title`、`subtitle`、`SessionState`

### `TerminalSurfaceState`

位置:
- `src/app/ssh/runtime.rs`

职责:
- 作为 terminal surface snapshot
- 向 UI 暴露 `seqno`、`rows`、`cols`、`visible_lines`
- 取代旧的单一 `screen_text` placeholder 投影

### `SshSessionRuntime`

位置:
- `src/app/ssh/runtime.rs`

职责:
- 持有 russh transport、wezterm terminal state 与 runtime command channel
- 负责 remote output -> terminal snapshot，以及 UI input / resize -> SSH channel

## Traits And Interface Contracts

### `SessionRuntimeLauncher`

位置:
- `src/app/ssh/session_manager.rs`

契约:
- `launch(profile, session_id, event_tx) -> Future<Result<Box<dyn SessionRuntimeControl>>>`
- `probe(profile) -> Future<Result<()>>`

约束:
- `launch()` 负责建立 runtime control，并把 runtime 事件通过 `event_tx` 发回 `SessionManager`
- `probe()` 只做连通性校验，不注册 workspace session

### `SessionRuntimeControl`

位置:
- `src/app/ssh/session_manager.rs`

契约:
- `disconnect()`
- `send_input(bytes)`
- `resize(rows, cols)`

约束:
- 必须可在 `SessionManager` registry 中按 `session_id` 查找并调用
- runtime 尚未 ready 时，`resize` 需要允许由 manager 层做 pending replay

### `AssetCatalogRepository`

位置:
- `src/app/assets_catalog/*`

与本轮关系:
- `Save` / `Save and Connect` 路径负责持久化 asset tree
- `Connect` 不得触发 asset persistence

### `CredentialStore`

位置:
- `src/app/ssh/credentials.rs`

与本轮关系:
- saved secret 的读写、merge、clear 都通过该接口统一完成
- edit-mode 留空保留旧值，explicit clear 才删除 bundle

## Slint Callbacks / Global State / Bindings

### `AppWindow` Global State

位置:
- `ui/app-window.slint`

本轮关键属性:
- `asset-ssh-modal-*`
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

### SSH Modal Callbacks

位置:
- `ui/components/assets-ssh-connection-modal.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`

关键回调:
- `draft-changed(string, string)`
- `action-requested(string)`
- `close-requested()`
- `focus-restore-requested()`

动作映射:
- `"save"`
- `"connect"`
- `"test"`
- `"save-and-connect"`

### Workspace / Terminal Callbacks

位置:
- `ui/shell/workspace-pane.slint`
- `ui/shell/terminal-session-host.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`

关键回调:
- `workspace-tab-selected(string)`
- `workspace-tab-close-requested(string)`
- `workspace-session-text-input(string)`
- `workspace-session-key-input(string, bool, bool, bool)`
- `workspace-session-resize-requested(int, int)`

数据流:
- `TerminalSessionHost` 采集 printable text / named key / resize
- `WorkspacePane` 只做 callback forwarding
- `AppWindow` 作为统一桥接层
- `bootstrap` 将事件路由到 active session

## Tokio Tasks / Channel / Actor 交互关系

### Session Open Path

1. UI 线程通过 `bootstrap` 收到 open / connect 动作
2. `SessionManager::open_session()` 立即注册 `SessionHandle`
3. `SessionManager` 启动两个异步分支
4. 分支 A: 监听 `event_rx`，把 runtime event 写回 registry
5. 分支 B: 调用 `SessionRuntimeLauncher::launch()`，拿到 `SessionRuntimeControl`

### Runtime Event Flow

方向:
- runtime -> `mpsc::UnboundedSender<SessionRuntimeEvent>` -> `SessionManager`

事件:
- `Connected`
- `SurfaceChanged(TerminalSurfaceState)`
- `Disconnected`
- `Error(String)`

### Runtime Command Flow

方向:
- UI -> `bootstrap` -> `SessionManager` -> `SessionRuntimeControl`

命令:
- printable text -> `send_input(bytes)`
- named key -> `encode_named_key_input()` -> `send_input(bytes)`
- resize -> `resize(rows, cols)`
- close/disconnect -> `disconnect()`

### Pending Resize Replay

位置:
- `src/app/ssh/session_manager.rs`

当前策略:
- runtime control 未 attach，但 session 已注册时，`resize_session()` 不报错
- manager 仅保留最近一次 `(rows, cols)` 到 `pending_resizes`
- runtime control attach 后回放最后一次尺寸

这是本轮新增的稳定化点，避免 active terminal 在 runtime ready 前丢失尺寸同步。

## 状态流转说明

### SSH Modal Action State

状态机:
- `Idle`
- `Busy(SshModalAction)`
- `Success(message)`
- `Error(message)`

流转:
- `Idle -> Busy(action) -> Success/Error`
- draft 变更、modal 关闭、下一次动作开始前会把状态重置为 `Idle`

### Session Lifecycle

状态机:
- `Connecting`
- `Connected`
- `Disconnected`
- `Error(message)`

流转源:
- `SessionRuntimeEvent::Connected`
- `SessionRuntimeEvent::Disconnected`
- `SessionRuntimeEvent::Error`

### Workspace Tab Activation / Close Fallback

规则:
- `ActivateExisting` 默认复用同 `asset_id` 的 live session
- `ForceNewTab` 为同 asset 创建并行 session
- 关闭 active tab 时，回退顺序为 `right -> left -> welcome`
- failed / disconnected tab 保留，用户手动关闭

### Host Key Approval

意图:
- `ModalTestConnection`
- `OpenSession(OpenSessionMode)`

规则:
- unknown host key 先弹确认 modal
- accept 后按原 intent 重试
- reject 后:
  - test path 更新 modal error feedback
  - open-session path 保留 error tab

## 关键错误处理策略

### Save / Save And Connect 回滚

位置:
- `src/app/bootstrap.rs`

策略:
- asset confirm 之前记录 `previous_state`
- secret sync 或 catalog persist 失败时回滚到之前状态
- 不留下半持久化资产或半更新 secret

### Stored Secret Lookup

位置:
- `src/app/ssh/credentials.rs`
- `src/app/ssh/runtime.rs`
- `src/app/bootstrap.rs`

策略:
- 缺 credential ref、缺 keyring entry、bundle field 为空时都生成明确错误文案
- edit-mode secret hydration 会把 inline error 显示回 modal

### Runtime Forwarding Failure

位置:
- `src/app/bootstrap.rs`

策略:
- active session 不存在时直接 no-op
- encoding 失败或 runtime forwarding 失败只记录日志，不崩 UI 线程

### Session Close / Disconnect

位置:
- `src/app/ssh/session_manager.rs`

策略:
- runtime control 已 ready 时立即 `disconnect()`
- runtime control 未 ready 时记录 `pending_disconnects`
- control attach 后如果 session 已关闭，则立即执行 disconnect

## Edge Cases

### Tokio channel 阻塞或消息堆积

当前状态:
- runtime event 与 runtime command 都使用 unbounded channel
- `pending_resizes` 只保留最后一次尺寸，避免 resize 风暴导致重放队列增长

风险:
- 高频 surface update 仍可能造成 event backlog

建议:
- 后续可考虑对 surface update 做 coalescing 或 bounded queue

### UI 线程更新时机不正确

当前状态:
- Slint property mutation 统一通过 UI 线程上的 `ShellViewModel` 与 `sync_*` 函数完成
- runtime 不直接触碰 Slint component

风险:
- 如果 future 新增后台线程直接写 UI，会破坏当前约束

建议:
- 继续坚持 `manager -> projection sync -> Slint binding` 的单向更新

### 数据竞争或共享状态不一致

当前状态:
- `SessionRegistry` 使用 `Mutex`
- `ShellViewModel` 使用 UI 线程本地 `Rc<RefCell<_>>`

风险:
- manager registry 与 UI projection 是最终一致，不是强一致

建议:
- 未来新增跨线程状态时，不要让 Slint model 与 runtime registry 双写

### 资源释放时序问题

当前状态:
- `close_session()` 会移除 registry、surface、pending_disconnects、pending_resizes
- runtime control 到达过晚时仍会根据 session 是否存活决定是否立刻 disconnect

风险:
- 如果后续增加更多附属资源，必须保证与 session removal 同步清理

### 异步任务取消或界面关闭后的悬挂回调

当前状态:
- active session lookup 失败时 input / key / resize forwarding 直接返回
- runtime command channel 关闭时返回明确错误

风险:
- window 已关但后台 runtime 仍在发事件时，仍需要 registry 层兜底丢弃无效 surface

### Slint model 更新与实际数据源不同步

当前状态:
- `workspace-tab-items` 与 `workspace-session-visible-lines` 都由 projection sync 统一更新
- active surface 只在 `session_id` 匹配 active tab 时投影

风险:
- 如果未来引入多 pane 或 detached terminal，当前 “单 active session surface” 假设会失效

### 同一 draft 的 ephemeral identity 漂移

当前状态:
- modal `Connect` 使用稳定的 `temporary_session_asset_id()`

风险:
- 如果 profile identity 规则未来改变，可能打破 tab reuse 语义

### Edit-Mode Secret 留空误清空

当前状态:
- 留空且未 clear 代表 retain
- 显式 `clear_saved_secret_requested` 才删除 bundle

风险:
- 如果未来新增认证方式但忘记更新 merge 规则，容易再次出现隐性数据损坏

## 后续适合补充的测试建议

### 单元测试

- `ConnectionProfile::temporary_session_asset_id()` 针对 password / inline key / key path 的 identity 稳定性测试
- `SessionManager` 针对 `pending_resizes` 的覆盖测试再补一个 “close before attach clears queued resize” 用例
- secret merge / clear 规则针对更多认证切换组合补边界测试

### 集成测试

- active tab 切换后，terminal input 只发送到当前 active session
- unknown host key accept / reject 后的 tab / modal state 清理
- save rollback 失败路径在 asset tree、credential store、workspace tabs 三处的一致性

### UI 交互测试

- modal header / drag / close 几何在窄宽度下的行为
- active tab 长标题与 subtitle 下的 elide 和 close hit target
- terminal host focus 获取后 printable text、named key、resize 的回调触发顺序

### 回归 smoke 建议

- 保持 `assets_modal_ui_contract_smoke.sh` 与 `ssh_connect_tabs_ui_contract_smoke.sh`
- 对日志 smoke 保留唯一临时目录与轮询读取，避免 suite 并发导致假阴性

