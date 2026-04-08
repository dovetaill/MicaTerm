# Terminal Settings + Active Idle Shrink Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the broken top-left `Settings` entry so it opens a real settings modal, make terminal scrollback globally configurable with a default of `1500`, add active-window idle transient-cache shrink, and defer deeper Slint/Skia/DXGI purge work into `todo-0206-0408.md`.

**Architecture:** Keep the existing `AppWindow -> bootstrap -> ShellViewModel -> SessionManager / SessionRuntime` pipeline, but add a lightweight settings modal state alongside the existing sync modal, extend `UiPreferences` with terminal-facing settings, and introduce a shared terminal runtime defaults object so newly launched SSH sessions pick up the configured scrollback limit. Reuse the existing presenter `clear_transient_caches()` path for active-idle shrink and explicitly avoid dropping the terminal host while a surface is still visible.

**Tech Stack:** Rust, Slint, existing blocking modal shell pattern, SSH session runtime, cargo tests, shell contract smoke tests.

---

### Task 1: Lock the settings modal contract and remove the wrong SFTP routing

**Files:**
- Create: `ui/components/settings-modal.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`

**Step 1: Write the failing smoke test**

In `tests/vault_settings_smoke.rs`, add a test that:

```rust
#[test]
fn settings_action_opens_settings_modal_without_touching_sftp_panel() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_settings_modal_open());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_open_settings_panel_requested();

    assert!(app.get_settings_modal_open());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");
}
```

**Step 2: Add the failing UI contract**

In `tests/top_status_bar_ui_contract_smoke.sh`, add greps requiring:

- `in-out property <bool> settings-modal-open: false;`
- `callback settings-modal-close-requested();`
- `if root.settings-modal-open : settings-modal-shell := BlockingModalShell {`
- `settings-modal-overlay := SettingsModal {`

Also add negative assertions that `open-settings-panel-requested` no longer changes right-panel settings behavior indirectly.

**Step 3: Run the targeted tests to verify RED**

Run:

```bash
cargo test --test vault_settings_smoke settings_action_opens_settings_modal_without_touching_sftp_panel -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected:

- Rust smoke fails because `settings-modal-open` contract does not exist yet.
- Shell contract smoke fails because `SettingsModal` is not mounted yet.

**Step 4: Implement the minimal modal contract**

Make the smallest change set that gets the contract green:

- Add `SettingsModal` component in `ui/components/settings-modal.slint`.
- Add `settings-modal-open` and related callback/properties to `ui/app-window.slint`.
- Add a `SettingsModalViewState` (or equivalent fields) to `ShellViewModel`.
- Change `open_settings_panel()` so it opens the settings modal and closes the global menu.
- Update `window.on_open_settings_panel_requested(...)` in `src/app/bootstrap/shell_chrome.rs` so it no longer routes to `open_sftp_panel()`.

**Step 5: Re-run the targeted tests to verify GREEN**

Run:

```bash
cargo test --test vault_settings_smoke settings_action_opens_settings_modal_without_touching_sftp_panel -- --exact
cargo test --test top_status_bar_smoke settings_no_longer_routes_into_vault_flow -- --exact
bash tests/top_status_bar_ui_contract_smoke.sh
```

Expected: all pass.

**Step 6: Commit**

```bash
git add ui/components/settings-modal.slint ui/app-window.slint src/shell/view_model.rs src/shell/view_model/projection.rs src/app/bootstrap/shell_chrome.rs tests/vault_settings_smoke.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh
git commit -m "feat: open a real settings modal from the titlebar"
```

### Task 2: Persist terminal settings in `UiPreferences` and project them into the modal

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/projection.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Test: `tests/ui_preferences.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/vault_settings_smoke.rs`

**Step 1: Write the failing persistence tests**

In `tests/ui_preferences.rs`, add assertions for:

```rust
#[test]
fn ui_preferences_default_terminal_settings_match_memory_plan() {
    let prefs = UiPreferences::default();
    assert_eq!(prefs.terminal_scrollback_limit, 1500);
    assert!(prefs.terminal_active_idle_shrink_enabled);
}

