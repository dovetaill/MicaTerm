# SSH Vault Sync Design

日期: 2026-03-28
方案名: `ssh-vault-sync`
状态: 已确认，可进入 implementation planning

## 背景

当前仓库已经具备 SSH 配置与敏感信息分离的基础能力：

- `src/app/ssh/credentials.rs` 已经有 `CredentialStore`、`StoredSshSecretBundle`、文件型 store、系统 keyring store；
- `src/app/ssh/profile.rs` 已经把 SSH draft 归一化为运行时 profile，并明确区分元数据与 secret；
- `src/app/bootstrap.rs` 负责 UI 与运行时桥接，但当前默认共享 credential store 仍偏向本地文件缓存，而不是默认 OS keychain；
- `src/app/ui_preferences.rs` 目前只持久化少量 UI 偏好；
- `ui/shell/right-panel.slint` 还比较空，适合承载新的 `Sync & Vault` 设置面板。

用户本次需求很明确：

- 希望把 SSH 配置与密码、私钥、passphrase 一起同步到远端；
- 远端后端希望覆盖 GitHub / GitLab / Gitee 代码片段，以及 S3 / 对象存储；
- 必须支持额外加密密码，远端只能看到密文；
- 使用体验上希望接近 Termius 那种“多设备即开即用”，也希望参考 Tabby 那类“统一同步入口”；
- 如果可以，连 provider 凭证也希望同步，减少换设备时重新配置的成本。

## 调研结论

### 1. 业界主流做法

更接近行业共识的做法，不是“把 SSH 配置明文丢进某个 snippet”，而是：

- 把远端服务当作密文载体；
- 客户端掌握主密码；
- 客户端本地派生密钥并完成端到端加密；
- 远端只保存密文与少量同步元数据；
- 本地优先使用 OS keychain 存储 bootstrap 凭证和短期解锁材料。

Termius 官方文档已经明确走的是这条路线：个人 vault 使用混合加密，主密码不随数据上云，云端只是同步密文。Tabby 方向更接近“中心服务或自建 tabby-web 同步配置”，但我没有找到同等完整的官方 E2EE 设计文档，因此这里只把它当作产品交互参考，而不直接照搬其信任模型。

### 2. 多后端同时启用时，不应做异构多主写入

用户希望“都同步”，但从工程与安全角度，不能把 GitHub Gist、GitLab Snippet、Gitee Gist、S3 同时当成平等多主库。

更稳妥的做法是：

- 一个 `Primary Remote` 负责版本推进与冲突检测；
- 若干 `Mirror Remotes` 负责异步镜像与灾备；
- 只有 primary 参与“当前 head 是谁”的判断；
- mirror 不做并发仲裁，只复制同一批密文对象。

这是因为：

- S3 / 对象存储可以利用条件写入和版本控制；
- GitHub / GitLab / Gitee snippet/gist 更像代码片段托管，缺少真正强约束的 compare-and-swap；
- 把这些后端当作平等多主会把冲突处理复杂度拉到不必要的高度。

### 3. 各 provider 的现实约束

#### S3 / S3-compatible object storage

- 最适合作为 primary；
- 支持标准 credential chain，便于桌面和云环境复用现有身份；
- 适合大对象和版本控制；
- 可利用条件请求 / 条件写入做乐观并发控制；
- 还能通过自定义 endpoint 支持 MinIO、Ceph、部分兼容 S3 的对象存储。

#### GitHub Gist

- 适合作为个人便捷同步或 mirror；
- “secret gist” 不是严格私密边界，只能当密文载体；
- 官方 REST API 返回单文件时，内联内容最多 1 MB，超出会标记 `truncated`；
- 文件超过 10 MB 时，获取完整内容需要依赖 `git_pull_url`；
- 顶层文件列表超过 300 个也会被截断。

这意味着 GitHub Gist 不能假设“无限小分块”，需要控制 pack 数量和单文件尺寸。

#### GitLab Snippet

