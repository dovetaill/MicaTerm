# SFTP Browser And Transfer Center Redesign

日期: 2026-03-31
执行者: Codex
状态: 方案已确认，待进入实现

## 背景

当前右侧 `SFTP` 面板同时存在产品和实现两层问题：

- 右侧面板默认落到无价值的 `Appearance` 遗留分支，已经偏离产品目标。
- 当前 `SFTP` 右栏把 session 信息、工具按钮、文件列表、传输队列全部塞进 392px 窄区域，视觉和交互都过于拥挤。
- 顶部 `Sharon / Connecting SFTP channel` 信息卡只占空间，不提供高价值操作。
- `Back / Next / Up / Sync / Follow` 文字按钮对窄面板不友好，且与 Fluent 体系不一致。
- 传输队列本质上是全局状态，不应该常驻嵌在右侧文件浏览区。
- 最关键的是当前 `SFTP` 功能链路未闭环：
  - 面板会进入 `Connecting/Loading`
  - 但没有统一、稳定的目录加载入口
  - 切换 SSH session 后，右侧 `SFTP` 也不会正确跟随

用户目标明确：

- 右侧 panel 只承担 `SFTP` 远程文件浏览。
- 路径栏采用 Windows 文件管理器式的可编辑 breadcrumb/location bar。
- 文件列表改为可横向滚动的多列视图。
- 传输队列从右侧移出，改为顶部状态栏入口 + 独立页面。
- 交互尽量简单，减少用户心智负担。

## 外部参考

本设计参考了 2026-03-31 可访问的官方或成熟产品资料：

- `Microsoft Learn: Breadcrumb Bar`
  - 明确说明 breadcrumb bar 常用于文件管理器场景，适合持续暴露当前位置并支持快速回跳。
  - 链接: <https://learn.microsoft.com/en-us/windows/apps/design/controls/breadcrumbbar>
- `GNOME Help: Show the directory path with text rather than buttons`
  - 官方说明文件管理器路径栏可以在按钮路径和文本输入之间切换，符合“breadcrumb + 可编辑路径”混合模型。
  - 链接: <https://help.gnome.org/users/gnome-help/gnome-help/files-show-directory-path.html>
- `WinSCP: Main Window (Commander Interface)`
  - 文件面板由工具栏、路径显示和文件列表构成，符合“浏览区专注于浏览”的模型。
  - 链接: <https://winscp.net/eng/docs/ui_commander>
- `WinSCP: Background Operations Queue List`
  - 队列是独立的可选组件，而不是常驻嵌入文件浏览面板。
  - 链接: <https://winscp.net/eng/docs/ui_queue>
- `FileZilla Client Tutorial`
  - 文件浏览区和传输队列分区明确，队列位于底部独立区域。
  - 链接: <https://wiki.filezilla-project.org/FileZilla_Client_Tutorial_(en)>
- `Cyberduck Browser / File Transfers`
  - 传输状态单独进入 transfer 视图/窗口，不挤压主浏览区。
  - 链接: <https://docs.cyberduck.io/cyberduck/browser/>
  - 链接: <https://docs.cyberduck.io/cyberduck/download/>

结论：

- 右侧 `SFTP` 面板应回归“文件浏览器”本职。
- 路径导航应允许 breadcrumb 与文本输入切换。
- 传输队列应成为全局视图，而非窄面板中的附属卡片。

## 目标

### 本轮目标

- 删除右侧 `SFTP` 面板内的无效信息卡和嵌入式传输队列。
- 将右侧 `SFTP` 面板重构为：
  - 单行工具条
  - Windows 文件管理器式地址栏
  - 多列文件表格
- 新增顶部状态栏 `Transfer` 入口，打开独立的“文件传输”页面。
- 用新的 `SftpBrowserController` 重构 SFTP 浏览状态流，修复：
  - 打开后卡在 `Connecting SFTP channel`
  - 切换 SSH session 后不跟随
  - follow cwd / manual browse 混乱
  - 旧请求回写污染新 session UI

### 体验目标

- 右侧面板只做一件事：浏览当前 SSH session 的远程文件。
- 用户始终知道自己在哪个远程路径。
- 常用动作显著，高频路径导航不需要理解额外状态机。
- 传输相关信息从右栏挪走，主浏览区更清晰。

## 非目标

本轮不包含：

- 新增复杂的双栏本地/远程 commander 模式
- 在首轮实现中加入列排序、拖拽列宽、收藏夹、预览面板
- 一次性重做所有 `SFTP` 上下文菜单能力
- 引入新的全局导航体系
- 将传输中心做成独立窗口

