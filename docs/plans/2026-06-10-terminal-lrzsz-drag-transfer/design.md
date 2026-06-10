# Terminal lrzsz / `sz` 下载 / 拖拽上传到当前终端目录 Design

日期: 2026-06-10
执行者: Codex
状态: 设计已收敛，待进入独立 worktree 实现

## 1. 调研方法与可信度说明

本轮结论来自三类证据：

1. **本地代码核查**：直接检查 `src/`、`tests/`、`docs/plans/` 当前实现与既有计划。
2. **外部成熟做法检索**：实际使用 Tavily/MCP 检索官方文档、开源代码与社区集成案例。
3. **真实多专家辩论**：已实际启动多位 subagent，围绕协议、架构、产品、测试、安全、性能做三轮讨论并收敛方案。

限制说明：

- 本轮只做规划文档，不修改业务代码。
- `sz` 下载虽然进入设计，但经本地架构和多专家评审后，不建议纳入首发实现范围。
- 外部资料里，Xshell 官方可直接拿到的数据以产品资料/文档检索结果为主，内部实现细节公开度低，因此只把它当作 UX 参考，不把它当作可直接复用的工程方案。

## 2. 当前架构调查

## 2.1 terminal stream 与 SSH runtime

关键文件与结论：

- `src/app/ssh/runtime/pump.rs`
  - `run_channel_pump(...)` 是 SSH shell channel 原始字节流的核心入口。
  - 远端输出会进入 `SynchronizedOutputBatcher`，再由 `process_ready_remote_output(...)` 统一处理。
  - `process_ready_remote_output(...)` 会调用 `parse_output_chunks(...)`，再把 `parsed.sanitized_bytes` 交给 `apply_remote_output(...)`。
  - `CurrentDirectoryChanged` runtime event 在此路径发出。
- `src/app/ssh/shell_integration.rs`
  - `runtime_shell_events(...)` 会边解析 shell integration escape sequence，边生成“去掉控制序列后的可见字节”。
  - 当前 `PrivateAction` 事件虽然能被解析，但 `apply_runtime_event(...)` 里对 `PrivateAction` 仍是 no-op。

设计边界：

- 如果未来要做 `sz` / ZMODEM 接收，**协议检测点必须在 raw SSH bytes 进入普通 terminal 渲染前**。
- 最合适的边界不是 Slint host，也不是 terminal semantic 层，而是 `src/app/ssh/runtime/pump.rs` 附近的 output pump seam。

## 2.2 cwd tracking 与 enhanced session

关键文件与结论：

- `src/app/ssh/shell_integration.rs`
  - 已支持 `OSC 7`、`OSC 133`、`OSC 1337;CurrentDir=...`。
  - `build_shell_bootstrap(...)` 已说明产品此前就走“增强 shell integration”路线。
- `src/app/ssh/session_manager.rs`
  - `current_working_directory(session_id)` 已可被调用。
  - `CurrentDirectoryChanged(path)` 已会写入 session 级 cwd 快照。
- `tests/ssh_shell_integration_spec.rs`
  - 已覆盖 shell integration 解析与清洗。
- `tests/ssh_terminal_interaction_spec.rs`
  - 已覆盖 `osc7_sequence_updates_current_working_directory_snapshot`。

设计边界：

- terminal cwd source 的**首选真相源**应是既有 shell integration 跟踪结果。
- `pwd` 轮询不应成为首选，因为它会污染用户会话、可能阻塞、也不适合在 TUI 或多命令混跑场景下注入。

## 2.3 SFTP subsystem 与 transfer queue

关键文件与结论：

- `src/app/bootstrap/sftp.rs`
  - 已有 `schedule_sftp_upload_paths(...)` 作为“从本地路径批量入队上传”的主入口。
  - quick browser 和 active workspace SFTP 上传都已复用该入口。
- `src/app/sftp/queue.rs`
  - `TransferQueue::enqueue_upload(...)` 已有完整 task queue 抽象。
