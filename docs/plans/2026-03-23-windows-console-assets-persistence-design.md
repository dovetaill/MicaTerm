# Windows Console Assets Naming And Persistence Design

日期: 2026-03-23
执行者: Codex
状态: 方案已确认，等待后续 implementation plan

## 背景

当前 `Windows Console` 资产区已经完成 `AssetTree -> ShellViewModel -> bootstrap -> Slint` 的单向状态投影链路，`create / rename / delete` 也已经统一进入 modal workflow。

但本轮仍有两个关键边界没有正式落档：

1. `Create` 链路的同父级重名校验与自动命名规则还没有和 `Rename` 完全对齐；
2. 资产树与 SSH 连接数据尚无本地持久化方案，且本轮明确禁止使用 JSON 与 SQLite。

本设计文档用于确认：

- 同父级唯一命名规则与输入阶段实时校验策略；
- 资产树持久化的数据边界；
- 本地持久化目录策略；
- `redb` 作为首选持久化方案时的文件布局、恢复策略与架构边界；
- 未来可扩展到远程同步 / 导入导出时需要预留的抽象层。

## 目标

### 本轮设计必须达成

- 同一父节点下，`Folder` 与 `SSH Connection` 统一执行唯一命名约束；
- `New Folder` 默认命名规则固定为 `Folder 1`、`Folder 1-1`、`Folder 1-2`；
- `New SSH Connection` 默认命名规则固定为 `SSH Connection 1`、`SSH Connection 1-1`、`SSH Connection 1-2`；
- 手动输入发生冲突时，使用轻量 inline 提示，且禁止确认提交；
- 资产持久化必须覆盖 SSH 连接字段，而不只是树节点标题；
- `expanded / search / selection / context-menu` 等 UI 会话态不进入持久化状态；
- 数据目录策略采用 logging-style root abstraction，并在 README 中说明；
- 当前首选持久化介质为 `redb`；
- 目录策略必须明确回答“相对 executable dir 还是 working dir”；
- 首次启动初始化、损坏检测、恢复与数据升级边界必须明确。

### 体验目标

- Explorer 交互语义统一，不出现“同样是命名，有时自动改名、有时直接失败”的割裂；
- 错误提示靠近输入点，不引入重型打断式 toast；
- 本地存储路径对便携模式和跨平台安装模式都可解释；
- 为后续 SSH/SFTP 真正接入、远程同步、导入导出保留清晰边界。

## 非目标 / 边界

本轮不覆盖以下内容：

- terminal runtime、renderer、`wezterm-term`、`termwiz`、`russh`、`russh-sftp` 运行时细节；
- 远程同步协议本身；
- 拖拽排序、多选、剪贴板粘贴真实业务；
- undo / recycle bin / soft delete；
- 已确认方案之外的业务代码实现；
- README 实际改写与实现代码提交。

## 当前实现现状

### 1. 资产树真相源

`AssetTree` 目前持有：

- `id`
- `kind`
- `title`
- `parent_id`
- `children`
- `expanded`

