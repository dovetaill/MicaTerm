# Input Field Right-Click Copy/Paste Design

日期: 2026-06-09
执行者: Codex
状态: 方案已确认；正式实现需在新窗口与独立 worktree 中执行

## 说明

- 本轮只做调研与文档产出，不修改 Rust / Slint / 测试业务代码。
- 正式实现必须在 `/home/wwwroot/mica-term/.worktrees/input-field-right-click-copy-paste` 或同等命名的新 worktree 中进行；本轮不要创建 worktree。
- 本轮按要求先使用了 `superpowers:brainstorming` 做问题拆解，并在文档中为后续实现明确 `superpowers:executing-plans`、`@superpowers:test-driven-development`、`@superpowers:systematic-debugging`、`@superpowers:verification-before-completion` 与 `superpowers:using-git-worktrees` 的使用边界。
- 与 2026-06-08 旧文档不同，本轮会话的 subagent 工具 `multi_agent_v1.spawn_agent` 支持显式指定 `model` 与 `reasoning_effort`；因此本轮实际使用了 4 个 `gpt-5.4`、`reasoning_effort: xhigh` 的 subagent 完成两轮讨论。这意味着“工具不支持显式指定 model”的旧结论在本会话里已经过时，不能继续照抄。

## 旧方案是否已落地

### 结论

- 旧方案已整理，但未落地，不能视为修复成功。

### 证据

- `git log -- docs/plans/2026-06-08-right-click-copy-paste-*.md` 当前仅看到 `78c1bbf docs: add right-click copy paste planning`。
- 在当前 HEAD 下，未发现后续“普通输入框右键 Copy / Paste 已实现”的对应提交、bridge 类型、菜单 domain、或 smoke test。
- 本地代码中的成熟 context menu 仍然主要覆盖 assets / keychain / SFTP，terminal 也仍是独立菜单域；没有看到普通文本输入与多行编辑器的统一右键实现。

因此，本轮新文档必须以当前代码事实为准，而不是把 2026-06-08 文档当成“已修复”的代名词。

## 本地代码现状

### 1. 现有 `Rust -> bootstrap -> Slint root overlay` context menu 只覆盖 assets / SFTP / keychain 资产动作

证据：

- `src/shell/context_menu.rs`
  - `ContextTargetKind` 目前只包含 `BlankArea`、`SshConnection`、`Folder`、`KeychainIdentity`、`KeychainSshKey`、`Snippet`、`SftpFile`、`SftpDirectory` 等资产域目标；
  - `ContextMenuSurface` 目前只有 `Assets`、`QuickBrowserSftp`、`WorkspaceSftp`；
  - `SelectionContext` 只包含 `selected_ids`、`clipboard_has_asset_payload`、`target_mutable`、`selected_file_count`、`selected_directory_count`。
- `src/shell/view_model.rs`
  - `ShellViewModel` 已管理 `context_menu_open`、`context_menu_surface`、`context_menu_target_kind`、anchor/origin/open_path 等状态；
  - `context_menu_selection()` 解析的是资产选择态，不是文本编辑态。
- `src/app/bootstrap/assets_keychain.rs`
  - 负责把上述状态同步到 Slint root overlay；当前同步的仍然是 assets / SFTP / keychain 菜单数据。

结论：现有 root overlay/context menu 架构成熟，但当前 domain 不包含普通输入框文本语义。

### 2. `SelectionContext` / `ContextTargetKind` 缺少 text-edit 语义

当前资产菜单使用的选择态字段无法表达普通输入框需要的 capability：

- 缺少 `field_id` / `field_kind`；
- 缺少 `is_read_only`、`is_secret`、`supports_multiline`；
- 缺少 `has_selection`、`clipboard_has_text`；
- 缺少 `can_copy`、`can_paste`；
- 缺少针对普通文本 field 的 action route。

结论：不能把普通输入框问题简单塞进当前 `SelectionContext`，而应新增独立 text-edit domain 或等价状态结构。

