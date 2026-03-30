# Terminal Atlas Selection And Visual Polish Design

## Goal

Keep the Rust atlas renderer as the single terminal rendering path while fixing three user-visible issues:

1. Mouse selection must visibly highlight selected text.
2. Glyphs must look less blurry, larger, and visually heavier.
3. ASCII and CJK mixed text must align more cleanly within the fixed grid.

## Current State

- The terminal surface image is produced entirely by [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs).
- Selection state currently lives only in [`ui/shell/terminal-session-host.slint`](../../ui/shell/terminal-session-host.slint).
- [`src/app/bootstrap.rs`](../../src/app/bootstrap.rs) re-renders the atlas only from `TerminalSurfaceState`, so changing selection state does not affect the image.
- Glyph alpha is currently written with linear coverage and conservative metrics, which makes text read small and gray.

## Root Cause

The selection visibility bug is architectural, not cosmetic:

- `TerminalSessionHost` tracks `selection-start-*` and `selection-end-*`.
- `TerminalAtlasRenderer::render()` only receives `TerminalSurfaceState`.
- Row hashing also only considers terminal cell content and colors.

Result: dragging a selection changes UI state but does not invalidate or repaint atlas rows, so the selection is functionally active but visually absent.

## Approved Approach

Implement selection directly inside the atlas renderer instead of adding a second Slint overlay path.

### Rendering

- Add an explicit atlas selection input that describes the active selected cell range.
- Fold selection into row hashing so selection changes invalidate affected rows.
- Paint selection background into the atlas image before glyph blitting.
- Swap selected foreground/background only when necessary for contrast; otherwise preserve readable foreground against the selection fill.

### Visual Tuning

- Increase the default terminal font size slightly.
- Recompute compact cell metrics around the new size instead of keeping extra vertical slack.
- Apply a coverage-to-alpha curve so strokes render darker and less washed out.
- Introduce separate horizontal placement rules for narrow ASCII clusters vs full-width CJK clusters.

### Non-Goals

- No return to Slint per-cell `Text`.
- No lazy font registration.
- No switch away from the current Sarasa NF+CJK font direction.
- No full TrueType hinting implementation in this iteration.

## Files

- Modify [`src/app/terminal_atlas.rs`](../../src/app/terminal_atlas.rs)
- Modify [`src/app/bootstrap.rs`](../../src/app/bootstrap.rs)
- Modify [`tests/terminal_atlas_renderer_spec.rs`](../../tests/terminal_atlas_renderer_spec.rs)
- Potentially extend smoke coverage in [`tests/bootstrap_smoke.rs`](../../tests/bootstrap_smoke.rs) if selection-triggered image refresh needs bootstrap-level locking

## Validation

- A renderer test must fail first when selection is omitted from redraw state.
- A renderer test must prove selected cells produce a different pixel buffer than unselected cells.
- Metrics tests must confirm the new default size is larger while staying grid-compact.
- Targeted Rust tests and `cargo check` must pass before claiming completion.
