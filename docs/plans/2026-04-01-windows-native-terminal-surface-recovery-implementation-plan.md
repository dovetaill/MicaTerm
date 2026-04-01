# Windows Native Terminal Surface Recovery Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Repair the Windows native terminal surface so terminal text is actually visible on Windows, then finish the missing font fallback, emoji, OpenType, damage, and lifecycle work to a ship-ready level.

**Architecture:** Keep the current `NativeTerminalFrame` and `PlatformNativeSurfaceBackend` contracts, but split rendering into a stable present-driver layer, a Windows D2D backend, and a real Windows font pipeline. Fix the render trigger first, then harden the backend, then port the missing font/fallback/emoji logic from proven terminal projects.

**Tech Stack:** Rust, Slint, Tokio, wezterm-term, termwiz, windows, Direct2D, DirectWrite/GDI fallback discovery, HarfBuzz via `rustybuzz` or equivalent bindings, D2D bitmap caches, focused contract/runtime tests, Windows packaging scripts

---

## Plan Notes

- 这是一份 recovery plan，不是“继续沿着旧文档宣称已完成”的 plan。
- 旧文档 `docs/plans/2026-04-01-native-only-terminal-surface-design.md` 与 `docs/plans/2026-04-01-native-only-terminal-surface-implementation-plan.md` 只能作为历史背景，不能继续当成当前真实状态。
- 每个 task 都要优先保证“能证明问题被修到哪一步”，而不是继续堆 source-level contract。

### Task 1: Replace the fragile notifier-only render trigger with a present-driver seam

**Files:**
- Create: `src/app/terminal_renderer/present_driver.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing test**
- Add source/behavior assertions that `NativeTerminalSurface` no longer depends on a single `set_rendering_notifier()` path for every backend present.
- Add assertions that a present-driver abstraction exists and that the native surface can mark itself dirty independently of notifier registration success.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: FAIL because there is no present-driver abstraction yet.

**Step 3: Write minimal implementation**
- Add a `present_driver` module with a small trait/API for scheduling UI-thread present work.
- Refactor `NativeTerminalSurface` to own a driver and a dirty flag.
- Keep `RenderingNotifier` as one possible driver, not the only path.

**Step 4: Run test to verify it passes**
Run: `cargo test --test native_terminal_surface_contract_spec -q`
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_renderer/present_driver.rs src/app/terminal_renderer/mod.rs src/app/terminal_renderer/native_surface.rs src/app/bootstrap.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "refactor: add native terminal present driver seam"
```

### Task 2: Add Windows runtime diagnostics so blank-surface failures become observable

**Files:**
- Create: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add assertions that the Windows backend exposes diagnostics for `HWND`, render-target generation, last prepared frame token, last presented frame token, and draw counters.
- Add assertions that `NativeTerminalSurface` can surface or snapshot the latest diagnostics.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because the diagnostics contract does not exist yet.

**Step 3: Write minimal implementation**
- Add a diagnostics struct for backend runtime state.
- Update the Windows backend to record attach/present/draw/end-draw outcomes.
- Make the native surface retain the latest snapshot for logging and smoke verification.

**Step 4: Run test to verify it passes**
Run: `cargo test --test terminal_renderer_dwrite_spec -q`
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_renderer/diagnostics.rs src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/native_surface.rs src/app/windows_frame.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add windows native surface diagnostics"
```

### Task 3: Make the Windows runtime actually call present on the chosen renderer path

**Files:**
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_renderer/present_driver.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `build-win-x64-software.sh`
- Modify: `build-win-x64.sh`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/build_win_x64_software_script_smoke.sh`
- Modify: `tests/build_win_x64_script_smoke.sh`

**Step 1: Write the failing test**
- Add assertions that Windows shipping/diagnostic profiles select a render-trigger path that is known to present native terminal frames.
- Add script smoke assertions so package scripts no longer imply that `winit-software` blank output is acceptable.

**Step 2: Run test to verify it fails**
Run: `cargo test --test runtime_profile -q && bash tests/build_win_x64_software_script_smoke.sh && bash tests/build_win_x64_script_smoke.sh`
Expected: FAIL because the runtime/build profiles do not encode the repaired trigger strategy yet.

**Step 3: Write minimal implementation**
- Thread the repaired present-driver selection through runtime/bootstrap.
- Update build scripts/comments/env to reflect the real Windows validation lane.
- Keep this task focused on runtime selection and build truthfulness, not on font work.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/runtime_profile.rs src/app/bootstrap.rs src/app/terminal_renderer/present_driver.rs src/app/terminal_renderer/native_surface.rs build-win-x64-software.sh build-win-x64.sh tests/runtime_profile.rs tests/build_win_x64_software_script_smoke.sh tests/build_win_x64_script_smoke.sh
git commit -m "fix: wire windows native present path into runtime profiles"
```

### Task 4: Port real Windows font locating and fallback discovery

**Files:**
- Create: `src/app/terminal_font/windows_locator.rs`
- Create: `src/app/terminal_font/windows_fallback.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_layout_harfbuzz_spec.rs`

**Step 1: Write the failing test**
- Add tests asserting that Windows font fallback resolution no longer returns only a hard-coded string list.
- Add tests asserting that fallback discovery can resolve multiple families for mixed text/emoji/symbol input.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_renderer_dwrite_spec --test terminal_layout_harfbuzz_spec -q`
Expected: FAIL because the current Windows font backend still resolves only the bundled primary face.

