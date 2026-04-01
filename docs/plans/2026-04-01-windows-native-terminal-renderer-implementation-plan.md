# Windows Native Terminal Renderer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current Windows `bitmap` terminal shipping path with a native renderer path that stays inside the Slint surface while adding DirectWrite-backed shaping/fallback, ligatures, OpenType feature controls, color emoji, and a real GPU present loop.

**Architecture:** Keep the existing session/runtime/UI flow, but add a Windows text engine and expand the current native presenter/renderer seam into a complete rendering pipeline. Delivery is staged: first prove the Slint surface bridge and native frame lifetime, then add text-engine contracts, then implement glyph/color caches and overlays, then switch Windows packaging to native-first with bitmap fallback.

**Tech Stack:** Rust, Slint rendering notifier APIs, wgpu, existing terminal presenter seam, Windows DirectWrite/Direct2D integration layer, focused Rust contract tests

---

### Task 1: Lock the native-only shipping contract in tests and docs

**Files:**
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `build-win-x64.sh`
- Modify: `build-win-x64-software.sh`

**Step 1: Write the failing test**

Add assertions that:

- Windows native shipping is documented as the preferred package path
- the runtime profile keeps both `Bitmap` and `Native` modes
- the software build script remains explicit fallback-only packaging
- the primary Windows build script advertises the native renderer path

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_scrollback_spec -q`
Expected: FAIL because the docs/build/runtime contract has not been updated to reflect native-first shipping.

**Step 3: Write minimal implementation**

- Update the relevant contract tests
- Update `src/app/runtime_profile.rs` comments/contracts if needed
- Update `build-win-x64.sh` and `build-win-x64-software.sh` comments/env descriptions so native-first vs fallback-only is explicit

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_scrollback_spec -q`
Expected: PASS

**Step 5: Commit**

```bash
git add tests/native_terminal_surface_contract_spec.rs tests/terminal_scrollback_spec.rs src/app/runtime_profile.rs build-win-x64.sh build-win-x64-software.sh
git commit -m "test: lock windows native renderer shipping contract"
```

### Task 2: Prove the Slint native surface bridge can own real frame state

**Files:**
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**

Add source-level assertions that the native surface bridge now exposes:

- a retained frame payload or frame state handle, not just `frame_token`
- an explicit draw/present hook reachable from the rendering notifier path
- bootstrap wiring that updates the bridge with both geometry and native frame state

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec native_surface_source_exposes_present_bridge_contract -q`
Expected: FAIL because `NativeTerminalSurface` only stores a token and requests redraw today.

**Step 3: Write minimal implementation**

- Extend `src/app/terminal_renderer/native_surface.rs` with a retained native frame state contract
- Re-export any new bridge structs through `src/app/terminal_renderer/mod.rs`
- Thread the bridge state through `src/app/bootstrap.rs`

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec native_surface_source_exposes_present_bridge_contract -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/mod.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: add slint native surface bridge contract"
```

### Task 3: Replace the fake DirectWrite backend contract with a Windows text engine contract

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_layout/shaper.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add assertions that the Windows text backend source exposes contracts for:

- font fallback chain discovery
- OpenType feature configuration
- ligature-aware shaping
- color glyph detection / raster contract
- glyph runs that are not limited to bundled Sarasa-only assumptions

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec windows_text_engine_source_exposes_fallback_and_feature_contracts -q`
Expected: FAIL because the current `DirectWriteFontSystem` is still a bundled-font placeholder over `ab_glyph/swash/rustybuzz`.

**Step 3: Write minimal implementation**

- Add new trait/data structures in `backend.rs` for fallback runs, feature sets, and color glyph raster outputs
- Refactor `windows_dwrite.rs` to expose a Windows text engine contract instead of a bundled Sarasa-only façade
- Update `shaper.rs` to accept the richer shaping output contract

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec windows_text_engine_source_exposes_fallback_and_feature_contracts -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/mod.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_layout/shaper.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: define windows text engine contracts"
```

### Task 4: Add native renderer tests for monochrome glyph atlas and color glyph cache separation

