# Modal Input and Select Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 修复所有复用 `DialogTextField` 的编辑类 modal 的文本点击定位/拖选问题，把 secret reveal 统一成 Fluent 眼睛持久 toggle，并让编辑 modal 内的 select/dropdown 不再在底部被裁切。

**Architecture:** 先在共享 `DialogTextField` 层移除会吞掉命中的整块 overlay，并补一个可复用的 trailing icon action API，再分别给 SSH / keychain identity / sync modal 接入持久显示/隐藏状态与 reset 规则。随后在共享 modal chrome 层增加 modal-local select primitive，由 SSH 和 snippet modal 在自身根节点渲染 dropdown overlay，彻底脱离 `ScrollView` 内的 stock `ComboBox` popup，同时尽量保持现有 draft field id、bootstrap binder、保存逻辑和 label/value 映射不变。

**Tech Stack:** Slint, Rust, bash smoke scripts, `cargo test`, `cargo check`, software renderer modal render specs

---

**Guardrails:**
- 不要改动 `docs/plans/2026-04-21-terminal-visual-highlight-redesign-design.md`
- 不要改动 `docs/plans/2026-04-21-terminal-visual-highlight-redesign-implementation-plan.md`
- 保持现有业务 field id 尽量稳定，例如 `password_visibility`、`proxy_socks5_password_visibility`、`keychain_identity_label`、`proxy_ssh_asset_label`、`package`
- 编辑 modal 内不要重新引入 stock `ComboBox` popup 到 `ModalBodyScrollArea` 的 `ScrollView` 内容层

### Task 1: 冻结共享输入 hardening contract 与 Fluent 眼睛资源

**Files:**
- Create: `tests/modal_input_select_contract_spec.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/keychain_ui_contract_smoke.sh`
- Create: `assets/icons/fluent/eye-20-regular.svg`
- Create: `assets/icons/fluent/eye-off-20-regular.svg`

**Step 1: Write the failing test**

在 `tests/modal_input_select_contract_spec.rs` 中新增 source-level contract，至少覆盖：

```rust
#[test]
fn dialog_text_field_contract_exposes_icon_action_and_no_full_surface_touch_overlay() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint").unwrap();

    assert!(source.contains("export component DialogFieldIconAction inherits Rectangle {"));
    assert!(!source.contains("field-touch := TouchArea {\n            width: parent.width;\n            height: parent.height;"));
}

#[test]
fn fluent_eye_assets_exist_for_modal_secret_toggle() {
    for path in [
        "assets/icons/fluent/eye-20-regular.svg",
        "assets/icons/fluent/eye-off-20-regular.svg",
    ] {
        assert!(std::path::Path::new(path).exists(), "missing {path}");
    }
}
```

同时更新：
- `tests/assets_modal_ui_contract_smoke.sh`，要求 `ui/components/modal-chrome.slint` 暴露新的 field icon action contract
- `tests/keychain_ui_contract_smoke.sh`，要求 keychain identity modal 使用 Fluent eye icon affordance，而不是文字 `Show`

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/keychain_ui_contract_smoke.sh
```

Expected:
- FAIL，因为 eye icon 资源还不存在，`DialogFieldIconAction` 也还没有落地，`DialogTextField` 仍被整块 `TouchArea` 覆盖。

**Step 3: Write minimal implementation**

先补最小资源和 contract 骨架：

```slint
export component DialogFieldIconAction inherits Rectangle {
    in property <image> icon-source;
    in property <string> action-label: "";
    in property <bool> enabled: true;
    callback clicked();
}
```

并添加两份 Fluent icon 资源：
- `assets/icons/fluent/eye-20-regular.svg`
- `assets/icons/fluent/eye-off-20-regular.svg`

此时只需要把资源与 exported component 骨架补齐，不要顺手改消费者逻辑。

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/keychain_ui_contract_smoke.sh
```

Expected:
- PASS，基础资源和 shared contract 被钉住。

**Step 5: Commit**

