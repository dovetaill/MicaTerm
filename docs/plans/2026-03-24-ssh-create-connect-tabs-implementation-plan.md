# SSH 新建 / 连接 / 标签页 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 基于 2026-03-24 已确认设计，修复资产 modal 的结构回归，落地单页 SSH 表单、secret 编辑语义、真实 tab/session 行为，以及首轮可交互 terminal session contract。

**Architecture:** 保持现有 `ShellViewModel -> bootstrap -> SessionManager -> SshSessionRuntime` 主链，不新增平行架构。UI 侧把 modal chrome 重新收回 `BlockingModalShell`，把 `AssetsSshConnectionModal` 收敛为单页分组表单；app 侧在现有 `credentials/profile/session_manager/runtime` 上补齐 edit-secret 语义、临时连接与已保存连接的动作投影，以及 UI 到 runtime 的 input / resize / surface 桥接。

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`, `keyring`, cargo test, shell smoke scripts

---

## 执行前提

- 只以 [2026-03-24-ssh-create-connect-tabs-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-24-ssh-create-connect-tabs-design.md) 为设计基准，不复用 `2026-03-23` 文档中的过时假设。
- 必须在独立 worktree 中执行；当前主工作区已经 dirty，不得回滚或覆盖无关改动。
- flat / no-radius 是硬约束，任何 modal、tab、terminal surface 都不能引入圆角回流。
- 本轮不扩展到 SFTP UI、proxy/tunnel/environment 实连、多 pane、完整资产持久化重构。
- 每个任务严格按 TDD 顺序推进：先写失败测试，再做最小实现，再跑回归，再 commit。

### Task 1: 收回 `BlockingModalShell` 的统一 chrome ownership

**Files:**
- Modify: `ui/components/blocking-modal-shell.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-folder-create-modal.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/components/assets-rename-modal.slint`
- Modify: `ui/components/assets-delete-confirm-modal.slint`
- Modify: `ui/components/ssh-host-key-confirm-modal.slint`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/assets_modal_smoke.rs` 增加基于源码字符串的契约测试：

```rust
#[test]
fn blocking_modal_shell_owns_shared_asset_modal_chrome_contract() {
    let shell = std::fs::read_to_string("ui/components/blocking-modal-shell.slint").unwrap();
    let folder = std::fs::read_to_string("ui/components/assets-folder-create-modal.slint").unwrap();
    let ssh = std::fs::read_to_string("ui/components/assets-ssh-connection-modal.slint").unwrap();

    assert!(shell.contains("in property <string> dialog-title"));
    assert!(shell.contains("callback close-requested();"));
    assert!(shell.contains("header := Rectangle {"));
    assert!(shell.contains("close-button := Rectangle {"));
    assert!(!folder.contains("drag-touch := TouchArea {"));
    assert!(!ssh.contains("drag-touch := TouchArea {"));
}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 更新断言，锁定 shared shell contract：

```bash
grep -F 'in property <string> dialog-title: "";' "$MODAL_SHELL" >/dev/null
grep -F 'callback close-requested();' "$MODAL_SHELL" >/dev/null
grep -F 'header := Rectangle {' "$MODAL_SHELL" >/dev/null
grep -F 'close-button := Rectangle {' "$MODAL_SHELL" >/dev/null
! grep -F 'drag-touch := TouchArea {' "$FOLDER_MODAL" >/dev/null
! grep -F 'drag-touch := TouchArea {' "$SSH_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL，因为当前 `BlockingModalShell` 还没有 header / close chrome；
- FAIL，因为 `AssetsFolderCreateModal` 与 `AssetsSshConnectionModal` 仍然各自拥有 drag/header/close；
- FAIL，因为现有 smoke 脚本仍然锁定了旧的 child-owned chrome 结构。

**Step 3: Write the minimal implementation**

实现目标：

- 在 `ui/components/blocking-modal-shell.slint` 新增统一 header contract：

```slint
in property <string> dialog-title: "";
callback close-requested();
```

- 把 title / divider / close button / drag hit area 全部迁回 `BlockingModalShell`；
- `ui/app-window.slint` 中的资产相关 modal 统一通过 shell 提供 `dialog-title` 与 `close-requested`；
- 从 `assets-folder-create-modal.slint`、`assets-ssh-connection-modal.slint`、`assets-rename-modal.slint`、`assets-delete-confirm-modal.slint`、`ssh-host-key-confirm-modal.slint` 删除顶层 header / close button / drag 区，只保留 body / footer / Esc / Enter 等局部交互；
- 保持 `focus-restore-requested`、`confirm-requested`、`folder-name-changed`、`draft-changed` 等业务回调不变。

