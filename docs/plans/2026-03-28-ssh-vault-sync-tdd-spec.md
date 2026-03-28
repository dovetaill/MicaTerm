# SSH Vault Sync TDD Spec

日期: 2026-03-28
来源计划: `docs/plans/2026-03-28-ssh-vault-sync-implementation-plan.md`
来源设计: `docs/plans/2026-03-28-ssh-vault-sync-design.md`
状态: Task 1-11 已完成并通过本地验证，可作为下一阶段 TDD 输入

## 1. 本轮验证结论

本轮已完成以下验证：

```bash
cargo test --test vault_model_spec --test vault_crypto_spec --test vault_snapshot_spec --test vault_bootstrap_spec --test vault_sync_engine_spec --test vault_provider_s3_spec --test vault_provider_github_spec --test vault_provider_gitlab_spec --test vault_provider_gitee_spec --test vault_settings_smoke --test bootstrap_smoke -- --nocapture

cargo test --test credential_store_spec --test ssh_profile_spec --test ui_preferences --test top_status_bar_smoke --test shell_view_model -- --nocapture

cargo check --workspace
cargo clippy --workspace -- -D warnings
```

结果：

- vault 定向套件全部通过；
- 既有 SSH / 设置回归套件全部通过；
- workspace 编译通过；
- clippy 零告警通过。

## 2. 当前核心接口与数据结构

### 2.1 Vault 领域模型

位置：`src/app/vault/model.rs`

核心 struct / enum：

- `VaultHead`
- `VaultManifest`
- `PackRef`
- `VaultSnapshot`
- `VaultAssetCatalog`
- `VaultAssetNode`
- `VaultKnownHostEntry`
- `SnapshotSyncPreferences`
- `SnapshotUiPreferences`
- `BootstrapBundle`
- `BootstrapRemoteConfig`
- `RemoteRole`
- `ProviderKind`
- `ProviderAuthKind`
- `BootstrapRemoteLocator`
- `RemoteHealth`
- `KdfConfig`
- `CipherKind`
- `CompressionKind`
- `PackLayout`

接口职责：

- `VaultHead` 表示当前 head 元数据，承载 revision、manifest 引用、wrapped key、KDF 与 cipher 信息；
- `VaultManifest` 与 `PackRef` 描述密文 pack 集；
- `VaultSnapshot` 承载资产树、SSH secrets、known_hosts、sync 偏好与 UI 偏好；
- `BootstrapBundle` 描述 vault id、remote 列表、remote 认证引用与健康状态。

### 2.2 本地 bootstrap / cache / snapshot

位置：

- `src/app/vault/bootstrap.rs`
- `src/app/vault/cache.rs`
- `src/app/vault/snapshot.rs`

核心 struct / 返回值：

- `ImportedBootstrapBundle`
- `LocalVaultBootstrapState`
- `AppliedVaultSnapshot`

关键函数：

- `save_local_vault_bootstrap_state(...)`
- `load_local_vault_bootstrap_state(...)`
- `export_bootstrap_bundle(...)`
- `import_bootstrap_bundle(...)`
- `store_encrypted_cache(...)`
- `load_encrypted_cache(...)`
- `export_vault_snapshot(...)`
- `apply_vault_snapshot(...)`

当前约束：

- 本地 bootstrap state 固定落盘到 `root_dir/vault-bootstrap-state.json`；
- 本地加密 cache 固定落盘到 `root_dir/cache/...`；
- `known_hosts` 走 `root_dir/known_hosts`；
- 本地 bootstrap state 保存的是 `wrapped_vault_key` 的 JSON 字符串，而不是直接内嵌结构体。

### 2.3 Sync 引擎与 provider 抽象

位置：

- `src/app/vault/engine.rs`
- `src/app/vault/provider/mod.rs`
- `src/app/bootstrap.rs`

核心 struct / trait：

- `SyncRequest`
- `SyncMirrorFailure`
- `SyncReport`
- `SyncError`
- `SyncEngine`
- `ProviderCapabilities`
- `ProviderReadResult`
- `ProviderWriteRequest`
- `VaultProvider`
- `VaultProviderFactory`
- `VaultRuntimeOptions`
- `VaultSessionState`

当前行为契约：

- `SyncEngine::sync(...)` 先读 primary head，再校验 `parent_revision`；
- primary 写入失败时整体失败；
- mirror 写入失败不会回滚 primary，而是记录到 `SyncReport.mirror_failures`；
- `SyncReport::is_mirror_degraded()` 用于 UI 降级提示；
- `ProviderCapabilities.supports_conditional_head_write` 决定 primary 是否启用条件写入；
- 当前 primary / mirror 角色由 `BootstrapBundle.remotes[*].role` 决定。

## 3. 当前 Slint 回调与 UI 绑定面

### 3.1 已落地的 window callback

位置：`ui/app-window.slint`

- `vault-create-requested(string)`
- `vault-unlock-requested(string)`
- `vault-sync-now-requested()`
- `vault-lock-requested()`

### 3.2 已落地的 Rust 绑定入口

位置：`src/app/bootstrap.rs`

- `bind_top_status_bar_with_injected_services_and_vault_runtime(...)`

该入口当前负责：

- 初始化 `VaultSessionState`；
- 启动时从本地 bootstrap state 恢复锁状态展示；
- 将上述 4 个 Slint callback 绑定到 create / unlock / sync / lock 逻辑；
- 把执行结果回写到 `ShellViewModel::vault_panel_state_mut()`。

### 3.3 当前 UI 状态面

位置：`src/shell/view_model.rs`

核心状态：

- `RightPanelView::Vault`
- `VaultPanelViewState`
- `ShellViewModel::vault_panel_state_mut()`

