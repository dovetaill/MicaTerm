# Async SFTP Browser And Transfer Center Redesign

## Context

The current SFTP quick browser already has an asynchronous directory-loading foundation, but the user-visible product still feels blocking because several high-impact paths remain synchronous and some projected UI state churn is too heavy. The most damaging problems are not purely visual: `Open` still downloads synchronously on the UI path, remote file save still uploads synchronously, some file operations only mutate local view-model state, the ready-state blank area lacks a context menu trigger, and the transfer center still behaves more like a placeholder list than an actionable task center.

This redesign keeps the existing browser-controller/session-state foundation and completes it instead of replacing it. The goal is to make every SFTP network operation background-driven, define honest file-operation semantics, and turn the transfer center into the single place where long-running work and recovery actions live.

## Verified Findings

1. Directory browsing is already partially asynchronous through `queue_sftp_browser_request(...)`, so the expert report is directionally right but not fully root-cause accurate.
2. `SessionManager` still exposes synchronous wrappers using `block_on(...)` for directory reads, downloads, and uploads, and the UI still hits those wrappers on critical interaction paths.
3. `Open` currently downloads the remote file synchronously before showing the modal editor.
4. Remote file save currently uploads synchronously from the modal save callback.
5. Browser/session projection runs on a 50ms timer and currently clones large browser/session payloads more often than necessary.
6. Some SFTP mutations (`New Folder`, `Delete`, `Rename`) still mutate local browser state without performing the remote operation.
7. The empty-state panel exposes a blank-area context menu trigger, but the ready-state list host does not.
8. The titlebar transfer badge is a raw count without clear semantics, while the transfer center row model still lacks the high-frequency actions users actually need.

## Goals

1. Remove UI-thread blocking from SFTP browsing, file open, remote editing, and transfer-triggering paths.
2. Make every context-menu action either truly functional or clearly disabled with an explanation.
3. Redefine `Open` and `Edit Locally` so they match common industry behavior and never freeze the UI.
4. Rebuild the transfer center into a compact task center with row actions for running, failed, and completed items.
5. Improve loading, stale, syncing, error, and destructive-action feedback without turning the quick browser into a heavy workspace.

## Non-Goals

1. This round does not introduce a full remote IDE or bidirectional live folder sync.
2. This round does not promise pause/resume support unless the existing backend can truly honor it.
3. This round does not ship server-side copy/cut/paste unless the backend path is real; those entries stay disabled with an explicit reason.

## Chosen Product Semantics

### Open vs Edit

- `Open` means `Download and Open Locally`.
- `Edit Locally` means `download working copy -> local edit -> asynchronous upload back on save/change`.
- Text/code/config files can expose `Edit Locally` as the primary action.
- Binary, large, or unknown files default to `Open` or `Download To...`.
- The current synchronous remote-text modal is no longer the default `Open` path.

This split follows the common pattern used by WinSCP, Cyberduck, and Xftp, while also respecting FileZilla's safer distinction between local-temp editing and upload-back confirmation.

### Honest Capability Rules

- `Rename`, `Delete`, `New Folder`, `Download To...`, and `Open` must execute real backend work.
- `Copy`, `Cut`, `Paste`, and `Permissions...` are shown only if they are real, otherwise disabled with a clear reason.
- No menu item is allowed to look clickable and then silently do nothing.

## Architecture Decisions

### 1. Unified asynchronous SFTP operation dispatcher

All SFTP network work moves behind one background-dispatch layer. Directory loads, download/open, edit-working-copy setup, upload-back, rename, delete, mkdir, and permission reads/writes all become asynchronous operations dispatched on the runtime and reported back over result channels.

The UI thread is limited to:
- state transitions
- cached/stale rendering
- queueing operations
- consuming operation results
- lightweight toasts/tooltips/modals

The UI must not call synchronous `block_on(...)` wrappers for user-facing SFTP work.

### 2. Generation IDs and cancellation

Each browser session and file operation gets a request token/generation ID.

- Directory refreshes use a session-scoped generation.
- `Open` / `Edit Locally` use operation-scoped tokens.
- When a result returns, the consumer verifies the token/generation before applying it.
- Closing a tab, switching target sessions, or navigating away invalidates stale results.
- Old results are dropped rather than overwriting newer state.

### 3. Stale-while-revalidate browser projection

When the quick browser becomes visible or a tab with SFTP binding becomes active:
- show cached rows if available, otherwise show a loading skeleton
- dispatch background refresh
- replace rows in place when the latest result returns

Follow mode refresh only triggers when the bound session, follow-mode state, or terminal cwd truly changes.

### 4. Transfer center as the action hub

