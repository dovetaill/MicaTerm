# Implementation Plan

1. ZMODEM receiver guard
   - Add an explicit pending-start guard to `ReceiverTransfer`.
   - Ensure cancellation still progresses to an abort/cancelled state.
   - Add a regression test for queued remote file-offer bytes before directory selection.

2. External drag target resolution
   - Normalize pointer coordinates used for drop-target hit testing.
   - Add a Windows-specific pointer fallback for `HoveredFile` / `DroppedFile`.
   - Reuse the same resolved pointer for SFTP and terminal drop target selection.

3. ZMODEM output-boundary regressions
   - Render a lone `*` held by split-prefix detection and erase it only after a confirmed ZMODEM prefix.
   - Use `rz -q` for automatic exec and interactive upload startup, retaining quiet-command echo stripping.

4. Verification
   - Run `cargo test -q` for the focused regression coverage if practical.
   - Run `cargo check -q`.
   - Verify a local `sz -q` / `rz -q` transfer exits cleanly with no stderr banner.
   - If full test coverage is too slow or unrelated tests fail, record the exact limit and the commands run.

## Review Gates

- Do not change unrelated transfer-center or modal styling while fixing runtime behavior.
- Keep the fix local to ZMODEM runtime / windowing glue unless a compile-driven dependency requires a small follow-up edit.

## Rollback Points

- If the receiver guard causes handshake stalls after folder selection, revert to the previous receiver polling path and move buffering outward into the controller.
- If the Windows pointer fallback proves unreliable, isolate it behind a helper and preserve the cached-pointer path for non-Windows platforms.
