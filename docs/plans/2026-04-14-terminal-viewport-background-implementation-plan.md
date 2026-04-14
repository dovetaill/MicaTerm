# Terminal Viewport Background Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Remove visible terminal row banding and restyle the terminal viewport as a quiet whole-surface background, with a subtle dark-theme vertical gradient and a uniform light-theme fallback.

**Architecture:** Keep the existing terminal data transport and renderer structure intact, but stop consuming `row_bg_even` / `row_bg_odd` as viewport chrome. Instead, render the viewport background as one continuous background layer per backend, then continue drawing ANSI background runs, selection, glyphs, and cursor in their current order.

**Tech Stack:** Rust, Slint software atlas rendering, retained native Windows renderer, existing cargo test suite.

---

### Task 1: Lock the new background behavior in tests

**Files:**
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing assertions for the software atlas background**

Update the atlas renderer test so it no longer expects alternating row colors. Replace it with checks that:
- dark theme rows share the same quiet background model instead of even/odd row changes
- dark theme top and bottom samples differ only slightly, proving a subtle vertical treatment rather than row striping
- light theme rows share one uniform background when no light gradient is applied

**Step 2: Run the focused atlas test to verify it fails for the right reason**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_default_background_rows_use_subtle_band_colors -- --exact`
Expected: FAIL because the renderer still paints row banding.

**Step 3: Update contract tests that still describe row stripe consumption**

Change contract wording so tests keep the transport fields but stop requiring renderer-side viewport banding. The contract should now assert:
- row banding fields remain available in the model/presenter contract if needed for compatibility
- software/native viewport paint paths should not consume them as alternating viewport chrome

**Step 4: Run the focused contract tests to verify they fail before implementation**

Run:
- `cargo test --test theme_semantic_token_contract_spec shell_chrome_consumes_semantic_tokens_for_tabs_sidebar_inputs_and_pills -- --exact`
- `cargo test --test native_terminal_surface_contract_spec retained_native_frame_sources_expose_background_display_list_contract -- --exact`
- `cargo test --test terminal_renderer_dwrite_spec windows_backend_source_consumes_retained_background_and_monochrome_payloads -- --exact`
Expected: At least the viewport-background wording checks fail until implementation and contract updates are aligned.

**Step 5: Commit the test updates once they describe the new intended behavior**

```bash
git add tests/terminal_atlas_renderer_spec.rs tests/theme_semantic_token_contract_spec.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "test: lock terminal viewport background behavior"
```

### Task 2: Define the new background parameters in the theme layer

**Files:**
- Modify: `src/theme/spec.rs`

**Step 1: Add explicit viewport background constants/fields**

Extend the terminal palette spec with viewport background knobs that support:
- dark base / top / bottom colors
- light base / top / bottom colors
- row banding enabled flag
- row banding alpha
- grain alpha

Keep existing `default_bg`, `row_bg_even`, and `row_bg_odd` fields for compatibility.

**Step 2: Set the approved defaults**

Dark theme:
- base `#07111A`
- gradient top `#0A1621`
- gradient bottom `#07111A`
- banding disabled
- banding alpha `0.0`
- grain alpha `0.0`

Light theme:
- keep a quiet uniform base background
- set top/bottom equal if no approved light gradient is introduced in this pass
- banding disabled
- banding alpha `0.0`
- grain alpha `0.0`

**Step 3: Run the smallest relevant tests**

Run:
- `cargo test --test terminal_theme_selection_spec -q`
- `cargo test --test theme_semantic_token_contract_spec -q`
Expected: either pass immediately or fail only where renderer-side behavior is still pending.

**Step 4: Commit the theme spec update**

```bash
git add src/theme/spec.rs
git commit -m "feat: define terminal viewport background parameters"
```

### Task 3: Replace software atlas row banding with whole-surface background painting

**Files:**
- Modify: `src/app/terminal_atlas.rs`
- Modify: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Write the failing focused atlas test if not already red**

