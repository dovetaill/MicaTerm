# Terminal Interaction Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Polish the SSH terminal so keyboard shortcuts, scrollback, scrollbar behavior, fonts, and ANSI color presentation feel closer to a mature Windows terminal product without rewriting the terminal core.

**Architecture:** Keep `wezterm-term` as the only terminal state engine in `src/app/ssh/runtime.rs`, extend `TerminalSurfaceState` to project scrollback metadata, and let `TerminalSessionHost` own only event capture plus pixel-space scrollbar interaction. `SessionManager` and `bootstrap` stay as the bridge between runtime state and Slint window properties so new local shortcut and scroll behaviors remain session-aware.

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`

---

### Task 1: Lock the runtime polish contract with failing tests

**Files:**
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `src/app/ssh/runtime.rs`

**Step 1: Write the failing tests**

Add runtime-focused tests for these behaviors:

```rust
#[test]
fn keyboard_input_snaps_local_scrollback_back_to_bottom() {
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(2);
    let before = session.surface_state(Uuid::new_v4());
    session.send_key_event(TerminalKeyEvent::character('a', false, false, false)).unwrap();
    let after = session.surface_state(Uuid::new_v4());
    assert!(!before.viewport_at_bottom);
    assert!(after.viewport_at_bottom);
}

#[test]
fn remote_output_snaps_scrollback_back_to_latest_view() {
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(2);
    session.apply_remote_bytes(b"7\r\n");
    let after = session.surface_state(Uuid::new_v4());
    assert!(after.viewport_at_bottom);
    assert!(after.visible_lines.iter().any(|line| line == "7"));
}

#[test]
fn surface_projection_exposes_scrollback_metadata() {
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    session.scroll_viewport_lines(3);
    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(snapshot.viewport_offset_lines > 0);
    assert!(snapshot.viewport_max_offset_lines >= snapshot.viewport_offset_lines);
}

#[test]
fn dark_theme_palette_uses_bright_default_foreground() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(b"[root@host ~]# ");
    let snapshot = session.surface_state(Uuid::new_v4());
    let prompt = snapshot.cells.iter().find(|cell| cell.col == 0).unwrap();
    assert_ne!(prompt.fg_rgba, 0xff00_0000);
}
```

**Step 2: Run the targeted tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -- --nocapture
```

Expected:

- FAIL because `TerminalSurfaceState` does not expose scrollback metadata yet;
- FAIL because local scrollback is not snapped to bottom on input/output yet;
- FAIL if the current palette still projects an unreadably dark default foreground/background combination.

**Step 3: Add only the missing public API surface**

In `src/app/ssh/runtime.rs`, extend the runtime-facing projection shape without implementing the behavior yet:

```rust
pub struct TerminalSurfaceState {
    pub viewport_offset_lines: u32,
    pub viewport_max_offset_lines: u32,
    pub viewport_at_bottom: bool,
    // existing fields...
}
```

Add any helper method signatures you will need:

```rust
impl TerminalSession {
    fn snap_viewport_to_bottom(&mut self);
}
```

**Step 4: Re-run the targeted tests until failures are behavioral**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -- --nocapture
```

Expected:

- Compile succeeds far enough that the remaining failures are now about incorrect runtime behavior.

**Step 5: Commit**

```bash
git add tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs tests/terminal_scrollback_spec.rs src/app/ssh/runtime.rs
git commit -m "test: lock terminal runtime polish contract"
```

### Task 2: Implement runtime scrollback snap-to-bottom and richer palette projection

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Re-run the targeted runtime tests**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -- --nocapture
```

Expected:

- FAIL with the runtime behaviors from Task 1 still missing.

**Step 2: Implement the minimal runtime behavior**

In `src/app/ssh/runtime.rs`:

- add scrollback metadata projection to `surface_state()`;
- clamp and project `viewport_offset_lines` as `offset`, `max`, and `at_bottom`;
- call `snap_viewport_to_bottom()` before any local key input, text input, paste, or remote output is committed;
- add explicit methods for `scroll_viewport_to_top()` and `scroll_viewport_to_bottom()` if they simplify later UI wiring;
- replace the current palette values with a higher-contrast VS Code / Windows Terminal inspired dark-light pair while preserving ANSI semantics.

Implementation sketch:

```rust
pub fn apply_remote_bytes(&mut self, bytes: &[u8]) {
    self.snap_viewport_to_bottom();
    let filtered = self.pending_remote_line_buffer.push_and_filter(bytes);
    self.keyboard_modes.observe(&filtered);
    self.terminal.advance_bytes(filtered.as_slice());
    self.clamp_viewport_offset();
}

pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>> {
    self.snap_viewport_to_bottom();
    // existing encode path...
}

fn surface_state(&self, session_id: Uuid) -> TerminalSurfaceState {
    TerminalSurfaceState {
        viewport_offset_lines: self.viewport_offset_lines as u32,
        viewport_max_offset_lines: self.max_viewport_offset_lines() as u32,
        viewport_at_bottom: self.viewport_offset_lines == 0,
        // existing fields...
    }
}
```

**Step 3: Re-run the targeted runtime tests**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec -- --nocapture
```

Expected:

- PASS

**Step 4: Run the broader SSH-related tests**

Run:

```bash
cargo test --test ssh_session_manager_spec --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS, confirming runtime changes did not break session wiring or UI contract tests.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs tests/terminal_scrollback_spec.rs
git commit -m "feat: polish terminal runtime scrollback and palette"
```

### Task 3: Add local shortcut routing and a visible scrollbar to `TerminalSessionHost`

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/theme/tokens.slint`
- Modify: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing UI contract tests**

