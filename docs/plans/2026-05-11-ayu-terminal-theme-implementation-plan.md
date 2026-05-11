# Ayu Terminal Theme Migration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current default terminal dark/light palette with Termius-like Ayu Dark / Ayu Light and make runtime, fallback, Slint terminal neighborhood, and renderer paths consume one preset family.

**Architecture:** Keep `src/theme/spec.rs` as the terminal palette truth, project that palette through `src/app/terminal_theme.rs`, and extend the bootstrap-to-Slint session contract so terminal-neighborhood surfaces stop relying on detached token hardcodes. Use flat viewport backgrounds for the default Ayu preset to eliminate bitmap/native background divergence.

**Tech Stack:** Rust, Slint, existing `wezterm-term` adapter, bootstrap workspace projection, bitmap atlas presenter, native terminal renderer, Rust source-contract tests.

---

### Task 1: Freeze Ayu preset expectations in tests

**Files:**
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ui_preferences.rs`
- Reference: `src/app/terminal_theme.rs`
- Reference: `src/theme/spec.rs`

**Step 1: Write the failing tests**

Add or update assertions that require:

- dark preset name is `Ayu Dark`
- light preset name is `Ayu Light`
- dark bg/fg match the chosen Ayu Dark values
- light bg/fg match the chosen Ayu Light values
- cursor bg/fg and selection values come from the preset
- ANSI anchors `0`, `7`, `8`, `15` stay fixed for dark and light
- fallback/no-surface fg/bg continue to match the preset after theme toggles

Example skeleton:

```rust
#[test]
fn dark_theme_maps_to_ayu_dark() {
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    assert_eq!(preset.name, "Ayu Dark");
    assert_eq!(preset.background, 0x0a_0e14);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test terminal_theme
cargo test ui_preferences
cargo test bootstrap_smoke
```

Expected: FAIL because the current defaults still describe the old preset family.

**Step 3: Add only the assertions needed to lock the Ayu target contract**

Keep the new coverage focused on names, fg/bg, cursor, selection, and ANSI anchors. Do not rewrite unrelated test intent.

**Step 4: Run the focused tests again**

Run:

```bash
cargo test terminal_theme -- --nocapture
```

Expected: FAIL for missing Ayu values, but fail for the correct reasons.

**Step 5: Commit**

```bash
git add tests/terminal_theme_selection_spec.rs tests/bootstrap_smoke.rs tests/ui_preferences.rs
git commit -m "test: freeze ayu terminal preset targets"
```

### Task 2: Replace the default terminal preset in the Rust theme spec

**Files:**
- Modify: `src/theme/spec.rs`
- Modify: `src/app/terminal_theme.rs`
- Test: `tests/terminal_theme_selection_spec.rs`

**Step 1: Write the failing test for the shared preset source**

Add a focused test that requires `preset_for_theme_mode()` to resolve to the same Ayu values as the explicit premium-default preset.

```rust
#[test]
fn default_theme_mode_wrapper_points_at_ayu_default() {
    let wrapped = preset_for_theme_mode(ThemeMode::Dark);
    let explicit = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    assert_eq!(wrapped.name, explicit.name);
    assert_eq!(wrapped.background, explicit.background);
}
```

**Step 2: Run the focused test to confirm failure**

Run:

```bash
cargo test terminal_theme_selection_spec default_theme_mode_wrapper_points_at_ayu_default -- --exact
```

Expected: FAIL until the theme spec is replaced.

**Step 3: Write the minimal implementation**

In `src/theme/spec.rs`:

- replace the `PremiumDefault` terminal names with `Ayu Dark` / `Ayu Light`
- replace default bg/fg, cursor, selection, ANSI, and scrollbar values with Ayu values
- set default Ayu viewport background endpoints equal to the chosen flat background values
- keep `ThemeMode` semantics unchanged

In `src/app/terminal_theme.rs`:

- preserve the projection helpers
- only adjust fields if the new preset data requires additional projected surfaces later in the plan

**Step 4: Run the focused tests to verify they pass**

Run:

```bash
cargo test terminal_theme -- --nocapture
```

Expected: PASS for the Rust-side preset mapping tests.

**Step 5: Commit**

```bash
git add src/theme/spec.rs src/app/terminal_theme.rs tests/terminal_theme_selection_spec.rs
git commit -m "feat: switch default terminal preset to ayu"
```

### Task 3: Extend the bootstrap-to-Slint contract for terminal-neighborhood colors

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing contract tests**

Add source-contract assertions requiring first-class workspace session properties for:

- terminal selection surface
- terminal scrollbar track
- terminal frame / host background if it remains terminal-scoped

Example skeleton:

```rust
#[test]
fn terminal_session_host_reads_selection_and_scrollbar_track_from_session_contract() {
    let host = std::fs::read_to_string("ui/shell/terminal-session-host.slint").unwrap();
    assert!(host.contains("session-selection-surface"));
    assert!(host.contains("session-scrollbar-track"));
}
```

**Step 2: Run the focused tests to verify they fail**

Run:

```bash
cargo test bootstrap_smoke terminal_shell_chrome_contract -- --exact
cargo test native_terminal_surface_contract_spec bitmap_host_selection_source_exposes_local_overlay_contract -- --exact
```

Expected: FAIL because the contract does not exist yet.

**Step 3: Write the minimal implementation**

- add new `workspace-session-*` properties in `ui/app-window.slint`
- thread them through `ui/shell/workspace-pane.slint`
- make `ui/shell/terminal-session-host.slint` consume session-scoped values instead of `ThemeTokens` for selection and scrollbar track
- in `src/app/bootstrap.rs`, project the values from the active preset in both active-surface and no-surface fallback paths

**Step 4: Run the focused tests to verify they pass**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
cargo test native_terminal_surface_contract_spec -- --nocapture
```

Expected: PASS for the new terminal-neighborhood contract checks.

**Step 5: Commit**

```bash
git add ui/app-window.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: project ayu terminal neighborhood into slint contract"
```

### Task 4: Sync Slint token defaults to the Ayu premium-default preset

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`

**Step 1: Write the failing token-parity tests**

Require token defaults to match the Ayu premium-default preset for boot-time parity:

- terminal default fg/bg
- terminal cursor fg/bg
- terminal selection surface
- terminal scrollbar track/thumb/active
- terminal frame background if still defined as a token default

**Step 2: Run the focused tests to confirm failure**

Run:

```bash
cargo test terminal_theme_selection_spec slint_terminal_tokens_match_shared_no_frame_defaults -- --exact
cargo test ui_preferences shell_terminal_tokens_stay_synced_to_theme_backed_terminal_palette_contract -- --exact
```

Expected: FAIL until the token ladder is updated.

**Step 3: Write the minimal implementation**

Update `ui/theme/tokens.slint` so the boot defaults reflect the Ayu premium-default preset values. Keep these as startup parity defaults, not as the active terminal-neighborhood truth once bootstrap projection runs.

**Step 4: Run the focused tests again**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test ui_preferences -- --nocapture
cargo test theme_semantic_token_contract_spec -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint tests/terminal_theme_selection_spec.rs tests/ui_preferences.rs tests/theme_semantic_token_contract_spec.rs
git commit -m "style: align slint ayu terminal token defaults"
```

### Task 5: Verify runtime, bitmap, native, and fallback paths stay on the same Ayu family

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Reference: `src/app/terminal_core/wezterm_adapter.rs`
- Reference: `src/app/terminal_presenter.rs`
- Reference: `src/app/terminal_renderer/platform/windows.rs`

**Step 1: Write the failing verification coverage**

Add or update tests that assert:

- runtime theme changes still reproject Ayu fg/bg/cursor correctly
- bitmap and native paths keep the same default background values
- fallback/no-frame paths use the same preset family as active runtime paths
- no test description or assertion message still claims Catppuccin / Mica Graphite / Mica Canvas as the default terminal preset

**Step 2: Run the focused tests to confirm the gaps**

Run:

```bash
cargo test terminal_session -- --nocapture
cargo test ssh_terminal_interaction -- --nocapture
cargo test bootstrap_smoke -- --nocapture
```

Expected: FAIL if any path still assumes the old preset family or detached fallback values.

**Step 3: Make only the minimal fixes needed**

- update stale assertion text
- adjust any fallback-path projection that still assumes the old default values
- if a native/background mismatch remains, resolve it without introducing a second palette source

**Step 4: Run the focused verification again**

Run:

```bash
cargo test terminal_theme
cargo test ui_preferences
cargo test bootstrap_smoke
cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/terminal_session_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "test: verify ayu terminal theme across runtime and fallback paths"
```

### Task 6: Audit leftovers and remove half-migrated terminology

**Files:**
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Reference: `src/app/ssh/runtime/contracts.rs`
- Reference: `src/app/terminal_renderer/platform/windows_child_host.rs`

**Step 1: Write the failing audit checks**

Add source checks or assertion updates that detect:

- stale `Catppuccin` wording in terminal-default tests
- stale `Mica Graphite` / `Mica Canvas` expectations in terminal-default tests
- any remaining terminal-default helper that hides old white/black/green fallback assumptions

**Step 2: Run the audit tests**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
cargo test ui_preferences -- --nocapture
```

Expected: FAIL until stale wording and fallback assumptions are removed.

**Step 3: Apply the minimal cleanup**

- rename outdated assertion messages
- note any intentionally retained historical fallback helper that is test-only and not a product path
- leave non-default variants alone unless they break Ayu parity tests

**Step 4: Run the final suite**

Run:

```bash
cargo fmt
cargo test terminal_theme
cargo test ui_preferences
cargo test bootstrap_smoke
cargo test
```

Expected: PASS.

**Step 5: Commit**

```bash
git add tests/bootstrap_smoke.rs tests/ui_preferences.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "chore: finish ayu terminal theme migration audit"
```
