# Input Field Right-Click Copy/Paste Tasks

日期: 2026-06-09
执行者: Codex
状态: 文档阶段；供后续新窗口中的正式实现执行

REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

Use @superpowers:test-driven-development before each implementation task.

If repo facts contradict this plan, stop and use @superpowers:systematic-debugging.

Before claiming complete, use @superpowers:verification-before-completion.

正式实现必须在 `/home/wwwroot/mica-term/.worktrees/input-field-right-click-copy-paste` 或同等命名新 worktree 中进行，并先使用 `superpowers:using-git-worktrees`。

本轮文档阶段不要执行会修改业务代码的命令；不要在当前工作区直接改 Rust / Slint / 测试业务代码。

## 执行总则

- 先在新 worktree 中执行，不要在当前主工作区直接动手。
- 每个任务都先写 failing tests，再补实现，再跑验证。
- 如果本地代码事实、测试结果或 Slint 行为与本计划冲突，立即停下，先做系统化排障，不要硬套计划。
- contract test 负责锁结构；smoke test 负责锁交互所有权与真实体验，二者都不能省。

## Task 0 - 创建并切换独立 worktree

- Goal
  - 在正式实现开始前，为该功能创建独立开发环境，避免污染当前工作区，并满足文档约束。
- Files expected to inspect/modify
  - 主要是 `.worktrees` 目录与 git worktree 元数据；本任务不应改业务代码。
- Tests to add/update
  - 无。
- Implementation notes
  - 先使用 `superpowers:using-git-worktrees`。
  - 目标目录建议为 `/home/wwwroot/mica-term/.worktrees/input-field-right-click-copy-paste`。
  - 创建后先确认 `git status` 干净，再开始后续任务。
- Verification commands
  - `git worktree list`
  - `git -C /home/wwwroot/mica-term/.worktrees/input-field-right-click-copy-paste status --short`
  - `pwd`
- Risks
  - 在错误工作区直接编码；
  - 把当前主工作区已有变更与新功能混在一起。

## Task 1 - 先写 failing tests，锁定输入框右键不丢 focus / selection

- Goal
  - 在任何实现前，先把“右键打开菜单不丢 `focus` / `selection` / `caret`”写成会失败的测试，防止实现后出现假绿。
- Files expected to inspect/modify
  - `tests/modal_input_select_contract_spec.rs`
  - `tests/assets_modal_smoke.rs`
  - `tests/keychain_modal_smoke.rs`
  - `tests/sync_vault_modal_smoke.rs`
  - `tests/workspace_paste_warning_modal_spec.rs`
  - 如需更高保真，可补充 `tests/bootstrap_smoke.rs` 中针对本地 editor/modal 的场景。
- Tests to add/update
  - modal 输入框：选中后右键，菜单出现但选区仍保留；
  - modal 输入框：右键 padding 区域不会把 `TextInput` ownership 弄丢；
  - paste warning editor：先聚焦 editor，再右键，再按 Enter，不能错误落到 confirm；
  - smoke 需断言“下一次键入仍进入原输入框”，而不是误发到 terminal / 其他 surface。
- Implementation notes
  - 优先做 pointer-driven smoke；
  - contract test 只辅助锁结构，不可替代交互回归；
  - 测试命名要把“focus 保持 / selection 保持 / no leak”写进断言文案。
- Verification commands
  - `cargo test --test modal_input_select_contract_spec`
  - `cargo test --test assets_modal_smoke`
  - `cargo test --test keychain_modal_smoke`
  - `cargo test --test sync_vault_modal_smoke`
  - `cargo test --test workspace_paste_warning_modal_spec`
- Risks
  - 只写 source-contract，不写真实交互 smoke；
  - 断言只看菜单开没开，不看 `selection`、`caret`、后续键入归属。

## Task 2 - 为 `DialogTextField` / 必要裸 `TextInput` 设计最小 text context menu bridge

- Goal
  - 为普通输入框建立最小 bridge，使右键菜单能够知道当前 field 的文本域能力，同时不让 root overlay 持有 selection 真相。
- Files expected to inspect/modify
  - `ui/components/modal-chrome.slint`
  - `ui/components/assets-snippet-package-modal.slint`
  - `ui/app-window.slint`
  - 可能涉及新的共享 Slint 组件或 `ui/components` 下的辅助 bridge 文件
  - `src/shell/view_model.rs`
  - `src/shell/context_menu.rs` 或等价新增 text menu domain 文件
  - `src/app/bootstrap.rs`
- Tests to add/update
  - `DialogTextField` 能上报最小 metadata：`field_id`、`field_kind`、`is_read_only`、`is_secret`、`has_selection`、`supports_multiline`；
  - bare `TextInput` outlier 能接入同一 bridge；
  - root overlay 打开时不主动 `focus()` 菜单层。
- Implementation notes
  - 优先复用 Slint `TextInput.copy()/paste()/select-all()` 等 API；
  - bridge 只传 capability 与 field owner，不传选中文本本身；
  - 避免“全局透明 `TouchArea` 兜右键”的方案；
  - 如果发现 Slint 现状无法满足桥接需求，先停下做 `@superpowers:systematic-debugging`。
