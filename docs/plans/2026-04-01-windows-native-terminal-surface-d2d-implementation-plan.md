# Windows Native Terminal Surface D2D Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a real Windows `native-only` terminal drawing path that renders terminal background, text, color glyphs, and overlays through `Direct2D HwndRenderTarget` inside the existing Slint host surface so packaged Windows builds stop showing a blank terminal region.

**Architecture:** Keep the current `terminal_presenter -> native_surface -> platform backend` seam, but upgrade the retained frame into a platform-consumable display list carrying background runs and glyph raster upload payloads. Then replace the Windows backend's no-op `present()` with a Direct2D-backed renderer that owns target lifetime, glyph bitmap caches, and overlay draw order.

**Tech Stack:** Rust, Slint rendering notifier APIs, existing terminal presenter/native surface contracts, Windows Direct2D / DirectWrite interop via Rust bindings, `cargo check`, `cargo clippy`, Linux-host Windows cross-build packaging

---

### Task 1: Lock the D2D retained-frame contract in source tests

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `src/app/terminal_layout/run_segmentation.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_presenter.rs`

**Step 1: Write the failing test**

Add source-level assertions that require:

- `TextStyleKey` to include `bg_rgba`
- prepared native frames to expose background runs
- monochrome and color glyph draws to expose stable cache/upload contracts instead of only atlas statistics
- presentable native frames to carry the new background/glyph payloads the Windows backend will consume

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because the current retained frame does not yet carry background runs or backend-consumable glyph upload payloads.

**Step 3: Write minimal implementation**

- Add `bg_rgba` to `TextStyleKey` in `src/app/terminal_layout/run_segmentation.rs`
- Add new background-run and glyph-upload structs in `src/app/terminal_renderer/wgpu_renderer.rs`
- Thread the retained-frame contracts into `src/app/terminal_presenter.rs`
- Keep the changes data-contract only; do not implement actual Windows drawing yet

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs src/app/terminal_layout/run_segmentation.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_presenter.rs
git commit -m "feat: define windows d2d retained frame contracts"
```

### Task 2: Thread glyph raster payloads through the prepare/presenter path

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing test**

Add assertions and focused unit expectations that require:

- single-channel monochrome raster payloads to be exposed with size/bearing/advance metadata
- color glyph raster payloads to be exposed with explicit RGBA upload data
- prepared frames to distinguish cache hits from first-use upload payloads
- presenter output to preserve those payloads for the backend

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: FAIL because the current prepare path does not yet thread upload payloads into the retained native frame.

**Step 3: Write minimal implementation**

- Extend font/backend contracts only where needed so the raster payloads are explicit and reusable
- Extend `src/app/terminal_renderer/atlas.rs` and `src/app/terminal_renderer/wgpu_renderer.rs` so prepared draws can carry cache keys plus optional upload payloads
- Update `src/app/terminal_presenter.rs` so `PresentableNativeFrame` retains the payloads without collapsing them back to diagnostics-only metadata

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_presenter.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: thread windows glyph raster payloads into retained frames"
```

### Task 3: Implement Windows `HwndRenderTarget` lifecycle and backend state

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/backend.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**

Add source-level assertions that require the Windows backend source to expose:

- Direct2D factory / render-target ownership
- target creation and recreate hooks tied to `HWND` and terminal rect
- retained glyph cache state for monochrome and color resources
- a `present()` implementation that is no longer a no-op token update

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: FAIL because `platform/windows.rs` still only stores frame state and updates `last_presented_frame_token`.

**Step 3: Write minimal implementation**

- Upgrade `src/app/terminal_renderer/platform/windows.rs` into a real backend state object with D2D factory/target lifecycle
- Keep `src/app/windows_frame.rs` host-`HWND` resolution compiling cleanly for the Windows target
- Do not add text drawing yet; this task is only about backend ownership, target creation, resize, recreate, and detach contracts

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/backend.rs src/app/terminal_renderer/platform/mod.rs src/app/windows_frame.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add windows d2d target lifecycle backend"
```

### Task 4: Draw terminal backgrounds and monochrome glyph masks through Direct2D

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add tests that require the Windows backend source to show:

- background run iteration before glyph drawing
- monochrome glyph upload into a reusable D2D bitmap or opacity-mask resource
- monochrome glyph drawing that uses retained placement and foreground color rather than recomputing layout

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because the backend still does not consume background runs or monochrome glyph payloads.

**Step 3: Write minimal implementation**

- Implement background clearing and per-run background fill in `src/app/terminal_renderer/platform/windows.rs`
- Implement monochrome glyph upload/cache reuse in the same backend
- Draw monochrome glyphs from retained placement data; do not add color glyphs or overlays in this task

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_presenter.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: draw windows terminal backgrounds and mono glyphs"
```

