# Design: Windows Clipboard Image Formats

## Boundaries

```
Windows clipboard formats
  -> bounded local byte acquisition
  -> existing constrained image decoder/re-encoder
  -> session-bound SFTP upload
  -> POSIX-quoted path pasted into the originating SSH session
```

`src/app/clipboard.rs` owns format recognition and byte acquisition.
`workspace_terminal.rs` continues to own selection precedence, background
encoding, upload scheduling, and error presentation. SFTP and terminal image
protocol code are not changed by this repair.

## Clipboard Format Contract

The Windows reader will choose one source in this order:

1. Registered `PNG`: preserves source pixels when the producer publishes a
   PNG payload. Read the `HGLOBAL` only after verifying its size is at most the
   existing 20 MiB encoded limit.
2. `CF_BITMAP`, `CF_DIB`, or `CF_DIBV5`: when any is advertised, request
   `CF_BITMAP` through `GetClipboardData`. Windows synthesizes it from DIB and
   DIBV5 when necessary. Existing bitmap metadata preflight remains before the
   `clipboard-win` pixel copy.
3. Exactly one supported `CF_HDROP` image file.

No candidate falls through to text after it is selected but found malformed or
too large. That condition is an image-upload error, shown in the existing
Transfer Center feedback path.

## Limits and Safety

- The registered PNG path checks `GlobalSize` before `GlobalLock` and copying.
- The existing 25M-pixel, 100 MiB decoded, and 20 MiB encoded limits remain
  the single shared policy in `image_policy.rs`.
- DIB/DIBV5 use the existing bounded `CF_BITMAP` metadata validation before
  `clipboard-win` allocates its bitmap byte vector.
- Clipboard handles remain owned by Windows; the implementation only locks,
  copies, unlocks, and never frees them.

## SSH Environment Decision

SSH `env` requests remain the only automatic way to set `TERM_PROGRAM`.
Servers may reject them. The client will not type a fallback command into the
remote shell because that can be visible, shell-specific, or mutate an
unexpected session. Kitty, iTerm2, and Sixel parsing already runs on received
bytes without consulting `TERM_PROGRAM`.

## Verification

- Pure unit tests cover source priority, DIB/DIBV5 candidate routing, PNG
  source encoding, and encoded-size rejection.
- Existing SFTP binding tests remain the regression proof that upload results
  cannot target a replacement session.
- Windows GNU all-target checks compile the Win32 clipboard path.
- Manual Windows validation covers screenshot (`CF_DIBV5`), copied PNG file
  (`CF_HDROP`), text fallback, and a direct Kitty protocol sequence that does
  not depend on clipboard upload.
