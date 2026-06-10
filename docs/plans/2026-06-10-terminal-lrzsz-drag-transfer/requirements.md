# Terminal lrzsz / `sz` 下载 / 拖拽上传到当前终端目录 Requirements

日期: 2026-06-10
执行者: Codex
状态: 需求已收敛，待进入独立 worktree 实现

## 1. 背景与用户问题

当前 MicaTerm 在 SSH terminal 场景下存在两类明显缺口：

1. 远端 shell 执行 `sz some-file` 时，客户端不会识别 ZMODEM/lrzsz 下载序列，用户感知为“`sz` 下载无效”。
2. 把本地文件或目录拖到 terminal surface 时，当前产品没有“上传到 active terminal 当前远端目录”的能力。

本轮需求收敛后的目标不是“泛化的文件管理器”，而是把用户已经在 terminal 里表达出来的文件传输意图收敛为可预期的、可验证的行为：

- 当用户正在某个 SSH terminal 中工作时，拖拽上传必须绑定该 active terminal session，而不是绑定 SFTP 面板当前目录或本地当前目录。
- 如果 terminal 当前远端目录已经不存在、不可访问或无法确认，必须明确失败并提示，不允许静默 fallback 到 `HOME`、`/`、SFTP 当前目录或其他目录。
- `sz` 下载必须被视为 terminal byte stream 层的协议能力，而不是普通文本输出。

## 2. 本地现状基线

### 2.1 终端 cwd 跟踪并非空白，仓库里已经存在 shell integration 基础设施

本地代码核查结论：

- `src/app/ssh/shell_integration.rs`
  - 已存在 `ShellIntegrationEvent::CurrentDirectory`、`PromptStart/End`、`CommandStart/Finished`。
  - `runtime_shell_events(bytes)` 已能解析并清洗 `OSC 7`、`OSC 133`、`OSC 1337;CurrentDir=...`。
  - `build_shell_bootstrap(...)` 已为 bash/zsh/fish 生成 shell bootstrap。
  - `MicaPrivateAction::{OpenPath, EditPath, DownloadPath}` 已存在，说明 terminal 私有动作通道已经被设计过，但目前未接入文件传输产品能力。
- `src/app/ssh/runtime/pump.rs`
  - `run_channel_pump(...)` 是 SSH channel 原始字节流进入 terminal 的核心路径。
  - `process_ready_remote_output(...) -> parse_output_chunks(...)` 会先运行 shell integration 解析，再把 `sanitized_bytes` 交给 `apply_remote_output(...)`。
  - `SessionRuntimeEvent::CurrentDirectoryChanged` 已在此路径发出。
- `src/app/ssh/session_manager.rs`
  - `current_working_directory(session_id)` 已能读取 session 级 cwd 快照。
  - `apply_runtime_event(... CurrentDirectoryChanged(path))` 会更新 `current_working_directories`。

结论：**active terminal cwd tracking 在本仓库里已经存在，不需要从零发明。**

### 2.2 SFTP 传输队列与传输中心已经产品化

本地代码核查结论：

- `src/app/bootstrap/sftp.rs`
  - `schedule_sftp_upload_paths(...)` 已能把本地文件/目录扫描后投递到统一上传队列。
  - `schedule_quick_browser_upload_from_paths(...)`、`schedule_active_sftp_upload_from_paths(...)` 已存在。
  - `sync_active_sftp_projection_from_manager(...)` 已把 active terminal session 与 SFTP quick browser 关联起来。
  - `initial_sftp_browser_path(...)` 当前在“有 session 但还没有 cwd”时允许 fallback 到 `/`；这适用于浏览器体验，但**不适用于 terminal drag-upload 的目标目录语义**。
- `src/app/sftp/queue.rs`
  - `TransferQueue::enqueue_upload(...)` 已存在，`TransferTaskState` 已覆盖 `Queued/Running/Paused/Interrupted/Completed/Failed/Cancelled/Conflict`。
- `src/app/sftp/local_ops.rs`
  - `scan_local_sources(...)` 已支持文件、目录、空目录扫描。
  - `build_remote_upload_path(...)` 已只接受 `Component::Normal`，具备基础路径净化能力。
- `src/app/sftp/session_binding.rs`
  - `execute_queued_transfers_with_progress(...)` 已有上传/下载执行主循环。
  - 上传前已大量复用 `runtime.path_exists(...)`、`runtime.read_dir(...)`、`ensure_remote_parent_dirs(...)`。
