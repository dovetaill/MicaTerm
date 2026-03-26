# SSH 输入层级与显示层级设计

日期: 2026-03-26
方案名: `ssh-terminal-input-rendering`
状态: 已确认设计，已生成 implementation plan

## 背景

当前 SSH terminal 相关链路已经从纯文本占位推进到 `wezterm-term` surface 投影，但最近两次提交仅完成了部分接线：

- `4ba3d27` 收敛了 tab interaction 与 keepalive。
- `bce7de8` 新增了 cell/cursor surface、selection、mouse forwarding。

用户反馈暴露出四类仍未闭环的问题：

- 登录后出现 `Activate the web console with: systemctl enable --now cockpit.socket`，需要精确隐藏；
- 亮色模式下 terminal 只有已渲染内容区域带黑底，其余区域仍露出 workspace 默认浅色背景，视觉割裂；
- 输入层不完整，`Ctrl/Alt/Shift` 组合键、F 键、滚轮、鼠标拖拽、bracketed paste、resize 同步均未形成稳定 contract；
- terminal 视觉仍停留在过渡态，需要朝 VSCode editor 风格靠拢，但不能破坏 terminal 语义。

本设计只固化最终确认的架构与交互决策，不包含实现 patch。

## 目标

- 在 SSH 输出进入 terminal parser 之前，精确过滤掉指定横幅行；
- 将输入链路升级为 session-aware live encoder，而不是静态命名键映射；
- 让 terminal palette 跟随应用 light/dark theme，消除独立黑底；
- 补齐键盘、鼠标、滚轮、bracketed paste、resize 的可交互 contract；
- 将视觉风格收敛到接近 VSCode editor 的终端体验，使用 monospace 字体与更协调的 surface 体系；
- 保持后续迁移到完整 custom renderer 时的架构连续性。

## 非目标 / 边界

- 本轮不实现 `russh-sftp`、文件传输、远程目录树；
- 不扩展到多 pane、分屏、terminal split；
- 不在本轮引入“代码编辑器级语义高亮”，terminal 仍以远端 ANSI/TUI 语义为准；
- 不把“相邻色交错底色”作为首轮硬约束；
- 不生成 implementation plan，除非后续明确要求。

## 当前实现现状

### 1. Runtime 已具备 surface 投影基础，但输入编码仍是静态过渡方案

- `TerminalSurfaceState` 已包含 `visible_rows`、`cells`、`cursor`、`mouse_grabbed`、`bracketed_paste_enabled`：
  - `src/app/ssh/runtime.rs:45`
- `TerminalSession::surface_state()` 已从 `wezterm-term` 投影 cell/cursor：
  - `src/app/ssh/runtime.rs:830`
- `send_mouse_input()` 已经走 `wezterm_term::Terminal::mouse_event(...)`：
  - `src/app/ssh/runtime.rs:870`
- 但命名键编码仍通过 `encode_named_key_input()` 新建临时 `TerminalSession`，并把 `application_cursor_keys` 固定为 `false`：
  - `src/app/ssh/runtime.rs:850`
  - `src/app/ssh/runtime.rs:1060`

结论：

- mouse/runtime 基础不是空白；
- keyboard/input contract 仍停留在“能用一点”的中间态；
- 这也是 `vim`、`htop`、方向键和快捷键体验不完整的直接来源。

### 2. UI 已接上部分 terminal 事件，但覆盖面不足

- `TerminalSessionHost` 当前只处理：
  - 普通文本输入；
  - `Enter / Tab / Esc / Backspace / Delete`；
  - `Arrow / Home / End / PageUp / PageDown`；
  - `Ctrl+A / Ctrl+C / Ctrl+V`；
  - 左右键 pointer 事件。
- 对应位置：
  - `ui/shell/terminal-session-host.slint:271`
  - `ui/shell/terminal-session-host.slint:286`
  - `ui/shell/terminal-session-host.slint:361`

