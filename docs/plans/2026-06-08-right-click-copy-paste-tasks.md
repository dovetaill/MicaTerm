# Right-Click Copy/Paste Tasks

日期: 2026-06-08
执行者: Codex
状态: 待在新 worktree 实施

> REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.

## Execution Notes

- REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.
- Use `@superpowers:test-driven-development` before each task.
- If a test or repo finding contradicts this plan, stop and use `@superpowers:systematic-debugging` before changing the plan.
- Before claiming complete, use `@superpowers:verification-before-completion`.
- 正式实现必须在 `/home/wwwroot/mica-term/.worktrees/right-click-copy-paste` 或同等命名的新 worktree 中进行。
- 本轮文档编写阶段不要执行会修改业务代码的命令。

## Task 0：创建 / 切换独立 worktree（仅正式实现阶段）

### Goal

为后续实现准备隔离工作目录，避免污染当前工作树；本轮不要执行。

### Files expected to modify

- 无业务文件；仅 worktree / 分支元数据。

### Tests to add/update

- 无。

### Implementation notes

- 使用 `superpowers:using-git-worktrees`。
- 目标目录建议：`/home/wwwroot/mica-term/.worktrees/right-click-copy-paste`。
- 创建后先确认当前分支、工作树干净性、以及只在新 worktree 内进行后续编码。

### Verification commands

```bash
git worktree list
pwd
git branch --show-current
```

### Risks

- 如果在主工作树直接实现，容易与当前未提交改动交叉污染；
- 若 worktree 未正确切换，后续 TDD/提交边界会失真。

## Task 1：锁定 text context menu domain 的 failing tests

### Goal

先把统一 capability/domain 的目标行为用 failing tests 钉住，避免先写 UI 分支逻辑再回补规则。

### Files expected to modify

- `tests/text_context_menu_spec.rs`（新增）
- `src/shell/context_menu.rs` 或新增 sibling domain 文件（实现阶段再定）
- 可能需要最小桥接状态定义文件，例如 `src/shell/view_model.rs`

### Tests to add/update

- 新增 capability matrix：
  - `PlainTextField`：有选区时 `Copy` enabled；editable 且 clipboard 有文本时 `Paste` enabled；
  - `SecretField`：`Paste` enabled（editable 且 clipboard 有文本时），`Copy/Cut` disabled；
  - `PublicMaterialField`：遵守 selection 与 editable/read-only；
  - `CodeTextField`：selection-based `Copy`、editable `Paste`；
  - `WorkspaceTerminal`：仅在 `has_selection` / `can_paste` 时开启对应动作。
- 新增 `SecretKind` / target kind 相关 rule tests。

### Implementation notes

- 严格先 RED：先写 capability rule，再跑出期望失败，再开始实现。
- 尽量把文本语义做成独立 resolver，不要把 assets `SelectionContext` 生硬堆满文本字段专用状态。
- 若最终选择复用现有 `src/shell/context_menu.rs`，也要保持“资产 menu 规则”和“文本 menu 规则”分离可读。

### Verification commands

```bash
cargo test --test text_context_menu_spec -- --nocapture
cargo test --test assets_context_menu_spec -- --nocapture
```

### Risks

- 过早把 text 规则散落到 Slint 组件，会失去统一 capability 真相源；
- 若直接修改 assets rule 而不分域，容易造成资产菜单回归。

## Task 2：实现普通 TextInput / TextEdit 右键菜单能力

### Goal

为共享 `DialogTextField` 和必要的裸 `TextInput` 建立统一 right-click bridge，让普通文本字段能稳定获得 `Copy/Paste`。

### Files expected to modify

- `ui/components/modal-chrome.slint`
- `ui/components/assets-snippet-package-modal.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`
- 可能新增 `ui/components/text-context-menu-overlay.slint`

### Tests to add/update

- `tests/modal_input_select_contract_spec.rs`
- 新增 `tests/text_context_menu_smoke.rs` 或等价 smoke 文件
- 可能扩 `tests/window_shell.rs`

### Implementation notes

- 先为 `DialogTextField` 写 failing contract/smoke，再接线；
- 裸 `TextInput` 至少覆盖 `assets-snippet-package-modal.slint`，不要假设 shared primitive 覆盖全部；
- 菜单 overlay 打开时不能像 assets menu 那样抢焦点；
- 如果 Slint 普通文本复制/粘贴调用面不足，优先用桥接 callback 明确路由，不要写隐式 UI 猜测逻辑。