**Files:**
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`

**Step 1: Write the failing test**

Add tests asserting:

- monochrome glyphs are cached through the existing atlas path
- color glyphs use a separate cache/state path
- renderer preparation records enough metadata to distinguish mono and color entries

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: FAIL because the native renderer only prepares monochrome glyph atlas state today.

**Step 3: Write minimal implementation**

- Extend `atlas.rs` with any additional cache keys/entry metadata needed
- Extend `wgpu_renderer.rs` prepared-frame state so mono vs color glyph resources are explicit
- Update tests accordingly

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_color_emoji_spec -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_color_emoji_spec.rs
git commit -m "feat: add native mono and color glyph cache contracts"
```

### Task 5: Implement presentable native frame composition contracts

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add assertions that the native presenter path now threads:

- shaped rows and renderer outputs into a retained native frame payload
- cursor/selection/underline metadata into the native frame
- a presentable frame state through bootstrap instead of only a `frame_token`

**Step 2: Run test to verify it fails**

Run: `cargo test --test terminal_renderer_dwrite_spec terminal_presenter_threads_presentable_native_frame_state -q`
Expected: FAIL because the native presenter currently returns only `frame_token` and cell metrics.

**Step 3: Write minimal implementation**

- Expand native frame structs in `terminal_presenter.rs`
- Expand `PreparedNativeFrame` in `wgpu_renderer.rs`
- Update bootstrap/native frame handoff code

**Step 4: Run test to verify it passes**

Run: `cargo test --test terminal_renderer_dwrite_spec terminal_presenter_threads_presentable_native_frame_state -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_presenter.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/bootstrap.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: thread presentable native frame state"
```

### Task 6: Add cursor, selection, underline, and IME overlay render contracts

**Files:**
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add tests that assert the native rendering path explicitly carries overlay data for:

- selection rectangles/colors
- cursor shape/visibility/blink state
- underline decorations
- IME preview overlays

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because overlays are still effectively modeled for the bitmap/UI path only.

**Step 3: Write minimal implementation**

- Add overlay data contracts to presenter/renderer structs
- Thread the selection and cursor state from bootstrap into the native frame data
- Update tests accordingly

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_presenter.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add native overlay rendering contracts"
```

### Task 7: Switch Windows presenter installation to native-first with bitmap fallback

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**

Add assertions that:

- Windows presenter installation prefers the native presenter
- bitmap presenter remains available only as fallback
- runtime profile and UI contracts still carry both render modes

**Step 2: Run test to verify it fails**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because Windows packages still default to the bitmap path today.

**Step 3: Write minimal implementation**

- Update presenter installation logic in `bootstrap.rs`
- Update runtime profile/defaults if needed
- Keep bitmap path available for explicit fallback/error cases

**Step 4: Run test to verify it passes**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/terminal_presenter.rs src/app/runtime_profile.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: make windows terminal rendering native-first"
```

### Task 8: Verify the new contracts compile cleanly before implementation deepening

**Files:**
- Verify only: `src/app/terminal_font/backend.rs`
- Verify only: `src/app/terminal_font/windows_dwrite.rs`
- Verify only: `src/app/terminal_layout/shaper.rs`
- Verify only: `src/app/terminal_presenter.rs`
- Verify only: `src/app/terminal_renderer/atlas.rs`
- Verify only: `src/app/terminal_renderer/native_surface.rs`
- Verify only: `src/app/terminal_renderer/wgpu_renderer.rs`
- Verify only: `src/app/bootstrap.rs`
- Verify only: `tests/native_terminal_surface_contract_spec.rs`
- Verify only: `tests/terminal_color_emoji_spec.rs`
- Verify only: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Run focused tests**

Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_color_emoji_spec --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 2: Run compile verification**

Run: `cargo check -q`
Expected: PASS

**Step 3: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_layout/shaper.rs src/app/terminal_presenter.rs src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_color_emoji_spec.rs tests/terminal_renderer_dwrite_spec.rs docs/plans/2026-04-01-windows-native-terminal-renderer-design.md docs/plans/2026-04-01-windows-native-terminal-renderer-implementation-plan.md
git commit -m "docs: plan windows native terminal renderer"
```

Plan complete and saved to `docs/plans/2026-04-01-windows-native-terminal-renderer-implementation-plan.md`. Two execution options:

**1. Subagent-Driven (this session)** - I dispatch fresh subagent per task, review between tasks, fast iteration

**2. Parallel Session (separate)** - Open new session with executing-plans, batch execution with checkpoints

**Which approach?**
