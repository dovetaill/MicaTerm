# Terminal Interaction Polish Follow-up Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fix the remaining terminal interaction regressions so modifier handling, wheel scrolling, light-mode canvas background, and terminal typography feel like a deliberate desktop IDE terminal instead of a prototype.

**Architecture:** Keep `wezterm-term` as the source of terminal truth, but extend `TerminalSurfaceState` to project default canvas colors and let `TerminalSessionHost` own stricter modifier routing plus wheel-delta accumulation. `bootstrap` remains the bridge that projects runtime surface data into `AppWindow`, while the terminal font stack moves from OS fallback assumptions to a bundled primary font with tighter metrics.

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `termwiz`, `russh`

---

### Task 1: Lock the follow-up regressions with failing tests

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `src/app/ssh/runtime.rs`

**Step 1: Add runtime tests for canvas palette projection**

In `tests/terminal_session_spec.rs`, add failing tests that verify `TerminalSurfaceState` exposes default canvas foreground/background for both dark and light theme modes.

**Step 2: Add runtime tests for multi-line wheel behavior**

In `tests/ssh_terminal_interaction_spec.rs`, add a failing test that exercises local scrollback via repeated wheel-like deltas and asserts the viewport moves by more than one line when the accumulated delta crosses the threshold.

**Step 3: Add bootstrap/UI contract tests for modifier routing**

In `tests/bootstrap_smoke.rs`, add a failing test that proves `Ctrl+Shift+F` or another reserved `Ctrl+Shift+<letter>` combination does not result in remote text/key input.

**Step 4: Add UI contract tests for font and metrics defaults**

In `tests/workspace_tabs_spec.rs`, add failing assertions that:

- the terminal host references the bundled primary terminal font;
- the terminal font size / cell width / cell height defaults are tighter than the current prototype values;
- pure `Ctrl` / `Shift` do not appear in the remote forwarding path.

**Step 5: Run the targeted tests and confirm failure**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- FAIL because the runtime does not yet project canvas colors;
- FAIL because wheel scrolling still behaves as single-line movement;
- FAIL because reserved `Ctrl+Shift+<letter>` combinations are still forwarded;
- FAIL because font and metrics defaults are still the prototype values.

**Step 6: Commit**

```bash
git add tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "test: lock terminal follow-up regressions"
```

### Task 2: Project runtime canvas colors and align terminal background rendering

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Test: `tests/terminal_session_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Extend the runtime snapshot shape**

In `src/app/ssh/runtime.rs`, add default canvas palette projection fields to `TerminalSurfaceState`, for example:

```rust
pub default_fg_rgba: u32,
pub default_bg_rgba: u32,
```

Populate them from `self.terminal.palette()` inside `surface_state(...)`.

**Step 2: Project the new fields through bootstrap**

In `src/app/bootstrap.rs`, extend `sync_workspace_session_state(...)` so `AppWindow` receives the terminal default foreground/background values, with sensible reset defaults when no surface is active.

**Step 3: Extend the Slint property chain**

In `ui/app-window.slint` and `ui/shell/workspace-pane.slint`, add the new terminal canvas color properties and pass them down into `TerminalSessionHost`.

**Step 4: Make the host background follow runtime palette**

In `ui/shell/terminal-session-host.slint`, replace the fixed `ThemeTokens.terminal-canvas-surface` usage for the terminal canvas and blank surface with the projected runtime background color.

**Step 5: Re-run the focused tests**

Run:

```bash
cargo test --test terminal_session_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for the new canvas palette projection tests;
- PASS for the existing scrollback and projection tests.

**Step 6: Commit**

```bash
git add src/app/ssh/runtime.rs src/app/bootstrap.rs ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint tests/terminal_session_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: project terminal canvas palette colors"
```

### Task 3: Rework modifier routing so `Ctrl+Shift` stays local

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Enumerate reserved local shortcut handling**

In `ui/shell/terminal-session-host.slint`, refactor `key-pressed(event)` so these categories are explicit and ordered:

