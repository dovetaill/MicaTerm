# SSH Proxy Chain Design

日期: 2026-03-28
方案名: `ssh-proxy-chain`
状态: 已确认，可进入 implementation planning

## 背景

当前仓库里的 SSH 能力已经具备以下基础：

- SSH 资产可保存 `host / user / port / auth_method / remark` 等信息；
- `ConnectionProfile` 会从 modal draft 或已保存资产归一化出运行时连接配置；
- `SshSessionRuntime` 已经使用 `russh` 接通真实 SSH 会话；
- 资产模型里残留了 `proxy_method` 占位字段，但它没有进入真实连接链路。

这导致两个明显问题：

- UI 上没有真正可用的代理能力；
- runtime 无法表达成熟产品常见的 `SOCKS5 -> Jump Host -> Target` 这类递归链路。

用户本次需求明确要求：

- SSH 连接增加中转代理功能；
- 可以选择自定义 `SOCKS5` 代理地址和端口；
- 可以选择“已有的 SSH 连接”作为上游代理；
- 代理链按单上游递归自动形成，例如：
  - `B` 使用 SOCKS5；
  - `A` 使用 `B` 作为上游 SSH；
  - `C` 使用 `A` 作为上游 SSH；
  - 最终链路自动展开为 `SOCKS5 -> B -> A -> C`；
- “已有的 SSH 连接”只指已保存 SSH 资产，不要求复用当前已打开会话。

## 需求澄清

### 1. 上游模型

采用“单上游递归”而不是“手工编辑完整 hop 列表”：

- 每个 SSH 资产最多只有一个 `upstream proxy`；
- 该上游可以是：
  - 无代理；
  - 一个 SOCKS5 代理；
  - 一个已保存 SSH 资产；
- 当上游本身也有上游时，连接时自动递归展开完整链。

### 2. SOCKS5 认证

按业界常见做法处理为“可选用户名密码认证”：

- 默认允许无认证 SOCKS5；
- 如果用户填写了用户名，则尝试用户名密码认证；
- 用户名和密码都不强制；
- 密码不写入资产元数据，继续走 secret store；
- 不引入更复杂的 GSSAPI 或企业代理协商。

### 3. 已有 SSH 连接的范围

本轮只支持选择“已保存 SSH 资产”作为上游，不支持：

- 选择当前已打开的 SSH 会话；
- 在运行中复用一个现有 `russh` transport；
- 针对 session 级别做连接池或共享跳板。

## 目标

- 在 SSH connection modal 中提供真正可用的代理配置；
- 支持两类上游代理：
  - `SOCKS5`
  - `Existing SSH Connection`
- 让代理链通过单上游递归自动形成，而不是要求用户手动维护整条链；
- 在保存、编辑、测试连接、真正连接四条路径上都使用同一套代理解析逻辑；
- 对环路、缺失上游、无效端口、过深链路给出明确错误；
- 保持 secret 管理边界清晰：代理密码不落盘明文。

## 非目标

- 本轮不支持从“当前已打开 session”直接选上游；
- 本轮不做手工排序的完整代理链编辑器；
- 本轮不做 `HTTP CONNECT`、`ProxyCommand`、`mosh`、`VPN` 等其他代理类型；
- 本轮不引入 SSH transport 复用池；
- 本轮不扩展到 SFTP、端口转发 UI、自动重连策略重构。

## 方案比较

### 方案 A：继续复用 `proxy_method: String`

把代理类型、SOCKS5 地址、SSH 上游资产 ID 等信息塞进一个字符串字段。

优点：

- 改动面最小；
- 初期看起来实现很快。

缺点：

- 数据结构不稳定，UI 回填和校验都很脆；
- 很难安全表达 SOCKS5 凭证引用；
- 很难做环路检测和递归解析；
- 后续维护成本最高。

### 方案 B：把单上游代理升级为结构化配置

给 SSH 资产新增结构化代理配置：

- `None`
- `Socks5 { host, port, username, password_credential_ref }`
- `SshAsset { asset_id }`

运行时根据目标资产递归解析完整 hop 列表。

优点：

- 与需求完全对齐；
- 数据结构清晰，测试边界明确；
- 能稳健支持递归链、环路检测和 secret 存储；
- 后续若要增加更多代理类型也有扩展位。

缺点：

- 比字符串字段多一次 schema 扩展；
- modal / mapper / runtime / tests 需要成体系改动。

### 方案 C：直接上完整“代理链编辑器”

让每个 SSH 资产直接维护一个 hop 数组，用户手工编辑顺序。

优点：

- 表达能力最强；
- 一次性覆盖所有复杂链路。

缺点：

- 复杂度明显高于当前需求；
- UI、校验、持久化、编辑体验都会变重；
- 用户已经明确接受“单上游递归”，没有必要现在过度设计。

## 最终决策

采用方案 B：结构化的单上游代理配置。

最终产品语义是：

- 每个 SSH 资产只有一个 `Proxy` 配置；
- `Proxy Type` 为：
  - `None`
  - `SOCKS5`
  - `Existing SSH Connection`