**Step 4: Re-run tests**

Run:

```bash
cargo test --test assets_modal_smoke
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/blocking-modal-shell.slint ui/app-window.slint ui/components/assets-folder-create-modal.slint ui/components/assets-ssh-connection-modal.slint ui/components/assets-rename-modal.slint ui/components/assets-delete-confirm-modal.slint ui/components/ssh-host-key-confirm-modal.slint tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "refactor: restore shared modal shell chrome ownership"
```

### Task 2: 把 `SSH New/Edit` modal 收敛为单页分组表单

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/assets_modal_smoke.rs` 替换依赖顶层 tab 的断言，新增：

```rust
#[test]
fn ssh_modal_round_trips_grouped_form_fields_without_top_level_tab_state() {
    i_slint_backend_testing::init_no_event_loop();
    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("22".into());

    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
}
```

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn new_ssh_modal_state_no_longer_tracks_top_level_tab_enum() {}

#[test]
fn new_ssh_modal_exposes_connection_authentication_metadata_and_more_settings_groups() {}
```

在 `tests/assets_modal_ui_contract_smoke.sh` 增加并替换断言：

```bash
! grep -F 'in property <string> active-tab:' "$SSH_MODAL" >/dev/null
! grep -F 'callback tab-selected(string);' "$SSH_MODAL" >/dev/null
grep -F 'text: "Connection"' "$SSH_MODAL" >/dev/null
grep -F 'text: "Authentication"' "$SSH_MODAL" >/dev/null
grep -F 'text: "Metadata"' "$SSH_MODAL" >/dev/null
grep -F 'text: "More Settings"' "$SSH_MODAL" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 `AssetModalState::NewSshConnection` 仍然依赖 `AssetSshModalTab`；
- FAIL，因为 SSH modal 仍然暴露 `active-tab` 和 `tab-selected`；
- FAIL，因为当前 UI 仍是顶层 tab + scroll 区结构，尚未出现分组标题。

**Step 3: Write the minimal implementation**

实现目标：

- 在 `src/shell/view_model.rs` 删除 `AssetModalState::NewSshConnection` 中的 `active_tab`；
- 删除 `AssetSshModalTab` 枚举以及 `asset-ssh-modal-tab-selected` 的主链绑定；
- 在 `ui/components/assets-ssh-connection-modal.slint` 重排为单页滚动表单：

```text
Connection
  Name / Host / User / Port
Authentication
  Authentication Type / Password or Private Key fields / Passphrase
Metadata
  Remark
More Settings
  Environment / Proxy Method
```

- footer 固定在 modal 底部，`ScrollView` 只负责 body，不再把动作区一起卷走；
- 保持 `Save / Connect / Test Connection / Save and Connect` 四动作，但不再用顶层 tabs 占位 `Proxy / Environment / Advanced`；
- `ui/app-window.slint` 与 `src/app/bootstrap.rs` 清理不再使用的 `asset-ssh-modal-active-tab` 状态桥接。

**Step 4: Re-run tests**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/shell/view_model.rs src/app/bootstrap.rs tests/assets_modal_smoke.rs tests/assets_modal_ui_contract_smoke.sh tests/shell_view_model.rs
git commit -m "feat: flatten ssh modal into grouped single-page form"
```

### Task 3: 固化 secret 保存与 edit-mode 留空保留语义

**Files:**
- Modify: `src/app/ssh/credentials.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/credential_store_spec.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/assets_modal_smoke.rs`

**Step 1: Write the failing tests**

在 `tests/credential_store_spec.rs` 增加：

```rust
#[test]
fn editing_saved_secret_fields_blank_keeps_existing_bundle() {}

#[test]
fn explicit_clear_saved_secret_deletes_bundle() {}
```

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn editing_saved_ssh_modal_exposes_leave_blank_helper_copy() {}

