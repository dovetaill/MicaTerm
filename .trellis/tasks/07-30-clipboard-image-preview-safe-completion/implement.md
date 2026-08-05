# Clipboard Image Upload Progress and Explicit Inline Display Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use inline execution (recommended) or manual inline execution to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add measured progress to standard clipboard-image upload and add an explicit, local-only action that places the clipboard image in terminal cells and scrollback without changing ordinary Paste or sending anything to the remote host.

**Architecture:** Keep the existing Paste pipeline as a session-bound SFTP queue, adding chunk progress messages and stable projection fields. Build inline display as a separate one-request controller that prepares a bounded PNG off the UI thread, revalidates its session generation and TUI state, and calls a dedicated terminal-core cell-placement API. The local API uses the existing WezTerm cell image primitive directly with no protocol IDs, PTY bytes, SSH replies, SFTP operation, or remote helper.

**Tech Stack:** Rust, Slint 1.15.1, Tokio, `image` 0.25.9, the existing Windows clipboard adapter, SSH session manager, vendored `tattoy-wezterm-term`, and the existing native/bitmap terminal renderers.

## Global Constraints

- Treat the current bounded preview, eight-item ordered upload queue, input-epoch completion guard, stale path recovery, and Windows clipboard repair as an implemented baseline. Preserve it.
- Keep `Ctrl+Shift+V` and terminal-menu Paste as upload-and-quoted-path. They must never render into the grid implicitly.
- Add inline display only at `Ctrl+Shift+I` and `Display Clipboard Image`; both entry points must invoke one Rust action.
- Preserve text-only Paste, image-over-text clipboard precedence, multiline confirmation, bracketed paste, remote permissions, stale-cache cleanup, and reconnect safety.
- Do not send preview data, progress text, local-image bytes, control sequences, or failure messages to the PTY.
- Do not create a Transfer Center transfer for either clipboard workflow. Existing client feedback may be reused without enqueuing a transfer.
- Keep upload writes at 64 KiB and UI progress at no more than roughly 10 updates per second, except mandatory initial and final messages.
- Use monotonic elapsed time and cumulative average speed. Never display a negative or regressing byte count.
- Keep the existing 25-million-pixel, 20 MiB encoded, and 100 MiB decoded limits. Keep local inline resources to 128 MiB retained decoded bytes per session.
- Place a local image at the current cursor, constrain it to cursor-right grid width and half the current viewport height, never upscale, reserve rows, finish below the image, and retain the cells in scrollback.
- Reject inline display while alternate-screen, mouse-grabbed, or application-cursor mode is active. Recheck immediately before terminal mutation.
- A switch away and back must invalidate an earlier inline preparation result. A newer inline request must invalidate the previous pending result.
- Local cells must use `image_id: None` and `placement_id: None`; remote Kitty registry and delete operations must not own them.
- Preserve remote Kitty/iTerm2/Sixel parsing, limits, replies, placement, clearing, and both renderer paths.
- Add no dependency. Use existing `image`, `tokio`, `uuid`, Slint, and WezTerm facilities.
- Preserve all unrelated dirty-worktree changes. Do not create a Git commit unless the user explicitly authorizes it after a review gate.

## File Map

### Upload Progress Track

- Modify `src/app/sftp/runtime.rs`: expose cumulative clipboard upload progress and replace the one-shot write with 64 KiB writes.
- Modify `tests/sftp_runtime_spec.rs`: prove chunk boundaries, cumulative progress, final emission, and partial-file cleanup.
- Modify `src/app/clipboard_image_paste.rs`: retain monotonic bytes, percentage, and average speed through success expiry.
- Modify `src/app/bootstrap/workspace_terminal.rs`: add request-keyed progress messages, monotonic throttling, projection formatting, and final-state retention.
- Modify `ui/components/clipboard-image-preview-overlay.slint`: add a stable progress bar and byte/speed line to existing cards.
- Modify `tests/bootstrap_smoke.rs`: verify progress message routing and generated Slint projection behavior.

### Explicit Inline Display Track

- Create `src/app/clipboard_inline_image.rs`: own the single pending request, active-session generation, TUI guard, and pure cell sizing.
- Modify `src/app/mod.rs`: register `clipboard_inline_image`.
- Modify `src/shell/view_model.rs`: retain the monotonic active-workspace-session generation.
- Modify `src/shell/view_model/workspace.rs`: expose the generation and advance it from central tab normalization.
- Modify `vendor/tattoy-wezterm-term/src/terminalstate/image.rs`: make only the existing `assign_image_to_cells` method public.
- Modify `vendor/tattoy-wezterm-term/src/terminalstate/mod.rs`: publicly re-export only `ImageAttachParams`, `ImageAttachStyle`, and `PlacementInfo`.
- Modify `src/app/terminal_core/types.rs`: define `LocalTerminalImage` and the `TerminalCoreAdapter::apply_local_image` boundary.
- Modify `src/app/terminal_core/mod.rs`: re-export `LocalTerminalImage`.
- Modify `src/app/terminal_core/wezterm_adapter.rs`: validate, budget, place, advance, and release local image resources.
- Modify `src/app/ssh/runtime/terminal.rs`: expose local placement on `TerminalSession`.
- Modify `src/app/ssh/runtime.rs`: apply local placement under the live terminal lock and return the updated surface.
- Modify `src/app/ssh/session_manager.rs`: bridge local placement to the correct runtime and refresh the projected surface.
- Modify `src/app/bootstrap/workspace_terminal.rs`: capture/prepare/revalidate/apply inline requests and show bounded client feedback.
- Modify `src/app/bootstrap.rs`: instantiate/drain the inline controller, read the active-session generation, and bind the action.
- Modify `ui/shell/terminal-session-host.slint`: add `Ctrl+Shift+I`, the terminal context-menu command, and the callback.
- Modify `ui/shell/workspace-pane.slint`: forward the callback.
- Modify `ui/app-window.slint`: expose the callback to Rust.
- Modify `tests/terminal_inline_image_spec.rs`: prove local cell placement, cursor/scrollback behavior, no protocol IDs, and remote-delete isolation.
- Modify `tests/bootstrap_smoke.rs`: prove UI callback wiring, TUI rejection, generation invalidation, and no Paste fallback.
- Modify `tests/workspace_tabs_spec.rs`: prove every real active-session transition, including A -> B -> A, advances generation.

