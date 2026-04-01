# Asset Sync Git Primary Design

日期: 2026-04-01
执行者: Codex
状态: 已确认

## 背景

上一轮 `asset sync` 设计已经把同步从“手动点按钮”推进到“后台自动 sync”，但当前实现仍然保留了一个根本问题：

- 远端与本地的裁决单位仍然是整个 `VaultSnapshot`；
- 双端同时变化时，本质仍是“谁赢了，就整包替换当前工作集”；
- 输掉的一侧虽然会写 recovery snapshot，但不会自动回到当前资产视图；
- 首次 attach 时，如果远端已有数据，而本地设备也已经先录入了很多资产，当前路径仍偏向 remote-first recover。

这会直接触发以下不合理场景：

1. 设备 A 已经同步更新资产，设备 B 离线期间新增了本地资产，重新同步时 B 的新增很可能不会进入当前结果集；
2. 设备 C 在尚未配置 Gitee 的情况下先录入了大量资产，之后再连接同步目标时，本地新增资产存在被远端整包替换的风险；
3. `Gitee Gist` / `GitHub Gist` / snippet 这类对象接口缺少真正的 compare-and-swap 语义，无法承担“强一致 primary”。

因此，本轮设计不再继续强化 `gist/snippet` primary，而是正式切换为：

- 同步粒度：`资产级三方 merge`
- 强一致 primary：`普通 Git 私有仓库 backend`
- 首发平台：`Gitee 普通私有仓库`
- 首次 attach：`attach-time merge`

## 目标

### 本轮目标

- 将同步冲突处理从“整包 snapshot 裁决”升级为“资产级三方 merge”。
- 将强一致 primary 从 `gist/snippet` 载体迁移到 `普通 Git 私有仓库`。
- 首发正式路径支持 `Gitee 普通私有仓库` 作为 primary。
- 在没有用户系统的前提下，仍能稳定地区分“同一 vault 的不同设备”。
- 首发同时支持两种 Git 认证方式：
  - `HTTPS` 凭证方式
  - `SSH key` 方式
- 首次 attach 遇到“本地已有资产 + 远端已有资产”时，执行 `attach-time merge`，而不是 remote-first 或 local-first 覆盖。
- 保留弱一致载体，但不再允许其承担强一致 primary。

### 体验目标

- 用户可以把“同一 vault 的多设备同步”理解为一个长期存在的数据集合，而不是每台设备谁最后覆盖谁。
- 设备首次接入远端时，不再因为 attach 顺序不同而直接丢失本地资产。
- 日常多端切换时，“新增一个资产”默认应自然并入结果，而不是进入隐藏 recovery 文件。
- 认证方式贴近用户已有 Git 使用习惯：既支持 `HTTPS`，也支持 `SSH`。

## 非目标 / 边界

本轮不包含：

- 引入 `PostgreSQL` 或自建云端账号系统。
- 引入通用 `CRDT` / oplog 框架。
- 设计多人实时协作语义。
- 在首发正式 UI 同时开放 `GitHub`、`GitLab`、`S3`、`Gitee` 全部 primary 选择器。
- 在本设计文档中展开逐任务实现拆分；implementation plan 另写。

边界约束：

- 当前仍沿用 `VaultSnapshot` 作为远端传输与加密封装单位。
- “资产级三方 merge”指的是在同步前对 `snapshot` 中的资产域进行语义合并，而不是直接把 snapshot 替换为 oplog。
- `gist/snippet` 后续可保留为导入导出或轻量备份路径，但不再承担强一致 primary。

## 当前实现现状

### 1. 当前自动决策仍是整包 snapshot 胜负

`decide_sync_action()` 当前基于：

- `base_revision`
- `local_snapshot_hash`
- `last_local_change_at`
- 远端 `vault_revision`
- `payload_hash`
- `committed_at`

来判断 `Push / Pull / Noop`，双端都变更时直接比较时间戳：

- `src/app/vault/sync_decision.rs:26`

这意味着当前并不是资产级 merge，而是整包 winner-take-all。

### 2. pull / recover 都是整包替换当前工作集

当前 `apply_vault_snapshot_to_shell()` 会把 snapshot 直接投影成新的资产树和 keychain 视图：

- `src/app/bootstrap.rs:4720`

在 pull 路径里，系统会先清理当前解密态，再应用远端 snapshot：

- `src/app/bootstrap.rs:5097`
- `src/app/bootstrap.rs:5302`

因此当前实现没有“把两边新增自然 union”的过程。

### 3. 首次 attach 仍偏向 remote-first