### Verification commands

```bash
cargo test --test modal_input_select_contract_spec -- --nocapture
cargo test --test text_context_menu_smoke -- --nocapture
cargo test --test window_shell -- --nocapture
```

### Risks

- 如果 overlay 抢焦点，容易破坏文本 caret/selection；
- 若只改 shared field，裸 `TextInput` 场景会漏掉；
- 若桥接协议不够小，后续 secret/code/terminal 会被过度耦合。

## Task 3：实现 workspace terminal 右键 Copy/Paste，并接入既有 terminal paste pipeline

### Goal

把 terminal 菜单能力统一到 Rust resolver / root overlay，但保留现有 terminal hit-test、selection 与 `mouse_grabbed + Shift override` 语义。

### Files expected to modify

- `ui/shell/terminal-session-host.slint`
- `ui/shell/workspace-pane.slint`
- `ui/app-window.slint`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- 可能涉及 `src/shell/view_model.rs` / text context menu resolver

### Tests to add/update

- `tests/workspace_tabs_spec.rs`
- `tests/workspace_paste_warning_modal_spec.rs`
- `tests/terminal_session_spec.rs`
- `tests/ssh_terminal_interaction_spec.rs`
- `tests/bootstrap_smoke.rs`

### Implementation notes

- terminal 右键菜单只迁“菜单解析/显示/派发”，不迁 terminal 选区状态机；
- `Paste` 必须继续走：`workspace_session_paste_requested -> forward_active_workspace_paste -> warning/editor -> send_session_paste -> encode_paste`；
- plain `Ctrl+C` 绝不能被本地 copy 逻辑覆盖；
- `mouse_grabbed && !Shift` 时，右键仍优先交给远端程序；
- 若 `can_paste` capability 本地没有现成真相源，先加一个最小 Rust capability 契约，再做 UI enable/disable。

### Verification commands

