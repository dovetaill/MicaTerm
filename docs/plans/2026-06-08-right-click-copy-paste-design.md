# Right-Click Copy/Paste Design

日期: 2026-06-08
执行者: Codex
状态: 方案已确认，待在新 worktree 进入实现规划

## 说明

- 本轮只产出文档，不改 Rust / Slint / 测试代码。
- 正式实现必须在新的工作窗口和 `/home/wwwroot/mica-term/.worktrees/<feature-name>` 中完成。
- 按要求已实际开启 4 个 subagent，分别承担 Desktop UI / Slint、Terminal runtime / clipboard、Security / secrets、QA / regression 视角，并完成了 2 轮讨论。
- 但当前 Codex `spawn_agent` 工具不暴露 `model` 参数，无法显式固定到 `gpt-5.4 xh`；因此本轮实际使用的是当前线程继承模型的 subagent，并在本设计中如实记录该限制。

## 本地代码现状

### 1. 现有统一 context menu 路线只覆盖 assets / SFTP

本地已经存在成熟的 `Rust -> bootstrap -> Slint root overlay` context menu 路线，但目标域目前仍是 assets / keychain / SFTP，而不是文本编辑或 terminal：

- `src/shell/context_menu.rs`
  - `ContextTargetKind` 目前只覆盖 `BlankArea`、`SshConnection`、`Folder`、`KeychainIdentity`、`KeychainSshKey`、`Snippet`、`SftpFile` 等资产类目标；
  - `SelectionContext` 目前只有 `selected_ids`、`clipboard_has_asset_payload`、`target_mutable`、file/dir 计数，没有 `has_selection`、`clipboard_has_text`、`read_only`、`secret_kind`、`session_can_paste` 等文本语义；
  - 已经沉淀了 placement helper，如 `resolve_root_menu_origin`、列宽/列高计算与边界翻转逻辑。
- `src/shell/view_model.rs`
  - `ShellViewModel` 已持有 `context_menu_open`、`context_menu_surface`、`context_menu_target_kind`、`context_menu_anchor_x/y`、`context_menu_origin_x/y`、`context_menu_open_path` 等菜单状态；
  - 但当前 `context_menu_selection()` 解析的是资产选择态，不是文本编辑态。
- `src/app/bootstrap/assets_keychain.rs`
  - 负责把 Rust context menu state 同步到 Slint；
  - 已经验证“Rust 真相源 + root overlay 渲染 + 统一 dismiss/placement”的路线可行。
- `ui/app-window.slint`
  - 根窗口已经承载 `AssetsContextMenuOverlay` 和 `WorkspaceTabContextMenu`；
  - 已有 dismiss layer 与 overlay 宿主经验。

结论：项目已经有成熟的 root overlay/context menu 架构主干，但当前 domain 与交互模型都偏资产树，不可直接等价套到 live text editing。

### 2. workspace terminal 已有一份 host-local 右键菜单，不是空白场景

`workspace terminal` 不是“尚未实现右键菜单”，而是已经有一份局部菜单实现，但它不走现有统一 context menu domain：

- `ui/shell/terminal-session-host.slint`
  - 自己维护 `context-menu-open` 与本地 anchor；
  - `TouchArea.pointer-event` 在右键时：
    - 若 `session-mouse-grabbed && !Shift`，转发远端鼠标事件；
    - 否则打开本地 terminal 菜单；
  - 当前菜单项是固定的 `Copy / Paste / Select All / Find`；
  - `Copy` 只在 `has-selection()` 时可用；
  - `Paste` 目前没有 Rust capability gating；
  - dismiss layer 目前主要处理左键关闭。
- `ui/shell/workspace-pane.slint`
  - 把 terminal host 的 `copy-selection-requested`、`paste-requested`、`context-menu-open-changed` 等 callback 往上透传。
- `ui/app-window.slint`
  - 暴露 `workspace-session-context-menu-open` 与 `workspace-session-context-menu-open-changed(bool)`。
- `src/app/bootstrap.rs`
  - 当前 `window.on_workspace_session_context_menu_open_changed(...)` 只做 geometry / selection projection 协调，没有 Rust 菜单模型。

