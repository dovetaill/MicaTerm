# Assets Keychain TDD Spec

日期: 2026-03-30
状态: implementation complete, ready for test-driven follow-up
工作树: `feature/assets-keychain-impl`

## 已落地范围

- 新增独立 `Keychain` 域模型、repository 和 redb store，不再把认证资产混入 `Window Console` 的 `AssetTree`
- `VaultSnapshot` 已支持：
  - `keychain_catalog`
  - `keychain_identity_secret_bundles`
  - `keychain_key_secret_bundles`
- `CredentialStore` 已支持 keychain identity / SSH key 两类 secret bundle
- SSH host 已新增 `auth_source` / `keychain_identity_id`
- `resolve_saved_ssh_profile(...)` 会在 runtime normalization 前把 identity-backed host 展开成现有 `ConnectionProfile`
- `Keychain` 侧边栏、SSH key modal、SSH host modal source switch 已接入现有 Slint + bootstrap 投影链路
- SSH host modal 现在支持：
  - `Manual`
  - `Keychain Identity`
  - keychain identity picker + username + auth summary

## 核心 Rust 结构与接口

### `src/app/keychain/model.rs`

关键结构：

- `KeychainCatalog`
- `KeychainNode`
- `KeychainNodeKind`
- `KeychainNodePayload`
- `KeychainIdentitySpec`
- `KeychainIdentityAuthKind`
- `KeychainSshKeySpec`

关键约束：

- `KeychainNodeKind` / `KeychainIdentityAuthKind` 使用 `kebab-case`
- `KeychainIdentitySpec` 只保存 identity 级元数据，不直接内联 SSH key 内容
- `KeychainSshKeySpec` 只保存 public metadata，私钥正文仍走 `CredentialStore`

### `src/app/keychain/repository.rs`

关键 trait：

- `KeychainCatalogRepository`
  - `load(&self) -> Result<KeychainCatalog>`
  - `save(&self, catalog: &KeychainCatalog) -> Result<()>`

测试建议：

- 下一阶段继续把它当作稳定 repository seam，避免测试直接依赖 redb 文件布局

### `src/app/ssh/credentials.rs`

关键接口：

- `CredentialStore`
  - `put_secret`
  - `get_secret`
  - `delete_secret`

关键 helper：

- `keychain_identity_credential_ref(identity_id)`
- `keychain_key_credential_ref(key_id)`
- `persist_keychain_identity_secret_bundle(...)`
- `load_keychain_identity_secret_bundle(...)`
- `snapshot_keychain_identity_secret_bundle(...)`
- `persist_keychain_key_secret_bundle(...)`
- `load_keychain_key_secret_bundle(...)`
- `snapshot_keychain_key_secret_bundle(...)`

关键结构：

- `StoredKeychainIdentitySecretBundle`
- `StoredKeychainKeySecretBundle`
- 兼容复用的 `StoredSshSecretBundle`

关键不变量：

- identity password 与 key private key 不能回写到 host-owned `ssh/saved-secrets/*`
- keychain-backed host 只允许保留 proxy password；host auth secret 必须来自 resolver 展开后的 keychain credential ref

### `src/app/keychain/resolver.rs`

关键函数：

- `resolve_saved_ssh_profile(asset_id, title, spec, keychain_catalog) -> anyhow::Result<ConnectionProfile>`
- `derive_public_key_material_from_private_key(...)`
- `derive_public_key_material_from_public_key(...)`

resolver contract：

1. `auth_source = manual`:
   - 直接走 `ConnectionProfile::from_saved_asset(...)`
2. `auth_source = keychain-identity`:
   - 必须先解析 `keychain_identity_id`
   - password identity -> `auth_method = password`
   - ssh-key identity -> `auth_method = private-key` + `private_key_source = content`
   - 展开后再走现有 `ConnectionProfile::from_saved_asset(...)`

当前错误分支必须保持稳定：

- identity 缺失
- identity username 为空
- identity 缺少 `ssh_key_id`
- SSH key 节点缺失

### `src/shell/keychain.rs`

关键结构：

- `KeychainItemKind`
- `KeychainCreateAction`
- `VisibleKeychainRow`
- `RemovedKeychainSummary`
- `KeychainDeleteError`

关键函数：

- `create_keychain_node(...)`
- `rename_keychain_node(...)`
- `delete_keychain_node(...)`
- `next_default_name_for_parent(...)`
- `project_keychain_rows(...)`

删除约束：

- 非空 folder 不能删
- 被 SSH host 引用的 identity 不能删
- 被 identity 引用的 SSH key 不能删

### `src/shell/view_model.rs`

关键结构：

- `AssetSshConnectionDraft`
  - 新字段：`auth_source`, `keychain_identity_id`
- `KeychainSshKeyDraft`
- `AssetModalState`

关键接口：

- `open_new_ssh_modal(...)`
- `open_edit_ssh_modal(...)`
- `update_ssh_modal_field(...)`
- `ssh_keychain_identity_option_labels()`
- `ssh_keychain_identity_selected_label()`
- `ssh_keychain_identity_selected_username()`
- `ssh_keychain_identity_selected_auth_summary()`
- `create_keychain_item(...)`
- `rename_keychain_item(...)`
- `delete_keychain_item(...)`

关键行为：

