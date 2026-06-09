# Right-Click Copy/Paste Requirements

日期: 2026-06-08
执行者: Codex
状态: 文档阶段，未进入实现

## 背景

当前 MicaTerm 在以下 surface 中，复制/粘贴主要依赖键盘快捷键，鼠标右键缺少一致、可发现、可测试的 copy/paste 入口：

- `workspace SSH terminal` 主体区域；
- 新建/编辑 SSH 连接表单；
- keychain / credential / vault / private key / passphrase 相关表单；
- snippets / code block 编辑区。

本轮只产出 requirements / design / tasks 文档，不实现 Rust / Slint / 测试代码。正式实现必须在新的工作窗口中，并在 `/home/wwwroot/mica-term/.worktrees/<feature-name>` 内完成。

## 范围

### In scope

- `workspace terminal` 的右键 `Copy` / `Paste` 行为定义；
- SSH 表单、key/credential/vault 相关文本字段的右键 copy/paste 规则；
- snippet / code block 编辑区的右键 copy/paste 规则；
- 统一的 text-edit context menu capability 模型；
- 右键菜单 placement、dismiss、focus/selection 保持约束；
- 后续 TDD/验证边界与任务拆分。

### Out of scope

- 正式功能实现；
- terminal renderer 重构；
- 资产树/Explorer 现有 assets context menu 的 IA 重做；
- SSH 协议、认证逻辑或 vault 加密模型修改；
- clipboard 自动清空、secret 内存擦除等超出本轮现有架构的安全工程；
- 平台 native menu 替换当前自绘/overlay 体系；
- 与本需求无关的 Slint 通用 TextInput 组件库重写。

## 用户故事

1. 作为 terminal 用户，我在 terminal 有选区时右键，希望看到可用的 `Copy`，并复制当前选区，而不是整屏文本。
2. 作为 terminal 用户，我在 terminal 右键 `Paste` 时，希望仍然走现有 paste warning / bracketed paste / CRLF normalization 管线，而不是旁路直发。
3. 作为 SSH 表单用户，我在 host/user/port/备注等普通文本字段里希望使用右键 `Copy` / `Paste`，不必记快捷键。
4. 作为密码/passphrase/token 用户，我希望右键 `Paste` 可用，但系统默认不要给我一个容易误触的 secret `Copy` 入口。
5. 作为 private key / code block 用户，我希望多行 `Paste` 保留原始换行、缩进、tab 语义，而不是套用 terminal shell 粘贴语义。
6. 作为桌面应用用户，我希望菜单跟随鼠标、不会跑出窗口、按 `Esc` / 外部点击 / 窗口失焦能关闭，且打开菜单不破坏当前 focus 与 selection。

## 功能需求

### 1. 统一模型与状态真相源

- 必须存在统一的 text-edit context menu capability 模型，禁止每个 `TextInput` / modal 随机各做一份菜单。
- 必须优先继承现有 `Rust -> bootstrap -> Slint root overlay` 架构约束，而不是新增与现有壳层完全平行的状态机。
- Rust 负责：
  - target 语义；
  - 菜单项集合；
  - enable/disable capability；
  - action dispatch 真相源。
- Slint 负责：
  - right-click 命中；
  - target metadata 上报；
  - overlay 呈现；
  - placement/dismiss。
- 若实现阶段证明现有 assets context menu domain 无法直接承载 live text editing 语义，允许新增轻量 `text-edit-context-menu` 子域；但必须继续复用 root overlay 承载路线，而不是做 modal-local 或每控件自管菜单。

### 2. Surface 分类

后续实现至少要区分以下语义 surface：

- `WorkspaceTerminal`：terminal runtime interaction surface；
- `PlainTextField`：普通单/多行文本字段；
- `CodeTextField`：snippet / code block 编辑字段；
- `PublicMaterialField`：public key、fingerprint 等非 secret 但可能是多行或只读的字段；
- `SecretField`：password、passphrase、token、proxy password、master password、private key 等 secret 字段。

说明：

- `SecretField` 的判定不能只依赖 `password-mode` 外观；
- 明文可编辑 private key 仍属于 `SecretField`；
- `PublicMaterialField` 与 `PlainTextField` 的差异在于：前者允许已有显式 copy 例外（如 public key），但是否允许 paste 仍需服从 read-only / editable 状态。

