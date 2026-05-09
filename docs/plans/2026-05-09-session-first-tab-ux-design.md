# Session-First Tab UX Design

日期: 2026-05-09
执行者: Codex
状态: 已获用户确认，进入实现计划

## 目标

在不重构 SSH、wezterm-term、terminal rendering 主链路的前提下，把当前顶部 tab 条升级为真正的“会话管理条”。

本次只聚焦四件事：

- tab 拖拽排序
- tab 右键菜单与批量关闭/复制/重连/克隆行为
- active tab 完整信息在顶部 chrome/header 区的补偿显示
- tab 宽度、tooltip、active/inactive close affordance 与溢出策略

设计基线采用 `Session-first` 心智，参考 Termius / Tabby：tab 是会话容器，不是文档标签，也不是浏览器页面标签。

## 当前代码约束与结论

### 已有有利条件

- 当前 `active_workspace_tab_id` 已按稳定 id 工作，而不是纯数组 index，见 `src/shell/view_model/workspace.rs`。
- 当前 `Reconnect` 已有按 `session_id` 原地重试能力，见 `src/app/ssh/session_manager.rs` 的 `retry_session()`。
- 当前 close fallback 已有“右邻优先、再左邻”的基础行为，见 `src/shell/view_model/workspace.rs`。
- 当前项目已有轻量 tooltip、popup、overlay 和 Esc/点击外部关闭模式，可复用到 tab 菜单与 tooltip。

### 必须先规避的问题

- `workspace_tabs` 现在既承担 manager 投影，又承担 UI 展示；而 tab 会被 `SessionManager::ordered_sessions()` 周期性重投影，见 `src/app/bootstrap/workspace_terminal.rs`。
- 存在 `50ms` 的重复 session projection 刷新，见 `src/app/bootstrap.rs`；如果拖拽只改当前数组顺序，后续同步会把顺序冲回去。
- 当前 `WorkspaceTabItem.session_id` 实际上传的是 `tab_id`，命名与语义存在混淆，容易在拖拽、关闭、重连时误把 session 身份和 tab 身份混用。
- 当前 tab 元数据保真度不足，不能继续依赖 `title` / `subtitle` 的字符串解析来支撑 `Copy Host`、tooltip、顶部 active info。

结论：必须把“会话真相”和“tab 展示真相”拆开；tab 顺序只能作为 UI presentation state 存在，不能写回 session lifecycle 顺序。

## 设计原则

- `tab_id` 是 tab 层唯一主键，稳定且独立于数组 index。
- `session_id` 是当前 tab 绑定的真实会话身份；reconnect 修复原 tab，clone 派生新 tab。
- 拖拽排序只改变 UI 顺序，不改变 session 生命周期，不重建 terminal view，不切换 active session。
- 右键菜单锚定被点击 tab；打开菜单时不强制激活该 tab。
- `Close Left/Right/Others/All` 一律基于当前可见 tab 顺序，而不是 manager open order。
- 顶部完整信息在 titlebar / header 解决，不侵入 terminal 内容区。
- 单行 tab bar，不换行，不改变顶部整体高度，避免影响 terminal rows/cols。

## 数据模型设计

### WorkspaceTab 扩展

`src/shell/tabs.rs` 中的 `WorkspaceTab` 扩展为面向会话容器的保真模型：

- `tab_id`
- `session_id`
- `asset_id`
- `kind`
- `display_name`
- `host`
- `username`
- `port`
- `connection_status`
- `enhanced_session_state`
- `error_detail`
- `connection_profile_ref` 或最小可重连快照
- `active`

补充说明：

- `display_name` 是 tab 主显示名，供 tab 文本、Copy Name、顶部主标题使用。
- `host` / `username` / `port` 是结构化字段，供 tooltip、Copy Host、顶部 summary、Reconnect/Clone 目标判定使用。
- `connection_profile_ref` 不要求复制 transport 状态，但要求保留“该 tab 启动时使用的连接配置语义”，避免 reconnect 静默变成“按资产最新值重新打开”。

### ViewModel 新状态

在 `ShellViewModel` 中新增 tab presentation state：

- `workspace_tab_order: Vec<String>`，按 `tab_id` 存当前 UI 顺序
- `workspace_tab_context_menu_state`
- `workspace_tab_drag_state`
- 必要的 tab tooltip / hover state

说明：

- `workspace_tab_order` 只负责 UI 顺序，不负责 session 生命周期。
- manager 投影 merge 到现有 tabs 后，最终渲染顺序由 `workspace_tab_order` 决定。
- 新 tab 没有顺序记录时追加到尾部。

## 交互设计

### 顶部 active tab 完整信息

位置：`MICA TERM` Logo 右侧，位于 titlebar header 区。

显示格式：

- 只有名称：`Interserver(7)`
- 有 host：`Interserver(7) · 172.22.0.2`
- 有状态：`Interserver(7) · 172.22.0.2 · Connected`

规则：

- 这是弱强调信息，优先级低于品牌、高于 tooltip。
- 窗口过窄时先压缩 host / status，再压缩名称，优先保留名称可读性。
- hover 这块区域时显示完整 tooltip。

### Tab hover tooltip

Tooltip 内容固定为：

- tab name
- host
- username
- port
- connection status

规则：

- tab 文本截断时必须可补偿。
- tooltip 只做补全，不承担主信息承载职责。
- 拖拽中、菜单打开时不弹 tooltip，避免闪烁和误导。

### 右键菜单

