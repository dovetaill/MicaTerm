# Terminal Memory Glyph Cache Cap And Surface Refresh De-Noise Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Bound native terminal glyph caches so scroll-heavy sessions stop growing without limit, and stop writing duplicate `surface-refresh` diagnostics when neither the seqno nor cache stats changed.

**Architecture:** Add bounded recency-aware cache management inside the native terminal renderer so atlas slots and raster caches stop growing unbounded while still reusing hot glyphs. Add a small workspace-local surface-refresh emission guard in bootstrap so memory diagnostics only log refreshes when a visible state change actually occurred.

**Tech Stack:** Rust, tracing, Slint workspace bootstrap, native terminal renderer tests

---

### Task 1: Guard the `surface-refresh` diagnostics

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `src/app/bootstrap.rs`

**Step 1: Write the failing test**

Add unit tests for a helper that:
- emits on the first refresh
- suppresses an identical refresh
- emits again when `seqno` changes
- emits again when cache stats change

**Step 2: Run test to verify it fails**

Run: `cargo test surface_refresh_logging --lib -q`
Expected: FAIL because the helper does not exist yet.

**Step 3: Write minimal implementation**

Add a tiny workspace-local snapshot struct plus a helper that compares:
- `session_id`
- `render_mode`
- `seqno`
- `TerminalPresenterCacheStats`

Only call `emit_terminal_memory_surface_refresh(...)` when the snapshot changed.

**Step 4: Run test to verify it passes**

Run: `cargo test surface_refresh_logging --lib -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs
git commit -m "fix: de-noise repeated terminal surface refresh diagnostics"
```

### Task 2: Cap native terminal glyph caches

**Files:**
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Test: `tests/terminal_renderer_prepare_cache_spec.rs`

**Step 1: Write the failing test**

Add tests that:
- drive more unique glyphs than the configured cap
- assert atlas / raster / color cache counts never exceed their caps
- assert the renderer still keeps a bounded prepared-row cache after eviction pressure

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_prepare_cache_spec -q`
Expected: FAIL because glyph caches currently grow monotonically.

**Step 3: Write minimal implementation**

Implement bounded recency-aware caches:
- atlas entries use `HashMap + VecDeque`
- glyph raster cache uses `HashMap + VecDeque`
- color glyph cache uses `HashMap + VecDeque`
- reuse freed slots for atlas/color glyph cache entries
- invalidate prepared-row reuse state whenever atlas/color eviction occurs

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_prepare_cache_spec -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_renderer_prepare_cache_spec.rs
git commit -m "fix: bound native terminal glyph caches"
```

### Task 3: Verify diagnostics and renderer contracts together

**Files:**
- Modify: `tests/logging_runtime.rs`

**Step 1: Write the failing test**

Add/adjust diagnostics coverage so repeated identical `surface-refresh` snapshots no longer produce repeated log lines while changed snapshots still do.

**Step 2: Run test to verify it fails**

Run: `cargo test --test logging_runtime -q`
Expected: FAIL until the de-noise behavior matches the new contract.

**Step 3: Write minimal implementation**

Keep the runtime logging emitters unchanged except for the new calling pattern, and update the contract assertions to match the de-noised behavior.

**Step 4: Run test to verify it passes**

Run: `cargo test --test logging_runtime -q`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/logging_runtime.rs
git commit -m "test: cover de-noised terminal memory diagnostics"
```

### Task 4: Final verification

**Files:**
- Reference: `tests/terminal_memory_diagnostics_contract_spec.rs`
- Reference: `tests/terminal_renderer_prepare_cache_spec.rs`
- Reference: `tests/logging_runtime.rs`

**Step 1: Run focused verification**

Run:

```bash
cargo test --lib surface_refresh_logging -q
cargo test --test terminal_renderer_prepare_cache_spec -q
cargo test --test logging_runtime -q
cargo test --test terminal_memory_diagnostics_contract_spec -q
```

Expected: PASS

**Step 2: Run broader terminal regression coverage**

Run:

```bash
cargo test --test terminal_runtime_perf_contract_spec --test bootstrap_smoke --test startup_font_memory_regression -q
```

Expected: PASS
