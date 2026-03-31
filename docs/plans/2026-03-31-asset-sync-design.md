# Asset Sync Design

日期: 2026-03-31
执行者: Codex
状态: 已确认，待实现

## 背景

当前资产同步链路已经不是单一缺陷，而是产品契约和实现路径一起偏离了可商用状态：

- 用户完成一次同步配置后，后续仍会被迫再次进入 `Unlock` / `Sync now` 分步流程，交互负担过重。
- `local_state == None` 且远端非空时，输入 master password 会优先从远端恢复并直接覆盖当前 shell snapshot；如果用户此前已经有本地改动，这些改动不会被保护。
- `vault-head.json` 缺少真实的同步时间语义，当前提交路径甚至把 `created_at` 固定写成常量，导致头文件中的时间字段不可信。
- 每次写入新 revision 都会新增一组 `vault-{revision}-manifest.bin` 和 `vault-{revision}-pack-0000.bin`，当前没有任何历史保留上限。
- 本地增删改虽然已经接入一部分 auto-sync 调度，但前提条件过多，用户体感仍然是“改了也不一定同步”。

本轮设计的目标不是继续给现有状态机补按钮，而是把同步改成用户低感知、默认后台运行、能自动决定 push / pull 的正式产品能力。

## 目标

### 本轮目标

- 去掉用户可见的 `Lock / Unlock` 产品动作。
- 首次完成同步配置后，后续重启应用不应再次要求重新配置。
- 启用同步后默认后台同步，不再依赖首发 UI 中的 `auto_sync_enabled` 开关。
- 本地资产增删改完成后，进入短防抖实时同步。
- 同步方向默认自动决定，不要求用户在日常场景中拍板 `Use local` 或 `Use remote`。
- `vault-head.json` 记录真实远端提交元数据；本地单独维护同步状态。
- revision 历史采用有上限保留，默认仅保留最新 `10` 个 revision。
- 当自动决策导致一侧失效时，失败侧必须先进入 recovery 备份，而不是静默丢失。

### 体验目标

- 用户只理解两件事：
  - 第一次配置同步。
  - 之后同步默认后台运行，顶部 `Sync` 只是一键立即同步或立即校验。
- 本地改动后的同步应该尽量“像实时”，而不是依赖用户再次打开设置。
- 不再出现“先输入 master password，再 unlock，再点 sync”的多段式交互。

## 非目标 / 边界

本轮不包含：

- 设计复杂的冲突合并 UI。
- 引入 CRDT、逐条操作日志、多人协作合并。
- 重做 vault 加密格式本身。
- 在首发正式 UI 中同时开放多类 provider 选择器。
- 在本设计文档中展开逐任务实施细节；implementation plan 单独整理。

边界说明：

- 本轮仍沿用 snapshot-based sync，而不是切换为 per-asset oplog。
- “无感知自动决策”不等于静默丢数据；系统必须在后台先做 recovery 备份。

## 当前实现现状

### 1. 用户可见状态机过重

当前 `Sync Settings` modal 仍然按 `NotConfigured / Locked / UnlockedButRemoteIncomplete / Ready` 驱动，并对外暴露 `Unlock`、`Lock`、`Sync now` 等动作：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4311)

这套状态机让“配置同步”和“解密恢复会话”混在一起，导致用户在首发体验上承担了过多内部概念。

### 2. 解锁与远端恢复都会直接替换本地视图

当前 `submit_sync_modal_master_password()` 的逻辑是：

- 有本地 bootstrap state 时，直接 `unlock_local_vault_into_shell(...)`
- 没有本地 bootstrap state 时，优先 `recover_local_vault_from_primary_remote(...)`

关键问题是这两条路径都会直接 `apply_vault_snapshot_to_shell(...)`，当前内存中的资产树不会先做保护：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4434)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4566)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4669)

### 3. 当前 auto-sync 是“可用但不可靠”

现有 scheduler 已具备：

