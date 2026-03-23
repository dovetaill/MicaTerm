# SSH / Shell / Modal / Runtime / Tabs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把已确认的 `Workspace / modal / SSH runtime / tabs / memory` 设计落成可工作的首轮版本，消除白色竖条与 tab 撑开异常，建立阻断式可拖动 modal，接通真实 SSH shell 会话、可关闭 tab、可编辑 SSH 资产，并先收敛当前明显偏高的自有内存浪费。

**Architecture:** 继续保持 `app runtime + domain services -> bootstrap bridge -> ShellViewModel -> Slint` 的单向真相源。`WorkspacePane` 负责稳定的 tab/content/terminal surface 布局边界；modal 统一走阻断式 contract 与 modal-local drag；SSH 会话由 `SessionManager -> SshSessionRuntime -> terminal adapter -> renderer host` 分层管理，`ShellViewModel` 只持有 UI 投影与待执行动作，不持有 transport 逻辑。内存优化优先从“单一 Tokio runtime + 收敛 worker thread 策略”开始，不在本轮切换主线 renderer。

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`, `keyring`, `redb`, cargo test, shell smoke scripts

---

## 执行前提

- 必须在独立 worktree 中执行。
- 严格按 TDD 顺序推进：先写失败测试，再做最小实现，再跑回归。
- 本轮不扩展 SFTP UI、proxy/tunnel 实连、多 pane、自动重连策略、完整 renderer strategy 切换。
- 保持当前 flat / no-radius 方向，不重新引入圆角。
- `TerminalSessionHost` 当前仍是 placeholder；实现阶段不得把占位 UI 误当成真实 renderer。
- `SshSessionRuntime::connect()` 当前仍是 stub；实现阶段不得保留“假 Connected”路径。

## 任务顺序

1. 先收敛 async/runtime 基线，去掉重复 Tokio runtime。
2. 再固定 `WorkspacePane` 布局边界，避免后续 terminal surface 接入时继续放大白条问题。
3. 然后统一 modal contract 与 tab close hit-testing，先把 UI 壳层交互稳定下来。
4. 再推进 SSH modal 动作状态机、编辑模式、持久化最小扩展。
5. 最后接通真实 SSH runtime、terminal adapter、tab/session 生命周期与总回归。

### Task 1: 合并重复 Tokio runtime 并锁定后台执行基线

**Files:**
- Modify: `src/main.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/async_runtime.rs`
- Modify: `tests/async_runtime_spec.rs`
- Modify: `tests/bootstrap_profile_smoke.rs`

**Step 1: Write the failing tests**

在 `tests/async_runtime_spec.rs` 增加：

```rust
#[test]
fn app_async_runtime_uses_bounded_worker_threads_for_mainline_profile() {}

#[test]
fn session_bridge_reuses_supplied_runtime_handle_instead_of_creating_another_runtime() {}
```

在 `tests/bootstrap_profile_smoke.rs` 增加：

```rust
#[test]
fn run_with_profile_accepts_external_async_handle_for_ssh_services() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test async_runtime_spec --test bootstrap_profile_smoke
```

Expected:

- FAIL because `build_session_bridge()` still calls `AppAsyncRuntime::new()` internally.
- FAIL because runtime thread strategy is not explicitly bounded.

**Step 3: Write minimal implementation**

在 `src/app/bootstrap.rs` 把：

```rust
fn build_session_bridge() -> Option<Rc<ShellSessionBridge>>
```

改成接收外部 handle：

```rust
fn build_session_bridge(runtime_handle: tokio::runtime::Handle) -> Rc<ShellSessionBridge>
```

并删除 bridge 内部对第二套 `AppAsyncRuntime` 的创建，让 `run_with_profile()` 传入的 handle 成为唯一后台执行入口。

在 `src/app/async_runtime.rs` 明确 worker thread 策略，例如：

```rust
let worker_threads = std::thread::available_parallelism()
    .map(|v| v.get().min(2))
    .unwrap_or(2);
```

实现要求：

- 先以低风险的固定上限收敛线程数，不做 profile-aware 复杂配置系统。
- 保留 `enable_all()`，避免破坏后续 `russh` / timer / channel 行为。
- `ShellSessionBridge` 不再拥有一整套 runtime 实例，只保留 `SessionManager`。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test async_runtime_spec --test bootstrap_profile_smoke
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/main.rs src/app/bootstrap.rs src/app/async_runtime.rs tests/async_runtime_spec.rs tests/bootstrap_profile_smoke.rs
git commit -m "refactor: reuse single async runtime for shell services"
```

