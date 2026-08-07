# Clipboard Image Paste Progress and Explicit Inline Display Design

## Context

Windows clipboard image acquisition, bounded preview, session-bound SFTP upload,
ordered completion, and input-epoch protection are already implemented on the
current branch. Live validation showed that a screenshot reached the remote cache
as a valid private PNG, while a direct Kitty payload rendered independently in the
terminal. Typing the quoted PNG path by itself correctly asked Bash to execute the
non-executable file; it did not request terminal rendering.

This extension adds byte progress and speed to the upload preview and introduces a
separate explicit action for placing the local clipboard image in the terminal
grid. Standard Paste remains upload-and-path. Remote Kitty, iTerm2, and Sixel
remain an independent server-output channel.

## Boundaries

```text
Windows clipboard image
  -> existing bounded image acquisition
  -> bounded local PNG + thumbnail preparation
  -> Standard Paste
       -> ordered session-bound SFTP upload with progress
       -> completion guard (session + binding + input epoch)
            -> safe automatic path insertion
            -> stale explicit path actions
  -> Display Clipboard Image
       -> session/TUI revalidation
       -> dedicated local terminal-core image ingress
       -> grid placement and scrollback, with zero PTY bytes

Remote Kitty / iTerm2 / Sixel bytes
  -> existing terminal parser and resource store
  -> existing grid-relative image projection
```

The upload preview remains application chrome. It never enters
`TerminalSurfaceState`, the terminal grid, escape parsing, scrollback, PTY size
calculation, or native terminal image caches. Only the explicit local-display
action creates terminal image resources and placements, and it does so through a
dedicated terminal-core API rather than the remote-output parser.

## Ownership

A bootstrap-scoped `ClipboardImagePasteController` owns ephemeral state for the
upload-and-path branch. It remains separate from `ShellViewModel` because it
contains request lifecycle data, bounded image buffers, captured SFTP handles, and
monotonic input epochs that are neither persisted nor part of the general shell
model.

The controller owns:

- one monotonically increasing input epoch per terminal session;
- an ordered queue of at most eight clipboard image requests;
- one request UUID per paste, used by every worker result and UI callback;
- the captured session UUID, SFTP binding UUID, and input epoch for each request;
- the captured SFTP runtime until the request begins upload;
- the prepared PNG only while it is waiting for the ordered upload slot;
- a thumbnail, remote path, status, message, dismissal flag, and optional success
  expiry for each request;
- the identity of the single request currently uploading.

The controller is mutated only on the Slint event thread. Blocking image work and
asynchronous SFTP work return typed messages through the existing bootstrap result
channel; workers never mutate UI or controller state directly.

A separate bootstrap-scoped `ClipboardInlineImageController` owns at most one
pending local-display request. It retains only a request UUID, originating session
UUID, captured active-session generation, and preparation state; it never owns an
SFTP runtime or remote path. `ShellViewModel` advances the generation in its central
workspace-tab normalization path whenever the active session changes. A newer
inline request invalidates an older pending request, and every active-session change
invalidates the captured generation even if the user switches away and back before
preparation completes.

## Request Model

Each request has these stable identity fields:

```text
request_id: UUID
session_id: UUID
binding_id: UUID
captured_input_epoch: u64
created_order: position in the controller queue
```

Its lifecycle state is one of:

| State | Meaning | Retained data |
| --- | --- | --- |
| `preparing` | The card exists and bounded local decode/encode is running. | Captured context and SFTP runtime |
| `queued` | PNG and thumbnail are ready; an earlier request owns the upload slot. | PNG, thumbnail, captured context/runtime |
| `uploading` | The PNG is being uploaded through the captured SFTP runtime. | Thumbnail, captured context, byte progress, and timing |
| `success` | The quoted path was inserted automatically or explicitly. | Compact acknowledgement and final transfer measurements until expiry |
| `stale` | Upload succeeded, but later terminal input changed the context. | Thumbnail or compact metadata plus remote path |
| `error` | Preparation, upload, clipboard copy, session, or binding validation failed. | Bounded diagnostic text and any still-useful path |

`pending` is the product-level grouping of `preparing`, `queued`, and `uploading`.

## Input Epoch Contract

An image request captures the originating session's current input epoch but does
not advance it because the request itself sends no terminal bytes.

The epoch advances for every client action that actually attempts to send bytes to
the remote terminal:

- non-empty text input;
- a recognized terminal key event;
- an accepted ordinary or confirmed text paste;
- mouse or wheel input only when it is forwarded to the remote application;
- automatic insertion of an uploaded path;
- explicit `Paste path` recovery.