#[test]
fn editing_saved_ssh_modal_allows_explicit_clear_saved_secret_action() {}
```

在 `tests/assets_modal_smoke.rs` 增加 round-trip 属性断言：

```rust
#[test]
fn ssh_modal_round_trips_secret_retention_copy_and_clear_affordance() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test credential_store_spec --test shell_view_model --test assets_modal_smoke
```

Expected:

- FAIL，因为当前 edit-mode 对 blank secret 没有正式“保留旧值”契约；
- FAIL，因为还没有 explicit clear saved secret action；
- FAIL，因为 app/window/slint 还没有 helper copy 与 clear affordance 的属性桥接。

**Step 3: Write the minimal implementation**

实现目标：

- 在 `src/shell/view_model.rs` 为 edit-mode 增加 secret UI state：

```rust
pub struct AssetSshConnectionDraft {
    // existing fields...
    pub secret_retention_message: String,
    pub can_clear_saved_secret: bool,
    pub clear_saved_secret_requested: bool,
}
```

- 在 `ui/components/assets-ssh-connection-modal.slint` 的 `Authentication` 组中增加说明文案与显式 clear affordance：

```text
Leave password / private key / passphrase blank to keep the saved secret.
[Clear Saved Secret]
```

- 在 `src/app/bootstrap.rs` 保存 edit-mode 资产时：
  - 如果 `clear_saved_secret_requested == true`，调用 `delete_secret`;
  - 如果 secret 字段为空且未 clear，先读取现有 `StoredSshSecretBundle`，再只覆盖非空输入；
  - 如果是 `Save and Connect`，先落盘 asset 与 merged secret，再继续打开会话；
- `src/app/ssh/profile.rs` 维持 `ConnectionProfile` 的 normalized 入口，但不要把 edit-mode 空字符串误判为“清空密钥”；
- `src/app/ssh/credentials.rs` 如有必要增加 merge helper：

```rust
fn merge_edit_bundle(existing: StoredSshSecretBundle, draft: &AssetSshConnectionDraft) -> StoredSshSecretBundle
```

**Step 4: Re-run tests**

Run:

```bash
cargo test --test credential_store_spec --test shell_view_model --test assets_modal_smoke
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/credentials.rs src/app/ssh/profile.rs src/shell/view_model.rs src/app/bootstrap.rs ui/components/assets-ssh-connection-modal.slint ui/app-window.slint tests/credential_store_spec.rs tests/shell_view_model.rs tests/assets_modal_smoke.rs
git commit -m "feat: preserve saved ssh secrets across blank edit fields"
```

### Task 4: 收紧 `Save / Connect / Test / Save and Connect` 动作模型

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

在 `tests/bootstrap_smoke.rs` 增加或改造：

```rust
#[test]
fn connect_action_keeps_session_ephemeral_and_does_not_persist_asset() {}

#[test]
fn save_and_connect_persists_asset_then_opens_session_with_saved_identity() {}

#[test]
fn test_connection_updates_feedback_without_creating_workspace_tab() {}
```

在 `tests/shell_view_model.rs` 增加：

```rust
#[test]
fn connect_family_enablement_depends_on_connection_minimum_fields() {}

#[test]
fn busy_action_blocks_duplicate_ssh_modal_submissions() {}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test shell_view_model
```

Expected:

- FAIL，因为现有动作虽然有 wiring，但对临时连接 / 已保存连接 / busy gate / feedback 文案的最终语义还没完全锁死；
- FAIL，因为 edit-mode secret merge 后的 `Save and Connect` 路径还没有完整覆盖；
- FAIL，因为 `Test Connection` 仍可能与 tab/session 投影边界产生回归。

**Step 3: Write the minimal implementation**

实现目标：

- 在 `src/shell/view_model.rs` 保持四动作枚举，但把状态机固定为：

```text
Idle -> Busy(action) -> Success(message) | Error(message) -> Idle
```

- 在 `src/app/bootstrap.rs` 明确四动作边界：
  - `Save`: 保存资产与 secret，不开 tab；
  - `Connect`: 从 draft 建临时 `ConnectionProfile`，只开 session，不持久化 asset；
  - `Test Connection`: 只 probe，更新 inline feedback，不创建 tab；
  - `Save and Connect`: 先保存资产与 merged secret，再打开 session；
- 临时连接统一使用稳定的 ephemeral identity 规则，避免误写入 `AssetCatalogRepository`；
- `busy` 状态下禁用重复提交，直到动作完成后才释放按钮；
- 保留 unknown-host 分支、失败分支和日志落点，不把失败吞掉。

**Step 4: Re-run tests**

Run:

```bash
cargo test --test bootstrap_smoke --test shell_view_model
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/shell/view_model.rs src/app/ssh/profile.rs tests/bootstrap_smoke.rs tests/shell_view_model.rs
git commit -m "feat: finalize ssh modal action flow"
```

### Task 5: 修正 `SSH tab` 的视觉、关闭命中与复用行为

**Files:**
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/workspace-pane.slint`
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
fn active_tab_layout_preserves_close_hit_target_and_elides_text() {}

#[test]
fn closing_active_tab_falls_back_right_then_left_then_welcome() {}
```

在 `tests/ssh_session_manager_spec.rs` 增加：

```rust
#[test]
fn reopening_same_saved_asset_activates_existing_session_by_default() {}

#[test]
fn force_new_tab_creates_parallel_session_for_same_asset() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加：