### 3. `DialogTextField` 是共享输入 primitive，但目前没有右键桥；且存在 focus/selection 易损风险

证据：

- `ui/components/modal-chrome.slint`
  - `DialogTextField` 内部直接包了 `field-input := TextInput { ... }`；
  - 公开接口只有 `focus-input()`、`select-all()`、`value`/`multiline`/`password-mode` 等少量属性；
  - 四周存在 `TouchArea` 仅负责补焦点，不提供 selection/caret/capability outward bridge。
- `ui/components/blocking-modal-shell.slint`
  - modal 使用 `FocusScope`，并会在初始化/切换时主动收焦点。
- `ui/app-window.slint`
  - 已存在全局 dismiss layer 与 root overlay 宿主。

历史风险判断：

- 若把现有 assets/workspace-tab 菜单那种“打开后主动 focus 菜单 overlay”的模式直接复制到文本输入场景，极易在右键时造成 `TextInput` 失焦，从而丢失 `selection` / `caret`；
- 若再叠加全局透明 `TouchArea` 覆盖 modal 命中，容易在 pointer down 阶段提前打断输入框自己的 hit-test。

结论：普通输入框菜单的根前提不是“能弹出来”，而是“弹出来时不抢 `TextInput` 焦点，不破坏当前选区”。

### 4. 存在少量未复用共享组件的裸输入 outlier

证据：

- `ui/components/assets-snippet-package-modal.slint` 仍直接声明 `name-input := TextInput`，没有走 `DialogTextField`。

结论：正式实现不能只改共享 `DialogTextField`，还要给裸 `TextInput` / `TextEdit` outlier 预留 adapter。

### 5. terminal 已有自己的右键菜单与 paste pipeline，不能与普通输入域混用

证据：

- `ui/shell/terminal-session-host.slint`
  - 已有 `context-menu-open`、锚点、`Copy / Paste / Select All / Find` 菜单；
  - 右键时若 `session-mouse-grabbed && !Shift`，优先转发远端鼠标事件；否则打开本地 terminal 菜单；
  - `selection-copy-enabled()` 控制 terminal `Copy` 能力；
  - `paste-requested()` 走 terminal paste callback。
- `ui/shell/workspace-pane.slint` 与 `ui/app-window.slint`
  - 已把 `workspace_session_copy_selection_requested`、`workspace_session_paste_requested`、`workspace-session-context-menu-open` 等 callback 串起。
- `src/app/bootstrap/workspace_terminal.rs`
  - 已有 `system_clipboard_text()`、`set_system_clipboard_text()`；
  - 已有 `normalize_workspace_paste_text()`、`workspace_paste_prompt_mode()`、`forward_active_workspace_paste()`。
- `src/app/ssh/session_manager.rs`、`src/app/ssh/runtime.rs`、`src/app/ssh/runtime/pump.rs`、`src/app/terminal_core/wezterm_adapter.rs`
  - 已形成 `send_session_paste -> RuntimeCommand::Paste -> encode_paste` 主链路。
- `tests/workspace_paste_warning_modal_spec.rs`、`tests/ssh_terminal_interaction_spec.rs`、`tests/bootstrap_smoke.rs`
  - 已锁住 terminal copy/paste、paste warning、bracketed paste、`mouse_grabbed + Shift` 等语义。

结论：terminal 是已有成熟实现的独立 domain。普通输入框右键 Copy / Paste 不得挪用 terminal pipeline。

### 6. secret 相关表单分布广，且当前产品语义偏保守 allowlist

证据：

- `ui/components/assets-ssh-connection-modal.slint`：password、private key、passphrase、proxy password 等 secret surface。
- `ui/components/assets-keychain-identity-modal.slint`：password 字段。
- `ui/components/assets-keychain-ssh-key-modal.slint`：`Paste Private Key`、`Paste Public Key`、`Copy Public Key` 已存在，但无 `Copy Private Key`。
- `ui/components/sync-vault-modal.slint`：master password、PAT、SSH private key、SSH passphrase 等 secret surface。
- `src/app/ssh/credentials.rs` 与相关 model：secret bundle 与 public metadata 明确分层。

