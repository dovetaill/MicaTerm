# Keychain Completion Design

日期: 2026-04-01
功能: `keychain-completion`
状态: 已确认，可进入 implementation planning

## 背景

当前仓库已经有了独立的 `KeychainCatalog`、`CredentialStore` namespace、`VaultSnapshot.keychain_*` 字段，以及 SSH host 的 `auth_source = keychain-identity` 解析链路，但功能仍然处于“半接通”状态：

- `New Identity` 仍然直接插入空节点，没有进入 identity modal；
- keychain 空白区和 keychain 节点右键菜单没有形成 keychain 专用语义，仍会落回 console / blank area 菜单；
- `identity` / `ssh-key` 行图标仍复用了 SSH connection 图标；
- keychain catalog 没有接入和 console assets 等价的本地 repo 持久化链路，导致重启后数据丢失；
- keychain mutation 没有完整走 dirty/save/sync 链路，导致 vault/sync 行为不完整。

因此这次的目标不是再“补几个按钮”，而是把 keychain 从占位实现补成和 console assets 同等级的一等资产模块。

## 本轮目标

- 补齐 keychain explorer 的空白区右键、节点右键、正确图标、创建入口与叶子编辑行为；
- 补齐 identity 的新建 / 编辑 / 删除 / 重命名 / 引用约束；
- 补齐 SSH key 的导入私钥、粘贴私钥、导入公钥、粘贴公钥、生成 key pair、复制公钥；
- 接通 keychain catalog 的本地持久化，确保重启不丢；
- 接通 keychain mutation 的 vault dirty / export / import / restore 流程；
- 保持现有 SSH runtime / resolver / legacy manual host 兼容，不做破坏式重写。

## 非目标

- 不把 console、snippets、keychain 三套 catalog 重构成统一泛型资产框架；
- 不引入 SSH agent、FIDO2、certificate、hardware token；
- 不自动迁移所有旧 SSH host 到 keychain identity；
- 不改动 vault snapshot 协议结构，只补齐现有 keychain section 的使用闭环；
- 不在本轮改变 `CredentialStore` 的 backend 策略。

## 现状判断

### 已经存在的基础

- `src/app/keychain/model.rs`、`src/app/keychain/repository.rs`、`src/app/keychain/redb_store.rs` 已经定义了独立 keychain catalog 模型与本地 store；
- `src/app/ssh/credentials.rs` 已经有：
  - `keychain/identity/*`
  - `keychain/key/*`
  两类 secret namespace；
- `src/app/vault/snapshot.rs` 已经导出 / 导入：
  - `keychain_catalog`
  - `keychain_identity_secret_bundles`
  - `keychain_key_secret_bundles`
- `src/app/keychain/resolver.rs` 已经能在 SSH 运行前把 identity-backed host 展开为现有 `ConnectionProfile`；
- `ui/components/assets-keychain-identity-modal.slint`、`ui/components/assets-keychain-ssh-key-modal.slint`、`ui/app-window.slint` 已经定义了 modal UI contract。

### 当前缺口

- `src/shell/view_model.rs` 只有 `AssetModalState::NewKeychainSshKey`，没有 `NewKeychainIdentity`，导致 `new-identity` 不走 modal；
- `src/app/bootstrap.rs` 已接通 `keychain-ssh-key-modal-*`，但没有接通 `keychain-identity-modal-*`；
- `src/shell/context_menu.rs` / `src/app/bootstrap.rs` / `ui/shell/assets-sidebar.slint` 还没有形成 keychain 专属空白区 / 节点 context menu 路由；
- `ui/components/asset-node-row.slint` 没有 identity / ssh-key 专属图标映射；
- keychain mutation 后只会保存 console `AssetCatalogRepository`，不会保存 `KeychainCatalogRepository`；
- keychain mutation 虽然部分会触发 vault dirty，但没有覆盖 identity create/edit/delete、repo save、secret cleanup 等完整闭环。

## 核心设计决策

### 1. 数据分层保持现有架构，不混回 console asset tree

继续维持：