Local scrollback, selection, search, tab switching, resize, and other client-only
actions do not advance the epoch. Events that cannot be translated to terminal
input do not advance it. Failed terminal writes may conservatively advance it,
because treating an uncertain write as unchanged would make later automatic
insertion unsafe.

Input callbacks and completion draining run on the same Slint event thread. The
epoch comparison and the call to the session manager therefore form one serialized
UI action; another local input callback cannot interleave between them.

The guard protects against deterministic client-originated input changes. It does
not claim to infer arbitrary changes made by remote background processes.

## Preparation and Ordered Upload

When `Ctrl+Shift+V` or the terminal Paste action selects an image:

1. Resolve the active terminal session and atomically capture its SFTP binding and
   runtime.
2. Reject the request if eight image requests are already retained. Clipboard type
   inspection is still required so a full image queue does not block ordinary text
   paste, but the ninth image is not decoded, retained, or uploaded.
3. Insert a `preparing` item immediately so the user sees acknowledgement in the
   same UI turn.
4. Run the existing bounded decode/re-encode on the blocking pool. During the same
   decode, derive an aspect-preserving RGBA thumbnail no larger than 320 x 180.
5. Return the PNG, original dimensions, encoded byte count, and thumbnail in a
   request-ID-keyed preparation message.
6. Store out-of-order preparation results, but start an upload only when every
   earlier non-terminal request has reached an uploadable or terminal state.
7. Move the PNG and captured SFTP runtime into one upload task. Only one clipboard
   image upload runs at a time; the controller starts the next request after the
   current request reaches success, stale, or error.

Preparation may run with a small fixed concurrency of two. The eight-item queue,
existing 25-million-pixel/100-MiB decoded/20-MiB encoded limits, and 320 x 180
thumbnail cap bound retained memory. Full decoded pixels are released after PNG and
thumbnail generation; prepared PNG bytes are released when upload starts.

## Upload Progress And Speed

Clipboard PNG upload uses a progress-capable SFTP operation dedicated to this
workflow. It preserves exclusive create, 0600 file mode, flush/shutdown, and
best-effort deletion of an incomplete remote file. The payload is written in 64 KiB
chunks. A chunk counts as transferred only after its asynchronous write succeeds.

The upload task sends typed progress messages through the existing bootstrap result
channel:

```text
Progress {
  request_id,
  bytes_transferred,
  bytes_total,
  elapsed
}
```

Messages are cumulative, monotonic, and keyed by request UUID. The controller
ignores progress for unknown, completed, or non-active requests. Emission is
throttled to approximately 10 UI updates per second, but the zero/initial state and
the final byte count are mandatory. Very small images may transition directly from
zero to complete.

Displayed speed is the average acknowledged bytes per second over monotonic elapsed
time. The completed request retains its final byte count and speed for the existing
3.2-second compact success lifetime. This flow creates no Transfer Center transfer,
ETA, cancellation state, PTY output, or persisted history.

## Completion Decision

For a successful upload, completion follows this decision table:

| Condition | Result |
| --- | --- |
| Session exists, binding is current, and input epoch is unchanged | Send the POSIX-quoted path without a newline, advance the epoch, enter `success`. |
| Session and binding are current, but input epoch changed | Send no terminal bytes, retain the raw remote path, enter `stale`. |
| Session closed or binding changed | Send no terminal bytes, release preview resources, report a connection error; never target a replacement runtime. |
| Session-manager send fails | Conservatively advance the epoch, enter `error`, and log only bounded metadata. |

The existing `send_session_paste_if_sftp_binding_current` operation remains the
atomic binding-and-send boundary. The epoch check happens immediately before that
operation on the event thread.

An automatic insertion advances the session epoch. Consequently, if several image
requests captured the same epoch, only the oldest successful request can insert
automatically. Later completions become `stale` and require explicit action. This
prevents adjacent quoted strings such as `'path-1''path-2'` and avoids making an
implicit separator decision for the user.

If an older request fails before inserting any bytes, the next request may still
insert automatically when its captured epoch remains current.

## Recovery Actions

`Paste path` is available only for a `stale` request whose originating session is
the active terminal and whose captured binding is still current. It POSIX-quotes
the retained path, sends it at the current cursor without a newline, advances the
input epoch, and changes the item to `success`. Other pending requests are then
evaluated against the advanced epoch normally.

`Copy path` copies the raw absolute remote path to the default system clipboard. It
does not alter terminal input or the input epoch. Clipboard errors remain on the
item and are logged without image bytes.

When a pending card is dismissed, its thumbnail is released and the card is hidden,
but lightweight request identity and completion metadata remain until the worker
finishes. A safely insertable completion may still follow the standard automatic
path behavior. A stale completion reappears as a compact actionable item so the
remote path cannot be lost. Dismissing a terminal `success`, `stale`, or `error`
item removes it permanently.

