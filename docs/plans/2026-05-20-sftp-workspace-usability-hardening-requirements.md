# SFTP Workspace Usability Hardening Requirements

日期: 2026-05-20
执行者: Codex
状态: 已整理，待在新 worktree 中执行

## 目标

在不重做 SFTP backend、也不把 MicaTerm 变成 WinSCP/FileZilla 的前提下，把当前已经落到 `master` 的真实 `SFTP Workspace Tab` 从“功能已接通但交互契约未收口”的状态，提升为真实可用、可滚动、可导航、可右键、可断线重连、可继续产品化演进的主工作区。

本轮文档只定义 requirements / design / tasks，不直接修改功能代码。正式实现应在新的 `.worktrees` 工作目录中进行。

## 当前真实基线

- 用户提供的历史基线 `79dc68d0c4bfe8c7d9bbacb0fa9f719aa4d374df` 已经过时。
- 当前仓库真实 `HEAD` 为 `46fc0bf (HEAD -> master, origin/master) Merge branch 'feat/sftp-workspace-productized-ui'`。
- 这意味着：本轮不是从“刚实现真实 workspace tabs”的最早版本起步，而是基于已经 merge 的 productized UI 分支继续做可用性与契约收口。
- 当前以下验证命令都能通过，但用户仍报告明显回归：
  - `bash tests/workspace_sftp_tab_contract_smoke.sh`
  - `cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test sftp_context_menu_spec -q`
  - `cargo test --test theme_semantic_token_contract_spec --test bootstrap_smoke -q`
- 因此本轮 requirements 必须明确：现有测试存在“假绿 / 过度静态 / 未覆盖真实用户路径”的问题，不能把“当前测试全绿”当作“当前体验已达标”。

## 输入文档

必须继续遵守并在此基础上收口：

- `docs/plans/2026-04-15-sftp-two-layer-workspace-design.md`
- `docs/plans/2026-04-15-sftp-two-layer-workspace-implementation-plan.md`
- `docs/plans/2026-03-31-sftp-browser-transfer-center-design.md`
- `docs/plans/2026-03-31-sftp-browser-transfer-center-implementation-plan.md`
- `docs/plans/2026-04-15-sftp-quick-browser-polish-design.md`
- `docs/plans/2026-04-15-sftp-quick-browser-polish-implementation-plan.md`
- `docs/plans/2026-05-12-ayu-terminal-neighborhood-design.md`
- `docs/plans/2026-05-14-ayu-refinement-design.md`
- `docs/plans/2026-05-19-sftp-workspace-productized-ui-design.md`
- `docs/plans/2026-05-19-sftp-workspace-productized-ui-implementation-plan.md`

## 产品定位与边界

### 必须保持的定位

- MicaTerm 是 terminal-first SSH 产品，不是 standalone 文件管理器。
- 右侧 `Quick Browser` 仍是轻量入口，不是唯一主文件视图。
- 中部 `SFTP Workspace` 是完整文件工作区，承担成熟文件浏览与文件操作。
- transfer center 仍然是跨 quick browser / workspace 共用的真实后台传输体系。

### 明确非目标

- 不重做 SFTP backend。
- 不使用 fake rows、硬编码 demo 数据、只隐藏 bug 的 workaround。
- 不在 Slint 内再造第二套 Ayu palette。
- 不新增 WinSCP/FileZilla 式 dual-pane local/remote 主视图。
- 不自动因 reconnect 新开 terminal tab。

## 核心成功标准

主区 SFTP Workspace 达标的定义是：

1. `/` 目录能从第一行看到 `home` 等前序项，并能稳定滚动到最后一项。
2. path bar 默认像文件管理器，不需要先点 pencil 才能编辑路径。
3. breadcrumb、Ctrl+L、Enter、Esc 都形成闭环状态机，而不是 submit-only 假编辑。
4. 所有 toolbar action 都有 tooltip；disabled 时也能说明为什么不可用。
5. row context menu、blank-area context menu、顶部按钮都走 SFTP 文件操作上下文，不再串到 Assets。
6. disconnected / loading / empty / error 都是内容区状态，不是错位 modal/card overlay。
7. 整体视觉贴近 20260518/7a832808 的 Ayu/MicaTerm 方向，但仍保持真实 MicaTerm flat/compact shell 语言。

## 行为需求

## 1. Toolbar tooltip 与 disabled reason

