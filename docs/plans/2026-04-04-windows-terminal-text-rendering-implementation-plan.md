# Windows Terminal Text Rendering Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 将 Windows 终端文本主路径从当前的 bitmap-in-scene / opacity-mask 文本渲染，升级为更接近 Windows Terminal 观感的 `DirectWrite + Direct2D` native text pipeline，同时修复混排割裂、发灰发虚、宽字符右侧裁切和 overlay ownership 分裂问题。

**Architecture:** 保持现有 `terminal_model -> terminal_presenter -> native_surface -> platform backend` 总体分层不变，但把 `WindowsMainline` 默认切到 `PostRenderNativeSurface`，并让 Windows 专项路径真正接管字体链、fallback、metrics、visible bounds、text drawing、selection/cursor/IME overlay。保留 `SceneImage` 作为 fallback/compatibility 路径，同时将旧 atlas/bitmap 路径收窄为兼容层，而不是继续充当 Windows 主文本方案。

**Tech Stack:** Rust, Slint, Tokio, Windows DirectWrite, Windows Direct2D, existing `terminal_presenter` / `native_surface` contracts, `wezterm-term`, `termwiz`, `cargo test`, `cargo check`, `cargo clippy`, Windows packaging scripts

---

## Input Design

- 设计基线固定为 `docs/plans/2026-04-04-windows-terminal-text-rendering-design.md`
- 实施不得偏离以下已确认决策：
  - `WindowsMainline` 默认终端主路径切换为 `PostRenderNativeSurface`
  - Windows 终端主文本目标架构为 `DirectWrite + Direct2D`
  - 默认显式字体链为 `Cascadia Mono -> Sarasa Term SC -> Segoe UI Emoji`
  - `Cascadia Mono` 与 `Sarasa Term SC` 作为随包字体资产提供
  - Windows native path 统一拥有 text、selection、cursor、underline、IME preview 的绘制权
  - glyph 可见边界以 visible bounds / overhang 为准，仅受行级或 viewport 级 clip 约束
  - 必须提供可选 debug overlay / log，暴露字体、metrics、AA、alpha mode、DPI 等信息
- 本计划允许修改文档、测试、渲染代码、Windows backend 和字体资产接线；不允许新增终端产品功能或改变终端语义模型

## Execution Notes

- 每个 task 都先使用 `@superpowers:test-driven-development`：先写失败测试或 source contract，再做最小实现，再验证通过。
- 如果 Windows 文本集成过程中出现渲染结果异常、AA 不符合预期、字体 fallback 不生效、右边界仍被裁切，立即切换 `@superpowers:systematic-debugging`，不要凭经验硬调 magic number。
- Windows 专项路径允许短期与 cross-platform 路径存在并行实现，但必须保持 `terminal_model` 与 cell/grid 合同稳定。
- 所有新增字体资产都必须同时补齐 license 文件；不要把外部下载步骤留成手工 TODO。
- 优先用小步提交，每个 task 都应保持可编译、可验证、可回退。
- 若在独立 worktree 中执行更安全；若在当前工作区执行，必须把改动范围限制在本计划列出的文件。

## Task Sequence Overview

1. 锁定 Windows native 主路径和 host ownership contract。
2. 引入显式字体链、随包字体资产和新的默认排版参数。
3. 把当前“伪 DirectWrite”字体层升级为真实 Windows DirectWrite font/metrics/fallback 层。
4. 在 Windows backend 中加入真正的 DirectWrite + Direct2D 主文本绘制路径，并确保满足 ClearType / opaque background 条件。
5. 修复 visible bounds / overhang / clip / atlas padding，并把 overlay ownership 统一到 native path。
6. 增加调试输出、overlay 和回归矩阵，确保后续能快速定位字体、clip、AA 和 DPI 问题。
7. 做 Windows 专项全链路验证，并保留 fallback / rollback 入口。

### Task 1: Switch Windows mainline to native surface and lock host ownership contracts

