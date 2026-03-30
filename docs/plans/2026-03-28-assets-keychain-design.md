# Keychain 模块设计

日期: 2026-03-28
功能: `assets-keychain`
状态: 方案已确认，可进入 implementation planning

注：本设计文档保留 2026-03-28 的方案决策；“当前实现现状”章节描述的是立项时基线，不作为最终实现盘点。
## 背景

当前仓库已经完成了资产侧边栏的一级导航壳体、`Window Console` 资产树、SSH 连接表单、`CredentialStore`、`VaultSnapshot` 与 SSH vault sync 主路径，但 `Keychain` 仍然只是导航占位。

已知现状有几个关键事实：

- `src/shell/sidebar.rs` 与 `ui/shell/assets-sidebar.slint` 已经存在 `Keychain` 一级入口，但当前面板只有静态说明文案；
- `src/shell/assets.rs`、`src/app/assets_catalog/model.rs`、`src/app/vault/model.rs` 目前只建模了 `Folder / SshConnection`，并没有 keychain 领域对象；
- `src/shell/view_model.rs`、`src/app/ssh/profile.rs`、`src/app/ssh/runtime.rs` 仍然把 SSH 主机认证绑定在主机自身的 `credential_ref`、`auth_method`、`private_key_source` 上；
- `ui/components/assets-ssh-connection-modal.slint` 已经具备密码、inline private key、legacy file path、passphrase、导入私钥等能力，这为“手动认证模式”保留了完整基础；
- `src/app/vault/snapshot.rs` 已经能导出导入 SSH secret bundle，但还没有 keychain 独立快照区域。

近期 Git 演进也说明了本轮设计边界：

- `4e74759 Add HTTP and dropdown SSH proxy support`
- `a075087 feat: add ssh vault sync workflow`
- `014d6dd docs: add windows skia mainline planning docs`

这说明当前主线已经具备：

- 持久化 schema 扩展能力；
- 通过 resolver / normalization 把 UI draft 过渡到 runtime profile 的架构基础；
- vault snapshot 与 secret store 的分层路径。

因此，本轮最合理的方向不是把 Keychain 临时塞进现有 SSH host 表单里，而是补齐一个独立但可复用的认证资产模块。

外部参考只用于校准主流习惯，不机械照搬：

- Termius 的 `Hosts` 与 `Keychain` 分离，`Keychain` 内核心对象是 `Keys` 与 `Identities`；
- VS Code 风格的密集型编辑器式表单和树视图更适合桌面生产力场景；
- Windows 11 Fluent Design 更强调清晰层级、柔和容器、稳定图标语义，而不是过度装饰。

参考来源：

- <https://support.termius.com/hc/en-us/articles/4401872025113-Keychain>
- <https://termius.com/documentation/identities>
- <https://termius.com/documentation/import-ssh-keys>
- <https://termius.com/documentation/generate-ssh-key>
- <https://code.visualstudio.com/docs/remote/ssh>

## 目标

本轮设计目标如下：

- 在 `assets` 模块下补齐一个一等公民的 `Keychain` 模块；
- 支持以大众习惯管理可复用的认证资产，而不是把所有认证信息散落在每个 SSH host 内；
- `Keychain` 需要覆盖两类业务对象：
  - `Identity`
  - `SSH Key`
- 满足以下核心用户动作：
  - 新增账号密码类型；
  - 导入私钥文件；
  - 粘贴私钥文本；
  - 导入或粘贴公钥文本；
  - 生成私钥公钥对；
  - 复制公钥文本；
  - 在 SSH host 新建/编辑时选择现有 keychain identity；
  - 保留 SSH host 手动输入账号密码或手动提供私钥的路径；
- Keychain 数据从第一版开始纳入 vault/sync；
- 保持与现有 SSH 运行时和旧资产数据兼容，不做破坏式切换。

## 非目标 / 边界

本轮不包含以下内容：

