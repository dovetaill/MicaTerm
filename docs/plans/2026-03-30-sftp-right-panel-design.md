# SFTP 右侧面板 Design

日期: 2026-03-30
执行者: Codex
状态: 已实现并完成 Task 8 验证（2026-03-31）

## 背景

当前仓库已经具备以下与本任务直接相关的基础：

- `SSH` 会话生命周期、标签页和连接进度链路已经存在；
- 右侧面板已经有独立布局位，但当前仅支持 `Appearance` 视图；
- 左侧资产区、右键菜单、模态框、偏好持久化、终端 surface 投影都已经有成熟壳层；
- 当前产品定位更偏 `SSH / Terminal` 主导，而不是独立双栏文件管理器。

关键代码位置：

- `src/app/ssh/runtime.rs`
- `src/app/ssh/session_manager.rs`
- `src/shell/view_model.rs`
- `src/app/bootstrap.rs`
- `ui/app-window.slint`
- `ui/shell/right-panel.slint`

## 竞品调研结论

本轮调研以官方文档或官方产品页面为主，结论如下：

### 1. 文件管理器优先产品

`WinSCP`、`FileZilla`、`Xftp`、`Cyberduck` 这类产品将文件浏览器作为主工作区。

共性：

- 路径栏、文件列表、目录树、传输队列都是核心主界面要素；
- 常见布局是双栏或宽区域布局；
- 用户可以在不依附终端的情况下独立完成文件操作。

参考：

- <https://winscp.net/eng/docs/ui_commander>
- <https://wiki.filezilla-project.org/FileZilla_Client_Tutorial_%28en%29>
- <https://docs.cyberduck.io/cyberduck/browser/>
- <https://www.netsarang.com/en/xftp/>

### 2. 终端优先产品

`MobaXterm`、`Xshell`、`HexHub`、`Termius` 更偏向“终端主导，SFTP 依附于当前主机或会话”。

共性：

- SFTP 跟随当前 SSH 会话或当前主机；
- SFTP 往往作为附属区域、文件管理面板或与连接系统同层级的工作区；
- 传输队列通常是全局队列，但会强调当前会话的任务；
- 路径联动或 Follow CWD 是高价值特性。

参考：

- <https://mobaxterm.mobatek.net/features.html>
- <https://www.netsarang.com/docs/Xshell8_manual.pdf>
- <https://www.hexhub.cn/>
- <https://termius.com/blog/termius-x>
- <https://termius.com/blog/termius-for-ios-new-navigation-and-sftp>

### 3. 针对 `mica-term` 的结论

`mica-term` 当前显然属于“终端优先产品”，而不是双栏文件传输主应用。

因此本轮不应复制 `WinSCP / Xftp` 的完整双栏工作区，而应采用：

- 右侧固定 `SFTP` 面板；
- 默认绑定当前激活 SSH tab；
- 面板主体做单栏远端文件工作区；
- 传输队列使用全局模型，但在右侧展示当前 session 摘要；
- 未来可扩展 `Pin / Unlink`，但首轮不做。

## 目标

### 本轮必须达成

- 在右侧面板增加固定 `SFTP` 区域；
- `SFTP` 默认跟随当前激活的 SSH 标签页；
- 顶部使用浏览器式导航区，包含 `Back / Forward / Refresh / Up / Path Bar`；
- 支持远端目录浏览、路径输入跳转、路径历史；
- 支持上传文件、上传文件夹、下载、重命名、删除、新建文件夹；
- 支持右键菜单，样式为 `图标 + 文字`；
- 支持拖拽上传、本地拖入、远端内部 move；
- 提供全局传输队列和当前会话摘要；
- 提供冲突处理对话框；
- 支持 `Follow CWD`；
- SSH 断开时，右侧面板进入可恢复状态，不直接崩坏或清空为不可理解状态。

### 体验目标

- 让右侧 SFTP 面板看起来像成熟桌面 SSH 工具的一部分，而不是临时拼接浮层；
- 保持 `mica-term` 当前 flat / tool-like 方向；
- 保持交互直接、密度高、主动作清晰；
- 不为了模仿双栏工具而牺牲 392px 右侧空间的可用性。