```bash
git add tests/modal_input_select_contract_spec.rs tests/assets_modal_ui_contract_smoke.sh tests/keychain_ui_contract_smoke.sh assets/icons/fluent/eye-20-regular.svg assets/icons/fluent/eye-off-20-regular.svg ui/components/modal-chrome.slint
git commit -m "test: freeze modal input icon contract"
```

### Task 2: 修正 `DialogTextField` 命中结构并切到 trailing icon action API

**Files:**
- Modify: `ui/components/modal-chrome.slint`
- Modify: `tests/modal_input_select_contract_spec.rs`
- Modify: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing test**

把 contract 再收紧一层，明确禁止 full-surface overlay 回归，并要求共享 field 为 trailing icon 留出稳定 gutter：

```rust
#[test]
fn dialog_text_field_contract_only_uses_focus_helpers_outside_text_viewport() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint").unwrap();

    assert!(!source.contains("field-touch := TouchArea {"));
    assert!(source.contains("trailing-icon-action"));
}
```

并更新 `tests/assets_modal_render_spec.rs` 里的 `ssh_modal_narrow_viewport_preserves_right_gutter_after_trailing_action`，让它改为断言 icon slot 的 input width / gutter contract，而不是旧的 64px 文字按钮：

```rust
assert!(chrome.contains("root.trailing-icon-visible ? 36px : 0px"));
assert!(chrome.contains("x: parent.width - self.width - 6px;"));
```

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
cargo test --features slint-renderer-software --test assets_modal_render_spec ssh_modal_narrow_viewport_preserves_right_gutter_after_trailing_action -- --exact
```

Expected:
- FAIL，因为 `DialogTextField` 仍有整块 `TouchArea`，也还没有 icon-based trailing action 宽度契约。

**Step 3: Write minimal implementation**

在 `ui/components/modal-chrome.slint` 中完成共享 field 修正：

```slint
in property <bool> trailing-icon-visible: false;
in property <image> trailing-icon-source;
in property <string> trailing-icon-label: "";
callback trailing-icon-requested();

field-input := TextInput {
    width: parent.width - 24px - (root.trailing-icon-visible ? 36px : 0px);
}

if root.trailing-icon-visible : trailing-icon-action := DialogFieldIconAction {
    x: parent.width - self.width - 6px;
    y: (parent.height - self.height) / 2;
    icon-source: root.trailing-icon-source;
    action-label: root.trailing-icon-label;
    clicked => { root.trailing-icon-requested(); }
}
```

实现要点：
- 删除 `ui/components/modal-chrome.slint:470` 一类覆盖整块 field 的 `TouchArea`
- 如果保留 click-to-focus 辅助，只允许命中左右 padding，不得盖住 `TextInput` 真实编辑区
- multiline 与 single-line 共用同一命中原则
- 旧的 `trailing-action-text` / `trailing-action-requested()` 在所有消费者迁完之前可以保留兼容，但不要再作为 secret reveal 的默认 API

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
cargo test --features slint-renderer-software --test assets_modal_render_spec ssh_modal_narrow_viewport_preserves_right_gutter_after_trailing_action -- --exact
```

Expected:
- PASS，`DialogTextField` 不再由 full-surface overlay 抢占命中，icon trailing slot contract 固定下来。

**Step 5: Commit**

```bash
git add ui/components/modal-chrome.slint tests/modal_input_select_contract_spec.rs tests/assets_modal_render_spec.rs
git commit -m "fix: harden dialog text field hit testing"
```

