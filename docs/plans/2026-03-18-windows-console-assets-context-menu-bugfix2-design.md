# Windows Console 资产列表右键菜单 Bugfix2 Design

日期: 2026-03-18
执行者: Codex
状态: 方案已确认，待按需进入 implementation plan

## 背景

本轮任务聚焦 `Windows Console` 资产区当前这一批交互与视觉问题：

1. 空白区域右键菜单顶部下方存在异常大留白；
2. 空白区域与资产项右键菜单仍使用中文顶部标题，且菜单项缺少 icon；
3. 新建文件夹 / SSH 连接进入 rename 后，点击其他区域无法正常退出；
4. 文件夹与 SSH 连接命名缺少唯一性约束与自动补号策略；
5. 顶部工具栏的 `Create`、tooltip、次级文案与当前 panel 状态不一致；
6. `Console Tree` / `Hosts, recent sessions, favorites` 这一类占位文案仍残留在 UI 中；
7. 资产项右键菜单的外层包裹与菜单表面层级不完整，视觉上像被拆成了两段。
8. 资产顶部搜索在“已展开但无输入”的状态下，点击资产列表区域或左侧活动栏不会自动关闭，只有主工作区等部分区域能关闭。

本轮不是 terminal runtime、renderer、SSH/SFTP 生命周期或持久化重构，而是一次围绕 `AssetsSidebar` 壳层交互的定向收敛。

## 调研结论

### 当前实现链路

- 当前菜单体系不是 Slint 原生 `ContextMenuArea/Menu`，而是现有项目自绘的 `Slint overlay + Rust state machine`：
  - `src/shell/context_menu.rs`
  - `src/shell/view_model.rs`
  - `src/app/bootstrap.rs`
  - `ui/components/assets-context-menu-overlay.slint`
- `asset-context-menu-requested(...)`、rename 回调、toolbar 回调都已经通过 `bootstrap.rs` 接到 `ShellViewModel`，因此本轮不应切换架构真源。

### 已确认的问题根因

- 菜单 overlay 与菜单列当前都使用固定高度，直接导致空白区域右键菜单出现“大块留白”：
  - `ui/components/assets-context-menu-overlay.slint`
  - `src/shell/context_menu.rs`
- 菜单列标题被硬编码为中文 `操作`，与用户期望的英文风格冲突：
  - `ui/components/assets-context-menu-column.slint`
  - `src/app/bootstrap.rs`
- 菜单项模型当前没有 icon 字段，UI 只能渲染纯文本：
  - `src/shell/context_menu.rs`
  - `ui/components/assets-context-menu-row.slint`
- inline rename 当前只依赖 `TextInput.has-focus` 的隐式提交，没有根层 click-away / `clear-focus()` 协调，因此退出语义不完整：
  - `ui/components/asset-node-row.slint`
  - `ui/app-window.slint`
- 唯一命名策略当前尚未实现；新建占位项仍直接使用固定字符串：
  - `src/shell/assets.rs`
  - `src/shell/view_model.rs`
- 顶部工具栏图标按钮当前没有 tooltip 接口；`Create` 的行为仍是 console 专用静态语义：
  - `ui/components/sidebar-toolbar-icon-button.slint`
  - `ui/components/assets-create-menu.slint`
  - `src/shell/view_model.rs`
- `Console Tree` / `Hosts, recent sessions, favorites` 文案仍被硬编码在列表区域头部：
  - `ui/shell/assets-sidebar.slint`
- 顶部搜索的 click-away 关闭路径当前是碎片化的：
  - `workspace-search-dismiss-layer` 只覆盖主工作区，不覆盖左侧活动栏；
  - `AssetsSidebar` 内部只在 header 与 panel 背景放了 dismiss touch；
  - 资产列表行、空态区域等交互层会抢占点击，因此“点资产列表模块退出搜索”并不稳定。
  - 相关位置：
    - `ui/app-window.slint`
    - `ui/shell/assets-sidebar.slint`
    - `ui/shell/sidebar.slint`