Long-running SFTP work is represented as transfer/operation tasks. The transfer center becomes the place to:
- see live progress/state
- retry failed tasks
- open completed downloads
- reveal containing folders
- clear or remove finished rows
- inspect errors without blocking the rest of the window

## UI And Interaction Design

### Quick Browser States

The quick browser surfaces explicit local states:
- `loading`: no cached data yet, show skeleton
- `stale/syncing`: cached rows visible while background refresh runs
- `failed`: inline status row with retry action and message
- `disconnected`: inline reconnect guidance
- `empty`: empty state copy plus working blank-area context menu

The terminal area must remain responsive while any of these states change.

### Context Menus

Menus will be regrouped with Fluent UI System Icons, separators, and disabled reasons.

#### File / folder row menu
- Open
- Edit Locally (eligible text-like files only)
- Download To...
- Rename
- Delete
- Copy File Path / Copy Paths
- Refresh
- Permissions... (only if backed by real behavior)

#### Blank area menu
- New File (only if real, otherwise disabled)
- New Folder
- Upload Files...
- Upload Folder...
- Paste
- Refresh
- Select All
- Sort by Name
- Sort by Size
- Sort by Modified
- Copy Current Path

#### Multi-select menu
- Download To...
- Delete
- Copy Paths
- Refresh
- Permissions... (only if real)

Dangerous actions such as delete must require confirmation before dispatch.

### Transfer Center Layout

The row layout becomes compact and action-first:
- filename as the primary line
- direction plus destination/source path as the secondary line
- status badge plus progress/size summary
- inline action buttons on the trailing edge

Completed rows expose:
- Open File
- Open Containing Folder
- Remove

Failed rows expose:
- Retry
- Show Error
- Remove

Running rows expose:
- Cancel
- Pause only if the backend truly supports it

Toolbar actions include:
- Running
- Queued
- Paused
- Failed
- Completed
- Clear Completed

### Titlebar Transfer Summary

Remove the raw blue numeric badge. If a titlebar summary remains, it must be semantic text such as:
- `Transfers - 2 running`
- `Transfers - 1 failed`

If there is no meaningful summary, show only the transfer-center button without an unexplained badge.

## Data And State Model Changes

1. Extend browser/session state with stronger request-generation handling and explicit stale/loading metadata.
2. Introduce operation records for non-directory SFTP work (`download-open`, `edit-working-copy`, `rename`, `delete`, `mkdir`, etc.).
3. Track local working-copy metadata for `Edit Locally` so upload-back can report progress, failure, and conflicts asynchronously.
4. Extend transfer-center row projection with completed-item actions (`open_file`, `open_folder`, `remove`) and failure actions (`retry`, `show_error`).
5. Keep unsupported actions representable as disabled menu items with an attached reason string.

## File/Module Impact

### UI
- `ui/shell/right-panel.slint`
- `ui/shell/transfer-center.slint`
- `ui/shell/titlebar.slint`
- `ui/app-window.slint`

### Bootstrap / dispatch
- `src/app/bootstrap/sftp.rs`
- `src/app/bootstrap.rs`
- `src/app/bootstrap/shell_chrome.rs`
- `src/app/bootstrap/assets_keychain.rs`

### SFTP browser/session state
- `src/app/sftp/browser_state.rs`
- `src/app/sftp/browser_controller.rs`
- `src/app/sftp/mod.rs`
- likely a new helper module for operation dispatch or working-copy state

### View-model and action wiring
- `src/shell/view_model/sftp.rs`
- `src/shell/view_model/context_menu_dispatcher.rs`
- `src/shell/view_model/asset_modal_executor.rs`
- `src/shell/view_model/assets.rs`

### Runtime boundary
- `src/app/ssh/session_manager.rs`

### Tests
- `tests/sftp_context_menu_spec.rs`
- `tests/sftp_follow_cwd_spec.rs`
- `tests/bootstrap_smoke.rs`
- `tests/transfer_center_smoke.rs`
- `tests/top_status_bar_smoke.rs`
- additional focused async/open-edit specs as needed

## Verification

1. Opening the quick browser must not block terminal tab switching.
2. Activating a new terminal tab with SFTP binding must show cached rows or a skeleton immediately.
3. `Open` must not synchronously download on the UI path.
4. `Rename`, `Delete`, and `New Folder` must either perform real backend work or render disabled with explicit reason text.
5. Blank-area right click must work in both empty and ready states.
6. Transfer-center completed rows must expose `Open File`, `Open Containing Folder`, and `Remove`.
7. The meaningless titlebar numeric badge must be removed or replaced with semantic summary text.
8. Existing SFTP/transfer smoke suites must remain green after the new task/action wiring lands.
