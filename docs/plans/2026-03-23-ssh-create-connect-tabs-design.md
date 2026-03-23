# SSH 新建 / 连接 / 标签页 Design

日期: 2026-03-23
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

当前仓库已经完成桌面 shell、`Windows Console` 资产树、create modal、基础 context menu 与 placeholder workspace，但与本轮目标仍存在明显断层：

- `SSH modal` 已存在壳层与 draft state，但 `Standard` 页仍是无标签输入框，缺少密码、备注、认证方式等首轮必需信息；
- `Save` 当前仅创建一个 `ssh` 资产节点，不会建立真实 SSH 连接；
- `TabBar` 与主工作区仍是 placeholder，尚无真实 session / tab model；
- 项目依赖里尚未引入 `wezterm-term`、`termwiz`、`russh`、`russh-sftp`；
- 需求明确要求本轮聚焦 `SSH 新建表单完善`、`可实际建立 SSH 连接`、`连接标签页模型`，但不扩展为完整持久化改造或完整 SFTP 体系。

最近直接相关提交如下：

- `53c4b1d 2026-03-16 feat: implement assets sidebar toolbar shell`
- `0f22556 2026-03-19 feat: finalize assets explorer modal bugfix4`
- `77b1f51 2026-03-20 feat: 完成 windows console assets 样式优化`
- `75a4236 2026-03-23 feat: finalize windows console assets explorer workflow`

本设计文档用于固化以下已确认决策：

1. 保留当前 `SSH modal` 的 tab 式壳层，但把 `Standard` 变成真正可用的主路径；
2. 首轮接入真实 SSH 连接能力，底层采用 `wezterm-term + termwiz + russh` 分层；
3. 建立真实的 `tab / session` 语义，并与当前资产树、context menu、workspace 占位壳层对齐；
4. 明确本轮边界，避免擅自扩成完整持久化、SFTP、proxy/tunnel、自动重连重构。

## 目标

### 本轮必须达成

- `New SSH Connection` modal 让用户明确知道每个字段填写什么；
- `Standard` 页至少覆盖：
  - `Name`
  - `Host`
  - `User`
  - `Port`
  - `Authentication Type`
  - `Password`
  - `Private Key`
  - `Remark`
- 首轮支持两类认证：
  - `Password`
  - `Private Key`
- 密码允许保存到系统凭据存储；
- `Private Key` 同时支持：
  - 导入文件内容
  - 指定文件路径
- modal 保留四个动作：
  - `Save`
  - `Connect`
  - `Test Connection`
  - `Save and Connect`
- 新建或打开 SSH 连接后，能够建立真实交互式 shell session；
- `TabBar` 升级为真实 tab/session 容器；
- 同一 SSH 资产再次打开时，默认激活已有 tab；只有显式 `Open in New Tab` 才创建第二个 session；
- 首次遇到未知 host key 时采用 `TOFU`：
  - 弹确认
  - 接受后写入 `known_hosts`
  - 后续 key 变化阻断连接

### 体验目标

- 表单可读性清晰，不再依赖 placeholder 传达语义；
- 连接动作语义明确，不让用户混淆“保存资产”和“发起连接”；
- tab/session 行为贴近桌面终端客户端与 VS Code 风格，但保持当前项目的方角、Fluent 气质；
- 错误反馈优先靠近当前操作，不引入大量全局打断。

## 非目标 / 边界

本轮不覆盖以下内容：

- `russh-sftp` 真实 UI 与文件传输工作流；
- proxy、tunnel、environment、advanced 的完整业务落地；
- 自动重连、会话恢复、后台 keepalive 策略矩阵；
- 完整资产持久化体系重构；
- 多窗口、多 workspace、拆分 pane；
- 终端渲染器的全面重写；
- 远程同步、导入导出、批量编辑；
- 移动端和触屏交互适配。

## 当前实现现状

### 1. `SSH modal` 只有基础 draft state 与占位壳层

当前 `SSH modal` 已定义以下字段：

- `name`
- `host`
- `user`
- `port`
- `environment`
- `proxy_method`