- 不把 `Keychain` 与当前 `Window Console` 资产树强行合并成单棵树；
- 不引入 SSH agent forwarding、FIDO2、hardware token、SSH certificate；
- 不做团队共享 keychain、多用户权限、审批流；
- 不做自动把所有旧 SSH host 批量迁移为 keychain identity；
- 不把“主机连接时需要公钥”这种误导性交互带入 host 表单；
- 不在本轮设计文档里生成实现代码；
- 不要求本轮直接产出最终 UI 视觉稿。

## 当前实现现状

### 1. `Keychain` 入口已经存在，但仍是占位模块

当前 `SidebarDestination` 已经包含 `Keychain`，并且 toolbar descriptor 为其保留了 `new-keychain` 与 `Search Keychain` 文案：

- `src/shell/sidebar.rs`
- `ui/shell/assets-sidebar.slint`

但 `keychain` 面板当前只有：

- 标题 `Keychain`
- 占位文案 `Accounts, identities, SSH keys`

没有真实 tree、数据源、创建菜单、context menu、表单或持久化。

### 2. 当前左侧 explorer 真相源仍然只服务 console 资产

`AssetTree` 与 `PersistedAssetCatalog` 只覆盖：

- `Folder`
- `SshConnection`

对应文件：

- `src/shell/assets.rs`
- `src/app/assets_catalog/model.rs`
- `src/app/assets_catalog/mapper.rs`
- `src/app/assets_catalog/redb_store.rs`

这意味着如果直接把 keychain 当成“又一个 console asset kind”塞进去，会把 host 语义与认证资产语义混在一起。

### 3. 当前 SSH host 的认证绑定是 host-owned，而不是 reusable identity-owned

现有 `AssetSshConnectionSpec` 与 `PersistedSshConnectionSpec` 仍然把以下字段挂在 host 自身：

- `user`
- `auth_method`
- `private_key_source`
- `private_key_path`
- `credential_ref`

对应文件：

- `src/shell/assets.rs`
- `src/app/assets_catalog/model.rs`
- `src/app/ssh/profile.rs`
- `src/app/ssh/runtime.rs`

这条链路已经足够成熟，所以本轮更适合在“进入 runtime profile 之前”增加 `Identity resolution`，而不是重写 runtime。

### 4. 现有 SSH modal 已经具备“手动模式”的完整基础

当前 SSH host modal 已经支持：

- `Password`
- `Private Key`
- `Private Key` 的 `content` 与 `path` 兼容模式
- `Passphrase`
- inline 私钥说明文案

对应文件：

- `ui/components/assets-ssh-connection-modal.slint`
- `src/app/bootstrap.rs`
- `src/shell/view_model.rs`

这意味着 Keychain 更适合被设计为：

- `SSH host authentication source` 的新增来源；
- 而不是替换掉现有手动模式。

### 5. vault/sync 已经具备扩展新快照域的基础

当前 `VaultSnapshot` 已经包含：

- `asset_catalog`
- `ssh_secret_bundles`
- `known_hosts`
- `sync_preferences`
- `ui_preferences`

对应文件：

- `src/app/vault/model.rs`
- `src/app/vault/snapshot.rs`

因此 Keychain 最合理的落地方式，是扩展出独立快照 section，而不是把 keychain secret 强行塞回 `ssh_secret_bundles`。

## 设计要点拆分

本轮设计拆分为五个关键点：

1. `Keychain` 在信息架构中的位置与树模型；
2. `Keychain` 领域对象如何建模；
3. SSH host 如何与 `Keychain` 建立认证绑定；
4. Keychain secret 如何持久化、同步与兼容旧数据；
5. 交互与视觉语言如何贴近主流桌面终端工具。

## 方案对比

### 设计要点 1：`Keychain` 的模块位置与树形抽象

#### 方案 A：把 `Keychain` 直接并入当前 `Console AssetTree`

做法：

- 扩展现有 `ConsoleAssetKind`；
- 让 `Folder / SshConnection / Identity / SSH Key` 全部进入同一棵树；
- `Console` 与 `Keychain` 只做视图过滤。

分析：