### Task 2: 抽出 `WorkspacePane` 并锁定全宽布局契约

**Files:**
- Create: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/shell_layout_ui_contract_smoke.sh`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/shell_layout_policy.rs`

**Step 1: Write the failing tests**

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn workspace_tab_projection_does_not_encode_container_width_behavior() {}
```

在 `tests/shell_layout_policy.rs` 增加：

```rust
#[test]
fn workspace_pane_requires_fill_width_contract_for_tab_strip_and_content_host() {}
```

在 `tests/shell_layout_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'WorkspacePane' "$APP_WINDOW" >/dev/null
grep -F 'horizontal-stretch: 1;' "$WORKSPACE_PANE" >/dev/null
grep -F 'min-width: 0px;' "$WORKSPACE_PANE" >/dev/null
grep -F 'width: 100%;' "$WORKSPACE_PANE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec --test shell_layout_policy
bash tests/shell_layout_ui_contract_smoke.sh
```

Expected:

- FAIL because `WorkspacePane` 组件还不存在。
- FAIL because `TabBar` / `content-host` 还没有稳定 fill-width 契约。

**Step 3: Write minimal implementation**

创建 `ui/shell/workspace-pane.slint`，把 tab strip、workspace content、future terminal surface 容器集中到同一组件。

实现要求：

- `AppWindow` 只负责组合 `Sidebar + WorkspacePane + RightPanel`。
- `WorkspacePane` 必须显式声明横向填充，避免退化为 `TabBar` intrinsic width。
- `TabBar` 与 `TerminalSessionHost` 外层容器都要显式 `min-width: 0px`，避免内容撑裂。
- 本任务只修 layout/boundary，不接真实 terminal renderer。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test workspace_tabs_spec --test shell_layout_policy
bash tests/shell_layout_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/workspace-pane.slint ui/app-window.slint ui/shell/tabbar.slint ui/shell/terminal-session-host.slint tests/shell_layout_ui_contract_smoke.sh tests/workspace_tabs_spec.rs tests/shell_layout_policy.rs
git commit -m "refactor: extract workspace pane layout contract"
```

### Task 3: 建立统一阻断式 modal shell 与 modal-local drag contract

**Files:**
- Create: `ui/components/blocking-modal-shell.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-rename-modal.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/windowing.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn asset_modal_backdrop_click_does_not_dismiss_blocking_modal() {}

#[test]
fn esc_closes_standard_asset_modals_but_host_key_prompt_remains_explicit_reject_path() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn blocking_modal_shell_exposes_drag_callbacks_and_focus_restore_hooks() {}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'callback drag-requested(' "$MODAL_SHELL" >/dev/null
grep -F 'clicked => { }' "$APP_WINDOW" >/dev/null
grep -F 'consume-event' "$MODAL_SHELL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because root dismiss layer still closes modal on outside click.
- FAIL because there is no shared blocking modal shell or drag callback contract.

**Step 3: Write minimal implementation**

创建共享 `blocking-modal-shell.slint`，负责：

- backdrop 拦截点击但不自动关闭；
- modal title drag hotzone；
- 焦点进入与焦点恢复 sequence；
- 统一 header / body / footer slot。

在 `src/app/bootstrap.rs` / `src/app/windowing.rs` 接入 modal-local drag 事件，把拖动解释为“移动 modal 偏移量”，不是“拖动宿主窗口”。

实现要求：

- `New SSH Connection`、`New Folder`、`Rename`、`Delete Confirm`、`SSH host key confirm` 全部迁移到共享 contract。
- `Esc` 保留，但语义分流：
  - 普通 create/rename/edit/delete: close
  - host key prompt: reject
- 不做跨会话记忆，只在当前打开周期内维护偏移量。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/blocking-modal-shell.slint ui/app-window.slint ui/components/assets-ssh-connection-modal.slint ui/components/assets-folder-create-modal.slint ui/components/assets-rename-modal.slint src/shell/view_model.rs src/app/bootstrap.rs src/app/windowing.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs
git commit -m "feat: unify blocking modal shell contract"
```

### Task 4: 修复 `ActiveTab` hit-testing，并建立稳定的 close/focus fallback 路径

**Files:**
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn closing_active_tab_falls_back_to_right_then_left_then_welcome() {}

#[test]
fn close_affordance_is_modeled_separately_from_select_action() {}
```

在 `tests/ssh_session_manager_spec.rs` 增加：

```rust
#[test]
fn closing_tab_removes_session_from_registry() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加：

