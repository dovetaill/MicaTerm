# Bootstrap ViewModel Runtime Refactor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在不改变现有业务行为、UI 契约与外部模块路径的前提下，把 `src/app/bootstrap.rs`、`src/shell/view_model.rs`、`src/app/ssh/runtime.rs` 重构为稳定 facade + 可维护内部模块结构。

**Architecture:** 保留 `crate::app::bootstrap`、`crate::shell::view_model`、`crate::app::ssh::runtime` 作为稳定入口。先冻结 facade / re-export / structural contract，再按已确认边界分阶段搬运 `bootstrap` 功能域、`ShellViewModel` 分域 `impl` 与重流量 helper、以及 SSH runtime 的 contracts / terminal / transport / auth / pump / sftp backend，最后统一清理 import、重复 helper 与注释。

**Tech Stack:** Rust, Slint, Tokio, `russh`, `russh-sftp`, `wezterm-term`, `termwiz`, shell smoke scripts, `cargo test`, `cargo check`, `cargo clippy`

---

## Input Design

- 设计基线固定为 `docs/plans/2026-04-03-bootstrap-view-model-runtime-refactor-design.md`
- 实施不得偏离以下已确认决策：
  - 全局路径策略选 A：保留外部入口文件路径稳定，对内拆分子模块
  - `bootstrap` 拆分策略选 A：按功能域拆分
  - `view_model` 拆分策略选 B：分域 `impl` + 选择性 helper / dispatcher 抽离
  - `runtime` 拆分策略选 A：保留 `crate::app::ssh::runtime` facade，对内按职责拆分
- 本计划只允许结构重构，不允许加入新功能、交互改版、状态语义变更或 public API churn

## Execution Notes

- 每个 task 都先使用 `@superpowers:test-driven-development`：先写失败测试/结构 smoke，再做最小搬运，再跑通过。
- 若搬运过程中出现生命周期、borrow、异步顺序或 re-export 回归，不允许猜测，立即切换 `@superpowers:systematic-debugging`。
- 优先用小步移动与阶段性提交，不要一次性大迁移三个文件。
- 能保持原函数签名、原调用顺序、原闭包捕获集合时，优先保持；必要时只做最小适配层。
- 只在复杂 ownership、隐式保活、事件转发顺序不明显处补简短注释，不用注释掩盖糟糕边界。
- 如果在独立 worktree 执行更安全；若在当前工作区执行，必须限制改动范围到本计划列出的文件。

## Task Sequence Overview

1. 冻结 facade / re-export / 模块骨架，建立结构回归护栏。
2. 拆 `bootstrap` 的 `vault_sync` 域。
3. 拆 `bootstrap` 的 `workspace_terminal` 域。
4. 拆 `bootstrap` 的 `sftp` 域。
5. 拆 `bootstrap` 的 `assets_keychain` 与 `shell_chrome/windowing` 域，收窄 root binder。
6. 把 `view_model.rs` 转成分域 `impl` 布局，先搬低风险状态域。
7. 为 `view_model` 提取 `asset modal executor`、`ssh modal`、`context menu dispatcher`。
8. 把 `ssh::runtime` 改成 facade + `contracts` / `terminal` 模块，先稳定 public contracts。
9. 继续拆 `ssh::runtime` 的 `transport` / `auth` / `pump` / `sftp_backend`。
10. 统一清理 import / re-export / 备注与全量回归验证。

### Task 1: Freeze facade contracts and create module skeletons