- `src/app/ssh/runtime/sftp_backend.rs`
  - `read_dir(...)`、`path_exists(...)`、`stat(...)` 已存在。
  - 但 `stat(...)` 当前只回传 `size_bytes` 和 `modified_unix_seconds`，**不提供“是否为目录”的直接元数据**；因此“cwd 仍存在且为目录”的首版校验更适合走 `path_exists + read_dir`。

结论：**拖拽上传首选复用现有 SFTP transfer queue / transfer center，而不是自己再发明一套上传引擎。**

### 2.3 OS 文件拖拽目前只接到 SFTP quick browser，不接 terminal surface

本地代码核查结论：

- `src/app/bootstrap/windowing.rs`
  - 当前只监听 `HoveredFile` / `DroppedFile`，并通过 `sftp_drop_target_contains(...)` 判断是否落在 SFTP panel drop target。
  - 命中后只会调用 `window.set_sftp_panel_external_drop_paths(...)` 和 `window.invoke_sftp_panel_external_drop_requested()`。
- `src/shell/view_model/sftp.rs`
  - `quick_browser_accepts_external_drop()` 已定义 quick browser 是否可接收外部拖拽。
  - `set_quick_browser_drop_target_active(...)` 已有 overlay 状态管理。
- `tests/bootstrap_smoke.rs`
  - 已有 `external_sftp_drop_callbacks_toggle_overlay_and_queue_background_uploads`
  - 已有 `native_windowing_bridge_wires_os_file_drop_events_into_sftp_callbacks`

结论：**拖拽上传不是“完全不存在”，而是目前仅支持 SFTP quick browser 面板落点，不支持 terminal surface 落点。**

### 2.4 active terminal 与 active SFTP 的 session 绑定已经存在

本地代码核查结论：

- `src/shell/view_model/workspace.rs`
  - `active_workspace_terminal_session_id()` 已能给出 active terminal session。
- `src/shell/view_model/sftp.rs`
  - `quick_browser_linked_terminal_session_id()`、`active_sftp_linked_terminal_session_id()` 已存在。
- `src/app/sftp/browser_session.rs`
  - `linked_terminal_session_id`、`follow_mode`、`follow_terminal_path(...)`、`reenable_follow(...)` 已定义“跟随 terminal cwd”的产品语义。
- `tests/sftp_follow_cwd_spec.rs`
  - 已覆盖 follow-cwd、manual-browse、re-enable、session switch 等契约。

结论：**“拖拽必须绑定 active terminal，不得串 session”在当前架构中是可表达、可测试的。**

## 3. 调研结论摘要

### 3.1 本地代码调研结论

1. shell integration / cwd tracking 已经存在，且核心解析点就在 terminal 原始输出链路里。
2. SFTP queue / transfer center 已经成熟，足以承接 terminal-context drag-upload。
3. 当前 OS 文件拖拽入口只认 SFTP panel drop target，不认 terminal surface。
4. 目前没有可复用的 ZMODEM/lrzsz 实现；仓库内未发现 `zmodem`、`lrzsz`、`rz`、`sz` 相关业务代码。
5. 当前 SFTP 浏览器在“cwd 未知”时允许 fallback `/`，这是浏览器特有行为，不能直接复用到 terminal 当前目录上传。

### 3.2 外部成熟做法结论

外部调研覆盖了官方文档、开源代码和社区成熟集成，至少包括：

- Tabby：在 session middleware 中插入 `ZModem.Sentry`，先检测、再确认、再进入独立收发流程。
- Xshell / ZOC：把 ZMODEM 作为 terminal 客户端内建能力，配合专用传输 UI、下载目录配置、冲突处理。
- iTerm2：官方只提供 `Triggers` / `Coprocess` 能力，社区用脚本外挂 `sz/rz`，不是原生内建协议栈。
- WezTerm：官方重点支持 `OSC 7` shell integration；未提供成熟的原生 ZMODEM 产品路径。
- `zmodem.js`：强调要在原始输入字节流上用 sentry/state machine 扫描，并在确认前避免把协议字节误交给普通终端渲染。
- `lrzsz` / `sz` manpage：说明多文件、覆盖策略、发送端驱动等协议语义。
- Rust `rzsz` 项目：证明 Rust 生态已有较新的 CLI 实现，但当前更像独立程序，不是稳定的嵌入式库。

综合结论：