缺口：

- 没有 `F1-F24`；
- 没有 wheel / `scroll-event`；
- 没有中键；
- 没有基于 `bracketed_paste_enabled` 的 paste 包裹；
- 没有真正基于活动 session terminal mode 的键编码。

### 3. 亮色模式背景割裂来自“背景层缺失 + terminal palette 未主题化”两层问题

- terminal host 只给存在文本的 cell 画矩形，空白 cell 没有背景层：
  - `ui/shell/terminal-session-host.slint:451`
- terminal host 自身底色目前是固定浅色 surface，但 cell 默认背景仍来自 terminal palette：
  - `ui/shell/terminal-session-host.slint:354`
- `SessionTerminalConfig::color_palette()` 仍返回 `ColorPalette::default()`：
  - `src/app/ssh/runtime.rs:1102`
- shell theme token 已有完整 light/dark surface 体系：
  - `ui/theme/tokens.slint:3`

结论：

- 当前黑底不是简单“某个 Rectangle 颜色错了”；
- 根因是 terminal 默认 palette 与 app theme 完全脱节，同时空白区域未按终端逻辑铺满。

### 4. 横幅过滤当前完全缺失

- 远端输出现在直接 `advance_bytes(bytes)`，没有前置过滤：
  - `src/app/ssh/runtime.rs:769`

结论：

- 如果要做到“这行既不显示、也不进入选择/复制/scrollback”，必须放在 parser 前处理。

## 设计要点拆分

### 设计要点 1：SSH 横幅过滤位置

候选方案：

- 方案 A：在 terminal parser 前做 exact-match 行过滤；
- 方案 B：在 surface 投影后只做视觉隐藏。

对比：

- 方案 A 实现复杂度中等，但与 terminal 语义最一致，复制、选择、scrollback 都不会残留脏数据；
- 方案 B 实现复杂度较低，但会造成“画面看不见、内部仍存在”的语义错位。

最终决策：

- 采用方案 A；
- 仅针对 exact match 文本：
  - `Activate the web console with: systemctl enable --now cockpit.socket`
- 过滤位置在 parser 前；
- 该行不进入可见区、复制结果、选择结果、scrollback。

### 设计要点 2：键盘输入架构

候选方案：

- 方案 A：继续扩展当前 `TextInput.key-pressed` + 静态 `encode_named_key_input()`；
- 方案 B：改为 session-aware live encoder，UI 只负责采集键事件，编码由活动 `TerminalSession` 根据实时 terminal mode 完成。

对比：

- 方案 A 与现状改动最小，但会持续堆积键表分支，且无法正确反映应用光标模式等 terminal state；
- 方案 B 复杂度更高，但与 `wezterm-term`/`termwiz` 架构一致，可持续扩展。

最终决策：

- 采用方案 B；
- 保留 UI 侧事件采集，但废弃“临时 session 静态编码”的路径；
- 键编码必须绑定当前活动 session 的真实 terminal state；
- `application_cursor_keys`、future `modify_other_keys`、bracketed paste 等行为都以 live session 为准。

### 设计要点 3：快捷键、粘贴与滚轮语义

候选方案：

- 方案 A：桌面应用优先，`Ctrl+C/V` 偏向本地 clipboard；
- 方案 B：终端原生优先，`Ctrl+C` 发远端，clipboard 走 `Ctrl+Shift+C/V`、右键菜单、`Shift+Insert`，滚轮在非 `mouse_grabbed` 时走本地 scrollback；
- 方案 C：混合策略，`Ctrl+C` 远端、`Ctrl+V` 本地 paste。

对比：

- 方案 A 更像普通桌面文本框，但与终端用户预期冲突最大；
- 方案 C 会制造长期语义混乱；
- 方案 B 与专业 terminal 习惯最一致，也最利于 `vim`/`htop`/shell REPL 等场景。

最终决策：