- Verification commands
  - `cargo test --test modal_input_select_contract_spec`
  - `cargo test --test assets_modal_render_spec`
  - `cargo test --test window_shell`
- Risks
  - 菜单壳体仍沿用旧 focus-stealing 模式；
  - 只覆盖 `DialogTextField`，漏掉 bare `TextInput`；
  - 让 root overlay 反向持有 selection/caret 真相，造成状态分裂。

## Task 3 - 接入普通 SSH 表单字段 Copy / Paste

- Goal
  - 让 SSH 连接 modal 中的普通字段具备右键 `Copy / Paste` 菜单与 capability 投影，同时不影响现有键盘快捷键与提交流程。
- Files expected to inspect/modify
  - `ui/components/assets-ssh-connection-modal.slint`
  - `src/shell/view_model/ssh_modal.rs`
  - `src/app/bootstrap.rs`
  - 如 bridge 需要，也可能涉及 `ui/app-window.slint` 与共享组件
- Tests to add/update
  - 普通 host/user/port/remark 等字段：有选区时 `Copy` 可用；
  - 无选区但 clipboard 有文本时 `Paste` 可用；
  - 右键后再键入，输入仍留在原 field；
  - 不影响已有 `submitted` / `focus-sequence` / trailing action 行为。
- Implementation notes
  - 普通字段先行，先不处理 secret 例外；
  - 保持菜单定位、dismiss、外部点击关闭、`Esc` 关闭行为一致；
  - 若 modal 内已有 popup / select overlay，注意 transient surface 冲突。
- Verification commands
  - `cargo test --test assets_modal_smoke`
  - `cargo test --test assets_modal_render_spec`
  - `cargo test --test modal_input_select_contract_spec`
- Risks
  - 右键菜单与 modal 现有 popup/dismiss layer 互相打架；
  - 菜单关闭后 field 失焦。

## Task 4 - 接入 keychain / credential / private key / passphrase 字段，并执行 secret policy

- Goal
  - 在 keychain、credential、sync vault、private key、passphrase 等字段上接入右键菜单，并正确执行 secret policy：`Paste yes / Copy no` 默认策略。
- Files expected to inspect/modify
  - `ui/components/assets-keychain-identity-modal.slint`
  - `ui/components/assets-keychain-ssh-key-modal.slint`
  - `ui/components/sync-vault-modal.slint`
  - `src/app/ssh/credentials.rs`
  - `src/shell/view_model/keychain.rs`
  - `src/shell/view_model/projection.rs`
  - `src/app/bootstrap.rs`
- Tests to add/update
  - secret 字段 `Paste` 可用；
  - secret 字段默认不暴露 `Copy`；
  - public key / fingerprint / public metadata 字段按 allowlist 继续可复制；
  - reveal 切换不自动让 secret 字段出现 `Copy`。
- Implementation notes
  - 不要把 `password-mode` 直接等价成 secret policy；
  - 以显式字段语义或 allowlist 驱动 `can_copy`；
  - 保留现有 `Paste Private Key` / `Copy Public Key` 等专用动作，不要互相冲突。
- Verification commands
  - `cargo test --test keychain_modal_smoke`
  - `cargo test --test sync_vault_modal_smoke`
  - `cargo test --test assets_modal_smoke`
- Risks
  - secret / public material 误分类；
  - reveal 状态与 copy policy 被错误绑定；
  - 新菜单与既有专用按钮发生重复或语义冲突。

## Task 5 - 接入 snippets / code block 多行编辑区

- Goal
  - 为 snippets / code block / script 等多行编辑器接入右键菜单，确保多行 `Paste` 保留原始换行、缩进、tab，不走 terminal paste 语义。
- Files expected to inspect/modify
  - `ui/components/assets-snippet-modal.slint`
  - `ui/components/assets-snippet-package-modal.slint`
  - 如有 code block 相关共享组件，也需一并检查
  - `src/app/bootstrap.rs`
- Tests to add/update
  - 多行脚本 / code block 右键 `Paste` 后换行和缩进不变；
  - tab 不被吞掉或重写；
  - 不触发 terminal paste warning / bracketed paste；
  - bare `TextInput` 包装场景与共享 `DialogTextField` 场景都要覆盖。
- Implementation notes
  - 这类字段仍属于 `TextEditContextMenuDomain`；
  - 使用普通文本插入，不走 terminal `normalize_workspace_paste_text()`、`workspace_paste_prompt_mode()`、`encode_paste()`；
  - 若后续发现 code block 使用了不同输入 primitive，需要补 adapter，而不是把它硬接到 terminal。
- Verification commands
  - `cargo test --test assets_modal_smoke`
  - `cargo test --test assets_modal_render_spec`
  - `cargo test --test modal_input_select_contract_spec`
- Risks
  - 多行字段误接到 terminal pipeline；
  - outlier 编辑区漏接入；
  - 粘贴后缩进/空行被改写。

## Task 6 - 如需要，接入 terminal 右键菜单，但必须保留 terminal pipeline 和 mouse mode 语义