## 当前实现问题分析

### 1. SFTP 目录加载链路没有闭环

当前代码中已经存在 `SessionManager::sftp_read_dir(session_id, path)` 和底层 `RusshSftpBackend::read_dir(...)`，但右侧面板交互路径没有统一收敛到这一调用。

现状表现：

- 打开面板会把状态推进到 `connecting/loading`
- `path submit / refresh / retry` 也会推进状态
- 但没有一个统一入口负责“发起真实目录加载 -> 回写 entries -> 切换到 ready/error”

因此当前 UI 很容易停留在“连接中”的假状态。

### 2. SFTP binding 只有连接态/断开态，没有浏览完成态

`SessionManager` 中的 `SftpSessionBinding` 主要表达：

- `Connecting`
- `Disconnected`

它并不承担“某个路径已经成功读取完成”的浏览状态投影，因此浏览态被卡在 `bootstrap + ShellViewModel` 之间，责任边界不清。

### 3. 切换 workspace tab 时，SFTP 右栏没有完整跟随

`workspace_tab_selected` 会更新 active session，但右侧 `SFTP` 没有自己的 controller 去处理：

- 新 active session 初次打开
- follow cwd 触发
- 旧请求过期
- 新 session 目录重新加载

结果就是 SSH session 切换了，右侧浏览区仍然停在旧状态或空状态。

### 4. UI 信息架构错误

当前右侧面板同时塞入：

- session 信息卡
- 文本按钮工具条
- 路径输入框
- 文件列表
- transfer queue 卡片
- queue drawer

这在 392px 宽度内会直接挤压文件列表，破坏主任务。

## 方案比较

### 方案 A：在现有 bootstrap 路径上增补真实目录加载

优点：

- 改动较小
- 可以更快修复“打不开”

缺点：

- `bootstrap` 已经过重，再把浏览加载逻辑堆进去只会继续恶化
- 不能从根上解决 `SFTP` 状态职责混乱
- 后续扩展多列列表、请求去重、旧请求丢弃时会更难维护

结论：

- 不采用

### 方案 B：引入 `SftpBrowserController`，把浏览状态流独立出来

优点：

- 结构更清晰：UI callback 只发事件，controller 负责读取与状态推进
- 能自然支持 request token、session 切换、follow cwd、retry 等场景
- 便于把传输中心与浏览中心解耦

缺点：

- 一次性重构面更大

结论：

- 采用

## 最终设计

## 设计要点 1：右侧 SFTP 面板只承担浏览职责

右侧 `SFTP` 面板重新定义为：

- 单行工具条
- 可编辑 breadcrumb 地址栏
- 多列文件表格
- 轻量状态条

明确删除：

- session-strip
- queue-strip
- queue-drawer
- 大块空态卡片

## 设计要点 2：单行工具条

顶部采用单行布局：

`[Back] [Forward] [Up]  [Breadcrumb / Editable Path]  [Refresh] [+]`

说明：

- `Back / Forward / Up` 为常驻高频动作。
- `Refresh` 常驻。
- `+` 下拉收纳次级动作：
  - `Upload File`
  - `Upload Folder`
  - `New Folder`
- `Follow terminal` 不做常驻大按钮：
  - 当用户处于 `manual browse` 时，以轻量入口出现在地址栏侧边或下拉中。

## 设计要点 3：Windows 文件管理器式地址栏

地址栏默认显示 breadcrumb：

`/ > root > .ssh > known_hosts`

交互规则：

- 点击 path 节点：跳转到对应上级路径
- 点击空白处或显式进入编辑态：切换为文本输入
- `Enter`：提交并加载目标路径
- `Esc`：退出编辑态，回到 breadcrumb 显示

这样既保留“可见路径层级”，又支持熟悉路径的用户快速输入。

## 设计要点 4：多列文件表格

主列表改为多列：

- `Name`
- `Modified`
- `Size`

首轮约束：

- 固定列宽
- 支持横向滚动
- 行高紧凑
- 双击目录进入
- 右键唤起上下文菜单

不采用当前卡片式二行列表，因为它在窄面板内信息密度过低。

## 设计要点 5：Transfer Center 独立化

顶部状态栏新增 `Transfer` 图标入口：

- 展示全局 queue badge
- 点击后切到独立的“文件传输”页面

页面结构参考 HexHub / FileZilla / WinSCP 的成熟做法：