- 若目标连接选择了某个上游 SSH 资产，则该上游 SSH 资产自己的代理配置也会继续生效；
- 因此无需手工维护完整链，链路自然递归展开。

## 交互设计

### Modal 布局

当前 modal 已经简化为 `Basic / Authentication / Notes` 三段。本轮保持这个整体形态，不恢复旧版多 tab 布局。

新增一个 `Proxy` 分组，放在 `Authentication` 与 `Notes` 之间。

### Proxy 分组字段

统一增加以下字段：

- `Proxy Type`
  - `None`
  - `SOCKS5`
  - `Existing SSH Connection`

当 `Proxy Type = SOCKS5` 时显示：

- `SOCKS5 Host`
- `SOCKS5 Port`
- `Username`（可选）
- `Password`（可选，密码输入）

当 `Proxy Type = Existing SSH Connection` 时显示：

- `Upstream SSH Connection`
  - 数据源为当前资产树中所有已保存 SSH 资产；
  - 过滤掉当前正在编辑的资产自身；
  - 显示用户可识别的标题，内部保存资产 ID。

### 交互语义

- `None` 为默认值；
- 选择 `SOCKS5` 时不强制填写用户名和密码；
- 选择 `Existing SSH Connection` 时必须选一个已保存 SSH 资产；
- 编辑已有资产时，字段必须能正确回填；
- 如果某个已引用的上游资产后来被删除：
  - 编辑时显示为无效引用；
  - 保存和连接都应报错，要求用户重新选择。

### 反馈文案

错误语义需要明确区分：

- 表单校验错误：
  - `SOCKS5 host is required`
  - `SOCKS5 port is invalid`
  - `upstream SSH connection is required`
- 链路解析错误：
  - `SSH proxy chain contains a cycle`
  - `SSH proxy chain is too deep`
  - `upstream SSH asset '<id>' was not found`
- 运行时连接错误：
  - `failed to connect to SOCKS5 proxy`
  - `SSH upstream '<name>' rejected direct-tcpip forwarding`

## 数据模型设计

### 资产持久化模型

需要把当前的 `proxy_method: String` 升级为结构化模型。

建议新增：

```rust
pub enum AssetSshProxySpec {
    None,
    Socks5(AssetSocks5ProxySpec),
    SshAsset { asset_id: String },
}

pub struct AssetSocks5ProxySpec {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password_credential_ref: Option<String>,
}
```

`AssetSshConnectionSpec` 改为持有：

```rust
pub proxy: AssetSshProxySpec
```

并删除旧的 `proxy_method: String`。

### Draft 模型

`AssetSshConnectionDraft` 需要对应 UI 状态，而不是直接等于持久化结构。

建议字段：

```rust
pub proxy_type: String,                // "none" | "socks5" | "ssh-asset"
pub proxy_socks5_host: String,
pub proxy_socks5_port: String,
pub proxy_socks5_username: String,
pub proxy_socks5_password: String,
pub proxy_socks5_password_visible: bool,
pub proxy_ssh_asset_id: String,
```

这样可以保证：

- modal draft 完整表达输入中状态；
- 保存时再归一化为 `AssetSshProxySpec`；
- 密码只存在 draft / secret store，不进入普通资产元数据。

### Runtime Profile 模型

`ConnectionProfile` 需要升级为运行时可直接消费的递归解析输入。

建议新增：

```rust
pub enum ConnectionProxyProfile {
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    SshUpstream {
        asset_id: String,
    },
}
```

`ConnectionProfile` 持有：

```rust
pub proxy: ConnectionProxyProfile
```

这里的 `SshUpstream` 保留资产 ID，而不是内嵌完整 profile。真正展开递归链时再从资产树解析，避免 profile 构造阶段和整个 catalog 强耦合。

## 运行时架构设计

### 整体思路

保持现有分层：

- 资产树 / modal draft
- `ConnectionProfile`
- `SessionManager`
- `SshSessionRuntime`

但把 transport 建立拆成独立的“代理链拨号”步骤。

建议新增一个 runtime 内部拨号层：

- `open_transport_stream_for_profile(...)`
- `resolve_proxy_chain(...)`
- `connect_via_socks5(...)`
- `connect_via_ssh_upstream(...)`

### 链路建立顺序

对于最终目标连接 `C`：

1. 从资产配置递归解析出 hop 列表；
2. 如果最外层 hop 是 SOCKS5：
   - 先建立到 SOCKS5 server 的 TCP；
   - 执行 SOCKS5 握手；
   - 通过 SOCKS5 CONNECT 连到下一跳目标；
3. 如果下一跳是 SSH 资产 `B`：
   - 在当前字节流上使用 `russh::client::connect_stream` 连 `B`；
   - 对 `B` 做认证；
   - 通过 `channel_open_direct_tcpip` 打开到下一跳的通道；
4. 如果还有下一跳 SSH `A`：
   - 把上一步的 direct-tcpip channel 作为下一层 `connect_stream` 的底层流；
   - 重复认证和 direct-tcpip；
5. 最后一跳连到真正目标 `C` 后，再像当前逻辑一样打开 session channel、PTY、shell。

### 为什么这个方案可行

