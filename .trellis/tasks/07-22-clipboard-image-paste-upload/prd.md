# Windows 剪贴板图片上传并插入路径

## Goal

On Windows, allow the terminal Paste command to accept a clipboard image, upload it
to the originating SSH session, and paste the remote absolute path without changing
normal text paste behavior.

## Requirements

- `Ctrl+Shift+V` and the terminal context-menu Paste command must use one
  `ClipboardPayload` decision path.
- If the clipboard contains a bitmap or exactly one image file, the image takes
  precedence. Otherwise, use the existing text path unchanged.
- Decode at most one image, reject more than 25 million pixels, encode PNG, and
  reject encoded output larger than 20 MiB.
- Upload with SFTP rather than ZMODEM.
- Resolve the canonical remote home and use
  `<home>/.cache/mica-term/clipboard/<session-id>/`.
- Create directories with mode 0700 and files with mode 0600. Use UUID filenames
  and exclusive create; never overwrite an existing path.
- Clean only matching clipboard-cache files older than seven days.
- On success, POSIX-shell-quote the remote absolute path and pass it to the existing
  `send_session_paste` flow for the session that initiated the operation.
- Do not append a newline, execute the path, wrap it in Markdown, or log clipboard
  contents.
- Show a user-visible error on decode, size, upload, or session failure and insert
  no path.
- Non-Windows text paste behavior must remain unchanged.

## Acceptance Criteria

- [x] A Windows bitmap is encoded as PNG, uploaded exclusively, and pasted as one
      shell-safe remote path.
- [x] A clipboard containing exactly one supported image file follows the same
      upload path.
- [x] Image payloads take precedence over simultaneous text payloads.
- [x] Text-only clipboard contents follow the pre-existing warning/editor/paste
      behavior byte-for-byte after newline normalization.
- [x] Oversized, invalid, or failed images never produce terminal input.
- [x] Cleanup cannot delete files outside the designated session cache or files
      newer than seven days.
- [x] Unit tests cover payload precedence, limits, path quoting, cache naming, and
      cleanup filtering; Linux builds and existing paste tests stay green except
      for documented baseline failures.

## Notes

- Windows is the first image-capable clipboard implementation. The public payload
  enum remains platform-neutral so other platforms can be added later.
