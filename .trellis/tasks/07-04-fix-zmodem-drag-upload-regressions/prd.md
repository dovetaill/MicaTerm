# Fix ZMODEM modal and terminal drag upload regressions

## Goal

Repair the shipped `rz` / `sz` workflow so that:

- remote `sz` can wait for a local download directory without failing early
- the ZMODEM modal buttons behave consistently for pending transfers
- dragging files from the OS onto the terminal reliably shows the hover affordance and uploads into the active terminal `pwd`
- an ordinary terminal `*` echo is not hidden while the ZMODEM sentry waits for a possible split protocol prefix
- automatic drag-triggered `rz` starts quietly and does not leave lrzsz's `rz waiting to receive.` banner in terminal output

## Requirements

- Keep a detected ZMODEM download session pending while the user is still choosing a local destination folder.
- Do not transition a pending download into `Failed` only because remote protocol bytes arrived before local confirmation.
- Preserve the existing progress-modal model for running, completed, failed, and cancelled ZMODEM transfers.
- Make terminal external-file hover and drop resolve against the real pointer location used during OS drag-and-drop, not only previously cached cursor movement.
- Preserve SFTP panel external drop behavior while fixing terminal external drop behavior.
- Keep terminal external drops uploading into the active terminal working directory tracked by the session manager.
- Preserve split `ZRINIT` / `ZRQINIT` detection while allowing a lone ordinary `*` to render immediately.
- Use lrzsz quiet mode for automatic interactive and dedicated-exec `rz` startup without adding shell probes or recovery commands.

## Acceptance Criteria

- [ ] Running `sz <file>` in an active terminal opens the ZMODEM modal and keeps it in the "Choose Folder" state until the user chooses a folder or cancels.
- [ ] After the user chooses a folder, the same `sz` transfer proceeds and updates the existing progress modal instead of immediately failing with `download target directory is not selected`.
- [ ] Cancelling a pending `sz` transfer leaves the modal in a cancelled/closable state instead of hanging the transfer.
- [ ] Dragging one or more local files over the terminal surface shows the terminal drop overlay while the pointer is over the terminal target.
- [ ] Dropping files onto the terminal triggers upload scheduling to the active remote working directory and clears the hover state afterward.
- [ ] Existing ZMODEM upload (`rz`) modal flow and SFTP external-drop flow continue to work.
- [ ] Typing `*` at an ordinary shell prompt visibly echoes the star before Enter; a later Bash expansion error still renders normally.
- [ ] Drag-triggered ZMODEM upload starts `rz -q`; lrzsz's non-protocol `rz waiting to receive.` banner is not emitted by the automatic path.

## Constraints

- Follow the current Rust + Slint architecture; do not replace the modal model or the session manager integration.
- Prefer a targeted runtime/windowing fix over broader refactors.
- Keep the solution compatible with the vendored `winit` version currently used by Slint in this repo.

## Notes

- Local root cause for `sz`: `src/app/ssh/runtime/zmodem.rs` advances the receiver before `target_dir` is set, so a queued `FileStarted` event throws `download target directory is not selected`.
- Local root cause for terminal drag hover/drop: `src/app/bootstrap/windowing.rs` decides the drop target from `CursorMoved` state, but external file drags do not reliably emit usable cursor-move updates in the current `winit` backend.
