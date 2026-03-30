# Maple Atlas Terminal Renderer Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current per-cell Slint terminal text renderer with a Maple-backed atlas renderer that outputs a single terminal surface image, while removing the Sarasa/Iosevka terminal font path entirely.

**Architecture:** Keep the existing terminal emulation/runtime model, but replace the UI rendering path with a Rust-owned atlas core that consumes `TerminalSurfaceState`, caches grapheme sprites from `MapleMonoNormalNL-NF-CN`, and publishes a single `slint::Image` plus cell metrics into the window. Keep selection, cursor, scrollbar, mouse reporting, and context menu behavior in Slint overlays so the atlas surface only changes when terminal content changes.

**Tech Stack:** Rust 2024, Slint 1.15.1, `slint::Image` / `SharedPixelBuffer`, bundled TTF font asset, terminal surface projection from `wezterm-term` runtime, `cargo test`, `cargo check`

---

### Task 1: Replace the terminal font contract with Maple only

**Files:**
- Modify: `Cargo.toml`
- Modify: `build.rs`
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Delete: `ui/fonts/IosevkaTerm-Regular.ttf`
- Delete: `ui/fonts/SarasaTermSCNerd-Regular.ttf`
- Create: `ui/fonts/MapleMonoNormalNL-NF-CN-Regular.ttf`
- Test: `tests/startup_font_memory_regression.rs`
- Test: `tests/terminal_font_registration_smoke.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`

**Step 1: Write the failing tests**

- Update the font contract tests to reject any `Sarasa` / `Iosevka` terminal path.
- Make them require a single Maple terminal family string.
- Make them reject lazy registration helpers entirely.

Example target assertions:

```rust
assert!(!content.contains("IosevkaTerm-Regular.ttf"));
assert!(!content.contains("SarasaTermSCNerd-Regular.ttf"));
assert!(content.contains("Maple Mono"));
assert!(!content.contains("ensure_terminal_font_registered"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke --test workspace_tabs_spec --test ssh_terminal_interaction_spec -q
```

Expected: FAIL because the current source still references `Sarasa`, `Iosevka`, and lazy font registration.

**Step 3: Write minimal implementation**

- Add `MapleMonoNormalNL-NF-CN-Regular.ttf` under `ui/fonts/`.
- Remove the old bundled terminal font files from the repo.
- Remove any `.slint` imports or runtime helpers tied to `Sarasa` / `Iosevka`.
- Update source contracts and docs so the terminal font family is Maple-only.
- Delete the no-longer-valid lazy terminal font module if it is now unused.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test startup_font_memory_regression --test terminal_font_registration_smoke --test workspace_tabs_spec --test ssh_terminal_interaction_spec -q
```

Expected: PASS

### Task 2: Introduce the atlas renderer core and metrics contract

**Files:**
- Create: `src/app/terminal_atlas.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/ssh/runtime.rs`
- Test: `tests/terminal_atlas_renderer_spec.rs`
- Test: `tests/terminal_session_spec.rs`

**Step 1: Write the failing tests**

- Add `tests/terminal_atlas_renderer_spec.rs` to cover:
  - Maple metrics are loaded successfully
  - repeated graphemes reuse atlas cache entries
  - wide CJK glyphs and Nerd Font icons produce stable sprite widths
  - dirty-row updates do not invalidate untouched rows
- Extend `tests/terminal_session_spec.rs` so runtime snapshots expose any new atlas-facing metadata needed for rendering.

Example test shape:

```rust
#[test]
fn atlas_renderer_reuses_cached_sprite_for_repeated_prompt_glyph() {
    let mut renderer = TerminalAtlasRenderer::new()?;
    let frame_a = renderer.render(&surface)?;
    let frame_b = renderer.render(&surface)?;
    assert_eq!(frame_a.cache_entries, frame_b.cache_entries);
}
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test terminal_session_spec -q
```

Expected: FAIL because the atlas renderer module and new contracts do not exist yet.

**Step 3: Write minimal implementation**

- Create `src/app/terminal_atlas.rs` with:
  - `TerminalAtlasRenderer`
  - `TerminalAtlasMetrics`
  - `TerminalSurfaceFrame`
  - atlas cache structures
- Load `MapleMonoNormalNL-NF-CN-Regular.ttf` from bytes in Rust.
- Compute cell width/height in Rust instead of via Slint text probes.
- Render terminal graphemes into an RGBA buffer and expose it as `slint::Image`.
- Add a simple dirty-row tracking path so unchanged rows are not rerasterized.
- Export the module from `src/app/mod.rs`.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test terminal_session_spec -q
```

Expected: PASS

