# SSH Terminal Input Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete parser-side banner filtering, session-aware terminal input, theme-aware palette, local scrollback routing, and VSCode-like terminal rendering for SSH sessions.

**Architecture:** Keep `wezterm-term` as the single source of terminal truth inside `src/app/ssh/runtime.rs`, and move interaction semantics into the live `TerminalSession` instead of using temporary stateless encoders. Slint remains responsible for event capture and pixel-space hit testing, while `SessionManager` and `bootstrap` forward structured commands into the active session. Theme-aware palette updates, parser-side filtering, and local scrollback all live at the runtime/session boundary so the future custom renderer can reuse the same terminal contract.

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`

---

### Task 1: Lock the runtime behavior with failing tests

**Files:**
- Modify: `src/app/ssh/runtime.rs:45-160`
- Modify: `tests/terminal_session_spec.rs`
- Create: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing tests**

Add runtime-level tests for these behaviors:

```rust
#[test]
fn exact_cockpit_banner_is_filtered_before_terminal_parser() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(
        b"Activate the web console with: systemctl enable --now cockpit.socket\r\n[root@host ~]# "
    );
    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(!snapshot.visible_lines.iter().any(|line| line.contains("cockpit.socket")));
    assert!(snapshot.visible_lines.iter().any(|line| line.contains("[root@host ~]#")));
}

#[test]
fn function_keys_and_application_cursor_keys_are_encoded_from_live_terminal_state() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(b"\x1b[?1h");
    let up = session.send_key_event(TerminalKeyEvent::named("up", false, false, false)).unwrap();
    let f5 = session.send_key_event(TerminalKeyEvent::function(5, false, false, false)).unwrap();
    assert_eq!(up, b"\x1bOA");
    assert_eq!(f5, b"\x1b[15~");
}

#[test]
fn bracketed_paste_wraps_clipboard_payload_when_enabled() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(b"\x1b[?2004h");
    let bytes = session.encode_paste("echo hi\n").unwrap();
    assert_eq!(bytes, b"\x1b[200~echo hi\n\x1b[201~");
}

#[test]
fn local_scrollback_changes_visible_projection_without_mutating_remote_screen() {
    let mut session = TerminalSession::new(4, 20);
    session.apply_remote_bytes(b"1\r\n2\r\n3\r\n4\r\n5\r\n6\r\n");
    let latest = session.surface_state(Uuid::new_v4());
    session.scroll_viewport_lines(2);
    let scrolled = session.surface_state(Uuid::new_v4());
    assert_ne!(latest.visible_lines, scrolled.visible_lines);
    assert!(scrolled.visible_lines.iter().any(|line| line == "3"));
}

