# Terminal Backend Purge Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Lower post-tab-close steady-state memory by purging Slint/Skia backend caches on the existing window instead of only clearing terminal-side caches and trimming the process working set.

**Architecture:** Keep the current terminal idle/no-surface shrink flow, but add one extra backend-purge step that targets the live Slint winit/Skia renderer. Expose a narrow vendor hook from `i-slint-backend-winit` into `i-slint-renderer-skia`, keep it a no-op for non-Skia renderers, and use a test hook in bootstrap so the no-surface idle path can be verified without requiring a real Windows graphics device.

**Tech Stack:** Rust, Slint winit backend, Skia renderer, Windows D3D/Skia integration, cargo tests.

---

### Task 1: Add a failing bootstrap-level purge contract test

**Files:**
- Modify: `src/app/bootstrap.rs`

**Step 1: Write the failing test**
- Add a test near the existing idle-shrink tests asserting that once the workspace stays without an active terminal surface past the idle threshold, bootstrap triggers a new backend-purge hook in addition to terminal cache clear / renderer-host release.

**Step 2: Run test to verify it fails**
- Run: `cargo test --lib idle_cache_shrink_requests_backend_purge_after_no_surface_threshold -- --exact`
- Expected: FAIL because no backend-purge hook exists yet.

**Step 3: Add a test-only hook scaffold**
- Add a `#[cfg(test)]` hook in `src/app/bootstrap.rs` for backend purge requests, but do not implement production wiring yet.

**Step 4: Run test to verify it still fails for the right reason**
- Run the same command.
- Expected: FAIL because production code does not call the hook yet.

**Step 5: Commit**
- Commit message: `test: cover no-surface backend purge contract`

### Task 2: Vendor and patch the Skia renderer crate with a purge API

**Files:**
- Create: `vendor/i-slint-renderer-skia/**`
- Modify: `Cargo.toml`
- Modify: `vendor/i-slint-renderer-skia/lib.rs`
- Modify: `vendor/i-slint-renderer-skia/d3d_surface.rs`

**Step 1: Vendor the crate**
- Copy `i-slint-renderer-skia-1.15.1` from the local cargo registry into `vendor/i-slint-renderer-skia`.

**Step 2: Patch Cargo**
- Add `[patch.crates-io] i-slint-renderer-skia = { path = "vendor/i-slint-renderer-skia" }`.

**Step 3: Write minimal purge API in renderer**
- Add a `pub fn purge_memory_resources(&self) -> Result<(), PlatformError>` on `SkiaRenderer` that:
  - clears `image_cache`, `path_cache`, and `layer_cache`
  - calls `surface.purge_memory_resources()` if a surface exists
  - calls Skia global cache purge (`skia_safe::graphics::purge_all_caches()`)

**Step 4: Extend the surface trait**
- Add `fn purge_memory_resources(&self) -> Result<(), PlatformError> { Ok(()) }` to the renderer surface trait.

**Step 5: Implement the D3D surface purge**
- In `d3d_surface.rs`, add the minimal safe purge path on the live swapchain-backed context:
  - synchronize pending GPU work if needed
  - call `gr_context.perform_deferred_cleanup(Duration::ZERO, None)`
  - avoid destroying the swapchain/window; keep the renderer usable
- Keep this intentionally conservative; do not tear down the live window surface in this task.

**Step 6: Make suspend reuse the purge helper**
- Update `SkiaRenderer::suspend()` to call the new purge helper before `clear_surface()`.

**Step 7: Run focused checks**
- Run: `cargo check`
- Expected: PASS

**Step 8: Commit**
- Commit message: `feat: add skia backend memory purge hook`

### Task 3: Expose the purge hook through the vendored winit backend

**Files:**
- Modify: `vendor/i-slint-backend-winit/lib.rs`
- Modify: `vendor/i-slint-backend-winit/renderer/skia.rs`
- Modify: `vendor/i-slint-backend-winit/winitwindowadapter.rs` (only if helper placement requires it)

**Step 1: Add a renderer-trait hook**
- Extend `WinitCompatibleRenderer` with a default no-op method such as `purge_memory_resources()`.

**Step 2: Implement the Skia renderer side**
- In `vendor/i-slint-backend-winit/renderer/skia.rs`, forward the new trait method to `self.renderer.purge_memory_resources()`.

**Step 3: Expose window-level API**
- Add a small public extension trait or helper on `i_slint_core::api::Window` in `vendor/i-slint-backend-winit/lib.rs` that downcasts to `WinitWindowAdapter` and invokes `adapter.renderer().purge_memory_resources()`.
- Keep behavior no-op or `Ok(())` when the window is not backed by winit.

**Step 4: Run focused checks**
- Run: `cargo check`
- Expected: PASS

**Step 5: Commit**
- Commit message: `feat: expose winit backend memory purge hook`

### Task 4: Wire backend purge into the workspace no-surface idle flow

**Files:**
- Modify: `src/app/bootstrap.rs`

**Step 1: Call the new hook from the idle path**
- Update `update_workspace_terminal_idle_cache_shrink(...)` (or a thin wrapper around it) so the delayed no-surface threshold path requests backend purge on the main Slint window before/around terminal renderer-host release.
- Keep the existing terminal transient-cache clear and working-set trim behavior.

**Step 2: Keep it narrow**
- Do not trigger purge on every redraw.
- Only run when the workspace has no active terminal surface and the idle threshold has elapsed.

**Step 3: Connect the test hook**
- Make the new production helper invoke the test hook under `#[cfg(test)]` so the bootstrap test can observe it without requiring a real backend.

**Step 4: Run the failing test again**
- Run: `cargo test --lib idle_cache_shrink_requests_backend_purge_after_no_surface_threshold -- --exact`
- Expected: PASS

**Step 5: Run nearby regression tests**
- Run:
  - `cargo test --lib idle_cache_shrink_ -- --nocapture`
  - `cargo test --test bootstrap_smoke bootstrap_clears_terminal_renderer_caches_when_no_surface_remains -- --exact`
- Expected: PASS

**Step 6: Commit**
- Commit message: `fix: purge slint backend caches after terminal idle close`

### Task 5: Verify the full slice and summarize risk

**Files:**
- Modify: `docs/plans/2026-04-09-terminal-backend-purge-implementation-plan.md` (append final verification notes if needed)

**Step 1: Run final verification**
- Run:
  - `cargo check`
  - `cargo test --lib idle_cache_shrink_ -- --nocapture`
  - `cargo test --test bootstrap_smoke bootstrap_tracks_no_surface_idle_before_terminal_cache_shrink -- --exact`

**Step 2: Manual smoke guidance**
- Record the exact manual smoke sequence for Windows:
  - start app
  - open one SSH tab
  - run large output (`history` / `cat bigfile`)
  - close all tabs
  - wait past idle threshold
  - compare working set and private usage

**Step 3: Note residual risks**
- If private usage still stays high after purge, next suspects are DirectWrite/font caches and allocator behavior, not terminal core.

**Step 4: Commit any doc-note adjustment**
- Commit message: `docs: record backend purge verification notes`