### Final Evidence

- Modify `.trellis/spec/backend/quality-guidelines.md`: record the verified local-ingress and progress contracts.
- Modify `.trellis/tasks/07-30-clipboard-image-preview-safe-completion/prd.md`: check acceptance items only when evidence exists and update implementation status.

---

## Track A: Standard Paste Upload Progress

### Task 1: Add Chunked Clipboard Upload Progress at the SFTP Boundary

**Files:**
- Modify: `src/app/sftp/runtime.rs`
- Test: `tests/sftp_runtime_spec.rs`

**Interfaces:**
- Consumes: the existing private-cache path, `CreateNew { permissions: 0o600 }`, and failure cleanup.
- Produces: a callback-based upload API used only by clipboard Paste; the current no-callback API remains compatible.

- [ ] **Step 1: Add failing SFTP progress tests**

Extend the recording backend/writer in `tests/sftp_runtime_spec.rs` so it stores each successful `poll_write` length. Add tests with a payload larger than two chunks:

```rust
#[tokio::test]
async fn clipboard_upload_reports_cumulative_chunk_progress() {
    let payload = vec![0x5a; CLIPBOARD_UPLOAD_CHUNK_BYTES * 2 + 17];
    let mut progress = Vec::new();

    runtime
        .upload_clipboard_png_with_progress(session_id, payload.clone(), |value| {
            progress.push(value);
        })
        .await
        .expect("upload clipboard image");

    assert_eq!(progress.first().map(|value| value.bytes_transferred), Some(0));
    assert_eq!(progress.last().map(|value| value.bytes_transferred), Some(payload.len() as u64));
    assert!(progress.windows(2).all(|pair| {
        pair[0].bytes_transferred <= pair[1].bytes_transferred
            && pair[0].bytes_total == pair[1].bytes_total
    }));
    assert!(backend.successful_write_lengths().iter().all(|length| {
        *length <= CLIPBOARD_UPLOAD_CHUNK_BYTES
    }));
}
```

Add a failure test that makes the second chunk fail and asserts that the partial remote file is removed and no final `bytes_total` sample is reported.

- [ ] **Step 2: Run the focused tests and confirm the missing API failure**

```bash
cargo test --test sftp_runtime_spec clipboard_upload_reports_cumulative_chunk_progress -- --nocapture
cargo test --test sftp_runtime_spec clipboard_upload_removes_partial_file_after_chunk_failure -- --nocapture
```

Expected: compilation fails because the progress API and chunk constant do not exist.

- [ ] **Step 3: Add the exact progress contract**

In `src/app/sftp/runtime.rs` add:

```rust
pub const CLIPBOARD_UPLOAD_CHUNK_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipboardUploadProgress {
    pub bytes_transferred: u64,
    pub bytes_total: u64,
}
```

Keep `upload_clipboard_png` as the compatibility entry point:

```rust
pub async fn upload_clipboard_png(&self, session_id: Uuid, data: Vec<u8>) -> Result<String> {
    self.upload_clipboard_png_with_progress(session_id, data, |_| {})
        .await
}

pub async fn upload_clipboard_png_with_progress<F>(
    &self,
    session_id: Uuid,
    data: Vec<u8>,
    on_progress: F,
) -> Result<String>
where
    F: FnMut(ClipboardUploadProgress) + Send,
{
    let now_unix_seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    self.upload_clipboard_png_at(
        session_id,
        data,
        now_unix_seconds,
        on_progress,
    )
    .await
}
```

Change the private implementation to accept `mut on_progress: F`. After the writer is successfully opened, emit `0/total`. Write `data.chunks(CLIPBOARD_UPLOAD_CHUNK_BYTES)` with `write_all`; after each non-final successful chunk emit cumulative progress. Flush and shut down using the existing sequence, then emit the mandatory final `total/total` sample. Retain the existing `drop(writer)` plus `remove_file` cleanup on every write, flush, or shutdown error.

- [ ] **Step 4: Run SFTP runtime tests**

```bash
cargo test --test sftp_runtime_spec clipboard_upload -- --nocapture
```

Expected: chunk, completion, mode 0600, directory 0700, stale cleanup, size-limit, and partial-file cleanup cases pass.

- [ ] **Step 5: Review checkpoint**

Inspect the diff and confirm the callback cannot change the remote path or skip cleanup, and the legacy API still delegates to the exact same implementation. Do not commit.

---

### Task 2: Retain Monotonic Progress and Average Speed in the Paste Controller

**Files:**
- Modify: `src/app/clipboard_image_paste.rs`
- Test: inline `tests` in `src/app/clipboard_image_paste.rs`

**Interfaces:**
- Consumes: request UUID, encoded byte total, and monotonic elapsed duration from Task 3.
- Produces: immutable projection fields for Slint; upload success does not clear final metrics.

- [ ] **Step 1: Add failing controller tests**

Add focused tests for monotonic updates, zero elapsed time, stale request IDs, and success retention:

```rust
#[test]
fn upload_progress_is_monotonic_and_retained_after_success() {
    let (mut controller, session_id, request_id) = uploading_controller_fixture(1_024);

    assert!(controller.mark_upload_progress(
        request_id,
        512,
        1_024,
        Duration::from_secs(1),
    ));
    assert!(!controller.mark_upload_progress(
        request_id,
        256,
        1_024,
        Duration::from_secs(2),
    ));
    assert!(matches!(
        controller.mark_upload_succeeded(request_id, "/tmp/image.png".into()),
        ClipboardImageCompletion::AutoInsert(_),
    ));

    let item = controller.projections(session_id)[0].clone();
    assert_eq!(item.bytes_transferred, 512);
    assert_eq!(item.bytes_total, 1_024);
    assert_eq!(item.bytes_per_second, 512);
}
```