### Git 调研

最近相关变更集中在两次提交，说明当前能力仍处于快速收敛阶段：

- `07692e3 feat: add windows console assets context menu`
- `24a2aa8 fix: complete console assets context menu bugfix`

结论：本轮应继续沿用当前 `AppWindow overlay + ShellViewModel + bootstrap bridge` 路线修正，而不是切到另一套菜单系统。

### 官方文档补充确认

经 Slint 官方文档确认：

- `PopupWindow` 支持 `close-on-click-outside`；
- `TextInput` / focus 系统支持 `focus()` 与 `clear-focus()`；
- `ContextMenuArea` 也可作为备选菜单体系，但平台外观与当前自绘风格控制力较弱。

参考：

- <https://docs.slint.dev/latest/docs/slint/reference/window/popupwindow/>
- <https://docs.slint.dev/latest/docs/slint/guide/development/focus>
- <https://docs.slint.dev/latest/docs/slint/reference/window/contextmenuarea/>

## 目标

- 让 blank-area 菜单与资产项菜单回到单层、紧凑、英文、icon+label 的 Fluent 风格；
- 让 rename 具备完整桌面端退出语义：`Enter` 提交、`Esc` 取消、点击外部提交并退出；
- 为文件夹与 SSH 连接建立稳定的一致性命名规则；
- 让顶部工具栏变成按当前 panel 动态驱动的动作条，而不是 console 固定写死；
- 让顶部搜索在“空查询”状态下具备统一的 click-away 关闭语义：除顶部状态栏外，点击 shell body 任意区域都能收起；
- 删除无意义常驻文案，把状态提示转移到 tooltip；
- 保持 Rust 为状态真源，Slint 为渲染真源，不引入新的业务层或菜单体系。

## 边界

### 本轮覆盖

- blank-area 菜单与 item 菜单视觉壳层
- 菜单 action metadata 模型
- inline rename session 与 click-away 语义
- 唯一命名与自动补号
- 顶部工具栏的动态动作与 tooltip
- 占位次级文案清理
- 顶部搜索在空查询状态下的全局 click-away 收起语义

### 本轮不覆盖

- 真实 SSH 配置表单
- Snippet / Keychain 的完整业务模型
- 真实资产树与持久化 schema
- terminal runtime、renderer、SFTP 接入
- 大规模信息架构扩展

## 设计要点与方案对比

### 设计点 1：菜单容器与顶部标题

#### 方案 `1A`

保留顶部标题条，但统一改成英文；blank-area 使用 `Create`，item 使用 `Actions`，同时把菜单高度改成内容自适应。

优点：

- 过渡成本较低；
- 仍保留“菜单有抬头”的结构感。

缺点：

- 标题条本身仍会拉高菜单视觉重心；
- 即使改英文，也仍比 Windows 11 的常见上下文菜单更重；
- 对“下半部分没有被菜单包裹住”的问题缓解有限。

#### 方案 `1B`

删除顶部标题条，菜单直接显示为 `icon + label` 列表，容器高度按内容自适应；只有未来真的出现复杂 submenu 分组时，才允许显示英文 section title。

优点：

- 同时解决大留白、中文抬头、菜单壳层割裂三个问题；
- 最接近 Windows 11 / Fluent 的轻量上下文菜单视觉；
- 更适合 blank-area 与 item 菜单复用同一套壳层。

缺点：

- 未来如果出现复杂多列菜单，需要再定义 section label 的触发条件。

#### 方案 `1C`

整体切换到 Slint 原生 `PopupWindow` / `ContextMenuArea`。

优点：

- 点击外部关闭、键盘等基础行为更原生；
- 部分交互由框架托管。

缺点：

- 与现有自绘菜单体系不一致；
- 当前项目对视觉壳层可控性要求高，切栈成本大于收益。

**最终选择：`1B`**

### 设计点 2：菜单 action metadata 真源

#### 方案 `2A`

