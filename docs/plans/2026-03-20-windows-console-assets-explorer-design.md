# Windows Console 资产列表 Explorer 行为优化 Design

日期: 2026-03-20
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

当前 `Windows Console` 资产区已经具备以下基础壳层能力：

- `AssetTree + visible row projection` 的 Rust 真源；
- `AppWindow -> Sidebar -> AssetsSidebar -> AssetNodeRow` 的 Slint 视图链路；
- `blank-area / item context menu`、`create modal`、基础 `rename session` 的桥接；
- `winit + femtovg-wgpu` 的当前主线桌面 shell。

最近直接相关提交如下：

- `07692e3 2026-03-17 feat: add windows console assets context menu`
- `24a2aa8 2026-03-17 fix: complete console assets context menu bugfix`
- `a9becfc 2026-03-18 feat: finalize windows console assets context menu bugfix2`
- `7cd28c4 2026-03-18 feat: implement console assets explorer bugfix3`
- `0f22556 2026-03-19 feat: finalize assets explorer modal bugfix4`
- `77b1f51 2026-03-20 feat: 完成 windows console assets 样式优化`

本轮任务不是 terminal runtime、`wezterm-term`、`termwiz`、`russh`、`russh-sftp` 或持久化层重构，而是在现有 `Assets Explorer shell` 上继续收敛以下交互：

1. 文件夹展开 / 收缩图标状态与真实树状态一致；
2. 同父节点下严格禁止重名，并在输入阶段实时校验；
3. `Rename` 与 `Delete` 从菜单 IA 升级为真实可用动作；
4. `Create / Rename / Delete` 在交互语义上保持统一、克制、桌面端风格一致。

## 调研结论

### 1. 当前主工作区仍是 shell 阶段

- `src/main.rs` 已锁定 `winit + femtovg-wgpu` 主线；
- `ui/app-window.slint` 当前主内容仍挂 `WelcomeView`；
- 因此本轮必须严格收敛在 `Windows Console Assets Explorer`，不扩散到底层 terminal widget 或协议栈。

### 2. 资产树的真实状态已经在 Rust 侧

当前展开态、层级关系、可见行投影都来自 `AssetTree`：

