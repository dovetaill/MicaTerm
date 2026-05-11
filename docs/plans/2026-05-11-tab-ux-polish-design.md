# Tab UX Polish Design

Date: 2026-05-11
Owner: Codex
Status: Approved for implementation

## Scope

This is a focused polish pass on top of the completed session-first tab UX work.

It does not change session identity, tab command semantics, terminal rendering,
font metrics, ANSI color handling, selection, cursor flow, or reconnect/clone
behavior. It only improves three presentation issues reported in review:

- the workspace tab context menu is too narrow for its longest labels
- drag-reorder feedback is too weak to show the final drop location clearly
- the titlebar active-session summary should show `IP · tab name`, not an
  IP-only-feeling secondary detail

## Current Issues

### Context menu width and label overflow

`ui/components/workspace-tab-context-menu.slint` uses a fixed width of `236px`.
That width is not sufficient for the longest commands:

- `Close Tabs to the Right`
- `Close Tabs to the Left`

The shared row component in `ui/components/assets-context-menu-row.slint` also
assumes a single-line text region without an explicit overflow strategy, so a
narrow menu can visibly overrun instead of degrading gracefully.

### Drag preview is too subtle

The current reorder feedback in `ui/components/active-tab.slint` and
`ui/shell/tabbar.slint` only provides:

- a slight `-2px` vertical lift on the dragged tab
- a faint border and small opacity change
- a thin `2px` insertion line

That is not enough to create the browser/IDE-style "space opens where the tab
will land" mental model. The feedback feels like hover or press state instead
of an obvious reordering gesture.

### Titlebar summary hierarchy

The titlebar already has a session summary lane, but it is structured as
"display name" primary text with host/status as secondary detail. In narrower
widths, the host portion is the first thing to get compressed, which makes the
summary feel like it does not reliably communicate `IP + name`.

## Approved Direction: Option B

Use a balanced approach:

- widen the context menu and shorten the two longest labels slightly
- keep the menu single-level and desktop-like; do not add a submenu yet
- strengthen drag feedback with a visible placeholder gap plus a stronger
  insertion indicator and clearer dragged-tab state
- collapse the titlebar summary to a single primary string in the order
  `IP · tab name`
- move connection status to a weaker supporting role (tooltip and, if useful,
  very low-emphasis secondary text)

This gives a mature browser/IDE feel without adding heavy animation or changing
existing session semantics.

## Detailed Behavior

### Titlebar active-session summary

Show a single-line primary summary beside the logo:

- `10.0.0.12 · Prod Bastion`
- `172.22.0.2 · Interserver(7)`

Rules:

- IP comes first, tab name second
- when width is constrained, preserve as much of the IP as possible before
  truncating the tab name
- connection status should not compete with the main summary; keep it in the
  tooltip and only expose it inline if there is clear remaining space
- tooltip remains the full structured form:
  - tab name
  - host/IP
  - username
  - port
  - connection status

### Context menu sizing and wording

Keep the current single-level command list and desktop Fluent styling.

Adjustments:

- increase menu width into a safe desktop range around `284px-296px`
- keep one-line rows; do not wrap menu text to multiple lines
- shorten the two longest labels to:
  - `Close Right Tabs`
  - `Close Left Tabs`
- add explicit ellipsis handling in the shared row component as a safety net,
  even though the target width should avoid truncation in normal English UI

This is intentionally conservative. It fixes the bug-like overflow without
introducing a new interaction pattern.

### Drag reorder feedback

Adopt a restrained three-signal pattern inspired by Chromium and VS Code:

1. Dragged tab becomes visually "picked up"
   - stronger vertical lift than today
   - clearer border/surface separation
   - slightly lower opacity
2. Target location opens a visible placeholder gap
   - the user should perceive a tab-width slot being reserved
   - this must be more noticeable than a standalone line
3. A stronger accent insertion marker remains inside the placeholder
   - thicker/brighter than today
   - communicates the exact anchor point

Constraints:

- do not rebuild sessions or terminal surfaces
- do not write drag order back to session manager order
- do not add flashy spring motion or game-like overshoot
- keep Windows 11 Fluent / Mica restraint

Recommended motion envelope:

- dragged-state transition: fast, around `100-140ms`
- neighbor/gap response: around `120-160ms`
- settle on drop/cancel: around `120-160ms`

## Code Boundaries

Likely files:

- `ui/shell/titlebar.slint`
- `ui/app-window.slint`
- `src/app/bootstrap/shell_chrome.rs`
- `src/shell/view_model/workspace.rs`
- `src/shell/tabs.rs`
- `ui/components/workspace-tab-context-menu.slint`
- `ui/components/assets-context-menu-row.slint`
- `ui/components/active-tab.slint`
- `ui/shell/tabbar.slint`
- `tests/workspace_tabs_spec.rs`

Rust-side work should stay limited to summary string shaping and any contract
fields needed by the titlebar. Drag semantics should remain UI-only.

## External References

- Microsoft TabView guidance:
  https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/tab-view
- Windows motion guidance:
  https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/motion
- Chromium tabs overview:
  https://www.chromium.org/user-experience/tabs/
- Chromium tab strip design notes:
  https://www.chromium.org/developers/design-documents/tab-strip-mac/
- VS Code UI overview:
  https://code.visualstudio.com/docs/getstarted/userinterface
- VS Code drag/drop theme hooks:
  https://code.visualstudio.com/api/references/theme-color