```bash
! grep -F 'width: 216px;' "$TABBAR" >/dev/null
grep -F 'min-width: 0px;' "$ACTIVE_TAB" >/dev/null
grep -F 'callback close-requested();' "$ACTIVE_TAB" >/dev/null
grep -F 'overflow: elide;' "$ACTIVE_TAB" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_session_manager_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 `TabBar` 当前仍然硬编码 `width: 216px;`；
- FAIL，因为 tab 文字和 close hit target 的布局仍可能互相挤压；
- FAIL，因为虽已有 close callback 和 reuse 基线，但视觉和 contract 还没有锁到本轮最终状态。

**Step 3: Write the minimal implementation**

实现目标：

- 在 `ui/components/active-tab.slint`：
  - 为 title/subtitle 容器保留 `min-width: 0px`；
  - 扩大 close hit target，避免点中 close 时触发 select；
  - 保持 subtitle elide，不允许文字溢出边界；
- 在 `ui/shell/tabbar.slint`：
  - 去掉固定 `216px`；
  - 改为 stretch + min width + overflow-safe 的 tab row；
- 在 `src/shell/tabs.rs` / `src/shell/view_model.rs` / `src/app/bootstrap.rs`：
  - 继续保持 `Name` 优先、`host` 回退的标题规则；
  - 默认复用同 asset 的 live tab；
  - `Open in New Tab` 仍然走 `OpenSessionMode::ForceNewTab`；
  - 失败 tab 保留，用户可手动关闭。

**Step 4: Re-run tests**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_session_manager_spec
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/components/active-tab.slint ui/shell/tabbar.slint ui/shell/workspace-pane.slint src/shell/tabs.rs src/shell/view_model.rs src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/ssh_session_manager_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "fix: refine ssh workspace tab layout and lifecycle"
```

### Task 6: 把 terminal host 从 `screen_text` 占位升级为可交互 session contract

**Files:**
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

在 `tests/ssh_session_manager_spec.rs` 增加：

```rust
#[test]
fn session_manager_forwards_text_input_and_resize_to_runtime_control() {}

#[test]
fn runtime_surface_snapshot_tracks_visible_rows_instead_of_single_placeholder_copy() {}
```

在 `tests/workspace_tabs_spec.rs` 增加：

```rust
#[test]
fn terminal_session_host_exposes_text_key_and_resize_callbacks() {}
```

在 `tests/bootstrap_smoke.rs` 增加：

```rust
#[test]
fn workspace_terminal_input_callback_updates_active_session_surface() {}
```

在 `tests/ssh_connect_tabs_ui_contract_smoke.sh` 增加：

```bash
grep -F 'callback workspace-session-text-input(string);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-key-input(string, bool, bool, bool);' "$APP_WINDOW" >/dev/null
grep -F 'callback workspace-session-resize-requested(int, int);' "$APP_WINDOW" >/dev/null
grep -F 'callback text-input(string);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback key-input(string, bool, bool, bool);' "$WORKSPACE_HOST" >/dev/null
grep -F 'callback surface-resize-requested(int, int);' "$WORKSPACE_HOST" >/dev/null
! grep -F 'Remote shell is ready but has not produced output yet.' "$WORKSPACE_HOST" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_session_manager_spec --test workspace_tabs_spec --test bootstrap_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- FAIL，因为 `SessionRuntimeControl` 目前只有 `disconnect()`，没有 input / resize；
- FAIL，因为 `TerminalSurfaceState` 仍然只面向 `screen_text` 字符串；
- FAIL，因为 `TerminalSessionHost` 与 `AppWindow` 还没有 terminal input / key / resize callbacks；
- FAIL，因为当前 terminal host 仍然带有 placeholder copy。

**Step 3: Write the minimal implementation**

实现目标：

- 扩展 `src/app/ssh/session_manager.rs` 的 runtime control contract：

```rust
pub trait SessionRuntimeControl: Send {
    fn disconnect(&self) -> Result<()>;
    fn send_input(&self, bytes: Vec<u8>) -> Result<()>;
    fn resize(&self, rows: u32, cols: u32) -> Result<()>;
}
```

- 在 `src/app/ssh/runtime.rs`：
  - 复用现有 `send_input()` / `resize()` 能力接到 trait；
  - 把 `TerminalSurfaceState` 从单一 `screen_text` 升级为“可渲染 surface snapshot”，至少包含 `rows / cols / visible_lines / seqno`；
  - 继续由 `wezterm-term` 提供 surface snapshot，不重写 terminal core；
- 在 `ui/shell/terminal-session-host.slint`：
  - 改为 focusable terminal surface host；
  - 通过 `TextInput` / `FocusScope` / `key-pressed` 捕获 printable text、named key 和 size 变化；
  - 通过 `text-input` / `key-input` / `surface-resize-requested` 回调把事件抛给 Rust；
  - 以 visible line model 渲染首轮 surface，而不是继续显示 placeholder 说明文字；
- 在 `ui/app-window.slint` / `ui/shell/workspace-pane.slint` / `src/app/bootstrap.rs`：
  - 增加对应 callback bridge；
  - 仅把事件发给 active session；
  - 在激活 tab 与尺寸变化时同步触发 `resize`。

**Step 4: Re-run tests**

Run:

```bash
cargo test --test ssh_session_manager_spec --test workspace_tabs_spec --test bootstrap_smoke
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ssh/session_manager.rs src/app/ssh/runtime.rs src/app/bootstrap.rs src/shell/view_model.rs ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint tests/ssh_session_manager_spec.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs tests/ssh_connect_tabs_ui_contract_smoke.sh
git commit -m "feat: wire interactive terminal session contract"
```

### Task 7: 跑完整回归并记录风险边界

**Files:**
- Modify: `docs/plans/2026-03-24-ssh-create-connect-tabs-design.md` (only if implementation reveals a material divergence that must be documented)
- No code changes expected otherwise

**Step 1: Run the full targeted verification set**

Run:

```bash
cargo test --test assets_modal_smoke --test bootstrap_smoke --test credential_store_spec --test shell_view_model --test workspace_tabs_spec --test ssh_session_manager_spec
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- all listed Rust tests PASS
- both shell smoke scripts PASS
- no test output references the removed top-level SSH tabs
- no test output references modal child-owned chrome