**Files:**
- Create: `tests/bootstrap_module_contract_smoke.sh`
- Create: `tests/view_model_module_contract_smoke.sh`
- Create: `tests/ssh_runtime_module_contract_smoke.sh`
- Create: `src/app/bootstrap/vault_sync.rs`
- Create: `src/app/bootstrap/workspace_terminal.rs`
- Create: `src/app/bootstrap/sftp.rs`
- Create: `src/app/bootstrap/assets_keychain.rs`
- Create: `src/app/bootstrap/shell_chrome.rs`
- Create: `src/app/bootstrap/windowing.rs`
- Create: `src/shell/view_model/projection.rs`
- Create: `src/shell/view_model/workspace.rs`
- Create: `src/shell/view_model/quick_launch.rs`
- Create: `src/shell/view_model/assets.rs`
- Create: `src/shell/view_model/keychain.rs`
- Create: `src/shell/view_model/sftp.rs`
- Create: `src/shell/view_model/validation.rs`
- Create: `src/app/ssh/runtime/contracts.rs`
- Create: `src/app/ssh/runtime/transport.rs`
- Create: `src/app/ssh/runtime/auth.rs`
- Create: `src/app/ssh/runtime/pump.rs`
- Create: `src/app/ssh/runtime/terminal.rs`
- Create: `src/app/ssh/runtime/sftp_backend.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ssh/runtime.rs`

**Step 1: Write the failing tests**
- 新增 3 个 shell smoke，分别锁定：
  - `bootstrap.rs` 仍是入口文件，但已声明功能域子模块
  - `view_model.rs` 仍保留 `ShellViewModel` 入口，但开始把 `impl` 分发到子文件
  - `ssh/runtime.rs` 仍保留 facade，同时开始 `pub use` / `mod` 化内部职责块
- 断言 root 文件仍保留稳定入口，不直接切换成 `mod.rs` 路径。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/bootstrap_module_contract_smoke.sh
bash tests/view_model_module_contract_smoke.sh
bash tests/ssh_runtime_module_contract_smoke.sh
```
Expected: FAIL，因为子模块骨架和 contract smoke 还不存在。

**Step 3: Write minimal implementation**
- 先创建空的内部模块文件和最小 `mod` 声明。
- `bootstrap.rs` / `view_model.rs` / `runtime.rs` 先只承担入口、`pub use`、共享类型和过渡性委派。
- 不移动业务逻辑，只建立后续可安全搬运的骨架。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add tests/bootstrap_module_contract_smoke.sh tests/view_model_module_contract_smoke.sh tests/ssh_runtime_module_contract_smoke.sh src/app/bootstrap.rs src/app/bootstrap/vault_sync.rs src/app/bootstrap/workspace_terminal.rs src/app/bootstrap/sftp.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/shell_chrome.rs src/app/bootstrap/windowing.rs src/shell/view_model.rs src/shell/view_model/projection.rs src/shell/view_model/workspace.rs src/shell/view_model/quick_launch.rs src/shell/view_model/assets.rs src/shell/view_model/keychain.rs src/shell/view_model/sftp.rs src/shell/view_model/validation.rs src/app/ssh/runtime.rs src/app/ssh/runtime/contracts.rs src/app/ssh/runtime/transport.rs src/app/ssh/runtime/auth.rs src/app/ssh/runtime/pump.rs src/app/ssh/runtime/terminal.rs src/app/ssh/runtime/sftp_backend.rs

git commit -m "refactor: add module skeletons for bootstrap view model and ssh runtime"
```

### Task 2: Move the `bootstrap` vault sync domain behind a dedicated module

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/vault_sync.rs`
- Create: `tests/bootstrap_vault_sync_split_smoke.sh`
- Test: `tests/vault_sync_service_spec.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言 `vault sync` 相关 helper/binder 已迁移到 `src/app/bootstrap/vault_sync.rs`。
- 保留现有行为测试，确认 sync modal、vault service 和 bootstrap 接线仍按原行为工作。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/bootstrap_vault_sync_split_smoke.sh
cargo test --test vault_sync_service_spec --test vault_settings_smoke --test bootstrap_smoke -q
```
Expected: FAIL，因为 `vault sync` 逻辑仍在 root 文件或 smoke 断言尚未满足。

**Step 3: Write minimal implementation**
- 把 `sync_local_vault`、`update_sync_modal_for_local_state` 及其直接相关 helper、binder 和状态投影搬到 `vault_sync` 模块。
- root 文件只保留入口装配、共享依赖注入与对 `vault_sync` 的薄委派。
- 保持所有回调顺序、错误分支与日志语义不变。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/vault_sync.rs tests/bootstrap_vault_sync_split_smoke.sh tests/vault_sync_service_spec.rs tests/vault_settings_smoke.rs tests/bootstrap_smoke.rs

git commit -m "refactor: move bootstrap vault sync domain into module"
```

