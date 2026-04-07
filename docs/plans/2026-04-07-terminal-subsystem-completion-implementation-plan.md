# Terminal Subsystem Completion Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Close the gap between the planned terminal subsystem rearchitecture and the actual shipped repository state by fixing the packaged Windows blank-terminal regression, proving Catppuccin theme behavior end-to-end, and honestly deferring the real Alacritty core migration until after packaged correctness is verified.

**Architecture:** First harden the current WezTerm-backed runtime and packaged `scene-image` default so presenter failures cannot leave the terminal blank. Then make the real packaged presenter path testable, add diagnostics and theme verification around fallback behavior, and only after that resume the longer Alacritty migration as a separately gated track. Keep rollback and feature flags throughout.

**Tech Stack:** Rust, Slint, current `wezterm-term` integration, `termwiz`, `TerminalRendererHost`, Windows DirectWrite/Direct2D path, `build-win-x64.sh`, `cargo test`, shell smoke tests.

---

### Task 1: Lock The Blank-Terminal Failure Into A Failing Regression Test

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Reference: `src/app/terminal_renderer/host.rs`
- Reference: `src/app/terminal_presenter.rs`

**Step 1: Write the failing test**

Add a focused regression test that proves a presenter failure must not leave the workspace terminal host with no visible frame and no retry path.

Example Rust skeleton:

```rust
#[test]
fn presenter_render_failure_falls_back_to_bitmap_presenter() {
    // install a failing presenter into a host
    // render once
    // assert the fallback presenter is used for retry
    // assert a bitmap frame is returned instead of an unrecoverable blank result
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test presenter_render_failure_falls_back_to_bitmap_presenter --lib -- --exact --nocapture`

Expected: FAIL because bootstrap currently calls `host.present_surface_update(...)` directly and has no helper that downgrades and retries.

**Step 3: Write minimal implementation**

Add a small helper in `src/app/bootstrap.rs` that:

- calls `host.present_surface_update(...)`
- if it succeeds, returns the frame immediately
- if it fails, swaps the host presenter to `BitmapAtlasPresenter`
- reapplies raster scale
- retries once on the same surface and options
- only falls back to the old blank/error behavior if the retry also fails

Pseudocode:

```rust
fn present_surface_update_with_bitmap_fallback(...) -> Result<PresentedTerminalFrame> {
    match host.present_surface_update(surface, options) {
        Ok(frame) => Ok(frame),
        Err(first_err) => {
            log first_err;
            host.replace_presenter(BitmapAtlasPresenter::new()?, TerminalRenderMode::Bitmap);
            host.set_raster_scale(scale_factor);
            host.present_surface_update(surface, options)
        }
    }
}
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test presenter_render_failure_falls_back_to_bitmap_presenter --lib -- --exact --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs
git commit -m "fix: fallback to bitmap presenter after render failure"
```

