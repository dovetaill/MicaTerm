# Asset Sync Background Service TDD Handoff

Date: 2026-04-02

## Scope

本轮实现把资产同步编排从 `bootstrap` 内部的散落控制逻辑收口到 `VaultSyncService`，统一承接：

- `manual sync`
- `debounced auto sync`
- `periodic refresh`
- `open sync settings -> refresh remote head`

同时，`Sync Settings` modal 从“仅配置表单”升级为“状态卡 + 配置表单”，新增本地/远端同步状态展示，并保持：

- 打开 modal 不阻塞 UI
- 远端状态读取始终脱离 UI 线程
- 成功静默更新状态
- 失败可回看但不阻塞当前交互

## Core Structs

- `VaultSyncService`
  - 本轮同步编排核心；维护 `dirty / running / remote_head_refresh_in_flight`
- `VaultSyncServiceConfig`
  - service 构造配置；目前主要承载显式注入的 `tokio::runtime::Handle`
- `VaultSyncIntent`
  - UI/业务层提交给 service 的意图枚举：
    - `ManualSync`
    - `LocalMutation`
    - `PeriodicRefresh`
    - `RefreshRemoteHead`
- `VaultSyncTrigger`
  - 真正驱动执行路径的触发来源：
    - `Manual`
    - `DebouncedAuto`
    - `Periodic`
- `VaultSyncExecution`
  - service 选出的后台执行类型：
    - `Push`
    - `Refresh`
- `RemoteHeadSnapshot`
  - primary remote head 的轻量回传载体；包含：
    - `revision`
    - `committed_at`
    - `error`
    - `loading`
- `VaultSyncBackgroundMessage`
  - `bootstrap` 中 UI 线程消费的后台消息：
    - `Completed { trigger, execution, result }`
    - `RemoteHeadRefreshed { snapshot }`
- `VaultSyncBackgroundSuccess`
  - 后台 sync 成功后的聚合投影；含 projection、modal state、panel state、本地 durable state
- `VaultSyncBackgroundFailure`
  - 后台 sync 失败后的最小回传；保留 modal/panel/local state
- `VaultSessionState`
  - vault runtime 会话态；持有 root、provider factory、bootstrap/local state、解密快照
- `SyncModalViewState`
  - sync modal 的 UI 状态载体；本轮关键字段包括：
    - `local_last_sync_text`
    - `remote_last_update_text`
    - `primary_revision_text`
    - `remote_status_text`
    - `remote_status_loading`

## Traits And Interface Contracts

### `VaultSyncService` Public Contract

- `request(intent: VaultSyncIntent) -> bool`
  - `LocalMutation`：标记 dirty，并总是返回 `true`
  - `RefreshRemoteHead`：通过 `AtomicBool` 去重；同一时间只允许一个 in-flight refresh
  - `ManualSync` / `PeriodicRefresh`：仅表示意图可进入调度
- `begin_trigger(trigger, background_ready, requires_initial_remote_sync) -> Option<VaultSyncExecution>`
  - 根据 dirty 状态、触发源、首次远端恢复需求选择 `Push` 或 `Refresh`
  - 如果当前已有 `running` 执行，返回 `None`
- `finish(execution, success)`
  - 结束一次 `Push` / `Refresh`
  - `Push` 成功才会清掉 dirty
- `finish_remote_head_refresh()`
  - 释放 `remote_head_refresh_in_flight`
- `spawn_blocking(work) -> Option<VaultSyncBackgroundTask>`
  - 依赖显式 `runtime_handle`
  - 无 runtime handle 时返回 `None`

### Provider / Credential Contracts

- `VaultProviderFactory`
  - `build_provider(&BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>>`
  - `build_provider_for_vault(&BootstrapRemoteConfig, &Path) -> Result<Arc<dyn VaultProvider>>`
- `VaultProvider`
  - `read_head()`
  - `read_revision(&VaultHead)`
  - `write_revision(&ProviderWriteRequest)`
  - `prune_revisions(...)`
- `CredentialStore`
  - 用于解析 sync 所需 credential material
  - remote head refresh 与正式 sync 共用同一套 credential 解析链路

## Slint Callbacks, Global State, Bindings

### AppWindow Sync Modal Properties

- 原有：
  - `sync-modal-open`
  - `sync-modal-mode`
  - `sync-modal-title`
  - `sync-modal-headline`
  - `sync-modal-status-text`
  - `sync-modal-error-text`
  - `sync-modal-provider-label`
  - `sync-modal-target-label`
  - `sync-modal-conflict-count`
  - `sync-modal-conflict-summary`