把 `ContextMenuActionNode` 扩展为统一 action metadata 真源，由 Rust 提供：

- `id`
- `english_label`
- `icon_id`
- `state`
- `children`
- `divider_before`

Slint 只消费渲染模型。

优点：

- blank-area、folder、ssh 三套菜单可以共用同一套动作真源；
- tooltip、toolbar create action、未来快捷键提示都可以围绕同一模型扩展；
- 更便于测试与保持英文文案一致。

缺点：

- 需要同步扩展 UI model 和 bridge。

#### 方案 `2B`

Rust 只给 `id/title/state`，图标和最终英文文案在 Slint 里按 `action-id` 二次映射。

优点：

- Rust 改动面较小。

缺点：

- 文案与图标分散在多个文件中，不利于维护；
- 不适合作为未来 tooltip / command palette / keyboard hint 的统一真源。

#### 方案 `2C`

图标由 Slint 决定，英文文案由 Rust 决定。

优点：

- 可局部复用现有 UI。

缺点：

- 责任边界最混乱；
- 后续一定会再次重构。

**最终选择：`2A`**

### 设计点 3：inline rename 的退出语义

#### 方案 `3A`

引入显式 rename session。行为定义：

- `Enter` -> commit
- `Esc` -> cancel
- 点击任意外部区域 -> commit 并退出
- 根层通过显式状态切换与 `clear-focus()` 完成关闭

优点：

- 最符合桌面端文件管理器与资产列表的常见语义；
- 与“点击任何地方都能退出”的预期一致；
- 不需要引入额外弹窗。

缺点：

- 需要在根窗口层增加 rename-aware 的 dismiss / focus 协调。

#### 方案 `3B`

同样使用显式 rename session，但点击外部视为 cancel。

优点：

- 数据更保守；
- 不会意外提交用户未确认的文字。

缺点：

- 与多数桌面端 rename 体验不一致；
- 对“点外部就结束”虽然成立，但用户可能丢失已编辑内容。

#### 方案 `3C`

放弃 inline rename，改成轻量 popover / dialog。

优点：

- 状态边界最清晰；
- 验证与错误提示更容易扩展。

缺点：

- 过重；
- 打断感强；
- 偏离当前任务的交互目标。

**最终选择：`3A`**

### 设计点 4：唯一命名与自动补号

#### 方案 `4A`

按资产类型分别执行“最小缺失正整数”分配：

- `Folder 2`、`Folder 3` 已存在时，新建得到 `Folder 1`
- 之后再次新建得到 `Folder 4`
- `SSH Connection` 采用同样规则

优点：

- 与用户给出的规则完全一致；
- 最像成熟桌面资产管理器；
- 序号分配稳定、可预测。

缺点：

- 需要做一次 label parsing 与同类型冲突扫描。

#### 方案 `4B`

每种类型维护单调递增计数器，只增不补洞。

优点：

- 实现最简单。

缺点：

- 与用户期望不符；
- 删除后留下大量序号空洞。

#### 方案 `4C`

仅在 create 时用固定占位名，等 rename commit 时再做冲突修正。

优点：

- create 逻辑简单。

缺点：

- create 阶段仍会出现重名占位；
- 交互抖动明显。

**最终选择：`4A`**

### 设计点 5：顶部工具栏的动态动作与 tooltip

#### 方案 `5A`

由 Rust 按 `active-sidebar-destination` 生成 toolbar descriptor，统一决定：

- 当前 panel 的 create 主动作
- tooltip 文案
- 按钮是否显示 / 是否置灰
- 搜索、树形、列表相关状态提示

Slint 只负责图标与 hover 呈现。

优点：

- 与现有 `ShellViewModel -> bootstrap -> Slint` 模式完全一致；
- 可以自然满足 `Console / Snippets / Keychain` 的差异化语义；
- 更便于后续测试。

缺点：

- 需要新增一层 descriptor 投影。

#### 方案 `5B`

继续在 `assets-sidebar.slint` 中通过 `if active-panel == ...` 分支硬编码。