Also prove `bytes_transferred` is clamped to total, a mismatched total is ignored, and a zero-duration initial sample yields speed zero without division failure.

- [ ] **Step 2: Confirm the tests fail before implementation**

```bash
cargo test --lib clipboard_image_paste::tests::upload_progress -- --nocapture
```

Expected: compilation fails because progress fields and `mark_upload_progress` do not exist.

- [ ] **Step 3: Extend request and projection state**

Add these fields to the internal request and `ClipboardImagePasteProjection`:

```rust
pub(crate) bytes_transferred: u64,
pub(crate) bytes_total: u64,
pub(crate) bytes_per_second: u64,
```

Initialize `bytes_total` from `png_bytes.len()` when preparation succeeds. Add:

```rust
pub(crate) fn mark_upload_progress(
    &mut self,
    request_id: Uuid,
    bytes_transferred: u64,
    bytes_total: u64,
    elapsed: Duration,
) -> bool
```

The method must update only an `Uploading` request with the matching request ID and the already-recorded total. Reject regressing values. Clamp the accepted byte count to total. Compute cumulative average speed with checked integer nanoseconds:

```rust
fn average_bytes_per_second(bytes: u64, elapsed: Duration) -> u64 {
    let nanos = elapsed.as_nanos();
    if bytes == 0 || nanos == 0 {
        return 0;
    }
    u64::try_from(
        u128::from(bytes)
            .saturating_mul(1_000_000_000)
            .checked_div(nanos)
            .unwrap_or_default(),
    )
    .unwrap_or(u64::MAX)
}
```

Increment revision only when a projected value changes. Do not clear metrics in `mark_upload_succeeded`, stale completion, or the 3.2-second success state.

- [ ] **Step 4: Run all controller tests**

```bash
cargo test --lib clipboard_image_paste::tests -- --nocapture
```

Expected: queue ordering, input epochs, stale recovery, cleanup, and new progress cases all pass.

- [ ] **Step 5: Review checkpoint**

Confirm progress cannot attach to a different request and no controller transition creates a Transfer Center task or terminal input. Do not commit.

---

### Task 3: Route Throttled Progress into the Existing Preview Card

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `ui/components/clipboard-image-preview-overlay.slint`
- Test: `tests/bootstrap_smoke.rs`

**Interfaces:**
- Consumes: per-chunk cumulative samples from Task 1 and controller state from Task 2.
- Produces: at most approximately 10 intermediate UI updates per second, plus mandatory initial/final state.

- [ ] **Step 1: Add failing throttle and projection tests**

Add pure unit tests in `workspace_terminal.rs` for a gate driven by supplied `Duration` values:

```rust
#[test]
fn clipboard_progress_gate_keeps_initial_tenth_second_and_final_samples() {
    let mut gate = ClipboardProgressGate::default();
    assert!(gate.should_emit(Duration::ZERO, false));
    assert!(!gate.should_emit(Duration::from_millis(40), false));
    assert!(gate.should_emit(Duration::from_millis(100), false));
    assert!(gate.should_emit(Duration::from_millis(101), true));
}
```

Add bootstrap source/Slint smoke assertions that require a request-keyed `Progress` message and these new Slint fields:

```slint
progress-value: float,
progress-text: string,
speed-text: string,
```

- [ ] **Step 2: Run the focused tests and confirm failure**

```bash
cargo test --lib workspace_terminal::tests::clipboard_progress_gate -- --nocapture
cargo test --test bootstrap_smoke clipboard_image_upload_progress -- --nocapture
```

Expected: tests fail because the message, gate, and model fields are absent.

- [ ] **Step 3: Add the background message and monotonic throttle**

Extend `ClipboardImagePasteBackgroundMessage`:

```rust
Progress {
    request_id: Uuid,
    bytes_transferred: u64,
    bytes_total: u64,
    elapsed: Duration,
},
```

Add a `ClipboardProgressGate` whose first sample always emits, non-final samples emit only after at least `Duration::from_millis(100)` since the last emission, and final samples always emit. In `schedule_prepared_clipboard_image_upload`, capture `let started_at = Instant::now()`, pass the callback to `upload_clipboard_png_with_progress`, and send request-keyed `Progress` messages only when the gate allows. Send `Uploaded` through the same `mpsc::Sender`; FIFO ordering then guarantees final progress is drained before completion.

Handle `Progress` in `drain_clipboard_image_paste_messages` by calling `mark_upload_progress`. Ignore unknown/expired request IDs without feedback.

- [ ] **Step 4: Project stable display strings**

Add pure formatting helpers with exact tests for B, KiB, and MiB boundaries:

```rust
fn format_clipboard_transfer_progress(done: u64, total: u64) -> String;
fn format_clipboard_transfer_speed(bytes_per_second: u64) -> String;
```

Use binary units and one decimal for KiB/MiB. Format the summary as `"64.0 KiB / 1.0 MiB (6%)"` and speed as `"512.0 KiB/s"`. For a zero total, use `"0 B / 0 B (0%)"`. Project:

```rust
progress_value: if projection.bytes_total == 0 {
    0.0
} else {
    (projection.bytes_transferred as f32 / projection.bytes_total as f32).clamp(0.0, 1.0)
},
progress_text: format_clipboard_transfer_progress(
    projection.bytes_transferred,
    projection.bytes_total,
).into(),
speed_text: format_clipboard_transfer_speed(projection.bytes_per_second).into(),
```

- [ ] **Step 5: Extend the overlay without changing terminal geometry**

In `ClipboardImagePreviewItem`, add the three fields above. Within the existing fixed-width card, add one fixed-height progress track and one elided metadata row. Show them during `uploading`, `success`, and stale-success states; retain final values through the existing 3.2-second success acknowledgement. Keep the overlay out of terminal layout and do not nest a new card inside the current card.

- [ ] **Step 6: Run focused and Slint compile tests**