- `src/app/sftp/local_ops.rs`
  - `scan_local_sources(...)` 已能递归扫描目录、保留空目录。
  - `build_remote_upload_path(...)` 已净化远端子路径。
- `src/app/sftp/session_binding.rs`
  - `execute_queued_transfers_with_progress(...)` 已具备上传/下载进度推进与冲突状态管理。
- `src/app/ssh/runtime/sftp_backend.rs`
  - `read_dir(...)`、`path_exists(...)`、`stat(...)` 已存在，但 `stat` 元数据还不够表达“这是目录”。

设计边界：

- drag-upload 首发应当复用现有 SFTP queue / transfer center，不应重新发明另一条上传执行链。
- cwd 目录存在性验证首版优先走 `path_exists + read_dir`，而不是假设 `stat` 足够。

## 2.4 drag-drop handler 与 drop target ownership

关键文件与结论：

- `src/app/bootstrap/windowing.rs`
  - 目前 OS 文件拖拽只命中 `sftp_drop_target_contains(...)`。
  - 命中后只触发 quick browser 的外部拖拽回调。
- `src/shell/view_model/sftp.rs`
  - quick browser 有完整的 drop overlay gating 与 linked-terminal session 检查。
- `tests/bootstrap_smoke.rs`
  - 已锁定 quick browser 外部拖拽与后台上传入队。

设计边界：

- terminal surface drag-upload 首发不是“在已有代码上加一个上传函数”那么简单，而是需要先补 **drop target ownership**：
  - pointer 落在 SFTP quick browser 上 -> 走现有 quick browser 上传。
  - pointer 落在 active terminal surface 上 -> 走新的 terminal-context upload。
  - 二者同时可见时不能争抢同一份 dropped file。

## 2.5 ShellViewModel / bootstrap wiring

关键文件与结论：

- `src/shell/view_model/workspace.rs`
  - `active_workspace_terminal_session_id()` 已能表达 active terminal 的绑定关系。
- `src/app/bootstrap/sftp.rs`
  - `sync_active_sftp_projection_from_manager(...)` 已把 active terminal cwd 投影到 quick browser。
- `src/app/sftp/browser_session.rs`
  - `linked_terminal_session_id` 和 `follow_mode` 已把“跟随 terminal cwd”和“手动浏览”区分开。

设计边界：

- terminal-context drag-upload 需要的是“active terminal cwd snapshot”，不是 quick browser 当前 path。
- quick browser 的“manual-browse”模式不应污染 terminal drag-upload 目标语义。

## 3. 外部成熟做法调研

| 案例 | 证据 | 技术路线 | 与 MicaTerm 适配度 | 风险点 | 采纳结论 |
| --- | --- | --- | --- | --- | --- |
| Tabby | `Eugeny/tabby` 的 `tabby-terminal/src/features/zmodem.ts` | 在 session middleware 中插入 `ZModem.Sentry`，先检测，再弹确认，再进入收发 | 高：与 MicaTerm 的 raw byte stream 边界理念接近 | Tabby 用 JS 中间件与平台下载器，不等价于 Rust + Slint 集成 | **采纳其检测边界思路**，不直接照搬实现 |
| Xshell | NetSarang 产品资料 / 文档检索结果 | 内建 ZMODEM，引导自动收发，强调集成传输体验 | 中：UX 可参考 | 内部实现不公开，且可能高度平台耦合 | 作为 UX 参考，不作为工程路线直接复用 |
| ZOC | `emtec.com` 官方帮助文档 | 内建 ZMODEM + Transfer Window + 下载目录设置 | 中高：对下载目录、进度、冲突 UI 有参考价值 | 官方文档偏使用说明，不涉及内部架构 | **采纳其 transfer UI 语义** |
| iTerm2 + iTerm2-zmodem | iTerm2 官方 `Triggers` 文档 + 社区 `iTerm2-zmodem` | 官方只给 trigger/coprocess，社区外挂脚本 | 低：MicaTerm 不适合做脚本外挂 | 脆弱、平台差异大、错误处理弱 | **不采用脚本外挂路线** |
| WezTerm | WezTerm 官方 shell integration 文档 | 强调 `OSC 7` cwd 追踪；不主打内建 ZMODEM | 高：验证 cwd tracking 设计方向 | 对 ZMODEM 本身帮助有限 | **采纳 OSC 7 作为 cwd 证据** |
| `zmodem.js` | README / 项目说明 | 在原始字节流上做 sentry/state machine 扫描 | 高：直接证明“检测必须在 renderer 前” | JS 环境实现不可直接复用 | **采纳 sentry/state-machine 边界** |
| `lrzsz` / `sz` manpage | 官方/镜像文档 | 发送端驱动，多文件、覆盖策略 | 中：协议语义参考 | 老项目，嵌入式设计参考不足 | 作为协议语义来源 |
| Rust `rzsz` | 开源仓库 | 现代 Rust CLI 实现 | 中：说明 Rust 可做 | CLI 工程，不是已验证的嵌入库 | **暂不默认引入** |