### 3. Workspace terminal 规则

- `Copy` 仅在 terminal 当前存在选区时可用。
- `Paste` 仅在 clipboard 有文本，且 Rust 能确认当前 terminal session 处于可接收本地 paste 的状态时可用。
- terminal 右键 `Paste` 必须走现有 terminal paste pipeline，不得绕过：
  - clipboard 读取；
  - terminal 专用 CRLF normalization；
  - multiline / large paste warning；
  - bracketed paste 包装；
  - `send_session_paste()` / runtime control。
- `Ctrl+C` 继续保留 terminal native interrupt / SIGINT 语义，不得被右键菜单设计破坏。
- 若 session 处于 `mouse_grabbed` 场景，必须保留现有 `Shift` 本地逃生阀语义：默认右键优先服务远端 TUI，本地菜单只能在本地可控条件下打开。
- terminal 菜单首发必需项为 `Copy` / `Paste`；`Select All` / `Find` 可保留为后续或同域扩展，但不阻塞本轮 copy/paste 主链路。

### 4. 普通 TextInput / TextEdit / 表单字段规则

- 普通文本字段：
  - `Copy`：有选区时可用；
  - `Paste`：字段 editable 且 clipboard 有文本时可用。
- `Select All`：
  - 允许作为后续低风险扩展；
  - 若首发加入，必须仅限普通可编辑文本与 code field，并补独立 RED tests。
- `Cut`：
  - 不是本轮 mandatory scope；
  - 若首发加入，只允许出现在非 secret、editable、非 terminal 的文本字段上，并通过 TDD 单独验证。
- 菜单 action 的 enable/disable 不能由 Slint 组件本地猜测，必须由统一 capability 结果驱动。

### 5. SecretField 规则

- `Paste` 默认允许，但仅在字段 editable 且 clipboard 有文本时可用。
- `Copy` 默认禁止。
- `Cut` 默认禁止。
- `Select All` 不作为 phase 1 的 secret-field 显式右键项。
- secret-field 默认 fail-closed；若某字段已有明确产品级例外（例如 public key copy 按钮），必须在 requirements/design 中显式标注例外原因。
- secret / private key / passphrase / token 内容不得写入：
  - debug/log/tracing 输出；
  - panic/expect message；
  - menu feedback text；
  - snapshot / fixture / 失败断言文本。

### 6. Private key / snippet / code block 多行文本规则

- private key / snippet / code block 的 `Paste` 必须保留原始文本语义，包括：
  - 换行；
  - 缩进；
  - 制表符。
- 这些字段不得复用 terminal 的 shell-oriented CRLF normalization / paste warning / bracketed paste 逻辑。
- `Copy` 必须基于用户选区；无选区时不得默认复制整个 code block。
- 若 future 产品另有“copy whole block”需求，必须作为单独动作设计，不与 generic text `Copy` 混用。

### 7. 菜单 placement / dismiss / focus 规则

- 菜单必须跟随鼠标 anchor 打开，并在根窗口坐标系中做边界避让。
- 菜单不能被 modal body 裁切；若 modal 内字段触发菜单，菜单仍应在 root overlay 层安全显示。
- `Esc`、外部点击、窗口失焦必须关闭菜单。
- 已打开菜单时，用户在另一处再次右键，应直接重定位并刷新 target，不要求先手动关闭旧菜单。
- 打开菜单不应破坏当前输入框或 terminal 的 focus / selection / caret 状态。
- 菜单关闭后，输入应能立即继续，不要求额外点击恢复焦点。

### 8. 状态判断来源

后续实现必须明确以下 capability 的来源，而不是在 UI 层猜测：

- `has_selection`：来自 terminal Rust selection truth 或 text field 当前选区状态；
- `clipboard_has_text`：来自系统 clipboard 可读且包含文本；
- `field_editable/read_only`：来自字段语义或 view-model state；
- `field_secret_kind`：来自显式语义标签，而不是 `password-mode`；
- `session_can_paste`：来自 terminal Rust capability，而不是 tab 标题、连接名称或 UI 推断。

## 非功能需求

### 安全

- secret-field 默认 fail-closed；
- 不新增 secret 复制路径；
- 不记录 clipboard/secret 实际内容；
- 现有 public key copy 白名单继续保留，但不能推导成 private key 也可复制。

