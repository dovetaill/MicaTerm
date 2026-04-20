# Async Latency Instrumentation

## Goal

Add precise async latency probes for the three UI paths that have been reported as causing visible hitching:

- `SFTP panel open`
- `SFTP session switch`
- `SSH open`

These probes are intentionally runtime-lightweight. They do not block the UI, and they exist to answer one question before the next optimization phase: is the remaining hitch in request dispatch, background completion, or UI-side list/render sync?

## Log Target

All new timing events emit through the tracing target `app.async_latency`.

Use debug logging when profiling these flows so the latency events are preserved in the app log alongside the existing async smoke coverage.

## SFTP Probe Points

### SFTP panel open

Flow id: `sftp-panel-open`

This flow is emitted for both the explicit `Open SFTP` action and the generic right-panel toggle when that toggle is opening the SFTP panel.

Stages:

- `ui-return`: emitted when the open-panel callback finishes its UI-side handoff work
- `request-queued`: emitted when the directory load request is queued to the async runtime
- `result-drained`: emitted when the UI thread first drains the async load result from the background queue
- `browser-state-applied`: emitted after the browser controller + `ShellViewModel` projection finish applying the result
- `right-panel-sync-start`: emitted immediately before `sync_right_panel_state(...)` starts pushing the updated SFTP state into Slint
- `right-panel-sync-finished`: emitted after the right-panel sync finishes
- `request-finished`: emitted when the async load result is applied back into the browser state

Diagnostic-only fallback stage:

- `sync-fallback-enter`: emitted only if the SFTP request falls back to the legacy synchronous path because no async runtime handle is available

Primary fields:

- `browser_session_id`
- `request_id`
- `path`
- `elapsed_ms`
- `elapsed_us`
- `row_count`

### SFTP session switch

Flow id: `sftp-panel-switch`

Stages:

- `ui-return`: emitted when the tab-switch callback finishes the UI-side handoff for the pending quick-browser retarget
- `request-queued`: emitted when the switch-triggered browser request is queued
- `result-drained`: emitted when the UI thread first drains the switched session's result
- `browser-state-applied`: emitted after the switched session state is merged into the quick-browser projection
- `right-panel-sync-start`: emitted immediately before the Slint right-panel sync starts
- `right-panel-sync-finished`: emitted after the right panel finishes syncing
- `request-finished`: emitted when the switched session's directory result is applied

This is the probe to read when the right panel is already visible and the user changes terminal/session context.

## SSH Probe Points

Flow id: `ssh-open`

Stages:

- `ui-return`: emitted after the UI-side open-session handoff completes and the workspace tab/projection has been updated
- `request-queued`: emitted when `SessionManager::open_session()` registers and spawns a new async connection attempt
- `session-connected`: emitted when the runtime reports that the session has actually reached the connected state

Primary fields:

- `session_id`
- `asset_id`
- `host`
- `user`
- `elapsed_ms`
- `elapsed_us`

## SSH Modal Probe Points

These flows sit one layer higher than `ssh-open`.

Use them when the user reports that clicking `Connect` or `Save and Connect` makes the whole app hitch before the async runtime even starts the real SSH connection attempt.

### SSH modal connect

Flow id: `ssh-modal-connect`

Stages:

- `session-profile-built`: emitted after the modal draft has been resolved into a runtime-ready profile on the UI thread
- `session-dispatched`: emitted after the modal callback has handed the request off through `attempt_open_session_with_profile(...)`
- `ui-return`: emitted after the modal callback finishes its synchronous UI sync work

### SSH modal save and connect

Flow id: `ssh-modal-save-connect`

Stages:

- `modal-confirmed`: emitted after `confirm_asset_modal()` mutates the in-memory asset tree
- `secrets-persisted`: emitted after `sync_saved_ssh_secrets(...)` finishes the synchronous credential-store writes
- `asset-catalog-saved`: emitted after `save_asset_catalog(...)` commits the saved asset state
- `session-dispatched`: emitted after the saved profile has been handed off for session open
- `ui-return`: emitted after the callback finishes the final sidebar/workspace sync pass

This is the probe to read when the remaining hitch feels like “click Save and Connect, then the UI freezes before the terminal even starts connecting”.

## How To Read The Data

- A high `ui-return` means the main-thread callback is still doing too much synchronous work.
- A high `request-queued` after a low `ui-return` means the open/switch callback returned quickly, but the 50ms projection/timer path still delayed the actual SFTP request dispatch.
- A low `ui-return` but high `result-drained` means the request is async and the delay is before the UI thread even sees the result (network/runtime/background queue wait).
- A high gap between `result-drained` and `browser-state-applied` points at controller/view-model projection work on the main thread.
- A high gap between `browser-state-applied` and `right-panel-sync-finished` points at the final Slint/VecModel/right-panel sync work on the main thread.
- A `sync-fallback-enter` event means the app actually entered the blocking SFTP fallback path, which is a real regression and should be treated as a main-thread blocker instead of an upstream renderer issue. Current builds should recover the `SessionManager` runtime before hitting that path, so this stage should now disappear unless the fallback regresses again.
- A low SFTP `request-finished` but visible hitch during huge directories is the signal to continue with list virtualization/windowing.
- A high SSH `session-connected` with a low `ui-return` means the UI is no longer blocking, and the remaining delay is network/runtime establishment instead of main-thread freeze.
- A high `ssh-modal-connect ui-return` means the modal callback is still spending too long building the runtime profile or syncing the workspace after dispatch.
- A high `ssh-modal-save-connect ui-return` with earlier spikes at `secrets-persisted` or `asset-catalog-saved` means the blocker is still synchronous save work on the UI thread.
- A low modal `ui-return` together with a later high `ssh-open session-connected` means the modal handoff is healthy and the remaining delay is in the real SSH runtime or network path.