说明：

- 当前 `AppWindow` 层 callback 已接线；
- `RightPanel` 内部控件的完整点击链路不是本轮验证重点；
- 现有 smoke test 主要通过 `invoke_vault_*` 驱动，不依赖真实 pointer/touch 交互。

## 4. Provider 角色验证覆盖

### 4.1 已验证为 Primary 的 provider / 模式

- `S3Compatible`
  - 在 `tests/vault_provider_s3_spec.rs` 中验证了 object key 布局与 conditional head write 能力；
  - 在 `tests/vault_sync_engine_spec.rs` 中验证了 primary 冲突检测、primary read failure、primary write 后再 fan-out mirrors；
  - 在 `tests/bootstrap_smoke.rs` 的本地 runtime smoke 中，primary 使用 `ProviderCapabilities::s3_like()` mock provider。

- `GitHubGist`
  - 在 `tests/vault_provider_github_spec.rs` 中验证了 `RemoteRole::Primary` + `ProviderAuthKind::DeviceFlow` 的配置可表达；
  - 同时验证 gist 读取在文件被截断时会走 `raw_url` 回退。

### 4.2 已验证为 Mirror 的 provider / 模式

- `GitHubGist`
  - `RemoteRole::Mirror` + `ProviderAuthKind::Pat`；
  - bundled files layout；
  - 无 conditional head write。

- `GitLabSnippet`
  - `RemoteRole::Mirror`；
  - 认证降级链：`DeviceFlow -> Pkce -> Pat`；
  - 10 文件上限约束下，当前 `max_pack_count = 8`；
  - 无 strict CAS。

- `GiteeGist`
  - `RemoteRole::Mirror`；
  - 支持 `Pat` 与标准 OAuth code flow；
  - bundled files layout；
  - 无 conditional head write。

## 5. 已人工验证的 bootstrap 恢复路径

来自 `tests/bootstrap_smoke.rs`：

- 新建 vault：
  - 通过 `invoke_vault_create_requested(...)` 创建本地 vault；
  - 成功后 UI 进入 `Unlocked`；
  - `vault-bootstrap-state.json` 成功落盘。

- 解锁已有 vault：
  - 启动时在 locked 状态下不会预加载资产树或 secrets；
  - 调用 `invoke_vault_unlock_requested(...)` 后，会从本地 bootstrap state + encrypted cache 恢复 snapshot；
  - 恢复后资产树与 credential store 均可见。

- 手动同步：
  - primary 成功但 mirror 失败时，UI 会展示 `Mirror degraded: ...`；
  - primary 读失败时，UI 会展示 `Provider auth error: ...` 风格的状态文本。

- 锁定 vault：
  - 调用 `invoke_vault_lock_requested()` 后，内存中的资产树与 SSH secrets 都会被清空；
  - decrypted snapshot / vault key 不再保留在会话状态中。

## 6. 下一阶段 TDD 应优先覆盖的断言

### 6.1 UI 交互链路

- `ui/shell/right-panel.slint` 内部实际按钮事件是否完整转发到 `AppWindow` callback；
- 多次点击 create / unlock / sync / lock 时是否存在重复提交；
- panel 状态文案在 callback 失败后是否始终与实际锁状态一致。

### 6.2 本地恢复与损坏场景

- `vault-bootstrap-state.json` 损坏、字段缺失、`vault_id` 为空；
- encrypted cache 缺失、损坏、cipher header 不匹配；
- 主密码错误时，unlock 不应污染现有 `ShellViewModel`；
- known_hosts 文件不存在或格式异常时，snapshot 导入行为是否仍可部分成功。

### 6.3 同步冲突与 provider 退化

- `parent_revision` 不匹配时，`SyncError::Conflict` 是否稳定映射到 UI；
- primary write 失败后 mirror 不应被触发；
- mirror 部分失败后二次 sync 的 revision 与状态文案是否一致；
- gist / snippet bundled layout 超过 pack 数限制时，错误是否在写入前即被拒绝。

### 6.4 Secret 生命周期

- lock / unlock / re-unlock 循环后，credential store 中不存在陈旧 secret；
- 空 secret 字段不会在 snapshot round-trip 后变成空白字符串；
- 导入 snapshot 时，同一 `credential_ref` 的旧 secret 会被正确替换。

## 7. 已知边缘情况与剩余限制

- `GitHubGist` / `GitLabSnippet` / `GiteeGist` 当前都不提供严格的 compare-and-swap 语义，因此只适合作为 mirror，或在受控条件下作为轻量 primary；
- snippet / gist provider 受 pack 数和单文件体积约束，后续新增字段时需要持续回归 `max_pack_count`；
- 当前 `AppWindow` callback 已接线，但 Right Panel 的真实 UI 点击链路仍应在下一轮 TDD 中补 pointer 级 smoke；
- 当前同步结果直接写回 `VaultPanelViewState.primary_status_label`，未来若改成后台 Tokio 任务，需要确保 UI 回写统一通过 `slint::invoke_from_event_loop`；
- 若后续引入 actor/channel 异步同步，不应在 vault 已锁定后继续消费旧的 decrypted snapshot；
- mirror degraded 目前只做状态提示，不会自动补偿重试，也不会维护单独的 mirror backlog。

## 8. 建议的下一轮测试切入点

建议先从以下 5 组测试开始：

1. `right_panel_vault_callbacks_smoke`
2. `vault_unlock_corrupted_bootstrap_spec`
3. `vault_unlock_wrong_password_spec`
4. `vault_sync_conflict_ui_mapping_spec`
5. `vault_mirror_retry_policy_spec`

这些测试可以直接围绕现有接口编写，不需要先改动领域模型。