调研结论：

1. terminal 类产品若要做 `sz`，**检测点必须在普通 renderer 前**。
2. drag-upload 到 terminal cwd 的成熟工程路径更偏向 **SFTP/文件传输子系统**，不应混入 terminal stdin。
3. iTerm2 式 trigger 脚本是“外挂 workaround”，不适合 MicaTerm 当前产品化目标。
4. cwd tracking 采用 `OSC 7` 是成熟做法，本仓库正好已有这一基础。

## 4. subagent 多专家辩论摘要

本轮实际组织了六类专家：

1. 终端协议专家
2. MicaTerm 架构专家
3. 产品与 UX 专家
4. 测试与可靠性专家
5. 安全专家
6. 性能与并发专家

并比较了至少三种路线：

1. **ZMODEM-only**：完整做 `sz/rz`，drag-upload 也走 ZMODEM。
2. **SFTP drag-upload + ZMODEM `sz` download**：上传复用 SFTP，下载做协议接收。
3. **SFTP-only**：只做 terminal-context drag-upload，不做 `sz`。
4. **hybrid with future `rz`**：先做 SFTP drag-upload，预留未来 `sz`/`rz` 协议扩展。

### 第 1 轮

- 协议专家：强推“如果做 `sz`，必须在 raw SSH byte stream 上用 sentry/state machine 检测，不允许协议字节进 renderer”。
- 架构专家：指出当前仓库最成熟的是 SFTP queue 与 cwd tracking，ZMODEM 从 0 开始，集成面较大。
- 产品专家：认为“拖拽上传到当前终端目录”是用户最直观的立即收益，`sz` 可作为后续增强。
- 测试专家：指出 drag-upload 有现成测试地基；ZMODEM 会引入新的协议输入面与误判回放问题。
- 安全专家：明确反对“cwd 失效时 fallback 到 HOME 或直接写 stdin”；反对无确认自动接收文件。
- 性能专家：担心在 runtime pump 中处理大文件协议时会阻塞终端 IO、引入背压与取消复杂度。

### 第 2 轮

- 协议专家：接受“上传先走 SFTP，`sz` 做 receive-only 候选”，但坚持必须保留未来 protocol gate 设计。
- 架构专家：建议首发只做 drag-upload，把 `sz` 放到 Phase 2，并把 protocol gate 位置写进设计。
- 产品专家：支持首发聚焦 drag-upload，因为它能直接复用 transfer center 并给出稳定错误语义。
- 测试专家：支持先做 drag-upload，再做 protocol-gated `sz`，避免首发同时打开两个高风险面。
- 安全专家：坚持 `sz` 未来也必须显式确认、本地路径净化、临时文件写入、无静默覆盖。
- 性能专家：指出当前 transfer queue 已经有进度与后台执行语义，而 `sz` 还没有对应的隔离执行模型。

### 第 3 轮收敛

- 五位专家收敛到同一结论：**首发范围只交付 SFTP drag-upload 到 active terminal cwd。**
- 协议专家接受“总体架构为 phased hybrid”，即：
  - Phase 1：SFTP drag-upload
  - Phase 2 候选：receive-only `sz`