```bash
cargo test --lib workspace_terminal::tests -- --nocapture
cargo test --test bootstrap_smoke clipboard_image_upload_progress -- --nocapture
cargo check --all-targets
```

Expected: throttling, formatting, message routing, generated Slint bindings, and all-target compilation pass.

- [ ] **Step 7: Review checkpoint**

Confirm final progress remains visible in success, small uploads may legitimately show only initial/final values, and no transfer task or PTY output was added. Do not commit.

---

## Track B: Explicit Local Terminal-Grid Display

### Task 4: Build the Single-Pending Inline Controller and Pure Sizing Policy

**Files:**
- Create: `src/app/clipboard_inline_image.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/shell/view_model.rs`
- Modify: `src/shell/view_model/workspace.rs`
- Test: `tests/workspace_tabs_spec.rs`
- Test: inline `tests` in `src/app/clipboard_inline_image.rs`

**Interfaces:**
- Consumes: active session UUID, request UUID, source dimensions, cursor/grid size, and viewport pixels.
- Produces: a generation-bound request token and a bounded cell span.

- [ ] **Step 1: Add failing controller and sizing tests**

Register the empty module, then add tests covering:

```rust
#[test]
fn active_session_generation_change_invalidates_pending_inline_result() {
    let session_a = Uuid::new_v4();
    let mut controller = ClipboardInlineImageController::default();
    let request = controller.begin(session_a, 7);

    assert!(!controller.is_current(request, Some(session_a), 9));
}

#[test]
fn newer_inline_request_invalidates_older_result() {
    let session_a = Uuid::new_v4();
    let mut controller = ClipboardInlineImageController::default();
    let first = controller.begin(session_a, 7);
    let second = controller.begin(session_a, 7);

    assert!(!controller.is_current(first, Some(session_a), 7));
    assert!(controller.is_current(second, Some(session_a), 7));
}
```

In `tests/workspace_tabs_spec.rs`, add an A -> B -> A case that asserts the generation advances twice, while reactivating the already-active tab leaves it unchanged. Add sizing cases for no upscale, a cursor near the right edge, half-viewport height, one-cell minimum, zero/invalid source dimensions, and aspect ratio within one cell of the source ratio.

- [ ] **Step 2: Run tests and confirm missing types**

```bash
cargo test --lib clipboard_inline_image::tests -- --nocapture
```

Expected: compilation fails because the module contracts are not implemented.

- [ ] **Step 3: Implement central active-session generation**

Add `active_workspace_session_generation: u64` to `ShellViewModel`, initialize it to zero, and expose this getter in `workspace.rs`:

```rust
pub fn active_workspace_session_generation(&self) -> u64 {
    self.active_workspace_session_generation
}
```

In `normalize_workspace_tabs`, compute `next_active_workspace_session_id` before assignment. If it differs from the previous `active_workspace_session_id`, advance the generation with `wrapping_add(1)` and then assign the new ID. Direct activation, launcher transitions, close fallback, connection projection, and tab replacement already converge on this normalization path, so A -> B -> A records two changes even if no background result completes between them.

- [ ] **Step 4: Implement pending-request ownership**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ClipboardInlineImageRequest {
    pub request_id: Uuid,
    pub session_id: Uuid,
    pub active_session_generation: u64,
}

#[derive(Debug, Default)]
pub(crate) struct ClipboardInlineImageController {
    pending: Option<ClipboardInlineImageRequest>,
}
```

`begin(session_id, active_session_generation)` replaces the single pending request. `is_current` must require matching pending request UUID, session UUID, captured generation, active session UUID, and current generation. Add `finish_if_current` to atomically validate and clear the pending token before apply.

- [ ] **Step 5: Implement the sizing contract**

Add:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LocalInlineImageCellSize {
    pub columns: u32,
    pub rows: u32,
}

pub(crate) fn inline_image_cell_size(
    source_width: u32,
    source_height: u32,
    surface: &TerminalSurfaceState,
) -> Result<LocalInlineImageCellSize>
```

Use `cell_width = max(1, pixel_width / cols)` and `cell_height = max(1, pixel_height / rows)`. Use `available_columns = max(1, cols - min(cursor.col, cols - 1))`, `max_width_px = available_columns * cell_width`, and `max_height_px = max(1, pixel_height / 2)`. Compute `scale = min(1.0, max_width/source_width, max_height/source_height)`, floor scaled pixel dimensions to at least one pixel, then use checked integer ceiling division to complete cell spans. Clamp the final columns/rows to the available columns and half-viewport pixel bound. Reject zero source dimensions or a zero grid before any mutation.

Add a shared guard:

```rust
pub(crate) fn surface_allows_inline_image(surface: &TerminalSurfaceState) -> bool {
    !surface.alternate_screen_active
        && !surface.mouse_grabbed
        && !surface.application_cursor_keys
}
```

- [ ] **Step 6: Run all module and workspace-generation tests**

```bash
cargo test --lib clipboard_inline_image::tests -- --nocapture
cargo test --test workspace_tabs_spec active_workspace_session_generation -- --nocapture
```

Expected: generation, replacement, guard, sizing, and overflow cases pass.

- [ ] **Step 7: Review checkpoint**

Confirm generation changes in the central normalization path on actual active-session transitions and does not change for ordinary surface refreshes. Do not commit.

---

### Task 5: Add a Dedicated Terminal-Core Local Image Ingress

**Files:**
- Modify: `vendor/tattoy-wezterm-term/src/terminalstate/image.rs`
- Modify: `vendor/tattoy-wezterm-term/src/terminalstate/mod.rs`
- Modify: `src/app/terminal_core/types.rs`
- Modify: `src/app/terminal_core/mod.rs`
- Modify: `src/app/terminal_core/wezterm_adapter.rs`
- Test: `tests/terminal_inline_image_spec.rs`
- Test: inline `tests` in `src/app/terminal_core/wezterm_adapter.rs`

**Interfaces:**
- Consumes: a validated local PNG plus source dimensions and cell span.
- Produces: terminal image cells and a cursor below them; the return type contains no SSH reply bytes.

