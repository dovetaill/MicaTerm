# SFTP Workspace Productized UI Design

日期: 2026-05-19
执行者: Codex
状态: 已获用户确认，进入实现计划阶段

## 目标

将当前 `79dc68d0c4bfe8c7d9bbacb0fa9f719aa4d374df` 上已经能显示真实远端目录的 `SFTP Workspace Tab`，从工程占位版升级为符合 MicaTerm / Ayu 当前产品语言的成熟文件工作区。此次设计不是把 MicaTerm 做成 WinSCP / FileZilla，而是在保持 terminal-first 定位的前提下，让中部工作区承担完整文件工作流，让右侧继续保持 lightweight shell-adjacent 工具属性。

## 输入资料

### 本地资料

- 设计目标图：`/home/images/20260518.png`
- 当前截图：
  - `/home/images/Snipaste_2026-05-19_15-14-15.png`
  - `/home/images/Snipaste_2026-05-19_15-15-13.png`
- 已确认设计/实现文档：
  - `docs/plans/2026-04-15-sftp-two-layer-workspace-design.md`
  - `docs/plans/2026-04-15-sftp-two-layer-workspace-implementation-plan.md`
  - `docs/plans/2026-04-15-sftp-quick-browser-polish-design.md`
  - `docs/plans/2026-04-15-sftp-quick-browser-polish-implementation-plan.md`
  - `docs/plans/2026-05-12-ayu-terminal-neighborhood-design.md`
  - `docs/plans/2026-05-12-ayu-terminal-neighborhood-implementation-plan.md`
  - `docs/plans/2026-05-14-ayu-refinement-design.md`
  - `docs/plans/2026-05-14-ayu-refinement-implementation-plan.md`

### 当前代码基线

- 提交：`79dc68d0c4bfe8c7d9bbacb0fa9f719aa4d374df`
- 提交说明：`Implement real SFTP workspace tabs`
- 已存在的重要结构：
  - `WorkspaceTab::Sftp`
  - `FileBrowserSession`
  - `workspace_sftp_*` AppWindow / WorkspacePane / ShellViewModel projection
  - workspace SFTP 虚拟列表窗口与 spacer 高度
  - SFTP context menu dispatcher
  - SFTP new file / new folder / rename / delete confirm / remote file editor / transfer center

### 外部参考

- Termius SFTP 文档强调 desktop SFTP 是完整会话，可以在 terminal 与 SFTP 之间切换，也支持 drag-and-drop 与 file editing。
- Termius 桌面导航文章强调 horizontal tabs 与更小侧栏是为了给 focused work 留出宽度。
- MobaXterm 的 SSH + auto SFTP 模式证明 terminal-led 产品可以集成文件浏览，但不应让整个产品心智退化成文件管理器。
- Fluent 2 toolbar 指南强调 toolbar 不应换行；空间不足时应 overflow，而不是把按钮截断。

参考链接：

- <https://docs.termius.com/organize-and-connect-to-hosts/managing-files-with-sftp>
- <https://termius.com/blog/termius-x>
- <http://mobaxterm.mobatek.net/en/>
- <https://fluent2.microsoft.design/components/web/react/core/toolbar/usage>
- <https://fluent2.microsoft.design/layout>

## 当前代码与现状结论

### 已经做对的部分

- `workspace-session-host-mode == "sftp"` 已能切换到独立 `SftpWorkspaceHost`，不是简单 placeholder。
- workspace SFTP 与 quick browser 已经基于独立 `FileBrowserSession` 工作，没有回退成一个全局可变 SFTP 状态。
- workspace SFTP 已经有真实 breadcrumb、真实 rows、真实 selection、真实 context menu request、真实 upload/new folder/retry callback。
- quick browser 与 transfer center 的大部分后台链路已经存在，本轮不需要重做 SFTP backend。

### 当前仍然阻碍产品化的问题

1. `ui/shell/sftp-workspace-host.slint` 仍然是“标题卡 + 文本按钮 + path input + 表格”的工程堆叠，不像 Ayu shell 的成熟工作区。
2. toolbar 还是文字按钮，无法与现有 Fluent icon、compact chrome、标题栏工具按钮语言对齐。
3. path bar、表格列与整体宽度协调不成熟，右侧存在裁切，`New Folder` 已出现 `New Folde` 截断。
4. active workspace tab 为 SFTP 时，右侧 quick browser 仍然展示同一路径文件列表，主区和右区信息重复，直接压缩可用宽度。
5. 顶部信息层级混乱：tab 标题、workspace 内标题、状态 pill 都在重复表达同一状态。
6. 文件列表只具备功能，没有成熟的文件图标、响应式列、selected subtle fill、accent rail、底部状态条。
7. 当前状态视图只停留在顶部大 pill / 文案，没有把 loading、error、disconnected、read-only 组织成一致的 workspace 状态系统。
8. 右键菜单、弹窗、transfer entry、响应式收缩策略虽然底层已有一部分，但 workspace host 没有把这些能力组织成产品级 UI。