优点：

- 首轮写起来快。

缺点：

- 文案、交互与状态很快会分散；
- 一旦 panel 数量继续增加，维护成本会迅速上升。

#### 方案 `5C`

为 `Console`、`Snippets`、`Keychain` 各拆一套完全独立的 toolbar 组件。

优点：

- 面向当前 panel 可高度定制。

缺点：

- 复用性差；
- 对当前体量来说过重。

**最终选择：`5A`**

## 最终决策

### 决策摘要

1. 菜单改为无顶部标题的单层 surface，容器高度按内容自适应；
2. 菜单动作由 Rust action metadata 驱动，所有菜单项统一变为 `icon + english label`；
3. inline rename 引入显式 rename session，点击外部时提交并退出；
4. 文件夹与 SSH 连接的默认命名使用“最小缺失正整数”算法；
5. 顶部工具栏改为 Rust 驱动的动态 action bar，并把常驻次级文案迁移为 tooltip；
6. `Console Tree`、`Hosts, recent sessions, favorites` 等占位文本从常驻 UI 中移除；
7. 资产项菜单与 blank-area 菜单都必须是完整包裹的单个菜单表面，不能再出现下半段视觉脱壳。
8. 顶部搜索的收起逻辑改为统一 shell-level 语义，而不是依赖多个局部 dismiss touch 拼接。

### 具体语义

#### 1. 菜单视觉壳层

- blank-area 菜单与 item 菜单共用同一套 surface 规范；
- 默认不显示顶部标题；
- 菜单高度根据可见 action 行数自动收缩；
- 只有真实存在子菜单列时，才绘制相邻 surface；
- 每一列都必须由完整边框与背景包裹，不允许“上半段有壳、下半段无壳”。

#### 2. 菜单文案与 icon

- 菜单顶部不再出现中文 `操作`；
- 菜单项全部使用英文；
- 菜单项统一使用 `icon + label`；
- action metadata 成为唯一真源，避免 Rust / Slint 双重映射。

建议的 icon 策略：

- 优先复用现有 `assets/icons/fluent`；
- 对现有库中缺失的 `Rename / Copy / Cut / Delete / Refresh / Import / Export` 等图标，补充对应 Fluent SVG 资源；
- 不再接受“部分菜单有 icon、部分没有”的混搭状态。

#### 3. 顶部工具栏语义

- 顶部工具栏下方不再显示 `Console Tree`；
- 顶部工具栏下方不再显示 `Hosts, recent sessions, favorites`；
- 相关状态改通过 tooltip 呈现。

当前阶段的 create 主动作定义为：

- `Console` -> `New SSH Connection`
- `Snippets` -> `New Snippet`
- `Keychain` -> `New Keychain`

也就是说，顶部 `+` 按钮在本轮应被理解为“当前 panel 的 primary create action”，而不是固定弹出 console 专用菜单。

`Folder` 创建能力保留在 `Console` 的 blank-area / item context menu 中。

#### 4. rename session

rename session 是一段显式状态，而不是单纯依赖 `TextInput.has-focus` 的副作用：

1. create 或 `Rename` 动作开始时，记录当前 `renaming_asset_id` 与草稿文本；
2. 输入中持续同步 draft；
3. `Enter` 提交；
4. `Esc` 取消；
5. 点击任意外部区域时，根层结束 rename session，并执行提交；
6. rename 结束后清理焦点与 session 状态。

#### 5. 唯一命名规则

命名规则按资产类型分别独立计算：

- `Folder {n}`
- `SSH Connection {n}`

规则：

- 序号从 `1` 开始；
- 使用最小缺失正整数；
- create 与 rename commit 都必须最终满足唯一性；
- 文件夹只与文件夹比较；
- SSH 连接只与 SSH 连接比较。

#### 6. 空查询搜索的 click-away 语义

顶部搜索在本轮不只需要“能展开”，还需要具备稳定、统一、可预测的关闭规则。

规则定义：

