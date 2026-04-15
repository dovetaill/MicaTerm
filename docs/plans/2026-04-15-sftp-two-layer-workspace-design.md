# SFTP Two-Layer Workspace Design

Date: 2026-04-15
Author: Codex
Status: Approved for implementation planning

## Summary

`mica-term` should stop treating SFTP as a single narrow right-sidebar table, but it should also avoid drifting into a system-window-based file manager. The recommended direction is a two-layer model:

- a lightweight right-side `Quick Browser` for terminal-adjacent file access
- a full `SFTP Workspace Tab` in the main workspace for heavier browsing and file operations

This keeps the product terminal-first while making SFTP usable for both quick and extended workflows.

## Why This Change

The current right-panel-only SFTP model has four structural problems:

1. the right panel is too narrow for a durable file-management workflow
2. SFTP state is too tightly coupled to the active terminal tab
3. users cannot naturally keep multiple file views open in parallel
4. the product has no good path toward split view, transfer queue, or cross-host workflows

The design should fix those limits without turning `mica-term` into a clone of a standalone file manager.

## Product Positioning

`mica-term` is a terminal-first SSH product, not a dual-pane transfer client.

That means the correct target is not:

- system-level detached SFTP windows
- a permanent WinSCP-style full file manager
- a single global SFTP state that blindly follows whichever terminal tab is active

The correct target is:

- keep fast terminal-adjacent browsing on the right
- promote serious file work into the main workspace
- make file browsing sessions first-class but still subordinate to the terminal-first workflow

## Research Notes

Industry patterns point toward a layered model rather than a single narrow panel:

- `Termius` treats SFTP as a serious navigation surface alongside terminal workflows and supports dedicated SFTP views
- `MobaXterm` and `Xshell` both show that terminal-led products can keep file browsing tightly integrated without making it the only primary workspace
- `Cyberduck` and `FileZilla` show what happens when the entire product is centered around file browsing, which is not the right fit for `mica-term`

References:

- <https://termius.com/documentation/connect-with-sftp>
- <https://termius.com/blog/termius-for-ios-new-navigation-and-sftp>
- <https://mobaxterm.mobatek.net/features.html>
- <https://www.netsarang.com/en/xshell/>
- <https://docs.cyberduck.io/cyberduck/browser/>
- <https://wiki.filezilla-project.org/FileZilla_Client_Tutorial_%28en%29>

## Decision Summary

Adopt a two-layer SFTP model:

### Layer 1: Quick Browser

The right panel remains, but it is repositioned as a lightweight `Quick Browser`.

It is responsible for:

- quick inspection of the current remote directory
- fast upload and download entry points
- opening remote files
- optional follow-current-terminal-directory behavior
- creating a full SFTP workspace tab via `Expand to Workspace`

It is not responsible for:

- long-running heavy file management
- dense multi-column workflows
- side-by-side copy flows
- large-batch operations as the main entry point

### Layer 2: SFTP Workspace Tab

The main workspace gains a new tab type for SFTP browsing.

It is responsible for:

- wider file-table presentation
- independent lifecycle from terminal tabs
- sorting, refresh, parent navigation, multi-select, and bulk actions
- future expansion into split view, dual-pane, transfer queue, preview, and conflict handling

## Corrections to the Initial Expert Prompt

The expert prompt is directionally strong, but three parts should be adjusted for a cleaner MVP:

### 1. Prefer `Locked to Host/Profile` over `Locked to Connection`

For the first version, the important identity is the saved remote target, not a heavyweight global live connection abstraction. A strict `ConnectionId` model can be added later if needed, but it should not be the forcing function for the MVP.

### 2. Do not introduce one global mutable SFTP state

The quick browser and workspace tabs must not share a single global file-browsing state. That would recreate the current coupling problems in a different shape. Each SFTP surface needs its own browser session state, with explicit cloning or derivation.

### 3. Treat `Permissions` as a reserved column, not an MVP blocker

The current SFTP entry model does not yet expose permission metadata. The workspace table should reserve the conceptual slot, but MVP implementation should focus on `Name`, `Type`, `Size`, and `Modified`.