- 适合作为 mirror 或小中型 vault 的 primary；
- 官方文档说明 snippets repository 默认大小上限 50 MB；
- 官方文档说明 snippet repository 最多 10 个文件；
- 新版 GitLab 支持 device authorization grant，但不能假设所有自建实例都足够新；
- 因此认证上要准备 `device flow -> PKCE browser flow -> PAT` 的降级链路。

GitLab 的 10 文件限制是个关键约束，因此它不适合直接映射成“很多小 chunk 文件”。

#### Gitee Gist

- 适合中国用户做镜像或轻量 primary；
- 官方 Swagger 和 OAuth 文档说明 Gist API 与 OAuth 能力存在；
- 但我没有找到像 GitHub / GitLab 那样清晰、稳定的 device flow 资料；
- 因此最稳的首发方式应是 `PAT first`，再加标准 OAuth code flow。

Gitee 的公开配额与 gist 细粒度限制资料不如 GitHub / GitLab 完整，所以首版要把它视为“小中型载体”，避免把最重的并发语义压在它身上。

## 目标

- 提供一个 `Personal Vault`，把 SSH 配置、密码、私钥、passphrase、代理密码等一起做端到端加密同步；
- 支持多个远端后端：
  - `S3 / S3-compatible Object Storage`
  - `GitHub Gist`
  - `GitLab Snippet`
  - `Gitee Gist`
- 支持多后端同时启用，但采用 `1 primary + N mirrors`；
- 让换设备时只需要：
  - 获取 bootstrap；
  - 输入主密码；
  - 拉取并解密 vault；
- 不让远端后端看到任何 SSH 明文；
- 本地优先使用 OS keychain，避免把同步后的明文 secret 常驻磁盘。

## 非目标

- 本轮不做团队共享 vault；
- 本轮不做细粒度多用户 ACL；
- 本轮不做 SSH agent forwarding、FIDO2、硬件密钥同步；
- 本轮不做完整 CRDT；
- 本轮不支持“所有后端平等多主同时写入”；
- 本轮不把 provider 的登录密码直接塞进同一个远端载体里做自举闭环。

## 方案比较

### 方案 A：直接把配置 JSON 明文同步到 gist / snippet / object

优点：

- 实现快；
- 调试简单；
- 迁移成本低。

缺点：

- 不满足“必须安全”的核心要求；
- 远端一旦泄露就是全量明文泄露；
- snippet / gist 的 secret/private 语义不等于加密；
- 不符合业界对密钥、密码、SSH 私钥处理的基本预期。

### 方案 B：单文件密文快照

把全部配置打成一个大 JSON / bincode，整体压缩、整体加密、整体上传。

优点：

- 实现简单；
- 对 gist / snippet 很友好；
- 初期读写逻辑直观。

缺点：

- 任意小改动都会重写整份大对象；
- 后端差异化能力无法利用；
- 很难在 S3 上做更细的对象校验、镜像与增量优化；
- 后续扩展到 bootstrap、镜像、多 pack 会越来越别扭。

### 方案 C：逻辑上 manifest + encrypted packs，物理上按 provider 能力打包

这是最终推荐方案。

逻辑层：

- `VaultHead`
- `VaultManifest`
- `VaultSnapshot`
- `EncryptedPack[]`

物理层：

- 对象存储类后端按对象集写入；
- snippet / gist 类后端按 provider 能力打包成少量 pack 文件；
- 对用户而言仍然是一个统一的 vault。

优点：

- 保留统一的逻辑模型；
- 能同时兼容 S3 与 snippet/gist；
- 可以为不同后端选最合适的上传布局；
- 为 primary / mirror、多 pack、恢复、版本演进留足空间。

缺点：

- 比单大文件方案复杂；
- 需要做 provider capability 抽象；
- 需要更严谨的同步与冲突设计。

## 最终决策

采用方案 C，并进一步收敛为以下产品原则：

- 一个逻辑 vault；
- 一个 primary remote；
- 零个或多个 mirror remotes；
- 逻辑上始终是 `head -> manifest -> encrypted packs`；
- 物理上由 provider 根据能力决定 pack 数量与对象布局；
- 主密码只在本地使用；
- provider 只搬运密文，不参与解密。

