# Sidebar Edge Handles Design

**Date:** 2026-04-21

## Goal

Make the left and right sidebars faster to dismiss and restore from the terminal workspace by moving the primary affordance to the workspace edges.

## Problem Statement

The current shell layout makes sidebar visibility changes feel more expensive than they should:

- the left assets sidebar is mainly collapsed from the far-left activity bar,
- the right SFTP panel is mainly toggled from the titlebar,
- both panels use fixed widths in Slint even though the responsive layout policy already treats them as auxiliary regions,
- the terminal workspace is the primary destination, but the fastest way to reclaim space is not anchored near the workspace itself.

This creates too much pointer travel for a frequent action and gives auxiliary chrome more attention than the terminal deserves.

## Chosen Approach

Use a mixed edge-handle interaction on both sides of the main workspace:

1. Add a narrow handle on the right edge of the left sidebar and the left edge of the right panel.
2. A short click on the handle toggles collapse/expand.
3. A drag on the handle resizes the panel width.
4. Dragging below a side-specific threshold auto-collapses the panel.
5. Re-expanding restores the last remembered expanded width for that side.
6. Keep the existing far-left and titlebar toggle buttons as secondary entry points.

This makes the fast path a single nearby action while still supporting finer width control.

## Alternatives Considered

### Option A — Add only a one-click edge toggle

Pros:

- cheapest implementation,
- fastest recovery for the most common collapse action.

Cons:

- no width control,
- still forces users to accept a single fixed panel size.

Rejected because it improves dismissal but not the "sidebars take too much attention" complaint.

### Option B — Add only drag-to-resize with threshold collapse

Pros:

- gives full control over panel width,
- matches common IDE panel resizing.

Cons:

- slower than a click when the user only wants to hide chrome and return to the terminal,
- increases precision demands for a very common action.

Rejected because the user explicitly wants the fewest possible actions.

### Option C — Edge handle with click-to-toggle plus drag-to-resize

Pros:

- one nearby click for the common case,
- drag for power users,
- remembered widths keep the layout stable after the first adjustment.

Chosen because it best matches the "simple and fast" requirement without sacrificing control.

## Industry References

- VS Code documents primary and secondary sidebars as hideable, layout-controlled regions and exposes layout controls close to the workbench surface.
- JetBrains tool windows are resized by dragging their borders and can remember per-tool-window custom sizes.

References:

- `https://code.visualstudio.com/docs/configure/custom-layout`
- `https://code.visualstudio.com/api/ux-guidelines/sidebars`
- `https://www.jetbrains.com/help/idea/manipulating-the-tool-windows.html`
- `https://www.jetbrains.com/help/idea/tool-windows.html`

## Architecture

### 1. Keep requested vs effective visibility split

Rust remains the single source of truth for whether the user wants each auxiliary region open (`requested`) and whether the current window width can actually show it (`effective`).

This existing policy stays intact:

- user actions change requested visibility,
- layout resolution computes effective visibility from requested state plus current window width,
- if the window narrows, requested state is preserved even when effective visibility becomes false,
- if the window widens again, the previously requested region can reappear automatically.

### 2. Add remembered widths per side

Store independent expanded widths for:

- the left assets sidebar content region,
- the right utility panel.

Recommended ranges:

- left sidebar width: `240px` to `420px`,
- right panel width: `320px` to `520px`.

Recommended defaults:

- left sidebar expanded width starts from the current visual size (`320px` total content block in Slint),
- right panel expanded width starts from the current `392px`.

When a panel is reopened after an explicit collapse or auto-collapse, it restores its last remembered expanded width.

### 3. Add edge handles at the workspace boundary

The primary interaction lives at the workspace boundaries inside `ui/app-window.slint`, where the shell already composes:

- `Sidebar`,
- `WorkspacePane`,
- `RightPanel`.

Each side gets a dedicated hit target:

- left handle sits on the right edge of the left sidebar,
- right handle sits on the left edge of the right panel,
- collapsed panels leave behind a slim revive strip on the workspace edge so they can be reopened without moving to distant chrome.

The handle should be visually quiet:

- default hit width about `8px` to `10px`,
- collapsed revive strip about `10px` to `12px`,
- visible accent only on hover or drag,
- resize cursor on hover,
- no large persistent button treatment.

### 4. Distinguish click from drag with a small motion threshold

The handle interaction uses a simple pointer threshold:

- pointer movement under `4px` is treated as a click,
- pointer movement at or above `4px` starts a resize drag.

This prevents a quick toggle from accidentally becoming a resize.

During drag:

- the left width is derived from the pointer's x-position relative to the shell body,
- the right width is derived from the distance between the pointer and the window's right edge,
- width is clamped to the allowed range,
- crossing the auto-collapse threshold collapses the panel.

Recommended collapse thresholds:

- left sidebar auto-collapses below `180px`,
- right panel auto-collapses below `220px`.

When the drag ends above threshold, the new clamped width becomes the remembered width.

### 5. Make layout resolution width-aware

The responsive policy in `src/shell/layout.rs` should stop assuming fixed auxiliary widths when panels are expanded.

Instead it should consume:

- requested left width,
- requested right width,
- current window width,
- requested visibility flags.

This keeps the terminal-first strategy but makes it honest about the actual occupied width:

- preserve the main workspace first,
- hide the left sidebar before the right panel when both cannot fit,
- keep requested visibility intact even when the layout hides one or both regions effectively.

### 6. Keep legacy buttons as secondary controls

The activity-bar toggle and titlebar right-panel button remain available, but they are no longer the primary path.

They should reuse the same Rust state transitions as the edge handles so every entry point:

- toggles the same requested visibility flags,
- restores the same remembered widths,
- stays consistent with responsive layout decisions.

## UI Contract Changes

### Slint properties

Add width properties that let Rust drive panel size instead of hard-coded literals:

- `assets-sidebar-expanded-width`
- `right-panel-expanded-width`

The left width ended up threading through both `ui/shell/sidebar.slint` and `ui/shell/assets-sidebar.slint` so the outer shell column and the inner assets content stay aligned on the same remembered value.

Keep the existing effective visibility booleans and use them to decide whether the live width is the expanded width or `0px`.

### Slint callbacks

Add edge-handle callbacks for both sides, with separate start/move/end lifecycle events so Rust owns the interaction state:

- `assets-sidebar-edge-toggle-requested()`
- `assets-sidebar-edge-drag-start-requested(length)`
- `assets-sidebar-edge-drag-move-requested(length)`
- `assets-sidebar-edge-drag-end-requested(length)`
- `right-panel-edge-toggle-requested()`
- `right-panel-edge-drag-start-requested(length)`
- `right-panel-edge-drag-move-requested(length)`
- `right-panel-edge-drag-end-requested(length)`

Exact callback naming can follow existing conventions, but the contract should stay symmetrical across both sides.

## State Model Changes

Add shell view-model state for:

- remembered left sidebar width,
- remembered right panel width,
- optional active edge drag session metadata if Rust needs to track drag origin and click-vs-drag classification.

The width values should remain runtime UI state only unless the project later decides to persist shell chrome preferences.

## Testing Strategy

### View-model tests

Add or extend tests for:

- left and right toggle actions preserving remembered width,
- drag resize updating the remembered width within bounds,
- dragging below threshold auto-collapsing the relevant panel,
- reopening after auto-collapse restoring the previous expanded width,
- narrow-window layout still preserving requested visibility while hiding regions effectively.

### UI contract tests

Add or update source-level smoke tests to verify:

- the fixed width literals are replaced by width properties,
- edge-handle callbacks exist in `ui/app-window.slint`,
- sidebar and right-panel components consume external width properties instead of owning fixed expanded widths,
- the collapsed revive strips exist at the workspace boundary.

### Interaction smoke tests

Add generated-window smoke coverage for:

- clicking the left handle toggles the assets sidebar,
- clicking the right handle toggles the right panel,
- width updates from drag handlers are reflected into Slint properties,
- terminal-area interactions remain unaffected outside the handle hit strips.

## Risks And Mitigations

### Risk: the handles steal too much workspace input

Mitigation:

- keep the hit area narrow,
- limit pointer capture to the handle strip,
- release drag state immediately on pointer up.

### Risk: click and drag feel inconsistent

Mitigation:

- use a small explicit movement threshold,
- keep the same threshold and visual feedback on both sides,
- centralize the logic in Rust so all entry points share the same behavior.

### Risk: dynamic widths drift out of sync with layout thresholds

Mitigation:

- resolve effective visibility from actual remembered widths,
- keep shared constants in `src/shell/metrics.rs`,
- update source tests that currently assert hard-coded widths.

## Success Criteria

- Users can collapse or reopen either sidebar from the workspace edge in one nearby action.
- Both auxiliary regions can be resized without moving to unrelated chrome.
- Dragging a panel below its collapse threshold hides it and preserves its last usable width.
- The terminal workspace remains the priority when the window narrows.
- Existing activity-bar and titlebar toggles keep working as secondary controls.