对应代码见 [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs#L122)。

当前可见行投影、同父级命名校验、删除子树能力已经存在，见：

- [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs#L275)
- [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs#L311)
- [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs#L339)

### 2. 交互状态与 modal 桥接

`ShellViewModel` 已经集中维护：

- `asset_modal_state`
- `selected_asset_ids`
- `focused_asset_id`
- `context_target_asset_id`
- `asset_search_query`
- `asset_tree_fully_expanded`

对应代码见 [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L89)。

当前 `Rename` 已经具备实时校验和 confirm gating：

- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L159)
- [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs#L251)
- [ui/components/assets-rename-modal.slint](/home/wwwroot/mica-term/ui/components/assets-rename-modal.slint#L3)

### 3. 当前 create 链路的缺口

`New Folder` 与 `New SSH Connection` 当前只校验：

- folder: 非空
- ssh: `name` 非空且 `host` 非空

对应逻辑见 [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L441)。

真正提交时仍会调用 `resolve_committed_name()` 自动补名：

- [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs#L502)
- [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs#L561)

这意味着当前 create 链路仍保留“提交时静默改名”的旧语义。

### 4. 当前持久化基础设施

仓库里当前只有两类持久化：

- logging root 解析与目录初始化：
  - [src/app/logging/paths.rs](/home/wwwroot/mica-term/src/app/logging/paths.rs#L32)
- UI preferences JSON：
  - [src/app/ui_preferences.rs](/home/wwwroot/mica-term/src/app/ui_preferences.rs#L37)

logging root 当前策略是：

1. `env override`
2. executable dir 下存在 `.mica-term-portable` 时走 portable root
3. 否则走 `ProjectDirs` 对应的平台本地数据目录

并且不是相对 working directory，见 [src/app/logging/paths.rs](/home/wwwroot/mica-term/src/app/logging/paths.rs#L64) 和 [tests/logging_paths.rs](/home/wwwroot/mica-term/tests/logging_paths.rs#L7)。

### 5. 当前与本轮目标的主要差距

- `AssetTree` 还不是完整的 persisted domain model；
- SSH 连接字段还没有进入可持久化领域对象；
- create modal 还没有接入统一的重名校验与 inline error；
- 没有资产数据 root resolver；
- 没有 repository / store 抽象；
- 没有损坏检测、升级策略、回滚策略。

## 设计要点拆分

### 设计点 1：同父级唯一命名与自动命名规则

#### 方案 A：默认名只用于预填，手动冲突禁止提交

- modal 打开时预填一个已唯一化的默认名称；
- 用户手动改成冲突值时，立即显示 inline error；
- confirm disabled；
- 提交阶段不再静默把用户输入改成另一个值。

优点：

- 与 Explorer 心智一致；
- 用户输入值与最终保存值一致；
- `Create` 和 `Rename` 语义统一。

缺点：

- 需要把 create modal 也接入校验链路。

#### 方案 B：保留提交时自动补名

- 输入阶段允许冲突；
- 点击确认后自动改成下一个可用名。

优点：

- 实现成本较低；
- 兼容当前 `resolve_committed_name()` 逻辑。

缺点：

- 用户输入与最终保存值可能不一致；
- 不符合本轮“输入阶段实时校验”的已确认方向。

#### 最终决策

选择方案 A。

最终命名规则：

- `Folder`: `Folder 1` -> `Folder 1-1` -> `Folder 1-2`
- `SSH Connection`: `SSH Connection 1` -> `SSH Connection 1-1` -> `SSH Connection 1-2`

补充约束：

- 同父级跨类型也不允许重名；
- 默认名生成只服务于“modal 初始预填”；
- 空字符串 fallback 仍可使用默认命名策略，但不能覆盖用户明确输入的冲突值。

### 设计点 2：持久化状态与 UI 状态分层

#### 方案 A：继续让 `ShellViewModel` 承担持久化真相源

优点：

- 改动表面上较小。

缺点：

- `ShellViewModel` 混合 UI 会话态、modal 态、持久化业务态；
- 后续导入导出、远程同步难以扩展；
- 很难明确哪些字段应该落盘。

#### 方案 B：拆出独立的 persisted domain model

建议分层：

- `PersistedAssetCatalog`
- `PersistedAssetNode`
- `PersistedSshConnectionSpec`
- `AssetExplorerUiState` 仅保留会话态

优点：

- 领域状态和 UI 状态边界清晰；
- 易于做版本迁移与持久化测试；
- 为导入导出 / 远程同步预留稳定接口。

缺点：

- 需要在实施阶段重构一层领域对象映射。

#### 最终决策

选择方案 B。

明确不持久化的字段：

- `expanded`
- `asset_search_query`
- `asset_search_expanded`
- `selected_asset_ids`
- `focused_asset_id`
- context menu / modal 开关态

明确要持久化的字段：

- 树结构与顺序
- folder / ssh 节点类型
- SSH 连接字段
- 后续为导入导出准备的 schema version

### 设计点 3：数据目录策略

#### 方案 A：强制 relative to executable dir

优点：

- 最符合“portable app”直觉；
- 肉眼可见、便于打包后随应用移动。

缺点：

- 对 Windows `Program Files`、macOS app bundle、Linux system package 写权限不稳定；
- 不利于跨平台正式分发。

#### 方案 B：logging-style root abstraction

策略与 logging 根目录一致：

1. 可选 override root
2. portable marker 存在时，使用 executable dir 对应 root
3. 否则使用平台本地数据目录

优点：

- 与现有 logging 行为一致；
- 兼顾 portable 与正式安装形态；
- 跨平台迁移成本最低。

缺点：

- 需要 README 明确说明，不然用户不易直觉理解。

#### 最终决策

选择方案 B。

明确回答：

- 资产数据目录不是相对 working directory；
- 只有在 portable root 生效时才相对 executable dir；
- 否则相对平台本地数据目录；
- Windows / macOS / Linux 后续迁移必须继续走 root abstraction，不能把路径拼接散落到 UI 或业务逻辑里。

### 设计点 4：持久化介质

#### 方案 A：`redb`

特点：

- 单文件；
- ACID；
- crash-safe；
- 纯 Rust；
- 适合本地嵌入式 catalog；
- 对当前规模的资产树与 SSH 元数据足够。

优点：

- 不违反“禁止 JSON / SQLite”；
- 单文件易于备份和迁移；
- 不需要自研磁盘格式；
- 与项目“优先复用标准生态”的规范一致。

缺点：

- 可读性不如自定义文本/快照；
- 需要额外设计 table/key 布局。

#### 方案 B：自定义二进制 snapshot + WAL

优点：

- 对树结构拟合度极高；
- 文件布局、恢复时机、回滚语义完全可控。

缺点：

- 自研维护成本高；
- 与项目现有规范冲突更大；
- 损坏恢复与升级链路都要自己扛。

#### 方案 C：`fjall`

优点：

- 多 keyspace；
- 适合更大规模的 LSM 场景；
- 未来 blob / 扩展字段能力更强。

缺点：

- 对当前资产 catalog 偏重；
- 默认 durability 语义需要额外显式管理；
- 多文件目录式布局不如单文件简洁。

#### 最终决策

选择方案 A：`redb`。

补充说明：

- `redb` 是当前首选落地方案；
- 方案 B 与方案 C 不作为本轮主方案；
- 其中“异步 command queue 写入”属于写入架构问题，不等同于存储介质选择，单独记录在待办。

### 设计点 5：写入接口与扩展点

#### 方案 A：domain service + repository/store abstraction

- UI 不直接操作磁盘；
- `ShellViewModel` 只和 application service 交互；
- persistence 通过 `AssetCatalogRepository` 或等价 store abstraction 隔离；
- 导入导出、远程同步将来继续走 service 层。

优点：

- 与当前单向真相源模型一致；
- 易于替换实现；
- 本地 store、导入导出、未来远程同步都能共用 catalog domain。

缺点：

- 需要额外定义 domain / repository 边界。

#### 方案 B：UI 直接持有 DB handle

优点：

- 表面步骤更少。

缺点：

- UI 和存储耦合；
- 难以测试；
- 扩展性差。

#### 最终决策

选择方案 A。

异步 Tokio command queue 不是当前确认基线，但作为未来可选演进方案记录到 `20260323-todo.md`。

## 方案对比汇总

| 设计点 | 备选方案 | 已确认结论 |
| --- | --- | --- |
| 命名冲突语义 | 实时阻止提交 / 提交时静默补名 | 实时阻止提交 |
| 持久化状态边界 | ViewModel 混存 / Domain 与 UI 分层 | Domain 与 UI 分层 |
| 目录策略 | 强制 executable dir / logging-style abstraction | logging-style abstraction |
| 存储介质 | `redb` / custom binary + WAL / `fjall` | `redb` |
| 写入边界 | repository abstraction / UI 直连 store | repository abstraction |

## 最终决策

### 1. 命名行为

- `Folder` 默认使用 `Folder 1` 起步，冲突时使用 `-N` 后缀；
- `SSH Connection` 默认使用 `SSH Connection 1` 起步，冲突时使用 `-N` 后缀；
- create / rename 全部使用统一的同父级跨类型唯一校验；
- 用户手动输入冲突名时，提示 inline error，禁止 confirm。

### 2. 持久化边界

- 建立独立的 persisted catalog domain；
- 必须覆盖 SSH 连接字段；
- UI 会话态不落盘；
- `AssetTree` 不应继续直接作为最终 persisted schema。

### 3. 存储目录约定

- 资产数据目录不相对 working directory；
- 采用 logging-style root abstraction；
- root 解析应被抽象为统一的 app data root resolver；
- 资产数据子目录建议为 `<root>/data/`；
- 资产主文件建议为 `<root>/data/assets.redb`。

### 4. 文件命名策略

- 主文件：`assets.redb`
- 升级前备份：`assets.backup-<timestamp>.redb`
- 损坏隔离：`assets.corrupt-<timestamp>.redb`

### 5. 文件格式 / 存储抽象

- 底层介质：`redb`
- 上层抽象：`AssetCatalogRepository`
- persisted schema 应内置 `schema_version`
- 需要有 metadata table 与 asset records table

### 6. 写入时机

- 启动时加载 catalog；
- create / rename / delete / edit SSH connection 成功后写入；
- 纯 UI 会话行为不写入；
- 后续若引入导入导出，同样通过 repository/service 层统一落盘。

### 7. 恢复策略

- 首次启动若主文件不存在，则初始化空 catalog；
- 打开 store 前先确保 root 与 `data` 目录存在；
- schema 版本不匹配时进入升级流程；
- 升级前必须先做 timestamped backup；
- 若打开或升级失败，保留原文件并写日志，不做静默覆盖。

### 8. 数据升级 / 兼容策略

- persisted metadata 中记录 `schema_version`；
- 仅允许前向迁移；
- 每次破坏性 schema 变更前先导出备份；
- 回滚以“回退到备份文件”为主，不依赖在线双写。

### 9. 损坏检测与回滚策略

- 依赖 `redb` 打开 / 事务失败结果作为底层损坏检测入口；
- 一旦检测到无法恢复的打开错误：
  - 原文件隔离到 `assets.corrupt-<timestamp>.redb`
  - 日志记录到 logging root
  - 不静默重建覆盖原文件
- 真正的自动重建策略应放到后续实现阶段单独决定。

## 实施步骤

1. 先定义独立的 persisted catalog domain 与 schema version；
2. 抽出 app data root resolver，把 logging-style root abstraction 提升为可复用基础设施；
3. 为 create / rename modal 建立统一的 `AssetNameValidation` 投影接口；
4. 将 create modal 从“提交时补名”改为“预填默认名 + 输入期校验”；
5. 引入 `AssetCatalogRepository`，以 `redb` 作为首个实现；
6. 为 startup load / create / rename / delete / edit SSH connection 接入持久化链路；
7. 为目录初始化、损坏检测、升级备份、README 说明补齐测试与文档。

## 风险与回滚策略

### 风险 1：当前 domain model 不含 SSH 连接完整字段

影响：

- 若直接把现有 `AssetTree` 写入磁盘，会得到不完整的 persisted model。

策略：

- 先拆 persisted catalog domain，再接 store；
- 不允许把当前 `AssetTree` 直接视作最终持久化 schema。

### 风险 2：logging-style root abstraction 容易被误解为 working directory

影响：

- 用户可能以为数据总在 exe 同目录。

策略：

- README 明确写清：
  - working directory 不参与路径解析；
  - portable 模式下才相对 executable dir；
  - 其他情况走平台本地数据目录。

### 风险 3：create 链路从自动补名改为实时阻止提交会改变现有测试预期

影响：

- 当前有部分测试锁定了“提交后自动补名”语义。

策略：

- 实施阶段先更新 TDD / smoke expectations；
- 再统一替换 create modal 行为。

### 风险 4：后续若改成异步 command queue，会改变 ack / flush / crash semantics

影响：

- repository 接口与 UI 交互会变复杂。

策略：

- 当前基线仍按 repository abstraction 设计；
- 异步 queue 作为后续演进项单独评估，不提前绑定本轮 store API。

## 验证清单

- [ ] `Create` 与 `Rename` 对同父级跨类型重名都能实时显示 inline error
- [ ] `Folder` 默认命名遵循 `Folder 1 / Folder 1-1 / Folder 1-2`
- [ ] `SSH Connection` 默认命名遵循 `SSH Connection 1 / SSH Connection 1-1 / SSH Connection 1-2`
- [ ] working directory 变化不影响资产数据 root
- [ ] portable marker 生效时，资产数据 root 与 executable dir 对齐
- [ ] 非 portable 模式下，资产数据 root 走平台本地数据目录
- [ ] 首次启动自动初始化目录与空 catalog
- [ ] create / rename / delete / edit SSH connection 能正确落盘并重启恢复
- [ ] `expanded / search / selection` 不进入 persisted state
- [ ] schema upgrade 前会先生成备份文件
- [ ] store 打开失败时不会静默覆盖原文件
- [ ] README 写明资产目录策略与 portable 行为

## 参考现状

- 代码现状：
  - [src/shell/assets.rs](/home/wwwroot/mica-term/src/shell/assets.rs)
  - [src/shell/view_model.rs](/home/wwwroot/mica-term/src/shell/view_model.rs)
  - [src/app/bootstrap.rs](/home/wwwroot/mica-term/src/app/bootstrap.rs)
  - [src/app/logging/paths.rs](/home/wwwroot/mica-term/src/app/logging/paths.rs)
- 历史设计：
  - [2026-03-20-windows-console-assets-explorer-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-20-windows-console-assets-explorer-design.md)
  - [2026-03-23-windows-console-assets-explorer-tdd-spec.md](/home/wwwroot/mica-term/docs/plans/2026-03-23-windows-console-assets-explorer-tdd-spec.md)