### Task 3: Move the `bootstrap` workspace terminal domain out of the mega binder

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Create: `tests/bootstrap_workspace_terminal_split_smoke.sh`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言 workspace terminal 相关投影与前向函数不再留在 root binder。
- 强化现有测试，覆盖 active workspace 切换、terminal tab projection、status bar 同步与终端输入基本通路。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/bootstrap_workspace_terminal_split_smoke.sh
cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec --test top_status_bar_smoke -q
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```
Expected: FAIL，因为 `sync_workspace_projection_from_manager` / `forward_active_workspace_*` 仍在 root 文件。

**Step 3: Write minimal implementation**
- 把 `sync_workspace_projection_from_manager`、`forward_active_workspace_*`、workspace tab / terminal projection 相关 binder 全部搬入 `workspace_terminal` 模块。
- root 文件只保留调用编排和共享依赖组装。
- 保持 `window.on_*` 注册顺序与 session bridge 行为不变。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/workspace_terminal.rs tests/bootstrap_workspace_terminal_split_smoke.sh tests/workspace_tabs_spec.rs tests/ssh_terminal_interaction_spec.rs tests/top_status_bar_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh

git commit -m "refactor: split bootstrap workspace terminal domain"
```

### Task 4: Move the `bootstrap` SFTP domain into its own binder module

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Create: `tests/bootstrap_sftp_split_smoke.sh`
- Test: `tests/sftp_right_panel_render_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`
- Test: `tests/sftp_browser_controller_spec.rs`
- Test: `tests/sftp_context_menu_spec.rs`
- Test: `tests/sftp_transfer_flow_spec.rs`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言 SFTP panel / browser / queue / transfer binder 已迁移到 `src/app/bootstrap/sftp.rs`。
- 保留现有 SFTP 行为测试，覆盖右侧面板渲染、controller 接线、队列与传输流程。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/bootstrap_sftp_split_smoke.sh
cargo test --test sftp_right_panel_render_spec --test sftp_runtime_spec --test sftp_browser_controller_spec --test sftp_context_menu_spec --test sftp_transfer_flow_spec -q
```
Expected: FAIL，因为 SFTP 绑定逻辑仍在 root 文件或结构断言尚未满足。

**Step 3: Write minimal implementation**
- 把 SFTP 相关 `window.on_*` 绑定、面板状态同步、controller 转发和 transfer queue 接线搬到 `sftp` 模块。
- 保持 root 文件只负责注入共享 store / runtime handle / view model。
- 不改 SFTP runtime 与 right panel 的产品行为。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/sftp.rs tests/bootstrap_sftp_split_smoke.sh tests/sftp_right_panel_render_spec.rs tests/sftp_runtime_spec.rs tests/sftp_browser_controller_spec.rs tests/sftp_context_menu_spec.rs tests/sftp_transfer_flow_spec.rs

git commit -m "refactor: isolate bootstrap sftp binder domain"
```

