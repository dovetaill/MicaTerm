# Terminal lrzsz / `sz` 下载 / 拖拽上传到当前终端目录 Tasks

日期: 2026-06-10
执行者: Codex
状态: 任务已拆分，待在独立 worktree 中执行

## 0. 执行前置约束

当前窗口只产出文档，不允许直接改业务代码。

正式实现必须：

1. 新开一个窗口 / 新会话。
2. 建立独立 worktree，例如：

```bash
git worktree add .worktrees/feature-terminal-lrzsz-drag-transfer -b feature/terminal-lrzsz-drag-transfer
```

3. 在该 worktree 中执行实现。
4. 实现窗口开始前必须重新应用以下 superpowers：
   - 总执行框架：`superpowers:executing-plans`
   - 每个功能/缺陷 task 开始前：`superpowers:test-driven-development`
   - 出现测试异常、协议误判、竞态或意外行为时：`superpowers:systematic-debugging`
   - 声称完成前：`superpowers:verification-before-completion`
   - 需要创建/核对 worktree 时：`superpowers:using-git-worktrees`
5. 最终 handoff 必须记录**精确命令与实际输出**，不能只写“已通过”。

## 1. Task 0：冻结 requirements / design / tasks 文档

### 目标

在独立 worktree 中锁定本轮范围，并确认首发只交付：

- terminal-context drag-upload 到 active terminal cwd
- 基于现有 SFTP transfer queue / transfer center 的产品接线

同时确认：

- `sz` 下载本轮只保留设计与后续阶段任务，不在首发直接落地
- `rz` 不进入首发范围

### 涉及文件

- `docs/plans/2026-06-10-terminal-lrzsz-drag-transfer/requirements.md`
- `docs/plans/2026-06-10-terminal-lrzsz-drag-transfer/design.md`
- `docs/plans/2026-06-10-terminal-lrzsz-drag-transfer/tasks.md`

### 先写的失败测试

- 无代码测试；先做实现范围核对与风险冻结。

### 最小实现范围

- 只确认文档与执行边界，不提前改任何业务代码。

### 验证命令

```bash
git status --short
rg -n "首发|Phase 2|rz|SFTP" docs/plans/2026-06-10-terminal-lrzsz-drag-transfer/*.md
```

### 回滚风险

- 若文档冻结不彻底，后续实现容易偷偷扩 Scope。

### 不能顺手做的事情

- 不能在这一步顺手开始改 `src/`、`ui/`、`tests/`。

## 2. Task 1：为 terminal byte stream ZMODEM detection 写失败测试

> 说明：该 task 属于 **Phase 2 gate**。只有 Phase 1 drag-upload 稳定后，才允许启动。

### 目标

先把 `sz` / ZMODEM 检测边界锁成失败测试，明确协议门必须放在 raw SSH output seam，而不是 renderer 后置补丁。

### 涉及文件

