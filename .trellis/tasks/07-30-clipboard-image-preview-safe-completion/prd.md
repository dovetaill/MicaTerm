# Clipboard image paste progress and explicit inline display

## Goal

Provide two deliberate clipboard-image workflows in an SSH workspace:

1. Standard Paste previews and uploads the image, reports progress and speed,
   then safely inserts the quoted remote path.
2. Display Clipboard Image renders the local image in the terminal grid and
   scrollback without uploading it or sending bytes to the remote process.

Both workflows must preserve existing text paste and remote Kitty/iTerm2/Sixel
behavior.

## Background And Confirmed Facts

- Windows `Win+Shift+S` clipboard acquisition works. A live paste produced a
  valid 330 x 210 PNG under `.cache/mica-term/clipboard` with 27,485 bytes,
  file mode 0600, and a containing directory with mode 0700.
- Entering the quoted PNG path by itself asks Bash to execute the PNG and
  correctly returns `Permission denied`; an uploaded path is not an inline-image
  display command.
- Static remote image rendering works independently. A valid Kitty direct-data
  sequence produced the expected blue block.
- Standard image Paste and remote image protocols are separate channels today:
  Paste encodes, uploads, and inserts a path, while Kitty/iTerm2/Sixel consume
  escape sequences from terminal output.
- `forward_active_workspace_paste` returns as soon as image upload is scheduled
  (`src/app/bootstrap/workspace_terminal.rs:1145-1210`).
- The original completion path wrote the quoted path without proving that input
  was unchanged (`src/app/bootstrap/workspace_terminal.rs:1300-1363`). This was
  reproduced when a delayed path altered a heredoc terminator and left Bash at
  its `>` continuation prompt.
- The current branch already implements bounded preview, ordered upload, input
  epoch protection, stale path recovery, and session/binding cleanup. Its prior
  automated baseline passed Linux tests, Clippy, Windows GNU/MSVC checks, and
  native/bitmap renderer feature combinations.
- The lower-right preview now exposes preparing, queued, uploading, success,
  stale, and error lifecycle states with upload byte progress/speed. Explicit
  local grid display is implemented through the dedicated shortcut and menu
  action without changing ordinary Paste.

## Requirements

- R1: Preserve ordinary text paste behavior and image-over-text clipboard
  precedence.
- R2: Preserve bounded Windows image acquisition, constrained decode/encode,
  session-bound SFTP upload, private remote permissions, stale-cache cleanup,
  and reconnect safety.
- R3: Keep the bounded lower-right preview independent from the terminal grid,
  PTY rows/columns, and remote protocol parser.
- R4: Keep preparing, queued, uploading, success, stale-input, and failure
  outcomes visible without writing status text into the PTY.
- R5: Never insert a completed upload path when terminal input changed after the
  request. Retain successful stale paths behind explicit Paste path and Copy path
  actions.
- R6: Preserve remote Kitty/iTerm2/Sixel parsing, placement, resource limits,
  generated replies, and native/bitmap rendering.
- R7: Keep up to eight image paste requests ordered and correctly paired with
  their request, session, binding, preview, progress, and completion state.
- R8: Report bounded upload bytes, percentage, and speed in the existing preview
  without creating a Transfer Center transfer or PTY output.
- R9: Keep standard Paste as upload-and-path. Provide inline display only through
  `Ctrl+Shift+I` and the terminal context-menu command `Display Clipboard Image`.
- R10: Place a local inline image at the current cursor in the real terminal grid,
  reserve its rows, advance the cursor below it, and retain it in scrollback.
- R11: Preserve aspect ratio and original pixel size when possible. Scale down,
  never up, to fit the grid width available from the cursor and 50 percent of the
  viewport height.
- R12: Reject local inline display in alternate-screen, mouse-grabbed, or
  application-cursor mode and show bounded client feedback.
- R13: A dedicated terminal-core local-image ingress must produce no SSH reply,
  PTY input, SFTP upload, shell command, or remote-helper invocation.
- R14: Apply the existing 25-million-pixel, 20-MiB encoded, 100-MiB decoded, and
  128-MiB per-session retained-resource limits to local inline images.
- R15: Revalidate the originating session and TUI state immediately before local
  placement so an asynchronous result cannot target a switched, closed, or newly
  interactive session.

## Product And Technical Decisions

- D1: Image `Ctrl+Shift+V` shows an immediate preview, uploads in the background,
  and auto-inserts the quoted remote path only while the captured input context
  remains current.
- D2: Text-only `Ctrl+Shift+V` retains existing paste behavior and opens no image
  preview.
- D3: The preview is a dismissible lower-right terminal-chrome overlay that does
  not resize the terminal or alter PTY geometry.
- D4: Safe success uses the existing 3.2-second acknowledgement lifetime; stale
  results remain actionable until handled or dismissed.
- D5: Stale `Paste path` inserts a POSIX-quoted path without a newline, while
  `Copy path` copies the raw path. Both revalidate the originating binding.