## 产品原则

### 1. Terminal-first，不做独立文件管理器产品

- 中部 `SFTP Workspace Tab` 是完整文件工作区，但仍然服务于 SSH / terminal 主工作流。
- 不引入 dual-pane local/remote 主视图，不把默认体验做成 WinSCP / FileZilla。

### 2. 右侧仍是 lightweight shell neighborhood

- 右侧存在的前提是“辅助”而不是“第二个主浏览器”。
- 当中部已经是完整 SFTP workspace 时，右侧不能再显示同一目录的 duplicate file list。

### 3. Ayu 主题只允许一条 runtime projection 链

- 颜色来源继续以 `src/theme/spec.rs` + `src/app/bootstrap/shell_chrome.rs` + `ui/app-window.slint` runtime projection 为准。
- `ui/theme/tokens.slint` 仍然只做 boot-time parity snapshot。
- `ui/shell/sftp-workspace-host.slint` 禁止硬编码第二套 Ayu palette。

### 4. 选中态沿用最新 Ayu refinement

- selected = subtle fill + accent rail。
- focus = focus ring，仅在键盘焦点场景出现。
- 不重新引入重橙色盒子边框。

### 5. Flat / no-radius 或 very-subtle-radius

- 不做网页卡片堆叠感。
- 内层表面允许 0px-6px 的极弱圆角，但不能破坏 MicaTerm 当前 flat chrome。

## 方案对比

### 方案 A：保留右侧，但切成 compact inspector

优点：

- 结构变化相对小；
- 视觉上仍保留右侧的信息位。

缺点：

- 主区宽度仍被持续挤压；
- 用户仍会觉得“同一个 SFTP 工作流有两个并列面板”；
- 无法最直接解决当前截图里最明显的重复目录问题。

### 方案 B：SFTP workspace 激活时自动折叠右侧 SFTP quick browser

优点：

- 最直接给主区释放宽度；
- 最符合 “right side is quick browser, center is full workspace” 的两层设计；
- 最符合用户当前明确要求；
- 不需要为本轮创造新的右侧 inspector 信息架构。

缺点：

- 需要在 ShellViewModel / layout 层引入“用户手动折叠”与“被策略隐藏”的区别；
- revive strip 不能误导性地允许用户在 SFTP active 时重新打开 duplicate list。

### 方案 C：把右侧彻底重做成 inspector + transfer summary

优点：

- 最完整的长期方向；
- 更有利于未来扩展 session health、queue summary、host metadata。

缺点：

- 超出本轮范围；
- 容易把“产品化 UI 修整”膨胀成“大规模 side region redesign”。

## 最终决策

采用方案 B，并明确以下硬决策：

- 当 active workspace tab kind 为 `SFTP`，且 right panel 当前 view 为 `SFTP` 时，右侧 duplicate quick browser 默认自动折叠。
- 这种折叠属于 `policy-hidden`，不是 `user-collapsed`；因此不展示用于恢复 duplicate file list 的 revive strip。
- transfer center 仍然独立存在，upload/download 入口继续走全局 queue，不受右侧 quick browser 折叠影响。
- 右侧未来可以演进为 compact inspector / transfer summary，但不是本轮默认形态。

## 目标信息架构

### Shell 级结构

- Tab strip 继续拥有完整 tab title，例如 `Files: Interserver`。
- Workspace 主区 header 以 host-first 方式显示，避免在主区再次重复完整 tab 标题。
- Titlebar active summary 对 SFTP tab 改为 host-first + status-first 的简版摘要，不在 titlebar、tab、workspace header 三处同时重复 `Files: Interserver + Ready`。

### Workspace 主区结构

从上到下固定为五层：

1. compact workspace header
2. toolbar + breadcrumb/path 所在的 single chrome band
3. responsive file table
4. centered state view overlay（仅 loading empty error disconnected 等场景可见）
5. bottom status bar

## 详细设计

### 1. Compact workspace header

Header 高度控制在 `44px-52px`。

布局：

- 左侧 primary：`Interserver`
- 左侧 secondary：`SFTP · Locked · Manual` 或 `SFTP · Follow · Linked`
- 右侧 status：小状态点 + 短标签，例如 `● Ready`
- 右侧可选轻量 lock badge：仅在 read-only 时出现，不抢占 primary 层级

不再出现：

- 大标题卡
- 再次完整重复 `Files: Interserver`
- 占满整行的大 `Ready` pill

### 2. Toolbar

