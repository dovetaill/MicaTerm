# Asset Sync TDD Handoff

日期: 2026-03-31
范围: `2026-03-31-asset-sync-implementation-plan.md` Task 1-7 落地结果
状态: 已实现并通过 `cargo check --workspace` 与 `cargo clippy --workspace -- -D warnings`

## 1. 核心 struct

### UI / Shell

- `SyncModalMode` (`src/shell/view_model.rs`)
  - 现在只保留 `NotConfigured` / `Ready` / `SyncError`
  - 旧的 `Locked` / `UnlockedButRemoteIncomplete` 已从首发契约移除
- `SyncModalViewState` (`src/shell/view_model.rs`)
  - 管理 `Sync Settings` modal 的所有首发字段
  - 关键字段:
    - `open`
    - `mode`
    - `headline`
    - `status_text`
    - `error_text`
    - `primary_action_label`
    - `secondary_action_label`
    - `primary_gist_id`
    - `primary_pat`
    - `mirror_enabled`
    - `mirror_gist_id`
    - `mirror_pat`
    - `master_password`
- `SyncFeedbackViewState` (`src/shell/view_model.rs`)
  - 为 titlebar `Sync` 按钮提供即时反馈文案和序列号
  - `sequence` 用于强制 Slint tooltip/feedback 重新显示
- `VaultPanelViewState` (`src/shell/view_model.rs`)
  - 现在只保留 `title` 和 `primary_status_label`
  - 旧 lock/unlock label 与按钮文案字段已经删除

### Vault Runtime / Local Durable State

- `VaultSessionState` (`src/app/bootstrap.rs`)
  - 同步运行时的内存根对象
  - 关键字段:
    - `root_dir`
    - `provider_factory`
    - `bootstrap_template`
    - `local_state`
    - `unlocked_vault_key`
    - `decrypted_snapshot`
- `LocalVaultBootstrapState` (`src/app/vault/bootstrap.rs`)
  - 本地持久化的 durable sync state
  - 关键字段:
    - `bundle`
    - `wrapped_vault_key`
    - `kdf`
    - `current_revision`
    - `local_snapshot_hash`
    - `last_local_change_at`
    - `last_successful_push_at`
    - `last_successful_pull_at`
    - `last_sync_error`
- `VaultHead` (`src/app/vault/model.rs`)
  - 远端 head 元数据
  - 关键字段:
    - `vault_revision`
    - `parent_revision`
    - `committed_at`
    - `committed_by_device`
    - `payload_hash`
    - `wrapped_vault_key`
    - `kdf`

### Sync Decision / Engine / Recovery

- `LocalSyncState` (`src/app/vault/sync_decision.rs`)
  - 从 `LocalVaultBootstrapState` 派生出的同步判定输入
- `SyncDecision` / `SyncAction` (`src/app/vault/sync_decision.rs`)
  - 输出 `Noop` / `Push` / `Pull`
  - 同时显式声明:
    - `backup_local_snapshot`
    - `backup_remote_snapshot`
- `SyncRequest` (`src/app/vault/engine.rs`)
  - 本地发起 push 时传入 `SyncEngine`
- `SyncReport` (`src/app/vault/engine.rs`)
  - 返回 primary revision、manifest、encrypted snapshot 和 mirror failure 列表
- `RecoverySnapshotRecord` / `RecoverySource` (`src/app/vault/recovery.rs`)
  - recovery 备份记录
  - 当前来源:
    - `LocalBeforePull`
    - `RemoteBeforePush`
- `VaultSyncSchedulerState` (`src/app/bootstrap.rs`)
  - 只有两个字段:
    - `dirty`
    - `running`
  - 负责 UI 线程上的 single-flight 调度

## 2. trait 与接口契约

### Credential

- `CredentialStore` (`src/app/ssh/credentials.rs`)
  - 本轮继续作为共享秘密存储抽象
  - 新增/强化的使用契约:
    - provider PAT 继续走 `put_secret/get_secret/delete_secret`
    - runtime auto-recovery key 通过 `persist_binary_secret/load_fixed_binary_secret` 读写
- `vault_runtime_key_credential_ref(vault_id)` (`src/app/vault/bootstrap.rs`)
  - 规范 runtime key 的 credential ref
- `persist_runtime_vault_key(...)`
- `load_runtime_vault_key(...)`
  - 约束: 只接受固定长度 `[u8; 32]`

### Provider

- `VaultProvider` (`src/app/vault/provider/mod.rs`)
  - 核心契约:
    - `read_head()`
    - `read_revision(&VaultHead)`
    - `write_revision(&ProviderWriteRequest)`
    - `prune_revisions(keep_latest, live_head)`
- `VaultProviderFactory` (`src/app/bootstrap.rs`)
  - 根据 `BootstrapRemoteConfig` 实例化 provider
- `ProviderCapabilities`
  - 用于描述 conditional write、pack layout、pack 限制
- `DEFAULT_REVISION_RETENTION_LIMIT`
  - 当前固定为 `10`
  - `SyncEngine::sync()` 在 primary 和 mirror write 成功后都会调用 `prune_revisions(...)`