### 1.1 必须存在的 tooltip

SFTP workspace toolbar 至少要覆盖以下文案：

- Back: `Back`
- Forward: `Forward`
- Up: `Up one level`
- Home: `Go to home directory`
- Refresh: `Refresh remote directory`
- Edit Path / Path Field: `Edit path`
- Upload: `Upload files or folders`
- New Folder: `Create folder`
- Transfer Center: `Open Transfer Center`

### 1.2 tooltip 体系约束

- 必须复用项目现有 tooltip / popup 模式，优先沿用 `titlebar`、`tabbar`、`right-panel` 已存在的全局 tooltip overlay 路径。
- 禁止在 `ui/shell/sftp-workspace-host.slint` 单独发明第二套 tooltip 样式、第二套锚点调度器或第二套 timing 逻辑。
- tooltip 不只是字符串存在；必须真的能在运行时 hover/focus 显示。

### 1.3 disabled reason 约束

- disabled button 依然必须能 hover/focus 显示 tooltip。
- 至少要支持 disconnected 场景的解释性文案，例如 `Reconnect to browse files`。
- `enabled: false` 只能关闭点击，不得顺带关闭 tooltip 解释能力。

## 2. Browser-like path bar 与 breadcrumb

### 2.1 默认形态

- path bar 默认显示 breadcrumb segments，而不是默认常驻纯文本输入框。
- `/home/wwwroot` 的显示语义为 `/ > home > wwwroot`。
- `/` 根节点必须可点击返回根目录。

### 2.2 进入编辑模式

以下任一方式都必须进入 path text edit：

- 单击 path shell 的空白区域
- 单击 path 文本本身
- 单击 pencil / edit affordance
- `Ctrl+L`

### 2.3 编辑行为

- 进入编辑后，input 必须获得焦点，且默认全选当前路径。
- `Enter` 提交当前输入并导航。
- `Esc` 取消编辑并恢复当前真实路径显示。
- `Esc` 不得伪装成“再次提交当前 path”。
- 空输入不导航。
- 多余斜杠需要归一化。
- 如果暂不支持相对路径，必须显式拒绝并给出确定反馈，而不是静默失败。

### 2.4 breadcrumb 导航

- 点击 `home` 导航到 `/home`。
- 点击 `wwwroot` 导航到 `/home/wwwroot`。
- 当前 segment 仍可点击，用于 re-enter 同一路径或保持一致交互。
- breadcrumb 至少要鼠标可达；如本轮能低成本补齐，也应支持键盘 focus/activate。

### 2.5 状态真源约束

- 所有 path navigation 必须走现有 `SftpBrowserController` / `browser_session` / `session_binding` 路径。
- 不允许 UI 直接本地改 path 文本来伪造“已导航”。

## 3. 文件列表滚动、viewport 与虚拟列表契约

### 3.1 基本可用性

- 文件列表必须能从第一行滚动到最后一行。
- `/` 下要能看到 `home`。
- path 改变、refresh 完成、expand workspace 后，viewport 必须 reset 到 top。
- tab 切换时应恢复该 workspace tab 自己最后一次稳定 viewport，但不能覆盖上述 reset 事件。

### 3.2 单一度量来源

- row height 必须只有一个真源，不能再出现 Rust 虚拟列表按 `44px` 计算、Slint 行实际 `40px` 渲染的分叉。
- `total content height`、`top spacer`、`bottom spacer`、`window_start_row`、`window_end_row` 必须基于同一行高契约。

### 3.3 ScrollView / viewport contract

- 如果继续使用 virtualization：
  - `ScrollView.viewport-height` 必须使用总内容高度。
  - top/bottom spacer 必须真实参与 layout。
  - `viewport-y` 改变必须可靠回传 Rust state。
  - Rust state 改变也必须能受控地下发回 UI，而不是 host 私有状态漂移。
- 必须考虑 Slint `viewport-y` 通常是负值这一事实，避免符号使用错误。

### 3.4 选择与滚动关系

- 选择行不应把列表错误跳到中间。
- refresh 之后的 reset-to-top 必须是确定性的，而不是偶发保留旧 viewport。

### 3.5 fallback 策略

- 如果当前 virtualization 在修复窗口内无法稳定，允许先落一个 bounded full-list fallback 以恢复可用性。
- 但 fallback 必须保留测试边界，明确大目录不能无上限卡死，并为后续恢复 virtualization 留出 contract。