结论：secret policy 不能仅凭 `password-mode` 或是否 reveal 决定，必须有明确 allowlist / deny-by-default 语义。

### 7. 当前测试最缺的是“普通输入框右键不丢 focus / selection”的 pointer-driven smoke

证据：

- `tests/modal_input_select_contract_spec.rs`、`tests/assets_modal_smoke.rs` 已覆盖 select-all、modal input hardening 等，但尚未针对普通输入框右键菜单做完整交互回归。
- `tests/workspace_paste_warning_modal_spec.rs` 已验证 terminal paste warning/editor 源码结构，但还缺“先点入 editor，再右键，再回车”这类焦点保真 smoke。
- 相对地，`tests/bootstrap_smoke.rs`、`tests/ssh_terminal_interaction_spec.rs`、`tests/sftp_context_menu_spec.rs` 对 terminal/SFTP 右键交互的覆盖更强、更贴近真实体验。

结论：正式实现必须补 pointer-driven smoke，避免“代码有 callback、测试全绿、实际右键仍然破坏选区”的假绿。

## 外部调研

本节只记录本轮实际执行过的 MCP 查询词、关键结论、采纳项与不采纳项。

### 查询词

1. `Slint TextInput right click context menu pointer-event focus selection`
2. `Slint TouchArea right click TextInput focus selection`
3. `Windows Terminal rightClickContextMenu copy paste mouse mode Shift`
4. `WezTerm bracketed paste clipboard right click paste terminal`
5. `xterm.js bracketed paste right click context menu clipboard`
6. `NIST SP 800-63B allow paste password fields`
7. `1Password clipboard clear password copy security guidance`
8. `Bitwarden clipboard clear timeout copy password guidance`
9. `official Bitwarden help clear clipboard timeout clipboard password manager`
10. `site:learn.microsoft.com Windows Terminal Shift mouse mode selection official`
11. `site:docs.slint.dev TextInput copy paste selection clear-selection official`
12. `site:docs.slint.dev TouchArea pointer-event official`
13. `site:github.com/slint-ui/slint ContextMenuArea TextInput discussion 6679`

### 关键来源与结论

#### 1. Slint

来源：

- <https://docs.slint.dev/latest/docs/slint/reference/keyboard-input/textinput>
- <https://docs.slint.dev/latest/docs/slint/reference/gestures/toucharea>
- <https://github.com/slint-ui/slint/discussions/6679>

结论：

- `TextInput` 官方已提供 `copy()`、`cut()`、`paste()`、`select-all()`、`set-selection-offsets()`、`clear-selection()` 等能力；
- `TouchArea.pointer-event` 可区分右键与修饰键；
- Slint 还存在 `ContextMenuArea` 能力，表明“字段侧感知右键 + 菜单动作回调”是框架支持的方向。

采纳：

- 采纳 Slint 文本编辑 API 作为普通输入框菜单动作的优先 backend；
- 采纳字段侧通过 `pointer-event` 捕捉右键、保留本地 selection/caret 真相的方向。

不直接采纳：

- 本轮不直接把所有普通输入替换成 Slint / 平台 native context menu；
- 不在 phase 1 里引入第二套完全独立于现有 overlay 的菜单体系，以免与 assets/SFTP overlay、测试模式、dismiss 逻辑分叉。

#### 2. Windows Terminal

来源：

- <https://learn.microsoft.com/en-us/windows/terminal/customize-settings/interaction>
- <https://learn.microsoft.com/en-us/windows/terminal/tips-and-tricks>
- <https://learn.microsoft.com/en-us/windows/terminal/tutorials/shell-integration>

结论：

- Windows Terminal 明确把 right-click menu、direct paste、copy-on-select 视作可配置的 terminal 专用行为；
- 在 mouse mode 下，`Shift` 作为本地交互逃生阀是成熟做法；
- terminal 的 copy/paste 与 selection 是 runtime surface 语义，而不是普通文本框语义。

