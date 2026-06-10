# Terminal Selection Rust Ownership Design

**Date:** 2026-06-10

**Goal:** Move workspace terminal selection state and drag gesture ownership out of `ui/shell/terminal-session-host.slint` and into Rust so drag-selection remains anchored to stable buffer coordinates across scrollback movement, and `Select All` can target the full terminal buffer instead of only the visible viewport.

## Problem

The current terminal host keeps selection anchor/focus/drag state in Slint properties bound to viewport-local gesture origins. That creates two correctness failures:

1. During drag-selection, wheel scrolling updates the viewport but does not advance the active selection against the new buffer rows.
2. If the user continues dragging after scrolling, the host reinterprets the stored gesture origin against the new viewport origin, effectively rebasing the anchor to the currently visible screen instead of the original buffer row.
3. The terminal context-menu `Select All` action only spans `visible-top-buffer-row() .. visible-bottom-buffer-row-exclusive()`, so it selects only the visible shell screen instead of the full scrollback buffer.

## Design

### 1. Rust becomes the single source of truth for terminal selection

Add a Rust-side workspace-terminal selection controller that stores:
- the committed selection range in buffer coordinates;
- the active drag gesture metadata (gesture mode, anchor range, last pointer row/col, drag-active flag);
- enough surface identity to invalidate stale selection when the grid geometry or alt-screen mode changes.

The Slint host stops owning anchor/focus state. It only:
- performs hit-testing for the current pointer position;
- determines click streak mode (single/double/triple click);
- forwards selection begin/update/finish requests into Rust;
- renders the projected selection state coming back from Rust.

### 2. Scroll while dragging recomputes against the new surface, not the old viewport

When wheel scrolling happens during an active drag gesture:
- the host forwards the wheel request together with the current pointer row/column and selection column;
- bootstrap records the current pointer location in the Rust selection controller;
- bootstrap forwards the scroll to the session manager;
- if a drag gesture is active, bootstrap immediately re-syncs the active terminal surface from the manager and asks the Rust controller to recompute the selection range against the newly scrolled surface.

This keeps the anchor pinned to the original buffer row while letting the focus follow the same screen-space pointer position onto newly revealed buffer rows.

### 3. `Select All` becomes a Rust-owned full-buffer action

The terminal context menu no longer mutates selection properties locally. Instead it calls back into Rust, which computes:
- `start_row = 0`
- `start_col = 0`
- `end_row = rows + viewport_max_offset_lines - 1`
- `end_col = cols`

This makes `Select All` cover the entire available shell buffer, including scrollback above the visible viewport.

### 4. Bitmap and native renderers consume the same Rust-owned selection projection

Selection rendering remains split by renderer mode:
- bitmap mode keeps using the fast local Slint overlay rectangles;
- native mode keeps using the Rust/native presenter overlay.

But both paths project from the same Rust-owned buffer selection truth, so scrolling, copying, and context-menu actions all behave identically.

## Files

- `src/app/terminal_model.rs`
- `src/shell/view_model.rs`
- `src/shell/view_model/workspace.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/bootstrap.rs`
- `ui/shell/terminal-session-host.slint`
- `ui/shell/workspace-pane.slint`
- `ui/app-window.slint`
- `tests/bootstrap_smoke.rs`
- contract/spec tests that currently assert host-owned selection callbacks

## Risks

- Regressing immediate bitmap overlay updates if Rust callback projection is not applied synchronously.
- Breaking native selection repaint if the old `selection-changed` callback path is removed without replacing the presenter refresh hook.
- Regressing double-click / triple-click token and line expansion if the Rust controller does not preserve gesture-mode semantics.

## Verification

- Add a regression test that reproduces drag-selection + wheel scroll + continued drag.
- Add a regression test for context-menu `Select All` covering full scrollback.
- Keep existing selection invalidation tests passing for alt-screen and resize transitions.