- [ ] **Step 1: Add failing terminal-core contract tests**

Extend `tests/terminal_inline_image_spec.rs` with a multi-pixel PNG helper and tests equivalent to:

```rust
#[test]
fn local_clipboard_image_uses_unowned_cells_and_advances_below_it() {
    let mut terminal = TerminalSession::new(6, 12);
    terminal.apply_remote_bytes(b"prompt> ");
    let before = terminal.frame_snapshot();

    terminal
        .apply_local_image(LocalTerminalImage {
            png_bytes: fixture_png(40, 20),
            source_width: 40,
            source_height: 20,
            columns: 4,
            rows: 2,
        })
        .expect("place local clipboard image");

    let after = terminal.frame_snapshot();
    assert!(after.seqno > before.seqno);
    assert!(!after.image_placements.is_empty());
    assert!(after.image_placements.iter().all(|placement| {
        placement.image_id.is_none() && placement.placement_id.is_none()
    }));
    assert_eq!(after.cursor.col, 0);
    assert!(after.cursor.row >= before.cursor.row.saturating_add(2));
}
```

Add tests that a remote Kitty delete-by-ID and delete-all leave local placements intact, local placement enters scrollback when following lines arrive, clear/scrollback eviction releases weak budget entries, oversized inputs fail without seqno/cursor changes, and `apply_local_image` has no reply-byte return channel.

- [ ] **Step 2: Run the focused tests and confirm the API failure**

```bash
cargo test --test terminal_inline_image_spec local_clipboard_image -- --nocapture
```

Expected: compilation fails because `LocalTerminalImage` and `apply_local_image` do not exist.

- [ ] **Step 3: Expose only the existing vendored placement primitive**

In `vendor/tattoy-wezterm-term/src/terminalstate/image.rs`, change only:

```rust
pub(crate) fn assign_image_to_cells(
```

to:

```rust
pub fn assign_image_to_cells(
```

In `vendor/tattoy-wezterm-term/src/terminalstate/mod.rs`, add:

```rust
pub use image::{ImageAttachParams, ImageAttachStyle, PlacementInfo};
```

Do not expose the Kitty registry, deletion functions, protocol parser, or raw external-media helpers.

- [ ] **Step 4: Add the terminal-core value and trait boundary**

In `src/app/terminal_core/types.rs` add:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTerminalImage {
    pub png_bytes: Vec<u8>,
    pub source_width: u32,
    pub source_height: u32,
    pub columns: u32,
    pub rows: u32,
}
```

Add to `TerminalCoreAdapter`:

```rust
fn apply_local_image(&mut self, _image: LocalTerminalImage) -> Result<()> {
    bail!("terminal core does not support local images")
}
```

Import `bail` beside `Result` and re-export `LocalTerminalImage` from `terminal_core/mod.rs`. The default keeps non-WezTerm/released adapters and test doubles source-compatible while making unsupported use explicit.

- [ ] **Step 5: Implement validation and per-session weak budgeting**

Add `LocalImageResourceBudget` to `WeztermTerminalCoreAdapter`:

```rust
struct LocalImageResourceLease {
    resource: Weak<ImageData>,
    decoded_bytes: usize,
}

