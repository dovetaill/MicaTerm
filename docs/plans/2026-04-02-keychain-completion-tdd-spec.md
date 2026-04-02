# Keychain Completion TDD Handoff

## 背景

本轮实现已把 keychain 补齐为与 console assets 等级一致的一等资产模块，覆盖了：

- keychain blank area / node context menu；
- identity / ssh-key modal create/edit flows；
- keychain catalog 本地 repo 持久化；
- vault dirty / export / import / attach-merge / restore 闭环；
- host `auth_source = keychain-identity` 的运行时解析链路。

下一个阶段建议以 TDD 方式围绕回归保护、异常路径和并发边界继续补强。

## 当前核心模块

### 1. Keychain catalog 与 secret ownership

- `src/app/keychain/model.rs`
  - `KeychainCatalog`
  - `KeychainNode`
  - `KeychainIdentitySpec`
  - `KeychainSshKeySpec`
- `src/app/ssh/credentials.rs`
  - `CredentialStore`
  - `keychain_identity_credential_ref(...)`
  - `keychain_key_credential_ref(...)`
- 约束：
  - identity secret 必须落在 `keychain/identity/*`
  - ssh-key secret 必须落在 `keychain/key/*`
  - 不允许回流到 `ssh/saved-secrets/*`

### 2. View model / bootstrap 接口

- `src/shell/view_model.rs`
  - `handle_assets_create_action(...)`
  - `handle_context_menu_leaf_action(...)`
  - `open_new_keychain_identity_modal(...)`
  - `open_edit_keychain_identity_modal(...)`
  - `open_new_keychain_ssh_key_modal(...)`
  - `open_edit_keychain_ssh_key_modal(...)`
  - `create_keychain_item(...)`
  - `rename_keychain_item(...)`
  - `delete_keychain_item(...)`
  - `replace_keychain_catalog(...)`
- `src/app/bootstrap.rs`
  - `mark_local_vault_dirty_and_arm_sync(...)`
  - `save_keychain_catalog_if_available(...)`
  - `replace_vault_projection(...)`
  - `apply_remote_keychain_id_remap(...)`

### 3. Vault snapshot / merge seam

- `src/app/vault/snapshot.rs`
  - `export_vault_snapshot(...)`
  - `apply_vault_snapshot(...)`
  - `normalize_snapshot_secret_refs(...)`
  - `obsolete_keychain_secret_refs(...)`
- `src/app/vault/merge.rs`
  - keychain catalog merge
  - keychain secret bundle merge
  - relationship rebuild

## 已落地的 Slint callback / UI contract

- `ui/app-window.slint`
  - `keychain-identity-modal-draft-changed(string, string)`
  - `keychain-identity-modal-action-requested(string)`
  - `keychain-ssh-key-modal-draft-changed(string, string)`
  - `keychain-ssh-key-modal-action-requested(string)`
  - `assets-create-action-selected(string)`
  - `assets-context-menu-action-invoked(string)`
  - `sidebar-destination-selected(string)`

## 本轮已经锁定的行为

### 1. mutation -> dirty -> sync

- keychain folder direct create 不再漏掉 vault dirty 标记；
- identity / ssh-key create/edit/delete/rename 会保存 keychain repo；
- 成功 mutation 后会复用现有 auto-sync debounce 链路。

### 2. snapshot / restore

- exported snapshot 包含：
  - `keychain_catalog`
  - `keychain_identity_secret_bundles`
  - `keychain_key_secret_bundles`
- apply snapshot 后会恢复：
  - keychain projection
  - identity secret
  - ssh-key secret
  - host -> identity -> key 引用链
- remap 后的 keychain `credential_ref` 会重新 canonicalize 到当前 node id namespace；
- 旧 ref 会在 apply 时清理，避免 stale secret 残留。

### 3. attach merge / collision remap

- concurrent addition 导致 keychain id collision 时：
  - remote identity / key 会 remap 到 `*-remote-merge-*`
  - remote host 的 `keychain_identity_id` 会同步 remap
  - remote identity 的 `ssh_key_id` 会同步 remap
  - secret bundle ownership 会跟随 remap 后的新 node id

## 下一阶段优先补测点

### P0 回归保护

- toolbar `new-folder` 与 context menu `new-folder` 都必须触发：
  - `save_keychain_catalog_if_available(...)`
  - `mark_local_vault_dirty_and_arm_sync(...)`
- identity / ssh-key delete 成功时才允许清理 secret；
- folder 非空、identity 被 host 引用、ssh-key 被 identity 引用时必须拒删。

### P0 snapshot / merge

- 针对 remote remap 后的 snapshot 做 round-trip：
  - export -> apply -> export
  - 确认 secret namespace 不回退到旧 ref
- 针对 attach merge 再加一组多节点 case：
  - folder + identity + key + host 一起冲突
  - 确认 parent/child 结构和 root order 仍然稳定。

### P1 resolver / runtime

- keychain identity 切换 `password <-> ssh-key` 后的 resolver 行为；
- public-only key 被 identity 选中时的连接失败提示是否稳定；
- identity 缺失 / key 缺失 / secret 缺失时 UI 与 runtime 错误文案是否一致。

### P1 UI contract

- keychain modal reopen 后字段 hydration 是否始终与 secret store 对齐；
- create menu / context menu 在 console, snippets, keychain 三面板间切换时不串状态；
- keychain selection/focus 与 console selection/focus 必须继续隔离。

## 并发与边缘情况

- `Tokio` 背景同步与 UI callback 之间不要共享长生命周期可变借用；
- `slint::invoke_from_event_loop(...)` 路径上的状态刷新应避免重复 re-entry；
- auto-sync debounce 需要防止连续 mutation 触发过量 push；
- remap 之后如果只更新 catalog 不更新 secret namespace，会造成 resolver 找到旧 ref；
- apply snapshot 时如果不删除 obsolete keychain refs，会留下 stale secret；
- attach/unlock restore 顺序必须保持：
  1. load/apply snapshot
  2. replace projection
  3. rehydrate modal / focused state

## 建议的 TDD 起手测试集

1. `tests/vault_snapshot_spec.rs`
   - remap 后 keychain ref canonicalization round-trip
2. `tests/vault_attach_merge_spec.rs`
   - 多层 folder + identity + key + host collision merge
3. `tests/bootstrap_smoke.rs`
   - keychain direct create / rename / delete 的 dirty + repo save 闭环
4. `tests/keychain_resolver_spec.rs`
   - public-only key / missing private material / stale ref 错误路径

## 本轮验证记录

- Focused regressions: 全部通过
- `cargo check --workspace`: 通过
- `cargo clippy --workspace -- -D warnings`: 通过
- `cargo test -- --nocapture`: 存在 1 个与 keychain 无关的既有失败
  - `tests/ssh_connection_timeline_spec.rs`
  - 失败点：`host_key_block_keeps_connection_timeline_waiting_for_user`
  - 现象：断言中的 host key detail 文案已带上 `:22 (SHA256:blocked-host-key)`，而旧期望仍是无端口/指纹的短文案

## 交接建议

- 后续 TDD 阶段先不要重构 keychain / console catalog 抽象；
- 优先围绕现有 seam 补测试，尤其是 snapshot normalize、merge remap、secret cleanup；
- 若要处理全量测试中的 timeline failure，请单独开一个非 keychain 任务，避免把 host-key timeline 文案回归混进当前 keychain 验收。
