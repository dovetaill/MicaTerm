# Shell Focus Mode Design

**Date:** 2026-04-21

## Goal

Make it more obvious that the workspace-edge handles support one-click collapse/restore, and add a fast shell-level focus mode that hides the left and right sidebars so the terminal workspace can take the full width immediately.

## Problem Statement

The current edge-handle implementation already supports click-to-toggle plus drag-to-resize, but that behavior is still too easy to miss in practice:

- the handle still reads primarily as a resize affordance,
- users can fall back to dragging toward the collapse threshold because the click path is not discoverable enough,
- reclaiming the full terminal workspace still takes multiple actions when both side regions are open,
- there is no shell-level "focus workspace" action that temporarily clears auxiliary chrome and then restores the previous shell layout.

The result is that the terminal-first layout policy exists technically, but the fastest way to enter a distraction-free workspace is still not explicit enough.

## Chosen Approach

Add two complementary improvements:

1. strengthen the edge-handle hover guidance so users immediately understand that a short click collapses or restores a side region,
2. add a shell focus mode that hides the left assets sidebar and right utility panel together, then restores their previous requested visibility state when toggled off.

This keeps the existing edge-handle interaction model intact while introducing a higher-level "clear the workspace now" action for frequent terminal use.

## Alternatives Considered

### Option A — Keep only the current edge handles and just tweak visuals slightly

Pros:

- smallest code change,
- no new shell state to manage,
- preserves the current interaction model fully.

Cons:

- still requires two actions when both side regions are open,
- does not give users an explicit "focus the workspace" command,
- relies too heavily on users discovering hover behavior on their own.

Rejected because it improves clarity a little but does not solve the full-width workspace use case.

### Option B — Add edge-handle guidance plus a dedicated focus mode

Pros:

- keeps the low-friction single-click path for each edge,
- adds a one-shot action for the common "hide all side chrome" workflow,
- maps well to existing IDE patterns where users can temporarily hide auxiliary panes and restore them later.

Cons:

- introduces one more layout state to model,
- requires careful restore semantics so focus mode does not corrupt requested visibility state.

Chosen because it improves both discoverability and workflow speed without changing the core resize interaction.

### Option C — Add edge-handle double-click collapse instead of a dedicated focus mode

Pros:

- technically cheap once pointer handling exists,
- adds another fast affordance for power users.

Cons:

- weak discoverability,
- overlaps awkwardly with resize-handle expectations,
- still does not provide a one-shot "hide both sides" action.

Rejected because the interaction is harder to learn and less useful than an explicit focus mode.

## Industry References

- VS Code documents layout controls for showing and hiding the primary sidebar, secondary sidebar, and panel, and treats layout state as a first-class workspace concern.
- JetBrains IDEs document hiding and restoring tool windows as a dedicated workflow in addition to border drag resizing.

References:

- `https://code.visualstudio.com/docs/editor/custom-layout`
- `https://code.visualstudio.com/api/ux-guidelines/sidebars`
- `https://www.jetbrains.com/help/idea/tool-windows.html`
- `https://www.jetbrains.com/help/idea/manipulating-the-tool-windows.html`

## Interaction Design

### 1. Keep edge handles single-purpose and explicit

The edge handles should continue to support exactly two primary behaviors:

- short click: collapse or restore the adjacent side region,
- drag: resize the adjacent side region.

We should not add double-click semantics to the handle. The edge already communicates "resize," so stacking an additional double-click action on top of it makes the interaction less legible.

### 2. Improve edge-handle discoverability with hover guidance

The handle remains visually quiet at rest, but hover feedback becomes clearer.

Recommended copy:

- expanded left or right panel handle: `Click to collapse, drag to resize`
- collapsed revive strip: `Click to expand`

Recommended visual behavior:

- keep the handle narrow and low-noise by default,
- strengthen the hover contrast slightly so it feels intentionally interactive,
- keep the collapsed revive strip easier to hit than the live resize strip,
- do not turn the handle into a persistent button or a large branded affordance.

This preserves the terminal-first visual hierarchy while making the click path understandable without guesswork.

### 3. Add a shell focus mode for the workspace

Focus mode is a shell-level layout toggle with one purpose: maximize the terminal workspace by hiding the left and right side regions together.

Entering focus mode:

- captures the current requested visibility of the assets sidebar,
- captures the current requested visibility of the right panel,
- turns both requested visibility flags off,
- leaves remembered widths unchanged.