- 安全专家更保守，认为 `sz` 最好写成“后续候选能力”，不要在本轮把它包装成已承诺首发功能。

### 决策记录

最终采用：**分阶段 hybrid 方案**。

- **首发实现范围**：SFTP drag-upload 到 active terminal cwd。
- **本轮只设计、不首发实现**：`sz` 下载的 raw-byte protocol gate 与接收状态机边界。
- **后置能力**：`rz` 上传继续后置，不做 drag-upload fallback。

## 5. 方案比较矩阵

| 方案 | 架构贴合度 | 用户价值 | 安全性 | 测试可行性 | 首发建议 |
| --- | --- | --- | --- | --- | --- |
| ZMODEM-only | 低 | 中 | 低 | 低 | 不推荐 |
| SFTP drag-upload + ZMODEM `sz` | 中高 | 高 | 中 | 中 | 适合作为总体路线，不适合首发同时落地 |
| SFTP-only | 高 | 高 | 高 | 高 | **首发推荐** |
| hybrid with future `rz` | 中 | 中 | 中低 | 低 | 只保留扩展位，不进入首发 |

### 不采用 ZMODEM-only 的原因

1. 不能稳定表达“上传到 active terminal cwd 且先验证目录仍存在”的产品语义。
2. drag-upload 若走 `rz`，会把文件传输耦进 terminal stdin / 协议时序，错误处理差。
3. 无法复用现有 transfer center / conflict handling / retry / queue 资产。

### 不采用 SFTP-only 作为“总体架构终点”的原因

1. 用户明确存在 `sz` 下载需求。
2. 远端执行 `sz` 时，不识别协议仍会留下产品缺口。
3. 本仓库的 raw-byte seam 很清晰，长期完全放弃 `sz` 并不合理。

### 采用 phased hybrid 的原因

1. 首发优先把价值最高、风险最低、复用度最高的 drag-upload 落地。
2. 同时在设计上明确 `sz` 的正确边界，避免未来为了补功能而走错位置。
3. 满足“当前终端目录”的严格语义和“后续支持 terminal 原生下载”的长期路线。

## 6. 推荐架构

### 6.1 总体架构

推荐架构是 **phased hybrid**：

- **Phase 1 / 首发**：terminal-context drag-upload 复用现有 SFTP transfer queue。
- **Phase 2 候选**：在 `src/app/ssh/runtime/pump.rs` 邻近边界增加 ZMODEM protocol gate，实现 receive-only `sz` 下载。
- **Future**：评估是否需要 `rz`，但不承诺首发，也不允许成为 drag-upload 的静默 fallback。

### 6.2 Phase 1：drag-upload 首发架构

流程建议：

```text
OS file drop on terminal surface
  -> native windowing hit-test terminal drop target
  -> resolve active terminal session_id
  -> read cwd snapshot from SessionManager
  -> capture terminal upload context snapshot { session_id, sftp_binding, cwd }
  -> validate snapshot against current SFTP runtime
  -> scan local sources
  -> enqueue upload via schedule_sftp_upload_paths(...)
  -> show progress/failure/cancel in transfer center
```

关键原则：

1. 目标目录来源于 active terminal cwd snapshot。
2. snapshot 只提供候选值，真正入队前必须做当前态校验。
3. 一旦校验失败，必须失败并提示，不得 fallback。

### 6.3 Phase 2 候选：`sz` 下载架构

流程建议：

```text
raw SSH output bytes
  -> ZMODEM sentry/protocol gate (before shell integration sanitization reaches renderer)
  -> tentative detect + buffer
  -> user confirm / deny
  -> if deny: replay buffered bytes into normal terminal path
  -> if confirm: enter dedicated receive session, bypass normal renderer for protocol bytes
  -> save to local temp files under user-chosen folder
  -> publish progress / failure / cancel
```

关键原则：

1. 检测前移到 raw byte stream seam。
2. 拒绝后必须能安全回放缓存数据。
3. 确认后协议字节不再进入普通 terminal renderer。