结论：terminal 侧已经有“前身”，但它与 assets overlay 是两套不同路线；正式设计不能把这份局部菜单当成最终统一方案，也不能无视它已经承载的 hit-test、selection 和 `mouse_grabbed + Shift` 语义。

### 3. terminal copy/paste 主链路已经成熟，必须复用

本地 terminal copy/paste 管线已经很清晰：

- `src/app/bootstrap/workspace_terminal.rs`
  - `set_system_clipboard_text(text: &str)` / `system_clipboard_text()` 管理系统 clipboard；
  - `forward_active_workspace_copy_selection(...)` 从 Rust-owned selection truth 复制，优先读 runtime/session buffer，fallback 才是 surface；
  - `normalize_workspace_paste_text(text)` 把 `CRLF` / bare `CR` 转成 `LF`；
  - `workspace_paste_prompt_mode(...)` 依据字符数、逻辑行数、bracketed paste 状态决定 `Confirm` / `Editor`；
  - `forward_active_workspace_paste(...)` 从系统 clipboard 读取、归一化、决定是否弹 warning/editor modal；
  - `forward_workspace_session_paste(...)` 最终发给 `SessionManager`。
- `src/app/bootstrap.rs`
  - `window.on_workspace_session_paste_requested(...)` 统一走 `forward_active_workspace_paste(...)`；
  - warning/editor modal confirm 再统一走 send path。
- `src/app/ssh/session_manager.rs`
  - `send_session_paste(session_id, text)` 转到 runtime control；
  - `selection_text_from_buffer_rows(...)` 为 terminal copy 提供 runtime buffer 优先的真相源。
- `src/app/ssh/runtime.rs` 与 `src/app/ssh/runtime/pump.rs`
  - `send_paste(text)` -> `RuntimeCommand::Paste(text)`。
- `src/app/terminal_core/wezterm_adapter.rs`
  - `encode_paste()` 会去除已有 bracketed markers，并在 bracketed paste enabled 时包 `ESC [200~ ... ESC [201~`。

现有测试也已经锁住这条主链路：