### Task 5: Move `assets_keychain` and `shell_chrome/windowing` binders out of `bootstrap.rs`

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Create: `tests/bootstrap_shell_chrome_split_smoke.sh`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/keychain_modal_smoke.rs`
- Test: `tests/keychain_ui_contract_smoke.sh`
- Test: `tests/window_chrome_contract_smoke.sh`
- Test: `tests/windows_frame_spec.rs`
- Test: `tests/window_resize_drag_contract_smoke.sh`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言资产/密钥链/标题栏/window hook 绑定已迁移到对应模块。
- 保留现有 modal、context menu、keychain、window chrome 和 window frame 行为测试。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/bootstrap_shell_chrome_split_smoke.sh
cargo test --test assets_modal_smoke --test assets_context_menu_smoke --test keychain_modal_smoke --test windows_frame_spec -q
bash tests/keychain_ui_contract_smoke.sh
bash tests/window_chrome_contract_smoke.sh
bash tests/window_resize_drag_contract_smoke.sh
```
Expected: FAIL，因为对应 binder 仍旧留在 root 文件或结构 smoke 尚未满足。

**Step 3: Write minimal implementation**
- 把 `sync_asset_modal_state`、资产/密钥链投影、titlebar/menu 回调和 native windowing hook 迁移到 `assets_keychain`、`shell_chrome`、`windowing`。
- 保持 `bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge` 仅做高层编排。
- 只有在闭包捕获或 Windows hook 生命周期不自解释时补少量备注。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/shell_chrome.rs src/app/bootstrap/windowing.rs tests/bootstrap_shell_chrome_split_smoke.sh tests/assets_modal_smoke.rs tests/assets_context_menu_smoke.rs tests/keychain_modal_smoke.rs tests/keychain_ui_contract_smoke.sh tests/window_chrome_contract_smoke.sh tests/windows_frame_spec.rs tests/window_resize_drag_contract_smoke.sh

git commit -m "refactor: split remaining bootstrap binder domains"
```

### Task 6: Convert `view_model.rs` into a split domain-impl layout

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/shell/view_model/quick_launch.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/keychain.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/shell/view_model/validation.rs`
- Test: `tests/shell_view_model.rs`
- Test: `tests/assets_explorer_projection.rs`
- Test: `tests/quick_launch_projection_spec.rs`
- Test: `tests/sftp_panel_state_spec.rs`
- Test: `tests/keychain_projection_spec.rs`
- Test: `tests/view_model_module_contract_smoke.sh`

**Step 1: Write the failing tests**
- 扩充 `view_model` 结构 smoke，断言 `ShellViewModel` 结构体与共享类型仍在 root，而低风险 `impl` 已分散到对应域文件。
- 保持现有投影/状态测试，锁定资产、quick launch、SFTP、keychain 与基础 validation 的既有行为。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/view_model_module_contract_smoke.sh
cargo test --test shell_view_model --test assets_explorer_projection --test quick_launch_projection_spec --test sftp_panel_state_spec --test keychain_projection_spec -q
```
Expected: FAIL，因为 `view_model.rs` 仍承载大部分低风险 `impl`。

**Step 3: Write minimal implementation**
- root 文件只保留 `ShellViewModel`、核心共享 type、构造入口和必须跨域共享的小工具。
- 把低风险 `impl ShellViewModel` 分别搬到 `projection.rs`、`workspace.rs`、`quick_launch.rs`、`assets.rs`、`keychain.rs`、`sftp.rs`、`validation.rs`。
- 不在这一任务提前引入新的 dispatcher，仅做清晰分域与最小委派。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/shell/view_model.rs src/shell/view_model/projection.rs src/shell/view_model/workspace.rs src/shell/view_model/quick_launch.rs src/shell/view_model/assets.rs src/shell/view_model/keychain.rs src/shell/view_model/sftp.rs src/shell/view_model/validation.rs tests/shell_view_model.rs tests/assets_explorer_projection.rs tests/quick_launch_projection_spec.rs tests/sftp_panel_state_spec.rs tests/keychain_projection_spec.rs tests/view_model_module_contract_smoke.sh

git commit -m "refactor: split shell view model domain impls"
```

### Task 7: Extract `ShellViewModel` heavy-flow helpers without changing state ownership