### Task 5: Add color glyph and overlay drawing to the Windows backend

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing test**

Add assertions that require:

- color glyph bitmaps to use a dedicated cache/draw path
- selection rectangles to draw below text but above background
- underline, cursor, and IME preview to be present in the backend draw order
- bootstrap/native surface handoff to keep threading the full retained frame without reintroducing bitmap image state

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: FAIL because color glyphs and overlay draw order are not fully implemented in the backend yet.

**Step 3: Write minimal implementation**

- Extend `src/app/terminal_renderer/platform/windows.rs` with color glyph upload/draw support
- Draw selection, underline, cursor, and IME preview in the intended order
- Keep `src/app/bootstrap.rs` and presenter contracts aligned with the final retained-frame payload shape

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: PASS

**Step 5: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_presenter.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: finish windows native color glyph and overlay drawing"
```

### Task 6: Verify the Windows native-only package end to end

**Files:**
- Verify only: `src/app/terminal_renderer/platform/windows.rs`
- Verify only: `src/app/terminal_renderer/native_surface.rs`
- Verify only: `src/app/terminal_presenter.rs`
- Verify only: `src/app/terminal_renderer/wgpu_renderer.rs`
- Verify only: `src/app/terminal_layout/run_segmentation.rs`
- Verify only: `src/app/windows_frame.rs`
- Verify only: `tests/native_terminal_surface_contract_spec.rs`
- Verify only: `tests/terminal_renderer_dwrite_spec.rs`
- Verify only: `tests/terminal_color_emoji_spec.rs`
- Verify only: `build-win-x64-software.sh`

**Step 1: Run focused contract tests**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: PASS

**Step 2: Run workspace verification**

Run: `cargo check --workspace`
Expected: PASS

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 3: Run package build verification**

Run: `./build-win-x64-software.sh`
Expected: PASS and produce the Windows software wrapper package without reintroducing the blank terminal regression in the packaged binary.

**Step 4: Commit**

```bash
git add docs/plans/2026-04-01-windows-native-terminal-surface-d2d-design.md docs/plans/2026-04-01-windows-native-terminal-surface-d2d-implementation-plan.md
git commit -m "docs: add windows native terminal surface d2d plan"
```

### Task 7: Write the final TDD handoff spec after implementation completes

**Files:**
- Create: `docs/plans/2026-04-01-windows-native-terminal-surface-d2d-tdd-spec.md`

**Step 1: Gather final implementation facts**

Record the final:

- core structs and traits
- Slint callbacks / bindings / global state usage
- Tokio task / channel interactions affected by the change
- state transitions and resource release ordering
- edge cases discovered during implementation

**Step 2: Write the handoff spec**

Document at least:

- retained frame structs and backend contracts
- Windows D2D target lifecycle
- glyph/background/cache ownership
- error handling and redraw behavior
- recommended unit / integration / UI tests for follow-up hardening

**Step 3: Verify docs are consistent**

Run: `cargo check --workspace`
Expected: PASS with no documentation-driven code regressions.

Run: `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 4: Commit**

```bash
git add docs/plans/2026-04-01-windows-native-terminal-surface-d2d-tdd-spec.md
git commit -m "docs: add windows native d2d renderer tdd handoff"
```

Plan complete and saved to `docs/plans/2026-04-01-windows-native-terminal-surface-d2d-implementation-plan.md`. Two execution options:

1. Subagent-Driven (this session) - I dispatch fresh subagent per task, review between tasks, fast iteration
2. Parallel Session (separate) - Open new session with executing-plans, batch execution with checkpoints

当前你已经明确要求继续在当前会话按 `executing-plans` 方式逐 Task 落地，因此后续可直接从 Task 1 开始执行。