- `tests/workspace_paste_warning_modal_spec.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- `tests/terminal_session_spec.rs`
- `tests/bootstrap_smoke.rs`

结论：terminal 右键 `Paste` 只能复用现有 pipeline，不能再做第二条“本地读 clipboard 后直接发 PTY”的旁路。

### 4. `Ctrl+C` 终端语义当前是正确的，不能被右键设计带坏

- `ui/shell/terminal-session-host.slint` 只把 `Ctrl+Shift+C` / `Ctrl+Insert` 当作本地 copy；
- 普通 `Ctrl+C` 仍会作为 terminal key input 继续下发；
- `tests/terminal_session_spec.rs` 已验证 plain `Ctrl+C` 编码为 `0x03`。

结论：右键菜单新增 copy/paste 时，不得引导实现阶段去篡改 plain `Ctrl+C` 语义。

### 5. 表单文本 primitive 目前没有统一右键桥

- `ui/components/modal-chrome.slint`
  - 共享表单 primitive 是 `DialogTextField`；
  - 当前已有 `focus-input()` / `select-all()` 这类 helper；
  - 文本核心仍是 `TextInput`；
  - 还没有通用的 text context menu request / capability bridge。
- `ui/components/assets-ssh-connection-modal.slint`
  - 复用了大量 `DialogTextField`，包括普通字段、password、passphrase、proxy password、inline private key 等。
- `ui/components/assets-keychain-identity-modal.slint`
  - password secret 字段。
- `ui/components/assets-keychain-ssh-key-modal.slint`
  - private key、public key、fingerprint 等多行字段。
- `ui/components/assets-snippet-modal.slint`
  - multiline script/snippet 字段。
- `ui/components/assets-snippet-package-modal.slint`
  - 仍保留裸 `TextInput`，没有走共享 primitive。
- `ui/components/sync-vault-modal.slint`
  - master password、PAT、SSH private key、SSH passphrase 等 secret surface。

结论：如果只改共享 primitive，会漏掉裸 `TextInput`；如果只改某个 modal，会失去统一性。必须设计成“共享 bridge + 少量 outlier adapter”。

### 6. secret 相关现状支持 fail-closed 方向

- `src/app/bootstrap.rs` 已存在：
  - `paste_private_key_into_keychain_modal`；
  - `paste_public_key_into_keychain_modal`；
  - `copy_public_key_from_keychain_modal`；
  - 但未发现 `copy_private_key_*` 等等价私钥复制动作。
- `src/app/ssh/credentials.rs`
  - `StoredSshSecretBundle`、`StoredKeychainIdentitySecretBundle`、`StoredKeychainKeySecretBundle` 明确区分多类 secret bundle；
  - 也说明 secret 的驻留面广，不能再轻易新增 clipboard 外泄面。
- `src/app/vault/model.rs`
  - vault snapshot 也承载 secret bundles，说明“是否 secret”是领域语义，不是 UI 样式。
- UI 上 private key 有 masked/unmasked 混用：
  - `assets-ssh-connection-modal` 与 `assets-keychain-ssh-key-modal` 的 private key 是明文多行可编辑；
  - `sync-vault-modal` 的 Git SSH private key 又是 masked。

结论：secret-field 不能靠 `password-mode` 或是否明文显示来推断，必须由显式语义标签决定；现有代码也支持“public key 有显式 copy 白名单，private key 没有”的保守策略。

### 7. 现有 docs/plans 已经给出可继承约束

本轮设计必须继承以下既有文档的结论，而不是另起炉灶：

- `docs/plans/2026-03-17-windows-console-assets-context-menu-design.md`
  - 确认 root overlay + Rust 真相源 是本仓库 context menu 主干。
- `docs/plans/2026-03-19-windows-console-assets-context-menu-bugfix4-design.md`
  - 明确了 Explorer 风格右键命中、root overlay 与 dismiss 语义的延续思路。
- `docs/plans/2026-04-24-modal-input-select-hardening-design.md`
  - 明确 `TextInput` 本体应拥有真实文本命中/选区，不应被覆盖层破坏。
- `docs/plans/2026-05-14-workspace-terminal-paste-crlf-normalization-design.md`
  - 明确 terminal paste normalization 是 shell-oriented 语义，不应轻易推广到普通文本编辑。
- `docs/plans/2026-03-28-assets-snippets-design.md`
  - 明确 snippet/script 是文本语义、非 terminal 语义。
- `docs/plans/2026-03-28-assets-keychain-design.md`
  - 明确 keychain/private key/public key 是独立领域对象与安全边界。

## 外部调研结论

本节基于 Tavily / Exa MCP 搜索与提取结果；仅记录本轮实际完成的查询和结论，不补写未做过的调研。

### 查询词

1. `Windows Terminal official docs rightClickContextMenu copyOnSelect right click copy paste settings`
2. `WezTerm official docs bracketed paste clipboard right click copy paste selection`
3. `xterm.js official docs clipboard right click paste selection bracketed paste mode`
4. `Slint official docs TextInput TouchArea FocusScope pointer-event context menu popup right click`
5. `Termius official docs clipboard paste terminal right click copy selection desktop`
6. `NIST password paste allow paste official guidance 800-63B paste password managers`
7. `1Password clipboard clear password copy official docs`
8. `Bitwarden clear clipboard password official docs copy password clipboard timeout`

### 成熟产品 / 官方资料摘要

#### 1. Windows Terminal

来源：

- <https://learn.microsoft.com/en-us/windows/terminal/customize-settings/interaction>
- <https://learn.microsoft.com/en-us/windows/terminal/selection>

关键结论：

- `copyOnSelect=false` 时，terminal selection 持久化，右键可用于复制当前选区；
- `experimental.rightClickContextMenu=true` 时，右键可打开 context menu；关闭时，右键可直接 paste；
- Windows Terminal 在 VT mouse mode 下保留 `Shift` 作为本地 selection 逃生阀；
- terminal paste warning（large/multiline）是成熟产品的常见防线。

采纳：

- 采纳“terminal 是独立 interaction surface，右键行为要尊重 VT mouse mode / Shift 本地逃生阀”；
- 采纳“selection-sensitive copy + paste warning”思路；
- 采纳“右键交互与 terminal runtime 语义耦合，不是普通文本框 paste”。

不直接采纳：

- 不直接复制 Windows Terminal 的“右键单击直接 copy / paste”二段行为，因为 MicaTerm 已有明确的菜单/警告管线，且本轮目标是菜单型 copy/paste；
- 不把 `copyOnSelect` 作为本轮需求。

#### 2. WezTerm

来源：

- <https://wezterm.org/config/lua/keyassignment/PasteFrom.html>
- <https://wezterm.org/copymode.html>

关键结论：

- WezTerm 把 paste 建模为显式 action（`PasteFrom`），而不是 UI 局部插入；
- copy mode、clipboard、selection 是 terminal 专有概念；
- X11/Wayland 下还会区分 `Clipboard` 与 `PrimarySelection`。

采纳：

- 采纳“terminal copy/paste 应走 terminal action / runtime 管线”的建模方式；
- 采纳“terminal clipboard 行为与普通 text editor 分层”。

不直接采纳：

- 不引入 `PrimarySelection` 专门语义；当前仓库和本轮需求都以系统 clipboard 为准。

#### 3. xterm.js / xterm 家族

来源：

- <https://xtermjs.org/docs/api/terminal/interfaces/iterminaloptions>
- <https://xtermjs.org/docs/api/vtfeatures>
- <https://invisible-island.net/xterm/xterm-paste64.html>

关键结论：

- `DECSET/DECRST 2004` 的 bracketed paste 是成熟 terminal 常识；
- xterm.js 明确建模了 `ignoreBracketedPasteMode`，反过来也说明“正常 paste 默认应尊重 bracketed paste mode”；
- 终端是否使用 bracketed paste 属于 terminal/runtime 层，不应由普通输入框行为决定。

采纳：

- 采纳“terminal Paste 不得绕过 bracketed paste”；
- 采纳“bracketed paste 是 terminal 能力，不下沉到普通文本字段”。

不直接采纳：

- 不在本功能里新增 bracketed paste 用户开关；当前仓库已有既有 pipeline，更适合沿用。

#### 4. Slint

来源：

- <https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea>
- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/focusscope>
- <https://codebrowser.dev/slint/slint/tests/cases/elements/popupwindow_context.slint.html>

关键结论：

- Slint 的右键/弹出菜单常用入口是 `TouchArea.pointer-event`；
- `FocusScope` 只有获得焦点后才处理 key event；
- popup/context-menu 一旦获得事件和焦点，底层 pointer/focus 行为就会被截断；
- 官方测试也说明 popup 可拦截后续点击，需要显式设计 dismiss/reposition。

采纳：

- 采纳 `TouchArea.pointer-event` 捕获右键 anchor 的方式；
- 采纳“text-edit menu 不能像 assets overlay 那样默认抢焦点”的结论；
- 采纳“重定位与 dismiss 必须显式设计”。

不直接采纳：

- 不把普通文本菜单做成 modal-local popup window；本仓库已有 root overlay 体系，且 modal body 存在 `clip: true` 风险。

#### 5. Termius / secret tooling

来源：

- <https://docs.termius.com/keychain/ssh-keys-and-certificates>
- <https://pages.nist.gov/800-63-3/sp800-63b.html>
- <https://pages.nist.gov/800-63-4/sp800-63b.html>
- <https://support.1password.com/copy-passwords>
- <https://support.1password.com/1password-security>
- <https://bitwarden.com/help/product-faqs>

关键结论：

- Termius 官方文档明确支持“粘贴 private key 导入”这一成熟工作流；
- NIST SP 800-63B 明确建议允许 password paste，以便 password manager 使用；
- 1Password / Bitwarden 等成熟 secret 工具把“复制到 clipboard”当作显式动作，并强调 clipboard 清理或 clipboard 风险；
- secret 工具通常会对私钥/硬件密钥/生物密钥采取更保守的“不可导出/显式导出”心智，而不是 ambient copy affordance。

采纳：

- 采纳“secret/password 字段默认允许 paste”；
- 采纳“secret copy 应该保守、显式，而不是默认右键项”；
- 采纳“private key paste 是正常导入工作流，但 private key copy 不应因为 text field 可编辑就自动暴露”。

不直接采纳：

- 不在本轮引入 clipboard auto-clear；现有仓库无相关通用基础设施，本轮聚焦右键 copy/paste 入口与管线复用。

### 面向 MicaTerm 的外部调研结论总表

- terminal：更接近 Windows Terminal / WezTerm / xterm.js 的成熟做法，必须保持 runtime-owned paste semantics；
- 普通文本：沿用桌面文本编辑器直觉，但不与 terminal paste 语义混用；
- secret 字段：允许 paste，默认不提供 ambient copy；
- root overlay：可以统一菜单承载与 placement，但不能复用 assets overlay 的抢焦点交互。

## 多专家辩论摘要

### 参与角色

- A. Desktop UI / Slint 专家
- B. Terminal runtime / clipboard / bracketed paste 专家
- C. Security / secrets / private key handling 专家
- D. QA / regression / test architecture 专家

### 第 1 轮观点

#### A. Desktop UI / Slint

- 认为 terminal 其实已经有一份 host-local 右键菜单，不应假装场景为空白；
- 倾向复用 `Rust + AppWindow root overlay` 主干，但不要直接复用 `assets-context-menu-overlay` 的 `focus-menu()`/`FocusScope` 抢焦点行为；
- 建议 terminal 只迁“菜单呈现层”，不迁 selection/mouse hit-test 状态机；
- 指出 `BlockingModalShell` 有 `clip: true`，modal-local 菜单容易被裁切；
- 明确提出 `assets-snippet-package-modal.slint` 是共享 primitive 之外的裸 `TextInput` outlier。

#### B. Terminal runtime / clipboard / bracketed paste

- 强调 terminal 右键 Paste 绝不能新增旁路，必须继续走 `workspace_session_paste_requested -> forward_active_workspace_paste -> send_session_paste -> encode_paste`；
- 强调 plain `Ctrl+C` 不得被本地 copy 劫持；
- 强调 `mouse_grabbed + Shift override` 必须原样保留；
- 指出当前代码里尚未发现稳定的 terminal `can_write` / `writable` capability 真相源；
- 反对在 Slint 层自行猜测 terminal 可 paste 状态。

#### C. Security / secrets / private key handling

- 坚持 secret-field 默认 fail-closed：允许 paste，不默认提供 copy；
- 强调 private key 即便明文可编辑，也仍是 secret；
- 反对用 `password-mode` 或 UI 外观判定 secret；
- 强调 private key / token / snippet 不能复用 terminal CRLF normalization / bracketed paste 语义；
- 支持 public key 作为显式 copy 白名单继续存在。

#### D. QA / regression / test architecture

- 建议把 terminal、普通文本、secret-field 分层测试；
- 建议新增独立 text capability spec，而不是把文本规则塞进 assets menu spec；
- 明确要求覆盖 `assets-snippet-package-modal.slint` 这种裸 `TextInput`；
- 明确要求增加“focus/selection 不被菜单打开破坏”“secret 不进入日志/快照”的回归测试；
- 建议 terminal 先 RED，普通文本后接线，最后再做 cross-surface stability。

### 第 1 轮主要冲突点

1. 是否直接复用现有 assets context menu 组件与交互模型；
2. terminal 菜单是继续 host-local，还是迁到 root overlay；
3. secret-field 是否由 UI 外观推断；
4. `Cut` / `Select All` 是否与 copy/paste 一起进首发；
5. terminal `can_paste` 的 capability 真相源缺口如何处理。

### 第 2 轮回应与收敛

#### A. Desktop UI / Slint 专家收敛

- 接受“root overlay 只负责 display，不抢文本焦点”的约束；
- 主张 terminal 迁菜单呈现到 root overlay，但保留 `TerminalSessionHost` 负责 right-click 命中、selection、`mouse_grabbed` 判定；
- 建议 `DialogTextField` 与裸 `TextInput` 走同一 bridge 契约，而不是两个菜单系统；
- 支持把当前 assets overlay 的承载层复用下来，但不复用其 `focus-menu()` 交互模型。

#### B. Terminal 专家收敛

- 接受“Rust resolver + root overlay 显示 + TerminalSessionHost 继续负责 hit-test/state machine”的三段式模型；
- 明确建议把 `can_paste` 收敛到 Rust capability 层，初版只承诺 `session runtime ready + 当前允许本地菜单` 级别，不声称知道远端 shell 的业务可写性；
- 明确建议 design 文档写出 terminal 与普通文本的边界句子，防止实现时混用 paste semantics。

#### C. Security 专家收敛

- 把 taxonomy 收敛为：`TerminalSurface / PlainTextField / CodeTextField / PublicMaterialField / SecretField`；
- `SecretField` 再细分 `password / passphrase / token / proxy-password / master-password / private-key`，但首发菜单能力只关心“是不是 secret”；
- 坚持 private key 即便明文可编辑仍默认无 `Copy` 菜单项；
- 建议 design 文档避免写成“secret 永远无法复制”，而是精确表述为“本功能不新增 secret 的右键 Copy/Cut affordance”。

#### D. QA 专家收敛

- 建议按任务顺序先锁 terminal，再做 text capability matrix，再接 SSH/key/snippet，再收口 overlay/focus/security；
- 建议哪些必须先 RED、哪些可以 source-contract smoke 都在 tasks 中明确；
- 明确要求把 `focus/selection` 与 `secret 不进日志/快照` 作为必须验证项；
- 给出未来 worktree 实施阶段的建议验证命令集合。

### 最终取舍

- 不采用“每个 TextInput 各画一份本地菜单”；
- 不采用“terminal 菜单永久 host-local、普通文本另起一套呈现体系”的长期目标；
- 不采用“平台 native menu”；
- 采用“复用现有 Rust -> bootstrap -> AppWindow root overlay 路线，但为 text-edit 建立 sibling 子域，并保持 terminal / plain text / secret / code 各自 capability 语义”的方案。

## 方案对比

### 方案 A：复用现有 context menu 路线，并为 text-edit 扩展 target / 子域

核心思路：

- 继续复用现有 `Rust -> bootstrap -> AppWindow root overlay` 主干；
- 但不把文本语义硬塞进现有 assets action tree，而是在同一路线上新增轻量 `text-edit-context-menu` 子域；
- placement helper、overlay 宿主、dismiss 总线可共享；
- capability、action、overlay 交互模型与资产树分开。

优点：

- 与本仓库已落地的 context menu 架构最一致；
- Rust 继续作为状态真相源；
- 可测试性高；
- 便于统一跨 surface 的 placement/dismiss；
- 能保留 terminal 专用语义，同时避免 assets overlay 抢焦点回归。

缺点：

- 需要新增 text-edit capability/resolver；
- 需要谨慎处理 live text focus/selection；
- 需要为 `DialogTextField` 与裸 `TextInput` 同时建立桥。

### 方案 B：Slint TextInput / Terminal 局部自建菜单

核心思路：

- terminal 继续自带本地菜单；
- 每个 modal/text field 在 Slint 里本地管理菜单项、placement、dismiss。

优点：

- 局部接入快；
- terminal 焦点风险看似较低。

缺点：

- 很快演变成多套菜单状态机；
- capability 判断会碎片化到各个 UI 组件；
- secret/clipboard 规则难以统一；
- QA 回归矩阵更大；
- 与现有 Rust-owned context menu 方向冲突。

### 方案 C：平台 native menu

核心思路：

- 依赖操作系统原生 context menu；
- UI 只负责触发，菜单绘制和部分行为交给平台。

优点：

- 某些平台上原生交互一致性高；
- placement/dismiss 一部分由系统处理。

缺点：

- 与当前自绘 shell / root overlay 体系冲突；
- terminal、modal、overlay、focus 行为难统一；
- 跨平台一致性与测试难度更差；
- secret / capability / telemetry 行为难以在 Rust 侧完全收口。

### 推荐方案

推荐方案 A，但明确为：

- 复用现有 route，不复用现有 assets overlay 交互模型；
- 共享承载层和 placement helper，新增 text-edit 子域和轻量 overlay。

拒绝 B 的原因：会固化两套甚至多套右键模型。

拒绝 C 的原因：与现有壳层架构和测试方式明显不匹配。

## 推荐架构

### 1. 新增 `text-edit-context-menu` 子域

推荐在现有 route 上新增 text-edit sibling 子域，而不是把 live text editing 完全塞进现有 `src/shell/context_menu.rs` 的资产语义里。

推荐语义对象：

- `TextContextTargetKind`
  - `WorkspaceTerminal`
  - `PlainTextField`
  - `CodeTextField`
  - `PublicMaterialField`
  - `SecretField`
- `SecretKind`
  - `Password`
  - `Passphrase`
  - `Token`
  - `ProxyPassword`
  - `MasterPassword`
  - `PrivateKey`
- `TextContextCapabilities`
  - `has_selection`
  - `clipboard_has_text`
  - `editable`
  - `can_copy`
  - `can_paste`
  - `can_cut`
  - `can_select_all`
  - `local_context_menu_allowed`

命名可在实现阶段调整，但语义边界应保持不变。

### 2. Rust 侧职责

Rust 侧 resolver 负责：

- 解析 target kind 与 secret kind；
- 读取 selection/clipboard/runtime capability；
- 产出菜单项与 enabled/disabled 状态；
- 把 action 派发到：
  - terminal paste/copy pipeline；
  - 普通 text-edit pipeline；
  - 已存在的显式 public key copy 等动作。

特别说明：

- terminal `can_paste` 初版不承诺洞察远端 shell 业务是否“只读”；
- 文档只要求实现期新增一个最小、可测试的 Rust capability 契约，至少能表达：
  - 当前有 active terminal session；
  - 当前允许本地菜单；
  - 当前 runtime 已就绪到可接收 paste 请求。

### 3. Slint 侧职责

Slint 侧负责：

- 在 terminal、`DialogTextField`、裸 `TextInput` 上捕获 right-click；
- 上报 anchor 和最小 metadata；
- 在 `AppWindow` root overlay 层显示 text-edit menu；
- 处理 placement / dismiss / pointer click action；
- 不接管 Rust capability 真相源。

推荐上报 metadata 至少包含：

- target kind；
- secret kind（若适用）；
- `has_selection`；
- `editable/read_only`；
- `multiline`；
- anchor `x/y`。

### 4. terminal 接线方式

terminal 采用“三段式”接线：

1. `TerminalSessionHost` 继续负责：
   - cell hit-test；
   - selection 状态机；
   - `mouse_grabbed` / `Shift` 本地逃生阀；
   - terminal-local anchor 计算。
2. Rust resolver 负责：
   - `Copy/Paste` capability；
   - action dispatch。
3. `AppWindow` root overlay 负责：
   - 菜单显示；
   - placement/dismiss；
   - 点击菜单项后回调 Rust action。

### 5. 普通文本 / secret / code 字段接线方式

- `DialogTextField`：作为主路径，新增统一 right-click bridge；
- 裸 `TextInput`：新增 adapter，至少覆盖 `assets-snippet-package-modal.slint` 这类 outlier；
- secret-field 与 plain/code field 的差异由 Rust capability 决定，不由 Slint 样式自行推断。

## 终端粘贴链路

本节是本设计最硬的约束之一：terminal 右键 `Paste` 不得绕过现有 pipeline。

必须保留的链路：

1. UI 右键菜单动作只触发现有 terminal paste request callback；
2. bootstrap 继续调用 `forward_active_workspace_paste(...)`；
3. 从系统 clipboard 读文本；
4. 走 terminal 专用 `normalize_workspace_paste_text(...)`；
5. 根据 multiline / length / bracketed paste 状态决定 warning/editor modal；
6. confirm 后继续走 `forward_workspace_session_paste(...)`；
7. `SessionManager.send_session_paste(...)` -> runtime control -> `encode_paste(...)`；
8. bracketed paste enabled 时由 terminal core 负责包装。

禁止事项：

- UI 层直接读 clipboard 后发 SSH bytes；
- 普通文本字段复用 terminal paste warning；
- private key / snippet 复用 terminal CRLF normalization；
- 为了做右键菜单而重写 `Ctrl+C` / `Ctrl+V` terminal 键盘语义。

## 安全设计

### 1. Secret 默认 fail-closed

- `SecretField` 默认允许 `Paste`；
- 默认不提供 `Copy`；
- 默认不提供 `Cut`；
- `Select All` 不作为 phase 1 secret-field 右键项。

### 2. 明文 private key 仍按 secret 处理

以下类型即便当前 UI 是明文可编辑，也仍然按 `SecretField(PrivateKey)` 处理：

- SSH connection modal inline private key；
- keychain SSH key modal private key；
- sync vault modal Git SSH private key。

理由：

- “明文显示”只是编辑体验，不是授权模型；
- 当前本地代码只存在显式 public key copy 动作，未发现等价 private key copy 动作；
- 本功能不应新增 private key 的 ambient clipboard 外泄面。

### 3. public key / fingerprint 例外策略

- `PublicMaterialField` 允许继续保留显式 `Copy Public Key` 白名单；
- generic text `Copy` 仍应以选区为准，不因为字段是 public key 就默认为无选区 copy-all；
- `Paste` 是否可用继续受 `editable/read_only` 控制。

### 4. 日志与断言约束

后续实现和测试必须避免把以下内容写入日志、panic、snapshot、debug message：

- password / passphrase / token 明文；
- private key 文本；
- clipboard secret 内容；
- 选区 secret 内容。

设计文档刻意不宣称“secret 完全不会以明文进入进程内存”，因为当前仓库已有 secret draft/runtime strings；本功能只承诺“不新增右键 Copy/Cut 的 secret 外泄面，并在日志/测试上保持 fail-closed”。

## 测试设计

### 1. 纯 Rust / capability 层

建议新增独立 text capability spec，而不是塞进 assets menu spec：

- terminal / plain text / secret / public material / code 的 enable/disable matrix；
- `clipboard_has_text`、`has_selection`、`editable`、`secret_kind` 的组合；
- terminal `can_paste` 最小 capability 契约。

### 2. terminal 低层与协议层

继续复用现有测试层次：

- `tests/terminal_session_spec.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- `tests/workspace_paste_warning_modal_spec.rs`