当本地还没有 bootstrap state 时，`submit_sync_modal_master_password()` 会优先尝试：

- `recover_local_vault_from_primary_remote(...)`

只有远端为空时才会：

- `create_local_vault_from_shell_state(...)`

相关代码：

- `src/app/bootstrap.rs:4618`
- `src/app/bootstrap.rs:4826`
- `src/app/bootstrap.rs:4766`

这就是“设备 C 先录本地资产，再连接同步目标时，本地会被顶掉”的根源之一。

### 4. 当前 `Gitee Gist` 不具备强一致 primary 能力

当前 provider 已明确声明：

- `supports_conditional_head_write = false`

相关代码：

- `src/app/vault/provider/gitee_gist.rs:309`

同时 revision 仍然采用全局递增 `rev-000x` 命名：

- `src/app/bootstrap.rs:4218`

这在多设备基于同一 parent 并发提交时，会产生明显的弱一致窗口。

### 5. recovery 目前主要是后台文件，而不是产品级冲突入口

当前 recovery snapshot 会被持久化到本地文件：

- `src/app/vault/recovery.rs:30`

但 `load_recovery_snapshots()` 目前只在测试中被使用，应用里没有正式冲突收件箱入口：

- `src/app/vault/recovery.rs:59`

这使得“逻辑上没丢”和“用户体感上没丢”仍然是两回事。

## 设计要点拆分

## 设计要点 1：同步粒度与冲突模型

### 方案 A：继续沿用 snapshot-level latest-wins

优点：

- 改动最小
- 延续当前 `SyncDecision` 与 `recovery` 结构

缺点：

- 无法真正解决“多端新增资产应 union”的核心诉求
- 首次 attach 双端有数据时仍需要偏向一侧
- recovery 仍只是事后补救，不是正常结果的一部分

### 方案 B：升级为资产级三方 merge

优点：

- 最符合当前任务目标
- 双端新增可自然 union
- 真正冲突的范围可缩小到单资产或单字段
- 与 `VaultSnapshot` 现有封装兼容，迁移成本可控

缺点：

- 需要补 `base/local/remote` merge contract
- 需要引入 tombstone / conflict copy / merge metadata

### 方案 C：直接改为 CRDT / oplog

优点：

- 并发语义最完整
- 长期理论上最强

缺点：

- 超出当前首发边界
- 与现有 vault/snapshot/provider 结构偏差过大

### 最终决策

采用方案 B：`资产级三方 merge`。

首发不引入 CRDT，而是在保留 snapshot 封装的前提下，把同步前的裁决单位从“整包 snapshot”改成“资产集合”。

## 设计要点 2：强一致 primary 的远端载体

### 方案 A：自建 `PostgreSQL` primary

优点：

- 容易表达事务和 version 列
- CAS 语义明确

缺点：

- 需要引入新的远端服务形态
- 当前没有用户系统，客户端直连数据库会把鉴权、隔离、连接管理一起带进来
- 明显偏离“先基于现有 Git 生态首发”的路径

### 方案 B：普通 Git 私有仓库 primary

优点：

- `commit DAG + parent commit + branch head` 天然适合 optimistic concurrency
- 与 `资产级三方 merge` 模型高度契合
- 不需要先做云端用户系统
- 用户已经熟悉 `Git URL + HTTPS/SSH` 这套接入方式

缺点：

- 需要新建 `GitRepoProvider`
- 需要设计 repo layout、ref 策略和 push/fetch 失败重试

### 方案 C：继续让 `gist/snippet` 做 primary

优点：

- 复用现有 provider 路径最快

缺点：

- 不能兑现“强一致 primary”
- 与本轮确认方向冲突

### 最终决策

采用方案 B：`强一致 primary = 普通 Git 私有仓库 backend`。

本轮明确不把 `PostgreSQL` 作为首发前提。

## 设计要点 3：首发强一致 primary 的平台暴露

### 方案 A：首发只做 `Gitee 普通私有仓库`

优点：

- 与当前项目首发平台和用户环境最匹配
- 国内网络可用性更现实

缺点：

- 容易把 provider 抽象做成平台特化

### 方案 B：首发只做 `GitHub 普通私有仓库`

优点：

- 文档与生态最成熟

缺点：

- 不符合当前首发更偏向 `Gitee` 的现实约束

### 方案 C：内部做 provider-agnostic `GitRepoProvider`，正式 UI 首发只暴露 `Gitee`

优点：

- 架构上正确
- 首发 UI 可控
- 后续扩展 `GitHub` / `GitLab` 成本更低

缺点：

- 首发设计工作量略高于平台写死

### 最终决策