```bash
grep -F 'callback close-requested' "$ACTIVE_TAB" >/dev/null
grep -F 'clicked => { root.close-requested(); }' "$ACTIVE_TAB" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_session_manager_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL because full-surface `TouchArea` still swallows close intent.
- FAIL because close fallback policy is not fully enforced in state logic.

**Step 3: Write minimal implementation**

在 `ui/components/active-tab.slint` 让 close hotspot 拥有独立命中区域和更高优先级，不再被 tab 根 `TouchArea` 覆盖。

在 `src/shell/view_model.rs` 明确实现：

```rust
fn close_workspace_session_with_fallback(&mut self, session_id: &str) -> bool
```

策略固定为：

- 优先激活右侧 tab；
- 否则激活左侧；
- 否则回到 welcome surface。

在 `src/app/bootstrap.rs` 保持 “close tab = close UI tab + close session registry”，不得只删 UI 投影。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_session_manager_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/active-tab.slint ui/shell/tabbar.slint src/shell/tabs.rs src/shell/view_model.rs src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/ssh_session_manager_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "fix: wire tab close hit testing and fallback lifecycle"
```

### Task 5: 把 SSH modal 升级为真实动作状态机与明确反馈语义

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/theme/tokens.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn ssh_modal_exposes_save_connect_test_and_save_connect_actions() {}

#[test]
fn invalid_draft_disables_connect_family_actions() {}

#[test]
fn beginning_modal_action_marks_state_busy_until_result_is_applied() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn ssh_modal_contract_round_trips_button_state_and_inline_feedback() {}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'save-and-connect' "$SSH_MODAL" >/dev/null
grep -F 'busy' "$SSH_MODAL" >/dev/null
grep -F 'hover' "$TOKENS" >/dev/null
grep -F 'pressed' "$TOKENS" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because `SshModalAction` 还缺 `Save`。
- FAIL because modal 还没有完整 `busy/success/error` 状态模型。
- FAIL because按钮视觉状态和业务状态仍未统一。

**Step 3: Write minimal implementation**

在 `src/shell/view_model.rs` 把 `SshModalAction` 扩展为：

```rust
pub enum SshModalAction {
    Save,
    Connect,
    TestConnection,
    SaveAndConnect,
}
```

增加独立 modal action state，例如：

```rust
pub enum SshModalActionState {
    Idle,
    Busy(SshModalAction),
    Success(String),
    Error(String),
}
```

实现要求：

- `Save` 只走资产保存路径。
- `Connect` 允许临时 draft，不要求先落资产。
- `TestConnection` 不创建 tab，不保存资产。
- `SaveAndConnect` 先持久化再打开 session。
- 主反馈必须位于 modal 内状态区，不用 toast 替代。
- hover / pressed / disabled / busy 颜色全部从 `ThemeTokens` 收敛，不在组件里硬编码。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/theme/tokens.slint src/shell/view_model.rs src/app/bootstrap.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs
git commit -m "feat: add ssh modal action state machine"
```

### Task 6: 建立 SSH 编辑模式、最小持久化扩展与 keyring secret 引用

**Files:**
- Modify: `src/app/assets_catalog/model.rs`
- Modify: `src/app/assets_catalog/mapper.rs`
- Modify: `src/app/assets_catalog/redb_store.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `src/app/ssh/credentials.rs`
- Modify: `src/shell/assets.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `tests/assets_catalog_domain.rs`
- Modify: `tests/assets_catalog_store.rs`
- Modify: `tests/credential_store_spec.rs`
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/assets_catalog_domain.rs` 增加：

```rust
#[test]
fn persisted_ssh_connection_spec_round_trips_extended_non_secret_fields() {}
```

在 `tests/credential_store_spec.rs` 增加：

```rust
#[test]
fn system_credential_store_can_replace_existing_secret_for_same_reference() {}
```

在 `tests/ssh_profile_spec.rs` 增加：