需要新增或扩展的重点：

- 右键 Paste 仍走 bracketed paste pipeline；
- `mouse_grabbed + Shift override` 保持不变；
- plain `Ctrl+C` 仍是 `0x03`；
- terminal selection copy 仍优先 Rust truth，而不是 UI mirror。

### 3. source-contract / UI contract smoke

继续沿用仓库现有 contract 测试风格，验证：

- `AppWindow -> WorkspacePane -> TerminalSessionHost` callback 仍存在；
- 新增 text context menu bridge 存在；
- root overlay / dismiss layer wiring 存在；
- `DialogTextField` 没有重新用覆盖层破坏 text viewport。

### 4. 交互 smoke

建议分成两组，而不是把 terminal 与普通文本全塞进一个巨型 smoke：

- terminal smoke：
  - copy/paste action；
  - warning confirm/editor；
  - focus/selection 稳定性；
  - `mouse_grabbed` 远端右键仍可用。
- modal/text smoke：
  - SSH password；
  - SSH private key；
  - keychain identity password；
  - keychain public key / fingerprint；
  - snippet script；
  - snippet package name 裸 `TextInput`。

### 5. 安全回归

建议补充：

- logging runtime 回归；
- panic logging 回归；
- secret sentinel 不进入日志/失败输出。

## 迁移 / 实施边界

- 本轮不改任何业务代码。
- 正式实现必须新开 worktree，例如：`/home/wwwroot/mica-term/.worktrees/right-click-copy-paste`。
- 实现阶段必须按 TDD 拆任务，先 RED 后 GREEN。
- 如果实现期发现本地代码与本设计冲突，必须停下并用 `@superpowers:systematic-debugging` 做根因核对，而不是猜测性修补。
