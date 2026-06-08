# New Tab Multi-Session + Terminal Selection Tasks

日期: 2026-06-08
执行者: Codex
状态: 任务已拆分，待在独立 worktree 中执行

## 0. 执行前置约束

当前窗口只产出文档，不允许直接改业务代码。

正式实现必须：

1. 新开一个窗口 / 新会话。
2. 建立独立 worktree，例如：

```bash
git worktree add .worktrees/feature-new-tab-multi-session-terminal-selection -b feature/new-tab-multi-session-terminal-selection
```

3. 在该 worktree 中执行实现。
4. 执行过程中必须遵守以下 superpowers 约束：
   - 总执行框架：`superpowers:executing-plans`
   - 每个实现 task 开始前：`superpowers:test-driven-development`
   - 测试、导出或行为异常时：`superpowers:systematic-debugging`
   - 声称完成前：`superpowers:verification-before-completion`
5. 最终 handoff 必须记录**精确命令与实际输出**，不能只写“已通过”。

## 1. Task 0：实现会话准备与文档冻结

### 目标

在独立 worktree 中锁定本轮需求、设计、任务文档，并确认当前实现范围只覆盖：

- launcher surface 新建 session
- terminal 双击/三击 selection

### 操作

- 新窗口进入 `.worktrees/feature-new-tab-multi-session-terminal-selection`
- 打开并确认本轮文档：
  - `docs/plans/2026-06-08-new-tab-multi-session-terminal-selection/requirements.md`
  - `docs/plans/2026-06-08-new-tab-multi-session-terminal-selection/designs.md`
  - `docs/plans/2026-06-08-new-tab-multi-session-terminal-selection/tasks.md`
- 在实现 session 中使用 `superpowers:executing-plans` 建立 task-by-task 执行节奏

### 验收

- worktree 正确创建
- 实现窗口明确不在主工作区直接改代码
- 实现范围没有偷偷扩张到 OS 多窗口、block selection、semantic zone 等非目标

## 2. Task 1：先锁 launcher 多 session 回归测试

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

先把“launcher 再次打开同一 asset 必须新建 session”的行为锁成失败测试。

### 候选测试文件

- `tests/bootstrap_smoke.rs`
- `tests/ssh_session_manager_spec.rs`（只在需要补 mode/source 级别契约时扩展）
- `tests/quick_launch_projection_spec.rs`（若 recent 展示语义需要补充）

### 必测用例

1. **Launcher Recent 再次打开同一 asset 会新建 session**
   - 已存在 `asset_id=A, session_id=S1`
   - 打开 launcher
   - 从 `Recent Connections` 激活 `A`
   - 期望得到新 `session_id=S2`
   - `S1` 仍保留
   - active tab 指向 `S2`

2. **Launcher Saved SSH picker 再次打开同一 asset 会新建 session**
   - 已存在 `A -> S1`
   - 打开 launcher
   - 打开 `Open Saved SSH` picker
   - 激活 `A`
   - 期望得到 `S2`

3. **launcher tab 接管语义**
   - active tab 是 launcher 时，新 session 可替换 launcher tab
   - 不得 focus 旧 terminal tab

4. **gesture 去重**
   - 模拟 Recent row 的重复 click / double-trigger
   - 一次物理双击最多只创建一个新 session

### 需要重写/替换的现有测试

- `tests/bootstrap_smoke.rs` 中现有的 `active_recent_connection_row_returns_to_existing_tab_without_duplicate_session` 应改写为新建 session 语义

### 验收

- 新测试先失败，且失败原因确实指向当前 `ActivateExisting` 行为

## 3. Task 2：实现 launcher `OpenAsNewSession` intent

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

把 launcher surface 的两条入口统一切到“显式新建 session”语义，但不全局推翻 `SessionManager` 默认值。

### 候选改动文件

- `src/app/bootstrap.rs`
- `src/app/ssh/session_manager.rs`（仅在需要补更清晰的 mode/source 建模时改）
- `src/shell/view_model.rs`
- `src/shell/view_model/quick_launch.rs`
- `ui/welcome/welcome-view.slint`
- `ui/welcome/quick-launch-card.slint`
- `ui/components/open-saved-ssh-modal.slint`

### 实现要求

1. 引入 launcher source / open behavior 的显式建模，避免 scattered boolean。
2. `Recent Connections` 与 launcher 内 `Open Saved SSH` picker 都映射到 `OpenAsNewSession`。
3. 该行为最终落到 `OpenSessionMode::ForceNewTab` 或等价的新 session 路径。
4. 不允许通过 `target_session_id_for_asset(...)` 或 `registry.asset_sessions` 去 focus existing。
5. 保持 `record_recent_saved_ssh_asset(...)` 更新时间行为。
6. 保持 launcher tab 被接管的现有 UX。
7. 增加 activation guard，解决 row click / double-click / queued callbacks 导致的双开风险。

### 验收

- Task 1 的 launcher 测试转绿
- 不出现“一次双击 Recent row 打开两个 session”的新回归

## 4. Task 3：先锁 terminal 双击/三击 selection 失败测试

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

在改交互前，先把 selection 语义测试锁死。

### 候选测试文件