```rust
#[test]
fn ssh_profile_can_be_built_from_saved_asset_and_credential_reference() {}
```

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn edit_connection_opens_modal_with_prefilled_non_secret_fields() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test credential_store_spec --test ssh_profile_spec --test shell_view_model
```

Expected:

- FAIL because persisted SSH schema still only has `host/user/port/environment/proxy_method`。
- FAIL because edit flow has no prefill path and no persisted secret reference contract.

**Step 3: Write minimal implementation**

把 `PersistedSshConnectionSpec` 最小扩展为：

```rust
pub struct PersistedSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    pub private_key_source: String,
    pub private_key_path: String,
    pub environment: String,
    pub proxy_method: String,
    pub remark: String,
    pub credential_ref: Option<String>,
}
```

实现要求：

- 密码、私钥内容、passphrase 不写入资产目录。
- secret 通过 `CredentialStore` 写入 `keyring`，资产只保留 `credential_ref`。
- `AssetsSshConnectionModal` 复用为 `create/edit` 双模式。
- edit 模式回填非敏感字段；密码输入框默认脱敏，尾部 eye icon 控制显隐。
- 只做最小 schema bump 和 mapper/store 适配，不扩展完整资产迁移系统。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_catalog_domain --test assets_catalog_store --test credential_store_spec --test ssh_profile_spec --test shell_view_model
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/assets_catalog/model.rs src/app/assets_catalog/mapper.rs src/app/assets_catalog/redb_store.rs src/app/ssh/profile.rs src/app/ssh/credentials.rs src/shell/assets.rs src/shell/view_model.rs src/app/bootstrap.rs ui/components/assets-ssh-connection-modal.slint tests/assets_catalog_domain.rs tests/assets_catalog_store.rs tests/credential_store_spec.rs tests/ssh_profile_spec.rs tests/shell_view_model.rs
git commit -m "feat: add ssh edit mode and secret references"
```

### Task 7: 把 `SshSessionRuntime` 从 stub 升级为真实 SSH transport + PTY shell

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `src/app/ssh/known_hosts.rs`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/known_hosts_spec.rs`

**Step 1: Write the failing tests**

在 `tests/terminal_session_spec.rs` 增加：

```rust
#[test]
fn terminal_session_applies_remote_bytes_and_tracks_seqno() {}

#[test]
fn terminal_session_encodes_keyboard_input_for_shell_writeback() {}
```

在 `tests/ssh_session_manager_spec.rs` 增加：

```rust
#[test]
fn session_manager_marks_connected_only_after_runtime_connected_event() {}

#[test]
fn runtime_error_marks_session_reconnectable() {}
```

在 `tests/known_hosts_spec.rs` 增加：

```rust
#[test]
fn unknown_host_requires_explicit_accept_before_connect_can_continue() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_session_manager_spec --test known_hosts_spec
```

Expected:

- FAIL because `SshSessionRuntime::connect()` still emits `Connected` immediately.
- FAIL because there is no real SSH handshake / channel / PTY / shell pump path.

**Step 3: Write minimal implementation**

在 `src/app/ssh/runtime.rs` 完成首轮真实链路：

- `russh` client connect
- host key check through `KnownHostsService`
- auth by password / key path / inline key
- `channel_open_session`
- `request_pty`
- `request_shell`
- remote output -> `SessionRuntimeEvent::Output`
- local input / resize write-back

实现要求：

- `Connected` 事件只能在握手、认证、channel、pty、shell 全部成功后发出。
- host key unknown/changed 不能静默跳过，必须回到 UI prompt。
- 失败路径必须保留错误文本，供 modal/tab 状态区展示。
- 本任务先接通协议和 runtime，不接 renderer host。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_session_manager_spec --test known_hosts_spec
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/ssh/profile.rs src/app/ssh/known_hosts.rs tests/terminal_session_spec.rs tests/ssh_session_manager_spec.rs tests/known_hosts_spec.rs
git commit -m "feat: implement real ssh session runtime"
```

### Task 8: 接通 terminal adapter、renderer host 和 runtime-to-UI 持续同步

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn connected_session_projects_terminal_surface_state_without_placeholder_copy() {}

#[test]
fn disconnected_and_error_tabs_remain_reconnectable() {}
```

在 `tests/terminal_session_spec.rs` 增加：

```rust
#[test]
fn terminal_runtime_snapshot_can_be_polled_without_exposing_transport_objects() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'terminal-surface-ready' "$TERMINAL_HOST" >/dev/null
grep -F 'connecting' "$TABBAR" >/dev/null
grep -F 'error' "$TABBAR" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec --test workspace_tabs_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL because `TerminalSessionHost` 还在渲染 placeholder 文案。
- FAIL because runtime output 还没有持续投影到 UI state。

**Step 3: Write minimal implementation**

建立 terminal adapter/snapshot 边界，例如：

```rust
pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub screen_text: String,
}
```

实现要求：

