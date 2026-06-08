# New Tab Multi-Session + Terminal Selection Designs

日期: 2026-06-08
执行者: Codex
状态: 设计已收敛，待进入独立 worktree 实现

## 1. 调研方法与可信度说明

本轮结论同时来自三类证据：

1. **本地代码核查**：直接检查 `src/`、`ui/`、`tests/` 当前实现。
2. **外部官方资料调研**：覆盖 Windows Terminal、iTerm2、Xshell、Termius、VS Code Terminal/Remote SSH，并补充 xterm.js、WezTerm、Ghostty 作为 terminal 交互语义参考。
3. **真实 subagent 并行调研**：本轮实际使用了两个真实 subagent（`Plato`、`Chandrasekhar`）并行收集产品行为与 terminal 交互证据。

限制说明：

- 当前 subagent 工具接口没有暴露模型选择参数，因此**无法证明已精确锁定到 `gpt-5.4 xh`**。
- 但当前环境**支持真实 subagent**，本轮不是伪造的“假子代理”；后文会明确区分“subagent 结论”与“本地多角色评审结论”。

## 2. 外部产品对标结论

### 2.1 重复打开同一 SSH 目标的主流语义

| 产品 | 再次打开同一目标的默认语义 | 与“跳到已有 tab”是否分离 | 备注 |
| --- | --- | --- | --- |
| Windows Terminal | 新 tab / 新 profile 实例 | 是 | `newTab`、`duplicateTab` 与 `switchToTab`/tab search 分离 |
| iTerm2 | 新 tab / 新 session | 是 | `Profiles` 打开与 `Open Quickly` 跳已有 session 分离 |
| Xshell | 新 tab 或新 window | 是 | 官方文档明确写到重复连接会开新 tab/新窗口 |
| Termius | 更接近新 tab / 新 session | 是 | 文档/博客区分“start a new connection”和“jump to a tab” |
| VS Code Integrated Terminal | 新 terminal instance | 是 | `New Terminal`、`Move to New Window` 与切换 terminal 分离 |
| VS Code Remote SSH | 默认新 remote window；具体 remote folder 可复用已有 window | 是 | remote host 打开与已有 window/folder 定位是不同动作 |

设计结论：**launcher / New Tab 的“打开连接”动作，应默认建新 session；切到已有 session 必须是单独动作，而不是隐式副作用。**

### 2.2 Terminal selection 的主流语义

| 产品 | 双击 | 三击 | mouse reporting 下本地选择逃生门 | 词边界模型 |
| --- | --- | --- | --- | --- |
| Windows Terminal | word | line | `Shift` | delimiter-only，可配置 `wordDelimiters` |
| iTerm2 | word / smart selection（可配） | line / full wrapped lines（可配） | `Option` | macOS word + smart selection 规则 |
| Xshell | word | line | `Shift` | delimiter-only，可配置 |
| VS Code / xterm.js | word | line | VS Code: `Alt`；xterm.js 默认 `Shift` | hybrid：word separators + link aware |
| WezTerm | word | line | `Shift` | 较宽松的 token 边界，支持 semantic zone |
| Ghostty | delimiter-based word | 行级选择能力 | 可配置保留 `Shift` | codepoint-based word chars |

设计结论：

- 本项目应优先采用 **`Shift` 作为 mouse reporting 下的本地 selection override**，因为：
  - 用户明确允许“必要时保留 Shift 强制本地选择语义”；
  - Windows Terminal / Xshell / xterm.js / WezTerm 与传统 terminal 生态更接近；
  - 本项目不是 IDE 内嵌 terminal，没必要强跟 VS Code 的 `Alt` 方案。
- 双击边界不能完全依赖最激进 delimiter；否则 URL/path/CJK 会被切坏。

## 3. 本地代码实情与设计边界

### 3.1 Launcher 路径当前确实在复用旧 session

当前 launcher surface 相关入口都落在 `OpenSessionMode::ActivateExisting`：

- `src/app/bootstrap.rs`
  - `on_welcome_quick_launch_connect_requested(...)`
  - `activate_saved_ssh_picker_asset(...)`
  - `open_saved_ssh_asset_from_quick_launch(...)`
- `src/app/ssh/session_manager.rs`
  - `open_session(..., OpenSessionMode::ActivateExisting)` 会按 `asset_id` 查 `registry.asset_sessions`

但与此同时：