- 实现复杂度：中等偏高。需要改写当前 tree projection 与 create/menu 语义；
- 与当前架构契合度：一般。当前 `AssetTree` 的语义明显偏向 console host，不是认证资产；
- 交互一致性：表面一致，实则会把“连接目标”和“认证材料”混成一类；
- 可维护性：较差。后续 snippets、keychain、console 都会争夺同一套 node 语义；
- 潜在风险：删除、搜索、拖拽、默认命名、context menu 都会被跨域语义污染。

#### 方案 B：保留 `Keychain` 顶层模块，并建立独立 `KeychainCatalog`

做法：

- `Keychain` 继续作为一级导航；
- `Window Console` 与 `Keychain` 各自维护独立 catalog；
- 但交互语言保持一致，都采用“folder + leaf item”的 explorer 模式；
- 共用 Fluent 图标、搜索入口、context menu 风格、重命名/删除交互。

分析：

- 实现复杂度：中等。需要新增 catalog、projection 和持久化，但边界清晰；
- 与当前架构契合度：高。符合当前 sidebar destination 已经拆开的架构；
- 交互一致性：高。用户能理解“主机”和“认证资产”是两个并列模块；
- 可维护性：高。后续 snippets、keychain、console 都能保持领域隔离；
- 潜在风险：需要多一套路由状态与数据源，但这是可控复杂度。

#### 最终决策

采用方案 B。

收敛原则：

- `Keychain` 仍然是一等一级模块；
- 不在 v1 把它合并进当前 `Console AssetTree`；
- 但 `Keychain` 内部依然使用树形 explorer 语义；
- 交互习惯与 `Window Console` 对齐，形成统一的资产侧边栏语言。

### 设计要点 2：`Keychain` 的领域对象模型

#### 方案 A：只做一种混合对象 `CredentialEntry`

做法：

- 用一个对象同时承载：
  - username
  - password
  - private key
  - public key
  - passphrase
- host 直接引用该对象。

分析：

- 实现复杂度：初期较低；
- 与当前架构契合度：一般，字段会快速膨胀；
- 交互一致性：一般，用户很难分清“账号”与“密钥”；
- 可维护性：差，一旦要支持一个 key 被多个 identity 复用，结构会立刻失真；
- 潜在风险：重复存储、重复生成、后续扩展 jump host / snippets / vault 时边界不清。

#### 方案 B：拆成 `Identity` 与 `SSH Key` 两种对象

做法：

- `Identity` 负责承载：
  - 显示名
  - username
  - 认证类型
- `SSH Key` 负责承载：
  - key label
  - algorithm / fingerprint / comment
  - public key
  - private key secret
  - passphrase
- 当 `Identity` 的认证类型为 `Password` 时，密码 secret 由 identity 自身持有；
- 当 `Identity` 的认证类型为 `SSH Key` 时，identity 只引用一个 `SSH Key`。

分析：

- 实现复杂度：中等；
- 与当前架构契合度：高，符合“可复用 identity + secret backend”模式；
- 交互一致性：高，接近 Termius 等业界常见模型；
- 可维护性：高，key 与 identity 可以独立复用、移动、搜索与同步；
- 潜在风险：需要处理 identity 删除、key 删除、引用关系约束，但这是可预期复杂度。

#### 最终决策

采用方案 B。

额外收敛：

- 不新增第三种“Password Account”叶子节点；
- “账号密码类型”由 `Identity.auth_kind = Password` 承载；
- 这样既满足用户诉求，也避免树节点类型膨胀。

### 设计要点 3：SSH host 与 `Keychain` 的绑定方式

#### 方案 A：host 选择 identity 后把解析结果复制回 host 自身

做法：

- 用户选择 identity 后，立即把 username、password 或 private key 复制进 host 草稿；
- 保存后 host 仍然是 self-contained。

分析：

- 实现复杂度：初期较低；
- 与当前架构契合度：一般；
- 交互一致性：较差，用户会误以为 host 与 keychain 是实时绑定，但实际上保存后已断开；
- 可维护性：差，identity 更新后不会影响已绑定 host；
- 潜在风险：重复 secret、同步冲突、删除引用追踪失效。

#### 方案 B：host 只保存 `auth_source` 与 `keychain_identity_id`

做法：

- host 新增：
  - `auth_source = manual | keychain-identity`
  - `keychain_identity_id`