#[test]
fn light_theme_palette_changes_default_background_projection() {
    let mut session = TerminalSession::new(24, 80);
    session.set_theme_mode(ThemeMode::Light);
    session.apply_remote_bytes(b"[root@host ~]# ");
    let snapshot = session.surface_state(Uuid::new_v4());
    let prompt = snapshot.cells.iter().find(|cell| cell.col == 0).unwrap();
    assert_ne!(prompt.bg_rgba, 0xff00_0000);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec -- --nocapture
```

Expected:

- FAIL because `TerminalKeyEvent`, `send_key_event`, `encode_paste`, `scroll_viewport_lines`, and `set_theme_mode` do not exist yet.
- FAIL because the exact banner is still visible in surface projection.

**Step 3: Add the missing runtime API surface in the plan target**

Introduce these public runtime entry points in `src/app/ssh/runtime.rs`:

```rust
pub enum TerminalKeyKind {
    Named(&'static str),
    Function(u8),
    Char(char),
}

pub struct TerminalKeyEvent {
    pub key: TerminalKeyKind,
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
}

impl TerminalSession {
    pub fn send_key_event(&mut self, event: TerminalKeyEvent) -> Result<Vec<u8>>;
    pub fn encode_paste(&mut self, text: &str) -> Result<Vec<u8>>;
    pub fn scroll_viewport_lines(&mut self, delta: i32);
    pub fn set_theme_mode(&mut self, mode: ThemeMode);
}
```

**Step 4: Re-run the tests until the failure surface is reduced to implementation details**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec -- --nocapture
```

Expected:

- The compiler now reaches concrete logic failures instead of missing-symbol failures.

**Step 5: Commit**

```bash
git add tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs src/app/ssh/runtime.rs
git commit -m "test: lock ssh terminal runtime interaction contract"
```

### Task 2: Implement parser-side exact banner filtering

**Files:**
- Modify: `src/app/ssh/runtime.rs:720-780`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Write the failing filter-focused regression**

Add one more test that proves only the exact string is filtered:

```rust
#[test]
fn similar_lines_are_not_filtered_when_they_do_not_exactly_match() {
    let mut session = TerminalSession::new(24, 80);
    session.apply_remote_bytes(
        b"Activate the web console with: systemctl enable --now cockpit.socket now\r\n"
    );
    let snapshot = session.surface_state(Uuid::new_v4());
    assert!(snapshot.visible_lines.iter().any(|line| line.contains("cockpit.socket now")));
}
```

**Step 2: Run the targeted test**

Run:

```bash
cargo test --test terminal_session_spec exact_cockpit_banner_is_filtered_before_terminal_parser similar_lines_are_not_filtered_when_they_do_not_exactly_match -- --nocapture
```

Expected:

- FAIL because no parser-side filter exists yet.

**Step 3: Implement a packet-safe line filter**

In `src/app/ssh/runtime.rs`, add a small line buffer owned by `TerminalSession`:

```rust
const FILTERED_EXACT_BANNER: &str =
    "Activate the web console with: systemctl enable --now cockpit.socket";

struct PendingRemoteLineBuffer {
    bytes: Vec<u8>,
}

impl PendingRemoteLineBuffer {
    fn push_and_filter(&mut self, incoming: &[u8]) -> Vec<u8>;
}
```

Implementation rules:

- Buffer incomplete lines across SSH packets;
- Compare normalized line content without trailing `\r\n`;
- Drop only the exact target line;
- Forward every other byte sequence to `terminal.advance_bytes(...)` unchanged.

**Step 4: Re-run the targeted tests**

Run:

```bash
cargo test --test terminal_session_spec exact_cockpit_banner_is_filtered_before_terminal_parser similar_lines_are_not_filtered_when_they_do_not_exactly_match -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs tests/terminal_session_spec.rs
git commit -m "feat: filter exact ssh cockpit banner before parsing"
```

### Task 3: Replace static key encoding with a live session encoder

**Files:**
- Modify: `src/app/ssh/runtime.rs:850-1105`
- Modify: `src/app/ssh/session_manager.rs:33-41, 184-240`
- Modify: `src/app/bootstrap.rs:910-977, 2765-2795`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_session_manager_spec.rs`

**Step 1: Write failing tests for the manager/runtime path**

Add a session-manager test proving key events are forwarded structurally instead of pre-encoded bytes:

```rust
#[test]
fn session_manager_forwards_structured_key_events_to_runtime() {
    // assert the fake runtime receives TerminalKeyEvent::Function(5)
    // instead of a pre-baked byte vector from bootstrap
}
```

**Step 2: Run the focused tests**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_session_manager_spec -- --nocapture
```

Expected:

- FAIL because `SessionRuntimeControl` only accepts raw bytes today.

**Step 3: Refactor the contract**

Change the runtime control boundary to prefer structured input:

```rust
pub trait SessionRuntimeControl: Send {
    fn send_text_input(&self, text: String) -> Result<()>;
    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()>;
    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()>;
    fn send_paste(&self, text: String) -> Result<()>;
    fn resize(&self, rows: u32, cols: u32) -> Result<()>;
}
```

In `bootstrap.rs`:

- remove the static `encode_named_key_input()` path;
- forward `TerminalKeyEvent` into the active session;
- keep plain text input separate from functional key input.

In `runtime.rs`:

- read live terminal mode before encoding;
- support named keys, `F1-F24`, and control/alt/shift modifiers;
- keep `Ctrl+C` terminal-native and move clipboard logic out of that branch.

**Step 4: Re-run the tests**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_session_manager_spec -- --nocapture
```

Expected:

- PASS for structured key forwarding and live encoding behavior.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs tests/terminal_session_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: route ssh terminal keys through live session encoder"
```

### Task 4: Add terminal-native clipboard and bracketed paste behavior

**Files:**
- Modify: `ui/shell/terminal-session-host.slint:271-350, 501-555`
- Modify: `ui/shell/workspace-pane.slint:31-36, 91-112`
- Modify: `ui/app-window.slint:191-196, 479-500`
- Modify: `src/app/bootstrap.rs:1006-1057, 2765-2825`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Write the failing UI contract assertions**

Extend `tests/workspace_tabs_spec.rs` with assertions for:

- `Ctrl+Shift+C` and `Ctrl+Shift+V` handling;
- `Shift+Insert` handling;
- no special local branch for plain `Ctrl+C`;
- explicit paste callback still exists for context menu usage.

Example:

```rust
assert!(terminal_host.contains("event.modifiers.control && event.modifiers.shift"));
assert!(terminal_host.contains("event.text == Key.Insert"));
assert!(!terminal_host.contains("event.modifiers.control && !event.modifiers.alt && !event.modifiers.shift && event.text == \"c\")"));
```

**Step 2: Run the tests**

Run:

```bash
cargo test --test workspace_tabs_spec -- --nocapture
```

Expected:

- FAIL because the current UI still special-cases plain `Ctrl+C` and `Ctrl+V`.

**Step 3: Update the Slint and bootstrap contract**

In `ui/shell/terminal-session-host.slint`:

- map `Ctrl+Shift+C` to `copy-selection-requested(...)`;
- map `Ctrl+Shift+V` and `Shift+Insert` to `paste-requested()`;
- let plain `Ctrl+C` fall through to `key-input(...)`.

In `src/app/bootstrap.rs`:

- keep local clipboard access in `forward_active_workspace_copy_selection(...)`;
- send paste text through the runtime’s dedicated `send_paste(...)` path so bracketed paste can be honored.

**Step 4: Re-run the tests**

Run:

```bash
cargo test --test workspace_tabs_spec --test terminal_session_spec -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/terminal_session_spec.rs
git commit -m "feat: align ssh terminal clipboard behavior with terminal conventions"
```

### Task 5: Implement wheel routing and local scrollback

**Files:**
- Modify: `ui/shell/terminal-session-host.slint:354-449`
- Modify: `ui/shell/workspace-pane.slint:31-36, 91-112`
- Modify: `ui/app-window.slint:191-196, 479-500`
- Modify: `src/app/ssh/runtime.rs:777-847`
- Modify: `src/app/ssh/session_manager.rs:33-41, 184-240`
- Modify: `src/app/bootstrap.rs:979-1004, 1059-1065, 2765-2825`
- Create: `tests/terminal_scrollback_spec.rs`

**Step 1: Write the failing tests**

Create `tests/terminal_scrollback_spec.rs`:

```rust
#[test]
fn wheel_without_mouse_grab_scrolls_local_viewport() {
    // build a session with more history than viewport height
    // assert scroll offset changes visible lines
}

#[test]
fn wheel_with_mouse_grab_is_forwarded_as_remote_mouse_input() {
    // enable mouse tracking and assert viewport does not change locally
}
```

**Step 2: Run the tests**

Run:

```bash
cargo test --test terminal_scrollback_spec -- --nocapture
```

Expected:

- FAIL because no scroll routing or viewport state exists yet.

**Step 3: Implement viewport ownership and Slint wheel capture**

In `runtime.rs`:

- add a viewport offset field owned by `TerminalSession`;
- clamp it against scrollback bounds;
- make `visible_rows()` and `visible_cells()` project from the current viewport, not always the newest bottom slice.

In `terminal-session-host.slint`:

- add `scroll-event(PointerScrollEvent)` on the main `TouchArea`;
- emit a new callback such as `scroll-requested(int delta_lines, bool shift, bool ctrl, bool alt)`.

In `bootstrap.rs`:

- if `surface.mouse_grabbed`, translate wheel into remote mouse input;
- else call a local scroll method on the active session.

**Step 4: Re-run the tests**

Run:

```bash
cargo test --test terminal_scrollback_spec --test terminal_session_spec -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs tests/terminal_scrollback_spec.rs tests/terminal_session_spec.rs
git commit -m "feat: add ssh terminal wheel routing and local scrollback"
```

### Task 6: Add theme-aware palette updates and full-surface rendering

**Files:**
- Modify: `src/app/ssh/runtime.rs:830-1148`
- Modify: `src/app/ssh/session_manager.rs:33-41, 184-240`
- Modify: `src/app/bootstrap.rs:243-247, 1696-1754, 2189-2194`
- Modify: `ui/shell/terminal-session-host.slint:81-103, 354-488`
- Modify: `ui/theme/tokens.slint:3-26`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing tests**

Add tests for:

- light theme default background is not black;
- theme toggle refreshes existing terminal palette;
- terminal host contains a dedicated blank-surface layer instead of only per-cell rectangles.

Example source contract assertion:

```rust
assert!(terminal_host.contains("blank-surface := Rectangle {"));
assert!(terminal_host.contains("font-family: \"Cascadia Mono\";"));
```

**Step 2: Run the tests**

Run:

```bash
cargo test --test terminal_session_spec --test workspace_tabs_spec -- --nocapture
```

Expected:

- FAIL because palette updates are not wired into live sessions and no blank-surface layer exists.

**Step 3: Implement the palette bridge**

In `runtime.rs`:

- replace `ColorPalette::default()` with a `ThemeMode`-driven builder;
- add a runtime command to update theme mode on the live session;
- refresh `TerminalSurfaceState` after theme changes.

In `bootstrap.rs`:

- after `toggle_theme_mode()`, notify the active and background SSH sessions;
- keep `window.set_dark_mode(...)` and shell tokens in sync with the same mode source.

In `terminal-session-host.slint`:

- add a full terminal canvas background layer using the projected default background color;
- keep cell rectangles only for explicit cell content and selection overlays;
- preserve cursor rendering on top.

**Step 4: Re-run the tests**

Run:

```bash
cargo test --test terminal_session_spec --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/ssh/session_manager.rs src/app/bootstrap.rs ui/shell/terminal-session-host.slint ui/theme/tokens.slint tests/terminal_session_spec.rs tests/workspace_tabs_spec.rs
git commit -m "feat: sync ssh terminal palette with app theme"
```

### Task 7: Tighten VSCode-like metrics and verify the full contract

**Files:**
- Modify: `ui/shell/terminal-session-host.slint:81-103, 451-488`
- Modify: `src/app/bootstrap.rs:1696-1754`
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/terminal_session_spec.rs`

**Step 1: Write the failing visual contract assertions**

Add source or runtime assertions for:

- cell metrics are no longer hard-coded as the only truth;
- cursor, selection, and hit testing depend on shared metrics helpers;
- terminal host remains mono-font based and editor-like.

Example:

```rust
assert!(terminal_host.contains("terminal-font-size"));
assert!(terminal_host.contains("terminal-cell-width"));
assert!(terminal_host.contains("terminal-cell-height"));
```

**Step 2: Run the tests**

Run:

```bash
cargo test --test workspace_tabs_spec --test terminal_scrollback_spec --test terminal_session_spec -- --nocapture
```

Expected:

- FAIL until metrics and rendering helpers are centralized.

**Step 3: Implement the shared metrics contract**

In `terminal-session-host.slint`:

- centralize font family, font size, cell width, and cell height in one visual contract block;
- keep the current monospace direction;
- align cursor geometry, row hit-testing, and resize calculations to the same metrics.

In `bootstrap.rs`:

- if needed, project any additional default background or metric state from runtime into Slint.

**Step 4: Run the full verification set**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test terminal_scrollback_spec --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/workspace_tabs_spec.rs tests/terminal_scrollback_spec.rs tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "feat: finish ssh terminal input and rendering contract"
```

### Task 8: Final doc sync and branch verification

**Files:**
- Modify: `docs/plans/2026-03-26-ssh-terminal-input-rendering-design.md`
- Modify: `verification.md`

**Step 1: Update the design doc implementation status**

Record which tasks were completed, any intentional scope cuts, and the final verification commands actually run.

**Step 2: Run the final command set**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test terminal_scrollback_spec --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 3: Record evidence**

Append the command list and observed outcomes to `verification.md`.

**Step 4: Review the resulting diff**

Run:

```bash
git diff --stat
```

Expected:

- Only the planned runtime, UI, test, and documentation files appear.

**Step 5: Commit**

```bash
git add docs/plans/2026-03-26-ssh-terminal-input-rendering-design.md docs/plans/2026-03-26-ssh-terminal-input-rendering-implementation-plan.md verification.md
git commit -m "docs: finalize ssh terminal input rendering plan"
```