现有依赖 `russh` 已具备两个关键能力：

- `client::connect_stream`：允许在任意异步字节流上建立 SSH client；
- `channel_open_direct_tcpip`：允许通过 SSH 连接打开到下游地址的 TCP 转发通道。

因此无需换库，也无需引入外部 `ssh` 命令。

### Runtime seam 调整

当前 `SshSessionRuntime::connect_with_credential_store` 直接：

- `client::connect(config, (host, port), handler)`

本轮应改成：

- 先通过 transport builder 拿到底层流；
- 再在最末 hop 上建立真正的目标 SSH client；
- 同时持有所有中间 hop 的 runtime handle，保证断开时链路整体释放。

### 生命周期

因为上游 hop 不再只是一个 `TcpStream`，而是一串 SSH client + direct-tcpip channel，所以 runtime 需要显式持有整条链的资源，避免：

- 只保存最内层 channel，导致上游 `Handle` 被 drop；
- 连接建立成功后，中间 hop 被提前释放。

建议在 runtime 内引入一个 `TransportChainGuard`，统一持有：

- SOCKS5 TCP stream；
- 上游 SSH handle；
- 上游 direct-tcpip channel；
- 目标 SSH handle。

## 校验与防御性规则

### 环路检测

保存和连接前都必须做递归检查。

例如：

- `A -> B`
- `B -> A`

这种情况要在 profile 解析阶段直接报错，不能等到 runtime 死循环。

### 最大深度

设置最大代理链深，例如 `8`。

目的：

- 防止异常数据导致无限递归；
- 防止用户把代理链堆得过深，调试成本失控。

### 上游引用约束

- 只允许引用 `ConsoleAssetKind::SshConnection`；
- 不允许引用自己；
- 被引用资产必须能成功归一化为有效 `ConnectionProfile`；
- 上游 SSH 资产本身也可以有自己的代理。

### SOCKS5 校验

- `host` 必填；
- `port` 必须能解析为 `u16`；
- 如果填写了密码但用户名为空，仍允许保存，但连接阶段按“无用户名密码认证”处理并记录明确文案，避免引入不对称表单逻辑；
- 用户名密码都为空时走 no-auth。

## Secret 存储策略

代理密码不进入 `PersistedSshConnectionSpec`。

复用现有 keyring secret bundle 扩展：

- 新增 `proxy_socks5_password` 字段；
- 若未来需要，可再补 `proxy_socks5_username` 是否也入 secret；
- 本轮用户名保持普通字段，密码入 secret 即可。

这样符合常见产品习惯：

- 地址和用户名通常可见、可同步；
- 密码属于 secret。

## 持久化与迁移

当前 schema version 为 `2`。本轮需要升级 schema，并处理旧字段兼容。

迁移策略：

- 读取旧资产时：
  - 如果只有 `proxy_method`，统一迁移为 `AssetSshProxySpec::None`；
  - 不尝试猜测旧字符串含义；
- 新保存资产只写新结构，不再写 `proxy_method`；
- 旧测试数据同步改为新结构。

原因：

- 旧字段本来就没有真实业务语义；
- 强行解析历史字符串只会引入虚假兼容。

## 测试策略

### Domain / profile 测试

需要新增或更新：

- draft -> profile 时 `None / SOCKS5 / SSH asset` 三类代理归一化；
- SOCKS5 端口非法时报错；
- 上游 SSH 资产缺失时报错；
- `A -> B -> A` 环路检测；
- 最大链深限制；
- 已保存资产回填代理字段。

### Store / mapper 测试

需要覆盖：

- 新 schema 的 round-trip；
- secret 引用保存与读取；
- 旧 schema 读取后默认成 `None` 代理。

### UI / bootstrap 测试

需要覆盖：

- modal 出现 `Proxy` 分组；
- 切换 `Proxy Type` 时正确显示不同字段；
- 选择 SOCKS5 后 draft 正确更新；
- 选择上游 SSH 资产后 draft 正确更新；
- 编辑已有资产能正确回填代理配置；
- 删除被引用资产后的错误反馈。

### Runtime 测试

至少覆盖：

- SOCKS5 握手成功 / 认证成功；
- SOCKS5 无认证路径；
- SOCKS5 认证失败；
- SSH 上游 direct-tcpip 成功；
- 多跳 `SSH -> SSH -> target`；
- 环路或缺失上游在进入真实网络前就被阻止。

如果当前 test harness 不足以稳定模拟 SOCKS5，可先补最小 fake SOCKS5 server。

## 验收标准

- SSH modal 可以配置 `None / SOCKS5 / Existing SSH Connection` 三种代理模式；
- SOCKS5 支持自定义 `host:port`，并支持可选用户名密码；
- 选择已保存 SSH 资产作为上游后，可递归形成代理链；
- `B` 走 SOCKS5、`A` 走 `B`、`C` 走 `A` 时，最终链路为 `SOCKS5 -> B -> A -> C`；
- 环路、缺失上游、非法端口会被清晰阻止；
- 代理密码不写入资产元数据；
- 已有无代理 SSH 资产不会回归；
- 不要求复用当前已打开 SSH 会话。
