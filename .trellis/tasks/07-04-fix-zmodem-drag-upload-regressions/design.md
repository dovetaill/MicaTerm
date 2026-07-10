# Design

## Scope

This task fixes two regressions in the new ZMODEM / terminal drop-upload path:

1. `sz` download sessions fail before the user can choose a destination.
2. OS file drag over the terminal never activates the terminal drop target reliably.

## Root Cause Summary

### 1. ZMODEM download fails before user choice

`ZmodemController::intercept_remote_bytes()` creates a `ReceiverTransfer` as soon as it sees the ZRQINIT prefix and immediately feeds the remaining wire bytes into the protocol. The next pump cycle calls `advance()`, which polls the receiver. If the queued protocol already includes `Event::FileStarted`, `ReceiverTransfer::handle_event()` requires `target_dir` and throws `download target directory is not selected`.

This is a timing bug at the UI/runtime boundary: protocol progress is allowed before the UI has supplied the required local directory.

### 2. Terminal drag target never becomes active

`bind_windows_window_state_tracking()` uses `last_pointer_position` from `WindowEvent::CursorMoved` to decide whether an external file hover/drop is over the SFTP panel or the terminal surface. In the vendored `winit` backend used here, external file drag events are `HoveredFile` / `DroppedFile` without coordinates, and upstream discussions show that cursor-move events are not reliably delivered during file drag. On Windows, the backend drop handler also ignores the COM drag point and only emits file-path events.

This is a platform-event boundary bug: the current app logic depends on pointer data that the active backend does not guarantee during external drag.

### 3. A normal `*` echo is held as a possible ZMODEM prefix

`ZmodemController::intercept_remote_bytes()` retains the longest suffix that can begin `**\x18B00` or `**\x18B01`. A remote shell echo containing only `*` therefore remains buffered until later output. Bash still receives the locally sent star and can expand it, so users see a command-not-found error for an expanded filename without ever seeing the typed star.

The same automatic interactive `rz` fallback previously ran ` rz\r`. GNU lrzsz writes `rz waiting to receive.` to stderr before its ZRINIT frames, which can appear in the merged PTY output and make a completed upload look stuck.

## Chosen Fixes

### A. Gate receiver progress until download directory exists

- Keep feeding detected protocol bytes into the receiver session so the transport stays synchronized.
- Add an explicit receiver-side guard: when the session is still awaiting a download directory, `advance()` returns idle instead of polling the protocol.
- Allow cancellation to bypass that guard so abort frames can still be emitted and the session can settle into a cancelled state.

Why this design:

- It is the smallest fix that addresses the actual failure boundary.
- It matches mature terminal behavior: the transfer window/prompt appears first, and file acceptance proceeds only after the user confirms a destination.
- It avoids buffering a second copy of protocol bytes outside `zmodem2`.

### B. Resolve external drag pointer from the real window position

- Normalize cached `CursorMoved` coordinates into the same coordinate space used by Slint layout hit-testing.
- For Windows external drag events, query the actual pointer location with `GetCursorPos` and convert it into client coordinates with `ScreenToClient`.
- Use that resolved pointer for both hover-target selection and final drop-target selection, falling back to the cached pointer when the platform query fails or on non-Windows targets.

Why this design:

- It directly addresses the missing-coordinate limitation in the current backend.
- It avoids patching the vendored `winit` event model for this app-only need.
- It preserves the same target-selection rules for SFTP and terminal drop zones.

### C. Render an ambiguous star and start automatic `rz` quietly

- When the sentry retains a lone leading `*`, pass it through to the terminal immediately and record that it is visible.
- If later bytes confirm a ZMODEM prefix, return `\x08 \x08` before the protocol is consumed to remove the provisional star from the local terminal frame.
- Use `rz -q` for both the dedicated exec channel and the interactive fallback. Preserve legacy echo stripping as well as the quiet-command echo form.

Why this design:

- Shell input remains visibly echoed without weakening fragmented ZMODEM prefix detection.
- `rz -q` is the lrzsz-supported way to suppress its non-protocol progress/banner output; a local `sz`/`rz -q` round trip confirms it still completes the wire handshake.

## Validation

- Add a regression test proving a `ReceiverTransfer` with queued remote file-offer bytes stays pending instead of failing before `start_download()`.
- Run targeted Rust verification for the touched code paths.
- Manual behavior expected after fix:
  - `sz` modal remains actionable until folder choice.
  - terminal drag hover visibly activates over the terminal surface.
  - drop schedules upload to current terminal `pwd`.
  - typing `*` visibly echoes before Enter, while a split ZMODEM header still opens the transfer flow without leaving a stray star.
  - automatic `rz` does not print `rz waiting to receive.`.
