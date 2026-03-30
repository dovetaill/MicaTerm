# Terminal Atlas Selection And Visual Polish Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make atlas-rendered terminal selection visible and improve terminal text readability without abandoning the single-image renderer.

**Architecture:** Selection state will be projected from the Slint host into the Rust atlas renderer during workspace sync. The renderer will include selection in dirty-row hashing, paint selection fills in-image, and tune glyph sizing and alpha mapping for clearer Sarasa output.

**Tech Stack:** Rust, Slint, `ab_glyph`, existing workspace terminal bootstrap wiring

---

### Task 1: Lock Selection Rendering Behavior

**Files:**
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `src/app/terminal_atlas.rs`

**Step 1: Write the failing test**

Add a renderer test that:
- renders a surface without selection
- renders the same surface with a selected cell range
- asserts that the second frame rerenders the affected row
- asserts that pixel output differs in the selected cell area

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_selection_changes_invalidate_and_repaint_rows -q`

Expected: FAIL because the renderer has no selection input and selection does not affect output.

**Step 3: Write minimal implementation**

Introduce selection-aware renderer input and row hashing.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_selection_changes_invalidate_and_repaint_rows -q`

Expected: PASS

**Step 5: Commit**

Run:

```bash
git add tests/terminal_atlas_renderer_spec.rs src/app/terminal_atlas.rs src/app/bootstrap.rs
git commit -m "fix: render atlas terminal selection"
```

### Task 2: Lock Larger And Clearer Metrics

**Files:**
- Modify: `tests/terminal_atlas_renderer_spec.rs`
- Reference: `src/app/terminal_atlas.rs`

**Step 1: Write the failing test**

Add expectations that the default metrics increase slightly from the current baseline while staying compact enough for a terminal grid.

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image -q`

Expected: FAIL once the expectations are tightened to the new target metrics.

**Step 3: Write minimal implementation**

Tune font size, baseline, padding, and alpha mapping until the updated expectations pass.

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_atlas_renderer_spec atlas_renderer_loads_sarasa_metrics_and_emits_a_surface_image -q`

Expected: PASS

**Step 5: Commit**

Run:

```bash
git add tests/terminal_atlas_renderer_spec.rs src/app/terminal_atlas.rs
git commit -m "fix: tune atlas terminal glyph metrics"
```

### Task 3: Verify Bootstrap Wiring

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Write the failing test**

If needed, add bootstrap-level coverage that selection state changes force a refreshed surface image.

**Step 2: Run test to verify it fails**

Run the narrow test command for the new coverage.

Expected: FAIL before the wiring change.

**Step 3: Write minimal implementation**

Project active selection bounds from the window into atlas render input during workspace session sync.

**Step 4: Run test to verify it passes**

Run the narrow test command again.

Expected: PASS

**Step 5: Commit**

Run:

```bash
git add src/app/bootstrap.rs tests/bootstrap_smoke.rs tests/terminal_atlas_renderer_spec.rs
git commit -m "test: lock atlas selection refresh wiring"
```

### Task 4: Full Verification

**Files:**
- Reference: `src/app/terminal_atlas.rs`
- Reference: `src/app/bootstrap.rs`
- Reference: `tests/terminal_atlas_renderer_spec.rs`

**Step 1: Run focused tests**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test bootstrap_smoke -q
```

Expected: PASS

**Step 2: Run compile verification**

Run:

```bash
cargo check
```

Expected: PASS

**Step 3: Review diff**

Run:

```bash
git diff -- src/app/terminal_atlas.rs src/app/bootstrap.rs tests/terminal_atlas_renderer_spec.rs
```

Expected: only selection/render-quality changes relevant to this task