**Step 2: Run one build-level sanity check**

Run:

```bash
cargo check
```

Expected: PASS without introducing new warnings that indicate dead modal/tab/runtime bridges.

**Step 3: Manually verify the three user-reported regressions**

手工检查：

- `New Folder` modal 的 title / close / drag / input / footer 几何恢复；
- `New SSH Connection` / `Edit SSH Connection` modal 不再出现标题漂移、输入框越界、端口不可编辑、footer 不可见；
- SSH tab 的 title / subtitle / close 命中正常，失败 tab 可关闭。

如果手工检查发现与设计偏差，先修实现，不要回退设计决策。

**Step 4: Record any divergence**

如果实现过程中发现必须偏离 [2026-03-24-ssh-create-connect-tabs-design.md](/home/wwwroot/mica-term/docs/plans/2026-03-24-ssh-create-connect-tabs-design.md)，只追加一节：

```md
## Implementation Divergence
- reason
- impact
- follow-up
```

若无偏差，不修改 design doc。

**Step 5: Commit**

```bash
git add docs/plans/2026-03-24-ssh-create-connect-tabs-design.md
git commit -m "docs: record ssh modal connect tabs implementation verification"
```

仅当 design doc 有实际改动时执行上面的 commit；如果无文档差异，本任务不新增 commit。

## 风险提醒

- `Task 1` 和 `Task 2` 必须分开提交，否则 modal shell 回收与 SSH IA 重排混在一起后很难定位回归来源。
- `Task 3` 不能跳过 edit-mode secret merge 测试，否则最容易出现“留空即清空”的隐性数据损坏。
- `Task 5` 不能只改视觉不改 tests；tab close hit target 与 reuse behavior 必须同时锁定。
- `Task 6` 是本轮技术风险最高的任务；如果 surface snapshot 复杂度超预期，可以先交付 line-model renderer，但不能把纯 `screen_text` placeholder 重新包装成“真实终端”。
- 如果 `Task 6` 暴露出需要额外 renderer 子计划，允许在完成 input / resize / active-session bridge 后再补一个 follow-up design doc，但当前任务必须先让用户实际可输入、可切 tab、可看到真实 surface 更新。

## 完成定义

满足以下条件才可宣称本轮完成：

- `BlockingModalShell` 重新成为所有 blocking asset modal 的统一 chrome owner；
- `New Folder` 与 `New/Edit SSH` 不再出现标题栏、关闭按钮、拖动区和 footer 几何回归；
- SSH modal 不再使用顶层 `Standard / Proxy / Environment / Advanced` tabs；
- edit-mode 下 secret 留空保留旧值，explicit clear 才删除 secret；
- `Save / Connect / Test Connection / Save and Connect` 行为与反馈一致；
- SSH tab 可关闭、文字不溢出、失败 tab 保留并可手动关闭；
- terminal host 已具备 active-session input / key / resize / surface update contract；
- 全部目标测试与 smoke scripts 通过。