Add string-contract tests that lock these requirements:

```rust
assert!(terminal_host.contains("callback scroll-thumb-drag-requested(float);"));
assert!(terminal_host.contains("callback scroll-jump-requested(float);"));
assert!(terminal_host.contains("event.modifiers.control && event.text == Key.Insert"));
assert!(terminal_host.contains("event.modifiers.shift && event.text == Key.PageUp"));
assert!(terminal_host.contains("scrollbar-track := Rectangle {"));
assert!(terminal_host.contains("scrollbar-thumb := Rectangle {"));
assert!(terminal_host.contains("font-family: root.terminal-font-family;"));
assert!(!terminal_host.contains("private property <string> terminal-font-family: \"Cascadia Mono\";"));
```

Also add assertions that the host exposes projected scrollback properties:

```rust
assert!(app_window.contains("in-out property <int> workspace-session-viewport-offset-lines: 0;"));
assert!(app_window.contains("in-out property <int> workspace-session-viewport-max-offset-lines: 0;"));
assert!(app_window.contains("in-out property <bool> workspace-session-viewport-at-bottom: true;"));
```

**Step 2: Run the focused contract tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec terminal_session_host_exposes_cell_cursor_selection_and_context_menu_contract terminal_session_host_uses_compact_terminal_layout_contract -- --nocapture
```

Expected:

- FAIL because the scrollbar callbacks, new scrollback properties, and expanded shortcut contract are not in the Slint files yet.

**Step 3: Implement the Slint-side interaction contract**

In `ui/shell/terminal-session-host.slint`:

- expand the font stack to `Cascadia Code`, `Cascadia Mono`, `Consolas`, `JetBrains Mono`;
- add projected properties for `viewport-offset`, `viewport-max`, and `viewport-at-bottom`;
- add right-side scrollbar track/thumb visuals;
- support click-to-jump and drag-to-scroll thumb interactions;
- intercept only the approved local shortcuts:
  - `Ctrl+Shift+C/V`
  - `Ctrl+Insert`
  - `Shift+Insert`
  - `Shift+PageUp/PageDown`
  - `Ctrl+Shift+Home/End`
- keep plain `Ctrl+C` and plain `Ctrl+V` untouched for remote use.

In `ui/app-window.slint` and `ui/shell/workspace-pane.slint`:

- thread the new scrollback properties and callbacks through the existing workspace terminal host bindings.

In `ui/theme/tokens.slint`:

- add scrollbar colors that match the new terminal palette.

**Step 4: Re-run the focused contract tests**

Run:

```bash
cargo test --test workspace_tabs_spec terminal_session_host_exposes_cell_cursor_selection_and_context_menu_contract terminal_session_host_uses_compact_terminal_layout_contract -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint ui/theme/tokens.slint tests/workspace_tabs_spec.rs
git commit -m "feat: add terminal shortcut and scrollbar ui contract"
```

### Task 4: Wire scrollback projection and local terminal commands through bootstrap and session manager

**Files:**
- Modify: `src/app/ssh/session_manager.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing integration tests**

Add tests that prove:

```rust
#[test]
fn session_manager_can_scroll_active_session_to_top_or_bottom() {
    // fake runtime records jump/top/bottom requests or line offsets
}

#[test]
fn bootstrap_projects_terminal_scrollback_state_into_window_properties() {
    // a runtime surface update with viewport metadata reaches the app window projection
}

#[test]
fn terminal_input_callback_snaps_scrolled_session_back_to_latest_surface() {
    // bootstrap sends key/paste/scroll commands and refreshes the active projection
}
```

**Step 2: Run the focused integration tests**

Run:

```bash
cargo test --test ssh_session_manager_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- FAIL because `SessionManager` and `bootstrap` do not yet expose jump-to-top/bottom or project the new scrollback properties.

**Step 3: Implement the wiring**

In `src/app/ssh/session_manager.rs`:

- add methods to scroll by lines, scroll to top, and scroll to bottom through the runtime control boundary;
- update cached `TerminalSurfaceState` after each local scroll command.

In `src/app/bootstrap.rs`:

- project the new scrollback properties into the Slint window;
- bind the new callbacks from `AppWindow`;
- refresh the active terminal projection after local shortcut, paste, and scroll commands so the scrollbar and viewport state stay synchronized.

Implementation sketch:

```rust
app.on_workspace_session_scroll_jump_requested(|ratio| {
    bridge.with_active_terminal_session(|session_id| {
        session_manager.scroll_session_to_ratio(session_id, ratio)
    });
    bridge.refresh_active_terminal_projection();
});
```

**Step 4: Re-run the focused integration tests**

Run:

```bash
cargo test --test ssh_session_manager_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS

**Step 5: Run the required validation commands**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 6: Commit**

```bash
git add src/app/ssh/session_manager.rs src/app/bootstrap.rs tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: wire terminal polish interactions through bootstrap"
```

### Task 5: Full regression verification

**Files:**
- Modify: `verification.md`

**Step 1: Run the focused regression suite**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 2: Run full workspace validation**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 3: Record verification evidence**

Append the command list and pass/fail result to `verification.md`.

**Step 4: Commit**

```bash
git add verification.md
git commit -m "docs: record terminal interaction polish verification"
```