- pure modifier keys are ignored;
- local action shortcuts execute copy/paste/page scroll/jump actions;
- reserved `Ctrl+Shift+<letter>` combinations without implemented actions are swallowed;
- only allowed remote combinations continue into `root.key-input(...)`.

**Step 2: Remove the accidental generic `Ctrl` forwarding path**

Ensure the generic branch that currently forwards `((event.modifiers.control || event.modifiers.alt) && event.text != "")` does not catch reserved `Ctrl+Shift+...` combinations anymore.

**Step 3: Keep bootstrap wiring minimal**

Only adjust `src/app/bootstrap.rs` if needed to support any new local callback or verification hook. Do not broaden the runtime API in this task.

**Step 4: Re-run modifier-focused tests**

Run:

```bash
cargo test --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS for the new `Ctrl+Shift` routing coverage;
- PASS for existing terminal host contract tests.

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "fix: keep ctrl-shift terminal shortcuts local"
```

### Task 4: Replace one-line wheel scrolling with accumulated multi-line scrollback

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Add wheel accumulation state**

In `ui/shell/terminal-session-host.slint`, add private state for retained wheel delta and constants for:

- wheel delta threshold;
- lines per wheel notch;
- optional acceleration multiplier.

**Step 2: Convert wheel delta into multi-line scroll requests**

Update `scroll-event(event)` so wheel delta is accumulated and converted into `root.scroll-requested(...)` with multi-line step sizes rather than `1` or `-1`.

**Step 3: Preserve existing mouse-grabbed semantics**

Do not change the `bootstrap` / runtime rule that wheel events become remote mouse input when `mouse_grabbed == true`.

**Step 4: Re-run scroll-focused tests**

Run:

```bash
cargo test --test ssh_terminal_interaction_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for the new multi-line wheel behavior;
- PASS for the existing local scrollback vs remote mouse wheel tests.

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/ssh_terminal_interaction_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add accumulated terminal wheel scrolling"
```

### Task 5: Apply the bundled terminal font and tighter IDE-like metrics

**Files:**
- Create: `ui/fonts/` bundled terminal font assets
- Modify: `build.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/theme/tokens.slint`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Add the bundled primary terminal font**

Add the chosen `Iosevka Term` asset under a UI-managed font path and wire it into the Slint build so it is available cross-platform.

**Step 2: Replace the current fallback-heavy font stack**

Update the terminal host font family default so it prefers the bundled terminal font instead of the current OS fallback stack.

**Step 3: Tighten metrics**

Replace the prototype metrics with tighter defaults, including:

- smaller `terminal-font-size`;
- reduced `terminal-cell-width`;
- reduced `terminal-cell-height`;
- slightly reduced canvas padding if needed.

Do not change row/column semantics or break existing hit testing.

**Step 4: Re-run font/UI contract tests**

Run:

```bash
cargo test --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS for the new font/metrics contract assertions;
- PASS for existing terminal host UI contract checks.

**Step 5: Commit**

```bash
git add ui/fonts build.rs ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint ui/theme/tokens.slint tests/workspace_tabs_spec.rs
git commit -m "style: tighten terminal typography defaults"
```

### Task 6: Full regression verification and handoff docs

**Files:**
- Modify: `verification.md`
- Create: `docs/plans/2026-03-27-terminal-interaction-polish-followup-tdd-spec.md`

**Step 1: Run the focused regression suite**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 2: Run workspace verification**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 3: Record verification evidence**

Append the command list and pass/fail result to `verification.md`.

**Step 4: Write TDD handoff**

Create `docs/plans/2026-03-27-terminal-interaction-polish-followup-tdd-spec.md` covering:

- runtime snapshot additions;
- Slint callback routing contract;
- wheel accumulation state flow;
- default canvas palette projection;
- bundled font asset and metrics assumptions;
- edge cases and future tests.

**Step 5: Commit**

```bash
git add verification.md docs/plans/2026-03-27-terminal-interaction-polish-followup-tdd-spec.md
git commit -m "docs: record terminal follow-up verification"
```