## 总体架构

### 1. Bootstrap 层

bootstrap 的职责是“让一个新设备知道去哪里取密文，以及如何认证到远端”。

它不直接保存 SSH 主数据，而是保存：

- vault id；
- 远端列表与角色；
- 每个远端的定位信息：
  - S3 bucket / prefix / endpoint / region / path-style；
  - GitHub gist id；
  - GitLab project / snippet id / base URL；
  - Gitee gist id；
- provider 凭证引用；
- 上次成功同步的远端健康信息；
- 是否启用自动同步；
- bootstrap 自身的加密包装信息。

bootstrap 必须支持三种形态：

- `Local Keychain Bootstrap`
  - 默认；
  - 凭证材料放 OS keychain；
  - 适合同机长期使用。
- `Bootstrap Export File`
  - 换设备、离线备份、灾难恢复；
  - 文件本身也要加密；
  - 用户可以手工拷贝。
- `Provider-auth Bootstrap`
  - 通过某个 provider 的登录流程重新拿到远端访问权；
  - 再用主密码解锁 vault。

### 2. Vault 层

vault 才是被端到端加密的真实业务数据。

建议拆成三个逻辑结构：

#### `VaultHead`

小型明文头对象，字段只包含：

- `format_version`
- `vault_id`
- `vault_revision`
- `parent_revision`
- `device_id`
- `created_at`
- `payload_hash`
- `manifest_ref`
- `wrapped_vault_key`
- `kdf`
- `cipher`
- `compression`
- `pack_layout`

它的作用是：

- 告诉客户端当前 head 是哪一版；
- 告诉客户端如何从主密码派生 KEK；
- 告诉客户端去哪里取 manifest。

#### `VaultManifest`

加密对象，描述：

- pack 列表；
- 每个 pack 的对象名、大小、digest；
- snapshot schema version；
- feature flags；
- provider capability fallback 信息。

#### `VaultSnapshot`

解密后的业务快照，建议至少包含：

- 资产目录：
  - folders
  - SSH connections
  - proxy settings
  - remarks
- SSH secret bundles：
  - password
  - private_key_content
  - passphrase
  - proxy_socks5_password
- known hosts；
- 同步相关 UI 偏好：
  - 自动同步开关
  - 选中的 primary / mirror
  - 最近一次同步结果
- 可选的轻量 UI 偏好：
  - `theme_mode`
  - `always_on_top`

首版不建议把所有工作区会话状态也放进 vault，因为那不是“安全同步 SSH 配置”的核心路径。

### 3. Local Secure Cache

本地必须有一个加密 cache，而不是每次启动都从远端重新抓全量。

建议：

- 解锁后把当前 vault snapshot 写入本地 encrypted cache；
- cache 使用 `vault key` 或本地 keychain 包装后的 `vault key`；
- 如果 OS keychain 可用，优先把本地 unwrap 所需材料交给 keychain；
- 如果 keychain 不可用，只保留加密 cache，不保留明文快照。

这样可以兼顾：

- 启动速度；
- 离线可用性；
- 不在磁盘长驻明文 secret。

### 4. Sync Engine

`SyncEngine` 负责：

- 从 primary 读取 `VaultHead`；
- 检查本地 `parent_revision` 是否匹配；
- 拉取并解密 manifest 与 packs；
- 生成新的 head；
- 先写 primary，再 fan-out 到 mirrors；
- 汇总同步状态给 UI。

建议模型：

- primary 成功才算“此次 revision 提交成功”；
- mirror 失败只影响健康状态，不回滚 primary；
- mirror 可稍后重试。

## 加密设计

### 1. 密钥层级

采用混合加密：

- 用户输入 `master password`；
- `Argon2id` 从主密码派生 `KEK`；
- 每个 vault 持有一个随机生成的 `vault key`；
- `vault key` 被 `KEK` 包装成 `wrapped_vault_key`；
- 真正的 manifest / packs 都由 `vault key` 加密。

这样做的好处是：

