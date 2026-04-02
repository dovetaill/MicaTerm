# Asset Sync Background Service Design

日期: 2026-04-02
执行者: Codex
状态: 已确认

## 背景

当前资产同步已经具备 `debounced auto sync`、`manual sync`、`periodic refresh` 的基本能力，但实际产品体验仍然存在两个直接影响首发质量的问题：

- 编辑并保存 SSH 资产时，云同步仍可能阻塞应用主线程，用户会体感到明显卡顿，严重时 Windows 任务管理器会显示“未响应”。
- `Sync Settings` modal 目前只强调“配置是否完成”和“是否 ready”，没有提供用户真正关心的状态确认信息：本地最近什么时候同步过、远端最近什么时候更新过、当前看到的状态是不是最新。

从现有实现看，这不是单一 bug，而是同步编排职责仍然滞留在 `bootstrap` UI 装配层的结果：

- 资产保存、重命名、删除、keychain 变更等多个入口都直接在 UI 层打 `dirty` 并启动同步调度。
- 同步后台执行能力已经存在，但主启动装配与测试装配并不完全一致，导致“测试路径可后台化，正式主路径仍可能回落为前台执行”的风险没有从结构上被消除。
- 本地 durable sync metadata 与远端 `committed_at` 已经存在，但 view model 与 Slint 合同没有把这些字段映射成 UI 状态。

本轮设计不再继续做局部补丁，而是确认一轮“收敛型结构升级”：

- 抽离 `VaultSyncService` 统一承担同步编排；
- 保证所有远端 I/O 都脱离 UI 线程；
- 让 `Sync Settings` 成为轻量状态确认面，而不是只有配置表单。

## 目标

### 本轮目标

- 抽离 `VaultSyncService`，统一处理：
  - `manual sync`
  - `debounced auto sync`
  - `periodic refresh`
  - `open sync settings -> refresh remote head`
- 保证资产保存操作先本地完成，远端同步在后台静默执行，不能阻塞 UI。
- `Sync Settings` modal 打开时后台静默读取一次 primary remote head，并刷新远端最新状态。
- `Sync Settings` modal 增加极简状态卡，至少展示：
  - `Local last sync`
  - `Remote last update`
  - 当前已知 primary revision / sync result
- 同步成功默认静默，不额外打断用户；同步失败以非阻塞方式暴露，并保留在设置面中可回看。

### 体验目标

- 保存 SSH 资产时，界面反馈应与“普通本地保存”一致，不能再出现整窗体冻结感。
- 用户打开 `Sync Settings` 后，可以直接判断：
  - 当前 primary 是否已配置；
  - 本地最近何时成功同步；
  - 远端当前最新 revision / 更新时间；
  - 当前是否存在待同步本地变更或失败状态。
- `Sync Settings` 仍以配置为主，但补足“确认状态是否最新”的产品职责。

## 非目标 / 边界

本轮不包含：

- 重写 `SyncEngine`、merge 策略或 provider 协议。
- 变更 `VaultSnapshot`、`VaultHead`、`LocalVaultBootstrapState` 的核心存储语义。
- 引入完整的 sync history / revisions 列表界面。
- 在首屏 UI 中额外展开 mirror 逐个时间状态。
- 改造 provider 为真正事件推送模型；远端状态仍采用拉取式刷新。

边界约束：

- 远端最新状态仅针对 `primary remote`。
- 时间展示层必须兼容当前两类时间格式：
  - 本地 durable metadata 中现有的 epoch-millis 字符串；
  - 远端 `committed_at` 与测试数据里可能出现的 ISO8601 字符串。
- 成功同步默认静默，不把“同步完成”升级为新的强提示交互。
- 失败状态必须可回看，但不能重新把资产保存动作变成“保存并等待同步完成”。

## 当前实现现状

### 1. 后台同步基础设施已存在，但主启动路径存在回落风险

当前 `run_vault_sync` 已支持在有 runtime handle 时走 `tokio::task::spawn_blocking`，并通过 channel + completion timer 回主线程更新状态：

- `src/app/bootstrap.rs:6878`
- `src/app/bootstrap.rs:6941`

但正式主启动路径 `bind_top_status_bar_with_profile_and_async_handle()` 在构造 `session_bridge` 后，把 `session_runtime_guard` 传成了 `None`：

- `src/app/bootstrap.rs:9981`

而 `run_vault_sync` 是否进入后台分支，依赖的是：