### Sync Decision

- `decide_sync_action(local, remote)` (`src/app/vault/sync_decision.rs`)
  - 契约:
    - payload hash 相同 => `Noop`
    - 仅本地变更 => `Push`
    - 仅远端变更 => `Pull`
    - 双方都变更 => 比较 `last_local_change_at` 与 `remote.committed_at`
    - 决策结果必须显式给出是否先写 recovery snapshot

## 3. Slint callbacks / global state / bindings

### 当前约束

- 本轮没有新增 Slint global singleton
- 状态仍然走 `AppWindow <-> bootstrap.rs <-> ShellViewModel`
- 绑定形式以 property push 为主，而不是把业务状态直接暴露给 Slint 自己推导

### 关键 UI 同步函数

- `sync_sync_modal_state(...)` (`src/app/bootstrap.rs`)
  - 把 `SyncModalViewState` 投影到 `AppWindow` 的 `sync_modal_*` properties
- `sync_top_status_bar_state(...)` / `sync_shell_state(...)`
  - 把 `sync_feedback_*`、`sync_modal_open` 等状态投影到 titlebar

### 关键 callbacks

- titlebar:
  - `on_sync_now_requested`
  - `on_open_sync_modal_requested`
- sync modal:
  - `on_sync_modal_draft_changed`
  - `on_sync_modal_toggle_changed`
  - `on_sync_modal_submit_master_password`
  - `on_sync_modal_sync_now_requested`
  - `on_sync_modal_primary_action_requested`
  - `on_sync_modal_secondary_action_requested`
  - `on_sync_modal_close_requested`

### 当前绑定语义

- titlebar `Sync`:
  - 已配置时直接进入即时 sync/check 语义
  - 不再打开显式 unlock 流程
- sync modal:
  - 只负责配置、首启、诊断、手动 sync
  - 不再暴露 `Lock` / `Unlock`
- `VaultPanelViewState.primary_status_label`:
  - 只作为反馈文本来源
  - 由 `sync_local_vault(...)`、`refresh_local_vault_from_primary_remote_if_changed(...)` 和定时调度路径更新

## 4. Tokio task / channel / actor 交互关系

### 本轮实际情况

- 本轮没有新增独立 Tokio channel 或 actor
- 同步调度当前由 Slint `Timer` 驱动，而不是异步消息队列:
  - `VAULT_AUTO_SYNC_DEBOUNCE_MS = 1200`
  - `VAULT_PERIODIC_SYNC_INTERVAL_MS = 120000`
- 并发约束依赖 `VaultSyncSchedulerState`:
  - `dirty = true` 表示有待同步本地变更
  - `running = true` 表示当前已有一次 sync 在执行
  - `run_vault_sync(...)` 负责 single-flight，避免 timer 重入导致并发写远端

### 对后续 Tokio 化的要求

- 如果未来把 provider I/O 挪到 Tokio task:
  - 必须保持单写者模型
  - 必须使用 bounded channel，不能让 UI mutation 无界堆积
  - UI 更新必须回到 Slint UI 线程，不能从 worker thread 直接改 `ShellViewModel`

## 5. 状态流转说明

### A. 首次启用同步

1. 用户填写 primary/mirror draft
2. `persist_sync_modal_settings(...)` 写入 bootstrap config 与 provider credential
3. `submit_sync_modal_master_password(...)`
4. 如果远端为空:
   - `create_local_vault_from_shell_state(...)`
   - 生成 vault key
   - 写本地 `LocalVaultBootstrapState`
   - 写加密 cache
   - 写 runtime vault key 到 `CredentialStore`
5. 状态进入 `SyncModalMode::Ready`

### B. 重启后的自动恢复

1. bootstrap 读取本地 `LocalVaultBootstrapState`
2. `silently_restore_vault_session_from_runtime_key(...)`
3. 从 `CredentialStore` 读取 runtime vault key
4. 解密本地 cache 并 `apply_vault_snapshot_to_shell(...)`
5. 成功则无需再次输入 master password
6. 失败则删除失效 runtime key，并把诊断文本写入 sync UI

### C. 本地改动后的短防抖同步

1. 资产树/片段/密钥链发生 mutation
2. scheduler 标记 `dirty = true`
3. `vault_auto_sync_timer` 在 1.2s 后触发
4. `run_vault_sync(VaultSyncTrigger::DebouncedAuto)`
5. 成功 push 或 pull 后清理 `dirty`
6. 如果失败，`dirty` 保留，等待下一次 periodic 或新的 local mutation 重试

### D. 周期校验

1. `vault_periodic_sync_timer` 每 120s 触发
2. `run_vault_sync(VaultSyncTrigger::Periodic)`
3. 若本地无脏数据但远端变更:
   - 走 `Pull`
4. 若本地与远端都变更:
   - `decide_sync_action(...)` 自动判定
   - 输的一侧先写 recovery snapshot

### E. Push / Pull 结果落盘

- `Push`:
  - 写 primary
  - 写 mirrors
  - prune revision 到最新 10 个
  - 更新:
    - `current_revision`
    - `local_snapshot_hash`
    - `last_successful_push_at`
    - `last_sync_error = None`