```bash
cargo test --test workspace_tabs_spec -- --nocapture
cargo test --test workspace_paste_warning_modal_spec -- --nocapture
cargo test --test terminal_session_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

### Risks

- 最容易出现的回归是旁路 terminal paste pipeline；
- `Esc` 关闭菜单与发送给远端 session 的边界如果没收好，会出交互副作用；
- 过度复用 assets overlay 焦点模型会破坏 terminal 即时输入。

## Task 4：实现 SSH / key / credential / snippet / code block 场景接线

### Goal

把统一 text context menu bridge 接到实际业务表单，覆盖 SSH、keychain、vault、snippet/code block 这些用户最关心的 surface。

### Files expected to modify

- `ui/components/assets-ssh-connection-modal.slint`
- `ui/components/assets-keychain-identity-modal.slint`
- `ui/components/assets-keychain-ssh-key-modal.slint`
- `ui/components/assets-snippet-modal.slint`
- `ui/components/assets-snippet-package-modal.slint`
- `ui/components/sync-vault-modal.slint`
- `src/app/bootstrap.rs`

### Tests to add/update

- `tests/assets_modal_smoke.rs`
- `tests/keychain_modal_smoke.rs`
- `tests/keychain_key_actions_spec.rs`
- 相关 `tests/*credential*` / `tests/*ssh*` / `tests/*snippet*` 文件
- `tests/text_context_menu_smoke.rs`（若 Task 2 已新增则继续扩充）

### Implementation notes

- 先按 surface 逐个 RED，不要“一次性全接所有字段”；
- 最小闭环优先级建议：
  1. SSH password
  2. SSH private key
  3. keychain identity password
  4. keychain public key / fingerprint
  5. snippet script
  6. snippet package name
- 普通字段与 secret 字段必须走不同 capability；
- snippet/code block 的 paste 必须保持原始文本语义，不借用 terminal normalization helper。

### Verification commands

```bash
cargo test --test assets_modal_smoke -- --nocapture
cargo test --test keychain_modal_smoke -- --nocapture
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test credential_store_spec -- --nocapture
cargo test --test keychain_secret_store_spec -- --nocapture
cargo test --test text_context_menu_smoke -- --nocapture
```

### Risks

- private key、public key、fingerprint、snippet script 的语义差异大，若 capability 不明确会互相污染；
- 只在一个 modal 做特判会逐步演变成不可维护的字段例外表。

## Task 5：secret-field 安全收口

### Goal

确保 `password / passphrase / token / proxy password / master password / private key` 默认 fail-closed，不因右键菜单新增 secret 复制出口。

### Files expected to modify

- text capability resolver 所在 Rust 文件
- `src/app/bootstrap.rs`
- 可能涉及 `src/shell/view_model.rs`
- 安全相关测试文件

### Tests to add/update

- `tests/keychain_key_actions_spec.rs`
- `tests/logging_runtime.rs`
- `tests/panic_logging.rs`
- 可能需要新建 `tests/secret_context_menu_spec.rs`

### Implementation notes

- 先 RED：明确断言 secret-field 默认没有 `Copy/Cut`；
- 保留 public key 的显式 copy 白名单，不把它推广到 private key；
- 用 sentinel secret 验证日志/失败输出不包含真实 payload；
- 设计上应表述为“本功能不新增 secret 右键 Copy/Cut affordance”，不要试图顺手解决更大的 in-memory secret 生命周期问题。

### Verification commands

```bash
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test logging_runtime -- --nocapture
cargo test --test panic_logging -- --nocapture
```

### Risks

- private key 在不同 modal 里 masked/unmasked 不一致，若实现期偷懒依赖 `password-mode` 会误判；
- 日志和测试输出是最容易被忽视的泄漏面。

## Task 6：UI overlay placement、dismiss、focus/selection 稳定性

### Goal

收口跨 surface 的 placement / dismiss / reposition / focus 行为，保证菜单打开与关闭都不破坏编辑连续性。

### Files expected to modify

- `ui/app-window.slint`
- 可能新增/修改 `ui/components/text-context-menu-overlay.slint`
- `ui/shell/terminal-session-host.slint`
- `ui/components/modal-chrome.slint`
- `src/app/bootstrap.rs`

### Tests to add/update

- `tests/window_shell.rs`
- `tests/bootstrap_smoke.rs`
- `tests/text_context_menu_smoke.rs`
- 可能的 overlay/source-contract smoke

### Implementation notes

- 必须覆盖：
  - outside click 关闭；
  - `Esc` 关闭；
  - window blur 关闭；
  - 已打开菜单时再次右键直接 reposition；
  - 打开菜单不丢失 focus/selection/caret；
  - 菜单关闭后输入立即恢复。
- terminal 与普通文本的焦点收口不完全相同，但都不能使用 assets overlay 的 `focus-menu()` 抢焦点模式。
- 如果右键重定位与 dismiss layer 冲突，先停下来做 `systematic-debugging`，不要通过“吞事件”硬修。

### Verification commands

```bash
cargo test --test window_shell -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test text_context_menu_smoke -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

### Risks

- 这是最容易出现“看起来能用，实际焦点已经坏了”的阶段；
- right-click reposition 与 dismiss layer 的事件优先级若处理错误，菜单会闪烁或无法换锚点。

## Task 7：全量验证与文档同步

### Goal

在新 worktree 中完成最终 verification，确认 tests/contracts 文档与行为一致，再同步更新文档说明。

### Files expected to modify

- 本轮新增/修改的实现文件
- `docs/plans/2026-06-08-right-click-copy-paste-*.md`（若实现期确认有轻微偏差再同步）

### Tests to add/update

- 无新增行为测试目标；重点是跑全量验证并补漏。

### Implementation notes

- 用 `superpowers:verification-before-completion` 做最后收口；
- 不能因为单条 smoke 通过就宣称完成；
- 若最终实现与本设计存在偏差，必须先更新文档再结束工作。

### Verification commands

```bash
cargo test --test text_context_menu_spec -- --nocapture
cargo test --test workspace_tabs_spec -- --nocapture
cargo test --test workspace_paste_warning_modal_spec -- --nocapture
cargo test --test modal_input_select_contract_spec -- --nocapture
cargo test --test terminal_session_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test assets_modal_smoke -- --nocapture
cargo test --test keychain_modal_smoke -- --nocapture
cargo test --test keychain_key_actions_spec -- --nocapture
cargo test --test credential_store_spec -- --nocapture
cargo test --test keychain_secret_store_spec -- --nocapture
cargo test --test logging_runtime -- --nocapture
cargo test --test panic_logging -- --nocapture
bash tests/assets_context_menu_ui_contract_smoke.sh
```

### Risks

- 若没有 fresh verification output，就不能声称任务完成；
- 若中途发现 terminal capability 或 Slint text bridge 与文档假设不符，必须回到计划/设计同步，不要硬冲到收尾。