## 4. Toolbar action 区宽度与 `New Folder` 不截断

### 4.1 文本不截断

- 正常宽度下，`New Folder` 必须完整显示。
- 不允许出现 `New Fold...`、`New Folde`、`New Fol...` 这类半残文案。

### 4.2 响应式策略

- 宽度充足时：`Upload` / `New Folder` / `Transfer Center` 可以显示 icon + text。
- 窄宽度时：可以退化为 icon-only，但 tooltip 必须保留完整语义。
- path bar 不得无限扩张并把右侧 action 挤出界面。

### 4.3 overflow policy

- action 区需要明确 min-width 与 width-tier 策略。
- toolbar 不应换行。
- 不允许为了避免截断而把按钮文案裁进看不懂的省略态。

## 5. SFTP workspace context menu 必须 surface-aware

### 5.1 row context menu

文件/文件夹 row 右键至少包含：

- Open
- Open With...
- Edit
- Download
- Upload Here
- New Folder
- New File
- Rename
- Delete
- Copy Path
- Properties
- Refresh

### 5.2 blank-area context menu

空白区域右键至少包含：

- Upload Here
- New Folder
- New File
- Paste / Paste Upload
- Refresh
- Copy Current Path

### 5.3 禁止出现的错误项

workspace SFTP context menu 中禁止出现：

- `New SSH Connection`
- Assets-specific menu item
- Console asset actions
- 只对 quick browser 成立的 workspace-expand 语义

### 5.4 surface-aware 约束

- context menu resolver / dispatcher 必须区分：
  - `AssetsSidebar`
  - `SftpQuickBrowser`
  - `SftpWorkspace`
- `copy-current-path`、`selected_ids`、`target_mutable`、`enabled/disabled` 必须来自 active workspace SFTP session，而不是 quick browser 泄漏值。

### 5.5 close behavior

以下事件必须先关闭 Assets create popup，再继续当前交互路由：

- 点击 workspace
- 右键 workspace
- 切换 workspace tab
- 关闭 workspace tab
- 切换到其他侧栏 surface

不得再依赖“全屏 dismiss layer 挡住事件，顺便关掉”的偶然行为。

## 6. Disconnected / Loading / Empty / Error 必须是内容区状态

### 6.1 disconnected 不是 modal

disconnected 必须表现为 `SftpWorkspaceHost` 文件列表区域里的 centered content state，而不是绝对定位 overlay-card / modal-like 小盒子。

### 6.2 disconnected content

内容区中心必须能显示：

- icon: disconnected / link-off
- title: `Disconnected`
- body: `The linked SSH or SFTP transport is offline.`
- secondary: `Last path: /...`
- primary button: `Reconnect`

### 6.3 toolbar / header 保留

- header、toolbar、status bar 仍保留。
- 文件操作禁用。
- `Reconnect` 仍然可用，并走真实 reconnect path。

### 6.4 统一状态体系

loading / connecting / empty directory / disconnected / error 必须使用同一类 content-state surface 语言。

## 7. Ayu / MicaTerm 产品化 polish

### 7.1 header

- 主标题显示当前 host label。
- host label 超长时 ellipsis，并在 tooltip 显示完整值。
- titlebar / tab / workspace header 不应三次重复同一长名字。

### 7.2 table

- `Name` 列带图标。
- 至少支持 `Type / Size / Modified / Permissions / Owner / Group` 的可投影列模型。
- 窄宽度下允许隐藏 optional columns，但不能裁坏必需列。
- selected row 使用 subtle fill + accent rail，而不是硬边框大橙盒。
- hover / active / focus 状态要清楚但克制。

### 7.3 status bar

底部状态条必须至少保留并增强：

- `Ready / Disconnected`
- item count
- selected count
- current path
- `Locked / Manual`
- Transfer Center badge / count

### 7.4 主题真源

- 所有颜色继续使用 runtime-projected Ayu semantic tokens。
- 禁止在 `ui/shell/sftp-workspace-host.slint` 直接硬编码第二套 Ayu 色阶。

## 自动化测试需求

## 1. 总体原则

- 先写失败测试，再做实现。
- 对滚动、viewport、context menu 串路由、overlay 错位这类问题，必须先做 root-cause 调查，再编码。
- 不能只新增 grep-style 静态 smoke；必须把用户真实操作路径转成行为 contract。