采纳：

- 采纳“terminal 仍是独立 domain”；
- 采纳 `mouse_grabbed && !Shift` 优先远端所有权、`Shift` 为本地逃生阀的语义边界。

不采纳：

- 不照搬 Windows Terminal 的“右键直接 paste / copy”二段行为；MicaTerm 本轮目标是菜单型 `Copy / Paste`。

#### 3. WezTerm

来源：

- <https://wezterm.org/config/lua/keyassignment/PasteFrom.html>
- <https://wezterm.org/config/lua/config/canonicalize_pasted_newlines.html>
- <https://wezterm.org/config/lua/pane/send_paste.html>

结论：

- terminal paste 是显式 action，并与 bracketed paste、newline canonicalization 强耦合；
- 在 bracketed paste mode 下，换行归一化与发送方式由 terminal runtime 控制。

采纳：

- 采纳“terminal paste 必须继续走现有 runtime pipeline”；
- 采纳“普通输入框 / code block 不能复用 terminal bracketed paste 路线”。

不采纳：

- 不把 terminal 的 newline / bracketed paste 语义推广到 snippets、private key、多行 code block。

#### 4. xterm.js / xterm 家族

来源：

- 本轮 Tavily 检索到的 xterm / xterm.js 相关资料与社区讨论，核心结论一致：bracketed paste 与浏览器/终端上下文菜单属于 terminal runtime 语义。

采纳：

- 只采纳其“bracketed paste 属于 terminal，不属于普通编辑器”的边界判断。

不采纳：

- 不把浏览器 terminal 场景里的右键限制、浏览器 clipboard 约束，机械照搬到桌面 Slint 文本输入框。

#### 5. NIST SP 800-63B / 63-4

来源：

- <https://nvlpubs.nist.gov/nistpubs/specialpublications/nist.sp.800-63b.pdf>
- <https://pages.nist.gov/800-63-4/sp800-63b.html>
- NIST SP 800-63 系列关于 memorized secret / password 输入的 paste 建议（本轮通过 Tavily 检索官方资料摘要）。

结论：

- 官方指导明确支持：密码输入时应允许 paste，而不是人为禁掉密码管理器 paste 流程；旧版 800-63B PDF 的 usability table 也把“copy and paste”列为 memorized secret 的支持项。

采纳：

- 采纳“secret 字段默认允许 Paste”。

不采纳：

- 不把“允许 paste”误解为“也应默认给 secret Copy 出口”。

#### 6. 1Password / Bitwarden

来源：

- <https://support.1password.com/copy-passwords/>
- <https://support.1password.com/1password-security/>
- <https://1password.com/blog/clipboard-conundrum>
- <https://bitwarden.com/help/app-settings/>
- <https://bitwarden.com/help/product-faqs/>
- <https://bitwarden.com/help/auto-fill-browser/>

结论：

- 成熟密码管理器普遍把 secret copy 视为显式动作，而不是默认 UI 暴露；
- 很多产品提供 clipboard auto-clear，但那是额外安全工程，不是 Copy 能力本身的前置条件。

采纳：

- 采纳“secret copy 应保守、显式、allowlist 化”的产品方向。

不采纳：

- 不在本轮实现 clipboard 自动清空或内存擦除；这超出当前问题边界，也需要额外生命周期与所有权工程。

## Subagent 辩论摘要

### 配置

本轮实际启用 4 个 subagent，均显式指定：

- `model: gpt-5.4`
- `reasoning_effort: xhigh`

角色分工：

1. Desktop UI / Slint 专家
2. Terminal runtime / clipboard 专家
3. Security / secrets 专家
4. QA / regression 专家

### 第一轮：各自提出方案与风险

#### 1. Desktop UI / Slint