## 边界

### 本轮覆盖

- 右侧面板新增 `SFTP` 模式；
- `SFTP` 跟随 active SSH session 的绑定；
- SFTP 面板导航、目录列表、路径跳转、基础文件操作；
- 右键菜单与操作调度；
- 全局传输队列模型与右侧摘要；
- 冲突处理与错误反馈；
- `Follow CWD` 行为；
- 与现有 SSH runtime / SessionManager / ShellViewModel / Slint shell 的集成。

### 本轮不覆盖

- 双栏本地/远端布局；
- 常驻目录树；
- 远端拖出到系统文件管理器；
- 完整属性编辑器（如 `chmod/chown/chgrp`）；
- 文件预览器、图片预览器、diff/sync 向导；
- 多面板固定 pin / unlink；
- 自动文件监视与实时协同刷新。

## 方案对比

### 方案 A：轻量检查器

右侧仅提供极简 inspector：

- 当前路径
- 少量列表
- 上传/下载入口

优点：

- 实现快；
- 侵入性低。

缺点：

- 无法满足“像成熟 SFTP 工具”的预期；
- 右键菜单、路径历史、冲突处理、拖拽流都容易半吊子；
- 很快需要第二轮推翻。

### 方案 B：右侧 SFTP 工作区

右侧固定完整单栏远端工作区，绑定当前 active SSH session。

优点：

- 与当前布局契合度最高；
- 与 `MobaXterm / Xshell / HexHub / Termius` 一类终端优先产品心智一致；
- 能完整承接路径栏、列表、右键菜单、传输摘要与 Follow CWD；
- 不需要改写主工作区结构。

缺点：

- 不是双栏本地/远端文件管理器；
- 队列和目录树需要做减法设计。

### 方案 C：独立 SFTP 标签页或主工作区

将 SFTP 提升为主工作区，右侧仅显示辅助信息。

优点：

- 能实现最大功能上限；
- 更容易向双栏文件工具演进。

缺点：

- 与“固定放在右边侧边栏”的要求冲突；
- 与现有 shell 结构冲突；
- 首轮改动面明显过大。

## 最终决策

采用 **方案 B：右侧 SFTP 工作区**。

理由：

- 最符合当前仓库真实结构；
- 最符合当前产品形态；
- 在 392px 右栏内能保住真正可用的路径栏和文件列表；
- 便于首轮先落稳定可用版本，再逐步加高级能力。

## 信息架构

### 1. 右侧面板模式

右侧面板新增：

- `RightPanelView::Appearance`
- `RightPanelView::Sftp`

行为：

- 用户在 SSH tab 激活后，右侧面板可以切换到 `SFTP`；
- 若当前无 active SSH session，则右侧显示空态引导；
- `right_panel_view` 继续走现有偏好持久化。

### 2. 面板绑定语义

默认采用：

- `SFTP 面板 = active SSH session 的远端文件工作区`

不是：

- 独立常驻的第二个连接中心；
- 独立于 tab 的固定主机浏览器。

附加规则：

- 路径历史按 `session_id` 保存；
- 当前路径、选中状态、Follow CWD 模式按 `session_id` 保存；
- 全局队列独立于 session，但支持按 session 过滤展示。

### 3. Follow CWD

首轮包含 `Follow CWD`：

- 默认开启；
- active SSH shell 当前工作目录变化时，右侧路径同步；
- 一旦用户在右侧主动浏览到其它目录，切换到 `manual-browse`；
- 面板提供“重新跟随终端路径”的显式入口。

## 右侧面板布局

右侧面板固定分为四层：

### 1. 会话条

内容：

- `user@host`
- 状态点：`Connected / Loading / Disconnected`
- `Follow CWD`
- `Open Queue`
- `More`

要求：

- 紧凑、工具栏化；
- 不做大卡片标题；
- 不挤占文件列表垂直空间。

### 2. 浏览器式导航栏

按钮与控件：

- `Back`
- `Forward`
- `Refresh`
- `Up`
- `Path Bar`
- `Upload`
- `New Folder`

路径栏行为：

- 默认显示远端绝对路径，如 `/var/www/app/releases`
- 支持点击编辑并回车跳转
- 支持非法路径 inline 报错
- 支持 Follow CWD 同步

