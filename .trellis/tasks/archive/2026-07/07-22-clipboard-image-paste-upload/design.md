# Design

## Clipboard Payload

Introduce a small bootstrap-facing enum with `Text` and `Image` variants. The
Windows adapter probes bitmap or single-image-file sources before text and returns
only the selected source. A background worker validates dimensions, performs the
bounded decode, and encodes PNG bytes plus non-sensitive metadata. Other platforms
continue to return text through the current Slint clipboard API.

## Upload Flow

The paste callback captures the active session ID before starting asynchronous work.
Image upload uses a session-manager operation that asks the SFTP backend to:

1. canonicalize the remote home directory;
2. create the cache hierarchy with explicit permissions;
3. remove only stale UUID `.png` entries in that session directory;
4. open a new UUID `.png` with create/exclude/write and mode 0600;
5. write the encoded bytes and return the absolute remote path.

The completion callback verifies the original session still exists, quotes the path
for a POSIX shell, and invokes the existing `send_session_paste` path without a
newline. Text payloads continue through the existing warning/editor/send path.

## Error Model

Internal errors retain causes for diagnostics, but messages and logs contain only
format, dimensions, byte counts, session ID, and remote path metadata. Clipboard
bytes and decoded pixels are never logged.