### Task 3: 给 SSH modal 接入持久 reveal toggle 与 reset 规则

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/assets.rs`
- Modify: `src/shell/view_model/ssh_modal.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/ssh_profile_spec.rs`
- Modify: `tests/credential_store_spec.rs`

**Step 1: Write the failing tests**

补 SSH reveal state 的 targeted tests，至少覆盖：
- `password_visible`、`passphrase_visible`、`proxy_socks5_password_visible` round-trip
- 切换 `auth_source`、`auth_method`、`proxy_type` 时相关 reveal state 被重置
- reopen / hydrate edit secret 后 reveal state 仍回到 hidden

示例：

```rust
#[test]
fn ssh_modal_reveal_flags_reset_when_auth_source_changes() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_field("password_visibility", "visible".into());
    view_model.update_ssh_modal_field("passphrase_visibility", "visible".into());
    view_model.update_ssh_modal_field("auth_source", "keychain-identity".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if !draft.password_visible && !draft.passphrase_visible
    ));
}
```

还要在 `tests/assets_modal_smoke.rs` 中补 window property round-trip：

```rust
app.set_asset_ssh_modal_passphrase_visible(true);
assert!(app.get_asset_ssh_modal_passphrase_visible());
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model --test ssh_profile_spec --test credential_store_spec -q
```

Expected:
- FAIL，因为 `passphrase_visible` 还不存在，SSH modal reset 规则也没有覆盖到 passphrase/icon toggle。

**Step 3: Write minimal implementation**

在 SSH draft / window / modal 三层接通 reveal state：

```rust
pub struct AssetSshConnectionDraft {
    pub password_visible: bool,
    pub passphrase_visible: bool,
    pub proxy_socks5_password_visible: bool,
}
```

在 `src/shell/view_model/ssh_modal.rs` 中处理：

```rust
"passphrase_visibility" => {
    draft.passphrase_visible = matches!(value.as_str(), "visible" | "show" | "true");
}
```

在 `ui/components/assets-ssh-connection-modal.slint` 中把旧文字 `Show/Hide` 改成 Fluent 眼睛 icon toggle：

```slint
trailing-icon-visible: true;
trailing-icon-source: root.password-visible ? root.eye-off-icon : root.eye-icon;
trailing-icon-label: root.password-visible ? "Hide password" : "Show password";
trailing-icon-requested => {
    root.draft-changed("password_visibility", root.password-visible ? "hidden" : "visible");
}
```

同样处理：
- 主密码
- 私钥 passphrase
- proxy SOCKS5 password

并在以下入口统一 reset 为 hidden：
- `open_new_ssh_modal`
- `open_edit_ssh_modal`
- `hydrate_edit_ssh_modal_secret`
- `auth_source` / `auth_method` / `proxy_type` 切换
- modal close / clear path in `src/app/bootstrap/assets_keychain.rs`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test assets_modal_smoke --test shell_view_model --test ssh_profile_spec --test credential_store_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- PASS，SSH secret reveal state round-trip 正常，reset 规则稳定，source smoke 里不再要求旧文字 `Show`。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/shell/view_model/assets.rs src/shell/view_model/ssh_modal.rs src/app/bootstrap/assets_keychain.rs ui/app-window.slint ui/components/assets-ssh-connection-modal.slint tests/assets_modal_smoke.rs tests/shell_view_model.rs tests/ssh_profile_spec.rs tests/credential_store_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: add ssh modal reveal icon toggles"
```

### Task 4: 给 keychain identity 与 sync modal 接入 reveal toggle 和 reset 规则

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/keychain.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/assets_keychain.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-keychain-identity-modal.slint`
- Modify: `ui/components/sync-vault-modal.slint`
- Modify: `tests/keychain_modal_smoke.rs`
- Modify: `tests/keychain_identity_actions_spec.rs`
- Modify: `tests/sync_vault_modal_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing tests**

新增 targeted tests，至少覆盖：
- keychain identity password reveal 不跨 auth kind 切换、不跨 reopen 持久化
- sync modal 的 `master-password`、`git-https-secret`、`git-ssh-passphrase` reveal state 在 close / reopen / auth-mode 切换后都会 reset
- source/render contract 要求这些 modal 改为 eye icon affordance，而不是纯隐藏字段

示例：

```rust
#[test]
fn keychain_identity_password_reveal_resets_when_switching_auth_kind() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_keychain_identity_modal(None);
    view_model.update_keychain_identity_modal_field("password_visibility", "visible".into());
    view_model.update_keychain_identity_modal_field("auth_kind", "ssh-key".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewKeychainIdentity { ref draft, .. }) if !draft.password_visible
    ));
}
```