#[derive(Default)]
struct LocalImageResourceBudget {
    leases: VecDeque<LocalImageResourceLease>,
    retained_bytes: usize,
}
```

Before admission, remove leases whose weak reference no longer upgrades and subtract their decoded bytes. Validate non-empty dimensions and spans; `png_bytes.len() <= MAX_ENCODED_IMAGE_BYTES`; `width * height <= MAX_IMAGE_PIXELS`; and `width * height * 4 <= MAX_DECODED_IMAGE_BYTES`. Decode with the existing bounded image reader, require the decoded dimensions to equal the declared source dimensions, and reject if live local decoded bytes plus the new decoded size exceed `MAX_TERMINAL_IMAGE_RESOURCE_BYTES` (128 MiB). Rejection must happen before terminal mutation.

- [ ] **Step 6: Place cells directly and finish below the image**

Construct `Arc<ImageData>` from the bounded encoded PNG and call the newly exposed primitive directly:

```rust
let resource = Arc::new(ImageData::with_raw_data(image.png_bytes));
self.terminal.assign_image_to_cells(ImageAttachParams {
    image_width: image.source_width,
    image_height: image.source_height,
    source_width: None,
    source_height: None,
    source_origin_x: 0,
    source_origin_y: 0,
    cell_padding_left: 0,
    cell_padding_top: 0,
    z_index: 0,
    columns: Some(image.columns as usize),
    rows: Some(image.rows as usize),
    image_id: None,
    placement_id: None,
    style: ImageAttachStyle::Kitty,
    do_not_move_cursor: false,
    data: Arc::clone(&resource),
})?;
```

After placement, feed only local `\r\n` to the terminal state so the cursor moves from the primitive's bottom-right position to column zero below the placement. Take the shared writer before and after this operation and treat any generated bytes as an invariant failure; never return them. Snap the local viewport to bottom, retain only a weak budget lease, and increment seqno through the normal terminal mutation path.

- [ ] **Step 7: Run core, parser, and renderer projection tests**

```bash
cargo test --test terminal_inline_image_spec -- --nocapture
cargo test --lib terminal_core::wezterm_adapter::tests -- --nocapture
cargo test --test terminal_atlas_renderer_spec -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
```

Expected: local tests pass and every existing Kitty/iTerm2/Sixel fixture remains unchanged.

- [ ] **Step 8: Review checkpoint**

Confirm no local placement enters `kitty_img`, both IDs are `None`, remote delete commands affect only protocol-owned placements, and weak bookkeeping releases memory when cells/session ownership disappears. Do not commit.

---

### Task 6: Bridge Local Placement Through Runtime and Session Manager

**Files:**
- Modify: `src/app/ssh/runtime/terminal.rs`
- Modify: `src/app/ssh/runtime.rs`
- Modify: `src/app/ssh/session_manager.rs`
- Test: inline session-manager/runtime tests
- Test: `tests/bootstrap_smoke.rs` runtime double

**Interfaces:**
- Consumes: `LocalTerminalImage` for one captured session.
- Produces: an updated `TerminalSurfaceState`; no command channel, SFTP runtime, or PTY writer is involved.

- [ ] **Step 1: Add failing bridge tests**

Add a `RecordingLocalImageRuntimeControl` in `tests/bootstrap_smoke.rs` whose `apply_local_image` records the value and returns an updated surface. Add a focused manager test that verifies the targeted runtime receives exactly one image and the session registry surface seqno/image placements update. Add an unknown-session case that returns an error without retargeting another runtime.

- [ ] **Step 2: Run focused tests and confirm the missing bridge**

```bash
cargo test --test bootstrap_smoke session_manager_applies_local_image_to_target_runtime -- --nocapture
```

Expected: compilation fails because the trait and manager methods do not exist.

- [ ] **Step 3: Add terminal and live-runtime methods**

In `TerminalSession` add:

```rust
pub fn apply_local_image(&mut self, image: LocalTerminalImage) -> Result<()> {
    self.core.apply_local_image(image)
}
```

In `SshSessionRuntime` add:

```rust
pub fn apply_local_image(&self, image: LocalTerminalImage) -> Result<TerminalSurfaceState>
```

Lock the live `TerminalSession`, obtain its current surface, reject alternate-screen, mouse-grabbed, or application-cursor state while still under that lock, call `terminal.apply_local_image(image)`, and return `terminal.surface_state(self.session_id)`. This is the second and authoritative TUI check.

- [ ] **Step 4: Add the manager contract and projection refresh**

Add a default unsupported method to `SessionRuntimeControl`:

```rust
fn apply_local_image(&self, _image: LocalTerminalImage) -> Result<TerminalSurfaceState> {
    Err(anyhow!("session runtime does not support local images"))
}
```

Override it in `SshSessionRuntime`. Add:

```rust
pub fn apply_session_local_image(
    &self,
    session_id: Uuid,
    image: LocalTerminalImage,
) -> Result<()> {
    let surface = self
        .runtime_control_for_session(session_id)?
        .lock()
        .expect("lock session runtime control for local image")
        .apply_local_image(image)?;
    update_terminal_surface(&self.registry, session_id, surface);
    Ok(())
}
```

The default method avoids mechanical changes to unrelated runtime test doubles. No implementation may call `send_text_input`, `send_paste`, the SSH command channel, or `sftp_runtime`.

- [ ] **Step 5: Run focused interaction tests**

```bash
cargo test --test bootstrap_smoke session_manager_applies_local_image_to_target_runtime -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test terminal_inline_image_spec local_clipboard_image -- --nocapture
```

Expected: manager targeting, live TUI recheck, surface refresh, and existing input behavior pass.

- [ ] **Step 6: Review checkpoint**

Trace the call graph from manager to terminal core and verify the only output is an updated local surface snapshot. Do not commit.

---

### Task 7: Capture, Prepare, Revalidate, and Apply Inline Clipboard Images

**Files:**
- Modify: `src/app/bootstrap/workspace_terminal.rs`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`

**Interfaces:**
- Consumes: system clipboard image only, current surface/session, the existing two-permit preparation gate, and Task 4 request tokens.
- Produces: one local placement or bounded client feedback; never text fallback, upload, or remote action.

- [ ] **Step 1: Add failing lifecycle tests**

Add bootstrap tests with injected clipboard and runtime doubles for:

- valid image -> one `apply_local_image`, zero `send_paste`, zero SFTP upload;
- no clipboard image -> feedback and no text fallback;
- alternate-screen, mouse-grabbed, and application-cursor state -> feedback and no mutation;
- switch A -> B -> A while encoding -> old result ignored;
- request 1 followed by request 2 -> only request 2 may apply;
- session close or runtime replacement -> prepared result ignored;
- state becomes interactive after capture -> runtime recheck rejects it.

- [ ] **Step 2: Run focused tests and confirm failure**

```bash
cargo test --test bootstrap_smoke clipboard_inline_image -- --nocapture
```

Expected: tests fail because no inline background path or controller binding exists.

- [ ] **Step 3: Add a separate result channel**

In `workspace_terminal.rs` add:

```rust
pub(super) enum ClipboardInlineImageBackgroundMessage {
    Prepared {
        request: ClipboardInlineImageRequest,
        result: std::result::Result<EncodedClipboardImage, String>,
    },
}
```

Do not reuse `ClipboardImagePasteBackgroundMessage`; the workflows have different ownership and completion semantics. Share only the existing `Arc<Semaphore>` preparation limit. Schedule `encode_clipboard_image` with `spawn_blocking` exactly as Paste does and return the captured token with the result.

- [ ] **Step 4: Implement the action start guard**

Add `forward_active_workspace_inline_clipboard_image`. It must:

1. Resolve the active terminal session and current surface.
2. Read `state.active_workspace_session_generation()`.
3. Reject interactive TUI state with client feedback before reading the clipboard.
4. Call `system_clipboard_image_source()` directly; do not call `select_clipboard_payload` and do not read text.
5. On an image, call `controller.begin(session_id, generation)` and schedule preparation.
6. On empty/error clipboard, show bounded feedback and perform no fallback.

Feedback may use `ShellViewModel::show_transfer_center_feedback` as the existing client toast mechanism, but this path must not add an SFTP/Transfer Center task.

- [ ] **Step 5: Instantiate the controller beside the existing Paste controller**

Own `Rc<RefCell<ClipboardInlineImageController>>` in `bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge`. The controller does not poll or infer tab changes; request start and completion read the central `ShellViewModel::active_workspace_session_generation()` added in Task 4.

- [ ] **Step 6: Drain with final revalidation**

For each prepared result:

1. Read the current active session UUID and `active_workspace_session_generation()`.
2. Use `finish_if_current` to require request UUID, session UUID, captured generation, pending ownership, active UUID, and current generation.
3. Resolve the current surface from `SessionManager`, not the stale capture-time surface.
4. Recheck all three TUI flags.
5. Compute `inline_image_cell_size` from current cursor/grid/viewport metrics.
6. Build `LocalTerminalImage` from the encoded PNG and current cell span.
7. Call `manager.apply_session_local_image(request.session_id, image)`; the runtime performs the locked authoritative TUI check.
8. Refresh/snap the active local viewport projection and show feedback only on errors.

Unknown or superseded requests are ignored and never retargeted. Switched, closed, replaced, or newly guarded requests release their PNG and show bounded feedback. Failure never calls the Paste path.

- [ ] **Step 7: Run lifecycle tests**

```bash
cargo test --test bootstrap_smoke clipboard_inline_image -- --nocapture
cargo test --lib clipboard_inline_image::tests -- --nocapture
```

Expected: all valid, empty, TUI, switch, replacement, close, and newer-request cases pass.

- [ ] **Step 8: Review checkpoint**

Search the inline handler and verify it contains no SFTP method, `send_session_paste`, `send_text_input`, shell command, protocol escape, or text clipboard fallback. Do not commit.

---

### Task 8: Add the Explicit Shortcut and Context-Menu Action

**Files:**
- Modify: `ui/shell/terminal-session-host.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/app-window.slint`
- Modify: `src/app/bootstrap.rs`
- Test: `tests/bootstrap_smoke.rs`
- Test: `tests/ssh_terminal_interaction_spec.rs`

**Interfaces:**
- Consumes: `Ctrl+Shift+I` or one terminal context-menu click.
- Produces: the same `workspace-session-display-clipboard-image-requested()` callback.

- [ ] **Step 1: Add failing source-contract and invocation tests**

Require all three Slint layers to expose/forward one callback, require `Display Clipboard Image` beside Paste in the terminal context menu, and invoke the generated AppWindow callback in a bootstrap test to prove it reaches the inline runtime double exactly once.

Also assert the existing `paste-requested()` path remains present and distinct.

- [ ] **Step 2: Run tests and confirm missing wiring**

```bash
cargo test --test bootstrap_smoke display_clipboard_image_action -- --nocapture
cargo test --test ssh_terminal_interaction_spec display_clipboard_image_shortcut -- --nocapture
```

Expected: tests fail because the callback, menu item, and shortcut do not exist.

- [ ] **Step 3: Add callback propagation**

Add `display-clipboard-image-requested()` to `TerminalSessionHost`, forward it through `WorkspacePane`, expose `workspace-session-display-clipboard-image-requested()` in `AppWindow`, and bind it in `bootstrap.rs` to the Task 7 action. Keep naming distinct from `paste-requested()` and `workspace-session-clipboard-image-paste-path-requested(string)`.

- [ ] **Step 4: Add keyboard handling**

In the terminal key handler, before generic Ctrl/Shift key forwarding, accept only Control+Shift+I (case-insensitive text/key handling following the existing Paste helper), close the terminal context menu if open, invoke `display-clipboard-image-requested()`, and return `accept`. Do not change the `Ctrl+Shift+V` branch.

- [ ] **Step 5: Add the context-menu row**

Add a normal command row labeled exactly `Display Clipboard Image` directly after Paste. Use the existing menu row component and spacing; invoking it closes the menu and calls the same callback. Leave it invokable in terminal sessions so Rust can provide the required TUI rejection feedback rather than silently disabling the action.

- [ ] **Step 6: Run UI and bootstrap tests**

```bash
cargo test --test bootstrap_smoke display_clipboard_image_action -- --nocapture
cargo test --test ssh_terminal_interaction_spec display_clipboard_image_shortcut -- --nocapture
cargo check --all-targets
```

Expected: shortcut and context-menu paths each invoke the same Rust action; standard Paste remains unchanged.

- [ ] **Step 7: Review checkpoint**

Inspect the rendered menu at narrow and normal terminal widths. Confirm labels do not overlap, the menu remains inside the viewport, and neither action changes terminal grid dimensions. Do not commit.

---

## Integration Gate

### Task 9: Cross-Track Regression, Windows Acceptance, and Contract Documentation

**Files:**
- Verify: every file in the File Map
- Modify: `.trellis/spec/backend/quality-guidelines.md`
- Modify: `.trellis/tasks/07-30-clipboard-image-preview-safe-completion/prd.md`

**Interfaces:**
- Consumes: both completed tracks.
- Produces: reproducible automated evidence and a Windows manual checklist ready for the user's explicit commit decision.

- [ ] **Step 1: Run formatting and structural checks**

```bash
cargo fmt --all -- --check
git diff --check
python3 ./.trellis/scripts/task.py validate 07-30-clipboard-image-preview-safe-completion
```

Expected: every command exits 0.

- [ ] **Step 2: Run all focused suites**

```bash
cargo test --lib clipboard::tests -- --nocapture
cargo test --lib clipboard_image_paste::tests -- --nocapture
cargo test --lib clipboard_inline_image::tests -- --nocapture
cargo test --test sftp_runtime_spec clipboard_upload -- --nocapture
cargo test --test terminal_inline_image_spec -- --nocapture
cargo test --test ssh_terminal_interaction_spec -- --nocapture
cargo test --test terminal_atlas_renderer_spec -- --nocapture
cargo test --test native_terminal_surface_contract_spec -- --nocapture
cargo test --test bootstrap_smoke clipboard_image -- --nocapture
```

Expected: all focused tests pass. Record actual counts and any intentionally filtered tests in the handoff.

- [ ] **Step 3: Run the complete Linux quality gate**

```bash
cargo check --all-targets
cargo clippy --all-targets --no-deps -- -D warnings
cargo test --all-targets --quiet -- --skip bundled_font_assets_cover_terminal_and_shell_contracts
```

Expected: every command exits 0. If the repository baseline contains unrelated warnings, rerun Clippy without `-D warnings`, document the baseline, and require zero new warnings from changed files.