菜单项：

- Reconnect
- Clone Connection
- Close
- Copy Name
- Copy Host
- Close Others
- Close All
- Close Tabs to the Right
- Close Tabs to the Left

菜单规则：

- 弹出位置跟随鼠标锚点。
- 点击外部关闭、Esc 关闭。
- 打开时不强制切 active tab。
- 菜单项顺序固定，不因状态变化重排；不可用项使用禁用态，不隐藏。

禁用规则：

- `Reconnect`：仅 `cancelled / disconnected / error`
- `Copy Host`：host 为空时禁用
- `Close Others`：只有一个 tab 时禁用
- `Close Tabs to the Left`：左侧没有 tab 时禁用
- `Close Tabs to the Right`：右侧没有 tab 时禁用
- `Clone Connection`：没有可用连接配置时禁用

### 拖拽排序

触发规则：

- 左键按下后，只有当横向位移超过合理阈值，且明显大于纵向位移时，才进入拖拽
- 阈值内释放，仍视为普通点击切换

视觉反馈：

- 被拖动 tab 轻微上浮、略降透明度
- 插入目标位置显示细亮线
- 不做大面积强动画，不做 game-like 效果

行为边界：

- 拖拽只改 UI 顺序
- 不重建 tab
- 不重建 terminal surface
- 不切 active session
- drop 后一次性提交排序变更

### Tab 宽度与溢出策略

单行 tab strip，不换行。

建议：

- inactive tab：最小宽度约 `104-112px`，最大宽度约 `180px`
- active tab：最小宽度约 `128-140px`，最大宽度约 `220-240px`
- active tab 比 inactive tab 保留更多可读信息
- inactive close 按钮 hover 显示；active close 按钮常显
- 文本一律 ellipsis；完整信息靠 tooltip 和顶部 summary 补足

本期不做：

- 多行换行 tab
- 跨窗口拖拽拆分 tab
- browser-style overflow dropdown
- pin tab

## 行为语义

### Close

- 锚定 tab：关闭该 tab 对应会话 / 工作区
- 若关闭的是 active tab：右邻优先，再左邻；无邻居则进入 empty state
- 若关闭的是 inactive tab：active tab 保持不变

### Close Others / Left / Right / All

- 目标集合先按当前 UI 顺序冻结，再执行关闭
- 范围只作用于当前 window
- 不按 session open order 计算
- 关闭逻辑必须继续走统一 tab close 编排入口，不能直接跳过现有 SFTP / hidden terminal 处理链

### Reconnect

- manager-backed terminal tab：优先原地 `retry_session(session_id)`
- failure/synthetic tab：按保存的连接快照重建连接，但复用原 `tab_id`
- 不新增 tab，不改变该 tab 的 UI 位置，不转换为“新开一个会话”

### Clone Connection

- 语义是基于当前连接配置派生一个新会话
- 优先新窗口
- 如果当前没有清晰的新窗口创建 API，则保留能力检测与扩展点，不硬编码伪多窗口方案
- 绝不能退化为 reconnect，也不能退化为 activate existing

### Copy Name / Copy Host

- 只允许显式用户动作触发
- `Copy Name` 复制 display name
- `Copy Host` 只复制结构化 host 字段，不复制 `user@host:port`
- 复制结果只写入本地系统剪贴板，不进入日志、toast payload 或 action id

### Empty state

- zero-tab 是合法状态
- `Close All` 后进入 empty state
- 不自动补一个伪 launcher tab 伪装成仍有 tab
- `+` 才是显式进入 New Tab / Launcher 的入口

## 视觉语言

- 保持 Windows 11 Fluent / Mica 风格
- tab bar 仍然克制、现代、轻量
- active tab 用微亮 surface + 细 accent line
- 连接状态色只用于小面积状态条 / 点，不做大面积染色
- 菜单与 tooltip 统一使用轻量半透明面板、细边框、圆角、轻阴影

## 专家辩论后的收敛结论

### 接受的共识

- 不能把拖拽排序写回 `SessionManager.open_order`
- 不能用 index 作为 session / tab 身份
- `Reconnect != Clone`
- `Close Left/Right` 必须按当前 UI 顺序
- tab 菜单不能直接共用 assets context menu 的状态机

### 明确拒绝的方向

- Chrome 式高密度压缩 tab 作为主方案
- 多行 wrap tab bar
- 用“新建 tab 再关旧 tab”模拟 reconnect
- 通过解析 `title` / `subtitle` 推导结构化 host 信息
- 用 `asset_id` 代替 `session_id` 作为 tab 实体身份

## 已知限制

- 本期不实现跨窗口拖拽和 tab detach
- `Clone Connection` 是否立即可用，取决于是否补出明确的新窗口创建入口
- 本期优先保证会话身份稳定与操作语义正确，不扩展到 pin / group / overflow search

## 验收标准映射

本设计直接覆盖以下验收点：

- tab 截断后，顶部 header 仍可显示 active tab 完整信息
- hover 任意 tab 或顶部 active summary 可见完整 tooltip
- 右键任意 tab 可打开菜单，位置正确，Esc / 外部点击关闭
- Copy Name / Copy Host 各自复制正确内容
- Reconnect 原地重连，不新建 tab
- Close Others / Left / Right / All 只作用于正确范围
- Close All 后进入合法 empty state
- 拖拽排序后 SSH 会话不中断，active session 不漂移
- 不影响 terminal renderer、字体、行高、颜色、光标和 selection