采用方案 C：

- 内部实现 `GitRepoProvider` 通用层；
- 正式 UI 首发只暴露 `Gitee 普通私有仓库`。

## 设计要点 4：首次 attach 时双端都有数据的处理方式

### 方案 A：remote-first

优点：

- 逻辑简单

缺点：

- 会继续吞掉“新设备先录入的本地资产”

### 方案 B：local-first

优点：

- 本机用户感知更稳定

缺点：

- 会反过来覆盖既有远端数据

### 方案 C：attach-time merge

优点：

- 最符合“设备后接入”的自然语义
- 与资产级三方 merge 完全一致
- 可以把首次 attach 和日常 sync 放进同一套冲突规则

缺点：

- 需要定义“无共同 base 时”的 merge 规则

### 最终决策

采用方案 C：`attach-time merge`。

无共同 base 的首次 attach 视为一次“导入现有本地状态 + 导入远端现有状态”的双源合并过程，而不是先判一边是权威。

## 设计要点 5：没有用户系统时，如何标识同一人多设备

### 方案 A：等待未来用户系统

优点：

- 概念上最直观

缺点：

- 会阻塞当前同步架构演进

### 方案 B：使用 `vault identity + device identity`

优点：

- 不依赖在线账号系统
- 与本地优先、端到端加密 vault 的设计天然一致
- 同一人多设备由同一 `vault_id`、bootstrap bundle、vault key 体系来绑定

缺点：

- 需要在本地持久化稳定 `device_id`

### 方案 C：共享远端账号即视为同一用户

优点：

- 最省事

缺点：

- 语义不稳定
- 一个远端账号可以服务多个 vault，也可以被多人共用

### 最终决策

采用方案 B：

- “同一逻辑用户 / 同一套同步资产”由 `vault_id + bootstrap bundle + remote locator` 定义；
- “不同设备”由本地稳定 `device_id` 定义；
- 当前不依赖独立用户系统。

## 设计要点 6：Git 仓库认证方式

### 方案 A：只支持 HTTPS

优点：

- UI 更简单
- 对初学者门槛低

缺点：

- 不满足已有 Git 用户习惯
- 在长期使用中不如 SSH 顺手

### 方案 B：只支持 SSH key

优点：

- 更贴近开发者习惯
- 适合长期 push/pull

缺点：

- 首次配置门槛更高
- 与部分用户现有 HTTPS 使用习惯不兼容

### 方案 C：同时支持 HTTPS 与 SSH key

优点：

- 最符合真实 Git 使用场景
- 用户可按平台政策与个人习惯选择
- 对后续扩展 `GitHub` / `GitLab` 也最合理

缺点：

- 需要额外定义 auth draft、验证与存储策略

### 最终决策

采用方案 C：同时支持两种认证方式。

首发 contract：

- `HTTPS` 模式：`username + secret`
  - 对 `GitHub`，根据官方文档，Git over HTTPS 的“password”实际应为 `personal access token`；
  - 对 `Gitee`，其官方帮助文档明确支持 `HTTPS` 与 `SSH` 两种推拉代码方式，HTTPS 路径按平台现有策略输入账户与对应密钥材料。
- `SSH` 模式：
  - 使用用户提供的 `private key` / `passphrase`
  - 通过 `git@host:owner/repo.git` 这类 remote URL 访问仓库

文档与产品文案层面统一称为：

- `HTTPS credentials`
- `SSH key`

避免把 `GitHub` 的 `PAT` 和 `Gitee` 的密码/令牌策略耦合成同一固定文案。

## 设计要点 7：`gist/snippet` 在新架构中的角色

### 方案 A：继续保留为 mirror

优点：

- 迁移后仍能复用既有 provider

缺点：

- 首发复杂度继续上升
- 容易让用户误解其一致性等级

### 方案 B：降级为 import/export 或轻量备份载体

优点：

- 职责清晰
- 不再与强一致 primary 竞争语义

缺点：

- 正式同步链路会从 gist 路线迁出

### 方案 C：完全移出产品范围

优点：

- 最干净

缺点：

- 放弃已有积累过于彻底

### 最终决策

采用方案 B：

- `gist/snippet` 不再做 primary；
- 首发把它们定位为后续可恢复的 `import/export / backup` 路径，而不是正式强一致同步主链路。

## 方案对比结论

本轮最终方案组合为：

- 同步粒度：`资产级三方 merge`
- 强一致 primary：`普通 Git 私有仓库`
- 首发平台暴露：`内部 GitRepoProvider + UI 首发只暴露 Gitee 普通私有仓库`
- 首次 attach：`attach-time merge`
- 身份模型：`vault_id + device_id`
- 认证方式：`HTTPS credentials + SSH key`
- 旧 `gist/snippet`：`降级为 import/export / backup`

