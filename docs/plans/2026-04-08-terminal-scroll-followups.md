# Terminal Scroll Follow-Ups Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Reduce remaining terminal scroll stutter by reusing shaped rows across larger viewport jumps and by exposing scene-image fallback reasons in perf logs.

**Architecture:** Extend the presenter-side row-shaping cache from previous-frame overlap reuse to a bounded row-content LRU keyed by stable row hashes and font identity, then thread scene-image fallback reasons through render diagnostics so perf logs explain why incremental scroll was not used. Keep changes local to the terminal presenter and scene-image renderer so the current renderer-side prepare cache remains intact.

**Tech Stack:** Rust, terminal presenter/layout/model pipeline, scene-image bitmap renderer, cargo tests.

---

### Task 1: Inspect and lock shaped-row cache behavior

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_runtime_perf_contract_spec.rs`

**Step 1: Write failing/contract test**
Add a source-level or unit assertion that presenter-side shaping is no longer limited to only `previous_frame` overlap reuse.

**Step 2: Run targeted test**
Run: `cargo test --test terminal_runtime_perf_contract_spec -- --nocapture`
Expected: contract fails before implementation.

**Step 3: Implement minimal cache plumbing**
Add a bounded shaped-row cache structure owned by presenters and thread it through native frame preparation.

**Step 4: Re-run test**
Run: `cargo test --test terminal_runtime_perf_contract_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add src/app/terminal_presenter.rs tests/terminal_runtime_perf_contract_spec.rs
git commit -m "Reuse shaped terminal rows across viewport jumps"
```

### Task 2: Cover large scroll jumps and reverse scroll reuse

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Test: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_scrollback_spec.rs`

**Step 1: Write failing tests**
Add presenter tests for large positive/negative viewport jumps that should reuse row shaping by content hash even when rows fall outside the previous viewport.

**Step 2: Run tests to verify failure**
Run: `cargo test --lib app::terminal_presenter::tests -- --nocapture`
Expected: FAIL before implementation.

**Step 3: Implement LRU reuse**
Use row content hash plus row styling/font identity to store/retrieve rebased `ShapedRow` values across frames with bounded memory.

**Step 4: Run tests to verify pass**
Run: `cargo test --lib app::terminal_presenter::tests -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add src/app/terminal_presenter.rs tests/terminal_scrollback_spec.rs
git commit -m "Cache shaped rows across scrollback jumps"
```

### Task 3: Diagnose scene-image incremental-scroll fallbacks

**Files:**
- Modify: `src/app/terminal_scene_image.rs`
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_scene_image_spec.rs`
- Test: `tests/terminal_runtime_perf_contract_spec.rs`

**Step 1: Write failing tests**
Add a scene-image test and/or source contract requiring a fallback reason field when reuse mode becomes `full-base-raster`.

**Step 2: Run tests to verify failure**
Run: `cargo test --test terminal_scene_image_spec -- --nocapture`
Expected: FAIL before implementation.

**Step 3: Implement diagnostics**
Thread a reason enum/string through incremental-scroll detection and render diagnostics, and log it from presenter perf output.

**Step 4: Re-run tests**
Run: `cargo test --test terminal_scene_image_spec -- --nocapture`
Expected: PASS.

**Step 5: Commit**
```bash
git add src/app/terminal_scene_image.rs src/app/terminal_presenter.rs tests/terminal_scene_image_spec.rs tests/terminal_runtime_perf_contract_spec.rs
git commit -m "Log scene-image scroll fallback reasons"
```

### Task 4: Verify end-to-end terminal perf diagnostics

**Files:**
- Modify: `tests/native_terminal_surface_damage_rect_spec.rs`
- Test: `tests/terminal_runtime_perf_contract_spec.rs`
- Test: `tests/terminal_scene_image_spec.rs`
- Test: `tests/native_terminal_surface_damage_rect_spec.rs`
- Test: `tests/terminal_renderer_prepare_cache_spec.rs`
- Test: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Run full targeted verification suite**
Run:
- `cargo test --test terminal_runtime_perf_contract_spec -- --nocapture`
- `cargo test --lib app::terminal_presenter::tests -- --nocapture`
- `cargo test --test terminal_scene_image_spec -- --nocapture`
- `cargo test --test native_terminal_surface_damage_rect_spec -- --nocapture`
- `cargo test --test terminal_renderer_prepare_cache_spec -- --nocapture`
- `cargo test --test terminal_renderer_dwrite_spec -- --nocapture`

Expected: all pass.

**Step 2: Commit**
```bash
git add tests/native_terminal_surface_damage_rect_spec.rs
git commit -m "Verify terminal scroll follow-up diagnostics"
```