### Task 3: Replace the UI cell repeater with a single terminal surface image

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing tests**

- Update source-contract tests so they reject the old `for cell in root.session-cells` repeater.
- Require a single terminal surface image property instead.
- Keep the existing callback contracts for resize, copy, paste, scroll, and mouse input.

Example assertions:

```rust
assert!(!terminal_host.contains("for cell in root.session-cells"));
assert!(terminal_host.contains("workspace-session-surface-image"));
assert!(terminal_host.contains("Image {"));
```

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec --test bootstrap_smoke -q
```

Expected: FAIL because the current terminal host still uses a per-cell repeater and no image contract exists.

**Step 3: Write minimal implementation**

- Add window properties for:
  - terminal surface image
  - cell width
  - cell height
- Replace the terminal text repeater in `TerminalSessionHost` with a single `Image` that fills the terminal canvas.
- Preserve selection/cursor/scrollbar overlays and hit-testing logic, but bind them to Rust-provided cell metrics instead of text probes.
- In `bootstrap.rs`, instantiate/own the atlas renderer and publish `slint::Image` frames into the window whenever the active terminal surface changes.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test workspace_tabs_spec --test ssh_terminal_interaction_spec --test bootstrap_smoke -q
```

Expected: PASS

### Task 4: Remove the UI-side `session-cells` / `visible-lines` hot path

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/terminal_scrollback_spec.rs`
- Modify: `tests/ssh_session_manager_spec.rs`

**Step 1: Write the failing tests**

- Update tests that currently assume UI state owns `workspace_session_cells` or `workspace_session_visible_lines`.
- Move those checks to runtime/state-level assertions where appropriate.
- Add assertions that UI image reuse happens across refreshes and clears.

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_scrollback_spec --test ssh_session_manager_spec -q
```

Expected: FAIL because the current tests and bootstrap path still depend on the old UI models.

**Step 3: Write minimal implementation**

- Stop pushing `surface.cells` into Slint models.
- Stop maintaining a UI text mirror for `visible_lines` unless a test/runtime path still strictly requires it.
- Keep `TerminalSurfaceState` focused on runtime behavior; if selection copy still needs row/cell data, keep it runtime-side only.
- Update bootstrap refresh logic so image state and surface seqno are the primary terminal render outputs.

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test bootstrap_smoke --test terminal_scrollback_spec --test ssh_session_manager_spec -q
```

Expected: PASS

### Task 5: End-to-end verification, docs refresh, and regression coverage

**Files:**
- Modify: `readme.md`
- Modify: `verification.md`
- Modify: `tests/terminal_session_spec.rs`
- Modify: `tests/ssh_terminal_interaction_spec.rs`
- Modify: `tests/workspace_tabs_spec.rs`
- Optional Create: `tests/terminal_atlas_memory_smoke.rs`

**Step 1: Write the failing verification tests**

- Add or update regression tests for:
  - prompt rendering
  - wide characters
  - Nerd Font icon rendering
  - copy-selection behavior after atlas migration
  - scrollback behavior after image-backed rendering

**Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test workspace_tabs_spec -q
```

Expected: FAIL until the atlas renderer path and docs are fully aligned.

**Step 3: Write minimal implementation**

- Refresh docs so they describe Maple + atlas renderer as the new terminal path.
- Add any missing contract tests for memory-sensitive renderer behavior.
- Ensure no lingering source/docs references to:
  - `Sarasa`
  - `Iosevka`
  - lazy terminal font registration
  - per-cell Slint text rendering

**Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --test terminal_session_spec --test ssh_terminal_interaction_spec --test workspace_tabs_spec -q
cargo check
```

Expected: PASS

### Task 6: Full verification pass before claiming completion

**Files:**
- Verify only

**Step 1: Run the focused terminal suite**

Run:

```bash
cargo test --test terminal_atlas_renderer_spec --test terminal_session_spec --test terminal_scrollback_spec --test ssh_terminal_interaction_spec --test workspace_tabs_spec --test bootstrap_smoke -q
```

Expected: PASS

**Step 2: Run a full compile check**

Run:

```bash
cargo check
```

Expected: PASS

**Step 3: Inspect the diff**

Run:

```bash
git status --short
git diff --stat
```

Expected: only the intended Maple/atlas terminal renderer files are changed.

**Step 4: Commit**

```bash
git add Cargo.toml build.rs readme.md verification.md ui src tests docs/plans/2026-03-30-maple-atlas-terminal-renderer-design.md docs/plans/2026-03-30-maple-atlas-terminal-renderer-implementation-plan.md
git commit -m "feat: replace terminal text nodes with maple atlas renderer"
```
