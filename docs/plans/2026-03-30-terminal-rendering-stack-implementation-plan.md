# Terminal Rendering Stack Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current `ab_glyph + whole-image` terminal path with a staged rendering stack foundation, then land a Windows-first high-quality text path using `HarfBuzz + DirectWrite rasterization + GPU glyph atlas`, while preserving the old atlas bitmap presenter as a migration fallback for non-Windows platforms.

**Architecture:** Keep `TerminalSurfaceState`, session management, and the Slint shell intact. Insert a `TerminalPresenter -> TerminalModel -> TerminalLayout -> TerminalFont -> TerminalRenderer` pipeline between runtime and UI, and move terminal canvas drawing away from `slint::Image` toward a renderer hook inside the Slint window. Deliver the first production slice on Windows, but keep the abstractions cross-platform so Linux/macOS backends can slot in without another architecture rewrite.

**Tech Stack:** Rust 2024, Slint 1.15.1, winit backend, `wezterm-term`, HarfBuzz, DirectWrite, WGPU/renderer notifier integration, legacy atlas fallback, `cargo test`, `cargo check`, `cargo clippy`

---

### Task 1: Introduce a terminal presenter boundary without changing behavior

**Files:**
- Create: `src/app/terminal_presenter.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_atlas.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`

**Step 1: Write the failing contract tests**

Add focused assertions that the bootstrap path no longer depends directly on `TerminalAtlasRenderer` and instead consumes a presenter result.

Example assertions:

```rust
assert!(bootstrap_source.contains("TerminalPresenter"));
assert!(bootstrap_source.contains("PresentedTerminalFrame"));
assert!(!bootstrap_source.contains("TerminalAtlasRenderer::new()"));
```

Also extend the existing workspace/session refresh tests so they still require:

- active terminal surface changes update the visible terminal output;
- clearing the surface clears the published terminal frame state.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test workspace_tabs_spec -q
```

Expected: FAIL because the presenter module and new bootstrap contract do not exist yet.

**Step 3: Write the minimal implementation**

Create `src/app/terminal_presenter.rs` with a stable interface:

```rust
pub enum PresentedTerminalFrame {
    Bitmap(BitmapTerminalFrame),
    Native(NativeTerminalFrame),
}

pub struct BitmapTerminalFrame {
    pub image: slint::Image,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
}

pub struct NativeTerminalFrame {
    pub frame_token: u64,
    pub cell_width_px: u32,
    pub cell_height_px: u32,
}

pub trait TerminalPresenter {
    fn present(
        &mut self,
        surface: &TerminalSurfaceState,
    ) -> anyhow::Result<PresentedTerminalFrame>;
}
```

Then:

- move the old atlas-backed behavior behind `BitmapAtlasPresenter`;
- export the presenter module from `src/app/mod.rs`;
- update `src/app/bootstrap.rs` so it owns a `Box<dyn TerminalPresenter>` or concrete presenter enum instead of directly instantiating `TerminalAtlasRenderer`.

Do not remove the old atlas logic yet. This task is only about freezing a seam.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test workspace_tabs_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_presenter.rs src/app/mod.rs src/app/bootstrap.rs src/app/terminal_atlas.rs tests/bootstrap_smoke.rs tests/workspace_tabs_spec.rs
git commit -m "refactor: add terminal presenter boundary"
```

### Task 2: Extract a renderer-focused terminal model from `TerminalSurfaceState`

**Files:**
- Create: `src/app/terminal_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/terminal_presenter.rs`
- Test: `tests/terminal_model_spec.rs`
- Reference: `src/app/ssh/runtime.rs`

**Step 1: Write the failing model tests**

Add `tests/terminal_model_spec.rs` covering:

- visible rows preserve grapheme text and color spans;
- cursor and selection metadata survive projection;
- dirty-row detection can distinguish unchanged rows from changed rows.

Example test shape:

```rust
#[test]
fn terminal_model_marks_only_changed_rows_dirty() {
    let previous = TerminalModelFrame::from_surface(&surface_a, None);
    let next = TerminalModelFrame::from_surface(&surface_b, Some(&previous));
    assert_eq!(next.dirty_rows, vec![3, 4]);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_model_spec -q
```

Expected: FAIL because the terminal model layer does not exist yet.

**Step 3: Write the minimal implementation**

Create `src/app/terminal_model.rs` with a renderer-facing model:

```rust
pub struct TerminalModelFrame {
    pub rows: Vec<TerminalModelRow>,
    pub cursor: TerminalCursorModel,
    pub selection: Option<TerminalSelectionModel>,
    pub palette: TerminalPaletteModel,
    pub dirty_rows: Vec<u32>,
}

pub struct TerminalModelRow {
    pub row_index: u32,
    pub cells: Vec<TerminalModelCell>,
    pub row_hash: u64,
}
```

Add a constructor like:

```rust
impl TerminalModelFrame {
    pub fn from_surface(
        surface: &TerminalSurfaceState,
        previous: Option<&TerminalModelFrame>,
    ) -> Self { /* ... */ }
}
```

Keep it pure:

- no HarfBuzz;
- no platform API;
- no Slint types;
- no renderer state.

Wire the presenter/bootstrap path so atlas presentation uses `TerminalModelFrame` as input instead of reading `TerminalSurfaceState` directly.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_model_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_model.rs src/app/bootstrap.rs src/app/terminal_presenter.rs tests/terminal_model_spec.rs
git commit -m "refactor: add terminal render model"
```

### Task 3: Add the layout engine and HarfBuzz shaping contracts

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/terminal_layout/mod.rs`
- Create: `src/app/terminal_layout/run_segmentation.rs`
- Create: `src/app/terminal_layout/shaper.rs`
- Create: `src/app/terminal_font/mod.rs`
- Create: `src/app/terminal_font/backend.rs`
- Create: `src/app/terminal_font/mock.rs`
- Modify: `src/app/mod.rs`
- Test: `tests/terminal_layout_harfbuzz_spec.rs`
- Test: `tests/terminal_session_spec.rs`

**Step 1: Write the failing shaping tests**

Add `tests/terminal_layout_harfbuzz_spec.rs` covering:

- ASCII prompt text stays single-run under consistent styling;
- emoji and wide CJK characters keep stable cluster boundaries;
- style or foreground changes split runs;
- ligature-capable runs return clustered glyphs instead of raw `chars()`.

Example test shape:

```rust
#[test]
fn harfbuzz_layout_splits_on_foreground_change_but_not_background_change() {
    let shaped = shape_row(&row, &mut mock_font_system())?;
    assert_eq!(shaped.runs.len(), 2);
    assert_eq!(shaped.runs[0].cell_range, 0..3);
}
```

Also add a guard in `tests/terminal_session_spec.rs` that the old atlas path is no longer the only place where cell width logic lives.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_layout_harfbuzz_spec --test terminal_session_spec -q
```

Expected: FAIL because there is no layout engine or HarfBuzz contract yet.

**Step 3: Write the minimal implementation**

Add HarfBuzz to `Cargo.toml` and create shared layout/font traits:

```rust
pub struct GlyphRun {
    pub row: u32,
    pub cell_range: std::ops::Range<u32>,
    pub glyphs: Vec<PositionedGlyph>,
    pub style: TextStyleKey,
}

pub trait TextShaper {
    fn shape_row(
        &mut self,
        row: &TerminalModelRow,
        fonts: &mut dyn FontSystem,
    ) -> anyhow::Result<ShapedRow>;
}

pub trait FontSystem {
    type FaceId: Copy + Eq;

    fn resolve_face(&mut self, request: &FontRequest) -> anyhow::Result<Self::FaceId>;
    fn metrics(&mut self, face: Self::FaceId, px_size: f32) -> anyhow::Result<FontMetrics>;
}
```

Implement:

- run segmentation from `TerminalModelRow` based on style / foreground / grapheme boundaries;
- a mock font system for tests;
- a HarfBuzz-backed shaper that outputs glyph ids and positions but does not rasterize yet.

Do not introduce platform rasterization here. This task only builds the common shaping layer.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_layout_harfbuzz_spec --test terminal_session_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add Cargo.toml src/app/terminal_layout src/app/terminal_font src/app/mod.rs tests/terminal_layout_harfbuzz_spec.rs tests/terminal_session_spec.rs
git commit -m "feat: add harfbuzz terminal layout layer"
```

### Task 4: Replace the `Image`-only terminal host contract with a renderer hook

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/main.rs`
- Create: `src/app/terminal_renderer/mod.rs`
- Create: `src/app/terminal_renderer/native_surface.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Write the failing UI/render contract tests**