- 仅当 `asset-search-expanded == true` 且 `assets-search-query == ""` 时启用 click-away 自动收起；
- 点击顶部状态栏不自动关闭；
- 点击 shell body 其余任意区域都应关闭，包括：
  - 左侧活动栏 `Activity Bar`
  - 资产列表区域中的已有行
  - 资产区空白区域
  - 主工作区
  - 右侧面板
- 当搜索框已有输入时，任意普通点击都不应自动收起，仍沿用当前“显式清空 / 关闭”语义。

设计结论：

- 空查询搜索的收起逻辑不应继续依赖 `AssetsSidebar` 内部 header/background 的局部 touch area；
- 它应上收为统一的 shell-level dismiss policy，保证点击命中某个子组件时也能先经过一致的“是否收起空搜索”判定；
- search dismiss、rename dismiss、context menu dismiss、create dismiss 之间必须纳入同一套事件优先级设计。

示例：

- 已有 `Folder 1`、`Folder 2` -> 新建得到 `Folder 3`
- 已有 `Folder 2`、`Folder 3` -> 新建得到 `Folder 1`
- 之后再次新建 -> `Folder 4`

## 目标状态

用户进入 `Windows Console` 后应看到：

- 干净的资产列表或空态区域；
- 顶部只有 icon toolbar，不再有多余占位说明；
- blank-area 右键出现紧凑英文菜单；
- SSH / Folder 行右键出现完整包裹的 icon+label 菜单；
- `+` 按钮的 tooltip 与行为随 panel 变化；
- 顶部搜索在未输入时，点击除顶部状态栏外的 shell body 任意区域都会自动收起；
- 新建后立即进入 rename；
- 点击任意外部区域可结束 rename；
- 自动命名永远不会产生同类型重名。

## 实施步骤

> 本节只定义 design 级实施顺序，不展开到 implementation-plan 颗粒度。

1. 扩展 context menu action metadata，增加英文文案与 icon 标识；
2. 改造菜单 surface，使其取消固定标题与固定高度；
3. 统一 blank-area / item 菜单的单层包裹视觉；
4. 为菜单项渲染 `icon + label + state`；
5. 为 toolbar 建立 panel-aware descriptor；
6. 清理 `Console Tree` / `Hosts, recent sessions, favorites` 常驻文本；
7. 将顶部 `Create` 改为当前 panel 的 primary create action；
8. 统一空查询搜索的 shell-level dismiss 路径，覆盖活动栏、资产区、主工作区与右侧面板；
9. 引入显式 rename session 与 click-away commit；
10. 为 create / rename 共用唯一命名分配器；
11. 补充 UI contract、view model、bootstrap smoke 验证。

## 风险与回滚

### 风险 1：菜单从固定高度改为内容自适应后，边缘定位与多列对齐可能抖动

- 缓解：继续保留 Rust 侧 `placement` 为唯一真源，只替换列高度与 surface 布局计算；
- 回滚：若多列高度联动在本轮不稳定，可先保持“每列独立内容高度 + 统一顶部对齐”，不回退到固定 320px。

### 风险 2：rename click-away 与菜单 dismiss layer 发生命中冲突

- 缓解：根窗口层明确区分 rename dismiss、create dismiss、context menu dismiss 的优先级；
- 回滚：若全局 click-away 在首轮不稳定，可先限制为“点击资产区外部提交”，但不回退到完全不可退出。

### 风险 3：唯一命名在手动 rename 场景下引发意外改名

- 缓解：在设计上先要求“最终必须唯一”，具体是自动补号还是保持编辑态，可在 implementation plan 中进一步细化；
- 回滚：若 UI 反馈来不及补齐，首轮至少保证 create 路径 100% 唯一，rename 路径保留显式冲突处理钩子。

### 风险 4：toolbar 语义动态化后，Snippets / Keychain 仍缺少完整业务模型