- Goal
  - 仅在正式实现过程中发现有必要统一终端菜单视觉或状态投影时，整理 terminal 右键菜单；但不得改变 terminal 语义边界。
- Files expected to inspect/modify
  - `ui/shell/terminal-session-host.slint`
  - `ui/shell/workspace-pane.slint`
  - `ui/app-window.slint`
  - `src/app/bootstrap.rs`
  - `src/app/bootstrap/workspace_terminal.rs`
  - `src/app/ssh/session_manager.rs`
  - `src/app/ssh/runtime.rs`
  - `src/app/ssh/runtime/pump.rs`
  - `src/app/terminal_core/wezterm_adapter.rs`
- Tests to add/update
  - terminal `Paste` 仍走 `workspace_session_paste_requested -> forward_active_workspace_paste -> warning/editor -> send_session_paste -> encode_paste` 或等价链路；
  - `mouse_grabbed && !Shift` 下右键仍优先远端；
  - `Shift` 本地逃生阀不回退；
  - plain `Ctrl+C` 不被本地 copy 劫持。
- Implementation notes
  - 这是可选任务，不是普通输入框方案的阻塞前置；
  - 若无需改 terminal，就只做非回归验证；
  - 不能因为“视觉统一”就让 terminal 共享普通输入框的 selection/caret truth。
- Verification commands
  - `cargo test --test workspace_paste_warning_modal_spec`
  - `cargo test --test ssh_terminal_interaction_spec`
  - `cargo test --test terminal_session_spec`
  - `cargo test --test bootstrap_smoke`
- Risks
  - 误把 terminal 视为普通文本控件；
  - 破坏 `mouse_grabbed + Shift`、paste warning、bracketed paste、selection rehydrate 等现有语义。

## Task 7 - 菜单 placement / dismiss / outside click / Esc / window blur / no-focus-steal 回归

- Goal
  - 锁定所有右键菜单生命周期行为，尤其是“不抢焦点、不丢选区、可关闭、关闭后仍可继续输入”。
- Files expected to inspect/modify
  - `ui/app-window.slint`
  - 相关 root overlay 组件
  - `ui/components/modal-chrome.slint`
  - `ui/components/blocking-modal-shell.slint`
  - 相关 text menu bridge / dispatcher 文件
  - `tests/window_shell.rs`
  - `tests/assets_modal_smoke.rs`
  - `tests/bootstrap_smoke.rs`
- Tests to add/update
  - placement 不越界；
  - outside click 关闭；
  - `Esc` 关闭；
  - window blur 关闭；
  - 菜单关闭后 field 仍可继续输入；
  - 打开 text menu 不影响 assets / SFTP 既有 dismiss 逻辑。
- Implementation notes
  - root overlay 负责 placement/dismiss，但不负责 selection 真相；
  - 如果当前 root overlay 机制必须 `focus()` 才能接收 `Esc`，要重新设计关闭通道，而不是牺牲输入框焦点；
  - 注意与 modal shell、select popup、workspace tab menu 共存。
- Verification commands
  - `cargo test --test window_shell`
  - `cargo test --test assets_modal_smoke`
  - `cargo test --test bootstrap_smoke`
- Risks
  - 为了拿到 `Esc` 而重新引入 focus steal；
  - 菜单层与其他 transient surface 冲突；
  - placement 修好但 dismiss / blur 回归。

## Task 8 - 补充 smoke tests、render specs、cargo tests，并验证 assets / SFTP context menu 不回归

- Goal
  - 用完整验证收尾，证明普通输入框增强没有破坏 assets / SFTP / terminal 现有菜单与 paste 流程。
- Files expected to inspect/modify
  - `tests/assets_context_menu_spec.rs`
  - `tests/assets_context_menu_smoke.rs`
  - `tests/sftp_context_menu_spec.rs`
  - `tests/window_shell.rs`
  - `tests/workspace_tabs_spec.rs`
  - `tests/bootstrap_smoke.rs`
  - 以及本轮新增/更新的 modal 与 keychain/sync/snippet 测试
- Tests to add/update
  - assets / SFTP 共享 overlay 仍正确工作；
  - workspace terminal 现有 right-click copy/paste smoke 不回退；
  - modal / snippet / keychain / sync 新增文本右键 smoke 全部通过；
  - render spec / contract spec 继续锁住结构约束。
- Implementation notes
  - 这一任务不是“跑一遍所有测试”这么简单，而是要确认新增 text domain 没污染现有 assets/SFTP domain；
  - 若任何既有 smoke 失败，不要直接打补丁绕过去，先判定是实现 bug 还是计划假设失真。
- Verification commands
  - `cargo test --test assets_context_menu_spec`
  - `cargo test --test assets_context_menu_smoke`
  - `cargo test --test sftp_context_menu_spec`
  - `cargo test --test workspace_tabs_spec`
  - `cargo test --test window_shell`
  - `cargo test --test bootstrap_smoke`
  - `cargo test`
- Risks
  - 只验证新增菜单，不验证既有 assets/SFTP/terminal 回归；
  - 用局部绿替代全局交付判断；
  - 忽略“测试通过但体验仍坏”的最后一公里检查。