## 2. 需要被纠正的假绿问题

当前测试不足主要体现在：

- 只检查字符串存在，不检查 tooltip 是否真的走全局 overlay。
- 只检查 source 里有 callback 名字，不检查 `Ctrl+L` / `Esc` / `Enter` 真正如何路由。
- 只检查 action tree 片段，不检查 workspace 右键是否仍通过 assets overlay / quick-browser-only accessor。
- 只检查 “workspace-style” 文案，却没有建立真实 workspace SFTP fixture。
- 只检查 theme token 名称存在，不检查是否仍然夹带 raw hex / 第二套 palette。

## 3. 必须加强的测试文件

### `tests/workspace_sftp_tab_contract_smoke.sh`

至少要锁住：

- toolbar action tooltip contract 存在且接到共享 tooltip 模式
- 不存在 `New Folde` / `New Fold...`
- SFTP workspace context menu 不包含 `New SSH Connection`
- disconnected state 不再以 `modal` / `overlay-card` 形式建模

### `tests/sftp_workspace_tab_render_spec.rs`

至少要锁住：

- root path `/` 渲染时 viewport reset 到 top
- 26 items 可滚动访问，且首项覆盖 `home`
- breadcrumb 正确显示 `/ > home > wwwroot`
- clicking breadcrumb segment 导航到正确 path
- `Ctrl+L` 进入 path editing
- `Enter` 提交 path
- `Esc` 取消 path editing

### `tests/workspace_sftp_projection_spec.rs`

至少要锁住：

- `workspace_sftp_render_rows()` 随 viewport 改变
- path change / refresh / expand resets viewport top
- selected row 不应导致 viewport 错跳
- total row count 与 visible row slice 分离

### `tests/sftp_context_menu_spec.rs`

至少要锁住：

- workspace row context menu 使用 SFTP file actions
- workspace blank context menu 使用 SFTP directory actions
- workspace context menu 排除 Assets actions
- assets create popup 打开时，右键 workspace 会先关闭 popup 再打开 SFTP menu
- `copy-current-path` 读取 workspace path，不读取 quick browser path

### `tests/theme_semantic_token_contract_spec.rs`

至少要锁住：

- SFTP workspace 继续走 runtime Ayu semantic tokens
- selected row 使用 subtle fill + accent rail 语义
- 不引入第二套硬编码 Ayu palette

### `tests/bootstrap_smoke.rs`

至少要锁住：

- disconnected workspace `Reconnect` callback 发到真实 reconnect path
- path edit focus / cancel / submit 回调链可达
- workspace context menu request 的 bootstrap 路由会同步关闭 create popup 并同步 transient state

## 人工验收场景

以下 10 条人工场景必须在后续实现完成时全部复现通过：

1. 打开 Interserver，进入 SFTP workspace。
2. 顶部所有图标 hover 都显示 tooltip。
3. 点击 path，直接可编辑；`Ctrl+L` 可编辑；`Enter` 跳转；`Esc` 取消。
4. `/home/wwwroot` breadcrumb 中点击 `home` 可跳 `/home`。
5. `/` 下文件列表从第一项显示，可以滚动到最后一项；能看到 `home`。
6. `New Folder` 完整显示，窄宽度下变 icon-only + tooltip，不截断。
7. 在文件 row 右键，菜单是 SFTP file menu；不出现 `New SSH Connection`。
8. 在空白处右键，菜单是 SFTP blank menu。
9. 断线后显示居中的 SFTP content empty state，不是错位 modal；`Reconnect` 可用。
10. 整体视觉接近 20260518/7a832808 的 Ayu shell 方向，但保持真实 MicaTerm 布局。

## 交付门禁

只有同时满足以下条件，后续实现轮才可以宣称完成：

- 强化后的新增测试先红后绿。
- `bash tests/workspace_sftp_tab_contract_smoke.sh` 通过。
- `cargo test --test sftp_workspace_tab_render_spec --test workspace_sftp_projection_spec --test sftp_context_menu_spec -q` 通过。
- `cargo test --test theme_semantic_token_contract_spec --test bootstrap_smoke -q` 通过。
- `cargo check --workspace` 通过。
- 若本轮触及 clippy 敏感逻辑，再跑 `cargo clippy --workspace -- -D warnings`。
- 人工验收 10 条全部可复现通过。

