# Asset Sync Git Primary TDD Handoff

Date: 2026-04-01

## Scope

本轮实现把正式同步主路径从 `gist/snippet` 切到 `Gitee Git repo primary`，补齐 `HTTPS credentials / SSH key` 双认证 draft、稳定 `device_id`、`GitRepoProvider`、资产级三方 merge、attach-time merge、冲突 inbox 可见化，以及旧 provider 的兼容回归。

## Core Structs

- `BootstrapRemoteConfig`
  - 正式远端配置入口；首发 primary 使用 `provider: ProviderKind::GitRepo`
- `BootstrapRemoteLocator::GitRepo`
  - Git primary locator，持有 `host_kind / remote_url / branch`
- `GitRepoRemoteDraft`
  - sync modal 的正式 Git draft 数据模型；默认 host 为 `Gitee`
- `LocalVaultBootstrapState`
  - 本地 durable sync state，持久化 `device_id / current_revision / logical_revision / last_successful_push_at / last_successful_pull_at / local_snapshot_hash`
- `GitRepoProvider`
  - 正式 primary provider；负责读取远端 head、读取 revision、拒绝 stale push
- `MergeInput`
  - 三方 merge 输入：`base / local / remote / device_id`
- `MergeResult`
  - merge 输出：`merged / conflicts / recovery_actions`
- `RecoverySnapshotRecord`
  - 冲突恢复快照记录；写入 `recovery/<vault_id>/`
- `ConflictInboxEntry`
  - 冲突 inbox 记录；写入 `conflicts/<vault_id>/`
- `SyncModalViewState`
  - sync modal 的 UI 状态载体；新增 `conflict_count / conflict_summary / conflict_review_available`

## Traits And Interface Contracts

- `VaultProvider`
  - `read_head() -> Result<ProviderReadResult>`
  - `read_revision(&VaultHead) -> Result<ProviderRevision>`
  - `write_revision(&ProviderWriteRequest) -> Result<()>`
  - `prune_revisions(...) -> Result<()>`
- `VaultProviderFactory`
  - `build_provider(&BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>>`
  - `build_provider_for_vault(&BootstrapRemoteConfig, &Path) -> Result<Arc<dyn VaultProvider>>`
  - Git primary 通过 `build_provider_for_vault` 注入 repo cache root
- `CredentialStore`
  - 保存 Git HTTPS secret、SSH key material、runtime vault key
- Git transport auth contract
  - `ProviderAuthKind::HttpsCredentials` -> `username + secret`
  - `ProviderAuthKind::SshKey` -> `private_key + optional passphrase`
  - 首发 formal provider label 固定为 `Gitee`

## Slint Callbacks, Global State, Bindings

### AppWindow Sync Modal Properties

- `sync-modal-git-remote-url`
- `sync-modal-git-branch`
- `sync-modal-git-auth-mode`
- `sync-modal-git-https-username`
- `sync-modal-git-https-secret`
- `sync-modal-git-ssh-private-key`
- `sync-modal-git-ssh-passphrase`
- `sync-modal-conflict-count`
- `sync-modal-conflict-summary`

### Slint Callbacks

- `open-sync-modal-requested`
- `sync-modal-draft-changed`
- `sync-modal-submit-master-password`
- `sync-modal-primary-action-requested`
- `sync-modal-sync-now-requested`
- `sync-modal-close-requested`

### State Projection Rule

- `bootstrap` 负责把 `ShellViewModel::sync_modal_state()` 投影到 `AppWindow`
- `update_sync_modal_for_local_state(...)` 是 sync modal 主入口
- conflict summary 来自本地 `conflicts/` inbox，而不是运行时临时字符串拼接

## Tokio Task / Channel / Actor Interactions

- 本轮功能没有新增独立的 Tokio actor；Git primary 同步仍通过现有 `bootstrap -> SyncEngine` 路径串行触发
- auto-sync / manual sync / attach-time merge 最终都会汇入同一套 `sync_local_vault(...)` 与 `recover_local_vault_from_primary_remote(...)`
- 现有 SSH runtime 仍通过 `tokio::sync::mpsc::UnboundedSender<SessionRuntimeEvent>` 维护终端事件流，但该 channel 不承载 vault sync 状态
- 若后续将 Git transport 完整异步化，UI 更新必须经 `slint::invoke_from_event_loop(...)` 回到 UI 线程，避免跨线程直接改 `ShellViewModel`