Add a new `tests/native_terminal_surface_contract_spec.rs` asserting:

- `TerminalSessionHost` exposes a renderer mode property;
- `session-surface-image` remains available only as fallback;
- bootstrap/runtime profile can switch between `bitmap` and `native` terminal render modes.

Example assertions:

```rust
assert!(host_source.contains("in property <string> session-render-mode"));
assert!(host_source.contains("session-surface-image"));
assert!(runtime_profile_source.contains("TerminalRenderMode"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test native_terminal_surface_contract_spec -q
```

Expected: FAIL because the UI and runtime profile do not have native render-mode contracts yet.

**Step 3: Write the minimal implementation**

Add a renderer mode contract:

```rust
pub enum TerminalRenderMode {
    Bitmap,
    Native,
}
```

Update the Slint host with fallback-friendly properties:

```slint
in property <string> session-render-mode: "bitmap";
in property <image> session-surface-image;
in property <int> session-native-frame-token: 0;
```

Create `src/app/terminal_renderer/native_surface.rs` with a small integration layer that uses Slint's window rendering notifier or shared renderer hook to register terminal draw callbacks against the existing window.

The first implementation can be a no-op native surface that:

- registers successfully;
- receives terminal rect updates;
- does not yet draw text.

Wire `src/app/runtime_profile.rs` / `src/main.rs` so a new runtime profile can opt into `TerminalRenderMode::Native` on supported builds.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test native_terminal_surface_contract_spec -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add ui/shell/terminal-session-host.slint ui/app-window.slint src/app/bootstrap.rs src/app/runtime_profile.rs src/main.rs src/app/terminal_renderer tests/bootstrap_smoke.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "refactor: add native terminal render host contract"
```

### Task 5: Implement the Windows-first high-quality text path

**Files:**
- Create: `src/app/terminal_font/windows_dwrite.rs`
- Create: `src/app/terminal_renderer/atlas.rs`
- Create: `src/app/terminal_renderer/wgpu_renderer.rs`
- Modify: `src/app/terminal_font/mod.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `Cargo.toml`
- Test: `tests/terminal_renderer_dwrite_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing Windows renderer tests**

Add `tests/terminal_renderer_dwrite_spec.rs` behind `#[cfg(target_os = "windows")]` covering:

- DirectWrite metrics round-trip for the chosen monospace face;
- repeated glyph rasterization reuses atlas entries;
- shaping output from Task 3 can be rasterized and staged without going through `slint::Image`;
- a native frame token increments when the renderer receives a changed terminal frame.

Example test shape:

```rust
#[test]
fn dwrite_renderer_reuses_glyph_atlas_entries() -> anyhow::Result<()> {
    let mut renderer = WgpuTerminalRenderer::new_for_test()?;
    let frame_a = renderer.prepare(&shaped_frame)?;
    let frame_b = renderer.prepare(&shaped_frame)?;
    assert_eq!(frame_a.glyph_cache_entries, frame_b.glyph_cache_entries);
    Ok(())
}
```

**Step 2: Run tests to verify they fail**

On Windows, run:

```bash
cargo test --test terminal_renderer_dwrite_spec --test bootstrap_smoke -q
```

Expected: FAIL because the DirectWrite backend and native renderer do not exist yet.

**Step 3: Write the minimal implementation**

Implement `windows_dwrite.rs` with:

```rust
pub struct DirectWriteFontSystem { /* factories, face cache, fallback */ }

impl FontSystem for DirectWriteFontSystem {
    type FaceId = FontFaceKey;

    fn resolve_face(&mut self, request: &FontRequest) -> anyhow::Result<Self::FaceId> { /* ... */ }
    fn metrics(&mut self, face: Self::FaceId, px_size: f32) -> anyhow::Result<FontMetrics> { /* ... */ }
}

impl DirectWriteFontSystem {
    pub fn rasterize(&mut self, request: GlyphRasterRequest) -> anyhow::Result<RasterizedGlyph> { /* ... */ }
}
```

Implement `wgpu_renderer.rs` / `atlas.rs` with:

- glyph texture atlas;
- instance list per glyph;
- background, selection, and cursor draw list slots;
- a `prepare()` method that converts `ShapedFrame` into GPU-ready state.

Wire the presenter so on Windows native mode it returns:

```rust
PresentedTerminalFrame::Native(NativeTerminalFrame {
    frame_token,
    cell_width_px,
    cell_height_px,
})
```

Keep non-Windows on the bitmap presenter for now.

**Step 4: Run tests to verify they pass**

On Windows, run:

```bash
cargo test --test terminal_renderer_dwrite_spec --test bootstrap_smoke -q
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_font/windows_dwrite.rs src/app/terminal_renderer/atlas.rs src/app/terminal_renderer/wgpu_renderer.rs src/app/terminal_font/mod.rs src/app/terminal_presenter.rs src/app/bootstrap.rs Cargo.toml tests/terminal_renderer_dwrite_spec.rs tests/bootstrap_smoke.rs
git commit -m "feat: add windows native terminal renderer"
```

### Task 6: Preserve fallback behavior and complete regression coverage

**Files:**
- Modify: `src/app/terminal_atlas.rs`
- Modify: `src/app/terminal_presenter.rs`
- Modify: `src/app/runtime_profile.rs`
- Modify: `src/app/logging/runtime.rs`
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `docs/plans/2026-03-30-terminal-rendering-stack-design.md`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/terminal_scrollback_spec.rs`
- Test: `tests/ssh_session_manager_spec.rs`

**Step 1: Add the failing migration regression checks**

Ensure regression suites verify:

- non-Windows builds still render through the bitmap presenter;
- Windows native mode falls back cleanly to bitmap mode if native renderer setup fails;
- copy/paste, scrollback, selection, mouse reporting, and resize still work after the presenter split.

Example expectation:

```rust
assert_eq!(profile.terminal_render_mode(), TerminalRenderMode::Bitmap);
assert!(surface_seqno_after_refresh > surface_seqno_before_refresh);
```

**Step 2: Run the regression suites and confirm failure**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec -q
```

Expected: FAIL until the fallback and runtime-selection path are fully wired.

**Step 3: Write the minimal implementation**

- Make `BitmapAtlasPresenter` the default on non-Windows.
- On Windows native mode, if renderer setup fails, log the failure and fall back to bitmap mode instead of leaving the terminal blank.
- Expose renderer mode in runtime logging:

```rust
info!(
    target: "app.renderer",
    terminal_render_mode = ?profile.terminal_render_mode(),
    "configured terminal renderer"
);
```

- Update `readme.md` and `verification.md` with:
  - current platform support matrix;
  - Windows-first native renderer status;
  - remaining Linux/macOS follow-up work.

**Step 4: Run the focused regressions**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec --test terminal_scrollback_spec --test ssh_session_manager_spec -q
cargo check --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS

**Step 5: Commit**

```bash
git add src/app/terminal_atlas.rs src/app/terminal_presenter.rs src/app/runtime_profile.rs src/app/logging/runtime.rs readme.md verification.md docs/plans/2026-03-30-terminal-rendering-stack-design.md tests/workspace_tabs_spec.rs tests/ssh_terminal_interaction_spec.rs tests/terminal_scrollback_spec.rs tests/ssh_session_manager_spec.rs
git commit -m "docs: verify terminal renderer migration path"
```

### Task 7: Create the follow-up backlog for Linux/macOS backends and the `libghostty` stop-loss

**Files:**
- Modify: `docs/plans/2026-03-30-terminal-rendering-stack-design.md`
- Create: `docs/plans/2026-03-30-terminal-rendering-stack-follow-up.md`

**Step 1: Write the follow-up backlog**

Create a short backlog document that explicitly tracks:

- `linux_freetype_fontconfig.rs`
- `macos_coretext.rs`
- moving cursor/selection fully into the renderer
- the trigger conditions for switching to the `libghostty` stop-loss route

Use a concrete table:

```markdown
| Item | Trigger | Owner | Notes |
| --- | --- | --- | --- |
| Linux font backend | Windows MVP stable | TBD | Reuse HarfBuzz layout layer |
```

**Step 2: Save the backlog and verify links**

Run:

```bash
rg -n "terminal-rendering-stack-follow-up|libghostty" docs/plans
```

Expected: both the design doc and follow-up doc reference the stop-loss path and next platform milestones.

**Step 3: Commit**

```bash
git add docs/plans/2026-03-30-terminal-rendering-stack-design.md docs/plans/2026-03-30-terminal-rendering-stack-follow-up.md
git commit -m "docs: add terminal renderer follow-up backlog"
```