- 缓解：本轮只先完成 descriptor 与 tooltip 语义，placeholder create 仍允许停留在壳层；
- 回滚：若某 panel 当前无 create placeholder 能力，可先显示 tooltip 并置灰，而不是回退成 console 固定逻辑。

### 风险 5：菜单列宽真源失配导致 placement 错位

- 风险：菜单改成 `icon + label` 后，Slint 列宽、内边距、icon 槽位会变化；如果 Rust 侧 `CONTEXT_MENU_COLUMN_WIDTH` 仍是旧值，会出现锚点偏移、边缘裁切和子菜单碰撞。
- 缓解：implementation plan 中必须把“菜单视觉宽度”和“Rust placement 宽度”作为一个原子改动处理，禁止只改单边。
- 回滚：如果首轮无法完全统一真源，可先保留单列菜单固定宽度，但不允许继续保留固定高度。

### 风险 6：rename、search、context menu、create overlay 的 dismiss 优先级冲突

- 风险：新增 rename click-away 与 search click-away 后，当前根窗口上的多个 dismiss layer 可能在一次点击中重复响应，造成提交顺序错误或状态重复关闭。
- 缓解：文档要求明确统一优先级，推荐顺序为 `rename dismiss > context menu dismiss > create dismiss > empty-search dismiss`。
- 回滚：若首轮难以做到完全统一，可先保证“一次点击只消费一个顶层退出动作”，再逐步收敛实现细节。

### 风险 7：rename session 在右键、切 panel、再次新建时没有先收束

- 风险：用户在 rename 中右键空白区、切到 `Snippets`、或再次点击 `+`，若旧 session 未先结束，会留下脏 draft、失焦未提交或选中态异常。
- 缓解：任何会打开新 overlay、切换 panel、或触发新建动作的事件，都必须先调用统一的 rename-session 收束逻辑。
- 回滚：如果跨模块统一入口一轮内做不完，首轮至少要覆盖“右键打开菜单”和“panel 切换”两条高频路径。

### 风险 8：唯一命名规则的解析边界不明确

- 风险：设计已规定“最小缺失正整数”，但如果没有限定哪些 label 参与编号池，手工命名、大小写差异、前后空格、带后缀文本等会让实现结果不可预测。
- 缓解：首版只识别严格格式 `Folder {n}` 与 `SSH Connection {n}`，并在比较前执行 trim；其余自由命名不进入默认编号池。
- 回滚：若 rename 冲突提示一轮内来不及补齐，至少先保证 create 路径严格唯一，rename 路径保留后续细化空间。

### 风险 9：动态 toolbar descriptor 与旧 create menu 语义混搭

- 风险：如果一部分 panel 开始走“primary action”，另一部分仍沿用旧的 console 双项 create menu，会出现 tooltip 与真实动作不一致的问题。
- 缓解：implementation plan 必须明确 `+` 在每个 panel 上是 direct primary action 还是 menu trigger，禁止过渡态长期共存。
- 回滚：若 `Snippets` / `Keychain` 暂时没有 placeholder create 行为，可先置灰并给 tooltip，而不是复用 console create menu。

### 风险 10：菜单 icon 资源不完整导致风格半成品

- 风险：当前仓库现成 Fluent icon 无法完整覆盖 `Rename / Copy / Cut / Delete / Refresh / Import / Export` 等动作，如果不作为前置任务处理，会导致菜单只有部分项带 icon。
- 缓解：在实现前先盘点现有图标资源，并把缺失项补齐为明确任务。
- 回滚：若个别低优先级 planned action 暂时没有 icon，可先隐藏该 action，而不是显示“无 icon 的孤立文本行”。

### 风险 11：空查询搜索的 click-away 路径继续碎片化

- 风险：如果实现继续沿用“header dismiss + panel background dismiss + workspace dismiss”的补丁式方案，活动栏点击、资产行点击、未来右侧面板点击仍会出现漏关。
- 缓解：把空查询搜索收起逻辑提升到统一 shell-level 策略，明确覆盖整块 shell body，而不是由各子区域自行决定。
- 回滚：若首轮无法做到真正统一，至少要先覆盖活动栏点击与资产列表点击这两条当前已知缺口。