- `tests/ssh_terminal_interaction_spec.rs`
- `tests/bootstrap_smoke.rs`
- `tests/terminal_atlas_renderer_spec.rs`
- 必要时增加新的 interaction/controller spec

### 必测用例

1. **双击 shell/path token**
   - 文本：`hello-world/path.txt`
   - 任意字符双击后，选择整个 token

2. **双击 URL/file-like token**
   - 文本：`https://example.com/a/b?x=1`
   - 期望不要在 `://`、`/`、`.` 等处被意外切碎

3. **双击连续 CJK 文本**
   - 选择整个连续非空白片段

4. **三击 visual row**
   - 选中当前 visual row
   - 复制结果去掉 trailing padding

5. **宽字符 trailing cell 命中归一**
   - 双击宽字符第二格，选区 anchor 归回 leading cell

6. **mouse grabbed 下的本地 override**
   - `mouse_grabbed=true` 时普通 double-click 继续给远端
   - `Shift + double-click` 强制本地 selection

7. **不同 render mode 的可视化更新**
   - bitmap/native 两种模式下 selection 改变都触发有效 repaint / visible diff

### 验收

- 新测试先失败，且失败点清楚暴露出“当前无 double/triple click 语义”和“mouse grabbed 无 Shift override”的问题

## 5. Task 4：实现统一的 selection gesture controller

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

把 raw pointer/click 事件收敛成稳定的 selection gesture 语义。

### 候选改动文件

- `ui/shell/terminal-session-host.slint`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/ssh/runtime/contracts.rs`
- 如有必要新增 selection helper/controller 模块

### 实现要求

1. 引入 `SingleClick / DoubleClick / TripleClick / Drag*` 级别的语义事件。
2. `DoubleClick -> expand_to_token(point)`。
3. `TripleClick -> expand_to_visual_row(point)`。
4. `DoubleClick + drag` 按 token 扩展。
5. `TripleClick + drag` 按 row 扩展。
6. `mouse_grabbed && !shift` 时维持远端 mouse input；`shift` 时切回本地 selection。
7. 不能破坏现有 link hover / context menu / local scrollback 行为。

### 验收

- Task 3 的 interaction 语义测试转绿
- `vim/tmux/less` 一类场景下普通 mouse reporting 未被破坏

## 6. Task 5：把 selection authority 升级为 Rust-owned truth，并统一渲染投影

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

消除 selection 只在 host/slint 层局部生效的分叉，让复制、bitmap/native renderer、无效刷新使用同一份 selection 语义。

### 候选改动文件

- `src/app/bootstrap.rs`
- `src/app/terminal_atlas.rs`
- `src/app/terminal_model.rs`
- `src/app/terminal_renderer/host.rs`
- `src/app/terminal_renderer/damage.rs`
- `ui/shell/terminal-session-host.slint`

### 实现要求

1. 建立 Rust-owned 的 `WorkspaceTerminalSelection` 真相状态。
2. 现有 Slint `selection-*` 属性退化为 mirror/export，不再是唯一 authority。
3. 把 selection 投影给 bitmap/native renderer；至少要保证：
   - selection 变化后一定发生 repaint
   - 两种 render mode 都可见
4. 若继续保留 host overlay，也必须让 repaint/invalidation 明确依赖 selection 变化。
5. 评估是否启用/接通 `TerminalModelFrame.selection`，避免继续让 model 层永久 `None`。

### 验收

- bitmap/native 两条渲染路径均有稳定 selection 可视化
- selection 改变不依赖偶然的 surface seqno 才更新

## 7. Task 6：复制链路与边界契约补齐

### 使用 superpowers

- 先用 `superpowers:test-driven-development`

### 目标

让新的词/行 selection 语义走稳定复制链路。

### 候选改动文件

- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/ssh/runtime/contracts.rs`
- `src/app/ssh/runtime/terminal.rs`
- `tests/ssh_terminal_interaction_spec.rs`

### 实现要求

1. `Ctrl+Shift+C` 复制当前 selection。
2. 右键 `Copy` 复制当前 selection。
3. 无 selection 时保持现有 copy 行为。
4. 多行 selection 使用 `\n` 拼接。
5. line selection 复制当前 visual row 文本。
6. wide-char trailing cell 不得重复复制。

### 验收

- 复制文本与屏幕选区一致
- CJK / wide-char / path / URL 不出现裂开或重复字符

## 8. Task 7：集成验证、回归验证与最终记录

### 使用 superpowers

- 如出现测试或行为异常，先用 `superpowers:systematic-debugging`
- 准备完成前必须用 `superpowers:verification-before-completion`

### 建议验证命令

```bash
cargo fmt --check
cargo test --test bootstrap_smoke
cargo test --test ssh_session_manager_spec
cargo test --test quick_launch_projection_spec
cargo test --test ssh_terminal_interaction_spec
cargo test --test terminal_atlas_renderer_spec
cargo clippy --all-targets --all-features -- -D warnings
```

若仓库已有更精确的 smoke/feature 过滤命令，以更精确的命令为准，但必须记录最终真实执行命令与输出。

### 最终 handoff 必须包含

1. 实际 worktree 路径
2. 实际执行的 superpowers 顺序
3. 实际运行的验证命令
4. 每条命令的精确输出摘要
5. 未完成项 / 风险 / 后续建议

