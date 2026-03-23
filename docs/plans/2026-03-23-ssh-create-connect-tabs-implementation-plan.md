# SSH 新建 / 连接 / 标签页 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 把当前仅能创建 SSH 资产壳层的 `Windows Console` 工作区升级为可填写清晰表单、可真实建立 SSH shell 连接、可用 tab/session 管理会话的首轮版本。

**Architecture:** 继续保持 `Rust state -> bootstrap bridge -> Slint` 的单向真相源。SSH 资产配置、凭据与 host key 服务放在 `app` 层；`ShellViewModel` 只负责 UI state、modal state 与 tab/session 投影；真实连接运行在 app-level Tokio runtime 中，通过 `SessionManager` 和 `SessionHandle` 把 `russh + wezterm-term + termwiz` 的运行时状态同步回 UI。`TabBar` 与主工作区从 placeholder 升级为真实 session 容器，但首轮只做单窗口、多 tab、单 pane 的 SSH shell 场景。

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`, `keyring` or equivalent system credential integration, cargo test, shell smoke scripts

---

## 执行前提

- 必须在独立 worktree 中执行。
- 严格按 TDD 顺序推进，每个任务先补失败测试，再做最小实现，再跑回归。
- 当前仓库还没有 Tokio runtime 启动链路；不要假设 `bootstrap` 已经能运行后台异步服务。
- 首轮只做 SSH shell session，不实现 `russh-sftp` UI、proxy/tunnel 实连、自动重连、多 pane。
- 保留现有 modal 壳层，但只把 `Standard` 做完整；`Proxy / Tunnel / Environment / Advanced` 本轮不能伪装成已接通功能。

### Task 1: 扩展 SSH modal 字段与动作契约

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn ssh_modal_round_trips_standard_fields_and_auth_fields() {}

#[test]
fn ssh_modal_exposes_action_buttons_for_save_connect_test_and_save_connect() {}
```

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn ssh_modal_default_draft_starts_with_password_auth_and_port_22() {}

#[test]
fn ssh_modal_validation_requires_name_host_user_and_active_auth_payload() {}

#[test]
fn switching_auth_method_clears_irrelevant_validation_errors() {}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 增加 grep 断言，锁定：

```bash
grep -F 'in property <string> auth-method:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> private-key-source:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> password:' "$SSH_MODAL" >/dev/null
grep -F 'in property <string> remark:' "$SSH_MODAL" >/dev/null
grep -F 'callback action-requested(string);' "$SSH_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL because the SSH modal still only exposes `name/host/user/port/environment/proxy_method`
- FAIL because there is no auth method or action callback
- FAIL because view model has no validation rules for password/private key modes

**Step 3: Write minimal implementation**

在 `src/shell/view_model.rs` 扩展 draft：

```rust
pub struct AssetSshConnectionDraft {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,          // "password" | "private-key"
    pub private_key_source: String,   // "content" | "path"
    pub password: String,
    pub private_key_content: String,
    pub private_key_path: String,
    pub passphrase: String,
    pub remark: String,
    pub validation_message: String,
}
```

在 `ui/components/assets-ssh-connection-modal.slint`：

- 给 `Standard` 页每个字段增加显式 label
- 增加 auth switch 与 private key source switch
- 增加 `Password`、`Private Key Content`、`Private Key Path`、`Passphrase`、`Remark`
- 用一个统一 callback：

```slint
callback action-requested(string);
```

按钮 id 固定：

- `save`
- `connect`
- `test`
- `save-and-connect`

在 `ui/app-window.slint` / `src/app/bootstrap.rs` 接入：

```rust
window.on_asset_ssh_modal_action_requested(move |action| {
    state.begin_ssh_modal_action(action.as_str());
    sync_asset_modal_state(&window, &state);
});
```

实现要求：

- `Standard` 是首轮完整路径
- `Proxy / Tunnel / Environment / Advanced` 保留但要明确“not wired”
- validation 先只做字段级校验，不发起连接

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs
git commit -m "feat: expand ssh modal fields and action contract"
```