- 认为当前 `DialogTextField` 没有 outward selection/caret/capability contract；
- 警告现有 root overlay / modal focus 模式如果直接复用，容易在右键时破坏 `TextInput` 选区；
- 倾向“字段侧保留真相，root overlay 只负责菜单展示和动作派发”。

#### 2. Terminal runtime / clipboard

- 强调 terminal 已有成熟的 selection / copy / paste / warning / bracketed paste 体系；
- 反对任何“统一文本剪贴板层”把 terminal 包装成普通输入框；
- 认为 terminal 与普通文本必须严格分域。

#### 3. Security / secrets

- 识别出 SSH、keychain、sync vault、private key、PAT、passphrase 等大量 secret surface；
- 认为成熟产品虽然允许 secret paste，但不应因新增右键菜单而默认暴露 secret copy；
- 反对本轮引入 clipboard auto-clear / secret memory wipe。

#### 4. QA / regression

- 指出现有 modal/text-input 覆盖不足，最容易出现“测试绿但真实交互仍坏”的假绿；
- 要求增加 pointer-driven smoke test，锁定右键不丢 `focus` / `selection` / `caret`；
- 特别点名 paste warning editor 也要纳入回归。

### 第二轮：互相反驳并收敛

#### 争议点 A：Slint 已有 `ContextMenuArea` 与 `TextInput.copy()/paste()`，是否应直接改成字段自带 native menu？

- Desktop UI / Slint 观点：这说明字段侧实现是可行的，但不等于应立刻把产品菜单体系全部改成 Slint/native menu；更合理的是复用 Slint 文本 API 作为动作 backend，同时保留 MicaTerm 的 root overlay 菜单壳。
- Terminal 观点：这进一步证明普通输入框根本不需要借 terminal copy/paste 管线；两域应更明确分开。
- QA 观点：无论菜单壳体选 overlay 还是 field-local，测试都必须锁定“owner state 没串线”，不能只看 `copy()` / `paste()` 被调到了。

收敛：

- 采纳 Slint 文本 API；
- 不在 phase 1 直接用 `ContextMenuArea` / native menu 替换掉现有菜单壳；
- root overlay 继续负责统一展示、定位、dismiss、动作派发，但不持有 selection 真相。

#### 争议点 B：secret 字段在 reveal 后是否自动允许 Copy？

- Security 观点：`reveal` 只是本地可见性切换，不应自动升级为“可复制”；当前仓库现有显式白名单更偏保守设计。
- Desktop UI / Slint 观点：如果把“reveal => 自动允许 Copy”塞进通用字段层，会让组件 contract 变复杂，也容易与 public/private material 混淆。
- QA 观点：如果此处规则不稳，测试会变成猜实现而不是锁需求。

收敛：

- secret 字段默认 `Paste yes / Copy no`；
- 如确有例外，应通过显式 allowlist / 专用按钮，而不是让 reveal 自动授予复制权。

#### 争议点 C：是否做“全局透明 TouchArea 覆盖 modal，统一抓右键”？

- Desktop UI / Slint 明确反对：这最容易破坏 `TextInput` 命中与选区；
- QA 同意：这类方案最容易出现 pointer down 先打断焦点，再导致右键假绿；
- Terminal 认为这还会与 terminal / overlay 所有权边界冲突。

收敛：

- 不采用全局透明 `TouchArea` 抓右键；
- 右键命中必须发生在字段侧或紧邻字段侧的本地桥接层。

### 主线程最终决策

最终采用以下权衡后的最优方案：

1. 普通输入框与 terminal 分成两个 domain：
   - `TextEditContextMenuDomain`
   - `TerminalContextMenuDomain`
2. 普通输入框继续复用 root overlay 作为菜单展示壳，但 root overlay 不得抢 `TextInput` 焦点；
3. 字段侧负责右键命中、`field_id`、字段类型、secret policy、selection/caret 保真；
4. 菜单动作优先调用 Slint `TextInput.copy()/paste()/select-all()` 等本地 API，不把普通输入框接入 terminal paste runtime；
5. secret 字段默认允许 `Paste`，默认不暴露 `Copy`；例外必须走显式 allowlist；
6. 测试采用“contract 保结构，smoke 保交互所有权”的分层策略。

