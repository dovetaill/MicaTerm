# SSH Create / Connect / Tabs Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Deliver the confirmed SSH refactor: single-page SSH modal with directly viewable/editable saved passwords, real terminal surface rendering, stable keepalive/reconnect behavior, and reliable saved-secret reuse across reconnects and app restarts.

**Architecture:** Keep the existing `ConnectionProfile -> SessionManager -> SshSessionRuntime -> ShellViewModel -> Slint` chain and strengthen it in-place. First lock the modal and secret contracts with failing tests, then replace the `visible_lines` placeholder projection with a richer terminal surface model, then finish reconnect/keepalive behavior and regression coverage.

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`, `keyring`, `cargo test`, shell smoke scripts

---

## Ground Rules

- Do not add SFTP, proxy runtime, pane splitting, or asset persistence redesign.
- Keep the modal footer at `Test` + `Save` only.
- Keep the visual direction flat / no-radius.
- Prefer extending existing files over introducing new modules.
- Follow TDD per task: write the failing test first, run it, implement the smallest passing change, rerun, then commit.

### Task 1: Lock the SSH modal contract to the new single-page grouped form

**Files:**
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Test: `tests/shell_view_model.rs`
- Test: `tests/assets_modal_smoke.rs`
- Test: `tests/assets_context_menu_smoke.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Add or replace assertions so the test suite expects:

```rust
#[test]
fn new_ssh_modal_is_a_grouped_single_page_form() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint").unwrap();
    assert!(!ssh_modal.contains("\"Standard\""));
    assert!(!ssh_modal.contains("\"Proxy\""));
    assert!(!ssh_modal.contains("\"Environment\""));
    assert!(!ssh_modal.contains("\"Advanced\""));
    assert!(!ssh_modal.contains("Clear Saved Secret"));
    assert!(ssh_modal.contains("label: \"Password\""));
    assert!(ssh_modal.contains("trailing-action-text: root.password-visible ? \"Hide\" : \"Show\""));
}
```

Replace the old secret-retention round-trip test with a grouped-form contract test:

```rust
#[test]
fn ssh_modal_round_trips_password_visibility_without_secret_retention_flags() {
    let app = AppWindow::new().unwrap();
    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_password("secret".into());
    app.set_asset_ssh_modal_password_visible(false);
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test shell_view_model new_ssh_modal_is_a_grouped_single_page_form -- --exact
cargo test --test assets_modal_smoke ssh_modal_round_trips_password_visibility_without_secret_retention_flags -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- Rust tests fail because the modal still contains `Standard / Proxy / Environment / Advanced` and secret-retention fields.
- The shell smoke test fails because it still greps for the old tab strings and legacy secret-retention properties.

**Step 3: Write minimal implementation**

Implement the grouped form directly in the existing modal:

```rust
// src/shell/view_model.rs
pub struct AssetSshConnectionDraft {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    pub private_key_source: String,
    pub password: String,
    pub private_key_content: String,
    pub private_key_path: String,
    pub passphrase: String,
    pub password_visible: bool,
    pub remark: String,
    pub environment: String,
    pub proxy_method: String,
    pub validation_message: String,
}
```

```slint
// ui/components/assets-ssh-connection-modal.slint
// Remove top-level tab chrome.
// Keep one scrollable body with grouped sections:
// 1. Basic
// 2. Authentication
// 3. Notes
// Proxy / Environment remain as secondary grouped fields, not first-level tabs.
```

Also remove legacy window properties that only exist for secret-retention / clear-secret control.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test shell_view_model -- --nocapture
cargo test --test assets_modal_smoke -- --nocapture
cargo test --test assets_context_menu_smoke ssh_modal_cancel_and_reopen_resets_tab_and_draft_fields -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- No test references `active_tab`, `secret_retention_message`, or `Clear Saved Secret`.
- The modal smoke and contract tests pass with the grouped single-page structure.

**Step 5: Commit**

```bash
git add ui/components/assets-ssh-connection-modal.slint ui/app-window.slint src/shell/view_model.rs tests/shell_view_model.rs tests/assets_modal_smoke.rs tests/assets_context_menu_smoke.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "feat: collapse ssh modal into grouped single-page form"
```

### Task 2: Hydrate saved secrets into the edit modal and make diagnostics explicit

**Files:**
- Modify: `src/app/ssh/credentials.rs`
- Modify: `src/app/ssh/profile.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Test: `tests/credential_store_spec.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/shell_view_model.rs`

