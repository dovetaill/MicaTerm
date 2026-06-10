# Terminal Selection Rust Ownership Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Move workspace terminal selection state into Rust so drag-selection survives scrollback movement and terminal `Select All` spans the full buffer.

**Architecture:** Keep Slint responsible for pointer hit-testing and menu presentation, but move selection range + drag gesture ownership into Rust (`ShellViewModel` + workspace-terminal helpers). Recompute drag selection against the latest synced `TerminalSurfaceState`, and project the resulting buffer selection back into both bitmap and native rendering paths.

**Tech Stack:** Rust, Slint, workspace terminal bootstrap/session manager, existing terminal selection model helpers

---

### Task 1: Add regression coverage for scroll-drag selection and full-buffer select-all

**Files:**
- Modify: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing drag-scroll regression test**

Add a smoke test that:
- launches `ScrollProjectionLauncher`;
- begins a terminal selection near the bottom of the viewport;
- keeps the left button pressed while sending `WindowEvent::PointerScrolled`;
- continues the drag after scrolling;
- verifies the final selection still includes the original anchor buffer row and the newly revealed buffer rows.

**Step 2: Run the drag-scroll test to verify it fails**

Run: `cargo test workspace_terminal_drag_selection_survives_scrollback_reprojection -- --exact`

Expected: FAIL because the current host-owned gesture state rebinds the anchor to the new viewport after scrolling.

**Step 3: Write the failing context-menu select-all regression test**

Add a smoke test that:
- launches `ScrollbackCopyLauncher`;
- opens the terminal context menu;
- invokes `Select All`;
- verifies the projected selection spans the full buffer rows, not only the visible viewport;
- optionally copies the selection and verifies the clipboard contains all scrollback lines.

**Step 4: Run the select-all test to verify it fails**

Run: `cargo test workspace_terminal_context_menu_select_all_covers_full_scrollback -- --exact`

Expected: FAIL because the current context-menu action only selects the visible viewport rows.

### Task 2: Introduce Rust-owned workspace terminal selection drag state

**Files:**
- Modify: `src/app/terminal_model.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`

**Step 1: Add Rust-side drag selection types**

Create a Rust model for the active workspace terminal drag gesture that stores:
- surface identity (`session_id`, `rows`, `cols`, `alternate_screen_active`)
- gesture mode (`Cell`, `Word`, `Line`)
- anchor selection/range in buffer coordinates
- latest pointer viewport row / hit column / selection column

**Step 2: Add helper methods to recompute a selection range from the current surface**

Implement helpers that:
- keep cell-mode anchors in buffer coordinates;
- preserve double-click/triple-click expansion semantics by storing the anchor range and recomputing the focus range from the active surface;
- return a normalized `TerminalSelectionModel` for projection/copy.

**Step 3: Thread the drag state through `ShellViewModel`**

Add view-model accessors for:
- active drag state lookup
- begin/update/finish drag gesture
- clear drag state
- recompute the committed workspace selection from the active drag state when the surface changes

**Step 4: Run focused unit / compile checks**

Run: `cargo test terminal_session_spec -- --nocapture`

Expected: compile succeeds and existing selection helpers still pass.

### Task 3: Replace host-owned selection mutations with Rust callbacks

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`

**Step 1: Replace `selection-changed` / gesture-range callbacks with explicit selection gesture callbacks**

Update Slint callback contracts so the host emits:
- selection-begin-requested
- selection-update-requested
- selection-finish-requested
- scroll-requested(..., selection_col, ...)
- select-all-requested

and no longer stores anchor/focus rows locally.

**Step 2: Make bootstrap mutate Rust selection state synchronously**

In bootstrap handlers:
- update Rust selection state on begin/update/finish;
- project the new selection state back into window properties immediately;
- refresh the native presenter path when bitmap-host-only early returns do not apply.

**Step 3: Recompute drag selection immediately after scroll when needed**

When a wheel scroll arrives while a drag gesture is active:
- record the current pointer location into the Rust drag state;
- forward the viewport scroll to the session manager;
- immediately sync the active surface from the manager;
- recompute the drag selection against the new surface;
- then project the updated selection state.

**Step 4: Keep selection invalidation rules intact**

Ensure alt-screen entry and terminal resize still clear both:
- committed selection range
- active drag gesture state

### Task 4: Move terminal `Select All` to a full-buffer Rust action

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/workspace_terminal.rs`

**Step 1: Replace local viewport-only `Select All` mutation**

Change the terminal context-menu `Select All` row to call a Rust callback instead of mutating Slint selection properties.

**Step 2: Implement the full-buffer range calculation in Rust**

Use the active `TerminalSurfaceState` to compute the terminal-wide buffer span.

**Step 3: Reuse the projected selection for copy**

Keep `copy-selection-requested` reading from the Rust-owned active workspace selection so context-menu select-all + copy automatically covers the same full-buffer range.

### Task 5: Update contract tests and run focused verification

**Files:**
- Modify: `tests/workspace_tabs_spec.rs`
- Modify: `tests/native_terminal_surface_contract_spec.rs`
- Modify any terminal host contract tests that assert the old host-owned selection callback path

**Step 1: Update contract assertions**

Replace expectations for:
- `selection-changed()` callback propagation
- `resolve-selection-gesture-range(...)` callback plumbing

with assertions covering the new Rust-owned selection gesture callback chain.

**Step 2: Run focused regression tests**

Run:
- `cargo test workspace_terminal_drag_selection_survives_scrollback_reprojection -- --exact`
- `cargo test workspace_terminal_context_menu_select_all_covers_full_scrollback -- --exact`
- `cargo test workspace_terminal_entering_alt_screen_clears_existing_selection -- --exact`
- `cargo test workspace_terminal_surface_resize_clears_existing_selection -- --exact`

Expected: all PASS.

**Step 3: Run broader affected suites**

Run:
- `cargo test --test bootstrap_smoke workspace_terminal_ -- --nocapture`
- `cargo test --test native_terminal_surface_contract_spec -- --nocapture`
- `cargo test --test workspace_tabs_spec -- --nocapture`

Expected: PASS with updated callback contracts.