**Files:**
- Create: `src/shell/view_model/asset_modal_executor.rs`
- Create: `src/shell/view_model/ssh_modal.rs`
- Create: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/keychain.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Create: `tests/view_model_flow_dispatchers_smoke.sh`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_context_menu_spec.rs`
- Test: `tests/ssh_profile_spec.rs`
- Test: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言 `confirm_asset_modal`、SSH modal 关键流和 context menu leaf dispatch 已不再作为 root 巨型函数直接堆在 `view_model.rs`。
- 保留现有资产 modal、context menu 和 SSH profile 测试，锁定流程顺序与状态更新语义。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/view_model_flow_dispatchers_smoke.sh
cargo test --test assets_modal_smoke --test assets_context_menu_spec --test ssh_profile_spec --test shell_view_model -q
```
Expected: FAIL，因为重流量 helper / dispatcher 还未抽出。

**Step 3: Write minimal implementation**
- 提取 `asset modal executor`、`ssh modal`、`context menu dispatcher` 为内部 helper 文件。
- 保持 `ShellViewModel` 仍是唯一状态 owner；helper 只能通过 `&mut ShellViewModel` 或受控输入输出工作。
- 不新增第二套状态容器，不改变 modal 或 context menu 行为契约。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/keychain.rs src/shell/view_model/workspace.rs src/shell/view_model/asset_modal_executor.rs src/shell/view_model/ssh_modal.rs src/shell/view_model/context_menu_dispatcher.rs tests/view_model_flow_dispatchers_smoke.sh tests/assets_modal_smoke.rs tests/assets_context_menu_spec.rs tests/ssh_profile_spec.rs tests/shell_view_model.rs

git commit -m "refactor: extract shell view model heavy flow helpers"
```

### Task 8: Turn `crate::app::ssh::runtime` into a facade with stable contracts and terminal engine modules

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/runtime/contracts.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Test: `tests/ssh_runtime_module_contract_smoke.sh`
- Test: `tests/terminal_session_spec.rs`
- Test: `tests/terminal_model_spec.rs`
- Test: `tests/terminal_scrollback_spec.rs`
- Test: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing tests**
- 扩充 runtime 结构 smoke，断言 public terminal DTO、surface/input contracts 和 `TerminalSession` 已由 root facade 重导出，而不是继续全部定义在 `runtime.rs`。
- 保留终端行为测试，覆盖 surface state、scrollback、session manager 依赖的 contract。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/ssh_runtime_module_contract_smoke.sh
cargo test --test terminal_session_spec --test terminal_model_spec --test terminal_scrollback_spec --test ssh_session_manager_spec -q
```
Expected: FAIL，因为 public contracts 和 terminal engine 仍混在 `runtime.rs`。

**Step 3: Write minimal implementation**
- 把 `TerminalSurfaceState`、`TerminalCursorShape`、输入/鼠标 DTO 等公共 contract 迁移到 `contracts.rs`。
- 把 `TerminalSession` 及 projection / input / filter / color helpers 迁移到 `terminal.rs`。
- root `runtime.rs` 只保留 facade、共享 glue 和 `pub use`，保持调用路径完全兼容。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/ssh/runtime.rs src/app/ssh/runtime/contracts.rs src/app/ssh/runtime/terminal.rs tests/ssh_runtime_module_contract_smoke.sh tests/terminal_session_spec.rs tests/terminal_model_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_session_manager_spec.rs

