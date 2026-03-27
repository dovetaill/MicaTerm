# Terminal Theme System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a structured terminal theme preset system with `Mica Code Dark` / `Mica Code Light`, wire it into the SSH runtime, and advertise `COLORTERM=truecolor` without rewriting the renderer.

**Architecture:** Extract terminal palette data into a dedicated `terminal_theme` module that maps `ThemeMode` to a named preset and converts the preset into `wezterm_term::color::ColorPalette`. Keep `TERM=xterm-256color` as the PTY baseline, add `COLORTERM=truecolor` via SSH environment negotiation, and verify the behavior through runtime and SSH-focused regression tests.

**Tech Stack:** Rust, Slint, Tokio, `wezterm-term`, `russh`, `termwiz`

---

### Task 1: Lock terminal theme system expectations with failing tests

**Files:**
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Reference: `src/app/ssh/runtime.rs`
- Reference: `src/app/mod.rs`

**Step 1: Write the failing runtime theme tests**

In `tests/terminal_session_spec.rs`, add tests that assert:

- dark mode default background becomes `#11161d`;
- light mode default background becomes `#f7f9fc`;
- cursor colors match the planned preset values.

Use assertions in the existing style:

```rust
assert_eq!(snapshot.default_bg_rgba, 0xff11_161d);
assert_eq!(snapshot.cursor.bg_rgba, 0xff7f_b7ff);
```

**Step 2: Write the failing ANSI palette regression test**

In `tests/ssh_terminal_interaction_spec.rs`, add a test that emits ANSI background sequences for at least:

- black (`40m`)
- bright white (`107m`)

and asserts the projected cell background matches the planned `Mica Code Light` palette.

**Step 3: Run the focused tests and confirm failure**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec -- --nocapture
```

Expected:

- FAIL because the current palette values still use the old hardcoded theme table.

**Step 4: Commit**

```bash
git add tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "test: lock terminal theme preset expectations"
```

### Task 2: Extract terminal theme presets into a dedicated module

**Files:**
- Create: `src/app/terminal_theme.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/ssh/runtime.rs`
- Test: `tests/terminal_session_spec.rs`

**Step 1: Create the terminal theme data model**

In `src/app/terminal_theme.rs`, define a small preset structure:

```rust
pub struct TerminalThemePreset {
    pub name: &'static str,
    pub background: u32,
    pub foreground: u32,
    pub cursor_bg: u32,
    pub cursor_fg: u32,
    pub selection_bg: (u8, u8, u8, f32),
    pub ansi: [(u8, u8, u8); 16],
    pub scrollbar_thumb: (u8, u8, u8),
    pub split: (u8, u8, u8),
}
```

Keep the exact representation practical for converting into `SrgbaTuple`; do not over-engineer a config parser.

**Step 2: Define the two presets**

In the same module, define:

- `mica_code_dark()`
- `mica_code_light()`

with the approved color values from the design doc.

**Step 3: Add conversion helpers**

Add helpers that convert a `TerminalThemePreset` into `wezterm_term::color::ColorPalette`.

Example shape:

```rust
pub fn palette_for_theme_mode(theme_mode: ThemeMode) -> ColorPalette {
    match theme_mode {
        ThemeMode::Dark => mica_code_dark().to_color_palette(),
        ThemeMode::Light => mica_code_light().to_color_palette(),
    }
}
```

**Step 4: Wire runtime to the new module**

In `src/app/ssh/runtime.rs`, remove the old in-place `build_terminal_color_palette(...)` table and delegate to the new module.

In `src/app/mod.rs`, export the new module.

**Step 5: Run the focused tests and confirm pass**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec -- --nocapture
```

Expected:

- PASS for the new preset color expectations;
- PASS for the existing palette projection tests.

**Step 6: Commit**

```bash
git add src/app/terminal_theme.rs src/app/mod.rs src/app/ssh/runtime.rs tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "feat: extract terminal theme presets"
```

### Task 3: Advertise `COLORTERM=truecolor` during SSH session bootstrap

**Files:**
- Modify: `src/app/ssh/runtime.rs`
- Modify: `tests/ssh_session_manager_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `/root/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/russh-0.58.0/src/channels/mod.rs`

**Step 1: Write the failing SSH environment negotiation test**

In `tests/ssh_session_manager_spec.rs` or another SSH runtime-facing test file, add a test that verifies the SSH session bootstrap attempts to negotiate truecolor support before requesting the interactive shell.

If a full transport-level test is too heavy, cover it through a small helper extracted from runtime bootstrap, for example:

```rust
let env = terminal_environment_variables();
assert_eq!(env, [("COLORTERM", "truecolor")]);
```

**Step 2: Extract a tiny helper for negotiated terminal environment**

In `src/app/ssh/runtime.rs`, add a focused helper such as:

```rust
fn negotiated_terminal_environment() -> [(&'static str, &'static str); 1] {
    [("COLORTERM", "truecolor")]
}
```

**Step 3: Apply the environment before requesting the shell**

Use `channel.set_env(true, "COLORTERM", "truecolor").await` after PTY success and before `request_shell(true)`.

If the server rejects the env request, convert that into a logged warning or non-fatal error handling path. Do not break session startup solely because `COLORTERM` was not accepted.

**Step 4: Run the targeted tests**

Run:

```bash
cargo test --test ssh_session_manager_spec --test bootstrap_smoke -- --nocapture
```

Expected:

- PASS for the new truecolor negotiation coverage;
- PASS for existing SSH/session bootstrap tests.

**Step 5: Commit**

```bash
git add src/app/ssh/runtime.rs tests/ssh_session_manager_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: advertise terminal truecolor support"
```

### Task 4: Verify terminal palette projection still matches UI-facing runtime contracts

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Reference: `ui/app-window.slint`
- Reference: `ui/shell/terminal-session-host.slint`

**Step 1: Add any missing UI contract assertions**

Ensure the UI-facing tests still verify:

- projected default fg/bg are forwarded into `AppWindow`;
- terminal host remains driven by runtime palette projection;
- no fallback to stale hardcoded palette constants appears in the Slint layer.

**Step 2: Run the UI/runtime contract tests**

Run:

```bash
cargo test --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 3: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "test: verify terminal theme UI contracts"
```

### Task 5: Run full regression verification

**Files:**
- No source changes required unless verification reveals a regression

**Step 1: Run focused terminal regression suites**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test ssh_session_manager_spec --test bootstrap_smoke --test workspace_tabs_spec -- --nocapture
```

Expected:

- PASS

**Step 2: Run workspace validation**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected:

- PASS

**Step 3: Commit verification-only follow-up if needed**

If no code changes were needed, do not create an extra commit. If verification required a small fix, commit it with a precise message:

```bash
git add <exact files>
git commit -m "fix: address terminal theme regression"
```

### Task 6: Write the post-implementation TDD handoff doc

**Files:**
- Create: `docs/plans/2026-03-28-terminal-theme-system-tdd-spec.md`

**Step 1: Summarize the implemented structures**

Document:

- the terminal theme preset struct(s);
- the runtime mapping path from `ThemeMode` to `ColorPalette`;
- the SSH environment negotiation helper for truecolor support.

**Step 2: Capture edge cases**

Include at least:

- servers that reject `set_env(COLORTERM=truecolor)`;
- ANSI black/white semantics drifting in light mode;
- theme projection mismatches between runtime palette and Slint background usage;
- future preset additions accidentally bypassing the shared conversion path.

**Step 3: Commit the handoff doc**

```bash
git add docs/plans/2026-03-28-terminal-theme-system-tdd-spec.md
git commit -m "docs: add terminal theme system tdd handoff"
```