这组决策的核心含义是：

- 不把同步问题继续压在弱一致对象接口上；
- 不为了 CAS 引入新的数据库后端；
- 不等待用户系统先落地；
- 直接用 Git 原生祖先关系承载强一致 primary。

## 最终决策

1. 正式放弃 `gist/snippet` 作为强一致 primary 的定位。
2. 强一致 primary 切换为 `普通 Git 私有仓库 backend`。
3. 首发正式 UI 暴露 `Gitee 普通私有仓库`。
4. 内部 provider 架构保持 `GitRepoProvider` 通用抽象。
5. 同步算法从 snapshot-level latest-wins 升级为资产级三方 merge。
6. 首次 attach 双端都有数据时，执行 `attach-time merge`。
7. 当前阶段不引入用户系统，通过 `vault_id + device_id` 建模同一 vault 的多设备。
8. 首发同时支持：
   - `HTTPS credentials`
   - `SSH key`
9. `gist/snippet` 降级为后续 `import/export / backup` 路径。

## 实施步骤

以下为设计级实施步骤，不展开为 implementation plan：

1. 为同步架构新增 `GitRepoProvider` 抽象，并定义其强一致 primary contract。
2. 设计远端 Git 仓库 layout：
   - branch/ref 约定
   - snapshot object layout
   - merge metadata layout
3. 将本地 durable sync state 从“snapshot 级输赢标记”升级为“资产级 merge 所需元数据”。
4. 为 `asset_catalog`、`keychain_catalog`、secret references 定义三方 merge 规则与 tombstone 规则。
5. 设计首次 attach 的无共同 base merge 流程。
6. 为冲突结果增加产品级入口，而不是只写 recovery 文件。
7. 在 sync settings 中加入 Git remote draft：
   - repo locator
   - auth mode
   - HTTPS credentials
   - SSH key material
8. 首发只接通 `Gitee` 普通私有仓库；其他 Git 平台保留抽象扩展位。
9. 将现有 `gist/snippet` provider 从正式 primary 路径移除，保留为后续 import/export 或 backup 能力。

## 风险与回滚策略

### 主要风险

- 资产级三方 merge 的规则若定义不稳，会出现“逻辑没丢，但结果不可预期”的问题。
- Git repo provider 若先做成平台特化，后续扩展 `GitHub/GitLab` 会再次返工。
- `HTTPS` 与 `SSH` 双 auth 模式会显著增加配置校验和错误处理分支。
- 首次 attach 的无共同 base merge 若处理不当，容易制造重复资产或错误关联。

### 回滚策略

- 在正式切换前，保留当前 snapshot-level sync 路径作为内部兼容 fallback。
- 首批实现时，先将旧 `gist` primary 路径标记为 deprecated，而不是立刻删除全部代码。
- attach-time merge 若发现 merge contract 不满足安全条件，先降级为“只生成 merge preview / conflict inbox，不直接提交远端”。
- Git repo provider 首发只做 `Gitee`，避免平台面同时扩散。

## 验证清单

- 设备 A、B 在离线期间分别新增不同资产，重新同步后两侧都能看到 union 结果。
- 设备 A、B 同时编辑同一资产时，系统会生成可理解的冲突结果，而不是整包覆盖。
- 设备 C 在未接入同步前先录入本地资产，首次 attach 到已有远端仓库时，本地与远端都能进入 attach-time merge。
- `Gitee` 普通私有仓库 primary 的 push/fetch 流程在并发写入时能正确检测非 fast-forward 冲突。
- `HTTPS credentials` 与 `SSH key` 两种 auth 模式都能完成 clone/fetch/push。
- `device_id` 在应用重启后保持稳定，并写入同步元数据与冲突记录。
- recovery 数据在正式 UI 中有可见入口，而不是只能落盘到 JSON。
- `gist/snippet` 不再出现在正式 primary 选择路径中。

## 参考

- Git `git-push` fast-forward 规则：<https://git-scm.com/docs/git-push>
- GitHub Git references API：<https://docs.github.com/rest/git/refs>
- GitHub remote repository auth：<https://docs.github.com/en/get-started/git-basics/about-remote-repositories>
- GitHub personal access token：<https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/managing-your-personal-access-tokens>
- Gitee 通过 HTTPS / SSH 推拉代码：<https://gitee.com/help/articles/4238>
- Gitee SSH 公钥设置：<https://gitee.com/help/articles/4191>