### Task 2: Remove The Test-Only Masking That Hides Real Presenter Failures

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_renderer_host_spec.rs`

**Step 1: Write the failing test**

Add a regression test that proves test builds can exercise the same presenter-selection branch as real runtime code instead of always forcing `BitmapAtlasPresenter`.

Example Rust skeleton:

```rust
#[test]
fn tests_can_install_requested_workspace_presenter_path() {
    let source = std::fs::read_to_string("src/app/bootstrap.rs").unwrap();
    assert!(!source.contains("#[cfg(test)] fn ensure_workspace_terminal_presenter(...) { ... BitmapAtlasPresenter ... only"));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_host_spec tests_can_install_requested_workspace_presenter_path -- --exact`

Expected: FAIL because the current `#[cfg(test)] ensure_workspace_terminal_presenter(...)` always installs `BitmapAtlasPresenter`.

**Step 3: Write minimal implementation**

Replace the hard-coded test-only presenter install with a small injectable seam:

- keep production behavior unchanged
- let tests request either:
  - real presenter selection logic
  - a custom failing presenter
  - bitmap-only fallback when that is the scenario under test

Pseudocode:

```rust
thread_local! {
    static TEST_PRESENTER_FACTORY: RefCell<Option<Box<dyn Fn(...) -> Result<TerminalRendererHost>>>> = ...;
}

#[cfg(test)]
fn ensure_workspace_terminal_presenter(...) -> Result<()> {
    if let Some(factory) = TEST_PRESENTER_FACTORY { use it }
    else { use build_workspace_terminal_presenter(profile) }
}
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_renderer_host_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/terminal_renderer_host_spec.rs
git commit -m "test: stop masking real workspace presenter paths"
```

### Task 3: Make Packaged Fallback Selection Explicit In Runtime Diagnostics

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/terminal_renderer/host.rs`
- Modify: `readme.md`
- Test: `tests/bootstrap_profile_smoke.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing tests**

Add source/behavior assertions that require runtime diagnostics to spell out:

- selected terminal subsystem mode
- selected presenter mode
- fallback downgrade events
- whether packaged mainline is pinned to `scene-image`

Example Rust skeleton:

```rust
#[test]
fn runtime_documents_packaged_scene_image_default_and_runtime_fallbacks() {
    let source = std::fs::read_to_string("src/app/runtime_profile.rs").unwrap();
    assert!(source.contains("scene-image remains the default terminal subsystem"));
    assert!(source.contains("fallback"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_profile_smoke -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
```

Expected: FAIL because current diagnostics do not fully record fallback transitions and the documentation still overstates completion.

**Step 3: Write minimal implementation**

Add explicit logging and documentation fields for:

- requested subsystem
- active presenter mode before render
- fallback presenter after render failure
- packaged wrapper default and rollback override

Keep the runtime behavior unchanged except for the added observability.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_profile_smoke -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/runtime_profile.rs src/app/terminal_renderer/host.rs readme.md tests/bootstrap_profile_smoke.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "docs: make terminal fallback state explicit"
```

### Task 4: Prove Catppuccin Theme Behavior Through Fallback And No-Frame States

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/theme/spec.rs`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add tests that require:

- dark mode uses Catppuccin Mocha in normal and fallback states
- light mode uses Catppuccin Latte in normal and fallback states
- workspace session default fg/bg stay synchronized even when no presenter frame is available yet

Example Rust skeleton:

```rust
#[test]
fn fallback_terminal_state_uses_catppuccin_defaults() {
    let preset = preset_for_theme_mode(ThemeMode::Light);
    assert_eq!(preset.name, "Catppuccin Latte");
    // assert fallback/no-frame projection uses preset.background and preset.foreground
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test bootstrap_smoke workspace_terminal_theme_refresh_keeps_terminal_local_projection -- --exact
```

Expected: FAIL if fallback and no-frame states do not fully project the same palette and shell-adjacent colors.

**Step 3: Write minimal implementation**

Make the fallback and no-frame projection path use the same terminal theme preset source as the live surface path and ensure terminal host background/scrollbar/selection tokens stay synchronized.

Pseudocode:

```rust
let preset = preset_for_theme_mode(theme_mode);
window.set_workspace_session_default_bg(...preset.background...);
window.set_workspace_session_default_fg(...preset.foreground...);
window.set_workspace_session_cursor_bg(...preset.cursor_bg...);
window.set_workspace_session_cursor_fg(...preset.cursor_fg...);
```

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_theme.rs src/theme/spec.rs ui/theme/tokens.slint ui/app-window.slint ui/shell/terminal-session-host.slint tests/terminal_theme_selection_spec.rs tests/bootstrap_smoke.rs
git commit -m "fix: project Catppuccin theme through fallback terminal states"
```

### Task 5: Keep The Packaged Windows Default Honest Until Native Bring-Up Is Proven

**Files:**
- Modify: `build-win-x64.sh`
- Modify: `tests/build_win_x64_script_smoke.sh`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`
- Modify: `tests/runtime_profile.rs`

**Step 1: Write the failing tests**

Add or tighten assertions that require:

- `build-win-x64.sh` to document `scene-image` as the packaged default
- retained native surface to remain opt-in
- build/runtime docs to clearly separate "current shipped default" from "future target"

**Step 2: Run tests to verify they fail**

Run:

```bash
./tests/build_win_x64_script_smoke.sh
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
cargo test --test runtime_profile -- --nocapture
```

Expected: FAIL if any doc, script, or runtime assertion still implies the full subsystem switch is already done.

**Step 3: Write minimal implementation**

Update wrapper help text, runtime profile comments, and contract wording so the shipped state is described precisely:

- native text renderer path is present
- packaged terminal subsystem default is still `scene-image`
- retained native surface is a bring-up switch
- rollback remains intentional

**Step 4: Run tests to verify they pass**

Run:

```bash
./tests/build_win_x64_script_smoke.sh
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
cargo test --test runtime_profile -- --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add build-win-x64.sh tests/build_win_x64_script_smoke.sh tests/windows_native_text_renderer_contract_spec.rs tests/runtime_profile.rs
git commit -m "docs: keep packaged Windows terminal default honest"
```

### Task 6: Publish An Explicit Completion Audit So The Repo Stops Overclaiming

**Files:**
- Create: `docs/plans/2026-04-07-terminal-subsystem-completion-audit-and-corrective-design.md`
- Create: `docs/plans/2026-04-07-terminal-subsystem-completion-implementation-plan.md`
- Modify: `readme.md`

**Step 1: Write the failing documentation test**

Add a doc/source-level test or grep-based smoke check that requires the repository to document:

- WezTerm still being the real default core today
- Alacritty being experimental only
- Rio being a reference, not a migrated runtime dependency

Example shell skeleton:

```bash
rg -n "WezTerm.*default core|Alacritty.*experimental|Rio.*reference" readme.md docs/plans
```

**Step 2: Run test to verify it fails**

Run: `rg -n "WezTerm.*default core|Alacritty.*experimental|Rio.*reference" readme.md docs/plans`

Expected: FAIL because the current documents do not state the gap clearly enough.

**Step 3: Write minimal implementation**

Add the audit and corrective plan docs, then update the top-level docs/readme pointers so future work starts from the honest shipped state instead of the earlier optimistic framing.

**Step 4: Run checks to verify they pass**

Run:

```bash
rg -n "WezTerm.*default core|Alacritty.*experimental|Rio.*reference" readme.md docs/plans
```

Expected: PASS.

**Step 5: Commit**

```bash
git add docs/plans/2026-04-07-terminal-subsystem-completion-audit-and-corrective-design.md docs/plans/2026-04-07-terminal-subsystem-completion-implementation-plan.md readme.md
git commit -m "docs: audit terminal subsystem completion status"
```

### Task 7: Introduce A Real Alacritty Core Behind The Existing Adapter Boundary

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/app/terminal_core/alacritty_adapter.rs`
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `tests/terminal_core_parity_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing tests**

Add parity and source-level assertions that require the Alacritty adapter to stop wrapping the WezTerm adapter and instead bind to a real upstream-backed core implementation.

Example Rust skeleton:

```rust
#[test]
fn alacritty_adapter_no_longer_wraps_wezterm_adapter() {
    let source = std::fs::read_to_string("src/app/terminal_core/alacritty_adapter.rs").unwrap();
    assert!(!source.contains("inner: WeztermTerminalCoreAdapter"));
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_core_parity_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
```

Expected: FAIL because the current experimental adapter is only a wrapper seam.

**Step 3: Write minimal implementation**

Add a real Alacritty-backed core behind the existing trait boundary and keep it gated behind a feature flag or explicit runtime selection.

Pseudocode:

```rust
pub struct AlacrittyTerminalCoreAdapter {
    inner: RealAlacrittyCore,
}
```

Do not change the default core yet.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_core_parity_spec -- --nocapture
cargo test --test terminal_scrollback_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
```

Expected: PASS for the defined parity subset.

**Step 5: Commit**

```bash
git add Cargo.toml src/app/terminal_core/alacritty_adapter.rs src/app/terminal_core/mod.rs src/app/terminal_core/types.rs src/app/ssh/runtime/terminal.rs tests/terminal_core_parity_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_terminal_interaction_spec.rs
git commit -m "feat: add real alacritty terminal core adapter"
```

### Task 8: Decide Default Switch Only After Packaged Verification Matrix Passes

**Files:**
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `build-win-x64.sh`
- Modify: `readme.md`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing tests**

Add final switch-gate tests that require:

- packaged Windows verification matrix is green
- rollback switch still exists
- default switch only happens once both packaged correctness and parity requirements are met

**Step 2: Run test to verify it fails**

Run:

```bash
cargo test --test bootstrap_smoke -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
cargo test --test windows_native_text_renderer_contract_spec -- --nocapture
```

Expected: FAIL until the repository intentionally and explicitly meets the switch criteria.

**Step 3: Write minimal implementation**

Only after all previous tasks are green:

- decide whether the default switch should target retained native surface, a stabilized scene-image path, or a new Alacritty-backed path
- preserve a rollback environment variable
- update docs and build scripts in the same change

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
git add src/app/runtime_profile.rs src/app/bootstrap.rs build-win-x64.sh readme.md tests/bootstrap_smoke.rs tests/native_terminal_surface_contract_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "feat: switch terminal default after packaged verification"
```

### Task 9: Reuse Scrollback Row Shaping Work In The Packaged Scene-Image Path

**Files:**
- Modify: `src/app/terminal_model.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/terminal_model_spec.rs`
- Modify: `tests/terminal_runtime_perf_contract_spec.rs`

**Step 1: Write the failing tests**

Add coverage that proves adjacent viewport shifts do not reshape every visible row from scratch.

Recommended checks:

- a model-level test that tracks row content identity across viewport shifts
- a presenter-level or contract test that requires reuse of previously shaped rows when the visible viewport scrolls by a small delta

Example Rust skeleton:

```rust
#[test]
fn prepare_native_terminal_frame_reuses_shaped_rows_for_overlapping_scrollback_rows() {
    // render one visible viewport
    // shift the viewport by one line
    // assert only newly exposed rows need fresh shaping work
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_model_spec -- --nocapture
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
cargo test prepare_native_terminal_frame_reuses_shaped_rows_for_overlapping_scrollback_rows --lib -- --exact --nocapture
```

Expected: FAIL because the current presenter hot path still reshapes every visible row on each scroll update.

**Step 3: Write minimal implementation**

Add a small reuse seam in the presenter path:

- derive a row-content hash that stays stable when identical row content slides to a new viewport row
- keep only the previous frame's shaped rows as a reuse cache
- rebase reused shaped rows onto the current viewport row index
- clear the cache whenever the loaded font changes

Do not claim full dirty-region rendering yet. The goal here is to remove the obvious repeated shaping work in the shipped path.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_model_spec -- --nocapture
cargo test --test terminal_runtime_perf_contract_spec -- --nocapture
cargo test prepare_native_terminal_frame_reuses_shaped_rows_for_overlapping_scrollback_rows --lib -- --exact --nocapture
```

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_model.rs src/app/terminal_presenter.rs tests/terminal_model_spec.rs tests/terminal_runtime_perf_contract_spec.rs
git commit -m "perf: reuse shaped rows across scrollback viewport shifts"
```

### Task 10: Tune Windows Terminal Typography For Dense CJK And Mixed Rows

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/windows_terminal_typography_defaults_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing tests**

Add focused typography assertions that require:

- slightly looser vertical rhythm for dense terminal text
- stable baseline alignment for mixed Chinese / Latin / Nerd Font rows
- scene-image/native presenter defaults to use the same typography contract

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test windows_terminal_typography_defaults_spec -- --nocapture
cargo test --test terminal_renderer_dwrite_spec -- --nocapture
cargo test --test terminal_color_emoji_spec -- --nocapture
```

Expected: FAIL until the typography defaults and verification contracts are tightened.

**Step 3: Write minimal implementation**

Tune the existing DirectWrite-backed defaults rather than introducing a new renderer:

- adjust the shared terminal font metrics contract where needed
- keep scene-image/native presenter font requests aligned
- preserve emoji / fallback / baseline safety

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_presenter.rs tests/windows_terminal_typography_defaults_spec.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: polish Windows terminal typography defaults"
```

### Task 11: Make Catppuccin Visible In Terminal-Adjacent Shell Chrome

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/theme/spec.rs`
- Modify: `ui/theme/tokens.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

Add checks that require terminal-adjacent shell chrome to read from the same Catppuccin preset source in:

- scrollbar thumb / active thumb states
- paused-follow affordance
- fallback / no-frame states

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_theme_selection_spec -- --nocapture
cargo test --test bootstrap_smoke -- --nocapture
```

Expected: FAIL if the shell chrome still carries detached terminal-adjacent token values.

**Step 3: Write minimal implementation**

Wire the shell-adjacent tokens to the same preset-backed theme source already used by terminal fg/bg/cursor/selection defaults.

**Step 4: Run tests to verify they pass**

Run the same commands from Step 2.

Expected: PASS.

**Step 5: Commit**

```bash
git add src/app/terminal_theme.rs src/theme/spec.rs ui/theme/tokens.slint ui/shell/terminal-session-host.slint tests/terminal_theme_selection_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: project Catppuccin through terminal shell chrome"
```