### Task 2: 建立 app-level Tokio runtime 与 SSH 配置领域模型

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/app/mod.rs`
- Create: `src/app/async_runtime.rs`
- Create: `src/app/ssh/mod.rs`
- Create: `src/app/ssh/profile.rs`
- Create: `tests/async_runtime_spec.rs`
- Create: `tests/ssh_profile_spec.rs`

**Step 1: Write the failing tests**

在 `tests/async_runtime_spec.rs` 增加：

```rust
#[test]
fn app_async_runtime_can_spawn_and_complete_background_tasks() {}

#[test]
fn app_async_runtime_exposes_handle_for_ssh_services() {}
```

在 `tests/ssh_profile_spec.rs` 增加：

```rust
#[test]
fn ssh_profile_normalizes_password_mode_from_modal_draft() {}

#[test]
fn ssh_profile_normalizes_private_key_path_mode_from_modal_draft() {}

#[test]
fn ssh_profile_normalizes_private_key_content_mode_from_modal_draft() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test async_runtime_spec --test ssh_profile_spec
```

Expected:

- FAIL because there is no app async runtime wrapper
- FAIL because there is no `ConnectionProfile` domain object

**Step 3: Write minimal implementation**

先加依赖：

```bash
cargo add wezterm-term termwiz russh keyring uuid
```

在 `src/app/async_runtime.rs` 建立 wrapper：

```rust
pub struct AppAsyncRuntime {
    runtime: Arc<tokio::runtime::Runtime>,
}

impl AppAsyncRuntime {
    pub fn new() -> anyhow::Result<Self> {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_name("mica-term-bg")
            .build()?;
        Ok(Self { runtime: Arc::new(runtime) })
    }

    pub fn handle(&self) -> tokio::runtime::Handle {
        self.runtime.handle().clone()
    }
}
```

在 `src/app/ssh/profile.rs` 定义：

```rust
pub enum SshAuthMethod {
    Password,
    PrivateKeyPath,
    PrivateKeyContent,
}

pub struct ConnectionProfile {
    pub asset_id: Option<String>,
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub auth_method: SshAuthMethod,
    pub credential_ref: Option<String>,
    pub private_key_path: Option<String>,
    pub remark: String,
}

impl ConnectionProfile {
    pub fn from_draft(draft: &AssetSshConnectionDraft) -> anyhow::Result<Self> {
        // normalize port/auth fields and reject inconsistent draft combinations
    }
}
```

在 `src/main.rs` 中，在 `bootstrap::run_with_profile()` 前初始化 runtime 并把 handle 传给 bootstrap。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test async_runtime_spec --test ssh_profile_spec
```

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/main.rs src/app/mod.rs src/app/async_runtime.rs src/app/ssh/mod.rs src/app/ssh/profile.rs tests/async_runtime_spec.rs tests/ssh_profile_spec.rs
git commit -m "feat: add tokio runtime and ssh profile domain"
```

### Task 3: 接入系统凭据存储与 `known_hosts` / TOFU 服务

**Files:**
- Modify: `src/app/ssh/mod.rs`
- Create: `src/app/ssh/credentials.rs`
- Create: `src/app/ssh/known_hosts.rs`
- Create: `ui/components/ssh-host-key-confirm-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/credential_store_spec.rs`
- Create: `tests/known_hosts_spec.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Create: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/credential_store_spec.rs` 增加：

```rust
#[test]
fn credential_store_round_trips_password_secret() {}

#[test]
fn credential_store_round_trips_inline_private_key_and_passphrase() {}
```

在 `tests/known_hosts_spec.rs` 增加：

```rust
#[test]
fn known_hosts_service_reports_unknown_host_for_first_contact() {}

#[test]
fn known_hosts_service_accepts_and_persists_tofu_entry() {}