- 如果 `auth_source = manual`，继续走现有表单和保存逻辑；
- 如果 `auth_source = keychain-identity`，host 只保存 identity 引用；
- runtime 前增加 resolver，把 identity 展开成现有 `ConnectionProfile + CredentialStore` 所需结构。

分析：

- 实现复杂度：中等；
- 与当前架构契合度：高，符合现有 normalize-to-runtime 的分层方式；
- 交互一致性：高，符合大众对“引用一个身份配置”的理解；
- 可维护性：高，identity 更新可复用到多个 host；
- 潜在风险：需要新增引用校验和 resolver 层，但不会破坏当前 runtime。

#### 最终决策

采用方案 B。

关键约束：

- SSH host 的认证来源严格二选一：
  - `Manual`
  - `Identity from Keychain`
- 当 host 使用 `Identity from Keychain` 时：
  - username 取自 identity；
  - host 自身不再保存 password / private key secret；
  - 连接时先解析 identity，再进入现有 SSH normalize / runtime；
- 当 host 使用 `Manual` 时：
  - 保留现有密码或私钥输入路径；
  - 继续兼容 `private_key_source = "path"` 的 legacy 资产。

### 设计要点 4：Keychain secret 的持久化、同步与兼容策略

#### 方案 A：直接复用 `ssh_secret_bundles`

做法：

- 把 keychain identity password、private key、passphrase 全部塞进现有 `ssh_secret_bundles`；
- 通过命名前缀区分来源。

分析：

- 实现复杂度：中等；
- 与当前架构契合度：一般，能工作但语义不干净；
- 交互一致性：用户无感；
- 可维护性：一般偏低，host-owned secret 与 keychain-owned secret 会混在同一快照域；
- 潜在风险：后续调试和迁移时难以区分“主机保存的 secret”和“keychain 保存的 secret”。

#### 方案 B：单独定义 keychain snapshot section，但复用现有 `CredentialStore` backend

做法：

- `CredentialStore` 继续作为底层 secret backend；
- 新增 keychain 自己的 secret bundle 与 ref namespace；
- `VaultSnapshot` 新增：
  - `keychain_catalog`
  - `keychain_identity_secret_bundles`
  - `keychain_key_secret_bundles`
- 旧 host 的 `credential_ref` 与 `private_key_source = "path"` 保持不变。

分析：

- 实现复杂度：中等偏高；
- 与当前架构契合度：高，符合当前 snapshot 可扩展结构；
- 交互一致性：高，用户心智清楚；
- 可维护性：高，host secret 与 keychain secret 各自独立；
- 潜在风险：要补一套 snapshot import/export 映射，但复杂度可控。

#### 最终决策

采用方案 B。

明确兼容规则：

- 旧 `credential_ref` 型 SSH host 不自动迁移；
- 旧 `private_key_source = "path"` 型 SSH host 保持 `Manual` 模式；
- 用户只有在编辑 host 时明确切换到 `Identity from Keychain`，才建立新的 identity 引用；
- vault 导入导出需要同时支持：
  - 老快照缺失 keychain section；
  - 新快照同时包含 host 与 keychain。

### 设计要点 5：交互与视觉语言

#### 方案 A：平铺列表 + 表单堆叠

做法：

- `Keychain` 面板采用 flat list；
- 新建按钮直接弹统一大表单；
- SSH host modal 里只放一个简单下拉框引用 keychain。

分析：

- 实现复杂度：较低；
- 与当前架构契合度：一般；
- 交互一致性：一般，和当前左侧 explorer 风格脱节；
- 可维护性：一般，后续 folder、拖拽、搜索、删除确认不自然；
- 潜在风险：很快会退化成“一个越来越大的设置面板”。

#### 方案 B：继续使用 explorer 心智，但做独立 keychain tree 与专用 modal

做法：

- `Keychain` 面板内使用树形 explorer；
- 节点类型：
  - `Folder`
  - `Identity`
  - `SSH Key`