- 切到 `keychain-identity` 时会清空：
  - `password`
  - `private_key_content`
  - `passphrase`
- 切回 `manual` 时保留 legacy 兼容字段：
  - `auth_method`
  - `private_key_source`
  - `private_key_path`
  - `user`

## UI / Slint 数据链路

### `AppWindow` 关键 callbacks

- `asset-ssh-modal-draft-changed(string, string)`
- `asset-ssh-modal-action-requested(string)`
- `keychain-identity-modal-draft-changed(string, string)`
- `keychain-identity-modal-action-requested(string)`
- `keychain-ssh-key-modal-draft-changed(string, string)`
- `keychain-ssh-key-modal-action-requested(string)`

### SSH host modal 关键属性

- `asset-ssh-modal-auth-source`
- `asset-ssh-modal-keychain-identity-options`
- `asset-ssh-modal-keychain-identity-selected-label`
- `asset-ssh-modal-keychain-identity-username`
- `asset-ssh-modal-keychain-identity-auth-summary`

### Keychain explorer 关键投影

- `active-sidebar-destination`
- `keychain-asset-items`
- `assets-create-action-selected("new-identity")`
- `assets-create-action-selected("new-ssh-key")`

### 当前 callback wiring 现状

- `asset-ssh-modal-*` 已在 `src/app/bootstrap.rs` 中接通
- `keychain-ssh-key-modal-*` 已在 `src/app/bootstrap.rs` 中接通
- `keychain-identity-modal-*` 当前只有 `AppWindow` 属性和 callback contract，尚未看到对应 bootstrap wiring

下一阶段如果要补 identity 新建/编辑行为，建议先为这两条 callback 写红测，再接 `ShellViewModel`

## 已确认的测试覆盖

focused keychain 集合：

- `tests/keychain_model_spec.rs`
- `tests/keychain_store_spec.rs`
- `tests/keychain_secret_store_spec.rs`
- `tests/keychain_resolver_spec.rs`
- `tests/keychain_projection_spec.rs`
- `tests/keychain_modal_smoke.rs`
- `tests/keychain_key_actions_spec.rs`

兼容性 / 回归集合：

- `tests/assets_modal_smoke.rs`
- `tests/shell_view_model.rs`
- `tests/ssh_profile_spec.rs`
- `tests/ssh_session_manager_spec.rs`
- `tests/bootstrap_smoke.rs`
- `tests/vault_snapshot_spec.rs`
- `tests/keychain_ui_contract_smoke.sh`
- `tests/assets_modal_ui_contract_smoke.sh`
- `tests/sidebar_ui_contract_smoke.sh`

## 下一阶段优先补测点

### 1. Keychain identity modal 行为测试

建议新增：

- draft 默认值
- `Password` / `SSH Key` 切换
- `ssh_key_id` label -> id 解析
- confirm/save 后 catalog + secret bundle 双写

原因：

- 当前 identity modal 只有 UI contract，缺少 bootstrap -> view model -> persistence 的行为回归

### 2. identity-backed host 的 host-owned secret 隔离

建议新增：

- keychain-backed host save/edit 后，`ssh/saved-secrets/<asset>` 不应残留 password/private key/passphrase
- 仅 SOCKS5/HTTP proxy password 允许继续走 host-owned secret bundle

### 3. resolver + vault restore 混合场景

建议新增：

- snapshot restore 后立即对 identity-backed host 执行 `save-and-connect`
- manual host 与 identity-backed host 混合存在时，resolver 不应污染手动 host 的 `credential_ref`

### 4. explorer 删除阻塞的 UI 反馈

建议新增：

- 删除被 host 引用的 identity 时的用户可见错误
- 删除被 identity 引用的 SSH key 时的用户可见错误
- folder 非空时的 modal / toast / inline feedback contract

### 5. runtime / channel 压力边界

建议新增：

- `save-and-connect` 与 repeated `test` 并发触发时，busy state 是否正确阻止重入
- keychain-backed host 在 `SessionRuntimeEvent::SurfaceDirty` 高频场景下不会丢失 active tab surface
- unknown host key prompt 与 identity-backed host 重试链路

## 关键边缘情况

### 1. keychain-backed host 与 legacy path host 的兼容切换

风险：

- 如果在 `manual` <-> `keychain-identity` 切换时错误清空 `private_key_path` / `auth_method`，会破坏旧资产编辑兼容性

### 2. keychain 删除与引用完整性

风险：

- SSH host 持有 `keychain_identity_id`
- identity 持有 `ssh_key_id`
- 任一删除链路遗漏检查，都会产生悬挂引用

### 3. vault snapshot 默认值兼容

风险：

- 旧 snapshot / 旧资产缺少 `auth_source` / `keychain_identity_id`
- 反序列化必须默认回 `manual` / `None`

### 4. runtime event backlog

风险：

- `SessionManager` 使用 `mpsc::UnboundedSender<SessionRuntimeEvent>`
- 高频 `SurfaceDirty` / `SurfaceChanged` 场景下，需要继续依赖 coalescing 逻辑避免 UI 卡顿或旧 surface 覆盖新 surface

### 5. secret namespace 污染

风险：

- `ssh/saved-secrets/*`
- `keychain/identity/*`
- `keychain/key/*`

三类 namespace 必须继续隔离，否则会造成 restore/delete/rotation 语义混乱