- 本轮新增：
  - `sync-modal-local-last-sync-text`
  - `sync-modal-remote-last-update-text`
  - `sync-modal-primary-revision-text`
  - `sync-modal-remote-status-text`
  - `sync-modal-remote-status-loading`

### Slint Callbacks

- `open-sync-modal-requested`
- `sync-now-requested`
- `sync-modal-primary-action-requested`
- `sync-modal-sync-now-requested`
- `sync-modal-draft-changed`
- `sync-modal-toggle-changed`
- `sync-modal-close-requested`

### Binding Rules

- `sync_sync_modal_state(...)`
  - 负责把 `ShellViewModel::sync_modal_state()` 的所有字段投影到 `AppWindow`
- `AppWindow -> SyncVaultModal`
  - 将新增的 5 个 sync status 字段继续透传给 `SyncVaultModal`
- `SyncVaultModal`
  - 本轮新增极简状态卡，固定展示：
    - `Local last sync`
    - `Remote last update`
    - `Primary revision`
    - 当前 remote status 文案

## Tokio Task / Channel / Actor Interactions

### Runtime / Task Model

- `VaultSyncService::spawn_blocking(...)`
  - 通过注入的 `tokio::runtime::Handle` 启动异步任务
  - 任务内部再使用 `tokio::task::spawn_blocking(...)` 执行真正的文件/远端 I/O
- `run_vault_sync`
  - 仍在 `bootstrap` 内，但只负责：
    - 调用 service 做调度决策
    - 组装 worker 所需数据快照
    - 把结果投递回 UI 线程消费通道
- `request_sync_modal_remote_head_refresh`
  - modal 打开后触发的专用后台路径
  - 与正式 sync 共用 `VaultSyncService` 的 runtime 能力与去重语义

### Channel / Timer

- `std::sync::mpsc::channel::<VaultSyncBackgroundMessage>()`
  - 后台 worker -> UI 线程 的结果回流通道
- `vault_sync_completion_timer`
  - `Slint Timer`，定时 `try_recv()`
  - 在 UI 线程上消费：
    - `Completed`
    - `RemoteHeadRefreshed`

### Actor-Like Responsibility Split

- `VaultSyncService`
  - 负责“是否可以开始”“当前执行类型”“是否需要去重”
- `bootstrap background worker`
  - 负责真正 I/O 与状态计算
- `vault_sync_completion_timer`
  - 负责把后台结果安全回写 UI state

## State Flow

### 1. 资产变更 -> 自动后台同步

1. 资产 / SSH / keychain 入口调用 `request(VaultSyncIntent::LocalMutation)`
2. `vault_auto_sync_timer` 到期后触发 `run_vault_sync(VaultSyncTrigger::DebouncedAuto)`
3. service 根据 dirty/running/background_ready 选择是否执行 `Push`
4. worker 在后台执行 `sync_local_vault(...)`
5. `Completed` 消息回到 UI 线程
6. `finish(Push, success)` 更新内部状态
7. `ShellViewModel` 与 Slint 绑定状态同步刷新

### 2. titlebar / modal 手动同步

1. `sync-now-requested` 或 `sync-modal-sync-now-requested`
2. `run_vault_sync(VaultSyncTrigger::Manual)`
3. service 根据当前 dirty 与首次远端恢复需求，选择：
   - `Push`
   - `Refresh`
4. UI 先进入 feedback running，再等待后台结果
5. completion timer 回写结果，并根据成功/失败更新 feedback

### 3. 打开 `Sync Settings` -> 后台刷新 remote head

1. `open-sync-modal-requested`
2. 先执行：
   - `hydrate_sync_modal_draft(...)`
   - `update_sync_modal_for_local_state(...)`
   - `state.open_sync_modal()`
3. 再执行 `request_sync_modal_remote_head_refresh(...)`
4. service 通过 `request(VaultSyncIntent::RefreshRemoteHead)` 做并发去重
5. 后台 worker 读取 primary remote head
6. completion timer 收到 `RemoteHeadRefreshed`
7. UI 调用：
   - `finish_remote_head_refresh()`
   - `apply_remote_head_snapshot_to_sync_modal(...)`
8. modal 中的 revision / remote update / status 文案异步更新

## Error Handling Strategy

- runtime handle 不存在
  - `spawn_blocking(...)` 返回 `None`
  - remote head refresh 会立即清除 loading，并释放 in-flight 标志，不阻塞 modal