- D6: Multiple image Paste requests form an ordered queue; a later request never
  overwrites or cancels an earlier request.
- D7: Uploading cards show transferred bytes, total bytes, percentage, and speed.
  Very fast uploads may skip intermediate frames but retain final measurements.
- D8: `Ctrl+Shift+V` never implicitly renders an image into terminal history.
- D9: Inline clipboard images use terminal-grid placement, not a viewport overlay
  or separate viewer, and advance the cursor below the placement.
- D10: `Ctrl+Shift+I` and `Display Clipboard Image` invoke the same local action;
  existing Paste shortcuts and menu actions remain unchanged.
- D11: Inline sizing uses the current cursor, viewport, and cell metrics at apply
  time, rounds the scaled rectangle up to complete cell spans, and never upscales.
- D12: Alternate-screen, mouse-grabbed, and application-cursor modes are hard
  local-display boundaries; rejection sends no terminal bytes.
- D13: `TerminalCoreAdapter::apply_local_image` bypasses remote-output filtering,
  SSH reply handling, and the Kitty parser/registry.
- D14: Clipboard preparation may be asynchronous, but placement revalidates the
  captured session and active-session generation immediately before mutation;
  switching away and back still invalidates the old result.
- D15: Clipboard SFTP upload uses 64 KiB writes and request-keyed cumulative
  progress, throttled to approximately 10 UI updates per second with mandatory
  initial and final states. Speed is the monotonic average since transfer start.
- D16: The vendored WezTerm patch exposes only its existing cell-image placement
  primitive. Local cells use no Kitty image or placement ID, preventing remote
  protocol IDs and delete commands from owning local clipboard resources.
- D17: Inline failures never fall back to text paste, upload, shell execution, or
  a remote helper.
- D18: The local-inline controller retains at most one pending request. A newer
  inline request invalidates an older pending result without affecting Paste.

## Acceptance Criteria

- [x] A Windows screenshot Paste shows a bounded preview and uploads the same PNG
      to the originating session cache with 0700 directory and 0600 file modes.
- [x] An unchanged input context receives the existing POSIX-quoted path without
      an automatic newline.
- [x] Changed or submitted input receives no delayed path; the completed card
      exposes deliberate Paste path and Copy path recovery.
- [x] Preparing, queued, uploading, success, stale, and error states remain visible
      without entering the terminal byte stream.
- [x] Uploading cards show monotonic byte progress and speed, and successful cards
      retain final measurements for their compact feedback lifetime.
- [x] `Ctrl+Shift+I` and `Display Clipboard Image` render the same clipboard image
      at the current cursor without upload or PTY input.
- [x] Local placement reserves rows, advances the cursor below the image, follows
      terminal scrolling, and is consumed by both native and bitmap renderers.
- [x] Local sizing preserves aspect ratio, does not upscale, and fits the current
      horizontal cell capacity and half-viewport height.
- [x] Interactive TUI state rejects local display with client feedback and leaves
      the terminal grid and PTY stream unchanged.
- [x] Local-image ingestion uses no Kitty ID, produces zero SSH reply bytes, and
      releases resources through scrollback eviction, clear, or session teardown.
- [x] Session switching and a newer inline request invalidate older prepared
      results, including a switch-away-and-back sequence.
- [x] The pre-extension baseline covers text paste, copied-image-file paste,
      session replacement, bounded preview cleanup, and remote Kitty/iTerm2/Sixel.
- [x] The completed extension passes focused controller/core/bootstrap/UI tests,
      full Linux regressions, formatting, Clippy, diff checks, Windows GNU/MSVC
      checks, and native/bitmap renderer feature combinations.
- [ ] Windows manual acceptance covers upload progress, explicit local display,
      scrolling, TUI rejection, stale path recovery, session switching, and the
      unchanged remote Kitty blue-block fixture.

## Out Of Scope

- Installing or invoking a remote image viewer or helper.
- Automatically attaching an image to arbitrary remote CLI/TUI applications.
- Making ordinary Paste both upload and display an image.
- A viewport-only image overlay or separate image viewer.
- Animated local clipboard images or external Kitty file/shared-memory media.
- Changing `TERM=xterm-256color` or requiring accepted SSH environment variables.
- Upload cancellation, pause/resume, or ETA estimation.

## Implementation Status

Track A upload progress/speed and Track B explicit local terminal-grid display are
implemented. Focused suites, the complete Linux regression gate, formatting,
diff validation, non-strict Clippy with zero new warnings, all four renderer
feature combinations, and Windows GNU/MSVC cross-checks pass. Strict
`-D warnings` remains blocked by the repository's pre-existing warnings. Final
Windows manual acceptance remains pending. The implementation work is committed
as `9c074fc` and merged into `master` as `1437d21`; integration details are
retained in `validation-and-handoff.md`. Post-merge all-target verification passes
after reconciling the modern README with the repository's runtime-documentation
contracts.