### 3. 远端文件区

首轮只做单栏远端浏览：

- 默认列：`Name | Size | Modified`
- 文件夹置顶；
- 支持单选、多选、双击进入；
- 392px 右栏内不做常驻目录树；
- 不加入 `Owner / Group / Permissions` 等高密度列。

双击行为：

- 文件夹：进入目录
- 文本文件：下载到临时区并走编辑/打开流程
- 其他文件：触发下载或默认打开

### 4. 底部传输摘要条

显示：

- 当前活动任务数
- 失败任务数
- 最近一次完成状态

点击后展开：

- `Queue Drawer`

不采用：

- 右栏底部长驻完整队列列表

理由：

- 392px 右栏空间不足；
- 必须优先保证路径栏和文件列表。

## 右键菜单设计

### 空白区域

- `Upload Files...`
- `Upload Folder...`
- `New Folder`
- `Paste`
- `Refresh`
- `Copy Current Path`

### 文件夹

- `Open`
- `Open in Terminal Here`
- `Download`
- `Upload Here`
- `Rename`
- `Delete`
- `Copy Path`
- `Properties`

### 文件

- `Open`
- `Download`
- `Edit`
- `Rename`
- `Delete`
- `Copy Path`
- `Copy SFTP URL`
- `Properties`

### 多选

- `Download Selected`
- `Delete Selected`
- `Copy Paths`
- `Cancel Transfers`
- `Refresh`

决策：

- 保持 `图标 + 文字`
- 最多一层子菜单
- 复用现有 context menu domain model，不重新造菜单系统

## 文件操作与拖拽

### 首轮必须支持

- 上传文件
- 上传文件夹
- 下载
- 新建文件夹
- 重命名
- 删除
- 本地拖入上传
- 远端内部拖动到文件夹执行 move

### 首轮明确降级

- 远端拖出到系统文件管理器不做第一版硬要求

原因：

- 系统级 drag-out 成本明显高于本地拖入；
- 与当前产品首轮目标相比，不应成为阻塞项。

## 冲突处理

上传/下载遇到同名文件时，弹出冲突对话框：

- `Replace`
- `Skip`
- `Keep Both`
- `Resume`（仅可续传时出现）
- `Apply to All`

目录冲突：

- 默认 `Merge`
- 真正遇到同名文件时再进入文件冲突决策

重命名冲突：

- 直接 inline 报错
- 不自动补号
- 不静默改名

## 错误状态

### 目录加载失败

- 文件区顶部显示紧凑错误条
- 提供 `Retry`

### SSH 断开

- 面板进入 `Disconnected`
- 保留最后一次目录快照
- 所有写操作 disabled
- 提供重连入口

### 单个传输失败

- 队列项展示失败原因
- 支持重试

### 权限不足

- 就地提示
- 不使用全局重型 toast

## 技术架构决策

### 核心决策

SFTP 采用：

- **复用当前 SSH 会话，在同一 runtime 内建立 SFTP 子通道**

不采用：

- 为右侧面板单独新建一条独立 SSH 连接

理由：

- 认证、代理链、断线语义都可复用；
- 更符合“跟随当前 SSH tab”的产品语义；
- 用户不会看到“终端连着、SFTP 却像另一台主机”的割裂。

### 模块边界

建议新增：

- `src/app/sftp/mod.rs`
- `src/app/sftp/model.rs`
- `src/app/sftp/runtime.rs`
- `src/app/sftp/queue.rs`
- `src/app/sftp/session_binding.rs`
- `src/app/sftp/local_ops.rs`

职责：

- `model`：面板状态、目录项、历史、冲突策略、视图行模型
- `runtime`：SFTP 子系统调用与文件操作
- `queue`：全局传输队列和进度聚合
- `session_binding`：session 与面板状态绑定、Follow CWD
- `local_ops`：本地文件选择、临时目录、上传源解析

## 状态模型

### Right Panel

- `appearance`
- `sftp`

### SftpPanelMode

- `empty`
- `connecting`
- `disconnected`
- `loading`
- `ready`
- `error`

### FollowMode

