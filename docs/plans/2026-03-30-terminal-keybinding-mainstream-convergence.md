# Terminal Keybinding Mainstream Convergence Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Converge the terminal host keyboard contract toward mainstream terminal defaults without regressing the existing copy/paste contract.

**Architecture:** Keep terminal key classification in `ui/shell/terminal-session-host.slint`, but emit explicit local terminal actions for reserved `Ctrl+Shift+...` shortcuts instead of swallowing them silently. Route those local actions through `WorkspacePane` and `AppWindow` into existing bootstrap handlers so the app reuses current tab, menu, and asset-search logic instead of inventing a separate shortcut subsystem.

**Tech Stack:** Slint UI callbacks, Rust bootstrap event bindings, smoke tests in `tests/bootstrap_smoke.rs`

---

### Task 1: Lock The Keyboard Contract In Smoke Tests

**Files:**
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add smoke coverage for:
- `Ctrl+Shift+T` opening a new workspace tab from the active terminal asset
- `Ctrl+Shift+W` closing the active workspace tab locally
- `Ctrl+Shift+F` expanding asset search locally
- `Ctrl+Shift+P` toggling the global menu locally
- `Shift+Home` and `Shift+End` jumping local scrollback to top and bottom
- non-reserved `Ctrl+Shift+<letter>` chords forwarding to the remote terminal instead of disappearing

**Step 2: Run tests to verify they fail**

Run: `cargo test --test bootstrap_smoke workspace_terminal_ctrl_shift_t_opens_new_tab_from_active_terminal_asset -q`

Run: `cargo test --test bootstrap_smoke workspace_terminal_shift_home_end_shortcuts_jump_scrollback_locally -q`

Expected: FAIL because the current terminal host only swallows `Ctrl+Shift+T/W/P/F` and still binds scroll jumps to `Ctrl+Shift+Home/End`.

### Task 2: Route Reserved Local Actions Through The Existing App Callbacks

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`

**Step 1: Write the minimal implementation**

- Add a terminal-local-action callback chain from `TerminalSessionHost` to `WorkspacePane` to `AppWindow`
- Map `Ctrl+Shift+T/W/P/F` to explicit local action ids
- Change local scroll jumps from `Ctrl+Shift+Home/End` to `Shift+Home/End`
- Stop silently swallowing unrelated `Ctrl+Shift+<letter>` combos
- Handle the new local action callback in bootstrap by reusing existing tab, search, and menu logic

**Step 2: Run the focused tests**

Run: `cargo test --test bootstrap_smoke workspace_terminal_ctrl_shift_shortcut_matrix_keeps_local_contract -q`

Run: `cargo test --test bootstrap_smoke ctrl_shift_non_reserved_letter_shortcuts_forward_remote_terminal_input -q`

Run: `cargo test --test bootstrap_smoke workspace_terminal_shift_home_end_shortcuts_jump_scrollback_locally -q`

Expected: PASS

### Task 3: Verify The Full Keyboard Matrix And Commit

**Files:**
- Modify: `tests/bootstrap_smoke.rs` if any assertion names need cleanup after green

**Step 1: Run the broader targeted suite**

Run: `cargo test --test bootstrap_smoke workspace_terminal_ -q`

Run: `cargo test --test bootstrap_smoke ctrl_shift_ -q`

Expected: PASS

**Step 2: Commit**

```bash
git add docs/plans/2026-03-30-terminal-keybinding-mainstream-convergence.md tests/bootstrap_smoke.rs ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint ui/app-window.slint src/app/bootstrap.rs
git commit -m "feat: align terminal keybindings with mainstream defaults"
```