- `KeychainCatalog` 保存目录树、identity 元数据、ssh key 元数据和引用关系；
- `CredentialStore` 保存 password / private key / passphrase 等 secret；
- vault snapshot 继续通过：
  - `keychain_catalog`
  - `keychain_identity_secret_bundles`
  - `keychain_key_secret_bundles`
  三段导出导入。

不把 keychain secret 混回 `ssh/saved-secrets/*`，避免 host-owned secret 与 keychain-owned secret 语义混线。

### 2. Identity 必须先编辑后创建

`New Identity` 不再直接调用 `create_keychain_item(...)` 插空节点，而是：

1. 打开 identity modal；
2. 用户填写 `name / username / auth_kind / password or ssh_key / remark`；
3. 点击确认后再创建 catalog 节点；
4. 若为 password identity，则同步写入 `CredentialStore`；
5. 若为 ssh-key identity，则只保存 `ssh_key_id` 引用。

编辑 identity 复用同一个 modal；编辑时需要回填现有 metadata，并在 password 模式下从 `CredentialStore` 读取 secret 进入 draft。

### 3. SSH Key modal 继续作为 key material 的唯一录入入口

`New SSH Key` / `Edit SSH Key` 复用现有 SSH key modal，并补齐动作：

- `import-private-key`
- `paste-private-key`
- `import-public-key`
- `paste-public-key`
- `generate-key-pair`
- `copy-public-key`

默认生成算法使用 `ed25519`。导入或粘贴私钥时尽可能自动派生公钥与 fingerprint；只录入公钥时允许保存 metadata，但若后续 identity 需要把它作为认证 key，则校验应失败，因为缺失私钥 secret。

### 4. Keychain explorer 使用专属 context menu 语义

Keychain 面板的空白区右键只展示：

- `New Folder`
- `New Identity`
- `New SSH Key`

Keychain 节点右键按节点类型区分：

- folder：`New Folder` / `New Identity` / `New SSH Key` / `Rename` / `Delete`
- identity：`Edit` / `Rename` / `Delete`
- ssh key：`Edit` / `Rename` / `Delete` / `Copy Public Key`

双击行为：

- folder：展开 / 折叠
- identity：打开编辑 modal
- ssh key：打开编辑 modal

### 5. 本地持久化必须把 keychain repo 视为一等存储

新增与 console assets 等价的 keychain repo 流程：

- 启动时加载 `RedbKeychainCatalogStore`；
- 成功后注入 `ShellViewModel.replace_keychain_catalog(...)`；
- create / edit / rename / delete folder、identity、ssh key 后立刻保存 `KeychainCatalogRepository`；
- identity / key secret 也在确认时写入 `CredentialStore`；
- 删除 identity / key 时同步清理相应 namespace secret；
- 删除约束继续保持：
  - 非空 folder 不能删；
  - 被 host 引用的 identity 不能删；
  - 被 identity 引用的 ssh key 不能删。

### 6. Vault / sync 继续复用现有 snapshot 管线

不新增新的 snapshot section，只补齐 mutation -> dirty -> export -> import -> projection restore 的完整闭环：

- keychain mutation 后调用现有 `mark_local_vault_dirty_and_arm_sync(...)`；
- export 继续基于 `state.keychain_catalog()` 和 `CredentialStore` 导出；
- apply / remote sync 后通过现有 `replace_vault_projection(...)` 恢复 keychain catalog；
- merge 后保持：
  - folder 树结构
  - identity -> ssh key 引用
  - host -> identity 引用
  三类关系完整。

## 详细交互设计

### Keychain 面板

- 顶部 `+` 在 `Keychain` 面板下始终使用 create popover，不再尝试“直接创建”；
- 空白区右键要像 console assets 一样可用，但菜单内容换成 keychain 专属；
- 搜索、展开 / 折叠、树视图沿用现有 explorer 语言。

### 图标

- folder：继续 folder icon
- identity：换成 person / credential 语义 icon
- ssh key：换成 key 语义 icon