- 资产树直接激活 SSH 已经走 `OpenSessionMode::ForceNewTab`

所以今天的问题不是“整个产品统一要求复用旧 session”，而是**launcher surface 还停留在较早的 asset-level reuse 语义**。

### 3.2 Terminal selection 不是“完全没有 renderer 参与”，而是“实现分叉”

当前真实状态：

- `ui/shell/terminal-session-host.slint` 维护 selection drag 状态；
- `src/app/bootstrap.rs` 能把 host selection 投影成 `TerminalAtlasSelection`；
- `src/app/terminal_atlas.rs` 的 row hash 已经支持 selection 参与；
- native renderer path 已经有 selection overlay/damage；
- **但 workspace bitmap 路径当前偏向 host overlay**，selection 还没有成为统一的、跨 render mode 的单一真相。

因此本轮设计不是“从零发明 selection renderer”，而是：

1. 补齐双击/三击 selection controller；
2. 把 selection authority 从“Slint 局部拖拽状态”升级为“Rust/renderer/copy 都能消费的统一状态”；
3. 消除 bitmap/native 两条路径在 selection 可视化与失效刷新上的分叉。

## 4. 方案收敛

## 4.1 Launcher session 语义：只收敛 launcher surface，不全局推翻 SessionManager 默认值

### 候选方案 A：只改 `Recent Connections`

优点：

- 变更面最小；
- 对现有测试影响最小。

缺点：

- 同一个 launcher surface 内，`Recent Connections` 与 `Open Saved SSH` picker 会出现不同打开语义；
- 用户很难理解“都在 New Tab 里，为什么一个新开、一个复用”。

### 候选方案 B：把所有 saved SSH 打开入口都改成始终新建 session

优点：

- 心智最统一；
- 最彻底摆脱 `asset_id -> only one live session`。

缺点：

- 波及面更大；
- 会改写大量历史测试与旧设计契约；
- 本轮用户明确点名的是 launcher/recent，不是全产品 session 生命周期翻修。

### 采用方案 C：**launcher surface 全部统一为“Open As New Session”，但不全局改写所有入口**

覆盖范围：

- `New Tab / Launcher` 的 `Recent Connections`
- 从 launcher 进入的 `Open Saved SSH` picker

暂不覆盖：

- 非 launcher 的程序化入口
- reconnect/retry 语义
- 其它还依赖 `ActivateExisting` 的旧链路

设计原因：

- 对用户来说，`New Tab` surface 本身就应表达“再开一个 session”的心智；
- 对架构来说，只改 launcher intent，风险远小于推翻整个 `SessionManager` 默认值；
- 对当前代码来说，launcher 两条路径已经共享 `open_saved_ssh_asset_from_quick_launch(...)`，很适合通过 source/intent 做精确分流。
- 补充说明：`Plato` 在更抽象的产品信息架构层面倾向更窄的 A（只改 `Recent Connections`）。但回到本仓库现状，**资产树直接激活 SSH 已经是 `ForceNewTab`**，因此若继续让 launcher 内 `Open Saved SSH` picker 保持 `ActivateExisting`，会让 launcher/picker 与资产树语义再次分裂。基于本地代码现状，方案 C 比纯 A 更自洽，同时又明显小于“全产品所有入口都改写”的 B。

## 4.2 Launcher 打开意图建模

