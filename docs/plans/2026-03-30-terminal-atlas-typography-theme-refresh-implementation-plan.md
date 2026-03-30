# Terminal Atlas Typography Theme Refresh Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the atlas-rendered terminal read sharper, tighter, and closer to the reference terminal screenshots without abandoning the current Sarasa-based renderer.

**Architecture:** Keep `src/app/terminal_atlas.rs` as the single text rasterization path, but swap the atlas font weight back to regular, retune glyph metrics/alpha shaping, project row band colors from `src/app/terminal_theme.rs`, and tighten host-side padding in `ui/shell/terminal-session-host.slint`. Tests lock the font asset contract, renderer metrics, row banding, and palette defaults before implementation.

**Tech Stack:** Rust, Slint, `ab_glyph`, existing atlas renderer tests

---

### Task 1: Lock The Refreshed Font Asset Contract

**Files:**
- Modify: `tests/startup_font_memory_regression.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `build.rs`
- Reference: `src/app/terminal_atlas.rs`

**Step 1: Write the failing test**

Tighten the font contract tests so they expect the regular Sarasa atlas asset instead of the stale unhinted-only contract.

**Step 2: Run test to verify it fails**

Run: `cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q`

Expected: FAIL because the current contract still points at the old filename assumptions.

**Step 3: Write minimal implementation**

Update the asset references in `src/app/terminal_atlas.rs`, `build.rs`, and the test expectations so they all agree on the refreshed bundled font file.

**Step 4: Run test to verify it passes**

Run: `cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke -q`

Expected: PASS

**Step 5: Commit**

```bash
git add tests/startup_font_memory_regression.rs tests/terminal_font_registration_smoke.rs build.rs src/app/terminal_atlas.rs
git commit -m "fix: align terminal font asset contract"
```

### Task 2: Lock Sharper Metrics And Default Row Bands

**Files:**
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `src/app/terminal_atlas.rs`
- Reference: `src/app/terminal_theme.rs`

**Step 1: Write the failing test**

Add or tighten renderer coverage so it proves:
- refreshed metrics stay compact but coherent;
- default-background rows alternate between two background shades;
- explicit colored cells still override the row band beneath them.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image -q`

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_default_background_rows_use_subtle_band_colors -q`

Expected: FAIL because the current renderer still uses the previous metrics and has no banded row logic.

**Step 3: Write minimal implementation**

Retune the atlas constants and add default-row band painting that only applies to cells using the default background.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image -q`

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_default_background_rows_use_subtle_band_colors -q`

Expected: PASS

**Step 5: Commit**

```bash
git add tests/terminal_atlas_renderer_spec.rs src/app/terminal_atlas.rs src/app/terminal_theme.rs
git commit -m "fix: retune atlas terminal typography"
```

### Task 3: Lock Theme Projection And Host Layout

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Write the failing test**

Add theme coverage for the refreshed dark/light defaults and ensure the host layout constants are updated in the implementation diff only after the palette contract is locked.

**Step 2: Run test to verify it fails**

Run the narrow terminal theme test command for the new assertions.

Expected: FAIL because the current palette still uses the older surface defaults and has no explicit row band colors.

**Step 3: Write minimal implementation**

Refresh dark/light palette defaults, selection colors, row band colors, and host padding/scrollbar reservations.

**Step 4: Run test to verify it passes**

Run the narrow terminal theme test command again.

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_theme.rs ui/shell/terminal-session-host.slint tests/terminal_atlas_renderer_spec.rs
git commit -m "fix: refresh terminal theme surfaces"
```

### Task 4: Full Verification

**Files:**
- Reference: `src/app/terminal_atlas.rs`
- Reference: `src/app/terminal_theme.rs`
- Reference: `ui/shell/terminal-session-host.slint`
- Reference: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Run focused tests**

```bash
cargo test --test terminal_atlas_renderer_spec --test startup_font_memory_regression --test terminal_font_registration_smoke -q
```

Expected: PASS

**Step 2: Run compile verification**

```bash
cargo check
```

Expected: PASS

**Step 3: Review diff**

```bash
git diff -- src/app/terminal_atlas.rs src/app/terminal_theme.rs ui/shell/terminal-session-host.slint tests/terminal_atlas_renderer_spec.rs tests/startup_font_memory_regression.rs tests/terminal_font_registration_smoke.rs build.rs readme.md
```

Expected: only terminal typography/theme refresh changes relevant to this task