**Step 1: Write the failing tests**

Add a direct edit-hydration test:

```rust
#[test]
fn editing_saved_password_modal_hydrates_real_secret_masked() {
    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-prod",
        &StoredSshSecretBundle { password: Some("secret".into()), private_key_content: None, passphrase: None },
    ).unwrap();
    bind_with_launcher_and_credential_store(&app, Some(saved_repo()), Arc::new(FakeLauncher), credential_store);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());
}
```

Add a restart-style saved-secret reuse test:

```rust
#[test]
fn saved_password_asset_reconnects_after_rebinding_with_same_store() {
    let shared_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    // first bind saves
    // second bind opens same saved asset
    // assert probe sees saved secret instead of "missing SSH password secret"
}
```

Add diagnostic specificity tests:

```rust
#[test]
fn missing_saved_secret_reports_missing_keyring_entry() { /* expect a specific message */ }

#[test]
fn missing_saved_secret_binding_reports_missing_credential_ref() { /* expect a different message */ }
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test credential_store_spec -- --nocapture
cargo test --test shell_view_model editing_saved_password_modal_hydrates_real_secret_masked -- --exact
cargo test --test bootstrap_smoke saved_password_asset_reconnects_after_rebinding_with_same_store -- --exact
```

Expected:

- The edit modal test fails because `open_edit_ssh_modal()` still initializes password fields as empty.
- The reconnect test fails because the second bind still falls back to the generic `missing SSH password secret`.

**Step 3: Write minimal implementation**

Add one explicit secret-loading path and typed diagnostics instead of generic fallback:

```rust
// src/app/ssh/credentials.rs
pub enum StoredSecretLookupError {
    MissingCredentialRef,
    MissingEntry { credential_ref: String },
    ReadFailed { credential_ref: String, source: anyhow::Error },
    EmptyBundleField { credential_ref: String, field: &'static str },
}
```

```rust
// src/shell/view_model.rs
pub fn hydrate_edit_ssh_modal_secret(
    &mut self,
    password: Option<String>,
    private_key_content: Option<String>,
    passphrase: Option<String>,
    inline_error: Option<String>,
) {
    // update the active SSH draft in-place after bootstrap loads the secret
}
```

```rust
// src/app/bootstrap.rs
// after state.open_edit_ssh_modal(asset_id):
// 1. resolve credential_ref from saved spec
// 2. load bundle from CredentialStore
// 3. call state.hydrate_edit_ssh_modal_secret(...)
// 4. sync modal state back into window
```

