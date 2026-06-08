# New Tab Multi-Session + Terminal Selection Requirements

日期: 2026-06-08
执行者: Codex
状态: 需求已收敛，待进入独立 worktree 实现

## 1. 背景

本轮只定义两个用户感知明显、且已经有本地代码证据支持的问题：

1. `New Tab / Launcher` 的 `Recent Connections`（以及同一 launcher surface 内的 `Open Saved SSH` picker）再次打开同一 SSH 资产时，当前实现会优先复用已有 session，而不是创建新的 terminal session tab。
2. SSH terminal host 当前只有基础拖拽选择；双击选词、三击选行、`mouse reporting` 下的本地 selection 逃生门、以及 selection 在不同渲染路径下的一致可视化仍不完整。

## 2. 本地现状基线

### 2.1 Launcher 重复打开语义现状

本地代码核查结论：

- `src/app/bootstrap.rs` 中：
  - `on_welcome_quick_launch_connect_requested(...)` 走 `open_saved_ssh_asset_from_quick_launch(..., OpenSessionMode::ActivateExisting)`。
  - `activate_saved_ssh_picker_asset(...)` 也走 `OpenSessionMode::ActivateExisting`。
- `src/app/ssh/session_manager.rs` 中：
  - `OpenSessionMode::ActivateExisting` 会按 `asset_id` 查询 `asset_sessions` 并复用 live session。
  - `OpenSessionMode::ForceNewTab` 已存在，且会创建新的 `session_id`。
- `src/app/bootstrap.rs` 中：
  - 资产树直接激活 SSH 时，已经走 `OpenSessionMode::ForceNewTab`；说明当前产品内部本来就存在“launcher 复用、资产树新开”的不一致。
- 现有测试已锁定当前 launcher/recent 的复用行为：
  - `tests/bootstrap_smoke.rs` 中 `active_recent_connection_row_returns_to_existing_tab_without_duplicate_session`
  - `tests/ssh_session_manager_spec.rs` 中多处 `ActivateExisting` 复用契约

### 2.2 Terminal selection 现状

本地代码核查结论：

- `ui/shell/terminal-session-host.slint` 当前以 Slint 本地属性维护 `selection-*` 和 drag 逻辑；支持拖拽选区、复制、右键菜单，但没有成型的双击/三击选择语义。
- `mouse_grabbed=true` 时，左键 down/move/up 会直接转发给远端；当前没有 `Shift` 强制本地选择的 override。
- `src/app/bootstrap.rs` + `src/app/terminal_atlas.rs` 表明：
  - native renderer 路径已经可以消费 `TerminalAtlasSelection`，selection 也会参与 row hash / damage；
  - 但 workspace 的 bitmap 路径当前仍偏向 host overlay，不是完全统一的 renderer-driven selection。
- `src/app/terminal_model.rs` 已有 `selection: Option<TerminalSelectionModel>` 字段，但当前 `from_surface()` 仍写死为 `None`，说明 selection 尚未成为完整统一的 renderer/model 真相。
- `src/app/ssh/runtime/contracts.rs` 已具备：
  - 宽字符 trailing cell 命中归一；
  - selection end exclusive 边界；
  - 复制文本时去除 trailing padding；
  - 因此本轮不是从零开始，而是需要把“词/行扩展语义 + 渲染/复制统一链路”补齐。

## 3. 调研结论摘要

外部调研至少覆盖了 Windows Terminal、iTerm2、Xshell、Termius、VS Code Terminal/Remote SSH，并补充参考了 xterm.js、WezTerm、Ghostty。

结论摘要：

- 重复打开同一连接目标时，成熟桌面终端更常见的是“新建 tab / 新建 session”，而“跳到已有 tab”通常是单独动作，不与 `New Tab`/launcher 打开语义混用。
- `mouse reporting` 下，本地 selection 几乎都会保留一个明确的 modifier 逃生门；最常见的是 `Shift`（Windows Terminal、Xshell、xterm.js/WezTerm 默认），也有 `Alt/Option` 派（VS Code、iTerm2）。
- 双击词边界不能只靠最保守的分隔符集合，否则 URL/path/file:line:col/CJK 体验会很差；成熟实现通常至少对 URL/path/token 做特殊处理或宽松处理。
- 三击选行在业界存在“visual row”与“wrapped logical line”两派；本轮按用户目标先收敛到 `visual row`，不在 v1 扩张成 wrapped logical line。

## 4. Scope

### 4.1 本轮 in-scope

本轮只规划并实现两个交互修复：

1. **Launcher surface 重复打开同一 SSH 资产时必须新建 session**
   - 覆盖 `New Tab / Launcher` 内的 `Recent Connections`
   - 覆盖从该 launcher 进入的 `Open Saved SSH` picker
   - 不要求本轮改写所有非 launcher 入口

2. **SSH Terminal 双击/三击选择能力**
   - 双击选择当前 token
   - 三击选择当前 terminal visual row
   - selection 必须可见、可复制，并在 `mouse reporting`/TUI 场景下可保留本地 override

### 4.2 本轮非目标

- 不引入 OS 级多窗口架构；本轮“新开一个窗口”的产品语义明确为“新建 workspace terminal session tab/page”。
- 不重写 `SessionManager` 的全部 session 生命周期模型。
- 不把所有 saved SSH 入口一口气统一改成新建 session；只先收敛 launcher surface。
- 不在 v1 实现完整 iTerm2 风格 smart selection、semantic zone selection、quadruple click selection。
- 不实现 block/column selection。
- 不重构 SFTP、Vault/Sync、Tunnel/Proxy、全局 workspace 架构。
- 当前阶段不修改业务代码，只产出需求/设计/任务文档。

## 5. 功能需求：Launcher 必须新建 session