## State Flow

1. 用户打开 `Sync Settings`
   - `hydrate_sync_modal_draft(...)` 从本地 bootstrap + credential store 还原 Git repo draft
2. 用户保存 Git primary
   - `build_sync_bundle_from_modal(...)` 生成 `BootstrapBundle`
   - `persist_sync_modal_settings(...)` 写入 bootstrap state 与 credential material
3. 用户提交 master password 或直接 `Sync now`
   - `resolve_remote_for_sync(...)` 内联取回 Git auth secret
   - `VaultProviderFactory::build_provider_for_vault(...)` 创建 `GitRepoProvider`
4. `SyncDecision`
   - `Noop / PullOnly / PushOnly / MergeThenPush`
5. 如果是 attach-time merge 或日常 merge
   - `prepare_remote_snapshot_for_merge(...)`
   - `merge_snapshots(...)`
   - 冲突时同时写 `recovery/` 与 `conflicts/`
6. UI 回写
   - `apply_vault_snapshot_to_shell(...)`
   - `update_sync_modal_for_local_state(...)`
   - sync modal 显示最新 conflict summary

## Error Handling Strategy

- 配置校验错误
  - Git remote URL、branch、HTTPS username/secret、SSH private key 缺失时，直接回写 modal `error_text`
- 远端凭证缺失
  - `resolve_remote_for_sync(...)` 直接报错，不允许 silent fallback
- stale push / non-fast-forward
  - `GitRepoProvider` 返回冲突错误，由上层归并为 `SyncError::Conflict`
- merge 冲突
  - 不静默覆盖；保留 merged 结果、写 recovery snapshot、写 conflict inbox entry
- mirror 失败
  - primary 成功后仍允许继续，但 UI 必须显示 degraded status
- legacy bundled snapshot 元数据缺失
  - 通过 manifest recovery metadata 报清晰错误，不伪造可恢复状态

## Edge Cases

- Tokio channel 阻塞或消息堆积
  - 本轮同步主路径未新增 channel；若未来把 Git transport 放入 Tokio task，必须为结果回传设置 bounded channel 或显式丢弃策略，避免 UI 等待无界积压
- UI 线程更新时机不正确
  - 任何后台 sync 完成后的 modal / asset tree 更新都必须回到 Slint UI 线程；否则会出现状态已变但界面未刷新的问题
- 数据竞争或共享状态不一致
  - `ShellViewModel`、`VaultSessionState`、credential store 的读取顺序必须固定；不能在持久化前后混用旧 `bundle`
- 资源释放时序问题
  - vault lock / app restart 时要先清理 decrypted snapshot 与 runtime key，再关闭 UI 投影，避免残留明文状态
- 异步任务取消或界面关闭后的悬挂回调
  - 关闭 modal 后若仍有后台 sync 结果回写，必须检查 window handle / state 是否仍有效
- Slint model 更新与实际数据源不同步
  - conflict summary 不能只依赖临时内存字符串；必须从本地 `conflicts/` 持久化结果重新投影
- attach-time merge 无共同 base
  - 允许 merge + push，也允许未来降级成 merge preview，但绝不能退回 remote-first 覆盖
- 并发新增但 node id 撞上
  - 通过 remote snapshot 预处理 remap，避免把不同资产错误合并成同一节点

## Suggested Tests

### Unit Tests

- `ConflictInboxEntry` round-trip 与排序加载
- `GitRepoProvider` stale push / branch round-trip / auth contract
- `merge_snapshots(...)` 的 asset delete-vs-modify、union add、keychain reference 保留
- `device_id` 持久化与删除后重建

### Integration Tests

- attach-time merge：本地已有资产再接入已有远端
- manual / periodic sync：remote only、local only、diverged merge
- conflict summary：merge 后 `Sync Settings` 能显示 count + latest summary
- compatibility：`gitee_gist` / `github_gist` / `gitlab_snippet` / `s3` provider 仍可完成既有 payload 契约

### UI / Contract Tests

- `tests/vault_settings_ui_contract_smoke.sh`
  - 确认正式 primary 设置路径不再暴露 `gist/snippet`
- `tests/sync_vault_modal_smoke.rs`
  - 确认 Git draft 字段与 conflict summary 字段都已接入
- `tests/vault_settings_smoke.rs`
  - 确认 titlebar sync 入口始终落到正式 sync modal，而不是旧 vault panel