Exiting focus mode:

- restores the captured requested visibility flags,
- reuses the remembered widths already stored for each side,
- returns the shell to the same pre-focus layout intent.

Focus mode does not affect:

- transfer center visibility,
- titlebar visibility,
- modal dialogs,
- workspace tab state,
- terminal render mode or session state.

### 4. Resolve manual overrides by exiting focus mode

If focus mode is active and the user manually reopens either side region through an edge handle, legacy toggle, or titlebar button, the shell should exit focus mode immediately.

That rule keeps the model simple:

- focus mode means "both side regions are intentionally hidden",
- any explicit request to show one of them ends focus mode,
- the shell should not try to maintain a half-overridden focus state.

This is easier to reason about than layering partial overrides on top of a batch layout mode.

## Entry Points

### Titlebar button

Add a dedicated focus-mode button to the titlebar utility cluster.

Recommended placement:

- between the existing right-panel toggle button and the transfer-center button.

Reasoning:

- it belongs to workspace layout control,
- it is more discoverable here than inside the global menu,
- it stays adjacent to the existing right-panel control without being confused with sync or pin behavior.

Recommended tooltip copy:

- inactive: `Enter focus mode`
- active: `Exit focus mode`

### Keyboard shortcut

Add a local workspace shortcut:

- `Ctrl+Shift+M` toggles focus mode.

Reasoning:

- existing local shell shortcuts already reserve `Ctrl+Shift+T`, `Ctrl+Shift+W`, `Ctrl+Shift+P`, and `Ctrl+Shift+F`,
- `M` reads naturally as `maximize workspace` / `main workspace`,
- it can route cleanly through the existing terminal local-action path instead of inventing a separate keyboard subsystem.

## Architecture

### 1. Keep requested versus effective visibility ownership in Rust

The shell already separates requested visibility from effective visibility. Focus mode should build on top of that model, not replace it.

That means:

- focus mode changes requested visibility for the two side regions,
- layout resolution still decides effective visibility from requested state plus width budget,
- remembered widths remain independent from focus mode.

### 2. Add explicit shell focus-mode state

Add runtime shell state for:

- whether focus mode is currently active,
- the saved pre-focus requested visibility for the assets sidebar,
- the saved pre-focus requested visibility for the right panel.

This state remains runtime-only unless the product later decides focus mode should persist across launches.

### 3. Reuse existing toggle and width plumbing

The design should not fork separate code paths for focus mode versus normal side toggles.

Instead:

- focus mode should update the same requested visibility fields used by existing toggles,
- restoring from focus mode should reuse the same width memory already implemented for edge handles,
- titlebar and keyboard entry points should both call the same Rust helper.

## UI Contract Changes

### Slint properties

Add a focus-mode property exposed from Rust into `AppWindow` and threaded into the titlebar button state.

Suggested property:

- `workspace-focus-mode`

### Slint callbacks

Add a shell-level focus-mode callback:

- `toggle-workspace-focus-mode-requested()`

This callback should be available both from the titlebar button and from the local workspace keyboard action bridge.

## Testing Strategy

### View-model tests

Add or extend tests to verify:

- entering focus mode records the pre-focus requested state and hides both side regions,
- exiting focus mode restores the pre-focus requested state,
- remembered widths survive a focus-mode round trip,
- manually opening one side while focus mode is active exits focus mode cleanly.

### UI contract tests

Add or update source smoke tests to verify:

- the titlebar exposes the focus-mode button and callback wiring,
- the edge-handle tooltip copy reflects click-to-collapse and drag-to-resize guidance,
- `AppWindow` exports the focus-mode callback/property.

### Interaction smoke tests

Add generated-window smoke coverage for:

- clicking the titlebar focus button hides both side regions,
- clicking it again restores the original requested side visibility,
- `Ctrl+Shift+M` toggles focus mode locally without forwarding remote terminal input,
- manually reopening one side while focus mode is active exits focus mode.

## Risks And Mitigations

### Risk: focus mode and manual toggles fight each other

Mitigation:

- define the rule clearly: any explicit reopen request exits focus mode immediately.

### Risk: users still miss the edge-handle click affordance

Mitigation:

- update tooltip copy and hover feedback before adding any more complex pointer semantics.

### Risk: focus mode becomes a vague catch-all layout preset

Mitigation:

- keep the first version narrowly scoped to left/right sidebar visibility only.
- do not include transfer center, titlebar, or unrelated chrome in v1.