### R1. Launcher Recent 激活行为

当 `New Tab / Launcher` 中存在 SSH 资产 `A`，且当前工作区已经存在 `A` 的活动 session 时：

- 单击 `Recent Connections` row/card 打开 `A`
- 键盘激活 `Recent Connections` 中的 `A`

系统必须创建新的 SSH session，而不是 focus、接管或复用旧 session。

### R2. Launcher Saved SSH Picker 激活行为

当用户从同一 launcher surface 进入 `Open Saved SSH` picker，并激活 SSH 资产 `A` 时：

- 双击 row 打开 `A`
- `Enter` / `Open` 打开当前选中的 `A`

系统同样必须创建新的 SSH session，而不是复用旧 session。

### R3. 每次有效激活都生成新的 `session_id`

对于 launcher surface 发起的每次有效打开动作：

- 必须生成新的 `session_id`
- 必须创建新的 workspace terminal tab/page
- 不得让 `asset_id` 充当 live session 唯一键

正确模型：

```text
asset_id: A
session_id: S1, S2, S3 ...
```

错误模型：

```text
asset_id: A -> 只允许一个 live terminal tab
```

### R4. Launcher tab 接管语义

如果当前 active tab 本身就是 launcher：

- 新开的 SSH session 可以接管当前 launcher tab
- 但不得接管或 focus 已存在的 terminal session tab

### R5. Gesture 去重

同一次用户 gesture 最多只能创建一个新 session：

- `Recent Connections` 的单击 connect 不能因为双击、重复投递或 launcher tab 关闭时序而打开两个 session
- Saved SSH picker 的 `clicked + double-clicked` 组合也不能造成双开

### R6. Recent 数据更新

新 session 发起成功后：

- `record_recent_saved_ssh_asset(asset_id)` 仍应更新 `opened_at`
- Recent 列表长度和 MRU 规则保持现有 productized 约束
- Recent 的“connected”展示仍可按 `asset_id` 聚合显示，但这不应反向决定 session 是否复用

### R7. 非 launcher 入口保持显式现状

本轮不要求修改以下入口的最终语义，除非实现时发现与 launcher 共享路径而无法安全隔离：

- 非 launcher 的程序化打开路径
- 与 reconnect/retry 相关的路径
- 非 launcher 的其它上下文菜单行为

若实现阶段发现共享路径不可分离，必须先补需求说明，再进入代码改动。

## 6. 功能需求：Terminal 双击/三击选择

### R8. 双击选 token

在 terminal viewport 内双击任意 cell 时：

- 若命中普通 ASCII shell token，选择整个 token
- 若命中 URL/path/file-like token，选择整个 URL/path/file token，而不是被 `/ . : - @` 轻易切碎
- 若命中连续 CJK 文本，选择连续非空白片段
- 若命中空白区域，不产生有效 selection
- 若命中宽字符 trailing cell，必须归一到 leading cell 所属字符/cluster

### R9. 三击选当前 visual row

在 terminal viewport 内连续三击任意 cell 时：

- 选择当前 **visual terminal row**
- 复制该选区时，去掉 filler/trailing padding
- 保留行内真实内容空格

### R10. Selection 可见且可复制

只要存在有效选区：

- 选区必须可见
- `Ctrl+Shift+C` 必须复制选区文本
- terminal 右键菜单的 `Copy` 必须复制选区文本
- 复制输出必须遵守 terminal grid 顺序
- 宽字符 trailing cell 不能重复复制

### R11. Selection 渲染一致性

selection 的实现不能只停留在某一条渲染路径的局部视觉状态：

- 选区变化必须让实际渲染结果发生更新
- bitmap render mode 与 native render mode 都必须正确显示 selection
- selection 的变化必须参与相应的 invalidation / repaint 判定，而不是依赖偶然的 surface seqno 变化

### R12. Mouse reporting / TUI 兼容

当 remote app 开启 `mouse reporting`，或 surface 表现为 `mouse_grabbed=true` 时：

- 默认仍优先保留远端 mouse input 行为
- `Shift + drag`、`Shift + double-click`、`Shift + triple-click` 必须强制进入本地 selection 语义
- 不得破坏 `vim`、`less`、`tmux`、`htop` 等 TUI 场景

### R13. Selection 与现有复制/滚动边界兼容

- 本地 scrollback selection 仍应使用 buffer row 语义
- 选区在 viewport 改变、surface 尺寸变化或 alt-screen 切换时，必须有明确清空/失效规则，不能留下肉眼不可见但可复制的脏状态
- 词/行 selection 与 drag 扩展组合时，扩展语义必须稳定：
  - 双击后拖拽按 token 扩展
  - 三击后拖拽按 row 扩展

## 7. 验收标准

### 7.1 Launcher

- 同一 SSH 资产已经连接时，再次从 launcher 的 `Recent Connections` 打开，会得到新的 `session_id` 与新的 terminal tab/page。
- 同一 SSH 资产已经连接时，再次从 launcher 的 `Open Saved SSH` picker 打开，也会得到新的 `session_id`。
- launcher tab 可被新 session 接管，但旧 terminal session 不会被 focus-existing 复用。
- 一次物理双击 Recent row 最多只产生一次新 session。

### 7.2 Terminal selection

- 双击 `hello-world/path.txt` 任意字符，能一次选中整个 token。
- 双击连续 CJK 文本任意字符，能选中整个连续片段。
- 三击某一 visual row，能选中该 row，并复制出正确文本。
- 命中宽字符 trailing cell 时，选区与复制都不会破碎或重复。
- `mouse_grabbed=true` 时，普通双击仍交给远端；`Shift + double-click` 能触发本地 selection。
- bitmap/native render mode 下 selection 都可见。