**Files:**
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/runtime_profile.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/terminal_scene_image_spec.rs`
- Create: `tests/windows_terminal_native_mode_contract_smoke.sh`

**Step 1: Write the failing tests**

Add source-level and runtime-profile assertions that require:

- `WindowsMainline` with Direct3D preference to resolve to `TerminalCompositionMode::PostRenderNativeSurface`
- `build_workspace_terminal_presenter()` to return native mode for the Windows mainline path instead of `SceneImage`
- `TerminalSessionHost` to keep bitmap image visibility gated to bitmap mode only
- Slint host cursor rectangle not to render in native mode
- bootstrap cursor synchronization to stop treating Slint as the final cursor owner when native frame rendering is active

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test native_terminal_surface_contract_spec --test terminal_scene_image_spec -q
bash tests/windows_terminal_native_mode_contract_smoke.sh
```

Expected: FAIL, because the current Windows mainline path still prefers `SceneImage`, and host/native ownership is not yet cleanly separated.

**Step 3: Write minimal implementation**

- Change `src/app/runtime_profile.rs` so `WindowsMainline` defaults to `PostRenderNativeSurface`
- Keep `SceneImage` as an explicit fallback path rather than deleting it
- Update `src/app/bootstrap.rs` so native mode stops feeding redundant host cursor ownership where possible
- Update `ui/shell/terminal-session-host.slint` so bitmap-only image and cursor overlay stay truly bitmap-only
- Keep changes focused on mode selection and ownership gates; do not implement DWrite text drawing yet

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/runtime_profile.rs src/app/bootstrap.rs ui/shell/terminal-session-host.slint tests/runtime_profile.rs tests/native_terminal_surface_contract_spec.rs tests/terminal_scene_image_spec.rs tests/windows_terminal_native_mode_contract_smoke.sh
git commit -m "feat: switch windows mainline terminal path to native surface"
```

### Task 2: Introduce the explicit font chain, bundled font assets, and new typography defaults

**Files:**
- Create: `assets/fonts/CascadiaMono/CascadiaMono-Regular.ttf`
- Create: `assets/fonts/CascadiaMono/LICENSE.txt`
- Create: `assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf`
- Create: `assets/fonts/SarasaTermSC/LICENSE.txt`
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_font/windows_fallback.rs`
- Modify: `src/app/terminal_atlas.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `tests/terminal_font_registration_smoke.rs`
- Modify: `tests/startup_font_memory_regression.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Create: `tests/windows_terminal_typography_defaults_spec.rs`

**Step 1: Write the failing tests**

Add assertions that require:

- the bundled default terminal font family contract to move away from `Fusion JetBrains Maple Mono`
- the Windows fallback chain to explicitly include `Cascadia Mono`, `Sarasa Term SC`, and `Segoe UI Emoji`
- default typography to land in the target range: `13px` or `14px`, line height compatible with `1.35 ~ 1.45`, `letter spacing = 0`, `Regular`
- font bundle smoke tests to require bundled Latin/CJK assets and license files
- workspace/terminal host tests to stop asserting the old `18px`-style defaults

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_font_registration_smoke --test startup_font_memory_regression --test terminal_renderer_dwrite_spec --test workspace_tabs_spec --test windows_terminal_typography_defaults_spec -q
```

Expected: FAIL, because the current defaults still point at Fusion Maple, 18px sizing, and the old fallback order.

**Step 3: Write minimal implementation**

- Bundle the approved Latin/CJK fonts and licenses into `assets/fonts`
- Change the terminal font default contract in `src/app/terminal_font/backend.rs`
- Update Windows fallback resolution order in `src/app/terminal_font/windows_fallback.rs`
- Update `src/app/terminal_presenter.rs` and `src/app/terminal_atlas.rs` so fallback/compatibility paths use the same typography defaults where applicable
- Keep this task focused on font assets, default configuration, and consistent defaults; do not implement the real DWrite font engine yet

**Step 4: Run tests to verify they pass**

Run: same command as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add assets/fonts/CascadiaMono/CascadiaMono-Regular.ttf assets/fonts/CascadiaMono/LICENSE.txt assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf assets/fonts/SarasaTermSC/LICENSE.txt src/app/terminal_font/backend.rs src/app/terminal_font/mod.rs src/app/terminal_font/windows_fallback.rs src/app/terminal_atlas.rs src/app/terminal_presenter.rs tests/terminal_font_registration_smoke.rs tests/startup_font_memory_regression.rs tests/terminal_renderer_dwrite_spec.rs tests/workspace_tabs_spec.rs tests/windows_terminal_typography_defaults_spec.rs
git commit -m "feat: adopt bundled windows terminal font chain defaults"
```