## UI Projection

Rust projects only items belonging to the active terminal session into a Slint
`ClipboardImagePreviewItem` model. Switching tabs hides but does not retarget items;
switching back restores the originating session's queue.

`ClipboardImagePreviewOverlay` is a separate component mounted in the same terminal
chrome layer as existing search, context-menu, and drop overlays. It is anchored to
the lower-right of the terminal viewport and does not affect layout inputs used for
rows, columns, or native surface geometry.

The overlay contract is:

- stable width with responsive clamping on narrow windows;
- maximum height of roughly 60% of the terminal viewport, with internal scrolling;
- oldest request first;
- thumbnail, source dimensions, and phase for pending items;
- an indeterminate preparing state, queued label, and determinate uploading state;
- upload progress bar, percentage, transferred/total bytes, and average speed;
- final byte count and speed on the compact success acknowledgement;
- `Paste path`, `Copy path`, and dismiss controls only when valid;
- a compact success acknowledgement removed after 3.2 seconds;
- persistent stale/error items until handled or dismissed;
- pointer events consumed by the overlay, while keyboard focus remains with or is
  restored to the terminal input.

Status text never enters the PTY byte stream. Existing Fluent icons and theme tokens
are reused; no new visual asset is required.

## Explicit Local Inline Display

### Entry And Preparation

`Ctrl+Shift+I` and the terminal context-menu action `Display Clipboard Image` call
one bootstrap callback. Standard `Ctrl+Shift+V`, Shift+Insert, and `Paste` keep their
existing upload-and-path behavior.

The callback captures the active session UUID and active-session generation, then
rejects alternate-screen, mouse-grabbed, or application-cursor state immediately.
It reads the image-first clipboard payload and runs the existing bounded PNG
preparation off the UI thread. The result carries its request UUID, originating
session UUID, and generation. Immediately before placement, the event thread
verifies that the request remains current, the generation is unchanged, the same
session is still active and live, and its current surface is still outside all
three interactive TUI modes. A failed check releases the PNG and shows bounded
client feedback.

### Sizing And Placement

Sizing uses the current surface rather than capture-time metrics. Let the available
width be the pixels represented by terminal columns from the current cursor through
the right edge, and let the height cap be 50 percent of the viewport pixel height.
The scale is the smaller of 1.0, available width divided by source width, and the
height cap divided by source height. The pixel rectangle is never enlarged and is
rounded up to complete cell spans.

The terminal core attaches the image at the current cursor, advances through the
occupied rows using its existing Kitty-style cell placement semantics, then places
the cursor at column zero below the image. The viewport snaps to the live bottom.
The attached image cells therefore scroll, clear, resize/reflow, render, and leave
scrollback through the same model as protocol-produced image cells.

### Terminal-Core Contract

Add a typed local ingress to the terminal abstraction:

```text
apply_local_image(LocalTerminalImage {
  png_bytes,
  source_width,
  source_height,
  columns,
  rows,
}) -> Result
```

`TerminalSession`, its runtime control, and `SessionManager` expose corresponding
session-bound operations. The runtime locks the target terminal, rechecks the
surface state, applies the image, obtains the updated snapshot, and updates only
that session's projected surface.

The WezTerm adapter does not call `apply_remote_bytes`, the remote image ingress
guard, the Kitty parser, or the terminal reply writer. The vendored
`tattoy-wezterm-term` patch only makes its existing cell-image placement primitive
callable by the adapter. The adapter supplies `image_id=None` and
`placement_id=None`, so local cells do not enter the Kitty registry and cannot
collide with or be deleted through remote Kitty IDs. Standard clear/scroll/session
lifecycle still owns the cells normally. The operation must leave the shared writer
empty; any unexpected generated bytes are discarded and treated as an invariant
failure, never forwarded to SSH.

### Local Resource Budget

Per-image validation retains the existing 25-million-pixel, 20-MiB encoded, and
100-MiB decoded limits. The adapter tracks the decoded-size estimate and a weak
reference for each local image. Live local images may consume at most 128 MiB per
session. Expired weak references are removed before each admission check; if the
new image would exceed the remaining budget, placement fails without evicting an
image still present in terminal history. Scrollback eviction, clear, memory release,
and session teardown drop the final cell references naturally.

## Lifecycle and Cleanup

The existing 50 ms session projection timer drains preparation/upload messages,
starts the next ordered upload, removes expired success items, and synchronizes the
active-session Slint model only when state changed.

Closing a terminal session removes its epochs, queued PNGs, thumbnails, paths, and
captured runtimes immediately. A preparation or upload already executing may finish,
but its request-ID result is ignored. The captured SFTP binding still prevents any
completion from entering another or replacement session.