- 采用方案 B；
- `Ctrl+C` 默认发给远端；
- `Ctrl+Shift+C` 触发本地 copy；
- `Ctrl+Shift+V`、右键 Paste、`Shift+Insert` 触发本地 paste；
- 如果 terminal 报告 `bracketed_paste_enabled = true`，paste 必须按 bracketed paste 协议发送；
- wheel 在 `mouse_grabbed = true` 时发给远端；
- wheel 在 `mouse_grabbed = false` 时走本地 scrollback。

### 设计要点 4：terminal palette 与 light/dark theme 同步

候选方案：

- 方案 A：完整 theme-aware terminal palette；
- 方案 B：只改默认背景层，不改完整 palette。

对比：

- 方案 A 复杂度更高，但能从根本上解决默认背景、cursor、ANSI 基础色与 shell surface 割裂；
- 方案 B 只能做表面修补，未来会继续暴露不协调。

最终决策：

- 采用方案 A；
- terminal 默认背景、默认前景、cursor、ANSI 基础色需要与 app `ThemeMode` 同步；
- theme toggle 后，现有 session 的 palette 也要随之刷新，而不是继续保留黑底独立 theme。

### 设计要点 5：VSCode 风格 visual contract

候选方案：

- 方案 A：只换字体和颜色，维持当前固定 `8x16` 近似 cell 度量；
- 方案 B：建立 editor-like terminal visual contract，使用真实 monospace metrics 驱动 cell、cursor、mouse hit-test、rows/cols 计算；
- 方案 C：在方案 B 上叠加默认行交错底色。

对比：

- 方案 A 成本低，但只会形成“贴皮”效果；
- 方案 B 与长期 custom renderer 方向一致，能真正接近 VSCode editor 区域气质；
- 方案 C 风险较高，容易破坏全屏 TUI 语义。

最终决策：

- 采用方案 B；
- 风格目标是“接近 VSCode editor 区域”，但仍以 terminal 语义优先；
- 采用 mono 字体路线；
- 不把“每一行轻微交错底色”作为首轮约束；
- 不在本轮实现本地假 syntax highlighting，颜色仍以远端 ANSI/TUI 输出为主。

## 方案对比

| 设计点 | 放弃方案 | 采用方案 | 原因 |
| --- | --- | --- | --- |
| 横幅过滤 | surface 后视觉隐藏 | parser 前 exact-match 过滤 | 避免选择/复制/scrollback 语义错位 |
| 键盘编码 | 静态命名键临时编码 | session-aware live encoder | 正确反映 terminal mode |
| 快捷键与粘贴 | 桌面文本框式 `Ctrl+C/V` | terminal-native clipboard contract | 与专业终端一致 |
| 滚轮 | 始终透传远端或始终本地 | `mouse_grabbed` 决定路由 | 兼顾 shell 与 TUI |
| palette | 维持独立黑底 | 跟随 app theme | 消除亮色模式割裂 |
| 视觉风格 | 纯贴皮 | editor-like metrics contract | 为后续 renderer 留稳定基础 |

## 最终决策

本轮确认的最终设计如下：

1. 指定横幅在 parser 前做 exact-match 过滤；
2. terminal 输入链路改为 session-aware live encoder；
3. 快捷键采用 terminal-native 语义：
   - `Ctrl+C` 发远端；
   - `Ctrl+Shift+C` 本地复制；
   - `Ctrl+Shift+V` / `Shift+Insert` / 右键 Paste 本地粘贴；
4. paste 必须支持 bracketed paste；
5. wheel 在 `mouse_grabbed` 时透传远端，否则进入本地 scrollback；
6. terminal palette 跟随 app light/dark theme；
7. visual contract 以 VSCode editor 风格为目标，但不引入首轮 zebra 行底色和本地假高亮。

## 实施步骤

### Phase 1：输出过滤与 palette 基础