### Task 3: Replace the faux `DirectWriteFontSystem` internals with a real DirectWrite-based font and metrics layer

**Files:**
- Modify: `src/app/terminal_font/backend.rs`
- Modify: `src/app/terminal_font/windows_dwrite.rs`
- Modify: `src/app/terminal_font/windows_locator.rs`
- Modify: `src/app/terminal_font/windows_fallback.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_layout/shaper.rs`
- Modify: `tests/terminal_renderer_dwrite_spec.rs`
- Modify: `tests/terminal_layout_harfbuzz_spec.rs`
- Modify: `tests/terminal_glyph_cell_span_spec.rs`
- Modify: `tests/terminal_color_emoji_spec.rs`
- Create: `tests/windows_directwrite_font_chain_spec.rs`

**Step 1: Write the failing tests**

Add assertions that require:

- `DirectWriteFontSystem` to stop hard-binding all shaping/rasterization to the bundled default face
- fallback faces for CJK and symbol text to resolve to actual per-face font data instead of metadata-only family tags
- baseline, ascent, descent, line gap, cell width, and visible bounds metrics to come from the Windows text stack rather than only `ab_glyph` heuristics
- Windows font chain tests to prove mixed Latin/CJK/emoji text no longer resolves to a single fake face

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_renderer_dwrite_spec --test terminal_layout_harfbuzz_spec --test terminal_glyph_cell_span_spec --test terminal_color_emoji_spec --test windows_directwrite_font_chain_spec -q
```

Expected: FAIL, because the current `windows_dwrite.rs` still uses a bundled `FontArc`, `rustybuzz`, and `swash` as the real source of monochrome shaping/raster.

**Step 3: Write minimal implementation**

- Rebuild `src/app/terminal_font/windows_dwrite.rs` around actual Windows DirectWrite font face loading, metrics lookup, and fallback resolution
- Keep the terminal grid model in Rust; only move face selection, metrics, fallback, and glyph-bound introspection into DirectWrite
- Preserve emoji fallback behavior where needed, but stop treating non-emoji fallback as metadata-only
- Update backend contracts in `src/app/terminal_font/backend.rs` so visible bounds / overhang metrics can flow downstream

**Step 4: Run tests to verify they pass**

Run: same command as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_font/backend.rs src/app/terminal_font/windows_dwrite.rs src/app/terminal_font/windows_locator.rs src/app/terminal_font/windows_fallback.rs src/app/terminal_font/mod.rs src/app/terminal_layout/shaper.rs tests/terminal_renderer_dwrite_spec.rs tests/terminal_layout_harfbuzz_spec.rs tests/terminal_glyph_cell_span_spec.rs tests/terminal_color_emoji_spec.rs tests/windows_directwrite_font_chain_spec.rs
git commit -m "feat: implement real windows directwrite terminal font pipeline"
```

### Task 4: Add a real DirectWrite + Direct2D primary text drawing path with opaque background guarantees

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/platform/backend.rs`
- Modify: `src/app/terminal_renderer/platform/mod.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify: `tests/windows_frame_spec.rs`
- Create: `tests/windows_native_text_renderer_contract_spec.rs`

**Step 1: Write the failing tests**

Add source-level and contract assertions that require:

- the Windows backend to expose a real DWrite text drawing path instead of only `FillOpacityMask` monochrome blits
- the primary text draw target to satisfy opaque-background requirements for ClearType-friendly rendering
- use of display/monitor-aware rendering parameters instead of hard-coded text AA heuristics
- `native_surface` diagnostics to expose the active Windows text renderer path

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test native_terminal_surface_contract_spec --test windows_frame_spec --test windows_native_text_renderer_contract_spec -q
```

Expected: FAIL, because the current Windows backend is still a premultiplied-alpha bitmap/mask renderer and does not expose a true DWrite text draw path.

**Step 3: Write minimal implementation**

- Upgrade `src/app/terminal_renderer/platform/windows.rs` to create and manage the D2D/DWrite resources needed for primary text drawing
- Ensure the text region is rendered against an opaque background before text draw calls
- Thread visible bounds / glyph metrics from `src/app/terminal_presenter.rs` into the backend
- Keep color glyph and compatibility bitmap code compiling, but make the primary monochrome text path use the new native text renderer

**Step 4: Run tests to verify they pass**

Run: same command as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/platform/backend.rs src/app/terminal_renderer/platform/mod.rs src/app/terminal_renderer/native_surface.rs src/app/terminal_presenter.rs src/app/windows_frame.rs tests/native_terminal_surface_contract_spec.rs tests/windows_frame_spec.rs tests/windows_native_text_renderer_contract_spec.rs
git commit -m "feat: add windows native directwrite text renderer"
```