```rust
#[test]
fn sync_modal_secret_visibility_resets_on_close_and_auth_mode_change() {
    let mut view_model = ShellViewModel::default();
    view_model.open_sync_modal();
    view_model.update_sync_modal_field("master-password-visibility", "visible".into());
    view_model.close_sync_modal();
    view_model.open_sync_modal();
    assert!(!view_model.sync_modal_state().master_password_visible);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test keychain_modal_smoke --test keychain_identity_actions_spec --test sync_vault_modal_smoke -q
cargo test --features slint-renderer-software --test assets_modal_render_spec sync_modal_short_viewport_keeps_master_password_field_actionable -- --exact
```

Expected:
- FAIL，因为 keychain identity draft 和 sync modal state 还没有 visibility fields，也没有 icon toggle wiring。

**Step 3: Write minimal implementation**

增加最小 reveal state：

```rust
pub struct KeychainIdentityDraft {
    pub password_visible: bool,
}

pub struct SyncModalViewState {
    pub master_password_visible: bool,
    pub git_https_secret_visible: bool,
    pub git_ssh_passphrase_visible: bool,
}
```

在 `src/shell/view_model/keychain.rs` / `src/shell/view_model/projection.rs` 中处理对应 field id：

```rust
"password_visibility" => {
    draft.password_visible = matches!(value.as_str(), "visible" | "show" | "true");
}
```

```rust
"master-password-visibility" => modal.master_password_visible = visible(value.as_str()),
"git-https-secret-visibility" => modal.git_https_secret_visible = visible(value.as_str()),
"git-ssh-passphrase-visibility" => modal.git_ssh_passphrase_visible = visible(value.as_str()),
```

UI 侧：
- `ui/components/assets-keychain-identity-modal.slint` 的 Password 字段改为 eye icon toggle
- `ui/components/sync-vault-modal.slint` 的 master password / HTTPS secret / SSH passphrase 改为 eye icon toggle
- `git-ssh-private-key` 仍保持隐藏 multiline 编辑，不强行加 reveal toggle，避免把大段私钥可见态带入这次范围

Reset 规则：
- keychain identity 切换 `auth_kind`
- sync modal 切换 `git-auth-mode`
- sync modal close / reopen / submit 成功后
- bootstrap/window projection clear path

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test keychain_modal_smoke --test keychain_identity_actions_spec --test sync_vault_modal_smoke -q
bash tests/keychain_ui_contract_smoke.sh
cargo test --features slint-renderer-software --test assets_modal_render_spec sync_modal_short_viewport_keeps_master_password_field_actionable -- --exact
```

Expected:
- PASS，keychain/sync 的 reveal state 与 reset contract 生效，short viewport render spec 不回归。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/shell/view_model/keychain.rs src/shell/view_model/projection.rs src/app/bootstrap/assets_keychain.rs src/app/bootstrap/windowing.rs ui/app-window.slint ui/components/assets-keychain-identity-modal.slint ui/components/sync-vault-modal.slint tests/keychain_modal_smoke.rs tests/keychain_identity_actions_spec.rs tests/sync_vault_modal_smoke.rs tests/keychain_ui_contract_smoke.sh tests/assets_modal_render_spec.rs
git commit -m "feat: extend reveal toggles to keychain and sync modals"
```

### Task 5: 冻结 shared modal select primitive contract