Toolbar 与 breadcrumb/path 必须在同一视觉带内，而不是两行笨重表单。

动作分组：

- 主导航 icon-only：
  - Back
  - Forward
  - Up
  - Home
  - Refresh
- 次级操作 icon + short label：
  - Upload
  - New Folder
- 余下动作进入 overflow：
  - New File
  - More actions / Properties

设计要求：

- toolbar 不换行；
- 按钮文字不得被截断；
- 禁用态低对比但可识别；
- icon 使用现有 Fluent 资产，不新增杂糅图标体系。

### 3. Breadcrumb / Path

默认态：

- 显示 breadcrumb，不默认展示粗糙 input。
- 根目录 `/` 固定渲染为稳定 root crumb，而不是空字符串。

编辑态：

- 点击 breadcrumb 区右侧 edit affordance，或走快捷键，切换到 editable single-line path input。
- input 必须完整利用当前可用宽度。

溢出策略：

- 优先保留最后一级路径和当前根信息；
- 中间路径段可以渐进折叠；
- 不允许 edit affordance 与 path 内容互相遮挡。

右侧 affordance：

- edit path
- copy path
- more / context

但这些 affordance 必须固定在同一宽度尾栏，不得侵蚀 breadcrumb 主文本区域。

### 4. 文件表格

Workspace 主区仍然使用多列表格，而不是回退成 quick browser 单列 list。

#### 列结构

常驻列：

- Name
- Type
- Size
- Modified

可选列：

- Permissions
- Owner
- Group

#### 响应式规则

- 宽度充足时显示所有列；
- 窄于第一阈值时隐藏 `Group`；
- 再窄时隐藏 `Owner`；
- 再窄时隐藏 `Permissions`；
- `Name / Type / Size / Modified` 仍必须完整可见，不允许 `Size` 贴边或被遮挡。

#### 行视觉

- Name 列左侧增加真实 file/folder icon。
- row hover 使用 subtle hover fill。
- row selected 使用 subtle selected fill + left accent rail。
- keyboard focus 与 selected 分离，不再使用重盒子边框。
- parent row `..` 使用更弱的导航语义色，不和普通 folder 混淆。

#### 行高与表头

- row height：`40px-44px`
- table header：`32px-36px`
- 分隔线使用低对比 `separator` / `hairline`
- 不做 alternating row banding

### 5. 文件图标策略

优先复用现有 Fluent 文件类图标和现有 runtime-projected shell palette。

最小语义分组：

- parent-directory
- directory
- file
- symlink
- archive
- image
- config
- executable

颜色策略：

- 尽量用已有 `text-primary` / `text-secondary` / `accent` / shell projected emphasis 做弱区分；
- 如果现有 token 仍不足以让 directory / neutral file / warning-like states 有可读差异，再新增最小数量 semantic icon tokens；
- 即便新增 token，也必须走 runtime projection 链，不允许直接在 `sftp-workspace-host.slint` 写裸 hex。

### 6. Bottom status bar

表格底部新增固定状态条，高度约 `28px-32px`。

内容分布：

- 左侧：`10 items`
- 中间：`1 selected`
- 右侧：`SFTP · Interserver · Ready`
- 最右轻量 transfer entry：如 `Transfers 3`

行为：

- transfer entry 点击打开全局 Transfer Center；
- 不把 transfer queue 嵌回表格主体。

### 7. 状态系统

#### Ready

- header 右侧显示小绿点 + `Ready`
- 表格显示真实 rows

#### Loading / Refreshing

- 如果已有快照：保留 rows，仅在 header 或表格上缘显示 subtle busy 状态
- 如果首次加载无快照：显示 loading state view

#### Error

- header 右侧显示 red dot + `Error`
- 主区出现 centered state view
- 提供 `Retry`

#### Disconnected

- centered state view 显示断连状态、last path、`Reconnect`
- 头部只保留轻量状态，不靠大 pill 传达全部信息

#### Read-only

- header 次级区域显示 lock badge
- 不把 read-only 做成一块新的主卡片

### 8. 右侧 Quick Browser 行为

当 active workspace tab kind 是 `SFTP`：

- 若 right panel 当前 view 为 `SFTP`，默认 `policy-hidden`
- layout 释放全部主区宽度
- 不显示 duplicate file list
- 不显示误导性 revive strip

当离开 SFTP workspace，回到 terminal tab：

- 若用户原先请求过 right panel，则恢复 quick browser 的可见性与宽度

这意味着系统需要区分：

- `user-requested visible`
- `user-collapsed`
- `policy-hidden because active workspace is SFTP`

### 9. 右键菜单与弹窗

workspace host 不新造 action path，继续使用现有 SFTP context menu dispatcher 与 modal 系统。

row context menu 最终应稳定包含：