## Proposed Architecture

Four concepts need to be separated:

- remote target identity
- terminal session identity
- file browser session identity
- workspace tab identity

### Core Entities

#### `HostProfileRef`

Lightweight reference to the configured host/profile used to establish SSH and SFTP access.

Responsibilities:

- identify which remote host the browser targets
- survive terminal-session closure
- drive reconnect behavior for SFTP-only recovery

#### `TerminalSessionId`

Existing live shell session identity.

Responsibilities:

- identify the active shell session
- drive terminal rendering and terminal tab lifecycle
- optionally provide current working directory updates to the quick browser when in follow mode

#### `FileBrowserSessionId`

New independent file-browsing identity.

Responsibilities:

- track current remote path
- track history and navigation stack
- hold directory entries and loading/error state
- remember binding mode and reconnect capability

#### `WorkspaceTabId`

New workspace-level identity for displayed tabs.

Responsibilities:

- identify a visual tab instance
- allow terminal tabs and SFTP tabs to coexist in one workspace model
- support future split-workspace behavior

## Recommended Data Model Split

### `WorkspaceTab`

`WorkspaceTab` should become a discriminated union similar to:

- `Terminal { tab_id, terminal_session_id }`
- `Sftp { tab_id, file_browser_session_id }`
- `Launcher { tab_id }`

This is the key decoupling move. The workspace should no longer assume that every meaningful tab maps directly to a terminal session.

### `FileBrowserSession`

Each SFTP browser session should contain at least:

- `file_browser_session_id`
- `host_profile_ref`
- optional `attached_terminal_session_id`
- `binding_mode`
- `current_path`
- `path_history`
- `entries`
- `loading_state`
- `error_state`
- `sort_state`
- lightweight selection state

### `QuickBrowserState`

The right panel needs a dedicated state container that points at one browser session and records its panel-specific behavior, such as:

- active `file_browser_session_id`
- `FollowActiveTerminal` or `LockedToHostProfile`
- compact UI affordances
- temporary path editing state

### `FileBrowserViewState`

Longer-term, table view concerns should live outside transport/session logic. Even in MVP, design the split so these can be separated cleanly later:

- selection
- scroll position
- column widths
- table density
- sort order

## Session Relationships

The relationships should become:

- one `HostProfileRef` can have multiple terminal sessions
- one `HostProfileRef` can have multiple file browser sessions
- one quick browser uses one file browser session
- one workspace SFTP tab uses one file browser session
- `Expand to Workspace` creates or clones a file browser session; it does not merely widen the quick browser

This avoids accidental cross-talk between unrelated views.

## Interaction Model

## Quick Browser

The right panel should become intentionally lighter.

### Top Bar

Keep only frequent actions:

- current connection badge
- follow/locked status toggle
- refresh
- upload
- `Expand`
- overflow menu for less frequent actions

Avoid reintroducing a crowded toolbar with low-frequency icons.

### Path UX

Use a hybrid path control:

- breadcrumb display by default
- inline editable path mode on click or shortcut
- parent navigation as a dedicated action

This preserves scanability while still allowing direct path jumps.

### Binding Modes

The quick browser should support:

- `Follow Active Terminal` as the default
- `Locked to Host/Profile` as the secondary mode

In follow mode:

- if the active terminal changes to another compatible host, the quick browser updates
- if the terminal reports cwd changes, the quick browser may optionally track them

In locked mode:

- the quick browser stays on its current host/profile and path until explicitly changed

## Expand to Workspace

When the user clicks `Expand to Workspace`:

1. create a new `FileBrowserSession`
2. inherit at least:
   - current `host_profile_ref`
   - current `path`
3. preferably inherit:
   - current sort state
   - current visible selection if it is cheap to copy
4. create a new `WorkspaceTab::Sftp`
5. activate the new SFTP tab in the main workspace

The expanded tab should default to locked behavior. It should not automatically follow later terminal-tab switches.

## SFTP Workspace Tab

The SFTP workspace tab should support:

- tab titles like `Files: Sharon11111`
- parent navigation
- refresh
- enter directory
- basic multi-select
- table sorting
- wider columns than the quick browser