## 7. `sz` 下载设计

> 注意：本节属于 **已设计但不建议纳入首发实现** 的能力。

### 7.1 handshake detection

推荐方案：

- 在 `src/app/ssh/runtime/pump.rs` 新增 ZMODEM sentry/gate 抽象。
- gate 维护一个小型 rolling buffer，允许跨 chunk 检测握手头。
- 检测逻辑只看原始远端输出字节，不看已经 sanitize 后的 terminal text。

原因：

- 当前 `parse_output_chunks(...)` 已经承担 shell integration 清洗责任。
- 若 ZMODEM 检测放在更后面，二进制头部和后续 frame 有机会污染 terminal 显示。

### 7.2 state machine boundary

推荐抽象：

```text
Idle
  -> TentativeDetect(buffering)
  -> AwaitUserDecision
  -> ActiveReceive
  -> Completed / Failed / Cancelled
```

关键要求：

1. `TentativeDetect` 期间缓存最小必要字节，不立刻污染 renderer。
2. `AwaitUserDecision` 允许用户拒绝；拒绝后需把缓存回放到正常 terminal 路径。
3. `ActiveReceive` 独占该 session 的协议处理权，直到 session 结束。

### 7.3 terminal renderer interaction

设计要求：

1. 仅当 gate 明确拒绝或未命中时，字节才进入普通 terminal 渲染。
2. 一旦进入 `ActiveReceive`，协议字节完全旁路普通 renderer。
3. 仅允许结构化状态消息进入 UI，例如“正在接收 1/3 文件”。

### 7.4 local save target

首版推荐：

- 每次 `sz` 接收都显式让用户选择本地保存目录。
- 若选择目录后存在多个文件，则在该目录下按文件名逐个落盘。
- 不推荐首版默认写 `Downloads`，也不推荐首版默认写“上次目录”而不确认。

原因：

1. 安全边界更清晰。
2. 多文件 `sz file1 file2` 也有统一保存语义。
3. 避免远端无感地把文件落到本地固定目录。

### 7.5 progress / cancel / failure

推荐：

- 接收过程进入统一 transfer center，但标记为 terminal transfer，而不是伪装成 SFTP download。
- 首版只承诺：`progress / cancel / completed / failed`。
- 不承诺 `pause/resume`。

### 7.6 multi-file `sz`

- 协议层允许 batch receive。
- UI 层应表现为“同一传输会话下的多个文件项”或“批次下载任务”。
- 同名冲突逐文件判定，不允许批量静默覆盖。

### 7.7 远端缺少 lrzsz / 命令失败时的表现

- 如果远端根本没有触发有效 ZMODEM 握手，MicaTerm 不应该误报“接收失败”；它只会表现为普通 terminal 文本输出。
- 若握手开始后中断，应显示明确的协议/连接失败，而不是仅在 terminal 里吐出乱码。

### 7.8 协议误判规避

1. 采用 sentry + confirm 机制，而不是仅凭单个短字符串判定。
2. 在进入 `ActiveReceive` 前保留拒绝分支。
3. 只有在协议头和后续帧足够可信时才切换状态。

### 7.9 Windows / Linux 平台差异

- 文件名保留字、路径分隔符、不可写目录提示不同。
- 本地保存路径净化必须是跨平台逻辑，不可只按 POSIX 处理。

### 7.10 Rust crate 还是自研最小状态机

本轮结论：

- **不默认直接引入现成 Rust crate。**
- 原因不是“永不引入”，而是本轮未验证到一个成熟、稳定、可嵌入、与 MicaTerm 架构自然贴合的 Rust 库。
- 推荐路线：
  1. 先在设计上定义 `ZmodemGate` / `ZmodemReceiveSession` 抽象。
  2. 实现阶段优先写最小检测与接收边界测试。
  3. 再决定是内部最小实现，还是用经验证的库替换具体协议解析器。

## 8. drag-upload 设计

### 8.1 drop target

推荐新增 terminal surface drop target，而不是复用 quick browser overlay：