对应代码见：

- [ui/components/assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L3)
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L67)

但现状问题明确存在：

- `Standard` 页只有无标签输入框；
- 缺少 `Password`、`Remark`、`Authentication Type`；
- `Test Connection` 只有视觉元素，没有 callback；
- `Tunnel / Advanced` 仍写着 follow-up shell 文案。

对应代码见：

- [ui/components/assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L128)
- [ui/components/assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L246)
- [ui/components/assets-ssh-connection-modal.slint](/home/wwwroot/mica-term/ui/components/assets-ssh-connection-modal.slint#L351)

### 2. 当前 `Save` 只创建资产节点，不会连接

`ShellViewModel::confirm_asset_modal()` 在 `NewSshConnection` 分支中仅做：

- 校验 `name` 与 `host` 非空；
- 以 `draft.name` 为资产标题；
- 在 `AssetTree` 中插入一个 `ConsoleAssetKind::SshConnection` 节点；
- 更新 selection/focus；
- 关闭 modal。

没有：

- SSH transport
- 认证
- terminal session
- tab 创建
- renderer 挂接

对应代码见：

- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L441)
- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L458)
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L923)

### 3. 主工作区与 `TabBar` 仍是 placeholder

当前主工作区固定渲染：

- `TabBar {}`
- `WelcomeView {}`

对应代码见：