- **drag-upload 到 terminal cwd 更适合复用 SFTP queue。**
- **`sz` 下载应被设计为 raw byte stream 上的独立协议门，而不是普通 terminal 文本分支。**
- **`rz` 上传不应在首版作为 drag-upload fallback 自动启用。**

## 4. 用户故事

### US1：`sz` 下载

作为一名通过 SSH 使用远端 shell 的用户，
当我在 terminal 中执行：

```bash
sz some-file
```

我希望 MicaTerm 能识别 ZMODEM/lrzsz 下载握手，进入专用接收流程，并把文件保存到我明确确认的本地位置，而不是把二进制数据污染到 terminal 画面里。

### US2：拖拽文件上传到当前终端目录

作为一名正在某个 SSH terminal 中工作的用户，
当我把本地文件拖到 active terminal surface 上时，
我希望文件上传到该 terminal 当前远端工作目录，而不是上传到别的 session、别的 tab、SFTP 面板目录或默认目录。

### US3：拖拽目录上传到当前终端目录

作为一名正在某个 SSH terminal 中工作的用户，
当我把本地目录拖到 active terminal surface 上时，
我希望目录结构被递归上传到该 terminal 当前远端工作目录，并且空目录、中文路径、空格路径都能得到明确处理。

### US4：当前终端目录已删除时失败提示

作为一名拖拽上传的用户，
如果 active terminal 对应的远端 cwd 在我拖拽前后被删除、替换、失效或不可访问，
我希望上传被阻止，并得到明确错误，例如“上传失败：当前终端目录不存在”，而不是静默失败或上传到错误目录。

### US5：多 tab / 多 session 不串联

作为一名同时打开多个 SSH session / terminal tab 的用户，
我希望拖拽上传严格绑定当前 active terminal session，
不会误传到另一个 tab，也不会因为 quick browser / workspace SFTP 同时可见而串到别的会话目标上。

## 5. Scope

### 5.1 总体能力目标

本轮规划覆盖两条能力路线：

1. terminal-context drag-upload 到 active terminal cwd。
2. `sz` 下载的协议级产品化设计。

### 5.2 首发范围（推荐进入第一阶段实现）

基于本地架构、外部案例和多专家评审，推荐首发只交付：

1. **拖拽上传到 active terminal cwd**。
2. 复用现有 **SFTP transfer queue / transfer center**。
3. cwd 未知、cwd 已删除、cwd 非目录、SFTP 不可用时的明确失败提示。
4. 多 session / 多 tab / 多 drop target ownership 的回归覆盖。

### 5.3 后续阶段候选范围（本轮只设计，不承诺首发实现）

1. `sz` 下载检测与接收。
2. `sz file1 file2` 多文件接收与冲突处理。
3. `sz` 接入 transfer center 的通用下载记录。
4. 为未来 `rz` 上传预留但不首发承诺的协议扩展点。

### 5.4 本轮非目标

1. 本轮不修改业务代码，只产出 `requirements / design / tasks` 文档。
2. 不实现完整文件管理器。
3. 不改变既有 SFTP workspace / quick browser 的产品方向。
4. 不把 terminal drag-upload 退化成“把路径或命令直接写进 terminal stdin”。
5. 首发不默认纳入 `rz` 上传。
6. 不在 cwd 未知或失效时 fallback 到 `HOME`、`/`、SFTP 当前目录或任意猜测目录。

## 6. 成功标准

### 6.1 首发成功标准

1. 用户把本地文件或目录拖到 active terminal surface 时，MicaTerm 能解析出唯一的 active terminal session。
2. 目标远端目录来自该 session 已跟踪的 cwd 快照，而不是来自 SFTP panel 当前目录。
3. 上传前系统会通过 SFTP 再次校验该 cwd 当前仍存在且为目录。
4. 校验失败时，系统明确提示失败原因；不进行静默 fallback。
5. 上传任务通过现有 transfer queue / transfer center 展示进度、失败、取消、重试等产品状态。
6. 多 tab / 多 session / terminal 与 SFTP panel 共存时，drop target ownership 清晰，不串 session。

### 6.2 后续阶段成功标准（`sz` 候选）

1. `sz` 握手能在 SSH 原始字节流层被识别。
2. 已确认的 ZMODEM 二进制流不会进入普通 terminal renderer。
3. 文件接收过程有明确的本地保存位置确认、进度、取消、失败提示。
4. 协议误判时，缓冲字节可回放到正常 terminal 路径，不破坏正常文本输出。

