# Input Field Right-Click Copy/Paste Requirements

日期: 2026-06-09
执行者: Codex
状态: 文档阶段；正式实现必须在新窗口与独立 `.worktrees` 中进行

## 说明

- 本轮只做调研与文档产出，不修改 Rust / Slint / 测试业务代码。
- 正式实现必须在新的工作窗口和 `/home/wwwroot/mica-term/.worktrees/input-field-right-click-copy-paste` 或同等命名的新 worktree 中进行。
- 本需求以当前 HEAD 的本地代码、现有测试、git log、以及外部 MCP 调研为准；2026-06-08 旧文档只可继承约束，不能直接视为已修复成功。
- 经本轮核查，`docs/plans/2026-06-08-right-click-copy-paste-*.md` 对应的 git 证据仍停留在 `78c1bbf docs: add right-click copy paste planning`，未发现后续输入框右键 Copy / Paste 的实现提交，因此结论必须是：旧方案已整理，但未落地，不能视为修复成功。

## 背景

当前在 MicaTerm 中，多数普通输入框的复制粘贴主要依赖键盘快捷键。对以下场景进行鼠标右键操作时，现状通常是：

- 右键会取消当前选中，或破坏 `focus` / `selection`；
- 菜单若存在，也不是面向普通输入框文本能力的 `Copy / Paste` 菜单；
- 用户无法像成熟桌面应用一样，在保持当前输入上下文的前提下，用鼠标右键完成 Copy / Paste。

这个问题在新建 SSH 连接、密钥/凭据录入、snippets / code block 编辑等表单里最明显，因为这些区域大量依赖文本编辑，又包含多类普通字段、secret 字段、以及多行文本字段。

## 覆盖 Surface

本轮需求覆盖以下普通输入与编辑 surface：

1. 新建 / 编辑 SSH 连接表单。
2. 新建密钥、Keychain Identity、private key、passphrase、credential / vault 相关表单。
3. snippets / code block 编辑区，包括多行脚本、private key 文本、多行说明性配置文本等。
4. 共享 `DialogTextField`，以及少量未复用共享组件的裸 `TextInput` / `TextEdit` 风格输入面。

相邻但不同域：

- `workspace terminal` 与普通表单输入框是相邻功能，但不是同一输入域。
- terminal 的 Copy / Paste 必须继续走现有 terminal pipeline，不应与普通输入框右键菜单混为一体。

## In Scope

本轮需求范围内必须明确的能力：

1. 普通 `TextInput` / `TextEdit` / 多行 code block 的右键菜单入口。
2. `Copy` / `Paste` 的 enable-disable capability 投影：
   - 是否有选区；
   - 是否只读；
   - 系统 clipboard 是否有可粘贴文本；
   - 是否属于 secret 字段；
   - 是否允许多行粘贴。
3. 菜单定位与交互：
   - 菜单相对右键锚点定位；
   - 菜单 dismiss；
   - 外部点击关闭；
   - `Esc` 关闭；
   - 窗口失焦关闭；
   - 打开菜单时不破坏当前输入框的 `focus` / `selection` / `caret`。
4. secret 字段的特殊规则：
   - `Paste` 默认可用；
   - `Copy` 默认保守，不能因为有右键菜单就自动给出 secret copy 出口。
5. 多行文本 / code block 的粘贴边界：
   - 保留原始换行、缩进、tab；
   - 不能误用 terminal `bracketed paste` 规则。
6. 后续正式实现阶段的 TDD、回归验证、worktree 约束。

## Out of Scope

以下内容不属于本轮目标：

1. 本轮不实现代码，只产出 requirements / design / tasks 文档。
2. 不重写 Slint 通用输入组件库，不做“大一统 UI 库重构”。
3. 不直接替换成平台 native menu；除非后续调研和原型验证证明当前 overlay 路线无法满足“右键不丢焦点/不丢选区”的硬约束，才可作为后续升级方案讨论。
4. 不修改 SSH 协议、认证流程、vault 加密模型、keychain 数据模型。
5. 不引入 clipboard 自动清空、secret 内存擦除、粘贴后销毁缓存等超范围工程。
6. 不把 terminal 的 `Copy / Paste` runtime 管线挪作普通输入框通用实现。
7. 不要求本轮顺手修复 assets / SFTP 以外的其他 context menu 体系问题；但正式实现不能破坏这些既有能力。