MVP columns:

- `Name`
- `Type`
- `Size`
- `Modified`

Reserved future column:

- `Permissions`

## Lifecycle Rules

### Terminal Tabs

Terminal tabs remain the live shell workspace.

### SFTP Tabs

SFTP tabs are separate workspace entities. They may reference the same host/profile as a terminal tab, but they do not disappear just because the original terminal tab changes or closes.

### On Terminal Close

If the terminal that originally informed the SFTP view closes:

- the SFTP workspace tab remains open
- it shows a disconnected state if no active transport is available
- it offers `Reconnect`
- reconnect restores file browsing only
- reconnect does not auto-open a new terminal tab

This keeps the file workflow stable and avoids unexpected shell churn.

## Error and Empty States

The design should explicitly support:

- no active terminal available
- host/profile no longer resolvable
- transport disconnected
- path load failed
- permission denied
- reconnect in progress

The quick browser should use concise, compact messaging. The workspace tab can afford a fuller inline status row or empty-state block.

## UI Scope for MVP

### Required in the Right Panel

- clear `Expand` action
- connection badge
- follow/locked status control
- lighter toolbar
- breadcrumb plus editable path
- simple lightweight list

### Required in the Main Workspace

- new SFTP tab type
- inherited host/profile plus path on open
- wider table layout
- sorting
- refresh
- parent navigation
- enter-directory flow
- multi-select basics

### Explicitly Deferred

- system-level separate windows
- dual-pane local/remote mode
- two-remote copy UX
- global transfer queue UI
- preview and inspector panels
- advanced conflict strategy UI
- full drag-and-drop choreography

## Code Areas Expected to Change

Likely touch points:

- `src/shell/tabs.rs`
- `src/shell/view_model/workspace.rs`
- `src/shell/view_model/sftp.rs`
- `src/app/sftp/model.rs`
- `src/app/sftp/browser_state.rs`
- `src/app/sftp/browser_controller.rs`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/workspace_terminal.rs`
- `src/app/bootstrap/sftp.rs`
- `ui/shell/right-panel.slint`
- `ui/shell/workspace-pane.slint`
- `ui/shell/tabbar.slint`

Likely additions:

- a dedicated SFTP workspace component in `ui/shell/`
- new view-model or model modules for browser sessions and SFTP workspace tabs

## Implementation Strategy

Implement in this order:

1. decouple workspace tab identity from terminal session identity
2. add `FileBrowserSession` as a first-class model
3. rework the quick browser around that new session model
4. add `WorkspaceTab::Sftp` and render it in the main workspace
5. support expand/clone flow from quick browser to workspace tab
6. add disconnect and reconnect handling for SFTP-only tabs

This order keeps the structural work ahead of the UI polish.

## Testing Strategy

The MVP should be verified at three levels:

### Model Tests

- file browser session creation and cloning
- binding mode transitions
- path navigation and parent-path behavior
- disconnect/reconnect state transitions

### View Model / Projection Tests

- workspace tab projection now supports terminal and SFTP tabs together
- quick browser follow mode reacts to active terminal changes
- expanded SFTP tab keeps its own independent session
- closing a terminal does not evict the SFTP tab

### UI / Integration Tests

- right-panel expand action opens a centered SFTP tab
- expanded tab inherits host/profile and path
- changing active terminal does not retarget a locked SFTP tab
- right panel still works in follow mode after expansion

## Future Extension Points

Design the code so the next stages can add:

- workspace split view
- local/remote dual-pane workflows
- remote-to-remote copy flows
- transfer queue surfaces
- overwrite/skip/rename conflict policies
- preview and properties sidepanels
- drag-and-drop upload/download

The MVP should not implement these now, but it should avoid painting the architecture into a corner.

## Final Recommendation

Proceed with:

- right-side `Quick Browser`
- center `SFTP Workspace Tab`
- independent `FileBrowserSession`
- workspace tabs decoupled from terminal sessions

This is the smallest architecture change that meaningfully upgrades SFTP from a narrow accessory into a flexible file workflow without abandoning `mica-term`'s terminal-first identity.