## 7. 错误提示标准

### 7.1 drag-upload 相关

- cwd 未知：`上传失败：当前终端目录尚未识别，请等待 shell 目录同步后重试。`
- cwd 不存在：`上传失败：当前终端目录不存在。`
- cwd 非目录：`上传失败：当前终端位置不是可上传目录。`
- SFTP 不可用：`上传失败：当前会话未提供 SFTP 通道。`
- 权限不足：`上传失败：没有写入当前终端目录的权限。`
- 连接断开：`上传失败：连接已断开。`
- 用户取消：`上传已取消。`
- 冲突待处理：沿用 transfer center 现有 conflict 语义，不静默覆盖。

### 7.2 `sz` / ZMODEM 相关

- 用户拒绝接收：`已拒绝远端发起的文件接收请求。`
- 协议解析失败：`文件接收失败：ZMODEM 协议解析异常。`
- 保存路径不可写：`文件接收失败：本地保存位置不可写。`
- 连接中断：`文件接收失败：连接已中断。`
- 用户取消：`文件接收已取消。`

## 8. 安全与权限要求

1. 远端 cwd 只能作为“候选上传目标”，上传前必须经当前 SFTP runtime 二次验证。
2. cwd 不存在或不可验证时必须失败，不能猜测 fallback。
3. 拖拽上传不允许通过 terminal stdin 自动注入 `rz`、`cd`、`mkdir` 或其他命令作为隐式 fallback。
4. `sz` 接收时，远端给出的文件名只能作为建议值，必须做本地路径净化，禁止路径穿越、控制字符、Windows 保留名、ADS、`..` 等危险路径片段。
5. 本地下载首选临时文件写入后再原子重命名，避免半成品覆盖目标文件。
6. 同名覆盖必须显式决策，不允许静默覆盖。
7. 协议检测必须可拒绝，不能因为远端输出任意字节就自动在本地写文件。

## 9. 需要测试覆盖的行为清单

### 9.1 drag-upload

1. active terminal session 解析正确。
2. cwd 已知时，drop 绑定正确 session。
3. cwd 未知时拒绝上传并提示。
4. cwd 已删除时拒绝上传并提示“目录不存在”。
5. cwd 是文件而不是目录时拒绝上传。
6. SFTP binding 已断开时拒绝上传。
7. terminal surface 与 quick browser 同时存在时，drop target ownership 正确。
8. 多 session / 多 terminal tab 下不串 session。
9. 文件上传、目录上传、空目录上传都能正确入队。
10. 本地路径包含中文、空格、符号链接时可正确扫描和映射。
11. 权限不足、网络断开、取消、重试、冲突提示等 transfer queue 行为可见。

### 9.2 `sz` / ZMODEM

1. 分片字节流下的握手检测。
2. 检测误判后的字节回放。
3. 已确认传输期间 renderer 不显示协议二进制内容。
4. 多文件 `sz file1 file2` 的队列化处理。
5. 用户拒绝接收、取消接收、连接中断、协议异常等失败路径。
6. Windows / Linux 保存路径语义差异。

## 10. 本地代码证据与外部案例证据摘要

### 10.1 实际查看过的本地关键文件 / 关键函数 / 关键类型

1. `src/app/ssh/shell_integration.rs`
   - `ShellIntegrationEvent`
   - `RuntimeShellEvents`
   - `runtime_shell_events(...)`
   - `build_shell_bootstrap(...)`
   - `MicaPrivateAction`
2. `src/app/ssh/runtime/pump.rs`
   - `run_channel_pump(...)`
   - `process_ready_remote_output(...)`
   - `parse_output_chunks(...)`
   - `SynchronizedOutputBatcher`
3. `src/app/ssh/session_manager.rs`
   - `current_working_directory(...)`
   - `sftp_binding(...)`
   - `attach_runtime_control(...)`
   - `clear_runtime_control(...)`
4. `src/shell/view_model/workspace.rs`
   - `active_workspace_terminal_session_id()`
5. `src/shell/view_model/sftp.rs`
   - `quick_browser_accepts_external_drop()`
   - `quick_browser_linked_terminal_session_id()`
   - `active_sftp_linked_terminal_session_id()`
6. `src/app/sftp/browser_session.rs`
   - `linked_terminal_session_id`
   - `follow_terminal_path(...)`
   - `reenable_follow(...)`