## 推荐架构

### 总体原则

- 不让 root overlay 抢 `TextInput` focus；root overlay 只负责菜单展示、placement、dismiss 和 action dispatch。
- 输入组件侧负责右键命中、当前 `field_id`、text domain、secret policy、selection/caret 保持。
- Rust view model / bootstrap 负责统一 action dispatch、clipboard capability 投影、以及和现有 overlay 状态机的最小集成。

### Domain 划分

#### 1. `TextEditContextMenuDomain`

适用范围：

- 普通 `TextInput` / `TextEdit`；
- `DialogTextField`；
- 多行 code block、snippet script、private key 编辑区；
- paste warning modal 内的本地 editor；
- 少量 bare `TextInput` outlier。

最小 bridge 信息：

- `field_id`
- `field_kind`
- `is_read_only`
- `is_secret`
- `has_selection`
- `clipboard_has_text`
- `supports_multiline`
- `can_copy`
- `can_paste`

建议实现边界：

- 字段侧通过 `pointer-event` 感知右键与锚点；
- 字段侧保留当前 selection/caret 真相；
- Rust / bootstrap 可用现有系统 clipboard helper 计算 `clipboard_has_text` 与最终 `can_paste`；
- root overlay 只拿 capability 快照与 anchor 定位，不直接拥有选区文本。

#### 2. `TerminalContextMenuDomain`

适用范围：

- workspace terminal host。

规则：

- 继续走现有 terminal host + runtime pipeline；
- `workspace_session_paste_requested -> forward_active_workspace_paste -> warning/editor -> send_session_paste -> encode_paste` 或当前等价链路不变；
- `mouse_grabbed && !Shift` 时右键优先交给远端程序；`Shift` 继续作为本地逃生阀；
- plain `Ctrl+C` 不得被本地 Copy 劫持。

### 普通输入框建议的数据流

```text
field right-click
-> field-local bridge captures anchor + field metadata + has_selection
-> Rust/bootstrap checks system clipboard text and policy
-> root overlay opens without calling focus() on menu shell
-> user clicks Copy/Paste
-> action dispatch returns to owning field_id
-> field executes TextInput.copy()/paste()/select-all() locally
-> menu dismisses
```

### Slint selection / action 策略

优先级顺序：

1. 首选利用 `TextInput` 自身 API：`copy()`、`paste()`、`select-all()`、`set-selection-offsets()`、`clear-selection()`。
2. 如果某个字段场景下 Slint 不足以直接暴露需要的 selection/caret 能力，则在组件层维护最小 selection/caret 状态，或通过 focused field command bridge 间接调用。
3. 不允许通过“先点 root overlay 导致 blur，再去 copy/paste”的方式实现，因为这正是当前 bug 根源。

### Secret 策略

- `password` / `passphrase` / `token` / `private key` / 类 secret credential 字段：`Paste` 默认可用。
- `Copy` 默认禁用；除非：
  - 该字段本身是 public label/comment/path/public key/fingerprint 等非 secret material；或
  - 产品已明确定义为 allowlist，并有专用按钮 / 明确文案。
- 不新增 clipboard 自动清空。
- 不新增 secret 内存擦除。
- reveal 不自动授予 copy 权限。

### Code Block 策略

- 多行 `Paste` 保留原始文本；
- 不使用 terminal bracketed paste 包裹；
- 不走 terminal paste warning/editor 流程；
- 缩进、tab、空行属于普通编辑语义，而不是 shell runtime 语义。

### Terminal 策略

- 如果正式实现阶段要整理 terminal 右键菜单，也必须保留 terminal 现有 pipeline，而不是把 terminal 塞进普通文本域；
- `Copy` 仍基于 terminal 自己的 selection truth；
- `Paste` 仍走 runtime clipboard + warning/editor + `encode_paste`；
- `mouse_grabbed && !Shift`、plain `Ctrl+C`、selection mirror rehydrate 等现有保障必须不回退。