- renderer host 只消费 terminal snapshot，不接触 `russh` transport。
- `SessionManager` 保持 session registry 与 terminal projection 分离。
- `ShellViewModel` 只拿到当前 active session 的 surface state 和 tab state。
- `TerminalSessionHost` 去掉 “Renderer host is reserved...” placeholder 文案；连接中、已连接、断开、错误分态都要能显示。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_session_spec --test workspace_tabs_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs src/shell/tabs.rs src/shell/view_model.rs ui/shell/terminal-session-host.slint ui/shell/tabbar.slint tests/terminal_session_spec.rs tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: project terminal session state into workspace host"
```

### Task 9: 打通 `Test / Connect / Save / Save and Connect` 与 tab/session 真实生命周期

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/tabs.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/bootstrap_smoke.rs` 增加：

```rust
#[test]
fn save_action_persists_asset_without_opening_session() {}

#[test]
fn connect_action_opens_temporary_session_without_persisting_asset() {}

#[test]
fn save_and_connect_persists_then_opens_session() {}
```

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn reopening_same_asset_activates_existing_session_by_default() {}

#[test]
fn explicit_open_in_new_tab_creates_second_session() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test ssh_session_manager_spec --test workspace_tabs_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL because `Connect` / `SaveAndConnect` 还没有真实分流。
- FAIL because同资产默认复用与 `ForceNewTab` 还没有完整落到 UI 行为。

**Step 3: Write minimal implementation**

在 `src/app/bootstrap.rs` 收敛动作语义：

- `TestConnection`: 调 runtime probe，只回写 modal 状态；
- `Connect`: 从 draft 构造临时 profile，`asset_id` 使用 `session:<uuid>` 风格临时标识；
- `Save`: 仅更新 catalog + secret store；
- `SaveAndConnect`: 持久化完成后再走 `OpenSessionMode::ActivateExisting`。

在 `src/app/ssh/session_manager.rs` 保持：

- 默认同资产复用已有活跃 session；
- 明确 `ForceNewTab` 才创建第二个 session；
- close tab = close session；
- `Disconnected` / `Error` 保留 tab 并允许 reconnect。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test ssh_session_manager_spec --test workspace_tabs_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/ssh/session_manager.rs src/shell/view_model.rs src/shell/tabs.rs tests/bootstrap_smoke.rs tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: wire ssh modal actions to real session lifecycle"
```

### Task 10: 做总回归与内存基线验证

**Files:**
- Modify: `docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-design.md`
- Create: `docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-verification.md`

**Step 1: Run focused test suites**

Run:

```bash
cargo test --test async_runtime_spec --test bootstrap_profile_smoke --test assets_modal_smoke --test shell_view_model --test assets_catalog_domain --test assets_catalog_store --test credential_store_spec --test ssh_profile_spec --test ssh_session_manager_spec --test terminal_session_spec --test workspace_tabs_spec --test bootstrap_smoke --test known_hosts_spec
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/shell_layout_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 2: Run full regression**

Run:

```bash
cargo test
```

Expected: PASS

**Step 3: Capture manual verification**

把以下结果写入 `docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-verification.md`：

- 启动后无白色竖条；
- tab 数量增加时 workspace 不再被撑裂；
- `New SSH / New Folder / Rename / Edit` modal 可拖动、不可 click-away dismiss；
- `Test / Connect / Save / Save and Connect` 各自语义符合设计；
- `Connect` 打开的不是假连接 tab；
- close tab 真正关闭 session；
- 编辑 SSH 资产时字段正确回填，secret 仍由 keyring 管理；
- Windows 任务管理器或 Process Explorer 对比前后常驻内存，确认重复 runtime 移除后有可观察下降。

**Step 4: Commit**

```bash
git add docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-design.md docs/plans/2026-03-23-ssh-shell-modal-runtime-tabs-verification.md
git commit -m "docs: record ssh shell modal runtime verification"
```

## 风险提示

- `russh` 首轮接入如果没有稳定的测试 SSH server harness，容易把真实网络问题和状态机 bug 混在一起；必要时优先补 fake transport seam，而不是硬写大块集成代码。
- modal-local drag 在 Slint/winit 自绘壳层里要避免与窗口标题栏拖拽语义冲突，命中区域必须明确。
- `credential_ref` 一旦入 catalog，后续 schema 不能随意改 key 命名规则，否则会影响老资产读取。
- terminal snapshot 如果直接传整屏文本，会先保证正确性但可能牺牲性能；首轮可接受，后续再做 delta 优化。
- 内存优化本轮先做“确定收益项”，不要把 renderer strategy 切换和 SSH 主线实现绑在一起。