- toolbar 改为 create popover，而不是单一 `New Keychain`；
- `SSH Key` 使用专用 modal；
- `Identity` 使用专用 modal；
- SSH host modal 的认证区切换为：
  - `Manual`
  - `Keychain Identity`

分析：

- 实现复杂度：中等；
- 与当前架构契合度：高；
- 交互一致性：高，保持 `Assets Sidebar` 的统一语言；
- 可维护性：高，后续增量能力可以继续沿 explorer + modal 演进；
- 潜在风险：需要补齐 view model、projection、搜索与 context menu 语义，但风险可控。

#### 最终决策

采用方案 B。

交互收敛如下：

- `Keychain` 面板采用与 console 一致的 explorer 结构；
- primary create action 改为 create popover，选项为：
  - `New Folder`
  - `New Identity`
  - `New SSH Key`
- 叶子节点使用 Fluent 风格的稳定图标语义：
  - folder 使用 folder icon；
  - identity 使用 person-related icon；
  - ssh key 使用 key-related icon；
- SSH host modal 采用更接近 VS Code 输入面板的密集布局，不做过度装饰；
- 所有 destructive action 都使用显式确认，不做隐式联动删除。

## 最终决策

## 1. 信息架构

- `Keychain` 保持一级导航模块；
- 建立独立 `KeychainCatalog`；
- `Keychain` 内部继续使用树形 explorer；
- `Window Console` 与 `Keychain` 不共享同一棵业务树，但共享交互语言与视觉基线。

## 2. 数据模型

### `KeychainCatalog`

`KeychainCatalog` 包含三类节点：

- `Folder`
- `Identity`
- `SSH Key`

### `Identity`

`Identity` 负责承载“可复用的登录身份”，字段建议收敛为：

- `id`
- `parent_id`
- `title`
- `username`
- `auth_kind`
- `ssh_key_id`
- `credential_ref`
- `remark`

规则如下：

- `auth_kind = password` 时：
  - `credential_ref` 指向 identity password secret；
  - `ssh_key_id = None`
- `auth_kind = ssh-key` 时：
  - `ssh_key_id` 必填；
  - `credential_ref = None`
- host 使用 identity 时，username 取自 identity，而不是 host draft。

### `SSH Key`

`SSH Key` 负责承载“可复用的密钥材料与公开信息”，字段建议收敛为：

- `id`
- `parent_id`
- `title`
- `algorithm`
- `fingerprint`
- `public_key`
- `comment`
- `credential_ref`
- `remark`

其中：

- `credential_ref` 指向 key secret bundle：
  - `private_key_content`
  - `passphrase`
- `public_key` 作为非敏感元数据保存在 catalog 中；
- `fingerprint` 作为搜索与确认展示字段保存在 catalog 中；
- `public_key` 可来自：
  - 导入公钥文件
  - 粘贴公钥文本
  - 由私钥解析派生
  - 生成密钥对时自动生成

## 3. SSH host 绑定规则

SSH host 新增以下语义字段：

- `auth_source`
- `keychain_identity_id`

规则如下：

- `auth_source = manual`
  - 延续当前 `auth_method`、`private_key_source`、`credential_ref` 逻辑；
- `auth_source = keychain-identity`
  - `keychain_identity_id` 必填；
  - host 保存时不再写入 manual auth secret；
  - 连接时先通过 resolver 解析 identity；
- manual 与 keychain identity 严格互斥，不允许一条 host 同时保存两套认证来源。

## 4. Key 编辑规则

`SSH Key` modal 必须支持：

- 导入私钥文件；
- 粘贴私钥文本；
- 导入公钥文件；
- 粘贴公钥文本；
- 生成私钥公钥对；
- 复制公钥文本；

产品规则如下：

- 连接 SSH host 时本地只需要私钥；
- 公钥主要用于：
  - 展示；
  - 复制到服务器；
  - vault 跨设备查看；
- v1 生成算法默认使用 `Ed25519`；
- 导入路径接受当前 runtime 能解码的主流私钥格式；
- 如果用户只提供私钥，系统应尽量派生 `public_key` 与 `fingerprint`；
- 如果派生失败，仍允许保存，但公钥区域为空，用户后续可补贴公钥文本。