git commit -m "refactor: split ssh runtime contracts and terminal engine"
```

### Task 9: Split `ssh::runtime` transport, auth, pump, and SFTP backend responsibilities

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/runtime/transport.rs`
- Modify: `src/app/ssh/runtime/auth.rs`
- Modify: `src/app/ssh/runtime/pump.rs`
- Modify: `src/app/ssh/runtime/sftp_backend.rs`
- Create: `tests/ssh_runtime_transport_split_smoke.sh`
- Test: `tests/ssh_connection_timeline_spec.rs`
- Test: `tests/known_hosts_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`
- Test: `tests/ssh_shell_integration_spec.rs`
- Test: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing tests**
- 新增结构 smoke，断言 transport / auth / pump / SFTP backend 已分域落在独立文件。
- 强化行为测试，覆盖 connection timeline、known hosts、shell integration、SFTP runtime 和 session manager。
- 额外锁定 `TransportChainGuard` 必须仍在真实连接生命周期内保活。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/ssh_runtime_transport_split_smoke.sh
cargo test --test ssh_connection_timeline_spec --test known_hosts_spec --test sftp_runtime_spec --test ssh_shell_integration_spec --test ssh_session_manager_spec -q
```
Expected: FAIL，因为连接管线、auth、pump 和 SFTP backend 仍耦合在 root runtime 文件。

**Step 3: Write minimal implementation**
- 把 `connect_target_handle_for_profile` 及 proxy / jump host 流程搬到 `transport.rs`。
- 把 auth / host key / progress 相关 helper 收敛到 `auth.rs`。
- 把 `run_channel_pump` 和 pump 辅助逻辑搬到 `pump.rs`。
- 把 `RusshSftpBackend` 及其相关 adapter 搬到 `sftp_backend.rs`。
- 保持 `TransportChainGuard` 保活语义、错误传播顺序与 public return type 不变。

**Step 4: Run tests to verify they pass**
Run: same commands as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
```bash
cargo check --workspace
```
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/ssh/runtime.rs src/app/ssh/runtime/transport.rs src/app/ssh/runtime/auth.rs src/app/ssh/runtime/pump.rs src/app/ssh/runtime/sftp_backend.rs tests/ssh_runtime_transport_split_smoke.sh tests/ssh_connection_timeline_spec.rs tests/known_hosts_spec.rs tests/sftp_runtime_spec.rs tests/ssh_shell_integration_spec.rs tests/ssh_session_manager_spec.rs

git commit -m "refactor: split ssh runtime transport auth pump and sftp backend"
```

### Task 10: Cleanup root files, remove duplication, add sparse comments, and run the final regression matrix

**Files:**
- Create: `tests/refactor_root_thinness_smoke.sh`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/bootstrap/vault_sync.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap/sftp.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Modify: `src/shell/view_model/quick_launch.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/keychain.rs`
- Modify: `src/shell/view_model/sftp.rs`
- Modify: `src/shell/view_model/validation.rs`
- Modify: `src/shell/view_model/asset_modal_executor.rs`
- Modify: `src/shell/view_model/ssh_modal.rs`
- Modify: `src/shell/view_model/context_menu_dispatcher.rs`
- Modify: `src/app/ssh/runtime/contracts.rs`
- Modify: `src/app/ssh/runtime/transport.rs`
- Modify: `src/app/ssh/runtime/auth.rs`
- Modify: `src/app/ssh/runtime/pump.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `src/app/ssh/runtime/sftp_backend.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/shell_view_model.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/terminal_session_spec.rs`
- Test: `tests/sftp_runtime_spec.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/keychain_modal_smoke.rs`
- Test: `tests/ssh_connection_timeline_spec.rs`
- Test: `tests/bootstrap_module_contract_smoke.sh`
- Test: `tests/view_model_module_contract_smoke.sh`
- Test: `tests/ssh_runtime_module_contract_smoke.sh`

**Step 1: Write the failing tests**
- 新增 root thinness smoke，断言三大 root 文件不再保留已迁移的重块函数名或大段职责实现。
- 把最终结构 smoke 与核心行为测试合并成回归矩阵。

**Step 2: Run tests to verify they fail**
Run:
```bash
bash tests/refactor_root_thinness_smoke.sh
cargo test --test bootstrap_smoke --test shell_view_model --test ssh_session_manager_spec --test terminal_session_spec --test sftp_runtime_spec --test top_status_bar_smoke --test assets_modal_smoke --test keychain_modal_smoke --test ssh_connection_timeline_spec -q
bash tests/bootstrap_module_contract_smoke.sh
bash tests/view_model_module_contract_smoke.sh
bash tests/ssh_runtime_module_contract_smoke.sh
```
Expected: FAIL，直到 root 文件真正收窄、重复 helper 被删除且最终 facade/re-export 稳定。

**Step 3: Write minimal implementation**
- 删除过渡期重复 helper、无效 re-export 和 root 中已不该保留的实现。
- 统一 import 顺序、模块可见性和简短备注位置。
- 确保 root 文件只承载入口、共享 type、`pub use`、薄委派与必要 glue。

**Step 4: Run tests to verify they pass**
Run:
```bash
bash tests/refactor_root_thinness_smoke.sh
cargo test --test bootstrap_smoke --test shell_view_model --test ssh_session_manager_spec --test terminal_session_spec --test sftp_runtime_spec --test top_status_bar_smoke --test assets_modal_smoke --test keychain_modal_smoke --test ssh_connection_timeline_spec -q
bash tests/bootstrap_module_contract_smoke.sh
bash tests/view_model_module_contract_smoke.sh
bash tests/ssh_runtime_module_contract_smoke.sh
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
Expected: PASS