7. `src/app/bootstrap/sftp.rs`
   - `schedule_sftp_upload_paths(...)`
   - `schedule_quick_browser_upload_from_paths(...)`
   - `schedule_active_sftp_upload_from_paths(...)`
   - `initial_sftp_browser_path(...)`
   - `sync_active_sftp_projection_from_manager(...)`
8. `src/app/sftp/queue.rs`
   - `TransferQueue::enqueue_upload(...)`
   - `TransferTaskState`
9. `src/app/sftp/local_ops.rs`
   - `scan_local_sources(...)`
   - `build_remote_upload_path(...)`
10. `src/app/sftp/session_binding.rs`
    - `execute_queued_transfers_with_progress(...)`
    - `ensure_remote_parent_dirs(...)`
11. `src/app/ssh/runtime/sftp_backend.rs`
    - `read_dir(...)`
    - `path_exists(...)`
    - `stat(...)`
12. `src/app/bootstrap/windowing.rs`
    - `HoveredFile` / `DroppedFile` 处理
    - `sftp_drop_target_contains(...)`
13. `tests/ssh_shell_integration_spec.rs`
14. `tests/ssh_terminal_interaction_spec.rs`
15. `tests/sftp_follow_cwd_spec.rs`
16. `tests/bootstrap_smoke.rs`
17. `docs/plans/2026-04-15-sftp-quick-browser-polish-design.md`
18. `docs/plans/2026-03-31-default-enhanced-remote-session-design.md`
19. `docs/plans/2026-03-31-sftp-browser-transfer-center-implementation-plan.md`
20. `docs/plans/2026-04-18-sftp-download-behavior-design.md`
21. `docs/plans/2026-04-28-terminal-semantic-boundaries-design.md`
22. `docs/plans/2026-06-08-new-tab-multi-session-terminal-selection/{requirements.md,designs.md,tasks.md}`

### 10.2 外部案例摘要

1. **Tabby**
   - 证据：`Eugeny/tabby` 仓库 `tabby-terminal/src/features/zmodem.ts`
   - 路线：在 terminal session middleware 中挂 `ZModem.Sentry`，先检测，再让用户确认，再独立收发。
2. **Xshell**
   - 证据：NetSarang 产品资料 / 手册检索结果
   - 路线：内建 ZMODEM，自动响应 `sz/rz` 场景，强调集成传输体验。
3. **ZOC**
   - 证据：`https://www.emtec.com/zoc/help/en/10381/zmodem-file-transfer`
   - 路线：内建 ZMODEM + 下载目录配置 + Transfer Window。
4. **iTerm2**
   - 证据：`https://iterm2.com/documentation-triggers.html` + `iTerm2-zmodem` 社区集成
   - 路线：官方提供 trigger/coprocess，社区外挂 `sz/rz` 脚本，不是内建协议引擎。
5. **WezTerm**
   - 证据：`https://wezterm.org/shell-integration.html` / `https://wezterm.org/config/lua/config/launch_menu.html`
   - 路线：强化 `OSC 7` cwd 追踪，而非主推内建 ZMODEM。
6. **zmodem.js**
   - 路线：sentinel/state machine 扫描原始字节流，避免把协议数据污染普通 terminal 输出。
7. **lrzsz / sz manpage**
   - 路线：定义多文件、发送端驱动、冲突/覆盖语义。
8. **Rust `rzsz`**
   - 路线：现代 Rust CLI 实现，可作未来协议实现参考，但当前不宜直接当成成熟嵌入库假定采用。

## 11. 冲突与修正记录

1. prompt 要求调研“仓库是否已有 shell integration / cwd tracking”。
   - 本地事实：**已有**，而且已经接入 SSH runtime output 路径。
   - 修正：drag-upload 设计必须优先复用现有 cwd tracking，而不是假设需要重新做 `pwd` 轮询。
2. prompt 要求调研“仓库是否已有 SFTP session / transfer queue / drag-drop pipeline”。
   - 本地事实：**已有成熟 SFTP transfer queue 与 quick browser 外部拖拽链路**。
   - 修正：drag-upload 方案必须优先复用现有 SFTP queue，而不是新造上传通道。
3. prompt 讨论“当前终端目录失败时是否 fallback”。
   - 本地事实：SFTP browser 当前存在 fallback `/` 的浏览体验代码。
   - 修正：该 fallback 只适用于浏览器初始展示，**不允许用于 terminal cwd 上传目标**。