禁止继续把 `identity` / `ssh-key` 映射到 `window-console` 图标。

### Identity modal

字段：

- `Name`
- `Username`
- `Auth Kind`
- `Password` 或 `SSH Key`
- `Remark`

规则：

- `Password` 模式显示 password input；
- `SSH Key` 模式显示 key picker，选项来自当前 keychain catalog 内所有 SSH key；
- 允许从 `Password` 切到 `SSH Key`，也允许反向切换；
- 切换认证模式时，只清理不再适用的 secret draft，不清理 `name / username / remark`。

### SSH Key modal

字段：

- `Name`
- `Private Key`
- `Public Key`
- `Fingerprint`

动作：

- 导入 / 粘贴私钥
- 导入 / 粘贴公钥
- 生成 key pair
- 复制公钥

保存时：

- metadata 写回 `KeychainCatalog`
- private key / passphrase 写入 `CredentialStore`

### SSH host modal 联动

- 继续保留 `Manual` 与 `Keychain Identity` 两种来源；
- 当 `auth_source = keychain-identity` 时，host 只保存 `keychain_identity_id`；
- username 与认证摘要从 selected identity 投影，不在 host 侧复制一份 secret；
- identity 更新后，host 下次连接时自动使用最新 identity / key 数据。

## 错误处理

- identity name 为空、username 为空：禁止保存；
- password identity 缺密码：禁止保存；
- ssh-key identity 缺 `ssh_key_id`：禁止保存；
- 导入非法私钥 / 公钥：保留用户输入，展示解析失败错误；
- `Copy Public Key` 时没有 public key：按钮禁用或给出提示；
- 删除被引用 identity / key：阻止删除并显示引用原因；
- resolver 遇到 identity 缺失 / key 缺失 / username 为空时，仅阻止该 host 连接，不影响 keychain 面板加载。

## 兼容性

- 老 snapshot 缺少 keychain section：按空 keychain 处理；
- 老 host 的 manual auth 保持不变；
- 现有 `resolve_saved_ssh_profile(...)` contract 保持稳定；
- secret namespace 继续隔离：
  - `ssh/saved-secrets/*`
  - `keychain/identity/*`
  - `keychain/key/*`

## 验收标准

- Keychain 空白区右键有效，且只出现 keychain 菜单；
- `New Identity` 打开 modal，不再直接创建空 identity；
- `Identity` 支持新建 / 编辑 / 删除 / 重命名；
- `SSH Key` 支持导入私钥、粘贴私钥、导入公钥、粘贴公钥、生成 key pair、复制公钥；
- keychain 节点使用正确图标；
- 重启后 keychain folder / identity / key 不丢；
- keychain mutation 会进入 vault dirty / sync；
- sync / restore 后 keychain catalog、identity secret、key secret、host 引用关系完整；
- host 使用 keychain identity 时，连接仍走现有 resolver/runtime 主路径。

## 主要落点文件

- UI / Slint
  - `ui/app-window.slint`
  - `ui/shell/assets-sidebar.slint`
  - `ui/components/asset-node-row.slint`
  - `ui/components/assets-keychain-identity-modal.slint`
  - `ui/components/assets-keychain-ssh-key-modal.slint`
- Shell / ViewModel
  - `src/shell/context_menu.rs`
  - `src/shell/view_model.rs`
  - `src/shell/keychain.rs`
- Bootstrap / persistence / sync
  - `src/app/bootstrap.rs`
  - `src/app/keychain/repository.rs`
  - `src/app/keychain/redb_store.rs`
  - `src/app/vault/snapshot.rs`
- 测试
  - `tests/keychain_modal_smoke.rs`
  - `tests/keychain_key_actions_spec.rs`
  - `tests/assets_context_menu_smoke.rs`
  - `tests/assets_modal_smoke.rs`
  - `tests/bootstrap_smoke.rs`
  - `tests/keychain_store_spec.rs`
  - `tests/vault_snapshot_spec.rs`
  - `tests/vault_bootstrap_spec.rs`
  - `tests/keychain_ui_contract_smoke.sh`