- `Pull`:
  - 读 remote revision
  - 必要时先写 `LocalBeforePull` recovery
  - 覆盖 shell projection 与本地 cache
  - 更新:
    - `current_revision`
    - `local_snapshot_hash`
    - `last_successful_pull_at`
    - `last_sync_error = None`

## 6. 关键错误处理策略

- runtime key 读取失败:
  - 删除不可读 credential
  - 仅保留诊断，不进入无限重试
- runtime key 解密 cache 失败:
  - 删除失效 runtime key
  - 要求用户重新输入 master password
- provider read/write 失败:
  - 记录到 `last_sync_error`
  - UI 侧写入 `primary_status_label` 与 modal error
  - 不静默吞掉
- mirror 写失败:
  - primary 成功仍视为可继续使用
  - `SyncReport::mirror_failures` 以 degraded 状态返回
- conflict / dual-change:
  - 先 recovery，再 push/pull
- legacy remote 缺失 snapshot recovery metadata:
  - 直接报错
  - 要求先由新版本设备完成一次正式 sync

## 7. 潜在边缘情况（Edge Cases）

### 已覆盖 / 已显式约束

- Tokio channel 阻塞或消息堆积
  - 当前未引入 channel，但若后续改为 Tokio task，必须使用 bounded channel；否则 debounce 和 periodic 事件可能排队失控
- UI 线程更新时机不正确
  - 目前同步与状态投影都在 UI 线程路径内执行
  - 后续若把 provider I/O 下放到 worker，必须通过 `slint::invoke_from_event_loop(...)` 或等价机制回到 UI 线程
- 数据竞争或共享状态不一致
  - `VaultSessionState` 与 `ShellViewModel` 当前都在闭包中顺序借用
  - 风险点在 future/Tokio 化后多线程同时读写 `dirty/running/local_state`
- 资源释放时序问题
  - Pull 前会先 `clear_vault_decrypted_state(...)`
  - 覆盖前必须先恢复/删除旧 secret bundle，避免 credential store 残留脏数据
- 异步任务取消或界面关闭后的悬挂回调
  - 当前没有新增后台 channel actor，但 timer 触发后的闭包仍依赖 window weak handle
  - 未来若迁移到 Tokio task，必须在窗口关闭时取消任务并阻止 late callback 写 UI
- Slint model 更新与实际数据源不同步
  - sync 完成后必须统一走 `apply_vault_snapshot_to_shell(...)` + `sync_shell_state(...)`
  - 不能只更新 cache 或 bootstrap state 而不更新 projection

### 额外高风险场景

- 首次 enable 时远端其实非空
  - 必须拒绝把一个空本地直接覆盖到已有远端
- periodic pull 时本地有未推送修改
  - 必须先写 `LocalBeforePull` recovery snapshot
- dual-change push 时远端更新更旧但仍被覆盖
  - 必须先写 `RemoteBeforePush` recovery snapshot
- provider prune 失败
  - 当前视为 sync 失败路径的一部分，不能标记为成功
- `last_local_change_at` 未正确维护
  - 会导致 `decide_sync_action(...)` 误判 push/pull
- runtime key 还在，但 cache 文件缺失
  - silent restore 必须失败并清除 runtime key

## 8. 后续适合补充的测试建议

### 单元测试

- `sync_decision.rs`
  - 比较 `last_local_change_at` 与 `remote.committed_at` 的边界条件
  - `None` / 空字符串 / 相同 payload hash 的判定
- `vault/bootstrap.rs`
  - runtime key 删除失败时的降级行为
  - `LocalVaultBootstrapState` 新字段的序列化兼容
- `vault/provider/*`
  - `prune_revisions(...)` 在 head 不连续、历史缺文件、镜像 provider 失败时的行为

### 集成测试

- restart 后本地 cache 缺失、runtime key 仍存在
- mirror degraded 后下一次成功写入能否清理旧错误态
- provider auth 失败后，debounced retry 与 periodic retry 是否只重试必要路径
- periodic pull 覆盖本地前，recovery snapshot 是否包含完整 console/snippet/keychain 数据

### UI / 交互测试

- titlebar `Sync` 在:
  - 未配置
  - 已配置且 clean
  - 已配置且 sync error
  - restart 后 silent restore 成功
  下的行为一致性
- sync modal 不再出现 `Unlock` / `Lock` / `locked`
- sync feedback tooltip 在连续多次 sync 结果切换时不会卡在旧文案
- modal 关闭后重新打开，draft/status/error 是否符合预期清理策略

## 9. 后续演进建议

- 把 `run_vault_sync(...)` 下沉为独立模块，减少 `bootstrap.rs` 继续膨胀
- 如果要引入真正的异步 provider I/O，先定义 single-writer actor 边界，再迁移 timer 回调
- 若后续支持 macOS / Linux / mobile，优先保持:
  - `CredentialStore`
  - `VaultProvider`
  - `LocalVaultBootstrapState`
  三层抽象不变，只替换平台实现