- `src/shell/assets.rs`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`

因此本轮应继续坚持“Rust 是真相源，Slint 只渲染投影”的方向，避免把展开状态、校验状态再次拆到 UI 本地。

### 3. 当前 disclosure 图标语义不正确

`AssetNodeRow` 现在无论展开还是收缩都使用同一个向下图标，仅通过透明度变化表达状态。这不符合 Explorer 心智，也容易造成“看起来没有切换”的误解。

### 4. 当前重名处理发生得太晚

目前命名冲突主要在提交时由 `resolve_committed_name()` 自动补号解决，缺少：

- 输入阶段的实时冲突判断；
- 输入框附近的直接反馈；
- `Create` 与 `Rename` 共用的统一校验语义。

### 5. 当前 `Rename` / `Delete` 还没有真正接完

`rename-asset`、`delete-asset` 已经出现在 context menu action tree 中，但 `handle_context_menu_leaf_action()` 目前真实接线仍以 create 为主，删除也缺少树级删除 API。

### 6. 当前产品方向已经从“行内新建”转为“modal workflow”

现有 `New Folder`、`New SSH Connection` 已经使用 modal。若 `Rename` 继续保持 inline rename，而 `Create` 使用 modal，会形成明显的交互语义分裂。用户已明确确认：本轮以统一性优先，`Rename` 也应进入 modal 体系。

## 目标

### 本轮必须达成

- `Tree` 模式下，folder disclosure 图标在 `collapsed / expanded / none` 三种状态下表现清晰且与真实树状态同步；
- 同一个父节点下，folder 与 SSH asset 统一执行唯一命名约束；
- `Create` 与 `Rename` 均使用 modal workflow；
- 输入阶段实时显示命名冲突，并禁止确认提交；
- `Delete` 支持 folder / SSH 节点，非空 folder 采用 destructive confirm modal 后递归删除；
- 删除后的 selection / focus 回落规则明确且稳定；
- 整体交互继续贴近 `VS Code Explorer`，但保持当前项目的 Fluent + 方角风格。

### 体验目标

- 让 Explorer 交互规则更统一，减少“有的动作在行内改，有的动作弹窗改”的割裂感；
- 让错误提示尽量靠近输入点，避免重型 toast 打断；
- 让 destructive action 足够明确，但不引入多余复杂度；
- 为后续真实 SSH/SFTP 接入保留清晰的节点行为契约。

## 边界

### 本轮覆盖

- `AssetTree` 的可见行投影字段补充；
- `Create / Rename / Delete` 的状态机与 modal 策略；
- `AssetsSidebar` / `AssetNodeRow` 的 disclosure 渲染契约；
- 命名唯一性校验、自动命名与错误提示；
- context menu 到业务动作的真实桥接；
- 删除后的 focus / selection 恢复规则；
- 单元测试、smoke test、UI contract 对上述行为的验证。

### 本轮不覆盖

- terminal emulator、renderer、SSH runtime、SFTP runtime；
- 持久化 schema、数据库迁移；
- 拖拽排序、多选、框选、剪贴板粘贴真实业务；
- undo / recycle bin / soft delete；
- 移动端交互适配。

## 方案对比与最终决策

### 设计点 1：展开 / 收缩图标状态

#### 方案 `1A`

由 Rust 直接投影 `disclosure_state = none | collapsed | expanded`，Slint 仅按状态渲染图标。

优点：

- 单一真相源，最不容易出现图标与树状态不同步；
- 后续若增加 loading / disabled / lazy children 等状态，扩展成本更低；
- 便于测试直接锁定投影结果。

缺点：

- 需要调整 visible row projection 结构。

#### 方案 `1B`

继续沿用 `show_disclosure + expanded` 两个布尔值，由 Slint 本地决定显示哪个图标。

优点：

- 改动较小。

缺点：

- UI 层承担更多语义判断；
- 后续状态复杂度上升时容易继续膨胀。

**最终选择：`1A`**

补充决策：

- `collapsed` 显示右向箭头；
- `expanded` 显示下向箭头；
- `none` 不绘制箭头，但保留固定对齐占位，保证整列节奏稳定。

### 设计点 2：同父级唯一命名范围

#### 方案 `2A`

同一个父节点下统一唯一，不区分 `folder` 与 `ssh` 类型。

优点：

- 最符合 Explorer / 文件系统心智；
- 搜索、右键菜单、删除确认中的目标识别更清晰；
- 避免“同名不同类型”带来的列表歧义。

缺点：

- 与当前“按类型分别补号”的实现方向不同，需要调整校验规则。

#### 方案 `2B`

只限制同类型唯一，folder 与 SSH 可以同名。

优点：

- 更贴近当前已有实现。

缺点：

- 用户心智复杂；
- 同名项在 UI 上更难区分。

#### 方案 `2C`

同类型强制唯一，跨类型允许但提示 warning。

优点：

- 是一个折中方案。

缺点：

- 规则不够干净；
- 既没完全统一，也没有真正降低认知成本。

**最终选择：`2A`**

### 设计点 3：自动命名规则

#### 方案 `3A`

新建时直接预填一个可提交的唯一默认名；若冲突，则继续按后缀补号。

初版英文规则：

- folder: `Folder 1` -> `Folder 1-1` -> `Folder 1-2`
- ssh: `SSH Connection 1` -> `SSH Connection 1-1` -> `SSH Connection 1-2`

优点：

- 减少首次创建操作成本；
- 规则稳定、可预测；
- 与实时校验不冲突。

缺点：

- 需要明确定义默认前缀与补号算法。

#### 方案 `3B`

输入框默认留空，仅提供推荐名 placeholder。

优点：

- 显式度高。

缺点：

- 比成熟桌面端 Explorer 多一步。

#### 方案 `3C`

允许用户输入冲突名，提交时静默改写为下一个可用名。

优点：

- 实现较简单。

缺点：

- 用户输入与最终保存值不一致，违背直觉；
- 与实时校验目标冲突。

**最终选择：`3A`**

### 设计点 4：实时冲突提示方式

#### 方案 `4A`

输入框下方显示 inline error，同时输入框红边，`Confirm / Save` disabled。

优点：

- 最靠近问题发生位置；
- 不打断输入流程；
- 同一套模式可同时复用于 `Create` 与 `Rename`。

缺点：

- modal 组件需要补充错误态样式与文案位置。

#### 方案 `4B`

只使用 toast / `StatusPill` 做冲突提示。

优点：

- 复用现有反馈通道。

缺点：

- 离输入点太远；
- 对表单校验不够直接。

#### 方案 `4C`

输入框旁浮动 tooltip 式错误提示。

优点：

- 提示靠近输入框。

缺点：

- 焦点与视觉抖动更难控制；
- 比 inline error 更重。

**最终选择：`4A`**

### 设计点 5：`Rename` 交互语义

#### 方案 `5A`

保留 inline rename。

优点：

- 更接近传统 Explorer 的原地改名。

缺点：

- 与本项目已确认的 `Create modal` 语义不统一；
- 需要继续维护复杂的焦点、失焦、blank-area click、context menu dismiss 优先级。

#### 方案 `5B1`

folder 与 SSH 一律使用同一个小型单字段 `Rename Modal`。

优点：

- 与 `New Folder` / `New SSH Connection` 的 modal 心智保持一致；
- 实时校验、禁用确认、错误提示都更容易统一；
- 比类型分裂的 rename modal 更一致。

缺点：

- 少了一点原地编辑的 Explorer 味道。

#### 方案 `5B2`

folder 用轻量 rename modal，SSH 用更大的编辑弹框。

优点：

- 方便未来把 SSH rename 扩成 edit connection。

缺点：

- 同样是 `Rename`，却出现两套不同弹框语义；
- 与当前“统一性优先”的目标冲突。

**最终选择：`5B1`**

### 设计点 6：删除逻辑

#### 方案 `6A`

非空 folder 允许删除，但必须进入 destructive confirm modal，确认后递归删除全部后代节点。

优点：

- 符合多数 Explorer / 文件管理器心智；
- 操作成本适中；
- 可在确认框中直接表达风险范围。

缺点：

- 需要补删除 API、后代统计与 focus 恢复规则。

#### 方案 `6B`

非空 folder 不允许直接删除，必须手动清空后再删。

优点：

- 最保守。

缺点：

- 操作负担明显偏高；
- 不符合常见桌面端心智。

#### 方案 `6C`

递归删除后提供 undo。

优点：

- 用户体验最好。

缺点：

- 明显超出本轮范围；
- 引入额外的状态恢复与持久化复杂度。

**最终选择：`6A`**

补充决策：

- destructive confirm modal 文案必须明确包含目标名；
- folder 还需明确提示“this will also remove N nested items”；
- 第一版不做 undo。

## 最终决策摘要

1. disclosure 状态由 Rust 真源直接投影，UI 只做渲染；
2. 同父节点下执行统一唯一命名，不区分节点类型；
3. 自动命名初版为英文规则，采用 `Base N` 与 `Base N-M` 形式；
4. 冲突提示采用 inline error，禁止静默改名；
5. `Rename` 统一改为小型单字段 `Rename Modal`；
6. `Delete` 对非空 folder 采用 destructive confirm + recursive delete；
7. 所有交互继续坚持 `Rust state -> bootstrap bridge -> Slint rendering` 路线。

## 详细交互定义

### 1. Disclosure 行为

- folder 且有子项：
  - collapsed: 右向箭头
  - expanded: 下向箭头
- folder 且无子项：
  - 不显示箭头图形
  - 保留固定占位，避免 label 左右跳动
- ssh：
  - 永远无 disclosure 图标
- 图标状态只能从 Rust 投影读取，禁止 Slint 额外推断树状态

### 2. `Create` 行为

- `New Folder` 打开 folder create modal
- `New SSH Connection` 打开 SSH create modal
- modal 打开时输入框预填一个唯一默认名
- 用户编辑时实时校验：
  - 空字符串：禁止确认
  - 同父级重名：显示 inline error，禁止确认
- 提交成功后：
  - 新节点写入树
  - 若是 child create，则自动展开父 folder
  - 选中新节点并赋焦点

### 3. `Rename` 行为

- 右键菜单点击 `Rename` 后，打开统一的单字段 `Rename Modal`
- modal 标题建议使用中性英文，例如 `Rename Item`
- 输入框初始值为当前名称
- 实时校验规则与 `Create` 完全一致
- `Escape` / `Cancel`：不保存，关闭 modal
- `Enter` / `Save`：仅在校验通过时提交
- 对于“名称未变化”的提交：
  - 允许直接关闭 modal
  - 不应报冲突

### 4. `Delete` 行为

- 右键菜单点击 `Delete` 后：
  - SSH 节点：弹确认框
  - 空 folder：弹确认框
  - 非空 folder：弹更明确的 destructive confirm modal
- folder 删除确认框至少包含：
  - 节点名称
  - 后代数量
  - destructive 文案
- 删除成功后的焦点规则：
  - 优先下一个同级
  - 否则上一个同级
  - 否则父级
  - 根级删空后回 blank area

## 实施步骤

1. 在 Rust 侧扩展 Explorer 行投影与命名校验辅助结构
2. 为 `Create` 与 `Rename` 建立统一的 name validation 规则
3. 补齐 context menu 到 `rename-asset` / `delete-asset` 的真实业务分发
4. 为 `AssetTree` 增加节点删除与后代统计能力
5. 在 `AppWindow` root overlay 增加统一 `Rename Modal` 与 `Delete Confirm Modal`
6. 调整 `AssetNodeRow` 的 disclosure 图标渲染契约
7. 为 create / rename / delete / focus fallback 补齐测试

## 风险与回滚

### 风险 1：命名规则从“按类型避重名”切到“统一父级唯一”会影响既有测试

- 风险：现有测试部分建立在同类型避重名逻辑上，修改后会有一批断言需要同步更新。
- 控制：
  - 先锁定新的唯一性契约；
  - 再统一替换 create / rename 两条链路的测试。

### 风险 2：`Rename Modal` 会让现有 inline rename 相关桥接显得冗余

- 风险：若只补 modal，不清理旧桥接，会留下双路径状态机。
- 控制：
  - 实现阶段应明确收敛到单一路径；
  - 保留必要兼容层，但不要保留双入口。

### 风险 3：递归删除第一版若同时引入 undo，会超出任务边界

- 风险：undo 会把任务扩展到恢复快照、通知、持久化一致性。
- 控制：
  - 第一版只做 destructive confirm；
  - 暂不引入 undo。

### 风险 4：后代数量统计若与未来懒加载树冲突

- 风险：未来若变成远端懒加载树，确认框里的精确后代数会变复杂。
- 控制：
  - 第一版在本地内存树中直接统计；
  - 后续若切懒加载，可降级为“contains nested items”。

### 回滚策略

若实现阶段发现复杂度明显超出预期，允许按以下顺序回滚，但不得回退到“提交时静默改名”：

1. `Delete Confirm Modal` 先保留固定文案，后代数量可暂时降级；
2. `Rename Modal` 先只覆盖 folder 与 SSH 的单字段改名，不扩展更多字段；
3. disclosure 先完成状态正确性，再做更细的视觉 polish。

## 验证清单

- [ ] `Tree` 模式下 folder disclosure 在 `collapsed / expanded / none` 三种状态下表现正确
- [ ] 点击 disclosure 后，图标状态与真实展开状态始终一致
- [ ] 同父节点下，不允许 folder 与 SSH asset 出现同名
- [ ] `New Folder` 初版默认名按 `Folder 1`、`Folder 1-1` 规则生成
- [ ] `New SSH Connection` 初版默认名按英文规则生成
- [ ] `Create` 输入阶段出现重名时，输入框附近显示 inline error，且确认按钮禁用
- [ ] `Rename` 使用统一的小型单字段 modal，而非 inline rename
- [ ] `Rename` 对“名称未变化”不会错误提示冲突
- [ ] `Delete` 点击后总会进入确认流程
- [ ] 非空 folder 删除确认框明确提示递归删除影响范围
- [ ] 删除后 selection / focus 按既定回落规则恢复
- [ ] 单元测试覆盖命名规则、删除规则、focus fallback
- [ ] smoke / UI contract 覆盖 context menu -> modal -> tree projection 的桥接链路