#[test]
fn ui_preferences_roundtrip_terminal_settings() {
    let prefs = UiPreferences {
        terminal_scrollback_limit: 3000,
        terminal_active_idle_shrink_enabled: false,
        ..UiPreferences::default()
    };
    // save + load + assert_eq!
}
```

In `tests/top_status_bar_smoke.rs` or `tests/vault_settings_smoke.rs`, add a smoke test that opening settings exposes the persisted values on `AppWindow`.

**Step 2: Run the targeted tests to verify RED**

Run:

```bash
cargo test --test ui_preferences ui_preferences_default_terminal_settings_match_memory_plan -- --exact
cargo test --test ui_preferences ui_preferences_roundtrip_terminal_settings -- --exact
```

Expected: both fail because the fields do not exist yet.

**Step 3: Implement minimal persistence and projection**

Add:

- `terminal_scrollback_limit: usize`
- `terminal_active_idle_shrink_enabled: bool`

to `UiPreferences`, `Default`, `From<&ShellViewModel>`, and snapshot migration defaults.

Project them into `ShellViewModel` and `AppWindow` so the settings modal can read/write them. The value-change callbacks should immediately update view-model state and call `save_ui_preferences(...)`.

**Step 4: Re-run the targeted tests to verify GREEN**

Run:

```bash
cargo test --test ui_preferences -- --nocapture
cargo test --test vault_settings_smoke -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
```

Expected: all targeted preference and modal projection tests pass.

**Step 5: Commit**

```bash
git add src/app/ui_preferences.rs src/shell/view_model.rs src/shell/view_model/projection.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs tests/ui_preferences.rs tests/top_status_bar_smoke.rs tests/vault_settings_smoke.rs
git commit -m "feat: persist terminal settings in ui preferences"
```

### Task 3: Thread scrollback settings into new terminal session creation

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/wezterm_adapter.rs`
- Modify: `src/app/terminal_core/alacritty_adapter.rs`
- Test: `tests/terminal_session_spec.rs`

**Step 1: Write the failing scrollback test**

In `tests/terminal_session_spec.rs`, replace the hard-coded expectation with a configurable constructor path:

```rust
#[test]
fn terminal_session_uses_configured_scrollback_limit_for_large_bursts() {
    let configured_scrollback_lines = 1500usize;
    let mut session = TerminalSession::new_with_scrollback(4, 20, configured_scrollback_lines);
    // existing burst logic...
}
```

Add a second test proving that a larger configured value retains more history than `1500`.

**Step 2: Run the targeted tests to verify RED**

Run:

```bash
cargo test --test terminal_session_spec terminal_session_uses_configured_scrollback_limit_for_large_bursts -- --exact
```

Expected: fail because no configurable constructor exists yet.

**Step 3: Implement the runtime defaults plumbing**

Implement the narrowest possible chain:

- Introduce a shared terminal defaults object in bootstrap that `LiveSessionRuntimeLauncher` can clone.
- Extend `SshSessionRuntime::connect_with_credential_store(...)` to accept terminal defaults.
- Extend `TerminalSession` / `create_terminal_core_adapter(...)` / adapter constructors to accept `scrollback_limit`.
- Change default scrollback from `3500` to `1500`.
- Ensure only newly launched sessions consume updated values.

**Step 4: Re-run the targeted tests to verify GREEN**

Run:

```bash
cargo test --test terminal_session_spec -- --nocapture
```