- `session_runtime_guard.as_ref().map(AppAsyncRuntime::handle)`
- `src/app/bootstrap.rs:6874`

这意味着当前代码结构下，主程序路径仍存在落回前台同步分支的高风险：

- `src/app/bootstrap.rs:7053`

### 2. 资产保存已经直接接入同步调度

SSH 资产保存成功后会直接触发：

- `mark_local_vault_dirty_and_arm_sync(...)`
- `src/app/bootstrap.rs:8917`

而该函数会：

- 立刻写入 `last_local_change_at`
- 立刻更新 modal 状态文案为 `Local changes queued for background sync.`
- 启动防抖定时器

相关代码：

- `src/app/bootstrap.rs:6131`

因此“保存 SSH 资产 -> 很快触发同步”是既定产品契约，不是偶发行为。

### 3. 本地与远端同步时间元数据已存在，但没有进入 UI 合同

本地 durable metadata 已经持久化：

- `last_local_change_at`
- `last_successful_push_at`
- `last_successful_pull_at`
- `last_sync_error`

相关定义：

- `src/app/vault/bootstrap.rs:35`

远端 `VaultHead` 也已经带有 `committed_at`：

- `src/app/vault/model.rs:49`

但当前 `SyncModalViewState` 没有任何时间字段：

- `src/shell/view_model.rs:180`

Rust -> Slint 同步也没有对应 setter：

- `src/app/bootstrap.rs:707`

`AppWindow` 与 `SyncVaultModal` 组件合同同样没有时间属性：

- `ui/app-window.slint:35`
- `ui/components/sync-vault-modal.slint:102`

### 4. `Sync Settings` 当前仍是配置/诊断面，不是状态确认面

当前 `update_sync_modal_for_local_state()` 在 `Ready` 状态下只渲染：

- `headline = "Sync ready"`
- `status_text = "Use the titlebar Sync button..."`

相关代码：

- `src/app/bootstrap.rs:4803`

这解释了为什么用户现在无法从 modal 判断“我看到的是不是最新状态”。

### 5. 近期 Git 历史说明问题已经从“补逻辑”演进到“该收口结构”

近期关键演进：

- `a32929d`：首次把资产保存路径接入同步
- `ff5b0c4`：引入 debounce sync scheduler
- `0f1fe7a`：推进 always-on background sync
- `939414f`：手动 sync 背景化
- `72f19d2`：自动/周期同步背景化补强
- `4d085c5`：加入 durable sync metadata

这些提交说明：

- 当前产品已经接受“资产变更自动触发同步”
- 当前产品也已经接受“同步必须后台化”
- 但同步编排仍然散落在 UI 装配层，没有形成独立服务边界

## 设计要点拆分

## 设计要点 1：同步编排职责是否从 UI 装配层抽离

### 方案 A：继续沿用当前闭包式调度链，仅修主路径回落问题

优点：

- 改动面最小
- 可以最快修复当前主线路阻塞风险
- 基本不碰现有测试结构

缺点：

- `manual / auto / periodic / modal-refresh` 入口仍散落在 `bootstrap.rs`
- 未来再加同步入口时，仍容易出现“某条路径忘了走后台”的回归
- 不能从结构上解决“同步逻辑过度贴近 UI”问题

### 方案 B：抽离 `VaultSyncService`，统一接管同步事件编排

优点：

- 可以把“UI 提交意图”和“后台执行同步”彻底解耦
- 适合统一承接：
  - `manual sync`
  - `debounced auto sync`
  - `periodic refresh`
  - `refresh remote head on modal open`
- 能从结构上消除“某个入口回落到前台同步”的风险
- 更容易维护一致的 `dirty / running / in-flight / last-known-remote-head` 状态

缺点：

- 改动范围明显大于局部补丁
- 需要明确 service 与 view model / vault session / UI 状态之间的边界

### 最终决策

采用方案 B。

本轮不做“全栈重写型重构”，而是做“收敛型结构升级”：

- 保留现有 `SyncEngine`、provider、merge 逻辑；
- 只把同步编排、排队、去重、状态回流从 UI 装配层收拢到 `VaultSyncService`。

## 设计要点 2：`Sync Settings` 打开时如何获取远端最新状态

### 方案 A：仅展示本地缓存的“上次已知远端状态”

优点：

- modal 秒开
- 不新增打开时网络请求

缺点：

- 用户无法确认打开当下的远端是否已经变化
- 很容易出现“时间有显示，但不是当前最新”的误导