## 风险与取舍

### 1. Slint TextInput selection API 边界

风险：

- 虽然 Slint 已提供 `copy()/paste()/select-all()` 等 API，但在复杂 modal overlay / pointer 命中顺序下，仍可能因为 focus 变化导致 selection 先丢失。

取舍：

- 方案上把 selection/caret 真相放回字段侧；
- 通过 pointer-driven smoke test 而不是只靠 source-contract 判断是否成功。

### 2. Overlay 抢焦点导致 selection 丢失

风险：

- 现有 assets/workspace-tab overlay 菜单会主动 focus 菜单层；这在文本输入里是高风险行为。

取舍：

- 普通输入框菜单壳不走“focus menu overlay”老路径；
- 若未来证明 root overlay 在 Slint 内无法做到不抢焦点，再评估 field-local `ContextMenuArea` 作为升级方案，而不是本轮直接替换。

### 3. Secret Copy 安全风险

风险：

- 一旦把 secret 字段与普通文本字段完全等同，右键菜单会无意中把私钥、密码、token 暴露到 clipboard。

取舍：

- 维持 `Paste yes / Copy no` 的默认保守策略；
- 例外走显式 allowlist。

### 4. terminal 与普通输入框 pipeline 混用风险

风险：

- 把 snippets / code block / private key 粘贴误接到 terminal runtime，会引入 `bracketed paste`、warning/editor、newline normalization 等错误语义。

取舍：

- 明确分出 `TextEditContextMenuDomain` 与 `TerminalContextMenuDomain`；
- 禁止共享 selection truth 与 paste transport。

### 5. 旧文档过时风险

风险：

- 2026-06-08 文档里的部分工具限制和假设已经过时；如果继续照抄，会得出错误实现边界。

取舍：

- 本轮所有结论以当前 HEAD、本会话工具能力、以及本轮实际调研为准。

## 为什么最终方案优于其他备选方案

### 1. 优于“每个输入框各写一套菜单”

- 共享 `DialogTextField` 已覆盖大部分表单字段；
- 把 bridge 做在字段层可以统一行为与测试；
- 只对少量 bare `TextInput` 做 adapter，成本更低、回归面更清晰。

### 2. 优于“用全局透明 TouchArea 覆盖 modal”

- 全局透明 `TouchArea` 最容易在 pointer down 时打断真实文本命中；
- 这正好与“右键不丢 selection / focus”的核心目标冲突。

### 3. 优于“直接使用 terminal paste pipeline 处理 code block”

- code block / script / private key 是普通编辑器文本，不是 PTY 输入；
- terminal pipeline 会引入 `bracketed paste`、warning/editor、newline normalization 等错误副作用。

### 4. 优于“平台 native menu 一步替换所有 overlay”

- 本仓库已存在 assets/SFTP/root overlay 菜单主干；
- 直接切 native menu 会让菜单体系分裂，并带来新的测试与平台差异面；
- 当前尚无证据表明 overlay 路线必然无法满足需求，因此不宜在 phase 1 一步切换。

### 5. 优于“右键直接 Paste 而不是显示菜单”

- 用户本轮目标明确要求菜单型 `Copy / Paste`；
- direct paste 既不利于 secret policy，也不利于用户先确认当前 field/caret/selection 状态；
- 与 MicaTerm 现有 terminal warning/editor 路线也不一致。

## 实施约束

- 正式实现前，必须先用 `superpowers:using-git-worktrees` 在新窗口中切入独立 worktree。
- 正式实现每个任务前，必须先走 `@superpowers:test-driven-development`。
- 如果实现阶段发现当前 HEAD 代码事实、测试行为、或 Slint 能力与本设计冲突，必须停止并使用 `@superpowers:systematic-debugging` 做根因核对。
- 在声称完成前，必须使用 `@superpowers:verification-before-completion`，以实际验证结果而不是主观判断为准。