- terminal surface 命中 -> terminal-context upload
- quick browser 命中 -> quick browser upload
- workspace SFTP 面板命中 -> 继续走当前 workspace SFTP 语义

### 8.2 active terminal binding

推荐以 `active_workspace_terminal_session_id()` 作为 terminal-context upload 的 session 真相源。

必须满足：

1. drop 发生时必须抓取 session snapshot。
2. session snapshot 至少包含：
   - `session_id`
   - `cwd_snapshot`
   - `sftp_binding_snapshot`
3. 入队前再次确认 snapshot 对应的当前 binding 仍有效。

### 8.3 cwd source

推荐优先级：

1. **首选**：`SessionManager.current_working_directory(session_id)`
2. **不采用**：SFTP quick browser 当前路径
3. **不采用**：现场执行 `pwd`
4. **不采用**：fallback `HOME` / `/`

原因：

- cwd 应表达“当前 terminal session 已知的远端工作目录”。
- SFTP quick browser 可能在 manual-browse，不等于 terminal cwd。
- `pwd` 注入会污染远端会话。

### 8.4 cwd 未知时的处理

推荐：**阻止上传并提示。**

不推荐：

- fallback 到 `HOME`
- fallback 到 SFTP 当前目录
- 弹远端目录选择器作为隐式替代

理由：

- 产品目标明确要求“当前终端目录”，不是“某个还能上传的目录”。
- 无法确认 terminal cwd 时，任何 fallback 都会制造“传到错误目录”的更大风险。

### 8.5 remote directory validation

推荐校验策略：

1. `sftp_binding` 当前仍可用。
2. `path_exists(cwd)` 为 true。
3. `read_dir(cwd)` 成功，证明它仍是可列目录。

若任何一步失败：

- 直接阻止入队。
- 优先给出中文错误：`上传失败：当前终端目录不存在。`
- 如果更精确地识别到权限问题，则提示权限错误。

### 8.6 SFTP queue reuse

推荐复用：

- 本地路径扫描：`scan_local_sources(...)`
- 远端路径拼接：`build_remote_upload_path(...)`
- 上传调度：`schedule_sftp_upload_paths(...)`
- 进度与状态：`TransferQueue` + transfer center

原因：

1. 已有产品化的进度、冲突、取消、失败 UI。
2. 已有目录上传与空目录扫描语义。
3. 已有后台执行模型。

### 8.7 deleted directory error

推荐错误优先级：

1. cwd 不存在 -> `上传失败：当前终端目录不存在。`
2. cwd 存在但非目录 -> `上传失败：当前终端位置不是可上传目录。`
3. cwd 可见但不可读/不可写 -> `上传失败：没有写入当前终端目录的权限。`

### 8.8 本地路径包含中文、空格、符号链接

设计要求：

1. terminal drop 路径在 bootstrap / Rust 层尽量保留 `PathBuf`，避免早早经 Slint `SharedString` 丢失原始路径信息。
2. 中文、空格路径应沿用 `scan_local_sources(...)` 的已有能力。
3. 符号链接沿用当前本地扫描策略，不在首发顺手改语义。

## 9. 数据模型 / 事件模型草案

### 9.1 Phase 1：terminal-context upload

建议新增概念模型：

```text
TerminalDropTargetKind
  - None
  - TerminalSurface
  - QuickBrowser
  - WorkspaceSftp

TerminalUploadContext
  - session_id: Uuid
  - cwd_snapshot: String
  - sftp_binding_snapshot: SftpSessionBinding
  - captured_at: Instant

TerminalUploadPreflightResult
  - Ready
  - CwdUnknown
  - CwdMissing
  - CwdNotDirectory
  - SftpUnavailable
  - PermissionDenied
  - Disconnected
```

建议新增事件：

```text
TerminalExternalDropHoverChanged
TerminalExternalDropRequested
TerminalUploadPreflightFailed
TerminalUploadQueued
```

### 9.2 Phase 2 候选：`sz` / ZMODEM

建议新增概念模型：