Run:
`cargo test --test terminal_atlas_renderer_spec atlas_renderer_default_background_rows_use_subtle_band_colors -- --exact`
Expected: FAIL before code changes.

**Step 2: Implement minimal software atlas background changes**

Update the atlas renderer so it:
- stops initializing/reseting the surface from `row_bg_even_rgba`
- stops selecting a per-row background via `row_background_rgba()` for viewport chrome
- paints one viewport-level background for the whole terminal image
- applies a very subtle dark-theme vertical gradient, or a uniform fallback when appropriate
- keeps ANSI cell backgrounds, selection overlays, glyph blits, and cursor-related behavior unchanged

**Step 3: Ensure empty cells no longer reintroduce row striping**

When a cell uses the terminal default background, it should inherit the viewport background model rather than a row-specific stripe color.

**Step 4: Run focused atlas tests**

Run:
- `cargo test --test terminal_atlas_renderer_spec -- --exact atlas_renderer_default_background_rows_use_subtle_band_colors`
- `cargo test --test terminal_atlas_renderer_spec -q`
Expected: PASS.

**Step 5: Commit the software atlas change**

```bash
git add src/app/terminal_atlas.rs tests/terminal_atlas_renderer_spec.rs
git commit -m "feat: quiet software terminal viewport background"
```

### Task 4: Replace native retained row banding with whole-rect background painting

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing focused contract tests if needed**

Run:
- `cargo test --test native_terminal_surface_contract_spec -q`
- `cargo test --test terminal_renderer_dwrite_spec -q`
Expected: FAIL only where native background painting still describes row banding.

**Step 2: Implement minimal native background changes**

Update `draw_background_runs()` so it:
- fills the full terminal clip rect once from the viewport base background
- optionally applies a subtle top-weighted or vertical gradient treatment when available
- stops iterating every grid row to paint alternating row background chrome
- still paints retained ANSI `background_runs` afterward
- preserves selection and glyph draw order

**Step 3: Remove dead row-background helper usage if it is no longer needed**

If `row_background_rect()` becomes unused after viewport-level background painting is introduced, delete it and update any related source contract tests.

**Step 4: Run focused native tests**

Run:
- `cargo test --test native_terminal_surface_contract_spec -q`
- `cargo test --test terminal_renderer_dwrite_spec -q`
Expected: PASS.

**Step 5: Commit the native renderer change**

```bash
git add src/app/terminal_renderer/platform/windows.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: quiet native terminal viewport background"
```

### Task 5: Full regression verification and cleanup

**Files:**
- Review: `src/theme/spec.rs`
- Review: `src/app/terminal_atlas.rs`
- Review: `src/app/terminal_renderer/platform/windows.rs`
- Review: `tests/terminal_atlas_renderer_spec.rs`
- Review: `tests/theme_semantic_token_contract_spec.rs`
- Review: `tests/native_terminal_surface_contract_spec.rs`
- Review: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Run focused regression checks around terminal surface behavior**

Run:
- `cargo test --test terminal_color_emoji_spec -q`
- `cargo test --test terminal_theme_selection_spec -q`
- `cargo test --test terminal_atlas_renderer_spec -q`

**Step 2: Run full project verification**

Run:
- `cargo test -q`
- `cargo check -q`
Expected: PASS. Note that `panic_logging` may emit expected child-process panic text while the overall suite still exits successfully.

**Step 3: Review the diff for accidental typography or cursor changes**

Run:
- `git diff -- src/theme/spec.rs src/app/terminal_atlas.rs src/app/terminal_renderer/platform/windows.rs`
Confirm only viewport background parameters/painting changed.

**Step 4: Commit the final integrated result if needed**

```bash
git add src/theme/spec.rs src/app/terminal_atlas.rs src/app/terminal_renderer/platform/windows.rs tests/terminal_atlas_renderer_spec.rs tests/theme_semantic_token_contract_spec.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: restyle terminal viewport background"
```
