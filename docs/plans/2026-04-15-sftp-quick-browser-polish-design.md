# SFTP Quick Browser Polish Design

日期: 2026-04-15
执行者: Codex
状态: 已确认，待实现

## 背景

当前仓库已经具备 SFTP quick browser、workspace SFTP tab、SSH 会话管理和 Slint 壳层，但本轮暴露出三个核心问题：

- 右侧 quick browser 仍然沿用多列表格布局，在 392px 窄栏下产生横向滚动和视觉拥挤；
- quick browser 的目录读取仍走同步链路，切换 SSH tab 时会阻塞 UI，严重时导致窗口未响应；
- 相同 SSH 连接名打开多个 tab 时，标签显示完全相同，无法快速区分。

直接相关代码位置：

- `ui/shell/right-panel.slint`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/sftp.rs`
- `src/app/ssh/session_manager.rs`
- `src/shell/view_model/sftp.rs`
- `src/shell/tabs.rs`

## 根因分析

### 1. 右栏布局根因

当前 quick browser 本质上仍是“迷你版资源管理器”：

- 顶部是 `Expand / connection badge / Follow` 等权按钮；
- 列表区使用 `Name / Type / Modified / Size` 四列表头；
- 通过 `viewport-x` 和 `horizontal-scrollbar-policy: always-on` 支撑横向滚动。

这套结构对宽工作区是成立的，但对固定 392px 右栏并不成立。用户需要的是“快速浏览、快速定位、快速切回终端”的 quick browser，而不是缩窄版表格文件管理器。

### 2. 卡顿与未响应根因

当前 quick browser 的目录同步路径为：

`session_projection_timer -> sync workspace projection -> ensure/sync active sftp browser -> execute_sftp_browser_request -> SessionManager::sftp_read_dir -> runtime_handle.block_on(...)`

这意味着：

- SFTP 目录读取被放进了 UI 轮询链路；
- 切换 tab、Follow CWD、refresh 时，UI 线程会同步等待远端目录结果；
- 当网络延迟或远端目录较大时，窗口消息循环被阻塞，最终表现为卡顿甚至未响应。

因此“仅加缓存”不是根因修复。真正需要修的是：目录读取必须移出 UI 同步链路，改成后台异步请求 + UI 结果回投。

### 3. 重名标签根因

`WorkspaceTab::from_session(...)` 当前直接使用连接名或 host 派生 title，没有做重复标题消歧。因此多个同名 SSH tab 同时存在时，显示层无法区分。

## 目标

### 本轮必须达成

- 右侧 quick browser 重做为真正的窄栏 quick browser；
- 顶部工具区改成两行层级：第一行 badge + icon actions，第二行 breadcrumb；
- `Expand` 与 `Follow` 改为图标按钮；
- quick browser 去掉对横向滚动的依赖，不再使用多列表格主布局；
- 文件图标按类型语义上色，不再统一纯白线稿；
- SFTP 目录同步改为后台异步，不再阻塞 tab 切换；
- quick browser 支持会话级目录快照复用，切 tab 先显示上次内容，再后台刷新；
- 同名 SSH tab 自动编号，且编号可补位复用。

### 本轮不做

- 完整重做 workspace SFTP host；
- 完整 pin/unlink 多主机 quick browser 模式；
- 远端目录树；
- 文件预览器升级；
- 高级缓存失效策略（如目录监听、mtime 校验、增量刷新）。

## 方案对比

### 方案 A：继续保留表格，仅修横向滚动和按钮样式

优点：改动小。

缺点：

- 仍然维持错误的信息架构；
- 右栏会继续像“缩小版 explorer”；
- 横向滚动即使修好也仍不适合 quick browser；
- 无法从根上改善拥挤感。

### 方案 B：重做为真正的 quick browser

特征：

- 头部两行；
- 列表只保留 Name 主列；
- 次级信息改为行内 meta；
- SFTP 读取改后台异步；
- 右栏优先展示快照与最新请求结果。

优点：

- 同时解决视觉和性能问题；
- 与当前产品“终端优先，文件浏览附属”定位一致；
- 实现范围可控，不需要推翻 workspace 架构。

缺点：

- 需要修改现有 quick browser 渲染契约；
- 需要补一套异步结果回投机制。

### 方案 C：直接把 workspace SFTP host 一并重做

优点：可以形成统一文件浏览体验。

缺点：

- 范围过大；
- 本轮用户痛点主要在 quick browser 与 tab 切换；
- 容易把问题从“修体验”扩展成“大重构”。

## 最终决策

采用 **方案 B：重做 quick browser + 后台异步刷新 + 标签消歧**。

## 信息架构

### 顶部工具区

第一行：

- `ConnectionBadge`
- `FollowToggleIconButton`
- `RefreshIconButton`
- `ExpandIconButton`

第二行：

- `BreadcrumbBar`

规则：

- `ConnectionBadge` 是信息展示，不做主操作按钮；
- `Follow` 是 toggle，激活时使用 filled icon；
- `Refresh` 是一次性动作；
- `Expand` 是一次性动作，不做持久 active 语义；
- breadcrumb 负责路径上下文与点击跳转。

### 文件列表

每一行由三部分组成：

- 左侧文件类型图标；
- 第一行主文本：文件名；
- 第二行次级 meta：`type / modified / size` 按宽度裁剪显示。

窄栏下不再出现单独的 `Type / Modified / Size` 列，也不再保留横向滚动依赖。

### 状态层

状态从“大块表格状态行”收敛成轻量 status strip：

- `Syncing...`
- `Disconnected`
- `Failed`

当目录正在后台刷新时：

- 若已有快照，则保留当前列表，仅显示轻量同步状态；
- 若首次加载无快照，则显示 loading 状态与骨架/空态。

## 图标与视觉语言

### 顶部动作图标

优先采用 Fluent UI System Icons：

- Expand: `panel-right-expand-20-regular`，hover/pressed 可切 `filled`
- Follow: `link-20-regular`，active 用 `link-20-filled`
- Refresh: `arrow-sync-20-regular`；若仓库内暂缺该资源，先补入对应 Fluent 资产

### 文件图标语义

按应用侧分类着色：

- directory：低饱和蓝系
- normal file：中性灰
- symlink：青蓝
- executable/script：绿色
- archive：橙/琥珀
- image：莓红/紫灰
- config/code：靛蓝
- parent row：导航弱化色

分类优先依据：

- `SftpDirectoryEntryKind`
- 文件扩展名
- 常见配置/脚本文件名

## 性能与状态模型

### 请求模型

目录读取改为异步请求：

1. UI 线程发出浏览请求；
2. quick browser 状态立即切为 `loading` 或 `refreshing`；
3. 后台 Tokio 任务执行 `read_dir`；
4. 任务完成后通过 `slint::invoke_from_event_loop` 把结果投回 UI 状态；
5. 使用 `request_id` 丢弃过期结果，只采纳最后一次有效请求。

### 快照策略

每个 file browser session 保留最近一次成功目录快照：

- 切换 SSH tab 时优先展示该 session 的上次目录内容；
- 如果处于 `Follow CWD`，后台再根据最新 cwd 发新请求；
- 用户显式点击 refresh 时无条件刷新；
- 自动跟随触发时可以做轻量去抖，避免短时间重复读同一路径。

### 线程边界

- `SessionManager` 不再在 UI 路径中 `block_on(read_dir)`；
- 同步接口只保留给测试或显式后台 worker 使用；
- quick browser 实际刷新由后台任务驱动，UI 层只消费投影结果。

## 同名标签命名

对 SSH workspace tabs 做显示层消歧：

- 第一项保留原名，如 `sharon`
- 后续同名依次为 `sharon(2)`、`sharon(3)`
- 若中间项关闭，则下次新开优先补位，例如 `sharon(2)` 被关后，下次新开复用 `sharon(2)`

规则仅影响显示 title，不影响：

- session id
- asset id
- 连接配置持久化名称

SFTP workspace tab 若来源于对应终端 tab，可同步使用消歧后的 host title，避免 `Files: sharon` 再次混淆。

## 实现影响面

- `ui/shell/right-panel.slint`：重做 quick browser 头部与列表布局
- `src/app/bootstrap/sftp.rs`：调整 quick browser 投影字段与异步请求回投
- `src/app/ssh/session_manager.rs`：补充后台可用的 async SFTP 目录读取接口
- `src/shell/view_model/sftp.rs`：收缩表格心智，增加快照/refreshing 状态投影
- `src/shell/tabs.rs` / workspace projection：实现重复 tab 标题编号
- 测试：render、view-model、session manager、bootstrap 行为回归

## 验证标准

- 右侧 quick browser 不再依赖横向滚动才能看到文件信息；
- 切换 SSH tab 时窗口保持可交互，不出现明显主线程卡死；
- Follow CWD 与 refresh 结果只采纳最新请求；
- quick browser 在已有快照时切换 tab 能立即显示内容；
- 同名 SSH 标签显示稳定编号，关闭后编号可复用；
- 相关渲染测试、view-model 测试、session manager 测试与 bootstrap 行为测试通过。