## 5. 删除与引用约束

为避免“删掉 key 导致一批 host 静默损坏”，v1 采用保守策略：

- 被 `Identity` 引用的 `SSH Key` 不允许直接删除；
- 被 SSH host 引用的 `Identity` 不允许直接删除；
- 删除对话框显示引用数量与受影响对象类型；
- folder 仅允许在空目录时删除；
- 不做隐式级联删除。

## 6. 同步与兼容

Keychain 从第一版开始进入 vault：

- `VaultSnapshot.keychain_catalog`
- `VaultSnapshot.keychain_identity_secret_bundles`
- `VaultSnapshot.keychain_key_secret_bundles`

兼容策略：

- 老快照没有 keychain section 时按空 keychain 处理；
- 老 SSH host 继续沿用手动模式；
- `private_key_source = "path"` 仅保留兼容，不作为 keychain key 的推荐来源；
- keychain resolver 发生错误时，只影响引用该 identity 的 host，不影响整个 catalog 加载。

## 实施步骤

这里只记录设计级实施顺序，不展开成 implementation task 列表：

1. 增加 `KeychainCatalog`、领域模型、持久化 schema 与 vault snapshot section；
2. 增加 keychain secret bundle 与 credential ref namespace；
3. 为 SSH host 增加 `auth_source` / `keychain_identity_id`，并引入 resolver；
4. 实现 keychain explorer、create menu、identity modal、ssh key modal；
5. 在 SSH host modal 中加入 `Manual / Keychain Identity` 切换与 identity picker；
6. 补齐 key import / key generation / public key copy 行为；
7. 补齐 snapshot、resolver、view model、UI smoke、兼容迁移测试。

## 风险与回滚策略

### 风险 1：身份引用链导致运行时解析失败

表现：

- host 引用了缺失 identity；
- identity 引用了缺失 key；
- vault 导入后出现 dangling reference。

缓解：

- 保存时校验引用存在；
- 删除时阻止破坏性删除；
- 连接前 resolver 返回明确错误；
- UI 用非阻塞错误反馈说明具体缺失项。

回滚：

- 只要 host 仍保留 `Manual` 模式，问题可局部回退；
- 新字段设计为可选字段，旧 host 不受影响。

### 风险 2：secret schema 混用导致迁移困难

表现：

- keychain secret 与 host secret 混在一个 bundle 域；
- 后续调试无法分辨所有权。

缓解：

- 单独定义 keychain snapshot section；
- 单独定义 keychain credential ref namespace；
- 只复用 `CredentialStore` backend，不复用 host-owned 语义层。

回滚：

- 即使 keychain 功能下线，旧 SSH host secret 不需要迁移回滚。

### 风险 3：公钥派生和密钥生成的格式兼容问题

表现：

- 某些导入私钥无法派生公钥；
- 不同格式的 fingerprint 显示不一致。

缓解：

- 生成策略统一为 `Ed25519`；
- 导入时派生失败不阻止保存，但显式提示“public key unavailable”；
- fingerprint 统一保存为单一展示格式。

回滚：

- 若生成流程不稳定，可临时关闭“生成密钥”入口，仅保留导入与粘贴，不影响整体 keychain 架构。

## 验证清单

- `Keychain` 面板不再是占位文案，而是可浏览的 explorer；
- 可以创建 `Folder / Identity / SSH Key`；
- `Identity` 可以配置为 `Password` 或 `SSH Key`；
- `SSH Key` 可以导入私钥、公钥，粘贴私钥、公钥，生成密钥对，复制公钥；
- SSH host modal 可以在 `Manual` 与 `Keychain Identity` 间切换；
- host 使用 keychain identity 时，username 来自 identity；
- 旧 `credential_ref` 型 host 仍可编辑、保存、连接；
- 旧 `private_key_source = "path"` 型 host 仍可编辑、保存、连接；
- vault 导出导入后，keychain catalog、identity password、private key、passphrase 都能完整恢复；
- 删除被引用的 identity 或 key 时，会阻止删除并显示依赖说明；
- resolver 错误不会破坏整个 sidebar，只会阻止具体连接动作。