```text
ZmodemGateState
  - Idle
  - TentativeDetect
  - AwaitUserDecision
  - ActiveReceive

ZmodemReceiveOffer
  - file_name
  - file_size
  - files_remaining
  - bytes_remaining

TerminalTransferRecord
  - transfer_kind: ZmodemReceive | SftpUpload
  - state
  - progress
  - error
```

## 10. UI / UX 草案

### 10.1 drag-upload

- 当 pointer 悬停在 terminal surface 且当前会话满足基本前置条件时，terminal surface 显示“上传到当前终端目录”的 drop affordance。
- drop 前即可做轻量前置判断：
  - 无 active terminal
  - 无 cwd
  - 无 SFTP binding
  - 均不显示“可上传”成功态 overlay。
- drop 后若 preflight 失败，直接 toast / modal 错误，不把失败伪装成“已入队后再立即失败”。
- 上传成功入队后，沿用 transfer center 展示进度。

### 10.2 `sz` 下载

- 检测到可信的 ZMODEM receive 握手后，弹明确确认：
  - 是否接受远端发起的文件接收
  - 本地保存目录选择
- 进入接收后，transfer center 展示单独记录。
- 协议取消/错误不应只出现在 terminal 文本区。

## 11. 错误语义

| 场景 | 推荐错误语义 |
| --- | --- |
| cwd unknown | `上传失败：当前终端目录尚未识别，请等待 shell 目录同步后重试。` |
| cwd deleted | `上传失败：当前终端目录不存在。` |
| cwd not directory | `上传失败：当前终端位置不是可上传目录。` |
| permission denied | `上传失败：没有写入当前终端目录的权限。` |
| SFTP unavailable | `上传失败：当前会话未提供 SFTP 通道。` |
| transfer canceled | `上传已取消。` / `文件接收已取消。` |
| connection lost | `上传失败：连接已断开。` / `文件接收失败：连接已中断。` |
| ZMODEM parse error | `文件接收失败：ZMODEM 协议解析异常。` |

## 12. 风险与缓解

### 12.1 cwd 快照过期

风险：

- drop 时拿到的 cwd 可能在入队前已失效。

缓解：

- snapshot 只做候选值；真正入队前必须重新验证当前态。

### 12.2 terminal 与 SFTP panel 争抢 drop ownership

风险：

- 同一份 dropped files 被 quick browser 和 terminal surface 同时消费。

缓解：

- 在 native windowing 层统一做 hit-test，只允许一个 target 获胜。

### 12.3 `sz` 协议误判污染 terminal

风险：

- 错误识别会让普通文本输出被吞掉或乱码。

缓解：

- 使用 `TentativeDetect + confirm/deny + replay` 机制。

### 12.4 本地路径编码/特殊字符丢失

风险：

- 当前 quick browser drop 通过 `SharedString` 传路径，可能丢失非 UTF-8 路径信息。

缓解：

- terminal drop 新链路尽量在 Rust 层保留 `PathBuf`，不要过早字符串化。

### 12.5 未来 `sz` 与 terminal pump 耦合过深

风险：

- 若直接在 `run_channel_pump(...)` 里堆业务逻辑，后续可维护性变差。

缓解：

- 抽象成独立 gate/session 模块，只把 seam 放在 pump 层。

## 13. 不做事项与后续扩展路线

### 13.1 本轮不做

1. 不修改业务代码。
2. 不首发实现 `sz` 下载。
3. 不实现 `rz` 上传。
4. 不把 drag-upload 自动 fallback 到 `rz`。
5. 不顺手重做整个 transfer center 数据模型。

### 13.2 后续扩展路线

1. **Phase 1**：drag-upload 到 active terminal cwd。
2. **Phase 1.5**：补强 terminal drop target、路径保真、错误细化、跨平台 smoke。
3. **Phase 2 候选**：receive-only `sz` 下载，带 protocol gate、用户确认、目录选择、transfer center 记录。
4. **Phase 3 候选**：评估是否需要 `rz`，前提是：
   - 明确产品价值
   - 明确安全模型
   - 不损害 SFTP-first 的 drag-upload 语义