- 在 SSH output -> terminal parser 之间引入行级过滤器；
- 为 live session 引入 theme-aware palette 配置与更新路径；
- 保证 terminal 默认背景能铺满整个绘制区域。

### Phase 2：输入链路重构

- 移除静态临时 session 编码路径；
- 建立活动 session 的 live key encoder；
- 补齐 `F1-F24`、`Ctrl/Alt/Shift` 组合键、`Home/End/PageUp/PageDown/Delete`；
- 补齐 bracketed paste 路径。

### Phase 3：pointer / wheel / resize contract

- 引入 Slint `scroll-event`；
- 在本地 scrollback 与远端 wheel 间做路由；
- 补齐鼠标点击、拖拽、选择、透传与 resize rows/cols 同步。

### Phase 4：editor-like visual contract

- 用真实 monospace metrics 统一 cell 宽高、cursor、mouse hit-test、rows/cols 计算；
- 收敛边框、padding、光标、选区和 surface 层次；
- 将 visual 方向向 VSCode editor 区域靠拢。

## 风险与回滚策略

### 风险

- parser 前过滤需要处理跨 packet 行拼接，避免误吞合法输出；
- live encoder 接入后，若 terminal mode 同步不完整，可能引入新的键行为回归；
- palette 热切换若只更新 UI 不更新 session，会出现新旧 session 表现不一致；
- scrollback 与远端 wheel 路由若边界不清，`vim`/`less`/普通 shell 会出现交互分裂；
- 真实 metrics 替换固定 `8x16` 后，已有 click/selection 命中逻辑需要一起收敛。

### 回滚策略

- 横幅过滤实现为独立阶段，若出现误吞，可先仅禁用过滤器而不影响其余输入/渲染改造；
- live encoder 与 old static encoder 在过渡期保留清晰切换边界，若新编码不稳定，可短期回退到旧编码通路；
- palette 同步以 session 层与 UI 层分别验证，若热切换有问题，可先保留“新会话生效、旧会话刷新受限”的临时降级策略，但不回退到固定黑底；
- visual contract 与 input contract 分阶段落地，避免一次性混改。

## 验证清单

- [ ] 指定横幅不出现在 terminal 可见区、复制结果、选择结果、scrollback
- [ ] 亮色模式下 terminal 默认背景铺满整个终端区域，不再出现局部黑底
- [ ] dark/light theme toggle 后，terminal palette 与 app shell 同步变化
- [ ] 普通字符输入、`Enter`、`Backspace`、`Tab`、`Esc` 行为正确
- [ ] `Ctrl/Alt/Shift` 组合键能按 terminal 语义发给远端
- [ ] `F1-F12` 至少首轮可用，扩展位预留到 `F24`
- [ ] `Arrow`、`Home/End`、`PageUp/PageDown`、`Delete` 在 shell 与 `vim`/`htop` 场景都可用
- [ ] `Ctrl+C` 发远端，`Ctrl+Shift+C` 本地复制，`Ctrl+Shift+V` 本地粘贴
- [ ] `Shift+Insert` 与右键 Paste 可用
- [ ] bracketed paste 在远端启用时按协议包裹发送
- [ ] 鼠标点击、拖拽、wheel 在 `mouse_grabbed` 场景下能透传远端
- [ ] 非 `mouse_grabbed` 场景下 wheel 进入本地 scrollback
- [ ] window resize 后 rows/cols 与 SSH PTY `window-change` 同步
- [ ] 字体、间距、cursor、选区与 VSCode editor 风格接近，但不破坏 terminal 语义

## 备注

- 本文档方案名为 `ssh-terminal-input-rendering`，文件名按约定写为 `2026-03-26-ssh-terminal-input-rendering-design.md`；
- 对应 implementation plan 已生成：`2026-03-26-ssh-terminal-input-rendering-implementation-plan.md`；
- 本轮仅修改文档，未修改业务代码。