- [ ] **Step 4: Compile renderer/build combinations**

```bash
cargo check --no-default-features --features slint-renderer-software --all-targets
cargo check --no-default-features --features slint-renderer-skia --all-targets
cargo check --no-default-features --features slint-renderer-software,terminal-native-renderer --all-targets
cargo check --no-default-features --features slint-renderer-skia,terminal-native-renderer --all-targets
```

Expected: bitmap-only and native-presenter builds compile with both Slint renderer selections.

- [ ] **Step 5: Cross-check supported Windows toolchains**

```bash
cargo check --target x86_64-pc-windows-gnu --all-targets
cargo xwin check --target x86_64-pc-windows-msvc --all-targets
```

Expected: both commands exit 0 and compile Windows clipboard acquisition plus Slint bindings.

- [ ] **Step 6: Perform Windows manual acceptance**

Run each scenario in Mica Term and retain screenshots/log excerpts where useful:

1. Start with `img=` at a Bash prompt, take a `Win+Shift+S` screenshot, press `Ctrl+Shift+V` once, and confirm the existing preview appears. Confirm uploading shows bytes/total/percentage/speed or, for a very fast transfer, the success card retains the final measurements for 3.2 seconds. Confirm only the quoted remote path is inserted after `img=` and no newline is sent.
2. Run `file "$img"` and `stat -c 'file-mode=%a bytes=%s path=%n' "$img"`; confirm a valid PNG, mode 0600, and the private 0700 session cache directory.
3. Start another image Paste, type before upload completion, and confirm no delayed path enters the modified input. Use `Paste path` and `Copy path` deliberately from the stale card.
4. Take a screenshot and press `Ctrl+Shift+I`. Confirm no preview upload card is created, no remote cache file is added, the image appears at the current cursor, and the prompt/cursor finishes at column zero below it. Repeat through `Display Clipboard Image` and confirm identical behavior.
5. Display a small image and confirm it is not enlarged. Display a wide/tall image near the right edge and confirm it fits cursor-right width and half viewport height without aspect distortion beyond cell rounding.
6. Print enough lines to move the local image into scrollback, scroll up, and confirm both native and bitmap presenter modes retain the image in terminal history. Clear/evict history and confirm the app remains stable.
7. Enter alternate-screen/application-cursor mode with `printf '\033[?1049h\033[?1h'`, invoke both inline actions, and confirm local feedback with no image or terminal bytes. Restore with `printf '\033[?1l\033[?1049l'`. Repeat in a mouse-reporting TUI.
8. Start inline display on session A, switch A -> B -> A before preparation completes, and confirm the old image never appears. Start two inline requests rapidly and confirm only the newer pending request can appear.
9. Copy ordinary text and confirm one-line, multiline-confirmation, and bracketed-paste behavior remain unchanged.
10. Re-run the known direct Kitty RGBA Python fixture and confirm the blue block still renders. Delete its Kitty ID and confirm protocol-owned content is removed without affecting a separately displayed local clipboard image.

- [ ] **Step 7: Record durable contracts only after evidence**

In `.trellis/spec/backend/quality-guidelines.md`, add concise rules covering:

- request-keyed monotonic clipboard upload progress with mandatory final state;
- local terminal-image ingress must have no PTY/SSH/SFTP side effect;
- local cells have no remote protocol IDs and use weak per-session budget accounting;
- async local placement requires active-session generation and locked TUI revalidation.

Check only PRD acceptance items supported by test output or Windows observations. Update Implementation Status with the actual remaining gap.

- [ ] **Step 8: Present the final review gate**

Report changed files, focused/full test counts, renderer and Windows build results, manual evidence, remaining gaps, and:

```bash
git status --short
git diff --stat
```

Do not commit. If the user later explicitly authorizes a commit, show `git diff --cached --stat` and the exact staged paths before committing; do not stage unrelated worktree changes.

## Requirement Coverage Matrix

| Requirement | Primary Tasks | Proof |
|---|---:|---|
| R1 | 1-3, 9 | Existing text/image Paste regressions and Windows manual cases 1 and 9 |
| R2 | 1-3, 9 | SFTP permission, cleanup, limit, replacement, and Windows path tests |
| R3 | 3, 9 | Overlay layout/source tests and unchanged terminal geometry |
| R4 | 2-3, 9 | Lifecycle projection tests and no-PTY status review |
| R5 | 2-3, 9 | Input-epoch/stale recovery tests and Windows manual case 3 |
| R6 | 5, 9 | Full terminal protocol fixtures and renderer combinations |
| R7 | 1-3 | Existing queue ordering plus request-keyed progress tests |
| R8 | 1-3 | Chunk, throttle, speed, projection, and no-transfer-task tests |
| R9 | 7-8 | Distinct callback/source-contract tests and manual cases 1/4 |
| R10 | 5-6 | Terminal placement, cursor, scrollback, and surface-refresh tests |
| R11 | 4-5 | Pure no-upscale, width, height, and cell-rounding tests |
| R12 | 4, 6-8 | Start guard, locked runtime guard, and TUI manual case |
| R13 | 5-7 | `Result<()>` local ingress, zero-writer invariant, runtime call-graph review |
| R14 | 4-5 | Boundary and weak-budget tests |
| R15 | 4, 6-7 | A -> B -> A, newer request, close/replacement, and locked apply tests |

## Rollback Points

- Track A can be reverted by restoring the one-shot SFTP write and removing progress-only fields/messages/UI; the existing safe upload/path behavior remains intact.
- Tasks 4 and 8 are inert until the bootstrap callback is bound; their module/UI contracts can be removed independently.
- Tasks 5-6 can be reverted by removing the local trait/runtime bridge and the two-line vendored visibility patch; remote image protocols remain on their original parser path.
- Task 7 is the only product integration point for local display. Removing its binding/channel restores the pre-extension behavior without touching standard Paste.
- Never discard the pre-existing Windows clipboard repair, bounded preview, safe completion, or unrelated dirty-worktree changes unless the user explicitly requests it.