Queue capacity counts visible and hidden in-flight/actionable requests. Expired or
permanently dismissed terminal items stop counting immediately.

Local inline preparation owns only its captured session UUID and bounded prepared
PNG. It owns no SFTP runtime or remote path. Switching the active session, closing
the originating session, entering a guarded TUI mode, or failing the local resource
admission check releases that result without mutating any other session. Local image
budget bookkeeping is session-owned and is discarded with the terminal core.

## Error and Logging Contract

- Paste preparation and upload errors appear on the preview item. Existing global
  error feedback may remain secondary, but no Transfer Center transfer is created.
- Queue-full rejection is immediate and starts no background work.
- Reconnect/session replacement is a connection error, not an input-stale result;
  `Paste path` must never target the new binding.
- Local-display no-image, decode, size, resource-budget, session, TUI, and placement
  failures use bounded client feedback and never fall back to Paste or upload.
- Local-image core errors and invariant violations produce no SSH or PTY bytes.
- Raw clipboard bytes, decoded pixels, thumbnails, and PNG payloads are never logged.
- Logs may contain request/session/binding UUIDs, dimensions, encoded byte count,
  controlled remote path metadata, lifecycle state, and bounded error text.

## Compatibility

- Text-only Paste retains image-first selection, newline normalization, multiline
  review, bracketed-paste behavior, and both existing Paste entry points.
- Existing Windows registered PNG, bitmap/DIB/DIBV5, and single-file extraction
  behavior remains unchanged.
- Existing SFTP cache paths, 0700 directories, 0600 files, cleanup, and binding
  checks remain unchanged.
- `Ctrl+Shift+I` and `Display Clipboard Image` are client-owned commands; they are
  disabled in interactive TUI state and do not require shell integration.
- `TERM=xterm-256color` and best-effort `TERM_PROGRAM` negotiation remain unchanged.
- Remote Kitty, iTerm2, and Sixel parsing/rendering remains entirely independent.

## Verification Strategy

Pure paste-controller tests cover queue capacity, request identity, input epochs,
ordered preparation, single-upload dispatch, monotonic progress, stale/unknown
progress rejection, state transitions, dismissal, expiry, session cleanup, and the
rule that the first automatic insertion makes later same-epoch requests stale.

SFTP tests use a recording writer to prove 64 KiB chunking, acknowledged-byte
progress, throttled intermediate updates, mandatory final values, and deletion of a
partially written remote file after failure. Timing tests use injected monotonic
timestamps rather than wall-clock sleeps.

Clipboard and sizing tests retain all decode/encode/thumbnail limits and add
original-size, no-upscale, available-cursor-width, half-viewport-height, aspect
ratio, and cell-rounding cases.

Terminal-core tests prove that local ingestion:

- produces one bounded resource and correctly anchored cell placement;
- advances the cursor to column zero below the reserved rows;
- follows scrollback, clear, resize/reflow, and session release;
- uses neither Kitty image nor placement IDs;
- cannot be removed by a remote Kitty ID deletion;
- produces no reply-writer or SSH bytes;
- enforces the local per-session resource budget;
- is projected identically to the native and bitmap rendering contracts.

Bootstrap/session-manager tests cover both inline entry points, no-image feedback,
session switch/close races, TUI rejection before and after preparation, unchanged
standard Paste, safe/stale path completion, and binding replacement. Slint
compilation and projection tests cover progress fields, progress-bar geometry,
labels, callback wiring, and narrow-window text fit.

Full verification includes formatting, focused and full Linux tests, Clippy, diff
checks, Windows GNU/MSVC all-target checks, and native/bitmap renderer feature
combinations.

Manual Windows verification covers:

1. screenshot `Ctrl+Shift+V` with preview, visible progress/final speed, and quoted
   path insertion without a newline;
2. typing or submitting input during upload, with no delayed bytes and actionable
   stale recovery;
3. `Ctrl+Shift+I` and `Display Clipboard Image` producing the same grid image at an
   ordinary shell, followed by scrolling into and out of history;
4. a large screenshot scaling down without upscaling a small image;
5. alternate-screen or mouse-grabbed TUI rejection with no layout mutation;
6. tab switching, pending completion, session close, reconnect, copied image-file,
   and text-only Paste;
7. the direct Kitty blue-block sequence, proving remote rendering is unchanged.

## Rollback

Upload progress fields and chunk callbacks can be reverted independently while
retaining current upload semantics. The local-image UI callback, runtime/core
ingress, adapter bookkeeping, and minimal vendor visibility change form a second
independent rollback unit. Neither rollback changes clipboard format reading,
remote cache layout, standard Paste, nor remote protocol parsing. Remote PNG files
already uploaded remain ordinary cache entries under the existing cleanup policy.