- 更换主密码时不需要重加密全部业务数据；
- 只需要重新包装 `vault key`；
- 远端永远拿不到明文 `vault key`。

### 2. 算法选择

建议首版采用：

- KDF: `Argon2id`
- AEAD: `XChaCha20-Poly1305`
- Compression: `zstd`
- Hash: `SHA-256`

顺序固定为：

- `serialize -> zstd compress -> encrypt`

而不是：

- `encrypt -> compress`

因为密文不可压缩，而且先压缩再加密更符合大多数安全工具实践。

### 3. 参数要求

建议把 Argon2id 参数写入 `VaultHead`：

- `memory_cost`
- `time_cost`
- `parallelism`
- `salt`

这样后续可以平滑升级参数，而不用猜测旧 vault 的 KDF 配置。

### 4. 本地内存处理

建议实现上尽量使用：

- `secrecy::SecretString`
- `zeroize`

目标不是“Rust 自动就安全”，而是尽量减少：

- 日志输出 secret；
- debug 格式化泄露；
- 长生命周期明文缓冲区。

## Provider 能力模型

需要明确区分“逻辑对象模型”和“物理承载能力”。

建议 provider trait 提供能力声明：

```rust
pub struct ProviderCapabilities {
    pub supports_conditional_head_write: bool,
    pub max_pack_count: usize,
    pub max_pack_bytes: usize,
    pub preferred_pack_strategy: PackStrategy,
}
```

其中：

- `PackStrategy::ObjectSet`
  - 适合 S3；
  - `head.json + manifest.bin + pack-*.bin`
- `PackStrategy::BundledFiles`
  - 适合 GitHub / GitLab / Gitee；
  - provider 自己决定一个 revision 对应几个 pack 文件；
  - 目标是低文件数、低 API 次数。

### 推荐能力配置

#### S3 / S3-compatible

- `supports_conditional_head_write = true`
- `preferred_pack_strategy = ObjectSet`
- 可作为 primary 默认推荐项

#### GitHub Gist

- `supports_conditional_head_write = false`
- `preferred_pack_strategy = BundledFiles`
- pack 文件数要严格控制；
- 默认更适合 mirror，或轻量 personal vault 的 primary

#### GitLab Snippet

- `supports_conditional_head_write = false`
- `preferred_pack_strategy = BundledFiles`
- 受 10 文件限制，更需要少 pack

#### Gitee Gist

- `supports_conditional_head_write = false`
- `preferred_pack_strategy = BundledFiles`
- 首版保守处理，不把最大规模数据压给它

## 为什么 provider 凭证不能直接并进主 vault

用户希望“连 provider 凭证也同步”，这个方向合理，但不能无脑把它们塞进同一套主 vault 数据里。

问题在于 bootstrapping loop：

- 如果 GitHub token 只存在于 GitHub Gist 里的密文里；
- 而读取这份密文又需要 GitHub token；
- 新设备会卡死在“先有鸡还是先有蛋”。

因此需要把 provider 凭证从主 vault 中拆出去，放进 `BootstrapBundle`。

### `BootstrapBundle`

它是一个独立的、也要加密的材料包，里面可以包含：

- primary / mirror 配置；
- provider access token / refresh token / static key；
- token 过期时间；
- provider 认证模式：
  - device flow
  - PKCE
  - PAT
  - AWS standard chain
- 最近一次成功认证方式。

### Provider 凭证同步规则

建议采用以下规则：

- 默认：provider 凭证只保存在本地 keychain；
- 可选：导出加密 bootstrap file；
- 可选：把 bootstrap bundle 同步到“另一个不依赖该 token 的 carrier”；
- 不建议：唯一 bootstrap 只存放在自己依赖的同一个 provider 上。

举例：

- S3 vault 可以把 bootstrap bundle 额外导出到本地文件；
- GitHub Gist vault 可以再配一个 S3 mirror 或本地 bootstrap file；
- 不建议只保留“GitHub token -> GitHub gist -> vault”这一条单链路。

## 认证策略

### GitHub

推荐：

