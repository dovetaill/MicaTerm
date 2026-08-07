# Repair Windows image clipboard and terminal environment fallback

## Goal

Restore reliable Windows image clipboard paste into an active SSH terminal while
keeping the existing image resource limits and remote-upload security
properties. Define `TERM_PROGRAM=mica-term` as a best-effort compatibility
signal without making graphics rendering depend on server `AcceptEnv` policy.

## Confirmed Facts

- The live Windows log reaches `forward_active_workspace_paste` and returns
  `Ignored` at `src/app/bootstrap/workspace_terminal.rs:1159-1167`; no SFTP
  upload is scheduled.
- `system_clipboard_image_source` recognizes only `CF_BITMAP` and
  `CF_HDROP` at `src/app/clipboard.rs:135-168`. It returns `None` before it
  opens the clipboard when neither is advertised.
- Windows documents synthesized conversion from `CF_DIB` and `CF_DIBV5` to
  `CF_BITMAP` on `GetClipboardData(CF_BITMAP)`. The early availability gate
  prevents the current code from requesting that conversion.
- Clipboard producers can also publish a registered `PNG` payload. A raw PNG
  fallback must enter the same bounded decoder and PNG re-encoder as bitmap
  and file sources.
- Existing bounds are 25,000,000 pixels, 20 MiB encoded, and 100 MiB decoded;
  the fix must not allocate or copy unbounded clipboard data before enforcing
  them.
- The user’s previous Kitty/iTerm2 commands had an empty `$img`, so they did
  not exercise terminal image parsing or presentation.
- `TERM_PROGRAM` is requested through SSH environment negotiation at
  `src/app/ssh/runtime/terminal.rs:60-90`, which a server may reject. The
  shell bootstrap builder containing the same export is currently only
  referenced by unit tests (`src/app/ssh/shell_integration.rs:175`).

## Requirements

- R1: On Windows, recognize clipboard images offered as `CF_DIB`, `CF_DIBV5`,
  synthesized `CF_BITMAP`, a registered `PNG` payload, or exactly one
  supported `CF_HDROP` file.
- R2: Preserve the existing precedence of image over text and the normal text
  paste behavior when no image is available.
- R3: Keep all image decoding behind the existing pixel, decoded-memory, and
  encoded-size limits. Clipboard data is untrusted.
- R4: Keep image upload session-bound; a reconnect or closed session must not
  paste an old upload into a new terminal.
- R5: Direct Kitty, iTerm2, and Sixel protocol rendering must remain
  independent of whether `TERM_PROGRAM` reaches the remote shell.
- R6: Add deterministic unit coverage for format selection and encoded PNG
  fallback, plus Windows-target compilation coverage.
- R7: Decide and document the fallback behavior when an SSH server rejects
  `TERM_PROGRAM` environment negotiation.

## Acceptance Criteria

- [x] A `CF_DIB` or `CF_DIBV5` clipboard candidate causes a bounded
      `CF_BITMAP` retrieval attempt and produces an uploadable image when
      Windows supplies the synthesized bitmap.
- [x] A registered `PNG` clipboard payload is copied with a pre-copy size cap,
      decoded through the existing constrained image path, and uploaded as the
      standard remote PNG.
- [x] A single copied image file still uploads; multiple files do not become a
      single image upload; ordinary text paste is unchanged.
- [x] Invalid, oversized, malformed, or unavailable clipboard image data
      produces an actionable error rather than an ignored paste or a crash.
- [x] `cargo test` coverage proves source selection, DIB candidate routing,
      PNG size bounds, and no regression in SFTP binding behavior.
- [x] `cargo check --target x86_64-pc-windows-gnu --all-targets` passes.
- [ ] Manual validation distinguishes clipboard upload from protocol rendering:
      a pasted screenshot inserts a remote path, while a valid Kitty/iTerm2/
      Sixel sequence renders without SFTP.

## Out of Scope

- Animated terminal images, external Kitty image media, and changes to remote
  server configuration.
- Changing the terminal `TERM` value from `xterm-256color`.

## Decision

`TERM_PROGRAM` remains best-effort SSH environment negotiation. Mica Term must
not inject a hidden or possibly visible command into an arbitrary remote shell
after connection, because that would alter the user's session and cannot be
made shell-neutral. Direct image protocols remain available regardless of this
variable; an explicit opt-in remote-shell bootstrap is future work.

## Validation Status

Automated verification passed on Linux plus Windows GNU and MSVC cross-target
checks. The remaining acceptance item requires a rebuilt Windows package and a
live `Win+Shift+S` screenshot paste into the user's SSH session; that cannot be
exercised from this Linux build host.