### Task 5: Fix visible bounds, overhang, clip policy, atlas padding, and native overlay ownership

**Files:**
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/atlas.rs`
- Modify: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_scene_image.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `tests/terminal_glyph_origin_snap_spec.rs`
- Modify: `tests/terminal_glyph_cell_span_spec.rs`
- Modify: `tests/terminal_scene_image_spec.rs`
- Modify: `tests/native_terminal_surface_damage_rect_spec.rs`
- Create: `tests/terminal_visible_bounds_regression_spec.rs`

**Step 1: Write the failing tests**

Add regression cases that require:

- `W`, `w`, `M`, `m`, `%`, `4`, `@`, `&`, `#`, `>`, `<`, `]`, `)`, `}`, `A`, `V`, `Y` not to lose their rightmost visible pixels
- glyph draw bounds to use visible bounds / overhang rather than strict cell-advance clipping
- atlas entries to include safety padding on both sides for compatibility paths
- native mode to own cursor/selection/underline/IME overlays without host double-draw
- scene-image fallback path to remain functional, but to use the relaxed clip/padding rules needed to avoid the old right-edge shave

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_glyph_origin_snap_spec --test terminal_glyph_cell_span_spec --test terminal_scene_image_spec --test native_terminal_surface_damage_rect_spec --test terminal_visible_bounds_regression_spec -q
```

Expected: FAIL, because the current clip and atlas policy still clamps glyphs too tightly and host/native overlay ownership is still split.

**Step 3: Write minimal implementation**

- Change Windows native path to draw based on visible bounds / overhang metrics with row-level or viewport-level clipping
- Add glyph atlas safety padding for compatibility paths
- Remove host-side cursor ownership in native mode
- Ensure selection, cursor, underline, and IME preview follow the agreed draw order without obscuring real glyph pixels

**Step 4: Run tests to verify they pass**

Run: same command as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_scene_image.rs src/app/bootstrap.rs ui/shell/terminal-session-host.slint tests/terminal_glyph_origin_snap_spec.rs tests/terminal_glyph_cell_span_spec.rs tests/terminal_scene_image_spec.rs tests/native_terminal_surface_damage_rect_spec.rs tests/terminal_visible_bounds_regression_spec.rs
git commit -m "fix: align terminal visible bounds clip and native overlays"
```

### Task 6: Add Windows text-rendering diagnostics, debug overlay hooks, and regression observability

**Files:**
- Modify: `src/app/terminal_renderer/diagnostics.rs`
- Modify: `src/app/terminal_renderer/platform/windows.rs`
- Modify: `src/app/terminal_renderer/native_surface.rs`
- Modify: `src/app/windows_frame.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `tests/terminal_rendering_stack_follow_up_spec.rs`
- Modify: `tests/windows_frame_contract_smoke.sh`
- Create: `tests/windows_terminal_diagnostics_spec.rs`

**Step 1: Write the failing tests**

Add assertions that require diagnostics to expose:

- current renderer path
- active text antialias mode and alpha mode
- current font chain
- baseline and pixel-alignment state
- glyph screen bounds / visible bounds snapshots or equivalent trace data
- current DPI / scale factor

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_rendering_stack_follow_up_spec --test windows_terminal_diagnostics_spec -q
bash tests/windows_frame_contract_smoke.sh
```

Expected: FAIL, because current diagnostics snapshots do not yet expose enough Windows text-rendering information to debug the target issues quickly.

**Step 3: Write minimal implementation**