- remote head 读取失败
  - `RemoteHeadSnapshot.error` 回写到 modal
  - `remote_status_text = "Failed to refresh remote status."`
  - `error_text` 保留底层错误
  - modal 保持打开
- 正式 sync 后台失败
  - `set_sync_modal_error_without_opening(...)`
  - 不强制重开 modal
  - 手动 sync 只显示 `"Sync failed"` feedback
- 时间戳解析失败
  - 统一通过 `format_sync_timestamp_for_ui(...)`
  - 无法识别时回退 `Unknown`
  - 不允许因为时间格式问题中断主流程
- 本地无历史同步时间
  - `local_last_sync_text` 回退 `Never synced`
- remote head 不存在
  - `primary_revision_text = "Unknown"`
  - `remote_last_update_text = "Unknown"`
  - `remote_status_text = "Primary remote is empty."`

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 当前使用 `std::sync::mpsc` + `try_recv` timer 轮询；若 future 中结果产生速度高于 UI 消费速度，可能形成短时积压
  - 后续若消息量继续增加，建议切换为 bounded channel 或在结果层按类型合并
- UI 线程更新时机不正确
  - 任何 modal / panel / tree 更新都必须在 completion timer 所在 UI 线程执行
  - 不能在 `spawn_blocking` worker 内直接修改 `ShellViewModel`
- 数据竞争或共享状态不一致
  - `VaultSyncService` 通过 `Mutex<VaultSyncState>` 保证 `dirty / running` 一致性
  - `RefreshRemoteHead` 使用 `AtomicBool`，避免多次打开 modal 触发重复远端读取
- 资源释放时序问题
  - 若 remote head refresh 在 runtime 缺失、窗口关闭或消息丢弃时没有调用 `finish_remote_head_refresh()`，后续刷新会永久被卡住
  - 当前已在 `RemoteHeadRefreshed` 消费路径和 `spawn_blocking(None)` fallback 路径释放
- 异步任务取消或界面关闭后的悬挂回调
  - completion timer 先检查 `handle.upgrade()`
  - 如果 window 已销毁，不再尝试 UI 回写
  - 但后台 worker 仍可能完成，后续若要进一步收紧，需要补显式取消或 drop 策略
- Slint model 更新与实际数据源不同步
  - modal 状态卡显示来自 `ShellViewModel`
  - 若 `update_sync_modal_for_local_state(...)`、`apply_remote_head_snapshot_to_sync_modal(...)` 与 `sync_sync_modal_state(...)` 任何一环漏掉，UI 会停留在旧值
- 快速反复打开/关闭 modal
  - service 会拒绝重复 `RefreshRemoteHead`
  - 直到前一次 refresh 结果被消费并 `finish_remote_head_refresh()` 才允许下一次读取
- 时间格式混用
  - 当前同时支持 20 位 epoch-millis 字符串与 RFC3339/ISO8601
  - 其它旧格式统一回退为 `Unknown`

## Suggested Tests

### Unit Tests

- `format_sync_timestamp_for_ui(...)`
  - epoch-millis -> UI 时间
  - RFC3339 -> UI 时间
  - 非法字符串 -> `Unknown`
- `VaultSyncService`
  - `LocalMutation` 后 manual/periodic 的执行选择
  - `running` 状态下拒绝重复 trigger
  - `RefreshRemoteHead` 去重与释放

### Integration Tests

- modal 打开 + slow provider
  - 保证 UI 秒开，结果异步回流
- modal 打开 + runtime handle 缺失
  - 保证不 panic、不永久卡 loading
- 手动 sync 与 remote head refresh 交错
  - 保证 feedback 与 modal 状态不会互相覆盖出错
- 快速连续打开两次 modal
  - 只触发一次 in-flight remote head refresh

### UI / Contract Tests

- `tests/sync_vault_modal_smoke.rs`
  - 继续覆盖 sync modal status props、后台 remote head refresh 成功/失败
- `tests/assets_modal_render_spec.rs`
  - 确认状态卡在 ready/not-configured 视图中仍有可见渲染区域
- `tests/top_status_bar_ui_contract_smoke.sh`
  - 锁定 `AppWindow` 与 `SyncVaultModal` 的新增 status props
- 可补充的 UI 交互测试
  - `remote-status-loading` 为 `true` 时显示 loading 文案
  - 窄窗口 / 短窗口下状态卡与表单不互相遮挡