建议增加明确的 launcher/open intent，而不是继续让 UI 直接传 `OpenSessionMode`：

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavedSshOpenSource {
    LauncherRecent,
    LauncherSavedSshPicker,
    AssetsTree,
    ContextMenu,
    Reconnect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SavedSshOpenBehavior {
    ActivateExisting,
    OpenAsNewSession,
}
```

launcher surface 的映射规则：

- `LauncherRecent` -> `OpenAsNewSession`
- `LauncherSavedSshPicker` -> `OpenAsNewSession`

桥接到当前运行时时：

- `OpenAsNewSession` -> `OpenSessionMode::ForceNewTab`
- `ActivateExisting` -> `OpenSessionMode::ActivateExisting`

好处：

- 把“来源”和“行为”显式化；
- 后续即使别的入口想切换语义，也不必再靠 scattered boolean 判断；
- `SessionManager` 本体可先保留现有模式，不必本轮强推全局破坏性变更。

## 4.3 Launcher 激活流程

目标流程：

```text
Launcher Recent / Launcher Saved SSH picker activation
  -> ShellViewModel / bootstrap semantic open request
  -> resolve saved SSH profile
  -> choose SavedSshOpenBehavior::OpenAsNewSession
  -> SessionManager.open_session(profile, ForceNewTab)
  -> new session_id
  -> merge_session_handle_into_tabs(...)
  -> launcher tab replaced if currently active
  -> record_recent_saved_ssh_asset(asset_id)
```

### 去重规则

允许的去重：

- 同一物理 gesture 在 UI 层重复投递时，仅保留一次真正的打开请求；
- launcher row 在连接请求刚发出、tab 尚未切换前，临时禁止再次激活同一 row。

不允许的去重：

- 按 `asset_id` 查 live session 然后 focus existing；
- 按 `host/user/port` 查已有 tab 然后 focus existing；
- 因为 Recent 是 MRU 卡片就隐式复用旧 tab。

### 推荐防抖/防双开策略

- Recent row 目前是 `clicked => activated` 型入口，物理双击可能导致两次 click；
- Saved SSH picker 则天然存在 `clicked` 与 `double-clicked` 双通道。

推荐采用**语义级 activation guard**：

- 先把 raw click/double-click 统一汇聚成 `activate_saved_ssh(asset_id, source)`；
- 在 bootstrap/view-model 中引入短生命周期的 `pending_launcher_activation`；
- 当同一 `asset_id + source` 在极短时间内重复触发、且第一次 activation 尚未出栈时，丢弃第二次。

## 4.4 Terminal selection：采用“Rust-owned selection truth + Slint mirror”的折中方案

### 不采用的方向

1. **继续让 selection 只保存在 Slint 局部状态里**
   - 问题：双击/三击规则、copy、bitmap/native renderer 难以统一；
   - 与当前已经存在的 `TerminalAtlasSelection` / renderer damage 机制脱节。

2. **一次性把所有 pointer/selection 全部重写成纯 Rust 控制器，Slint 只做像素容器**
   - 问题：本轮范围过大；
   - 容易连带破坏现有 link hover、context menu、scrollbar hit-testing。

### 采用方向

采用折中方案：

- **Rust 持有 selection authority**；
- **Slint 仍负责 pointer 采样、hover 和 host overlay 呈现镜像**；
- 双击/三击的“范围扩张逻辑”放到 Rust helper/controller；
- selection 最终可同时投影到：
  - copy pipeline
  - bitmap renderer / atlas selection
  - native renderer overlay/damage
  - Slint host overlay（若保留）

可采用的数据结构：

```rust
struct WorkspaceTerminalSelection {
    anchor: BufferPoint,
    focus: BufferPoint,
    mode: WorkspaceTerminalSelectionMode,
}

enum WorkspaceTerminalSelectionMode {
    Cell,
    Word,
    Line,
}

struct BufferPoint {
    row: u32,
    col: u32,
}
```

说明：

- `row/col` 使用 **buffer coordinates**，与现有 copy/scrollback 语义一致；
- Slint 的 `selection-start-row/col` 可以继续作为 mirror/export 属性，但不再是唯一真相。

## 4.5 Gesture controller 设计

### 事件层级

不要直接把 `click count` 散落进业务逻辑；先收敛成明确的语义事件：

```text
SingleClick(point, modifiers)
DoubleClick(point, modifiers)
TripleClick(point, modifiers)
DragStart(point, modifiers)
DragUpdate(point, modifiers)
DragEnd(point, modifiers)
```

### 处理规则

- `SingleClick`：
  - 无现有 selection 时，准备 cell/drag selection；
  - 有现有 selection 且点击落在非扩展语义时，可清空 selection。
- `DoubleClick`：
  - 调 `expand_to_token(point)`；
  - 进入 `Word` mode。
- `TripleClick`：
  - 调 `expand_to_visual_row(point)`；
  - 进入 `Line` mode。
- `DragUpdate`：
  - `Cell` mode 按 cell 扩展；
  - `Word` mode 按 token 边界扩展；
  - `Line` mode 按 visual row 扩展。

### Mouse reporting gate

默认规则：

```text
if mouse_grabbed && !shift:
    forward to remote app
else:
    handle locally
```

即：

- 普通 click/drag/double/triple click：远端优先
- `Shift + click/drag/double/triple click`：本地 selection 优先

## 4.6 双击 token 边界：v1 采用“实用 token”而非全量 smart selection

本轮不直接做 iTerm2 级 regex/precision smart selection；但也不能退回“Windows Terminal 默认分隔符把 URL/path 切碎”的保守模型。

建议 v1 按以下优先级扩张：

### 优先级 1：已知 URL/path/file-like token

如果命中点位落在当前已有 link detection / openable range 上：

- 直接选择整段 URL/path/file-like token
- 避免把 `https://host/path`、`/var/log/app.log`、`src/main.rs:10:5` 切碎

### 优先级 2：shell-ish plain token

若不是 link/path，按较宽松的 token 字符集合扩张：

```text
A-Z a-z 0-9 _
. / \ : @ % + = ~ - #
```

目的：

- 让常见 shell token、路径片段、镜像名、`user@host`、`foo-bar`、`key=value` 能整体被选中。

### 优先级 3：CJK 连续非空白片段

- 若命中 CJK 文本，则选择连续非空白片段；
- 不要求 v1 做更复杂的中文分词。

### 优先级 4：fallback non-whitespace run

- 处理不属于以上集合、但用户仍期望“一次双击能选到一段可复制符号串”的场景。

### 宽字符与 trailing cell 规则

- 命中宽字符 trailing cell 时，先归一到 leading cell；
- `width=0` 的 trailing cell 不能单独成为 boundary；
- emoji / 多 codepoint cluster 若当前 surface 以单 terminal cell cluster 表达，也应整体选中，不得裂开复制。

## 4.7 三击行选择：v1 固定 visual row

业界存在两派：

- visual row
- wrapped logical line

本轮按用户目标与风险控制，**固定为 current visual row**：

- 双击/三击行为更可预测；
- 不需要立刻处理 wrapped logical line 的跨行聚合与复制规范；
- 便于先补齐 bitmap/native render mode 下的可视化一致性。

若后续要扩张到 wrapped logical line，应另开新需求，而不是在本轮隐式加码。

## 4.8 渲染与复制投影

### 当前真实问题

- native path 已有 selection overlay/damage
- atlas renderer 已支持 selection 进入 row hash
- 但 workspace bitmap path 还存在 host overlay 分叉

### 设计目标

无论最终渲染模式为何，selection 都要满足：

1. **可见**：用户立刻看到选区；
2. **可复制**：复制拿到与屏幕一致的文本；
3. **可失效刷新**：selection 改变后必然触发对应 repaint/invalidation。

### 推荐落地方式

优先推荐把 workspace bitmap path 也喂给现有 renderer selection 输入，而不是继续依赖纯 Slint overlay：

```text
WorkspaceTerminalSelection
  -> TerminalAtlasSelection / TerminalSelectionModel projection
  -> bitmap/native renderer consume same range
  -> row hash / overlay damage / repaint stay coherent
```

如果实现阶段因为 host/native surface 约束需要短期保留 Slint overlay，也必须满足：

- Rust selection truth 仍然存在；
- bitmap/native 的 repaint trigger 明确依赖 selection 变化；
- 不能再出现“屏幕没变，但复制有选区”的状态分裂。

### 复制链路

继续复用现有 `selection_text_from_buffer_rows(...)` / `normalize_selection_hit_col(...)` 的基础能力，但要确保新语义也走同一条链：

- 双击/三击最终产出 buffer-row selection range；
- `Ctrl+Shift+C` 与右键 `Copy` 统一读该 range；
- 行选择复制 visual row 文本时，继续 trim trailing padding，不重复复制 wide-char trailing cell。

## 5. 多角色评审与收敛结论

### 5.1 真实 subagent 结论摘要

- `Plato`：
  - 业界 New Tab / launcher 更常见“新建 tab / 新建 session”；
  - “跳到已有 tab”通常是单独动作；
  - Xshell、iTerm2、Windows Terminal 在这点上都比当前 launcher 的 `ActivateExisting` 更接近“新开”。
- `Chandrasekhar`：
  - `mouse reporting` 下必须保留稳定的本地 selection 逃生门；
  - `Shift` 是更贴近传统 terminal 的默认；
  - token 边界不应只靠保守 delimiter，URL/path/CJK/wide-char 必须单独照顾。

### 5.2 本地多角色辩论结论

#### 产品视角

- `New Tab` 的核心价值是“我再开一个连接”，不是“帮我找回旧连接”。
- 同一 launcher 内 Recent 和 Saved SSH picker 若语义不同，会形成新的认知冲突。

#### 架构视角

- 当前 launcher 两条路径共享 quick-launch open 管线，适合在 source/intent 层切换到 `ForceNewTab`。
- 本轮不应全局废除 `ActivateExisting`，否则会把问题从 launcher 修复升级为全产品 session 生命周期重写。

#### 终端视角

- 双击/三击如果只做 Slint 层视觉 hack，最终一定会在 copy、renderer damage、render mode 切换上回归。
- 应该优先建立 Rust-owned selection truth，再投影给 UI/renderer。

#### QA 视角

- launcher 路径必须先锁失败测试，再改实现；否则很容易被旧 `ActivateExisting` 契约和 shared helper 吞回去。
- terminal selection 必须覆盖：ASCII、URL/path、CJK、wide-char trailing cell、mouse-grabbed + Shift override、bitmap/native 两条渲染路径。

### 5.3 最终设计结论

1. **Launcher surface 统一 new session**：`Recent Connections` 与 launcher 内 `Open Saved SSH` picker 都改成 `OpenAsNewSession`。
2. **SessionManager 默认值暂不全局翻案**：只在 launcher source 上显式走 `ForceNewTab`。
3. **Terminal selection v1 收敛**：
   - 双击选实用 token
   - 三击选 visual row
   - `Shift` 强制本地 selection
   - 不在本轮扩张为 full smart selection / wrapped logical line
4. **Selection authority 升级**：从 Slint 局部状态升级为 Rust-owned selection truth，bitmap/native 渲染和 copy 使用同一份范围语义。

## 6. 不采用的方案

- **不采用“只改 Recent，不改 launcher picker”**：因为同一 launcher 内会出现语义分裂。
- **不采用“本轮全产品彻底取消 ActivateExisting”**：范围过大、历史契约过多。
- **不采用“v1 直接上 iTerm2 级 smart selection + wrapped logical line”**：收益高，但测试爆炸与回归风险过大。
- **不采用“继续保留 selection 仅在 Slint 本地 state 中”**：与当前 renderer/copy 架构方向冲突。

## 7. 参考资料

### 本地代码

- `src/app/bootstrap.rs`
- `src/app/ssh/session_manager.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/ssh/runtime/contracts.rs`
- `src/app/terminal_atlas.rs`
- `src/app/terminal_model.rs`
- `ui/shell/terminal-session-host.slint`
- `ui/welcome/welcome-view.slint`
- `ui/welcome/quick-launch-card.slint`
- `ui/components/open-saved-ssh-modal.slint`
- `tests/bootstrap_smoke.rs`
- `tests/ssh_session_manager_spec.rs`
- `tests/quick_launch_projection_spec.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- `tests/terminal_atlas_renderer_spec.rs`

### 外部资料

- Windows Terminal
  - https://learn.microsoft.com/en-us/windows/terminal/selection
  - https://learn.microsoft.com/en-us/windows/terminal/customize-settings/interaction
  - https://learn.microsoft.com/en-us/windows/terminal/tutorials/new-tab-same-directory
- iTerm2
  - https://iterm2.com/documentation-general-usage.html
  - https://iterm2.com/documentation-preferences-general.html
  - https://iterm2.com/documentation-smart-selection.html
  - https://iterm2.com/documentation-menu-items.html
- Xshell
  - https://netsarang.atlassian.net/wiki/spaces/ENSUP/pages/2237305221/Multi-session+Handling
  - https://www.xshell.com/en/xshell-all-features/
  - https://cdn.netsarang.net/docs/Xshell8_manual.pdf
- Termius
  - https://docs.termius.com/terminal/workspaces
  - https://docs.termius.com/organize-and-connect-to-hosts/groups-and-tags
  - https://docs.termius.com/changelog
  - https://termius.com/blog/termius-x
- VS Code / Remote SSH
  - https://code.visualstudio.com/docs/terminal/basics
  - https://code.visualstudio.com/docs/remote/ssh
  - https://github.com/microsoft/vscode/issues/189891
- 终端语义参考
  - https://github.com/xtermjs/xterm.js/blob/master/src/browser/services/SelectionService.ts
  - https://wezterm.org/config/mouse.html
  - https://wezterm.org/config/lua/config/selection_word_boundary.html
  - https://ghostty.org/docs/config/reference
