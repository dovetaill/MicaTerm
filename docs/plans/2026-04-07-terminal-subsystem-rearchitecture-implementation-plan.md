# Terminal Subsystem Re-architecture Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Preserve the existing Slint shell and business features while replacing the current terminal subsystem with a single-path renderer, an extracted core adapter boundary, Catppuccin-ready themes, and a migration path toward an Alacritty-style terminal core.

**Architecture:** First simplify the render/present architecture while keeping the existing core behind a new adapter seam. Then introduce a second terminal-core implementation behind the same contract, validate parity, and finally switch the default implementation. Keep terminal-only updates on a surface-local path and avoid whole-workspace recomputation during viewport changes.

**Tech Stack:** Rust, Slint, existing bootstrap/view-model shell, current `wezterm-term` integration, planned `alacritty_terminal` adapter, GPU renderer host, Catppuccin palette presets.

---

### Task 1: Freeze Behavior And Performance Contracts

**Files:**
- Modify: `tests/terminal_runtime_perf_contract_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Create: `tests/terminal_subsystem_perf_smoke.rs`
- Reference: `src/app/bootstrap/workspace_terminal.rs`
- Reference: `src/app/terminal_presenter.rs`

**Step 1: Write the failing tests**

Add source-level and behavior-level tests that assert:

- terminal-only scroll refreshes stay off any full workspace projection path
- renderer-facing hot paths consume a compact terminal snapshot contract
- theme mode changes continue to propagate into terminal state without whole-subsystem rebuilds

Example Rust skeleton:

```rust
#[test]
fn terminal_scroll_refresh_avoids_full_workspace_projection() {
    let source = std::fs::read_to_string("src/app/bootstrap/workspace_terminal.rs").unwrap();
    assert!(source.contains("refresh_active_terminal_surface_only("));
    assert!(!source.contains("refresh_active_workspace_projection("));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test terminal_runtime_perf_contract_spec terminal_scroll_refresh_avoids_full_workspace_projection -- --exact`

Expected: FAIL because the new terminal subsystem contracts do not exist yet.

**Step 3: Add the minimal placeholder coverage needed to express the target contracts**

Create a new focused smoke/spec file and update existing perf-contract files to describe the desired subsystem seams rather than today’s implementation details.

**Step 4: Run the focused tests again**

Run: `cargo test --test terminal_runtime_perf_contract_spec -- --nocapture`

Expected: the new tests still fail, but now fail for the correct missing-contract reason.

**Step 5: Commit**

```bash
git add tests/terminal_runtime_perf_contract_spec.rs tests/bootstrap_smoke.rs tests/ssh_terminal_interaction_spec.rs tests/terminal_subsystem_perf_smoke.rs
git commit -m "test: freeze terminal subsystem migration contracts"
```

### Task 2: Introduce A Terminal Core Adapter Boundary

**Files:**
- Create: `src/app/terminal_core/mod.rs`
- Create: `src/app/terminal_core/types.rs`
- Create: `src/app/terminal_core/wezterm_adapter.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `src/app/ssh/runtime/contracts.rs`
- Test: `tests/terminal_core_adapter_spec.rs`

**Step 1: Write the failing test**

Add a test that requires terminal runtime code to depend on a trait/object-safe adapter boundary rather than directly on concrete `wezterm-term` frame projection internals.

```rust
#[test]
fn runtime_depends_on_terminal_core_adapter_contract() {
    let source = std::fs::read_to_string("src/app/ssh/runtime/terminal.rs").unwrap();
    assert!(source.contains("dyn TerminalCoreAdapter"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_core_adapter_spec runtime_depends_on_terminal_core_adapter_contract -- --exact`

Expected: FAIL because no adapter module exists yet.

**Step 3: Write minimal implementation**

Create:

- `TerminalCoreAdapter` trait
- `TerminalFrameSnapshot`
- `ViewportState`
- `SelectionState`
- `WeztermTerminalCoreAdapter`

Move direct `wezterm_term`-specific logic behind the new adapter while preserving current behavior.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test terminal_core_adapter_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/terminal_core/mod.rs src/app/terminal_core/types.rs src/app/terminal_core/wezterm_adapter.rs src/app/ssh/runtime/terminal.rs src/app/ssh/runtime/contracts.rs tests/terminal_core_adapter_spec.rs
git commit -m "refactor: add terminal core adapter seam"
```

### Task 3: Replace Multi-Path Presenter Wiring With A Single Renderer Host Contract

**Files:**
- Create: `src/app/terminal_renderer/host.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/terminal_renderer_host_spec.rs`
- Reference: `src/app/terminal_scene_image.rs`
- Reference: `src/app/terminal_renderer/native_surface.rs`

**Step 1: Write the failing test**

Add a contract test that the active terminal presenter/bootstrap path routes through one renderer-host seam.

```rust
#[test]
fn bootstrap_routes_terminal_presentation_through_renderer_host() {
    let source = std::fs::read_to_string("src/app/bootstrap.rs").unwrap();
    assert!(source.contains("TerminalRendererHost"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_host_spec bootstrap_routes_terminal_presentation_through_renderer_host -- --exact`

Expected: FAIL because the host contract does not exist yet.

**Step 3: Write minimal implementation**

Introduce `TerminalRendererHost` as the only rendering entry point for terminal frames. Route current presenter variants through this contract and start isolating legacy scene/native helpers behind the host instead of exposing them to bootstrap directly.

**Step 4: Run tests to verify they pass**

Run: `cargo test --test terminal_renderer_host_spec -- --nocapture`

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/host.rs src/app/terminal_presenter.rs src/app/terminal_renderer/mod.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/bootstrap.rs tests/terminal_renderer_host_spec.rs
git commit -m "refactor: add single terminal renderer host contract"
```

### Task 4: Keep Terminal-Only Updates On A Surface-Local Path

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_renderer/host.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_runtime_perf_contract_spec.rs`

**Step 1: Write the failing test**

Add behavior tests asserting that:

- viewport scroll only refreshes terminal surface state
- theme change only invalidates terminal-local palette/render state
- renderer host can consume dirty-region updates without whole-workspace rebuilds

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke workspace_terminal_scroll_callbacks_update_active_session_surface -- --exact`

Expected: FAIL if the new host contract regresses terminal-local update routing.

**Step 3: Write minimal implementation**

Update the scroll/theme/terminal refresh callbacks so they talk to the renderer-host and terminal-core boundaries directly, not to heavier workspace-wide projection flows.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke workspace_terminal_scroll_callbacks_update_active_session_surface -- --exact
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap/workspace_terminal.rs src/app/bootstrap.rs src/app/terminal_renderer/host.rs tests/bootstrap_smoke.rs tests/terminal_runtime_perf_contract_spec.rs
git commit -m "perf: keep terminal updates on surface-local refresh paths"
```

### Task 5: Add Catppuccin-Backed Terminal Theme Presets And Shell Token Sync

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/theme/spec.rs`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/ui_preferences.rs`

**Step 1: Write the failing test**

Add tests that require:

- dark mode maps to Catppuccin Mocha
- light mode maps to Catppuccin Latte
- terminal foreground/background/cursor/selection values remain synchronized with shell-adjacent UI tokens

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_theme_selection_spec -- --nocapture`

Expected: FAIL because Catppuccin presets are not wired yet.

**Step 3: Write minimal implementation**

Replace the current terminal preset definitions with Catppuccin-backed palette data and update shell token defaults so terminal-adjacent UI surfaces remain visually coherent in both theme modes.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test ui_preferences -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_theme.rs src/theme/spec.rs ui/theme/tokens.slint ui/app-window.slint ui/shell/terminal-session-host.slint ui/shell/workspace-pane.slint tests/terminal_theme_selection_spec.rs tests/ui_preferences.rs
git commit -m "feat: add Catppuccin terminal theme mapping"
```

### Task 6: Introduce An Experimental Alacritty-Style Core Adapter

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/terminal_core/alacritty_adapter.rs`
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Test: `tests/terminal_core_parity_spec.rs`
- Test: `tests/terminal_scrollback_spec.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing test**

Add parity tests that run the same interaction scenarios through both adapters and compare:

- visible viewport text
- cursor position and visibility
- viewport offsets
- selection ranges
- truecolor values

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_core_parity_spec -- --nocapture`

Expected: FAIL because the Alacritty-style adapter is not implemented yet.

**Step 3: Write minimal implementation**

Add the new adapter behind a feature flag or explicit runtime selection, keeping the `wezterm` adapter as the control implementation until parity is good enough to compare behavior and performance.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_core_parity_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
```

Expected: PASS for the covered parity subset.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/terminal_core/alacritty_adapter.rs src/app/terminal_core/mod.rs src/app/terminal_core/types.rs src/app/ssh/runtime/terminal.rs tests/terminal_core_parity_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "feat: add experimental alacritty-style terminal core adapter"
```

### Task 7: Switch The Default Terminal Subsystem And Retire Legacy Paths

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_scene_image.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `readme.md`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`
- Test: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing test**

Add tests that require the new terminal subsystem to be the default path while preserving a rollback switch for the legacy stack during bring-up.

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke -- --nocapture`

Expected: FAIL because bootstrap/runtime profile still default to the legacy terminal subsystem.

**Step 3: Write minimal implementation**

Flip the default terminal subsystem, keep an escape hatch for rollback, and demote or remove obsolete presenter/native-surface paths from the mainline execution path.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/terminal_presenter.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_scene_image.rs src/app/runtime_profile.rs readme.md tests/bootstrap_smoke.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "feat: switch default terminal subsystem to new architecture"
```