## 回滚策略

如果实现阶段发现改动面超出预期，允许按以下顺序降级，但不改变本轮选定方向：

1. 保留 `1B`、`2A`、`5A`，先完成菜单壳层、action metadata 与动态 toolbar；
2. 顶部搜索先完成统一的空查询 click-away 收起，再细化各子区域事件协同；
3. rename 先完成显式 session 与 click-away 提交，再细化 rename 冲突表现；
4. Snippets / Keychain 若暂时只完成 tooltip 与按钮状态，也不回退成静态 console 文案。

## 验证清单

### 功能验收

- [ ] blank-area 菜单不再出现顶部中文标题
- [ ] blank-area 菜单高度紧贴内容，不再出现大块留白
- [ ] folder / ssh 菜单为完整单层 surface，不再出现下半段脱壳
- [ ] 所有菜单项都显示 `icon + english label`
- [ ] `Console Tree` 与 `Hosts, recent sessions, favorites` 已从常驻 UI 移除
- [ ] toolbar 所有 icon 都有 tooltip，且 tooltip 随当前 panel / 当前状态动态变化
- [ ] `Console` 下 `+` 按钮主动作是 `New SSH Connection`
- [ ] `Snippets` 下 `+` 按钮主动作是 `New Snippet`
- [ ] `Keychain` 下 `+` 按钮主动作是 `New Keychain`
- [ ] 新建文件夹后立即进入 rename
- [ ] 新建 SSH 连接后立即进入 rename
- [ ] rename 期间点击外部区域会提交并退出
- [ ] `Esc` 可以取消 rename
- [ ] 同类型资产在 create 后不会产生重名
- [ ] “最小缺失正整数”分配规则与示例行为一致
- [ ] 顶部搜索在空查询状态下，点击资产列表行会自动收起
- [ ] 顶部搜索在空查询状态下，点击左侧活动栏会自动收起
- [ ] 顶部搜索在空查询状态下，点击主工作区会自动收起
- [ ] 顶部搜索在有输入内容时，不会因普通点击而自动收起

### 实现守护

- [ ] 菜单列宽的视觉值与 Rust placement 宽度只有一个真源，或在实现计划中明确成对修改
- [ ] rename dismiss、context menu dismiss、create dismiss、empty-search dismiss 的优先级已明确定义
- [ ] 任何打开新 overlay、切换 panel、或触发新建的动作都会先收束 rename session
- [ ] 默认编号池规则已限定为严格格式 `Folder {n}` / `SSH Connection {n}`
- [ ] 菜单 icon 资源已覆盖所有可见 action，或无 icon 的 action 在首轮被显式移除

### 测试分层映射

- [ ] `tests/assets_context_menu_ui_contract_smoke.sh` 补充：
  - 菜单不再依赖中文 `操作`
  - 菜单行具备 icon 槽位
  - 菜单 overlay / column 不再写死固定高度
- [ ] `tests/assets_context_menu_smoke.rs` 补充：
  - 菜单边缘定位仍保持在窗口 bounds 内
  - rename / context menu / create action 之间的桥接顺序稳定
- [ ] `tests/shell_view_model.rs` 补充：
  - `Folder 2 + Folder 3 -> Folder 1`
  - `Folder 1 + Folder 2 -> Folder 3`
  - `SSH Connection` 与 `Folder` 序号池互不干扰
  - rename session 在 `Enter / Esc / click-away / panel switch` 下行为一致
- [ ] `tests/assets_sidebar_toolbar_ui_contract_smoke.sh` 或新增同层脚本补充：
  - `+` tooltip 与动作随 panel 变化
  - 顶部搜索的 dismiss 路径不再只覆盖主工作区
- [ ] `tests/sidebar_tooltip_ui_contract_smoke.sh` 保持：
  - 活动栏 tooltip 线路仍然完整，且不会阻断空查询搜索的 click-away 收起