- `src/app/ssh/runtime/pump.rs`
- `src/app/ssh/shell_integration.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- 如有必要新增 `tests/ssh_zmodem_detection_spec.rs`

### 先写的失败测试

1. 分片字节流中的 ZMODEM 握手仍可被识别。
2. 误判后缓存字节会被完整回放到普通 terminal 路径。
3. 一旦确认进入接收态，协议字节不会进入 `apply_remote_output(...)`。
4. shell integration 与 ZMODEM 检测互不踩踏。

### 最小实现范围

- 只建立检测 gate 抽象与最小状态流，不实现完整 `rz`。
- 不在这一步接 UI 复杂交互。

### 验证命令

```bash
cargo test ssh_zmodem_detection_spec -- --nocapture
cargo test parse_output_chunks_merges_split_shell_integration_sequences -- --nocapture
```

### 回滚风险

- 检测边界选错会污染 terminal renderer，或把正常文本错误吞掉。

### 不能顺手做的事情

- 不要顺手做完整 ZMODEM 上传。
- 不要在没有失败测试的情况下直接写协议解析器。

## 3. Task 2：实现最小 `sz` download detection / receive abstraction

> 说明：该 task 同样属于 **Phase 2 gate**。

### 目标

在 Task 1 失败测试转绿后，实现最小 receive-only `sz` 抽象，打通：检测 -> 用户确认 -> 接收会话 -> 失败/取消。

### 涉及文件

- `src/app/ssh/runtime/pump.rs`
- 可能新增 `src/app/ssh/runtime/zmodem_gate.rs`
- 可能新增 `src/app/ssh/runtime/zmodem_receive.rs`
- `src/app/bootstrap.rs`
- transfer center 相关投影文件
- 新增或扩展 `tests/ssh_zmodem_detection_spec.rs`

### 先写的失败测试

1. 检测后用户拒绝，字节回放正常。
2. 用户确认后，接收态能产生命名、大小、进度记录。
3. 连接中断、协议异常、用户取消时状态正确落到 failed/cancelled。
4. 多文件 batch receive 的骨架状态可表达。

### 最小实现范围

- 只做 receive-only `sz`。
- 只做最小目录选择、接收进度、取消、失败。
- 不承诺 pause/resume。

### 验证命令

```bash
cargo test ssh_zmodem_detection_spec -- --nocapture
cargo test transfer_summary -- --nocapture
```

### 回滚风险

- 若抽象层次太低，后续很难把接收记录接入 transfer center。

### 不能顺手做的事情

- 不要顺手实现 `rz`。
- 不要把 `sz` 伪装成 SFTP download。

## 4. Task 3：为 active terminal cwd resolution 写失败测试

### 目标

先把“拖拽上传目标必须来自 active terminal cwd，而不是 SFTP panel path”的语义锁成失败测试。

### 涉及文件

- `src/shell/view_model/workspace.rs`
- `src/app/ssh/session_manager.rs`
- `src/app/bootstrap/sftp.rs`
- `tests/sftp_follow_cwd_spec.rs`
- `tests/bootstrap_smoke.rs`
- 如有必要新增 `tests/terminal_drop_upload_spec.rs`

### 先写的失败测试

1. active terminal session 有 cwd 时，terminal drop 目标目录取该 cwd。
2. quick browser 在 manual-browse 时，terminal drop 仍使用 terminal cwd，不使用 quick browser path。
3. 没有 active terminal session 时，terminal drop 直接失败。
4. cwd 未知时，terminal drop 直接失败，不 fallback `/`。

### 最小实现范围

- 只建立 cwd resolution 与 preflight 入口，不先接实际上传。

### 验证命令

```bash
cargo test sftp_follow_cwd_spec -- --nocapture
cargo test terminal_drop_upload_spec -- --nocapture
```

### 回滚风险

- 若 cwd source 选错，后续所有上传都可能传错目录。

### 不能顺手做的事情

- 不要在此阶段引入 `pwd` 注入或远端命令 fallback。

## 5. Task 4：为 drag upload target selection 写失败测试

### 目标

锁定 terminal surface、quick browser、workspace SFTP 三类 drop target 的 ownership 规则，防止同一份 dropped files 被错误消费。

### 涉及文件

- `src/app/bootstrap/windowing.rs`
- `src/shell/view_model/sftp.rs`
- 可能新增 terminal drop target hit-test 辅助模块
- `tests/bootstrap_smoke.rs`

### 先写的失败测试

1. pointer 在 quick browser 区域时，只触发现有 quick browser 上传。
2. pointer 在 active terminal surface 时，只触发 terminal-context upload。
3. 两者同时存在时，只有命中的 target 获胜。
4. hover cancel / repeated drop 不留下脏 overlay 状态。

### 最小实现范围

- 只补 hit-test 与 routing，不先做真实上传逻辑。

### 验证命令

```bash
cargo test native_windowing_bridge_wires_os_file_drop_events_into_sftp_callbacks -- --nocapture
cargo test external_sftp_drop_callbacks_toggle_overlay_and_queue_background_uploads -- --nocapture
cargo test terminal_drop_upload_spec -- --nocapture
```

### 回滚风险

- 若 ownership 不清晰，terminal drop 会误传到 quick browser path。

### 不能顺手做的事情

- 不要顺手改 quick browser 的既有拖拽产品语义。

## 6. Task 5：为 cwd deleted / missing 目录错误写失败测试

### 目标

把“当前终端目录不存在必须明确失败”锁成失败测试，防止未来实现阶段偷用 fallback。

### 涉及文件

- `src/app/bootstrap/sftp.rs`
- `src/app/ssh/runtime/sftp_backend.rs`
- `src/app/sftp/session_binding.rs`
- `tests/bootstrap_smoke.rs`
- `tests/terminal_drop_upload_spec.rs`

### 先写的失败测试

1. `path_exists(cwd) == false` 时提示“当前终端目录不存在”。
2. `path_exists(cwd) == true` 但 `read_dir(cwd)` 失败时，区分“非目录”或“权限不足”。
3. SFTP binding 已断开时提示“SFTP 不可用”或“连接已断开”。
4. 不允许 fallback 到 `/`、`HOME`、SFTP current path。

### 最小实现范围

- 只建立 preflight 校验与错误映射。

### 验证命令

```bash
cargo test terminal_drop_upload_spec -- --nocapture
cargo test bootstrap_smoke -- --nocapture cwd
```

### 回滚风险

- 若错误映射不清晰，用户会误以为文件已经开始上传。

### 不能顺手做的事情

- 不要顺手新增“自动创建远端 cwd”语义。

## 7. Task 6：复用 SFTP transfer queue 实现 drag upload 到 cwd

### 目标

在 Tasks 3-5 通过后，正式把 terminal-context drop 接到现有 SFTP queue。

### 涉及文件

- `src/app/bootstrap/windowing.rs`
- `src/app/bootstrap/sftp.rs`
- `src/shell/view_model/workspace.rs`
- `src/app/ssh/session_manager.rs`
- `src/app/sftp/local_ops.rs`
- `src/app/sftp/queue.rs`
- `tests/bootstrap_smoke.rs`
- `tests/terminal_drop_upload_spec.rs`

### 先写的失败测试

1. 单文件 terminal drop 成功入队并指向 cwd。
2. 多文件 / 目录 terminal drop 成功入队。
3. 空目录能按现有 SFTP queue 语义进入上传集合。
4. 中文、空格路径不丢失。

### 最小实现范围

- 复用 `schedule_sftp_upload_paths(...)`。
- 新增 terminal-context upload routing。
- 不重写 transfer engine。

### 验证命令

```bash
cargo test external_sftp_drop_callbacks_toggle_overlay_and_queue_background_uploads -- --nocapture
cargo test terminal_drop_upload_spec -- --nocapture
cargo test sftp_follow_cwd_spec -- --nocapture
```

### 回滚风险

- 若实现时绕开 queue，后续很难继承现有冲突、取消、进度语义。

### 不能顺手做的事情

- 不要顺手重构整个 SFTP queue 数据结构。
- 不要顺手改 quick browser follow mode 语义。

## 8. Task 7：进度、取消、失败提示与 transfer center 接线

### 目标

确保 terminal-context upload 在用户看来是统一、可见、可取消、可诊断的，而不是“后台默默发生”。

### 涉及文件

- `src/app/bootstrap/sftp.rs`
- transfer summary / transfer center 投影文件
- `src/shell/view_model/...` 中与 transfer 展示相关的模块
- `tests/bootstrap_smoke.rs`

### 先写的失败测试

1. terminal drop 上传后，transfer center 显示 queued/running。
2. 失败时能显示精准错误，而不是 generic failed。
3. cancel 后状态变更正确。
4. conflict 情况沿用现有冲突语义。

### 最小实现范围

- 只把 terminal-context upload 正确投影到现有 transfer UI。
- 不顺手重做 transfer center 信息架构。

### 验证命令

```bash
cargo test transfer_summary -- --nocapture
cargo test bootstrap_smoke -- --nocapture transfer
```

### 回滚风险

- 如果 UI 投影不一致，用户会误判上传状态。

### 不能顺手做的事情

- 不要顺手扩展全新的 terminal transfer center 分类页。

## 9. Task 8：多 session / 多 tab / drop ownership 回归测试

### 目标

锁定“拖拽必须绑定当前 active terminal，不得串 session”的回归面。

### 涉及文件

- `tests/bootstrap_smoke.rs`
- `tests/sftp_follow_cwd_spec.rs`
- `tests/terminal_drop_upload_spec.rs`
- 可能新增 workspace/session 交互测试

### 先写的失败测试

1. 两个 terminal tab 同时存在时，drop 只绑定 active tab。
2. active tab 切换后，drop 目标随之切换。
3. quick browser linked terminal 与 active terminal 不同时，terminal drop 仍绑定 active terminal。
4. 连接断开后，不允许继续使用旧 session snapshot 入队。

### 最小实现范围

- 只补回归锁定，不重写 session lifecycle。

### 验证命令

```bash
cargo test sftp_follow_cwd_spec -- --nocapture
cargo test terminal_drop_upload_spec -- --nocapture
cargo test bootstrap_smoke -- --nocapture session
```

### 回滚风险

- 多 session 串传是高严重度回归，必须有明确测试闸门。

### 不能顺手做的事情

- 不要顺手改全部 session manager 生命周期模型。

## 10. Task 9：Windows / Linux smoke 验证

### 目标

确认首发 drag-upload 方案在两个目标平台上至少具备基本正确性，且错误语义不依赖单一平台行为。

### 涉及文件

- 平台相关 drag/drop bridge
- 本地路径处理相关模块
- 如有必要补充平台 smoke 文档记录

### 先写的失败测试

- 若仓库已有平台 smoke harness，则补对应 case；否则以人工 smoke checklist 为主。

### 最小实现范围

- 覆盖：
  - 本地中文/空格路径拖拽
  - 目录拖拽
  - cwd 已删除时错误提示
  - transfer center 进度可见性

### 验证命令

```bash
cargo test -- --nocapture
```

人工 smoke 记录必须至少包含：

```text
平台：Windows / Linux
会话类型：SSH terminal
场景：文件拖拽 / 目录拖拽 / cwd 删除 / 权限不足 / 取消
结果：通过 / 失败
证据：精确命令、截图或日志摘要
```

### 回滚风险

- 若只在单平台验证，路径与文件系统语义问题容易漏出。

### 不能顺手做的事情

- 不要把 smoke 变成大范围 UI 改版。

## 11. Task 10：最终 verification handoff

### 目标

在声称完成前，按 `superpowers:verification-before-completion` 做最后一轮证据化验证，并形成可审计 handoff。

### 涉及文件

- 本轮所有实现改动文件
- 最终 handoff 说明文档或 PR 描述

### 先写的失败测试

- 不新增功能测试；重点在“所有必要验证命令均已运行并记录输出”。

### 最小实现范围

最终 handoff 必须至少记录：

1. worktree 创建命令与输出
2. 每个关键测试命令与输出
3. 若有人工 smoke，记录平台、步骤、结果
4. `git status --short`
5. 任何未解决问题、跳过项、平台差异

### 验证命令

建议最少包含：

```bash
git status --short
git diff --stat
cargo test terminal_drop_upload_spec -- --nocapture
cargo test sftp_follow_cwd_spec -- --nocapture
cargo test bootstrap_smoke -- --nocapture
```

若进入 Phase 2，还需补：

```bash
cargo test ssh_zmodem_detection_spec -- --nocapture
```

### 回滚风险

- 如果 handoff 只写“已通过”，后续无法复查真实验证范围与输出。

### 不能顺手做的事情

- 不能在 verification 阶段再顺手改需求、扩范围、补 unrelated refactor。