- Open
- Open With
- Edit
- Download
- Upload Here
- New Folder
- New File
- Rename
- Delete
- Copy Path
- Refresh
- Properties

blank area context menu 至少包含：

- Upload Here
- New Folder
- New File
- Refresh

实现要求：

- 已有真实链路的 action 继续走 `PendingSftpContextAction`
- 本轮暂不具备真实 backend 的 action 必须 disabled / planned，而不是 fake success
- `SftpNewFolder` / `SftpRenameEntry` / `SftpDeleteEntriesConfirm` / `SftpRemoteFileModal` 继续复用现有 modal 架构，但视觉上要与 Ayu shell 对齐

### 10. Transfer Center 入口

- upload/download 继续进入全局 transfer queue
- workspace header 或 bottom status bar 提供轻量 transfer badge / entry
- 右侧 quick browser 被策略隐藏后，transfer center 仍可从 titlebar 与 workspace 内打开

## 架构变更建议

### 1. 新增 right panel display policy

当前只有：

- `show_right_panel`
- `effective_show_right_panel`

但还没有“为什么被隐藏”的语义。

本轮应新增一个明确的 policy projection，例如：

- `Visible`
- `UserCollapsed`
- `PolicyHiddenForSftpWorkspace`

实际落地不一定必须是这个枚举名，但必须让 Slint 和 layout 层能区分：

- 是否还能显示 revive strip
- 是否应该扣除右侧宽度
- 是否应该把 right panel tooltip 文案改成“当前 SFTP workspace 激活，右侧 quick browser 已自动收起”

### 2. 让 active summary 感知 live SFTP session

当前 `WorkspaceTab::sftp(...)` 仍然把 `host` 设为空、`connection_status` 固定成 `ready`。这足以打开 tab，但不足以支持更成熟的 titlebar / tooltip / policy summary。

本轮应让 active workspace summary 在 SFTP 场景下从真实 `FileBrowserSession` 读取：

- host label
- mode / status
- binding mode

从而避免 titlebar 继续展示静态或重复信息。

### 3. 扩展 `SftpPanelRenderRow` / `SftpPanelItem`

为了支持产品级 table，本轮需要在现有 render row 合同上补充：

- `icon_kind`
- `permissions_label`
- `owner_label`
- `group_label`
- `is_parent_row` 或等价字段

同时保留既有字段：

- `name`
- `type_label`
- `size_label`
- `modified_label`
- `selected`

这样可以继续复用现有 `workspace_sftp_render_rows` 与 quick browser 基础投影，而不是重新发明第二套 workspace-only row model。

### 4. workspace SFTP item sync 应避免无谓全量替换

`sync_workspace_sftp_state()` 当前每次都 `replace_sftp_panel_items_model(...)`。本轮建议让 workspace host 也走与 quick browser 一致的 dirty index / full resync contract，减少 selection、loading、refresh 过程中的 model 抖动。

这不是产品需求本身，但对成熟交互表现有帮助。

## 测试策略

### 结构 smoke

`tests/workspace_sftp_tab_contract_smoke.sh` 需要从旧的“类型和文件存在”升级为 UI 合同 smoke，至少锁定：

- compact header
- icon toolbar
- breadcrumb/path region
- file table
- bottom status bar
- 不存在 `New Folde`
- active workspace kind 是 SFTP 时，不再同时渲染右侧 duplicate file list

### source / projection tests

`tests/sftp_workspace_tab_render_spec.rs` 需要锁定：

- Ready 状态下显示真实 render rows
- selected row 走 subtle selected fill + accent rail 语义
- root path `/` 下 breadcrumb 结构稳定
- 窄宽度下隐藏 optional columns，不裁切 required columns

`tests/workspace_sftp_projection_spec.rs` 需要锁定：

- active workspace kind 为 SFTP 时 right panel display policy 正确
- workspace SFTP 使用独立 `FileBrowserSession` snapshot，而不是 quick browser 当前 session 引用

`tests/theme_semantic_token_contract_spec.rs` 需要锁定：

- `sftp-workspace-host.slint` 不硬编码独立 Ayu palette
- 选中态继续使用 semantic selected/focus/accent token

## 非目标

- 不在本轮引入 dual-pane local/remote 主布局
- 不在本轮重做右侧为常驻 inspector
- 不在本轮重构 SFTP 网络层
- 不在本轮新增完整文件预览系统
- 不在本轮把 transfer queue 嵌回 workspace 主表

## 实施建议

- 当前窗口仅写文档，不直接改功能代码。
- 真正实现应在新的 `.worktrees` 工作目录中执行。
- 实现时优先从测试和 projection 合同开始，再重写 `SftpWorkspaceHost` 结构，最后处理 right panel policy、modals、transfer entry 与整体验证。