**Files:**
- Modify: `tests/modal_input_select_contract_spec.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `ui/components/modal-chrome.slint`

**Step 1: Write the failing test**

在 `tests/modal_input_select_contract_spec.rs` 追加 shared select primitive contract，至少覆盖：
- `ui/components/modal-chrome.slint` 导出 `DialogSelectField`
- field 和 popup 分离，popup 具备 `open`、`dismiss-requested()`、`option-selected(string)`、`move-highlight-requested(int)` 之类的最小 callback/property
- popup 支持上/下展开方向或最大高度约束，不依赖 stock `ComboBox`

示例：

```rust
#[test]
fn dialog_select_contract_exposes_modal_local_popup_primitives() {
    let source = fs::read_to_string("ui/components/modal-chrome.slint").unwrap();

    assert!(source.contains("export component DialogSelectField inherits Rectangle {"));
    assert!(source.contains("export component DialogSelectPopup inherits Rectangle {"));
    assert!(source.contains("callback option-selected(string);"));
    assert!(source.contains("in property <length> popup-max-height"));
}
```

同步更新 `tests/assets_modal_ui_contract_smoke.sh`，要求 shared modal chrome 已经暴露 select primitive，而不是继续依赖 `ComboBox` contract。

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- FAIL，因为 shared select primitive 还不存在。

**Step 3: Write minimal implementation**

在 `ui/components/modal-chrome.slint` 先落共享 primitive，不接具体业务：

```slint
export component DialogSelectField inherits Rectangle {
    in property <string> label: "";
    in property <string> selected-label: "";
    in property <bool> open: false;
    callback toggle-requested();
}

export component DialogSelectPopup inherits Rectangle {
    in property <[string]> options: [];
    in property <int> highlighted-index: -1;
    in property <length> popup-max-height: 240px;
    callback option-selected(string);
    callback dismiss-requested();
}
```

实现注意：
- shared primitive 只负责视觉和最小 keyboard/click contract
- 先不要在这一步把 SSH/snippet consumer 一起迁掉
- 不要在 shared primitive 内部偷偷再包一个 `ComboBox`

**Step 4: Run test to verify it passes**

Run:

```bash
cargo test --test modal_input_select_contract_spec -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- PASS，shared select primitive 已经被冻结。

**Step 5: Commit**

```bash
git add ui/components/modal-chrome.slint tests/modal_input_select_contract_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "test: freeze modal select primitive contract"
```

### Task 6: 迁移 SSH modal 的四个 select 到 modal-local overlay

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/components/modal-chrome.slint`
- Modify: `tests/modal_input_select_contract_spec.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

补 SSH select migration 的 targeted tests，至少覆盖：
- source contract：SSH modal 不再包含 `ComboBox {`
- `auth_source`、`keychain_identity_label`、`proxy_type`、`proxy_ssh_asset_label` 仍通过原 field id 发出 `draft-changed`
- software renderer smoke：在 short viewport 里打开底部 select 后，footer 上方会出现新的 overlay pixels，而不是被底部裁掉

示例 source contract：

```rust
#[test]
fn ssh_modal_contract_uses_dialog_select_field_instead_of_combobox() {
    let source = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint").unwrap();
    assert!(source.contains("DialogSelectField"));
    assert!(!source.contains("ComboBox {"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test modal_input_select_contract_spec --test assets_modal_smoke -q
cargo test --features slint-renderer-software --test assets_modal_render_spec ssh_modal_short_viewport_keeps_primary_auth_field_actionable -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- FAIL，因为 SSH modal 仍在用 `ComboBox`，底部 select overlay 还没接入。

**Step 3: Write minimal implementation**

在 `ui/components/assets-ssh-connection-modal.slint` 中做 modal-local overlay：

```slint
private property <string> open-select-id: "";
private property <length> select-popup-x: 0px;
private property <length> select-popup-y: 0px;
private property <length> select-popup-width: 0px;
private property <bool> select-popup-opens-upward: false;