## 用户故事

### 1. 普通文本字段

- 作为用户，我希望在普通文本字段中先选中文本，再右键打开菜单时选区仍然保留，并能直接点 `Copy`。
- 作为用户，我希望在普通文本字段无选区但 clipboard 有文本时，右键后 `Paste` 可用。

### 2. 多行 code block / script / key material 字段

- 作为用户，我希望在 snippets / code block / private key / script 等多行字段中右键粘贴时，原始换行、缩进、tab 被完整保留。
- 作为用户，我不希望普通编辑器的粘贴被 terminal 的 `bracketed paste` 或 paste warning 规则污染。

### 3. secret 字段

- 作为用户，我希望 password / passphrase / token / private key 字段默认允许 `Paste`，因为这符合密码管理器与桌面应用常见体验。
- 作为产品和安全边界，我不希望 secret 字段默认暴露 `Copy`；除非该字段已有明确 reveal / allowlist 语义，或后续设计给出额外安全论证。

### 4. 焦点与菜单行为

- 作为用户，我希望右键打开菜单时不丢失当前 `focus`、`selection`、`caret`。
- 作为用户，我希望点击菜单外、按 `Esc`、或窗口失焦时菜单关闭，但输入内容与选区状态不被破坏。

### 5. 相邻 terminal 域

- 作为用户，我希望普通输入框右键菜单增强后，不影响 terminal 现有 `Ctrl+Shift+C` / `Ctrl+Shift+V`、paste warning、`mouse_grabbed + Shift` 等既有行为。

## 验收标准

### A. 基础文本编辑

1. 在输入框内先选中文字，再右键：
   - 选区仍保留；
   - 菜单出现；
   - `Copy` 可用；
   - 点击 `Copy` 后，系统 clipboard 更新为所选文本。
2. 在输入框无选区但 clipboard 有文本时右键：
   - 菜单出现；
   - `Paste` 可用；
   - 点击 `Paste` 后，文本插入当前 field 的 caret 位置。
3. 打开右键菜单不会破坏已有键盘快捷键：
   - `Ctrl+C` / `Ctrl+V` 在普通输入框中继续按原有逻辑工作；
   - terminal 中的 `Ctrl+Shift+C` / `Ctrl+Shift+V` 继续按原有逻辑工作。

### B. 多行文本 / code block

1. 在 code block / script / private key 多行字段中粘贴多行文本：
   - 换行、缩进、tab 保持原样；
   - 不出现 terminal `bracketed paste` 包裹；
   - 不触发 terminal paste warning/editor 流程。

### C. Secret Policy

1. `password` / `passphrase` / `private key` / `token` 等 secret 字段：
   - `Paste` 默认可用；
   - 默认不暴露 `Copy`，除非该字段已有明确 allowlist 或后续设计批准。
2. public label/comment/path/public key/fingerprint 等非 secret 字段：
   - `Copy` / `Paste` 是否可用由字段只读性、选区状态、clipboard 状态共同决定。

### D. 菜单生命周期

1. 外部点击关闭菜单。
2. `Esc` 关闭菜单。
3. 窗口失焦关闭菜单。
4. 菜单关闭后，输入框仍保持合理的 focus / selection / caret 行为，不出现“菜单消失但输入框已经失焦”的体验退化。

### E. 非回归

1. 不破坏 assets / SFTP 既有 context menu。
2. 不破坏 workspace terminal 既有 copy/paste、paste warning、`mouse_grabbed + Shift` 行为。
3. 不把普通输入框与 terminal 误接到同一 paste pipeline。

## 后续测试边界

正式实现阶段的测试必须覆盖：

- 共享 `DialogTextField`；
- 少量裸 `TextInput` / `TextEdit` outlier；
- SSH 连接 modal；
- keychain / credential / sync secret modal；
- snippets / code block 多行编辑区；
- paste warning editor 等同类本地多行编辑器；
- assets / SFTP / terminal 非回归。

且必须避免“测试全绿但真实体验仍坏”的假绿：除了 source-contract / unit test，还需要 pointer-driven smoke test 锁定右键不丢 `focus` / `selection` / `caret`。