Expected: terminal scrollback tests pass with the configurable limit, and the default-path expectations now reflect `1500`.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/ssh/runtime.rs src/app/ssh/runtime/terminal.rs src/app/terminal_core/mod.rs src/app/terminal_core/wezterm_adapter.rs src/app/terminal_core/alacritty_adapter.rs tests/terminal_session_spec.rs
git commit -m "feat: make terminal scrollback configurable"
```

### Task 4: Add active-window idle transient-cache shrink

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing unit tests**

In `src/app/bootstrap.rs` tests, add focused cases covering:

- active surface + stable `seqno`/viewport + threshold elapsed => caches are cleared but host remains retained
- active surface + surface changes before threshold => shrink does not fire
- preference disabled => active idle shrink never fires

Prefer tests shaped like:

```rust
#[test]
fn active_idle_shrink_clears_transient_caches_without_releasing_host() {
    // seed WORKSPACE_TERMINAL_RENDERER_HOST with a presenter
    // construct a stable visible surface signature/state
    // advance time past WORKSPACE_TERMINAL_ACTIVE_IDLE_CACHE_SHRINK_MS
    // assert host still exists after shrink
}
```

**Step 2: Run the targeted tests to verify RED**

Run:

```bash
cargo test --lib active_idle_shrink -- --nocapture
```

Expected: new active-idle tests fail because the path does not exist yet.

**Step 3: Implement minimal active-idle tracking**

Add a second idle tracker beside the existing no-surface path:

- track last active-surface `seqno`
- track last active-surface `viewport_offset_lines`
- track when the active visible surface became idle
- gate on `terminal_active_idle_shrink_enabled`
- after threshold, call only `clear_workspace_terminal_transient_caches(...)`
- do **not** call `release_workspace_terminal_renderer_resources()`

Keep the existing no-surface path unchanged.

**Step 4: Re-run the targeted tests to verify GREEN**

Run:

```bash
cargo test --lib active_idle_shrink -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: all active-idle unit tests pass and broader bootstrap smoke stays green.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "fix: shrink active terminal caches after idle"
```

### Task 5: Record deferred backend purge work in `todo-0206-0408.md`

**Files:**
- Create: `todo-0206-0408.md`

**Step 1: Write the pending-work checklist**

Create `todo-0206-0408.md` with short sections for:

- `SkiaRenderer layer_cache` lifecycle cleanup
- minimized / hidden / occluded `suspend()` integration
- `SkGraphics::PurgeAllCaches()`
- `GrDirectContext::performDeferredCleanup()` and related cleanup APIs
- `IDXGIDevice3::Trim()`

For each item, note why it is deferred and the likely insertion point.

**Step 2: Verify the file contents**

Run:

```bash
grep -F 'SkGraphics::PurgeAllCaches()' todo-0206-0408.md
grep -F 'GrDirectContext::performDeferredCleanup()' todo-0206-0408.md
grep -F 'IDXGIDevice3::Trim()' todo-0206-0408.md
```

Expected: all three lines are present.

**Step 3: Commit**

```bash
git add todo-0206-0408.md
git commit -m "docs: capture deferred backend purge follow-ups"
```

### Task 6: Run the focused verification suite

**Files:**
- Test: `tests/ui_preferences.rs`
- Test: `tests/vault_settings_smoke.rs`
- Test: `tests/top_status_bar_smoke.rs`
- Test: `tests/top_status_bar_ui_contract_smoke.sh`
- Test: `tests/terminal_session_spec.rs`
- Test: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Run the final suite**

Run:

```bash
cargo test --test ui_preferences -- --nocapture
cargo test --test vault_settings_smoke -- --nocapture
cargo test --test top_status_bar_smoke -- --nocapture
bash tests/top_status_bar_ui_contract_smoke.sh
cargo test --test terminal_session_spec -- --nocapture
cargo test --lib active_idle_shrink -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: all pass.

**Step 2: Commit any remaining test-only adjustments**

```bash
git add tests/ui_preferences.rs tests/vault_settings_smoke.rs tests/top_status_bar_smoke.rs tests/top_status_bar_ui_contract_smoke.sh tests/terminal_session_spec.rs src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "test: verify terminal settings and idle shrink behavior"
```