### 方案 B：打开 modal 后后台静默读取一次 primary remote head

优点：

- 最符合“用户需要确认当前是不是最新”的诉求
- 可以保持 modal 先打开，再异步刷新，不阻塞交互
- 与后台同步服务的职责边界一致

缺点：

- 每次打开 modal 都会增加一次轻量远端读取
- 需要补去重、超时、失败状态处理

### 最终决策

采用方案 B。

行为约束：

- 打开 modal 时先显示本地已知状态；
- 同时后台发起一次 `read_head(primary)`；
- 读取成功后刷新 `Remote last update / remote revision`；
- 读取失败时仅更新状态卡错误信息，不阻塞 modal 打开。

## 设计要点 3：同步时间信息的展示粒度

### 方案 A：极简状态卡

展示：

- `Local last sync`
- `Remote last update`
- `Primary revision`
- 简短 sync result / warning

优点：

- 信息密度合适
- 最符合“快速确认是否最新”
- 不会把设置 modal 变成诊断控制台

缺点：

- 看不到 `last push` 与 `last pull` 的细分差异

### 方案 B：完整诊断卡

展示：

- `Last sync`
- `Last push`
- `Last pull`
- `Pending local change`
- `Remote revision/time`

优点：

- 诊断能力更强

缺点：

- 认知负担更高
- 更容易把 modal 做成“工程面板”

### 最终决策

采用方案 A。

定义约束：

- `Local last sync` = `max(last_successful_push_at, last_successful_pull_at)` 的展示值
- `Remote last update` = 当前已知 primary head 的 `committed_at`
- 如没有值，则清晰显示 `Never synced` / `Unknown`

## 设计要点 4：保存后的同步反馈策略

### 方案 A：成功静默，失败非阻塞暴露

优点：

- 最符合“用户不该感知同步”的原始诉求
- 保存动作可以保持纯本地完成的流畅体验
- 同步失败仍能在状态区和 modal 中回看

缺点：

- 如果状态可见性设计不够清晰，成功会显得“没反馈”

### 方案 B：成功也给轻提示

优点：

- 用户更容易意识到同步在工作

缺点：

- 仍会增加“同步存在感”
- 高频保存时容易形成噪音

### 最终决策

采用方案 A。

具体要求：

- 资产保存成功，只反馈本地保存结果；
- 后台同步成功不额外弹强提示；
- 后台同步失败时，更新 sync 状态卡与非阻塞错误状态；
- 不能让失败重新阻塞保存流程。

## 方案对比

| 设计点 | 备选方案 | 结论 | 选择理由 |
| --- | --- | --- | --- |
| 同步编排职责 | A 局部修补 / B 抽 `VaultSyncService` | 选 B | 当前入口已过多，继续补丁式修复回归风险高 |
| modal 远端状态刷新 | A 只看缓存 / B 打开后后台读 head | 选 B | 用户必须能确认打开当下的远端是否最新 |
| 时间展示粒度 | A 极简状态卡 / B 诊断卡 | 选 A | 首发重点是确认状态，不是做诊断控制台 |
| 保存后反馈 | A 成功静默失败显式 / B 成功也提示 | 选 A | 最符合“用户不该感知同步”的要求 |

## 最终决策

### 1. 新增 `VaultSyncService`

`VaultSyncService` 作为唯一同步编排入口，统一处理：

- `request_manual_sync`
- `mark_dirty_and_schedule`
- `request_periodic_refresh`
- `request_remote_head_refresh`

核心职责：

- 排队和去重
- 保证远端 I/O 脱离 UI 线程
- 汇总结果并回流主线程
- 维护“最近已知远端 head 状态”

### 2. UI 侧只提交意图，不直接决定同步执行方式

UI 层保留：

- 资产保存成功后通知“本地已变更”
- titlebar / modal 发起“手动 sync”
- `Sync Settings` 打开时请求“刷新远端 head”

UI 层不再负责：

- 直接拼装后台任务
- 直接判断某条路径是否应该走前台还是后台
- 直接管理 `dirty / running` 的执行细节

### 3. `Sync Settings` 增加极简状态卡

状态卡最少展示：

- `Local last sync`
- `Remote last update`
- `Primary revision`
- 一条当前结果/错误文案

首发不展开 `last push / last pull / mirror detail`。

### 4. 时间展示兼容层必须统一

展示层新增统一格式化逻辑：