**Step 5: Commit**
```bash
git add src/app/bootstrap.rs src/app/bootstrap/vault_sync.rs src/app/bootstrap/workspace_terminal.rs src/app/bootstrap/sftp.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/shell_chrome.rs src/app/bootstrap/windowing.rs src/shell/view_model.rs src/shell/view_model/projection.rs src/shell/view_model/workspace.rs src/shell/view_model/quick_launch.rs src/shell/view_model/assets.rs src/shell/view_model/keychain.rs src/shell/view_model/sftp.rs src/shell/view_model/validation.rs src/shell/view_model/asset_modal_executor.rs src/shell/view_model/ssh_modal.rs src/shell/view_model/context_menu_dispatcher.rs src/app/ssh/runtime.rs src/app/ssh/runtime/contracts.rs src/app/ssh/runtime/transport.rs src/app/ssh/runtime/auth.rs src/app/ssh/runtime/pump.rs src/app/ssh/runtime/terminal.rs src/app/ssh/runtime/sftp_backend.rs tests/refactor_root_thinness_smoke.sh tests/bootstrap_smoke.rs tests/shell_view_model.rs tests/ssh_session_manager_spec.rs tests/terminal_session_spec.rs tests/sftp_runtime_spec.rs tests/top_status_bar_smoke.rs tests/assets_modal_smoke.rs tests/keychain_modal_smoke.rs tests/ssh_connection_timeline_spec.rs tests/bootstrap_module_contract_smoke.sh tests/view_model_module_contract_smoke.sh tests/ssh_runtime_module_contract_smoke.sh

git commit -m "refactor: finish bootstrap view model and ssh runtime split"
```

## Final Verification Checklist

- `src/app/bootstrap.rs` 仍保留稳定入口路径，但已退化为 facade / composition root / thin binder glue。
- `src/shell/view_model.rs` 仍保留 `ShellViewModel` owner 和共享类型，没有演变成多 owner 架构。
- `src/app/ssh/runtime.rs` 仍保留 `crate::app::ssh::runtime` 调用面，public contracts 保持兼容。
- `TransportChainGuard` 保活语义、known-hosts / auth / progress 行为没有回归。
- `window.on_*` 注册覆盖面不变，只是按域重组。
- `confirm_asset_modal`、SSH modal、context menu leaf dispatch 已实质降复杂度，而不是仅把同样复杂度搬到别处。
- 注释数量有限，只覆盖复杂 ownership、异步编排、隐式保活语义。
- 最终 `cargo check --workspace` 与 `cargo clippy --workspace -- -D warnings` 通过。

## Suggested Execution Mode

- 若继续在当前会话执行，实现阶段优先用 `@superpowers:subagent-driven-development`，按 task 单独开工、逐 task 复核。
- 若新开执行会话，必须使用 `@superpowers:executing-plans`，严格按本文件顺序推进，不跳 task。