**Step 3: Write minimal implementation**
- Add a Windows locator/fallback helper inspired by `wezterm-font/src/locator/gdi.rs`.
- Preserve the current bundled default font, but allow real system fallback face discovery.
- Keep the initial port small and testable; avoid redesigning unrelated font APIs.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_font/windows_locator.rs src/app/terminal_font/windows_fallback.rs src/app/terminal_font/mod.rs src/app/terminal_font/windows_dwrite.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_layout_harfbuzz_spec.rs
git commit -m "feat: add windows font fallback discovery"
```

### Task 5: Make OpenType features, ligatures, and fallback shaping real

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_layout/shaper.rs`
- Modify: `tests/terminal_layout_harfbuzz_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add tests asserting that feature tags are actually passed into shaping.
- Add tests asserting that mixed clusters retry fallback faces instead of truncating to the first resolved face.
- Add tests asserting that ligature and cluster-to-cell mapping remains stable for terminal cell accounting.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_layout_harfbuzz_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because shaping still calls HarfBuzz with an empty feature array and only uses the first resolved face.

**Step 3: Write minimal implementation**
- Translate `OpenTypeFeatureSet` into HarfBuzz-compatible features.
- Rework shaping so unresolved clusters can recurse through fallback faces.
- Preserve terminal cell accounting and cluster boundaries.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_layout/shaper.rs tests/terminal_layout_harfbuzz_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: wire windows fallback shaping and opentype features"
```

### Task 6: Replace fake color emoji output with a real color glyph path

**Files:**
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add tests asserting that color glyph rasterization no longer synthesizes fake RGBA blocks from glyph IDs.
- Add tests asserting that Windows color glyph resources remain separate from the monochrome atlas and carry stable destination geometry.

**Step 2: Run test to verify it fails**
Run: `cargo test --test terminal_color_emoji_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because the current backend still creates placeholder RGBA blocks for emoji/color glyphs.

**Step 3: Write minimal implementation**
- Port a real color glyph strategy inspired by `wezterm-font/src/rasterizer/colr.rs`.
- Keep mono and color caches separate.
- Update the Windows backend to upload and draw real color glyph data.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_font/windows_dwrite.rs src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/wgpu_renderer.rs tests/terminal_color_emoji_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: add real windows color glyph rendering"
```

### Task 7: Harden damage tracking, resize, device-loss, and shutdown sequencing

**Files:**
- Create: `src/app/terminal_renderer/damage.rs`
- Modify: `src/app/terminal_renderer/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`

**Step 1: Write the failing test**
- Add tests asserting that resize invalidates the whole native surface frame.
- Add tests asserting that selection/cursor/IME-only changes can damage and repaint without a full text frame rebuild.
- Add tests asserting that device recreation clears stale D2D resources and that detach blocks further present work.

**Step 2: Run test to verify it fails**
Run: `cargo test --test native_terminal_surface_contract_spec --test terminal_renderer_dwrite_spec -q`
Expected: FAIL because damage tracking and lifecycle hardening are still ad hoc.

**Step 3: Write minimal implementation**
- Add a focused damage helper inspired by Alacritty’s `display/damage.rs`.
- Use it to drive full/partial redraw decisions.
- Harden detach/recreate-target/shutdown ordering.

**Step 4: Run test to verify it passes**
Run: same command as Step 2
Expected: PASS

**Step 5: Verify compile quality**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
Expected: PASS

**Step 6: Commit**
```bash
git add src/app/terminal_renderer/damage.rs src/app/terminal_renderer/mod.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_renderer/platform/windows.rs src/app/windows_frame.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_renderer_dwrite_spec.rs
git commit -m "feat: harden windows native surface lifecycle and damage tracking"
```

### Task 8: Run Windows packaging, real-machine validation, and write the honest TDD handoff

**Files:**
- Modify: `docs/plans/2026-04-01-native-only-terminal-surface-tdd-spec.md` or create a new recovery TDD doc if the scope has diverged
- Create: `docs/plans/2026-04-01-windows-native-terminal-surface-recovery-tdd-spec.md` (preferred if the previous TDD doc is misleading)
- Modify: `mustdo.md`
- Modify: any verification log doc if one is introduced during execution

**Step 1: Run final validation commands**
Run:
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `./build-win-x64-software.sh`
- `./build-win-x64.sh`
Expected: PASS

**Step 2: Perform Windows real-machine verification**
- Launch the packaged app on Windows.
- Verify first-paint text visibility.
- Verify selection, underline, cursor, IME preview, emoji, resize, close, and reconnect flows.
Expected: PASS with no blank terminal region.

**Step 3: Write the handoff document**
- Record the real core structs/traits/callbacks/channels that shipped.
- Record what was copied/ported from WezTerm/Alacritty and what remained custom.
- Record remaining risks honestly.

**Step 4: Reconcile must-do status**
- Remove or strike completed items from `mustdo.md`.
- Leave any unresolved items clearly marked instead of silently dropping them.

**Step 5: Commit**
```bash
git add docs/plans/2026-04-01-windows-native-terminal-surface-recovery-tdd-spec.md mustdo.md
git commit -m "docs: hand off repaired windows native terminal surface"
```

## Final Verification Gate

Do not mark the feature complete until all of the following are true:
- `cargo check --workspace` passes
- `cargo clippy --workspace -- -D warnings` passes
- both Windows build scripts pass
- Windows real-machine validation shows visible text and overlays
- the final TDD/handoff doc reflects reality rather than source-level assumptions