- 能解析 epoch-millis 字符串
- 能解析 ISO8601
- 输出统一的人类可读格式

如果解析失败：

- 不让 UI 崩溃
- 回退到 `Unknown`

### 5. 保存与同步彻底解耦

最终产品语义：

- `Save` = 本地数据落盘成功
- `Sync` = 后台异步进行

两者不能重新捆绑成“保存并等待远端完成”。

## 实施步骤

1. 明确 `VaultSyncService` 的输入事件、内部状态和结果消息边界。
2. 将现有 `run_vault_sync`、completion channel、timer 驱动逻辑迁移进 `VaultSyncService`。
3. 修正主启动路径与注入路径的 runtime 装配一致性，确保正式主线路也进入后台执行模型。
4. 将资产保存、重命名、删除、keychain 变更入口统一改为调用 service 的“mark dirty”接口，而不是直接操作底层调度细节。
5. 将 titlebar `Sync now` 与 modal `Sync now` 改为统一调用 service 的手动同步接口。
6. 为 `Sync Settings` 打开事件接入“后台刷新 primary remote head”的请求入口。
7. 扩展 `SyncModalViewState`、`AppWindow` 与 `SyncVaultModal` 合同，加入极简状态卡所需字段。
8. 新增统一时间格式化层，把本地 durable metadata 与远端 `committed_at` 转为稳定展示文本。
9. 调整 `update_sync_modal_for_local_state()`，让 ready 状态从“纯静态说明”升级为“状态卡 + 配置表单”。
10. 校准成功静默、失败非阻塞暴露的反馈策略，确保失败可回看、成功不打断。

## 风险与回滚策略

### 风险 1：服务抽取范围失控

风险：

- 容易顺手把 provider、merge、snapshot 也一起重构，超出本轮边界。

控制策略：

- 只收口“编排层”，不改 `SyncEngine` 契约与 provider 行为。

回滚策略：

- 若 service 抽离后不稳定，可先保留旧同步实现分支，在不改 provider 的前提下回退到“旧编排 + 主路径后台修正”。

### 风险 2：modal 打开即刷新远端状态导致重复请求

风险：

- 用户频繁打开关闭 modal 时，可能产生短时间重复 `read_head`。

控制策略：

- service 层去重；
- 同一时间仅允许一个 in-flight remote head refresh；
- 失败结果短时间内可复用，避免风暴式重试。

回滚策略：

- 若读 head 频率过高，可回退为“modal 打开时仅在缓存过期后刷新”。

### 风险 3：时间兼容解析不一致

风险：

- 当前历史数据中既有 epoch-millis，也有 ISO8601 测试样例。

控制策略：

- 使用统一解析/格式化入口；
- 所有 UI 文案都依赖同一个 helper。

回滚策略：

- 若存在未知旧格式，先回退为显示原始字符串或 `Unknown`，不阻塞主功能。

### 风险 4：成功静默导致“用户没感知”

风险：

- 如果状态卡信息太弱，用户仍可能怀疑是否同步成功。

控制策略：

- 状态卡必须稳定显示：
  - `Local last sync`
  - `Remote last update`
  - `Primary revision`

回滚策略：

- 若测试后用户仍觉得反馈不足，可再追加轻量状态文案，但不回到阻塞式等待。

## 验证清单

- [ ] 主启动路径保存 SSH 资产时，在 slow provider 条件下不阻塞 UI 线程。
- [ ] 注入测试路径与正式主启动路径使用一致的后台同步执行语义。
- [ ] `manual sync`、`debounced auto sync`、`periodic refresh` 都通过 `VaultSyncService` 统一编排。
- [ ] 打开 `Sync Settings` 时，modal 先正常打开，再后台静默刷新 primary remote head。
- [ ] remote head 刷新失败不会阻塞 modal，只会更新错误状态。
- [ ] `Sync Settings` 能显示 `Local last sync`。
- [ ] `Sync Settings` 能显示 `Remote last update`。
- [ ] `Sync Settings` 能显示当前 primary revision 或明确的 `Unknown` / `Never synced`。
- [ ] 时间格式化层能同时处理 epoch-millis 与 ISO8601。
- [ ] 保存成功后不会因为同步动作造成新的强提示打断。
- [ ] 同步失败后用户可以在状态卡或 modal 中看到错误，而不影响本地保存结果。
- [ ] 现有 Git primary、merge、conflict inbox 契约不被本轮服务抽取破坏。