- 首选 `OAuth device flow`
- 备选 `fine-grained PAT`

原因：

- 桌面应用体验更好；
- 避免直接让用户粘贴长期经典 PAT；
- 但 PAT 依然要保留作 fallback。

### GitLab

推荐：

- 首选 `device flow`
- 若实例不支持，降级到 `browser code flow + PKCE`
- 最后再保留 `PAT`

### Gitee

推荐：

- 首选 `PAT`
- 备选 `OAuth code flow`

原因：

- 公开资料里没有足够强的 device flow 证据；
- 对中国用户来说，PAT 首发更稳。

### S3 / Object Storage

推荐：

- 首选 `standard credential chain`
- 备选手工 access key / secret key

标准 credential chain 能覆盖：

- 环境变量；
- shared credentials file；
- IAM role；
- 可能的 Identity Center / SSO 场景。

这比强制用户每台设备都手填 AK/SK 更贴近行业做法。

## 冲突与版本模型

### 1. 版本字段

每次提交新的 vault head 时都带上：

- `vault_revision`
- `parent_revision`
- `payload_hash`
- `device_id`
- `created_at`

### 2. Primary 写入策略

#### S3 Primary

使用条件写入：

- 只在远端 head 仍是预期 revision 时覆盖；
- 不满足时直接视为冲突。

#### Snippet/Gist Primary

做 best-effort 乐观并发控制：

- 先读取远端当前 head；
- 若本地 `parent_revision` 与远端不一致，则判定冲突；
- 因为缺少严格 CAS，仍要承认存在极短时间窗口竞争。

这也是为什么 snippet/gist 不应该是默认 primary。

### 3. 冲突处理策略

首版建议不要做静默 last-write-wins。

建议行为：

- 检测到冲突时停止提交；
- 拉取远端 head；
- 在 UI 中提供：
  - `Use Remote`
  - `Keep Local as Conflict Copy`
  - `Retry After Merge`

如果需要自动化一点，可以在本地生成：

- `Host Name (conflict from <device_id> <timestamp>)`

但不做“无提示覆盖别人修改”。

## UI 设计

### 1. 入口

当前仓库已有 titlebar menu 与空白右侧面板，最合适的承载方式是：

- titlebar menu 的 `Settings` 进入 `Sync & Vault` 面板；
- 面板渲染在现有 `right-panel` 内；
- 不把 vault 配置塞进 SSH connection modal。

### 2. 面板结构

建议右侧面板新增 `Sync & Vault` 模块，至少包含：

- Vault 状态卡
  - Locked / Unlocked
  - current revision
  - last sync time
  - primary status
- Master Password 区
  - Set
  - Change
  - Lock now
- Remotes 列表
  - provider 名称
  - role: Primary / Mirror
  - health
  - last error
- Bootstrap 区
  - Export bootstrap
  - Import bootstrap
  - Copy recovery summary
- Sync 行为区
  - Auto sync on save
  - Sync on startup
  - Manual sync now

### 3. 交互原则

- 默认先引导用户创建 vault，再添加 remote；
- 如果已有 remote 但无 master password，不允许启用同步；
- provider 授权成功后，不立刻暴露任何明文 secret；
- 同步失败要显示是：
  - provider auth error
  - vault decrypt error
  - remote conflict
  - mirror replication error

## 模块设计

建议新增模块：

- `src/app/vault/mod.rs`
- `src/app/vault/model.rs`
- `src/app/vault/crypto.rs`
- `src/app/vault/bootstrap.rs`
- `src/app/vault/cache.rs`
- `src/app/vault/engine.rs`
- `src/app/vault/provider/mod.rs`
- `src/app/vault/provider/s3.rs`
- `src/app/vault/provider/github_gist.rs`
- `src/app/vault/provider/gitlab_snippet.rs`
- `src/app/vault/provider/gitee_gist.rs`

建议整合点：

- `src/app/mod.rs`
  - 暴露 `vault` 模块
- `src/app/bootstrap.rs`
  - UI 事件绑定
  - 自动同步触发
  - 解锁状态同步