### 跨平台

- 交互以 Windows 优先体验校准，但业务模型不能写死为 Windows-only；
- 平台差异应尽量收敛在 clipboard/provider/placement 细节，不扩散到需求语义层。

### 可测试性

- 每条核心需求都必须能转化为后续 TDD 的 RED/GREEN 测试；
- terminal、普通文本、secret-field 需要分层验证；
- source-contract、unit、smoke、integration、logging/panic 防泄漏测试要能分别承载不同风险。

### 性能与稳定性

- 打开/关闭菜单不应阻塞 UI 主流程；
- terminal paste 仍复用现有 warning/editor modal，不引入第二条大文本传输链；
- overlay 打开/关闭不应引入显著闪烁或焦点抖动。

### 可访问性

- `Esc` 关闭、焦点恢复与菜单禁用态必须一致；
- 若后续加入键盘菜单导航，不得以破坏文本编辑焦点为代价；
- secret reveal 状态不应因为菜单打开而变化。

## 验收标准

以下验收标准必须可转化为后续 Given/When/Then 测试：

1. `WorkspaceTerminal` 有选区、clipboard 无关时：
   - Given active terminal selection exists
   - When user right-clicks terminal in local-menu-allowed state
   - Then `Copy` is enabled and copies current selection through existing Rust selection truth.
2. `WorkspaceTerminal` 无选区时：
   - Given no terminal selection
   - When user opens terminal context menu
   - Then `Copy` is disabled.
3. `WorkspaceTerminal` paste 主链路：
   - Given clipboard contains multiline text and terminal session is paste-capable
   - When user invokes terminal `Paste`
   - Then the request reuses existing paste warning / bracketed paste / normalization pipeline.
4. `Ctrl+C` 终端语义：
   - Given active terminal session
   - When user presses plain `Ctrl+C`
   - Then the terminal still sends native interrupt semantics rather than local clipboard copy.
5. 普通表单字段：
   - Given editable plain text field with selected text
   - When user opens text context menu
   - Then `Copy` is enabled and `Paste` is enabled only when clipboard has text.
6. secret-field：
   - Given editable secret field with selected or unselected content
   - When user opens text context menu
   - Then `Paste` may be enabled, but `Copy` and `Cut` are absent or disabled by default.
7. private key / code block：
   - Given editable multiline private key or snippet code field
   - When user pastes multiline text with indentation and tabs
   - Then the inserted text preserves original newline/indent/tab semantics and does not pass through terminal normalization.
8. focus/selection 稳定性：
   - Given field or terminal already has focus and selection/caret state
   - When user opens and closes the right-click menu without executing a destructive action
   - Then focus and selection/caret remain usable without extra recovery clicks.
9. dismiss：
   - Given a text context menu is open
   - When user presses `Esc`, clicks outside, or the window loses focus
   - Then the menu closes.
10. 安全日志：
   - Given sentinel secret values are used during copy/paste regression tests
   - When actions fail, warn, or panic
   - Then logs, panic output, snapshots, and debug strings do not contain secret payload text.

## 风险与待确认项

1. 当前本地代码里未发现稳定的 terminal `session_can_paste` / writable capability 真相源；实现阶段需要先定义最小可测 capability 契约。
2. 当前 `DialogTextField` 只明确暴露了 `focus-input()` / `select-all()` 风格能力；普通文本字段的统一 `copy/paste` 调用面在本地代码中尚未成型，需要实现期确认。
3. `ui/components/assets-snippet-package-modal.slint` 仍使用裸 `TextInput`，如果只覆盖共享 field primitive，会漏掉该场景。
4. private key 在不同 modal 中存在 masked / unmasked 表现不一致，说明 secret 语义不能依赖控件外观。
5. `workspace terminal` 当前已有 host-local 菜单实现；正式实现要避免“双菜单状态并存”造成回归。
6. terminal `Esc` 当前路径与菜单关闭的关系需要在实现阶段定清，否则可能出现“关闭菜单同时把 Escape 发给远端”的副作用。
7. `Cut` / `Select All` 是否纳入首发需要根据实现期 TDD 结果和风险控制决定；本轮文档只把 copy/paste 作为 mandatory scope。