- `follow-cwd`
- `manual-browse`

### TransferTaskState

- `queued`
- `running`
- `paused`
- `completed`
- `failed`
- `cancelled`
- `conflict`

## 数据流

1. active SSH tab 变化，或 SSH runtime 发出 `Connected / Disconnected / CurrentDirectoryChanged / SurfaceChanged` 事件
2. `SessionManager` 维护每个 session 的 `SftpSessionBinding` 与 `current_working_directories`
3. `bootstrap::sync_active_sftp_projection_from_manager(...)` 将 active session snapshot 投影到 `ShellViewModel`
4. `ShellViewModel` 根据当前 session reducer 决定路径：
   - `follow-cwd` 时跟随 `current_working_directories`
   - `manual-browse` 时保留该 session 的 `current_path/history`
5. UI 操作通过 `AppWindow` / `RightPanel` callback 回到 Rust
6. Rust 通过 `SessionManager` 与 `app/sftp/*` 调度 runtime / queue
7. `ShellViewModel` 与 queue 摘要重新投影回 Slint

## 已落地补充（2026-03-31）

### 实际架构补充

- `app/sftp/` 承担 reducer、queue、local ops 和 session binding helper；
- `src/app/ssh/runtime.rs` 仍保留 `RusshSftpBackend` 与 OSC7 cwd 提取逻辑，因为 SFTP 子通道复用 SSH runtime；
- `src/app/ssh/session_manager.rs` 作为 session 级 authority，统一保存 live SFTP binding、cwd snapshot 与 retry/disconnect 语义；
- `follow-cwd` 仅在当前 session 仍处于 follow 模式时推进路径；一旦用户通过 path submit / back / forward / up 进入 `manual-browse`，后续 cwd 推送不会覆盖用户路径；
- 断线时面板仅切换到 `disconnected`，保留最后的 `current_path/history/selection`；重试通过 `SessionManager::retry_session(...)` 重新绑定 live runtime。

### 实际 UI 合同补充

- 已落地工具栏为 `Back / Next / Up / Sync / Re-follow / Path Bar`；
- 设计稿中的 `Upload / New Folder` 顶部按钮本轮未落地，相关动作仍主要从 SFTP blank-area / row context menu 进入；
- `TransferQueue` 已支持 `Overwrite / Skip` conflict policy；
- `SftpConflictModalState` 与 `ui/components/sftp-conflict-modal.slint` 已存在，但当前尚未挂载到 `AppWindow`，后续 TDD 需要补齐端到端冲突弹窗覆盖。

## 首版验证要求

### 必须覆盖的测试

- active session 切换时的绑定规则
- Follow CWD 与 manual-browse 切换
- 路径历史 `back / forward / up`
- 队列摘要统计
- 目录加载 / 断线 / 重连状态跳转
- 右键菜单 enable / disable
- UI 视图投影与偏好持久化

### 手工 smoke

- 连接 SSH 后右侧自动可用
- shell `cd` 后路径栏跟随
- 手动浏览后 Follow CWD 暂停
- 上传/下载/删除/重命名/新建文件夹
- 断线后进入禁用态且可重连

## 风险与取舍

### 风险 1：右栏宽度不足

处理：

- 首轮坚持单栏远端浏览
- 目录树不常驻
- 队列只做摘要 + drawer

### 风险 2：runtime 复杂度上升

处理：

- SFTP 子系统独立到 `app/sftp/`
- SSH runtime / SessionManager 只保留会话桥接、cwd 事件发布与 live binding 生命周期
- 不再额外引入第二套 session watcher 或独立连接管理层

### 风险 3：系统拖拽集成过深

处理：

- 首轮优先本地拖入上传和远端内部 move
- 远端拖出到系统后置

## 最终结论

`mica-term` 的 SFTP 首版应被实现为：

- 固定右侧单栏远端文件工作区；
- 默认跟随当前 active SSH tab；
- 复用当前 SSH runtime 开 SFTP 子通道；
- 保留成熟桌面 SFTP 工具的核心能力，但在 392px 右栏里做严格减法；
- 用全局传输队列承接上传下载，用会话级路径历史和 Follow CWD 保证“终端优先”体验。