- 本地 mutation 后 `dirty = true`
- `1.2s` 防抖 auto-sync
- `120s` 周期轮询

但它仍然受制于 `local_state + unlocked_vault_key + auto_sync_enabled` 三重条件：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4948)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4954)
- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L5553)

因此用户体感仍然是“我改了，但不一定真的同步了”。

### 4. 远端头文件元数据不完整且时间语义错误

`VaultHead` 当前只包含 `created_at`，没有真实的 `committed_at` / `last_synced_at`：

- [model.rs](/home/wwwroot/mica-term/src/app/vault/model.rs#L49)

同时 `sync_local_vault()` 里构造 `SyncRequest` 时把 `created_at` 固定写成 `2026-03-28T00:00:00Z`：

- [bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L4789)

这意味着当前 `vault-head.json` 中的时间字段没有可靠产品价值。

### 5. revision 文件会持续增长

`GITEE_MAX_PACK_COUNT` 和 `ProviderCapabilities.max_pack_count` 约束的是“单个 revision 最多切几个 pack”，不是“历史保留几个 revision”：

- [gitee_gist.rs](/home/wwwroot/mica-term/src/app/vault/provider/gitee_gist.rs#L16)
- [provider/mod.rs](/home/wwwroot/mica-term/src/app/vault/provider/mod.rs#L12)

provider 每次写新 revision 时都会生成新的：

- `vault-head.json`
- `vault-{revision}-manifest.bin`
- `vault-{revision}-pack-0000.bin`

且当前没有 prune 逻辑：

- [gitee_gist.rs](/home/wwwroot/mica-term/src/app/vault/provider/gitee_gist.rs#L363)

### 6. 现有测试已经锁定了旧契约

最近提交已把以下行为固化为测试预期：

- 远端非空时允许 remote recovery，但不上传空本地
- unlock 本身不触发 push

关键测试：

- [bootstrap_smoke.rs](/home/wwwroot/mica-term/tests/bootstrap_smoke.rs#L2151)
- [bootstrap_smoke.rs](/home/wwwroot/mica-term/tests/bootstrap_smoke.rs#L2370)

这意味着新方案是正式产品契约调整，不是单纯修一处 bug。

## 设计要点拆分

## 设计要点 1：会话解密与用户交互模型

### 方案 A：保留当前显式 `Lock / Unlock`

优点：

- 最贴近当前代码
- 迁移成本最低

缺点：

- 用户每次都要理解 vault 状态机
- 与“首次配置后低感知同步”的目标直接冲突
- 继续制造 `Unlock -> Sync` 的冗余步骤

### 方案 B：隐藏 `Lock`，保留后台会话解密状态

优点：

- 用户界面更简单
- 仍可内部保留已解密/未解密状态

缺点：

- 仍然会把“解密会话”作为独立产品概念保留下来
- 仍可能在重启后要求显式恢复动作

### 方案 C：取消用户可见 `Lock / Unlock`，首次启用后自动恢复

优点：

- 最符合“配置一次后不用再管”
- 与 Tabby、1Password 一类产品的日常体验一致
- 顶部 `Sync` 可以回归成纯同步动作

缺点：

- 需要补一套自动恢复材料持久化策略

### 推荐方案

采用方案 C。

最终决策：

- 不再在首发 UI 中暴露 `Lock / Unlock`。
- 首次启用同步后，将用于自动恢复的 vault key material 保存到共享 credential store。
- 应用重启后自动恢复同步能力，不再要求再次配置，也不要求再次显式 unlock。

## 设计要点 2：自动决定 push / pull 的策略

### 方案 A：最新 `committed_at` 获胜，失败侧自动做 recovery 备份

优点：

- 用户无需参与日常冲突判定
- 与 snapshot-based 架构兼容
- 能保证“无感知同步”和“不可静默丢数据”同时成立

缺点：

- 依赖可信的远端提交时间
- 需要落一套 recovery 备份路径

### 方案 B：永远本地优先

优点：

- 逻辑简单
- 不会让本机用户感知本地被覆盖

缺点：

- 另一设备刚提交的数据可能被覆盖
- 多设备切换场景体验不稳

### 方案 C：永远远端优先

优点：

- 逻辑简单
- 远端可被视作唯一权威

缺点：

- 用户刚在本地完成修改时，最容易感知“改动消失”

### 推荐方案

采用方案 A。

最终决策：

- 本地状态维护 `base_revision`、`local_snapshot_hash`、`last_local_change_at`。
- 远端 head 维护 `vault_revision`、`parent_revision`、`payload_hash`、`committed_at`、`committed_by_device`。
- 若本地与远端都相对 `base_revision` 发生变化：
  - 比较双方 `committed_at`
  - 更新较新的一侧获胜
  - 失败侧在被替换前自动写入 recovery snapshot
- 用户不需要参与日常 push / pull 决策。

## 设计要点 3：同步元数据模型

### 方案 A：只修正现有 `created_at`

优点：

- 改动最小

缺点：

- 仍无法表达“本机上次推送/拉取成功”
- 远端头文件仍承担了过多模糊语义

### 方案 B：远端 head 与本地 sync state 分层

优点：

- 职责清晰
- 方便后续 titlebar 状态反馈和 recovery
- 最符合产品诊断需求

缺点：

- 需要补字段和迁移

### 方案 C：另起独立 meta 文件

优点：

- 远端 head 可以更精简

缺点：

- 会增加多文件一致性复杂度
- provider cleanup 也更复杂

### 推荐方案

采用方案 B。

最终决策：

- `vault-head.json` 改为表达远端提交事实：
  - `vault_revision`
  - `parent_revision`
  - `payload_hash`
  - `committed_at`
  - `committed_by_device`
  - `manifest_ref`
  - `wrapped_vault_key`
- 本地独立维护 sync state：
  - `base_revision`
  - `local_snapshot_hash`
  - `last_local_change_at`
  - `last_successful_push_at`
  - `last_successful_pull_at`
  - `last_sync_error`

说明：

- 当前写死的 `created_at` 常量必须移除。
- `SnapshotSyncPreferences.last_sync_result` 不再承担主要同步状态来源，而改为 local sync state 的附属投影。

## 设计要点 4：revision 历史保留策略

### 方案 A：无限保留全部 revision

优点：

- 实现最简单

缺点：

- gist/snippet/s3 对象会持续膨胀
- 长期不可维护

### 方案 B：固定保留最新 N 个 revision

优点：

- 简单明确
- 首发足够可控
- 最容易在各 provider 上统一实现

缺点：

- 回滚窗口固定

### 方案 C：分层保留

优点：

- 更节省空间
- 历史窗口更灵活

缺点：

- provider 端清理和索引明显更复杂

### 推荐方案

采用方案 B。

最终决策：

- 默认保留最新 `10` 个 revision。
- 新 revision 写入成功后，清理更老的 manifest / pack 文件。
- 对于 object-set provider，也执行等价的 revision cleanup。

## 设计要点 5：同步触发与调度

### 方案 A：保留 `auto_sync_enabled` 开关

优点：

- 贴近当前实现

缺点：

- 会继续制造“为什么我改了没有同步”的困惑
- 与“启用后默认后台同步”冲突

### 方案 B：启用同步后默认后台同步，保留短防抖 + 周期兜底

优点：

- 最符合低感知体验
- 兼顾即时性和稳定性
- 能覆盖本地 mutation、远端变化校验、失败重试

缺点：

- 需要统一所有 mutation 接入点

### 方案 C：改回手动同步为主

优点：

- 容易理解

缺点：

- 产品体验明显退化
- 用户会频繁怀疑数据是否已同步

### 推荐方案

采用方案 B。

最终决策：

- 移除首发正式 UI 中的 `auto_sync_enabled` 开关。
- 所有成功的资产增删改在提交后立即：
  - 更新 `local_snapshot_hash`
  - 标记 `dirty`
  - 进入短防抖 push
- 保留周期兜底任务，用于：
  - pull 远端新 head
  - 重试失败 push
- 顶部 `Sync` 始终表示“立即同步 / 立即校验”，不再承担设置入口。

## 方案对比结论

本次确认后的正式方案组合为：

- 会话模型：`1C`
- 自动决策：推荐方案 A，即“最新 `committed_at` 获胜，失败侧自动 recovery”
- 元数据模型：`3B`
- revision 保留：`4B`
- 调度模型：默认后台同步，去掉首发 `auto_sync_enabled` UI

这组组合满足三个优先级：

- 用户低感知
- 不再要求重复配置与重复 unlock
- 不静默丢失失败侧数据

## 最终决策

1. 去掉用户可见 `Lock / Unlock`，首次启用后自动恢复。
2. 顶部 `Sync` 改为纯同步动作，不再引导用户先 unlock。
3. 同步默认后台运行，本地 mutation 后立即进入短防抖 push。
4. 远端与本地同时变化时，采用“最新 `committed_at` 获胜，失败侧自动 recovery”的无感知策略。
5. `vault-head.json` 改为记录真实远端提交元数据，本地另存 sync state。
6. revision 历史默认仅保留最新 `10` 个。

## 实施步骤

以下为设计级实施步骤，不展开成 implementation plan：

1. 收敛同步产品入口，移除首发 UI 中的 `Lock / Unlock` 与 `auto_sync_enabled` 暴露。
2. 引入本地 durable sync state，替代当前仅靠内存 `dirty` 标记的做法。
3. 为共享 credential store 增加 vault auto-recovery key material 的读写路径。
4. 重写 sync decision pipeline，基于 `base_revision + local_snapshot_hash + payload_hash + committed_at` 自动决定 push / pull。
5. 为失败侧增加 recovery snapshot 持久化。
6. 扩展 remote head / local state schema，替换当前错误的时间字段写入。
7. 为各 provider 增加 bounded retention cleanup。
8. 统一所有资产写入路径的同步触发 helper。

## 风险与回滚策略

### 主要风险

- 自动恢复材料保存路径处理不当，可能导致重启后无法自动恢复。
- `committed_at` 驱动的自动裁决如果未统一时钟来源，可能造成非预期胜负。
- provider cleanup 若实现不完整，可能误删仍在保留窗口内的 revision 文件。
- 现有测试已经锁定旧契约，切换后需要系统性回归。

### 回滚策略

- 在 schema 升级阶段保留旧字段读取兼容，但新写入统一落新格式。
- 在 retention 功能正式开启前，可先只记录候选清理列表并做 dry-run 验证。
- recovery snapshot 写入失败时，禁止继续自动替换失败侧。
- 若自动恢复材料不可读，降级为“同步暂停 + 重新输入 master password 恢复”，但不重新要求配置远端。

## 验证清单

- 应用首次启用同步后，关闭并重新打开，不需要重新配置远端。
- 应用重启后，顶部 `Sync` 不会再要求显式 `Unlock`。
- 任一成功的资产新增、编辑、删除后，都会在短防抖后自动 push。
- 周期任务能在本地无新改动时自动 pull 远端新 revision。
- 本地与远端同时变化时，系统会自动按 `committed_at` 决定胜者，并把失败侧写入 recovery。
- `vault-head.json` 中不再出现固定常量时间，而是实际提交时间。
- provider 存储中超过 `10` 个的旧 revision 会被清理。
- Gitee provider、GitHub provider、S3 provider 的 retention 行为保持一致语义。

## 参考

- 1Password Sync: <https://support.1password.com/sync/>
- 1Password Item History: <https://support.1password.com/item-history/>
- Syncthing Versioning: <https://docs.syncthing.net/users/versioning.html>
- AWS AppSync Conflict Detection: <https://docs.aws.amazon.com/appsync/latest/devguide/conflict-detection-and-resolution.html>