#[test]
fn known_hosts_service_rejects_changed_host_key() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn host_key_confirm_modal_round_trips_target_host_and_fingerprint() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'export component SshHostKeyConfirmModal inherits Rectangle {' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback accept-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'callback reject-requested();' "$HOST_KEY_MODAL" >/dev/null
grep -F 'in-out property <bool> ssh-host-key-modal-open: false;' "$APP_WINDOW" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test credential_store_spec --test known_hosts_spec --test assets_modal_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL because there is no credential store abstraction
- FAIL because there is no known_hosts service or TOFU state
- FAIL because the host key confirm modal does not exist

**Step 3: Write minimal implementation**

在 `src/app/ssh/credentials.rs` 定义抽象：

```rust
pub trait CredentialStore: Send + Sync {
    fn put_secret(&self, key: &str, value: &str) -> anyhow::Result<()>;
    fn get_secret(&self, key: &str) -> anyhow::Result<Option<String>>;
    fn delete_secret(&self, key: &str) -> anyhow::Result<()>;
}

pub struct SystemCredentialStore;
pub struct MemoryCredentialStore;
```

在 `src/app/ssh/known_hosts.rs` 定义：

```rust
pub enum KnownHostCheck {
    Trusted,
    Unknown { fingerprint: String },
    Changed { expected: String, actual: String },
}

pub struct KnownHostsService { /* path + parser */ }

impl KnownHostsService {
    pub fn check(&self, host: &str, port: u16, key: &ssh_key::PublicKey) -> anyhow::Result<KnownHostCheck>;
    pub fn accept_unknown(&self, host: &str, port: u16, key: &ssh_key::PublicKey) -> anyhow::Result<()>;
}
```

在 `ui/components/ssh-host-key-confirm-modal.slint` 建立确认弹窗：

```slint
in property <string> host;
in property <string> fingerprint;
callback accept-requested();
callback reject-requested();
```

在 `src/shell/view_model.rs` 为连接前流程增加临时 host key prompt state，而不是直接把未知 host key 当 fatal error。

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test credential_store_spec --test known_hosts_spec --test assets_modal_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/credentials.rs src/app/ssh/known_hosts.rs ui/components/ssh-host-key-confirm-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/credential_store_spec.rs tests/known_hosts_spec.rs tests/assets_modal_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: add credential store and tofu host key flow"
```

### Task 4: 实现 `russh + wezterm-term + termwiz` 的 session runtime

**Files:**
- Modify: `src/app/ssh/mod.rs`
- Create: `src/app/ssh/runtime.rs`
- Create: `src/app/ssh/session_manager.rs`
- Create: `tests/ssh_session_manager_spec.rs`
- Create: `tests/terminal_session_spec.rs`

**Step 1: Write the failing tests**

在 `tests/ssh_session_manager_spec.rs` 增加：

```rust
#[test]
fn session_manager_creates_connecting_session_handle() {}

#[test]
fn session_manager_reuses_existing_session_for_same_asset_by_default() {}

#[test]
fn session_manager_can_force_new_tab_session_for_same_asset() {}

#[test]
fn session_manager_marks_session_as_error_when_runtime_fails() {}
```

在 `tests/terminal_session_spec.rs` 增加：

```rust
#[test]
fn terminal_session_applies_remote_bytes_to_wezterm_terminal() {}

#[test]
fn terminal_session_encodes_keyboard_input_with_termwiz_before_write() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_session_manager_spec --test terminal_session_spec
```

Expected:

- FAIL because there is no `SessionManager`
- FAIL because there is no terminal wrapper over `wezterm-term`

**Step 3: Write minimal implementation**

在 `src/app/ssh/runtime.rs` 建立运行时骨架：

```rust
pub enum SessionRuntimeEvent {
    Connected,
    Output(Vec<u8>),
    Disconnected,
    Error(String),
}

pub struct SshSessionRuntime {
    session_id: Uuid,
}