Also update `authenticate_client()` so each failure path emits a specific, user-facing diagnostic instead of collapsing everything into `missing SSH password secret`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test credential_store_spec -- --nocapture
cargo test --test shell_view_model -- --nocapture
cargo test --test bootstrap_smoke editing_legacy_saved_ssh_asset_reuses_fallback_saved_secret_for_test_connection -- --exact
cargo test --test bootstrap_smoke saved_password_asset_reconnects_after_rebinding_with_same_store -- --exact
```

Expected:

- Edit modal opens with the real saved secret masked.
- Saving, rebinding, and reconnecting with the same store succeeds.
- Diagnostic tests distinguish missing binding vs missing keyring entry vs empty saved bundle field.

**Step 5: Commit**

```bash
git add src/app/ssh/credentials.rs src/app/ssh/profile.rs src/app/ssh/runtime.rs src/app/bootstrap.rs src/shell/view_model.rs tests/credential_store_spec.rs tests/bootstrap_smoke.rs tests/shell_view_model.rs
git commit -m "fix: hydrate saved ssh secrets and report explicit lookup errors"
```

### Task 3: Remove dead modal actions and keep asset-driven connect as the only open path

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/components/assets-ssh-connection-modal.slint`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/assets_modal_render_spec.rs`
- Test: `tests/assets_modal_ui_contract_smoke.sh`

**Step 1: Write the failing tests**

Replace the old `save-and-connect` expectations with a strict two-action contract:

```rust
#[test]
fn ssh_modal_action_enum_only_supports_test_and_save() {
    let content = fs::read_to_string("src/shell/view_model.rs").unwrap();
    assert!(content.contains("SshModalAction::Save"));
    assert!(content.contains("SshModalAction::TestConnection"));
    assert!(!content.contains("SshModalAction::Connect"));
    assert!(!content.contains("SshModalAction::SaveAndConnect"));
}
```

Add a render assertion that only two footer buttons remain:

```rust
#[test]
fn new_ssh_modal_renders_only_test_and_save_actions() {
    // render modal and assert two visible action zones, not four
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke ssh_modal_action_enum_only_supports_test_and_save -- --exact
cargo test --test assets_modal_render_spec new_ssh_modal_renders_only_test_and_save_actions -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- The enum/content test fails because `Connect` and `SaveAndConnect` still exist.
- The shell contract still passes the old property name `connect-family-enabled`.

**Step 3: Write minimal implementation**

Remove dead actions and rename stale semantics:

```rust
// src/shell/view_model.rs
pub enum SshModalAction {
    Save,
    TestConnection,
}
```

```slint
// ui/app-window.slint
in-out property <bool> asset-ssh-modal-test-enabled: false;
```

```rust
// src/app/bootstrap.rs
window.on_asset_ssh_modal_action_requested(move |action| {
    match action.as_str() {
        "save" => { /* persist asset + secret */ }
        "test" => { /* probe only */ }
        other => state.finish_ssh_modal_action_error(format!("Unsupported SSH modal action: {other}")),
    }
});
```

Keep connection entry outside the modal:

- asset activation
- context menu `open-connection`
- context menu `open-in-new-tab`

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test assets_modal_render_spec new_ssh_modal_renders_only_test_and_save_actions -- --exact
bash tests/assets_modal_ui_contract_smoke.sh
```

Expected:

- No code path references modal `connect` or `save-and-connect`.
- Save still persists the asset without opening a tab.
- Test still probes without opening a tab.

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/components/assets-ssh-connection-modal.slint tests/bootstrap_smoke.rs tests/assets_modal_render_spec.rs tests/assets_modal_ui_contract_smoke.sh
git commit -m "refactor: reduce ssh modal actions to test and save"
```

### Task 4: Replace the `visible_lines` placeholder projection with a real terminal surface snapshot

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Test: `tests/terminal_session_spec.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Replace `visible_lines`-only expectations with a richer surface contract:

```rust
#[test]
fn terminal_runtime_snapshot_exposes_cells_and_cursor_state() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(b"\x1b[31mred\x1b[0m\r\n");
    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(snapshot.cells.iter().any(|cell| cell.text == "r" && cell.fg != cell.default_fg));
    assert!(snapshot.cursor.visible);
}
```

Add a projection test:

```rust
#[test]
fn connected_session_projects_terminal_cells_without_placeholder_copy() {
    let terminal_host = fs::read_to_string("ui/shell/terminal-session-host.slint").unwrap();
    assert!(!terminal_host.contains("Interactive terminal ready."));
    assert!(!terminal_host.contains("session-title"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec terminal_runtime_snapshot_exposes_cells_and_cursor_state -- --exact
cargo test --test workspace_tabs_spec connected_session_projects_terminal_cells_without_placeholder_copy -- --exact
```

Expected:

- The runtime snapshot test fails because `TerminalSurfaceState` still only exposes `visible_lines`.
- The workspace projection test fails because `terminal-session-host.slint` still references placeholder copy and title/subtitle fields.

**Step 3: Write minimal implementation**

Introduce explicit surface structs in `src/app/ssh/runtime.rs`:

```rust
pub struct TerminalCellState {
    pub row: u32,
    pub col: u32,
    pub text: String,
    pub fg_rgba: u32,
    pub bg_rgba: u32,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

pub struct TerminalCursorState {
    pub row: u32,
    pub col: u32,
    pub visible: bool,
    pub shape: String,
}

pub struct TerminalSurfaceState {
    pub session_id: Uuid,
    pub seqno: usize,
    pub rows: u32,
    pub cols: u32,
    pub cells: Vec<TerminalCellState>,
    pub cursor: TerminalCursorState,
}
```

Then update:

- `TerminalSession::surface_state()` to walk the visible grid and emit flattened cells.
- `ShellViewModel` accessors to expose terminal cells instead of `visible_lines`.
- `bootstrap.rs` window sync to push `workspace-session-cells`, `workspace-session-cursor-row`, `workspace-session-cursor-col`, and `workspace-session-cursor-visible`.

Do not add selection, clipboard, or hyperlinks yet.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_session_spec -- --nocapture
cargo test --test workspace_tabs_spec -- --nocapture
cargo test --test bootstrap_smoke runtime_events_refresh_workspace_terminal_projection_after_opening_saved_asset -- --exact
```

Expected:

- Terminal snapshot now exposes cells and cursor state.
- Projection tests stop depending on `visible_lines`.
- Async projection smoke still passes through the active workspace session.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/shell/workspace-pane.slint tests/terminal_session_spec.rs tests/workspace_tabs_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: project ssh terminal surfaces as cells and cursor state"
```

### Task 5: Render a real terminal canvas in Slint and remove placeholder chrome

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Test: `tests/assets_modal_render_spec.rs`

**Step 1: Write the failing tests**

Add terminal-host contract assertions:

```rust
#[test]
fn terminal_session_host_renders_cell_canvas_without_title_subtitle_copy() {
    let terminal_host = fs::read_to_string("ui/shell/terminal-session-host.slint").unwrap();
    assert!(!terminal_host.contains("session-title"));
    assert!(!terminal_host.contains("session-subtitle"));
    assert!(!terminal_host.contains("Interactive terminal ready."));
    assert!(terminal_host.contains("for cell in root.session-cells"));
}
```

Update the UI contract smoke script so it expects:

- cell model properties
- cursor properties
- no placeholder reconnect sentence

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec terminal_session_host_renders_cell_canvas_without_title_subtitle_copy -- --exact
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- Rust test fails because the host still renders title/subtitle/status blocks.
- The shell smoke test fails because `terminal-session-host.slint` still contains the placeholder reconnect sentence.

**Step 3: Write minimal implementation**

Update `ui/shell/terminal-session-host.slint` so terminal mode contains only:

```slint
terminal-canvas := Rectangle {
    background: ThemeTokens.dark-mode ? #0d1218 : #f4f7fb;
    for cell in root.session-cells : Text {
        x: root.padding-left + (cell.col * root.terminal-cell-width);
        y: root.padding-top + (cell.row * root.terminal-cell-height);
        text: cell.text;
        color: cell.fg;
    }
    if root.session-cursor-visible : Rectangle {
        x: root.padding-left + (root.session-cursor-col * root.terminal-cell-width);
        y: root.padding-top + (root.session-cursor-row * root.terminal-cell-height);
        width: root.terminal-cell-width;
        height: root.terminal-cell-height;
    }
}
```

Visual defaults for the first pass:

- font size: `14px`
- cell width: based on the chosen monospace font, start with `9px`
- cell height: start with `20px`
- cursor: solid block with contrasting fill and text inversion

Keep `session-error` as a separate state, but remove the fake reconnect placeholder line.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test workspace_tabs_spec -- --nocapture
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
cargo test --test assets_modal_render_spec -- --nocapture
```

Expected:

- Terminal host contract tests pass without placeholder copy.
- SSH UI contract smoke confirms the new host properties are wired.
- No modal render regressions are introduced by the workspace-pane property changes.

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint tests/workspace_tabs_spec.rs tests/ssh_connect_tabs_ui_contract_smoke.sh tests/assets_modal_render_spec.rs
git commit -m "feat: render ssh terminal surface with cells and cursor"
```

### Task 6: Fix keepalive, disconnect, and reconnect lifecycle around a reused asset tab

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/tabs.rs`
- Test: `tests/ssh_session_manager_spec.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing tests**

Add a config helper test:

```rust
#[test]
fn ssh_client_config_uses_keepalive_instead_of_inactivity_timeout() {
    let config = build_ssh_client_config();
    assert_eq!(config.inactivity_timeout, None);
    assert!(config.keepalive_interval.is_some());
}
```

Add reconnect reuse tests:

```rust
#[test]
fn reconnect_reuses_disconnected_asset_tab_instead_of_opening_duplicate() { /* activate asset twice, expect one tab */ }

#[test]
fn disconnected_asset_remains_editable_while_tab_is_reconnectable() { /* edit modal still opens */ }
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test ssh_session_manager_spec reconnect_reuses_disconnected_asset_tab_instead_of_opening_duplicate -- --exact
cargo test --test bootstrap_smoke disconnected_asset_remains_editable_while_tab_is_reconnectable -- --exact
```

Expected:

- The config helper test fails because no helper exists and `inactivity_timeout` is still set to 30 seconds.
- Reconnect behavior still relies on partial lifecycle wiring and may open stale tabs or block edit flow.

**Step 3: Write minimal implementation**

Make runtime config explicit and testable:

```rust
pub fn build_ssh_client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(15)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    })
}
```

Then:

- use `build_ssh_client_config()` in `SshSessionRuntime::connect()`
- ensure `disconnect_session()` leaves the tab in `Disconnected`
- when asset activation targets an existing disconnected/error tab, trigger reconnect on that tab instead of creating a new duplicate
- keep `open_edit_ssh_modal()` reachable from context-menu or selection even while the tab is disconnected

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test bootstrap_smoke close_connection_context_action_tracks_live_workspace_session_state -- --exact
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test workspace_tabs_spec -- --nocapture
```

Expected:

- No test depends on `Reconnect is available once session lifecycle wiring is completed.`
- Disconnect keeps one reconnectable tab.
- Reopening the same asset reuses the existing disconnected tab.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs src/shell/tabs.rs tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "fix: reuse disconnected ssh tabs and add keepalive config"
```

### Task 7: Run the full SSH regression matrix and close the gap between unit, smoke, and UI-contract tests

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/credential_store_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/assets_modal_ui_contract_smoke.sh`
- Modify: `tests/ssh_connect_tabs_ui_contract_smoke.sh`
- Modify: `docs/plans/2026-03-25-ssh-create-connect-tabs-design.md` only if any implementation detail forces a documented correction

**Step 1: Write the failing regression tests**

Add final high-value assertions that mirror the user-reported failures:

```rust
#[test]
fn saved_password_asset_can_reopen_after_restart_without_missing_secret_error() { /* no generic error */ }

#[test]
fn terminal_error_view_does_not_render_placeholder_reconnect_copy() { /* grep slint */ }

#[test]
fn editing_saved_password_modal_supports_show_hide_and_overwrite() { /* open edit, show, change, save, reopen */ }
```

**Step 2: Run the targeted regression tests**

Run:

```bash
cargo test --test credential_store_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test terminal_session_spec -- --nocapture
cargo test --test ssh_session_manager_spec -- --nocapture
cargo test --test workspace_tabs_spec -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- At least one regression test initially fails if any old placeholder string, old action, or old secret path remains.

**Step 3: Apply the final cleanup pass**

Cleanup targets:

- delete dead helper methods and fields that only existed for `Connect` / `SaveAndConnect`
- delete dead secret-retention fields and reset paths
- remove `visible_lines`-specific accessors from `ShellViewModel`
- remove placeholder copy from `terminal-session-host.slint`
- delete tests that only validated the removed legacy contract

**Step 4: Run the complete verification sweep**

Run:

```bash
cargo test --test credential_store_spec --test bootstrap_smoke --test terminal_session_spec --test ssh_session_manager_spec --test workspace_tabs_spec -- --nocapture
bash tests/assets_modal_ui_contract_smoke.sh
bash tests/ssh_connect_tabs_ui_contract_smoke.sh
```

Expected:

- Full SSH-focused regression suite passes.
- No source file still contains `Clear Saved Secret`, `save-and-connect`, or `Interactive terminal ready.`.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/credential_store_spec.rs tests/workspace_tabs_spec.rs tests/assets_modal_ui_contract_smoke.sh tests/ssh_connect_tabs_ui_contract_smoke.sh src/app src/shell ui
git commit -m "test: close ssh modal runtime and reconnect regressions"
```

## Execution Notes

- Keep commits task-scoped and reversible.
- Do not collapse Tasks 4, 5, and 6 into one patch; they touch different failure surfaces.
- If Task 4 introduces too much Slint friction, finish the backend cell/cursor projection first, then adapt the UI in Task 5.
- If any Windows keyring behavior differs from `MemoryCredentialStore`, add one additional diagnostic-focused test helper instead of weakening the error model.

## Verification Matrix

- `cargo test --test credential_store_spec -- --nocapture`
- `cargo test --test shell_view_model -- --nocapture`
- `cargo test --test assets_modal_smoke -- --nocapture`
- `cargo test --test bootstrap_smoke -- --nocapture`
- `cargo test --test terminal_session_spec -- --nocapture`
- `cargo test --test ssh_session_manager_spec -- --nocapture`
- `cargo test --test workspace_tabs_spec -- --nocapture`
- `bash tests/assets_modal_ui_contract_smoke.sh`
- `bash tests/ssh_connect_tabs_ui_contract_smoke.sh`

