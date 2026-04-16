# SFTP Browser And Transfer Center Bugfix Design

## Context

The current quick SFTP browser regressed in several core interactions after the async-browser and transfer-center work:
- left click on the file list no longer reliably selects rows or activates directories
- the context-menu refresh action does not perform a real remote refresh
- upload-folder does not preserve the selected folder root and drops empty directories
- `new-file` is exposed in the SFTP blank-area menu but has no real backend action
- drag-and-drop upload needs regression coverage
- the transfer center is harder to dismiss than the rest of the shell overlays
- the quick browser still shows an intrusive `Refreshing remote directory` status string during normal refreshes

The user confirmed two UX decisions:
1. `New File` should prompt for a name and create an empty file in the current remote directory.
2. When the transfer center is open, clicking outside the panel should close it.

## Verified Findings

1. `ui/shell/right-panel.slint` renders a full-size `list-host-blank-area-touch` on top of the ready-state list region. This is the most likely reason the list loses primary left-click interaction.
2. `ui/shell/right-panel.slint` hardcodes `Refreshing remote directory` as the loading headline.
3. `src/shell/view_model/context_menu_dispatcher.rs` handles `refresh-sftp` by toggling local loading state only; it does not dispatch a real remote refresh.
4. `src/shell/view_model/context_menu_dispatcher.rs` exposes `new-file`, but the SFTP action dispatcher does not implement it.
5. `src/app/sftp/local_ops.rs::scan_local_sources(...)` only emits file entries, so selecting a folder loses the folder root and empty directories.
6. `src/app/sftp/queue.rs` and `src/app/sftp/session_binding.rs` support download-directory tasks but do not yet model upload-directory tasks.
7. `src/app/bootstrap.rs` toggles the transfer center from the titlebar but there is no panel-external dismiss layer in `ui/app-window.slint`.
8. There is an existing baseline failure in `tests/sftp_right_panel_render_spec.rs` because the current compact-row source no longer uses the expected `meta-text := Text` identifier. This should be normalized while fixing the quick browser source contract.

## Goals

1. Restore row selection and directory activation in the SFTP quick browser.
2. Make both toolbar refresh and context-menu refresh perform the same real remote refresh path.
3. Replace the noisy refresh copy with quieter stale-while-refresh behavior.
4. Implement `New File` as a real remote empty-file creation flow.
5. Make folder uploads preserve the picked folder name and create empty remote directories when needed.
6. Keep drag-and-drop upload working and covered by regression tests.
7. Allow transfer center dismissal by clicking anywhere outside the panel.
8. Fix the compact-row render contract drift so related UI specs go green again.

## Non-Goals

1. This round does not implement remote copy/cut/paste.
2. This round does not add a full remote permissions editor.
3. This round does not redesign the transfer center visual style beyond dismissal behavior.

## Chosen Approach

### 1. Restore list hit testing instead of rewriting the browser

Keep the compact two-line row layout, but change the blank-area context-menu layer so it only handles actual empty space and does not sit on top of interactive rows. Row click and double-click remain owned by each list row `TouchArea`.

### 2. Unify refresh semantics

Toolbar refresh, context-menu refresh, and post-mutation refreshes all use the existing `SftpBrowserController::refresh(...)` path. When cached rows already exist, the browser keeps rendering them during refresh instead of surfacing the `Refreshing remote directory` status copy.

### 3. Add a real async `New File` action

Introduce a new pending SFTP context action for creating an empty file in the current directory. Bootstrap will execute it asynchronously through `SessionManager::sftp_upload_file_async(...)` with an empty byte buffer, then request a real directory refresh on success.

### 4. Extend upload queue semantics for directories

Add an upload-directory task model that can represent directory placeholders in the transfer queue. `scan_local_sources(...)` will emit both file entries and explicit directory entries for the picked folder root plus any nested empty directories. Queue execution will create remote directories for those tasks and continue to use the existing file-upload path for files.

### 5. Make transfer center dismiss like a shell overlay

Add a transparent dismiss layer behind the transfer center but above the rest of the workspace content. Clicking that layer closes the panel. Pointer events inside the panel continue to work normally.

## File Impact

### UI
- `ui/shell/right-panel.slint`
- `ui/app-window.slint`
- `ui/shell/transfer-center.slint` (only if small source contract changes are needed)

### Bootstrap and dispatch
- `src/app/bootstrap.rs`
- `src/app/bootstrap/sftp.rs`

### SFTP domain
- `src/app/sftp/local_ops.rs`
- `src/app/sftp/queue.rs`
- `src/app/sftp/session_binding.rs`
- `src/app/sftp/mod.rs` if re-exports change

### View model and modal wiring
- `src/shell/view_model/context_menu_dispatcher.rs`
- `src/shell/view_model/asset_modal_executor.rs`
- `src/shell/view_model/assets.rs`
- `src/shell/view_model/sftp.rs`

### Tests
- `tests/sftp_right_panel_render_spec.rs`
- `tests/sftp_context_menu_spec.rs`
- `tests/sftp_transfer_flow_spec.rs`
- `tests/bootstrap_smoke.rs`
- `tests/transfer_center_smoke.rs`
- `tests/top_status_bar_smoke.rs` if the dismiss path is surfaced through the titlebar toggle contract

## Verification

1. Left click selects a row again.
2. Double click on a directory requests navigation.
3. Context-menu refresh triggers the same real refresh path as the toolbar button.
4. The quick browser no longer shows `Refreshing remote directory` during ordinary cached refreshes.
5. `New File` creates an empty remote file and refreshes the current directory.
6. Uploading a folder preserves the selected folder root and creates empty remote directories.
7. Drag-and-drop upload still queues transfers and clears the overlay.
8. Clicking outside the transfer center closes it.
9. `tests/sftp_right_panel_render_spec.rs` no longer fails on the compact-row naming drift.