- Extend `src/app/terminal_renderer/diagnostics.rs` with the Windows text-rendering fields needed for support and regression debugging
- Update `src/app/terminal_renderer/platform/windows.rs` and `src/app/terminal_renderer/native_surface.rs` to publish those values
- Expose the diagnostics cleanly through the existing native surface inspection path without coupling them to release-only UI behavior

**Step 4: Run tests to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Verify compile quality**

Run:

```bash
cargo check --workspace
```

Expected: PASS

**Step 6: Commit**

```bash
git add src/app/terminal_renderer/diagnostics.rs src/app/terminal_renderer/platform/windows.rs src/app/terminal_renderer/native_surface.rs src/app/windows_frame.rs src/app/bootstrap.rs tests/terminal_rendering_stack_follow_up_spec.rs tests/windows_frame_contract_smoke.sh tests/windows_terminal_diagnostics_spec.rs
git commit -m "feat: add windows terminal text rendering diagnostics"
```

### Task 7: Run end-to-end verification, fallback checks, and packaging validation

**Files:**
- Modify: `docs/plans/2026-04-04-windows-terminal-text-rendering-design.md`
- Modify: `docs/plans/2026-04-04-windows-terminal-text-rendering-implementation-plan.md`
- Modify: `tests/build_desktop_windows_msvc_tool_shims_smoke.sh`
- Modify: `build-win-x64.sh`

**Step 1: Write the failing checks**

Add or tighten verification coverage so the final validation requires:

- Windows packaging to continue advertising the correct native renderer path
- Windows packaging and smoke hooks to continue advertising `directwrite-d2d` as the expected native primary text path for the Windows-first renderer
- design doc and implementation plan to reflect any implementation-era constraint adjustments
- final verification to cover `100%`, `125%`, `150%` DPI assumptions and `12px`, `13px`, `14px`, `15px` font-size expectations through source-level or scripted validation hooks

**Step 2: Run checks to verify they fail**

Run:

```bash
cargo test --test runtime_profile --test terminal_font_registration_smoke --test terminal_renderer_dwrite_spec --test native_terminal_surface_contract_spec --test terminal_visible_bounds_regression_spec --test windows_terminal_diagnostics_spec -q
bash tests/build_desktop_windows_msvc_tool_shims_smoke.sh
./build-win-x64.sh
```

Expected: At least one check FAILS until all implementation work and packaging hooks are aligned.

**Step 3: Write minimal implementation**

- Update packaging and smoke scripts only where needed to reflect the final Windows native text path
- Refresh plan/design docs if implementation uncovered an architecture-level constraint worth preserving
- Do not add new product features during final verification cleanup

**Step 4: Run checks to verify they pass**

Run: same commands as Step 2

Expected: PASS

**Step 5: Capture manual verification notes**

### Task 7 completion notes

**Implementation-era constraint notes:**

- Final verification uses source-level/scripted hooks to lock the Windows packaging contract because the repository cannot automatically run a full real-display DPI matrix during smoke validation.
- Verification hooks: `build-win-x64.sh` exports the authoritative Windows validation matrix and the smoke scripts keep the contract source-visible.
- The scripted validation must explicitly enumerate `100%`, `125%`, `150%` DPI assumptions and `12px`, `13px`, `14px`, `15px` font-size targets so future packaging or default-profile changes do not silently drift.
- Manual visual notes are still required for dark background English text, CJK, mixed CJK/Latin, Nerd Font, and emoji after the scripted hooks pass.

Record results for:

- dark background English text
- Chinese text
- mixed CJK/Latin lines
- Nerd Font / powerline glyphs
- emoji
- `100%`, `125%`, `150%` DPI
- `12px`, `13px`, `14px`, `15px`

Store notes in the implementation log or final verification report before merge. Automatic checks only validate that the hooks and target matrix remain present; real visual sign-off still needs manual notes.

**Step 6: Commit**

```bash
git add docs/plans/2026-04-04-windows-terminal-text-rendering-design.md docs/plans/2026-04-04-windows-terminal-text-rendering-implementation-plan.md tests/build_desktop_windows_msvc_tool_shims_smoke.sh build-win-x64.sh
git commit -m "chore: finalize windows terminal text rendering rollout"
```