impl SshSessionRuntime {
    pub async fn connect(
        profile: ConnectionProfile,
        credentials: Arc<dyn CredentialStore>,
        known_hosts: Arc<KnownHostsService>,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> anyhow::Result<Self> {
        // russh::client::connect
        // authenticate_password or authenticate_publickey
        // channel_open_session
        // request_pty
        // request_shell
        // output loop
    }
}
```

在 `src/app/ssh/session_manager.rs` 建立：

```rust
pub enum OpenSessionMode {
    ActivateExisting,
    ForceNewTab,
}

pub struct SessionHandle {
    pub session_id: Uuid,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: SessionState,
}

pub struct SessionManager { /* maps + runtime handle + stores */ }
```

terminal 包装要求：

- `wezterm-term::Terminal` 负责状态
- `termwiz` 负责把键盘/控制输入编码成写往 SSH channel 的字节
- 先通过 fake writer / fake runtime 测试 terminal wrapper，不要先连真实服务器

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ssh_session_manager_spec --test terminal_session_spec
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs tests/ssh_session_manager_spec.rs tests/terminal_session_spec.rs
git commit -m "feat: add ssh session runtime and manager"
```

### Task 5: 建立真实 tab model 与 workspace session host

**Files:**
- Create: `src/shell/tabs.rs`
- Modify: `src/shell/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/app-window.slint`
- Create: `ui/shell/terminal-session-host.slint`
- Create: `tests/workspace_tabs_spec.rs`
- Modify: `tests/window_shell.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn tab_model_prefers_asset_name_then_host_for_title() {}

#[test]
fn tab_model_tracks_active_session_and_closeability() {}

#[test]
fn disconnected_session_stays_visible_and_can_reconnect() {}
```

在 `tests/assets_modal_smoke.rs` 增加：

```rust
#[test]
fn app_window_round_trips_workspace_tab_items_and_active_session() {}
```

在 `tests/window_shell.rs` 增加：

```rust
#[test]
fn tab_bar_contract_requires_workspace_tab_model_instead_of_single_placeholder() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加断言：

```bash
grep -F 'in-out property <[WorkspaceTabItem]> workspace-tab-items: [];' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-selected(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-tab-close-requested(string);' "$APP_WINDOW" >/dev/null
grep -F 'export component TerminalSessionHost inherits Rectangle {' "$WORKSPACE_HOST" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec --test window_shell --test assets_modal_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL because there is no workspace tab model or session host
- FAIL because `TabBar` is still a single static `ActiveTab`

**Step 3: Write minimal implementation**

在 `src/shell/tabs.rs` 定义：

```rust
pub struct WorkspaceTab {
    pub session_id: String,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: String,
    pub active: bool,
}
```

在 `ui/shell/tabbar.slint` 改为消费 model：

```slint
export struct WorkspaceTabItem {
    session_id: string,
    title: string,
    subtitle: string,
    state: string,
    active: bool,
}
```

在 `ui/shell/terminal-session-host.slint` 提供三种 UI 状态：

- `welcome`
- `terminal`
- `session-error`

在 `ui/app-window.slint`：

- 主工作区不再固定 `WelcomeView {}`
- 没有 session 时显示 `WelcomeView`
- 有 session 时显示 `TerminalSessionHost`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test workspace_tabs_spec --test window_shell --test assets_modal_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/tabs.rs src/shell/mod.rs src/shell/view_model.rs src/app/bootstrap.rs ui/shell/tabbar.slint ui/components/active-tab.slint ui/app-window.slint ui/shell/terminal-session-host.slint tests/workspace_tabs_spec.rs tests/window_shell.rs tests/assets_modal_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: add workspace tab model and session host"
```

### Task 6: 把 modal 动作、资产树、context menu 与 session manager 端到端接通

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/context_menu.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_context_menu_spec.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn connect_action_marks_modal_as_busy_and_requests_session_open() {}

#[test]
fn test_connection_action_does_not_create_workspace_tab() {}

#[test]
fn save_action_updates_asset_without_session_creation() {}
```

在 `tests/assets_context_menu_spec.rs` 增加：

```rust
#[test]
fn close_connection_is_disabled_without_live_session_and_enabled_with_live_session() {}

#[test]
fn open_in_new_tab_stays_enabled_for_ssh_assets() {}
```

在 `tests/assets_explorer_smoke.rs` 增加：

```rust
#[test]
fn opening_same_asset_twice_activates_existing_tab_by_default() {}

#[test]
fn open_in_new_tab_creates_second_session_for_same_asset() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec --test assets_explorer_smoke --test bootstrap_smoke
```

Expected:

- FAIL because action routing still only knows `confirm_asset_modal`
- FAIL because context menu still hardcodes `target_has_active_connection: true`
- FAIL because asset selection does not open or activate sessions

**Step 3: Write minimal implementation**

在 `src/shell/view_model.rs` 引入 SSH modal submit intent：

```rust
pub enum SshModalAction {
    Save,
    Connect,
    TestConnection,
    SaveAndConnect,
}
```

关键收敛：

- `Save` 只更新资产/profile
- `Connect` 默认先落 profile，再让 bootstrap 调用 `SessionManager`
- `TestConnection` 走 runtime，但不把结果写入 tab model
- `SaveAndConnect` 串联 `Save + Connect`

在 `src/app/bootstrap.rs`：

- 用 `slint::invoke_from_event_loop` 把后台 runtime 事件安全回送 UI 线程
- 维护 `SessionManager`
- 把 tab selection、tab close、asset selected、open-in-new-tab、close-connection 全部接入真实 session state

在 `src/shell/context_menu.rs` / `src/shell/view_model.rs`：

- 移除 `target_has_active_connection: true` 硬编码
- 让 `Close` 是否可点取决于 `SessionManager` 中是否有 live session

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model --test assets_context_menu_spec --test assets_explorer_smoke --test bootstrap_smoke
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/shell/context_menu.rs src/app/bootstrap.rs tests/shell_view_model.rs tests/assets_context_menu_spec.rs tests/assets_explorer_smoke.rs tests/bootstrap_smoke.rs
git commit -m "feat: wire ssh modal actions and session lifecycle"
```

### Task 7: 完成回归验证与最终文档留痕

**Files:**
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/assets_explorer_ui_contract_smoke.sh`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_explorer_smoke.rs`
- Modify: `docs/plans/2026-03-23-ssh-create-connect-tabs-design.md` (only if implementation reveals an approved contract mismatch)

**Step 1: Write the failing tests**

补充最终回归用例：

```rust
#[test]
fn unknown_host_key_prompts_once_then_reconnect_uses_trusted_key() {}

#[test]
fn disconnected_tab_stays_visible_until_user_closes_it() {}
```

并在 shell smoke 里锁定：

```bash
grep -F 'text: "Save and Connect";' "$SSH_MODAL" >/dev/null
grep -F 'text: "Test Connection";' "$SSH_MODAL" >/dev/null
grep -F 'for tab-item in root.workspace-tab-items' "$TABBAR" >/dev/null
```

**Step 2: Run the full verification suite**

Run:

```bash
cargo test
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- All Rust tests PASS
- All smoke scripts exit 0
- No stale placeholder-only tab contract remains

**Step 3: Fix any final failures**

Only apply minimal fixes needed to satisfy the confirmed design:

- no extra auth methods
- no `russh-sftp` UI
- no automatic reconnect
- no multi-pane

**Step 4: Re-run the full verification suite**

Run:

```bash
cargo test
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/assets_explorer_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add tests/assets_modal_ui_contract_smoke.sh tests/assets_explorer_ui_contract_smoke.sh tests/assets_modal_smoke.rs tests/assets_explorer_smoke.rs
git commit -m "test: lock ssh connect tabs regressions"
```