source-select := DialogSelectField {
    selected-label: root.auth-source == "keychain-identity" ? "Keychain Identity" : "Manual";
    toggle-requested => {
        root.open-select("auth-source", self.absolute-position.x, self.absolute-position.y, self.width);
    }
}
```

同一个 modal root 内新增 popup sibling：

```slint
if root.open-select-id != "" : popup := DialogSelectPopup {
    x: root.select-popup-x;
    y: root.select-popup-y;
    width: root.select-popup-width;
}
```

要求：
- popup 是 `body-scroll` 与 `footer` 的 sibling，不是 `ScrollView` children
- 向上/向下展开逻辑由 modal 本地属性控制
- 点击外部关闭，`Esc` 关闭，关闭后 focus 回到 trigger
- 选中后仍走既有 `draft-changed(...)`

覆盖四个 SSH select：
- Source
- Identity
- Proxy type
- Upstream SSH connection

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test modal_input_select_contract_spec --test assets_modal_smoke -q
cargo test --features slint-renderer-software --test assets_modal_render_spec ssh_modal_short_viewport_keeps_primary_auth_field_actionable -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- PASS，SSH modal 不再依赖 `ComboBox`，底部 select contract 已迁到 modal-local overlay。

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/components/modal-chrome.slint tests/modal_input_select_contract_spec.rs tests/assets_modal_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: move ssh modal selects to local overlay"
```

### Task 7: 迁移 snippet package select 并做最终验证

**Files:**
- Modify: `ui/components/assets-snippet-modal.slint`
- Modify: `tests/modal_input_select_contract_spec.rs`
- Modify: `tests/assets_modal_smoke.rs`
- Modify: `tests/assets_modal_render_spec.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Test only: `tests/keychain_ui_contract_smoke.sh`
- Test only: `tests/sync_vault_modal_smoke.rs`

**Step 1: Write the failing tests**

把 snippet 也纳入最终 contract：
- `ui/components/assets-snippet-modal.slint` 不再 import/use `ComboBox`
- package picker 仍按原 contract 发出 `draft-changed("package", value)`，并保留 `No Package -> ""` 的映射
- source contract 里确认“所有编辑 modal”范围内不再有 `ComboBox` 残留（至少 SSH + snippet）

示例：

```rust
#[test]
fn snippet_modal_contract_uses_dialog_select_field_for_package_picker() {
    let source = fs::read_to_string("ui/components/assets-snippet-modal.slint").unwrap();
    assert!(source.contains("DialogSelectField"));
    assert!(!source.contains("ComboBox {"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test modal_input_select_contract_spec --test assets_modal_smoke -q
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:
- FAIL，因为 snippet modal 仍然在用 stock `ComboBox`。

**Step 3: Write minimal implementation**

把 snippet package picker 切到 shared select primitive：

```slint
package-select := DialogSelectField {
    label: "Package";
    selected-label: root.package-selected-label;
    toggle-requested => { root.open-package-select(); }
}

package-popup := DialogSelectPopup {
    options: root.package-options;
    option-selected(value) => {
        root.draft-changed("package", value == "No Package" ? "" : value);
    }
}
```

然后执行完整验证：
- 清理 SSH/snippet 中最后的 `ComboBox` import
- 确认 shared select primitive 没有破坏键盘 Tab / Esc / Enter / 上下键 contract
- 确认 snippet package 仍保持既有 label/value 映射，不额外改 Rust binder

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo check --quiet
cargo test --test modal_input_select_contract_spec --test assets_modal_smoke --test keychain_modal_smoke --test keychain_identity_actions_spec --test sync_vault_modal_smoke -q
cargo test --features slint-renderer-software --test assets_modal_render_spec --quiet
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/keychain_ui_contract_smoke.sh
```

On a desktop session, also run:

```bash
cargo run
```

Expected:
- CLI verification 全绿
- GUI 手工烟测满足以下 checklist：
  - SSH `Name` / `Host` / `Port` 点击文本中间时，caret 落点正确
  - SSH 与 keychain/sync 的 secret eye icon 可以点一次显示、再点一次隐藏
  - 鼠标可拖选已有文本
  - SSH 底部 `Proxy type` / `Upstream SSH connection` 下拉不再被 footer 裁切
  - snippet `Package` picker 在 modal 底部附近展开时完整可见或可滚动
  - 关闭 dropdown 后焦点回到触发 field

**Step 5: Commit**

```bash
git add ui/components/assets-snippet-modal.slint tests/modal_input_select_contract_spec.rs tests/assets_modal_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: harden modal selects and secret toggles"
```