- `src/app/ui_preferences.rs`
  - 增加 vault 面板显示与同步偏好
- `src/app/ssh/credentials.rs`
  - 继续作为 secret bundle 边界
  - 增加 vault import / export 接口
- `src/app/ssh/known_hosts.rs`
  - 纳入 vault snapshot
- `src/app/assets_catalog/*`
  - 纳入 vault snapshot
- `src/shell/view_model.rs`
  - 增加 `Sync & Vault` 面板状态
- `ui/app-window.slint`
  - 新增 vault 面板绑定属性与回调
- `ui/components/titlebar-menu.slint`
  - `Settings` 切换到对应 panel state
- `ui/shell/right-panel.slint`
  - 承载新设置 UI

## 安全注意事项

- provider 的 `secret/private` 属性都不能被当成真正的保密边界；
- 远端 carrier 泄露时，只应暴露：
  - object key 名
  - revision metadata
  - pack 大小
  - 时间戳
- 主密码丢失后，数据应视为不可恢复；
- 日志里禁止记录：
  - SSH password
  - private key content
  - passphrase
  - OAuth access token
  - refresh token
  - S3 secret key
- UI 里不要默认自动显示 secret；
- bootstrap export 文件必须二次加密，而不是明文导出。

## 推荐默认组合

如果要给用户一个默认推荐，我建议是：

- `Primary`: S3-compatible object storage
- `Mirror`: GitHub Gist
- `Recovery`: local encrypted bootstrap export file

理由：

- S3 最像真正对象存储，适合做版本推进与恢复；
- GitHub Gist 对个人用户跨设备最方便，适合作为 mirror；
- 本地 bootstrap export file 保证最坏情况下还能重新接管 vault。

如果用户完全不想碰对象存储，也可以降级为：

- `Primary`: GitHub Gist
- `Mirror`: GitLab Snippet 或 Gitee Gist
- `Recovery`: local encrypted bootstrap export file

但这条路径在并发与恢复语义上弱于 S3 primary。

## 分阶段落地

### Phase 1

- 建立 vault 数据模型；
- 建立加密与本地 encrypted cache；
- 支持 `Sync & Vault` 设置面板；
- 支持 S3 primary。

### Phase 2

- 加入 GitHub Gist provider；
- 加入 bootstrap export / import；
- 加入 primary + mirror fan-out。

### Phase 3

- 加入 GitLab Snippet provider；
- 加入 Gitee Gist provider；
- 补齐 provider-specific auth fallback。

### Phase 4

- 优化冲突处理；
- 增加更多恢复手段；
- 视需要评估团队 vault 与共享模型。

## 参考资料

- Termius Secure Credentials Sync: https://termius.com/documentation/secure-credentials-sync
- Termius Vault Encryption: https://termius.com/documentation/encryption
- GitHub OAuth Apps Authorization: https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps
- GitHub Gists REST API: https://docs.github.com/en/rest/gists/gists
- GitLab OAuth2: https://docs.gitlab.com/api/oauth2/
- GitLab Snippets Administration: https://docs.gitlab.com/administration/snippets/
- Gitee API Swagger: https://gitee.com/api/v5/swagger
- Gitee OAuth Docs: https://gitee.com/api/v5/oauth_doc
- AWS Standardized Credentials: https://docs.aws.amazon.com/sdkref/latest/guide/standardized-credentials.html
- AWS S3 Conditional Requests: https://docs.aws.amazon.com/AmazonS3/latest/userguide/conditional-requests.html
- RFC 9106 Argon2: https://datatracker.ietf.org/doc/html/rfc9106
- OWASP Password Storage Cheat Sheet: https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html

## 补充说明

关于 Tabby，我找到的主要是社区与自建 `tabby-web` 资料，而不是完整的官方加密设计文档。因此这里对 Tabby 的借鉴仅限于：

- 有单独同步入口；
- 支持自建 / 自托管思路；
- 不把同步配置和普通 SSH 表单混在一起。

真正的安全边界与密钥设计，仍然以 E2EE vault 模型为准。