- 顶部分类 tab：
  - `Running`
  - `Queued`
  - `Paused`
  - `Failed`
  - `Completed`
- 下方为表格
- 空数据时展示轻量空态

`SFTP` 浏览区不再显示传输队列摘要。

## 设计要点 6：SftpBrowserController

新增 `SftpBrowserController`，按 `session_id` 管理每个 SSH session 的浏览状态。

职责：

- 处理 `Open`
- 处理 `SessionActivated`
- 处理 `FollowCwd(path)`
- 处理 `Navigate(path)`
- 处理 `Refresh`
- 处理 `Retry`
- 生成 request token，丢弃过期结果

`bootstrap` 只负责绑定 UI callback，不再直接处理 SFTP 浏览逻辑。

## 设计要点 7：SFTP 浏览状态机

每个 session 对应一个 `SftpBrowserSessionState`：

- `mode`: `Idle | Connecting | Loading | Ready | Error | Disconnected`
- `follow_mode`: `FollowCwd | ManualBrowse`
- `current_path`
- `entries`
- `selected_entry_ids`
- `history`
- `last_error`
- `active_request_id`

状态流转：

- `Idle -> Connecting -> Loading -> Ready`
- `Loading -> Error`
- 任意态在 SSH runtime 断开时 -> `Disconnected`
- `Disconnected -> Connecting` 仅允许由 `Retry` 或 session 重建触发

## 设计要点 8：统一目录加载入口

所有以下事件最终都要收敛到一次真实目录读取：

- 打开 `SFTP`
- workspace tab 切换
- breadcrumb 跳转
- 文本路径提交
- refresh
- retry
- follow cwd 触发

统一入口负责：

- 计算目标 `session_id + path`
- 生成 `request_id`
- 调用 `SessionManager::sftp_read_dir(...)`
- 根据结果推进 `Ready/Error/Disconnected`
- 仅在 `request_id` 仍是当前活动请求时才回写 UI

## 设计要点 9：follow cwd 规则

规则固定为：

- 默认 `FollowCwd`
- 当终端 cwd 变化且用户仍处于跟随模式时，自动刷新浏览区
- 一旦用户手动跳路径或进入目录，切到 `ManualBrowse`
- `ManualBrowse` 下不再被终端 cwd 强制覆盖
- 用户显式点 `Follow terminal` 才恢复跟随

## 设计要点 10：错误与断开反馈

错误反馈改为列表上方轻量状态条：

- `Connecting`
- `Loading`
- `Error`
- `Disconnected`

行为：

- `Error`：保留当前路径和最近浏览上下文，可直接 `Retry`
- `Disconnected`：提示 reconnect 或 retry session
- 不再用大块空白卡片覆盖整个浏览区

## 代码改动范围

### 新增

- `src/app/sftp/browser_controller.rs`
- `src/app/sftp/browser_state.rs`
- `ui/shell/transfer-center.slint`

### 重构

- `src/app/bootstrap.rs`
- `src/shell/view_model.rs`
- `src/app/sftp/mod.rs`
- `ui/shell/right-panel.slint`
- `ui/shell/titlebar.slint`
- `ui/app-window.slint`

### 测试

- `tests/sftp_follow_cwd_spec.rs`
- `tests/sftp_right_panel_render_spec.rs`
- `tests/top_status_bar_smoke.rs`
- 新增 transfer center 相关 smoke/render/spec 测试

## 风险与缓解

### 风险 1：旧请求回写新 session

缓解：

- request token
- 只接受最新活动请求的回调

### 风险 2：tab 切换与 cwd 跟随相互覆盖

缓解：

- follow cwd 只在 `FollowCwd` 模式下生效
- `SessionActivated` 和 `FollowCwd` 共用同一控制器入口

### 风险 3：bootstrap 重构引入行为回归

缓解：

- 先用测试冻结旧 callback contract 中仍需保留的部分
- 按任务拆分逐步迁移

## 验证策略

- 新增 controller 级单元测试，覆盖：
  - open / navigate / refresh / retry
  - request token 丢弃旧结果
  - follow cwd / manual browse 切换
- 维持并扩展 UI smoke/render 测试，覆盖：
  - 单行工具条
  - breadcrumb / editable path
  - 多列文件表格
  - transfer icon badge
  - transfer center 页面
- 全量验证：
  - `cargo test --workspace`
  - `cargo check --workspace`
  - `cargo clippy --workspace -- -D warnings`

