# SFTP Right Panel Virtualization Design

## Goal

Make large-directory SFTP open, switch, and steady-state browsing stop feeding the right panel with the full directory row set. The panel must stay fully async and must not visibly stall the SSH terminal area or the host window while a big directory is opened or when the user scrolls through it.

## Problem Statement

The current right-panel path already avoids some unnecessary rebuilds:

- directory projection is cached per browser session,
- rendered row text/icon metadata is cached per browser session,
- selection-only changes patch only dirty rows,
- async latency probes now tell us whether the remaining hitch is UI-return, request-queued, or request-finished.

But the UI-facing model is still a full `VecModel<SftpPanelItem>` containing every row in the active directory. Slint `ListView` only instantiates visible delegates, but the app still pays for creating, storing, diffing, and synchronizing a large UI model.

## Chosen Approach

Use true windowed virtualization for the right-side SFTP list:

1. Keep the full directory render cache in Rust.
2. Track a viewport-driven row window per SFTP browser session.
3. Expose only the visible window plus overscan rows to Slint.
4. Replace `ListView` with a `ScrollView`-backed content host that keeps the full scroll height via top/bottom spacer blocks.
5. Continue to patch only dirty visible rows for selection changes.

This keeps the total number of UI rows bounded even when the underlying directory has thousands of entries.

## Why This Approach

### Option A — Keep `ListView`, but feed it a small model

Rejected.

`ListView`'s built-in layout assumes the model length represents the total list length. Feeding it only a small slice breaks the total scroll range and item-height math.

### Option B — Replace `ListView` with a `ScrollView` plus spacer-backed visible rows

Chosen.

This lets the UI preserve a correct full-height scrollbar while only rendering a small set of rows near the viewport.

### Option C — Keep incremental sync only

Rejected.

That still leaves the full directory row set inside the UI model, so large-directory open/switch can still hitch on model creation or reconciliation.

## Architecture

### 1. Full render cache stays in Rust

Per browser session we keep:

- the full ordered render rows,
- row index lookup by entry id,
- the latest viewport offset/height,
- the active visible window range,
- top spacer height,
- bottom spacer height,
- visible-row dirty indices,
- a full-resync flag for the visible window model.

The full render cache remains the truth source for selection and directory updates.

### 2. Visible window is derived from viewport metrics

The window is computed from:

- `viewport_y_px`,
- `visible_height_px`,
- fixed row height (`44px`),
- overscan rows above and below the viewport.

The derived window is:

- `window_start_row`,
- `window_end_row`,
- `top_spacer_height_px`,
- `bottom_spacer_height_px`,
- `total_content_height_px`.

For small directories the visible window naturally expands to the full row set.

### 3. UI model becomes a bounded slice

The Slint-facing `sftp-panel-items` model will hold only the rows inside the visible window.

The scroll height stays correct because the scroll content contains:

- a top spacer rectangle,
- the visible rows,
- a bottom spacer rectangle.

### 4. Scroll events become lightweight viewport updates

The right panel sends the latest `viewport-y` and `visible-height` back into Rust.

Rust recomputes the window only when needed:

- if the viewport moves far enough to cross the overscan threshold,
- if the panel height changes,
- if the active session changes,
- if the directory snapshot changes.

### 5. Selection patches stay incremental

Selection changes update the full cache first.

If the changed rows are currently visible, only the corresponding visible-window indices are marked dirty and patched into the `VecModel`. If the changed rows are outside the visible window, no UI row patch is needed.

## UI Contract Changes

The right panel needs these extra bindings:

- `sftp-panel-total-content-height`
- `sftp-panel-top-spacer-height`
- `sftp-panel-bottom-spacer-height`
- `sftp-panel-viewport-changed(length viewport_y, length visible_height)`

The panel also switches from `ListView` to `ScrollView` so the app can own total content height while keeping only a small visible row set in the repeater.

## Async Safety

This design does not move directory reads or SSH connection setup onto the UI thread.

Open/switch/SSH-new-open remain async. The main-thread work becomes smaller because it now synchronizes only a bounded row window instead of a full directory-sized UI model.

## Risks And Mitigations

### Risk: scroll thrash causes too many window rebuilds

Mitigation:

- use overscan rows,
- only trigger a visible-model rebuild when the computed window range actually changes,
- keep viewport-only property sync cheap.

### Risk: row interaction ids break after virtualization

Mitigation:

- keep `item.id` as the stable global SFTP entry id,
- keep parent-row id as `__sftp_parent__`,
- never route actions by visible-row index.

### Risk: session switching shows stale viewport state

Mitigation:

- store viewport/window state per browser session,
- mark the visible model for full resync when the active session changes.

## Tests

### View-model tests

- large directory produces a bounded visible window instead of exposing the full row set,
- viewport change recomputes window range and spacer heights,
- visible selection changes dirty only window-local indices,
- non-visible selection changes do not force visible-row patching.

### Source/UI contract tests

- right panel uses `ScrollView` instead of `ListView` for the SFTP row host,
- viewport callback and spacer properties exist in `.slint`,
- bootstrap binds the new viewport callback and syncs spacer heights.

## Success Criteria

- SFTP open/switch on a very large directory no longer feeds the full row set into the right-panel UI model.
- Row count in the UI stays bounded to the visible window plus overscan.
- Selection, activation, context menu, and parent-row navigation still work.
- Existing async latency probes remain valid and still show UI-return separately from background completion.