- [ui/app-window.slint](/home/wwwroot/mica-term/ui/app-window.slint#L348)
- [ui/welcome/welcome-view.slint](/home/wwwroot/mica-term/ui/welcome/welcome-view.slint#L5)

`TabBar` 本身仍只是一个静态 `ActiveTab`，没有 model、事件或 session state：

- [ui/shell/tabbar.slint](/home/wwwroot/mica-term/ui/shell/tabbar.slint#L1)

### 4. 当前连接相关状态仍是占位语义

context menu 的 `target_has_active_connection` 目前被硬编码为 `true`，因此 `Close` 等连接动作尚不可信：

- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L860)

### 5. 项目依赖还没有真正接入 SSH / terminal core

当前 `Cargo.toml` 尚未包含：

- `wezterm-term`
- `termwiz`
- `russh`
- `russh-sftp`

对应见：

- [Cargo.toml](/home/wwwroot/mica-term/Cargo.toml#L13)

### 6. 当前实现与本轮需求之间的主要差距

- 表单字段不完整；
- 表单信息架构不可用；
- 无认证模型；
- 无系统凭据存储边界；
- 无 `known_hosts` 策略；
- 无真实 session/runtime 分层；
- 无真实 tab/session model；
- 无 terminal surface 挂接点。

## 设计要点拆分

### 设计点 1：`SSH modal` 信息架构与字段布局

#### 备选方案

##### 方案 A：收敛成单页分组表单

优点：

- 首轮体验最清晰；
- 避免多标签造成认知跳转；
- 更适合当前字段量级。

缺点：

- 会直接推翻已存在的 tab 壳层；
- 与当前 UI 基线偏差较大；
- 后续 `Proxy / Tunnel / Environment / Advanced` 重新落位时需要二次改造。

##### 方案 B：保留现有 tab 壳层，只把 `Standard` 做完整

优点：

- 与当前源码结构最契合；
- 变更范围可控；
- 可把首轮必需字段集中在 `Standard`，其余 tab 继续留作后续扩展位。

缺点：

- 首轮会同时存在“可用页”和“未落地页”；
- 需要额外约束未落地 tab 的文案与启用语义，避免误导。

#### 最终决策

选择方案 B。

落地约束如下：

- 保留当前顶层 tab strip；
- `Standard` 变为首轮唯一完整主路径；
- `Proxy / Environment / Advanced / Tunnel` 允许继续保留，但必须清楚表达“未进入本轮连接链路”；
- 首轮必需字段全部放在 `Standard`；
- 每个输入控件必须有显式字段标签；
- placeholder 只提供示例值，不承担字段说明职责；
- 必填项采用明确标识；
- 校验反馈采用字段附近 inline message，不用全局 toast 代替。

#### `Standard` 页字段分组

建议分为三组：

1. `Connection`
   - `Name`
   - `Host`
   - `User`
   - `Port`

2. `Authentication`
   - `Authentication Type`
   - `Password`
   - `Private Key Source`
   - `Private Key Content`
   - `Private Key Path`
   - `Passphrase`（若导入 key 且需要）

3. `Metadata`
   - `Remark`

补充约束：

- `Name` 首轮继续作为资产显示名；
- tab 标题优先使用 `Name`；
- `Remark` 仅作资产元数据，不进入连接参数；
- `environment` 与 `proxy_method` 保留在各自 tab，不进入首轮真实连接链路。

### 设计点 2：认证能力与凭据保存语义

#### 备选方案

##### 方案 A：首轮只支持密码

优点：

- 复杂度最低；
- 连接路径最短。

缺点：

- 与已确认需求不一致；
- 真实桌面 SSH 客户端首发体验不足。

##### 方案 B：首轮支持 `Password + Private Key`

优点：

- 与已确认需求一致；
- 更符合成熟 SSH 客户端的基本能力边界；
- 便于后续扩到 agent / cert / jump host。

缺点：

- 表单、验证、存储和 runtime 都会更复杂；
- 需要明确内容导入与路径引用的边界。

#### 最终决策

选择方案 B。

补充决策：

- `Password` 可保存到系统凭据存储；
- `Private Key` 同时支持：
  - 导入文件内容
  - 保存文件路径
- `Private Key` 导入内容时，资产只保存“引用方式 + 凭据标识”，敏感内容不写自定义明文配置；
- `Private Key` 走路径模式时，资产保存路径字符串，运行时按需读取；
- 若 key 需要 passphrase，沿用系统凭据存储，不单独设计自定义明文存储。

#### 认证字段模型

首轮建议在 `ConnectionProfile` 中显式建模：

- `auth_method`
  - `password`
  - `private_key_inline`
  - `private_key_path`
- `credential_ref`
  - 指向系统凭据存储中的 password / key / passphrase 项
- `private_key_path`
  - 仅路径模式需要
- `remark`

### 设计点 3：`Save / Connect / Test Connection / Save and Connect` 动作模型

#### 备选方案

##### 方案 A：只保留两个动作

优点：

- 状态最简单；
- 实现路径最短。

缺点：

- 与已确认需求不一致；
- 缺少“仅测试连接”的独立语义。

##### 方案 B：保留四动作模型

优点：

- 与用户确认一致；
- 资产管理与连接动作分离更明确；
- 更贴近成熟 SSH 客户端行为。

缺点：

- 需要更精确的动作边界；
- 共享的错误流与 host key 交互需要统一设计。

#### 最终决策

选择方案 B。

四个动作的最终语义如下：

- `Save`
  - 校验当前 draft；
  - 保存/更新 SSH 资产；
  - 写入或更新凭据引用；
  - 不发起连接；
  - 不创建 tab。

- `Connect`
  - 使用当前 draft 发起连接；
  - 为避免资产与真实连接参数脱节，默认先完成与 `Save` 同等的配置归一化与保存，再进入连接；
  - 成功后创建或激活 tab。

- `Test Connection`
  - 使用当前 draft 发起一次短生命周期连接；
  - 覆盖 host key、认证、握手与 shell 可达性检查；
  - 成功后立即断开；
  - 不创建 tab；
  - 失败后 modal 保持打开并显示错误。

- `Save and Connect`
  - 先执行 `Save`；
  - 成功后执行真实连接；
  - 成功后创建或激活 tab。

#### 共用流程

四个动作共享以下流水线：

1. draft 归一化
2. 字段校验
3. 凭据解析/写入
4. host key 策略处理
5. 连接或测试
6. UI 结果分流

### 设计点 4：runtime 分层与首轮接入边界

#### 备选方案

##### 方案 A：分三层

- `ConnectionProfile`
- `SessionHandle`
- `SshSessionRuntime`

优点：

- UI 状态、资产配置、网络 runtime 边界清楚；
- 更适合后续接入真实 tab、SFTP、重连；
- 不会把 Tokio / SSH 对象塞进 `ShellViewModel`。

缺点：

- 首轮 plumbing 稍多；
- 需要新增事件桥接层。

##### 方案 B：把连接对象直接放入 `ShellViewModel`

优点：

- 短期实现看似更快。

缺点：

- 强耦合；
- 后续维护成本高；
- 与当前 `ShellViewModel` 作为 UI state 容器的职责相冲突。

#### 最终决策

选择方案 A。

#### 分层定义

##### `ConnectionProfile`

职责：

- 表示 SSH 资产的稳定配置；
- 供 `Save / Connect / Test Connection / Save and Connect` 复用；
- 不持有 UI 临时状态或网络句柄。

建议字段：

- `asset_id`
- `name`
- `host`
- `user`
- `port`
- `auth_method`
- `credential_ref`
- `private_key_path`
- `remark`
- `known_host_policy`

##### `SessionHandle`

职责：

- 作为 UI 可观察会话对象；
- 为 tab、workspace、context menu 提供统一 session 入口。

建议字段：

- `session_id`
- `asset_id`
- `title`
- `subtitle`
- `state`
  - `connecting`
  - `connected`
  - `disconnected`
  - `error`
- `terminal_id`
- `can_reconnect`

##### `SshSessionRuntime`

职责：

- 持有 `russh` client、channel、runtime task；
- 管理交互式 shell 生命周期；
- 将远端输出送入 terminal core；
- 将 UI 输入编码后送回 SSH channel；
- 处理 resize、disconnect、exit-status、错误上报。

#### terminal / SSH glue 边界

- `wezterm-term`
  - 作为 terminal state core；
  - 消费远端字节输出；
  - 向 renderer 提供 screen state。

- `termwiz`
  - 作为键盘、鼠标、控制序列输入辅助；
  - 负责把 UI 输入事件编码成发送到 channel 的字节。

- `russh`
  - 负责 transport、认证、channel、PTY、shell、window change。

#### 首轮必须落地的 runtime 边界

- 真实 SSH 建连；
- password / private key 认证；
- PTY 请求；
- interactive shell；
- I/O pump；
- resize；
- 断开 / 失败态反馈。

#### 首轮明确不做

- `russh-sftp` UI；
- proxy/tunnel 实连；
- 自动重连；
- session 恢复；
- 多 pane；
- agent forwarding。

### 设计点 5：host key 与 `known_hosts`

#### 备选方案

##### 方案 A：`Strict`

未知 host key 直接失败。

优点：

- 语义最严格。

缺点：

- 首次连接体验较差；
- 与成熟桌面 SSH 客户端默认心智不符。

##### 方案 B：`TOFU`

首次未知 host key 弹确认，接受后写入 `known_hosts`；后续变化阻断。

优点：

- 首次连接 UX 与安全语义平衡较好；
- 与已确认需求一致；
- 能为 `Test Connection` 与 `Connect` 复用相同策略。

缺点：

- 需要新增 host key 确认 UI 与存储层。

##### 方案 C：`Trust Always`

完全不校验。

优点：

- 实现最简单。

缺点：

- 不符合本项目定位；
- 语义不可接受。

#### 最终决策

选择方案 B。

补充约束：

- 首次未知 host key：
  - modal 内或连接前流程弹确认；
  - 用户接受后写入 `known_hosts`；
  - 继续连接。
- host key 变化：
  - 直接阻断；
  - 不允许静默覆盖；
  - 错误信息明确指出 host key mismatch。

### 设计点 6：tab / session 模型

#### 备选方案

##### 方案 A：默认同资产复用已有 tab，显式 `Open in New Tab` 才多开

优点：

- 与用户确认一致；
- 与 context menu 里已有 `Open in New Tab` 占位动作相契合；
- 减少重复 tab 污染；
- 更容易让资产树与 workspace 形成稳定映射。

缺点：

- 需要维护 `asset_id -> active session` 索引；
- 同资产多 session 时要处理标题去重。

##### 方案 B：每次都创建新 tab

优点：

- 创建逻辑简单。

缺点：

- 很容易生成重复 tab；
- 资产与 tab 的关系模糊；
- 与已确认方向不一致。

#### 最终决策

选择方案 A。

#### tab 语义

- `tab` 是 session 的 UI 投影，不是资产本身；
- 每个 tab 对应一个 `SessionHandle`；
- 默认情况下，一个 SSH 资产最多只有一个“默认活跃 session tab”；
- 用户通过 `Open in New Tab` 才能为同一资产打开第二个 session。

#### 标题规则

- 主标题优先 `Name`；
- `Name` 为空时 fallback `Host`；
- 副标题显示 `user@host`；
- 同资产多 session 时，在标题尾部增加轻量后缀区分，如 `#2`。

#### 关闭 / 断线 / 重连

- 关闭 tab：
  - 主动关闭对应 session/channel；
  - 从 tab model 移除；
  - 更新资产到 session 的索引。

- 连接中断：
  - tab 不自动消失；
  - 保留为 `Disconnected` 或 `Error` 状态；
  - 允许 `Reconnect`。

- `Test Connection`
  - 不创建 tab；
  - 不污染 session 索引。

## 方案对比汇总

| 设计点 | 备选方案 | 最终选择 | 选择理由 |
| --- | --- | --- | --- |
| 表单信息架构 | 单页分组 vs 保留 tab 壳层 | 保留 tab 壳层，只补强 `Standard` | 最契合当前代码结构，改动范围可控 |
| 认证能力 | 密码 only vs 密码+私钥 | 密码 + 私钥 | 与已确认需求一致 |
| 动作模型 | 双动作 vs 四动作 | 四动作 | 资产管理与连接动作分离更清楚 |
| runtime 分层 | 直接塞入 UI state vs 三层分离 | 三层分离 | 可维护性与后续扩展更好 |
| host key | strict vs TOFU vs trust always | TOFU | 首次连接体验与约束平衡最好 |
| tab 模型 | 总是新建 vs 默认复用 | 默认复用，显式多开 | 更符合桌面终端心智与当前资产树语义 |

## 最终决策

本轮确认后的总设计为：

1. 保留现有 `SSH modal` 的 tab 外壳；
2. `Standard` 成为首轮完整主路径；
3. 首轮支持 `Password` 与 `Private Key` 两类认证；
4. 密码、私钥内容、passphrase 等敏感信息可保存到系统凭据存储；
5. `Private Key` 既可导入内容，也可只保存路径；
6. 保留 `Save / Connect / Test Connection / Save and Connect` 四动作；
7. `Connect` 默认执行配置归一化与保存，再发起连接；
8. runtime 采用 `ConnectionProfile + SessionHandle + SshSessionRuntime` 三层；
9. 首轮只做真实交互式 SSH shell，不做 `russh-sftp` UI；
10. host key 策略采用 `TOFU`；
11. tab 默认按资产复用已有 session，显式 `Open in New Tab` 才多开；
12. 断线后保留 tab，进入 `Disconnected / Error` 状态，并支持重连。

## 实施步骤

以下为设计级实施顺序，不是 implementation plan 的细颗粒任务拆分。

### 阶段 1：表单与资产模型收敛

- 扩展 SSH draft state 与资产配置模型；
- 为 `Standard` 页补齐标签、分组、认证切换与 inline validation；
- 明确 `Password`、`Private Key Content`、`Private Key Path`、`Remark` 的显示/隐藏规则；
- 为四动作建立独立 callback 语义，而不是继续复用单一 `confirm-requested`。

### 阶段 2：凭据与 host key 服务

- 接入系统凭据存储抽象；
- 建立 `credential_ref` 写入与读取路径；
- 新增 `known_hosts` 读写与 host key 比对服务；
- 设计 `TOFU` 确认 UI 与错误反馈模型。

### 阶段 3：session/runtime 与 terminal core

- 引入 `wezterm-term`、`termwiz`、`russh`；
- 建立 `ConnectionProfile -> SshSessionRuntime` 启动链路；
- 实现 interactive shell session、I/O pump、resize；
- 将 terminal state 暴露给 renderer 层。

### 阶段 4：tab / workspace 真实化

- 为 `TabBar` 引入真实 model；
- 用 `SessionHandle` 驱动 tab 标题、状态与激活逻辑；
- 将主工作区从固定 `WelcomeView` 切换到“无 session 时 welcome，有 session 时 terminal surface”；
- 接通资产树、context menu、tab 之间的复用与多开语义。

### 阶段 5：错误流与验证

- 统一连接失败、认证失败、host key 冲突、凭据缺失、连接中断等错误流；
- 为 `Test Connection`、`Connect`、`Save and Connect` 做分流验证；
- 补充单元测试、UI 状态测试、smoke test。

## 风险与回滚策略

### 风险 1：四动作模型导致状态分支膨胀

表现：

- `Save / Connect / Test Connection / Save and Connect` 共用校验与连接路径，容易出现重复逻辑或边界不一致。

缓解：

- 强制共用统一的“draft 归一化 + 校验 + 凭据解析 + host key 处理”流水线；
- 只在结果落点上分流。

回滚：

- 若实施阶段证明四动作过重，可回退到“保留按钮 UI，但内部先合并 `Connect` 与 `Save and Connect` 语义”，再继续收敛。

### 风险 2：`Private Key` 双模式增加表单与存储复杂度

表现：

- 导入内容与路径模式会放大字段切换、校验和凭据写入逻辑。

缓解：

- 在 `Authentication Type` 下进一步显式区分 `Private Key Source`；
- 内容模式与路径模式只允许一条活跃路径。

回滚：

- 若首轮阻力过高，可保持 UI 结构不变，但暂时只启用其中一种模式，不破坏字段模型。

### 风险 3：host key `TOFU` 需要额外确认 UI

表现：

- `Connect` 与 `Test Connection` 都会遇到首次 host key 确认，流程设计不当会打断用户。

缓解：

- 把 `TOFU` 确认视作连接前置步骤，而不是失败后的异常态；
- 统一为同一套确认组件。

回滚：

- 若确认 UI 在首轮难以及时落地，可暂时把未知 key 视为可恢复阻断错误，由二次明确操作继续，不改动整体策略。

### 风险 4：workspace placeholder 向真实 terminal surface 过渡时耦合过高

表现：

- 若直接在 `AppWindow` 层硬接 runtime，后续 tab、多 session、断线态会迅速复杂化。

缓解：

- 坚持 `SessionHandle` 作为 UI 边界；
- `ShellViewModel` 只持有 UI state 与 session 引用，不直接持有 `russh` 对象。

回滚：

- 若 terminal surface 首轮仅能落单 session，可先保持单 tab model，但不改变 `SessionHandle` 与 tab 数据结构。

## 验证清单

### 表单与动作

- `Standard` 页中所有输入项都有明确标签；
- `Password` 和 `Private Key` 切换时，字段显示/隐藏正确；
- 必填项缺失时，按钮禁用与 inline error 一致；
- `Save` 不创建 tab；
- `Connect` 创建或激活 tab；
- `Test Connection` 不创建 tab；
- `Save and Connect` 先保存后连接。

### 连接能力

- password auth 可成功连接；
- private key path 模式可成功连接；
- private key content 模式可成功连接；
- host key 首次未知时触发 `TOFU`；
- host key 变化时被阻断；
- resize 能正确通知远端 PTY；
- 连接中断后 tab 进入错误态而不是直接消失。

### tab / session

- 同一资产重复打开时默认激活已有 tab；
- `Open in New Tab` 能创建第二个 session；
- tab 标题优先 `Name`，无 `Name` 时 fallback `Host`；
- 关闭 tab 后 session 被释放；
- 断线 tab 可触发 `Reconnect`。

### 回归

- 资产树、modal、context menu 基线不被破坏；
- `WelcomeView` 仅在没有 session 时显示；
- 当前 `Proxy / Environment / Advanced / Tunnel` 占位页不误导为已连通功能。

