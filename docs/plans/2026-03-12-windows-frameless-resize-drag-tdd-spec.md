# Windows Frameless Resize Drag TDD Spec

Date: 2026-03-12
Status: implementation landed, targeted automated verification passed, Windows 11 runtime interaction still requires native execution evidence

## Purpose

This document is the handoff input for the next `test-driven-development` phase of the Windows frameless resize / drag work.

The next phase should not redesign the solution. It should harden the implementation that now exists around:

- minimum window size as a real shell contract
- explicit edge / corner drag-resize entry points
- Windows frame adapter boundaries versus resize grips
- titlebar drag start timing after maximize / restore

## Implemented Architecture Snapshot

- `ShellMetrics::WINDOW_MIN_WIDTH` and `ShellMetrics::WINDOW_MIN_HEIGHT` are now exposed through `WindowCommandSpec` and mirrored in `AppWindow` via `min-width` / `min-height`.
- `WindowCommandSpec` now explicitly declares both `uses_winit_drag` and `uses_winit_drag_resize`.
- `WindowResizeDirection` and `parse_resize_direction(...)` provide a Rust-side semantic bridge from Slint string callbacks into `winit::window::ResizeDirection`.
- `WindowController::drag_resize(...)` is now the single runtime entry point for explicit frameless resize requests.
- `WindowResizeGrips` adds eight invisible resize hotspots for north / south / east / west and all four corners.
- `AppWindow.drag-resize-requested(string)` forwards grip events into `bootstrap`, where they are parsed and dispatched to `WindowController::drag_resize(...)`.
- `windows_frame.rs` remains a medium Windows-only adapter. It still does not take over the full non-client frame, but it now reserves an outer resize band so maximize-button hit-testing does not steal edge resize input.
- `Titlebar.drag-touch` now starts drag on `pointer-event(down)` with the left mouse button, while `double-clicked` remains the maximize / restore route.

## Core Files

- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/app/windows_frame.rs`
- `src/shell/metrics.rs`
- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `ui/components/window-resize-grips.slint`
- `tests/window_shell.rs`
- `tests/window_resize_direction_spec.rs`
- `tests/window_geometry_spec.rs`
- `tests/windows_frame_spec.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/window_resize_drag_contract_smoke.sh`
- `tests/windows_frame_contract_smoke.sh`
- `tests/windows_drag_restore_contract_smoke.sh`
- `tests/top_status_bar_ui_contract_smoke.sh`
- `verification.md`

## Core Types And Interfaces

### `WindowCommandSpec`

Location: `src/app/windowing.rs`

Relevant fields:

- `uses_winit_drag`
- `uses_winit_drag_resize`
- `resize_border_width`
- `min_window_width`
- `min_window_height`

Testing importance:

- future shell changes must keep drag and drag-resize capability explicit instead of reintroducing hidden assumptions
- minimum size budget must remain aligned with `ShellMetrics`
- `resize_border_width` must stay exported for geometry regression coverage

### `WindowResizeDirection`

Location: `src/app/windowing.rs`

Values:

- `North`
- `South`
- `East`
- `West`
- `NorthEast`
- `NorthWest`
- `SouthEast`
- `SouthWest`

Related function:

- `parse_resize_direction(&str) -> Option<WindowResizeDirection>`

Testing importance:

- every Slint string direction must continue to map to exactly one `winit` resize direction
- unknown strings must continue to fail closed as `None`

### `WindowController`

Location: `src/app/windowing.rs`

Relevant methods:

- `drag()`
- `drag_resize(direction)`
- `toggle_maximize(current)`

Testing importance:

- drag and drag-resize must remain separate entry points
- future refactors must not fold resize into the titlebar drag path
- error handling must continue to fail gracefully when the `winit` window is unavailable

### `CaptionButtonGeometry`

Location: `src/app/windows_frame.rs`

Purpose:

- export maximize-button geometry from Slint into the Windows adapter layer
- keep maximize-button hit-testing unit-testable outside Win32

### `point_hits_outer_resize_band(...)`

Location: `src/app/windows_frame.rs`

Purpose:

- define the reserved outer band that belongs to explicit resize grips
- prevent `WM_NCHITTEST` maximize hit-testing from overriding outer-edge resize interaction

Testing importance:

- corners and edges must stay reserved before maximize-button logic runs
- if grip size or adapter band changes later, tests should force both layers to stay in sync

### `WindowResizeGrips`

Location: `ui/components/window-resize-grips.slint`

Input / callback contract:

- `grip-size: length`
- `resize-requested(string)`

Behavior:

- every edge and corner uses `pointer-event(down)` with left mouse button only
- every hotspot emits one stable string direction
- every hotspot exposes the matching resize cursor

Testing importance:

- future UI refactors must not accidentally cover, shrink, or remove a grip zone
- right-bottom corner must continue to emit a corner direction, not a single-axis fallback

## Slint Callback Chain

### Explicit Resize Path

1. `WindowResizeGrips.resize-requested(direction)`
2. `AppWindow.drag-resize-requested(direction)`
3. `bootstrap` callback `window.on_drag_resize_requested(...)`
4. `parse_resize_direction(direction.as_str())`
5. `WindowController::drag_resize(...)`
6. `winit::window::Window::drag_resize_window(...)`

### Titlebar Drag Path

1. `Titlebar.drag-touch.pointer-event(event)`
2. `Titlebar.drag-requested`
3. `AppWindow.drag-requested`
4. `bootstrap` callback `window.on_drag_requested(...)`
5. `WindowController::drag()`
6. `winit::window::Window::drag_window()`

### Double-Click Maximize / Restore Path

1. `Titlebar.drag-touch.double-clicked`
2. `Titlebar.drag-double-clicked`
3. `AppWindow.drag-double-clicked`
4. `bootstrap` callback `window.on_drag_double_clicked(...)`
5. `WindowController::toggle_maximize(...)`
6. `ShellViewModel::set_window_placement(...)`
7. `notify_windows_window_recovery_transition(...)`
8. `sync_top_status_bar_state(...)`

### Windows Frame Adapter Refresh Path

1. `AppWindow.shell-layout-invalidated(...)`
2. `bootstrap` callback `window.on_shell_layout_invalidated(...)`
3. `install_windows_frame_adapter(...)`
4. `install_window_frame_adapter(...)`
5. `WindowFrameState` refreshes maximize-button geometry and `reserved_resize_band`

## Regression Themes For The Next TDD Phase

### Rust Contract Tests

- keep `WindowCommandSpec.min_window_width` and `min_window_height` aligned with `ShellMetrics`
- keep `uses_winit_drag_resize` exported as a first-class shell capability
- extend `parse_resize_direction(...)` tests with duplicate / malformed / mixed-case inputs if string sources expand later
- test `point_hits_outer_resize_band(...)` around exact boundary values such as `band - 1`, `band`, and `window_size - band`

### Slint Contract Tests

- verify `AppWindow` still declares `min-width: 688px` and `min-height: 640px`
- verify `WindowResizeGrips` still exports all eight directions
- verify `Titlebar.drag-touch` continues to use `pointer-event(down)` and does not regress to `moved`

### Windows Manual Behavior

- restored state: top / bottom / left / right resize
- restored state: four-corner resize, especially `south-east`
- minimum-size floor: repeated shrink attempts must stop at `688 x 640`
- maximize then titlebar drag: restore and continue drag without dead clicks
- maximize button then titlebar drag: restore path must remain stable
- snap / unsnap: edge resize and titlebar drag must both remain available

## Edge Cases And Risks

- `WindowResizeGrips.grip-size` is currently `10px`, while `WindowCommandSpec.resize_border_width` remains `6`. This is intentional today, but it creates two geometry budgets that future tests should keep explicitly documented.
- `WINDOW_FRAME_RESERVED_RESIZE_BAND` is currently hard-coded to `10`, matching the grip layer instead of the fallback `resize-border-width`. If either side changes later, Windows adapter tests must fail until both stay aligned.
- titlebar `pointer-event(down)` and `double-clicked` now coexist. Future Windows-native testing should watch for input ordering regressions where double-click maximize could accidentally trigger an unwanted drag start.
- explicit grips sit above the shell frame as a transparent overlay. Future overlays or popups near the window edge must not steal pointer input from those hotspots.
- this feature does not add any new Tokio task, channel, actor mailbox, mutex, or cross-thread shared state. There is no new async data race surface in the current implementation.
- because there is no new Tokio communication path, the main concurrency risk remains UI event ordering between Slint input callbacks and `winit` window state changes, not channel blockage or thread contention.

## Traits And Async Boundary Notes

- No new trait was introduced for this feature.
- Existing `PlatformWindowEffects` integration remains unchanged and should continue to be treated as an orthogonal concern.
- No new `tokio::spawn`, channel send/recv, `slint::invoke_from_event_loop`, `ModelRc`, or actor message bridge was added by this work.

## Final Verification Record

Passed in this implementation round:

- `cargo fmt --check -- src/app/bootstrap.rs src/app/windowing.rs src/app/windows_frame.rs tests/top_status_bar_smoke.rs tests/window_shell.rs tests/windows_frame_spec.rs tests/window_resize_direction_spec.rs`
- `cargo test --test window_shell --test window_resize_direction_spec --test windows_frame_spec --test window_geometry_spec --test top_status_bar_smoke -q`
- `cargo check --workspace`
- `cargo clippy --workspace -- -D warnings`
- `bash tests/window_resize_drag_contract_smoke.sh`
- `bash tests/windows_frame_contract_smoke.sh`
- `bash tests/windows_drag_restore_contract_smoke.sh`
- `bash tests/top_status_bar_ui_contract_smoke.sh`

Manual follow-up still required:

- Windows 11 real-machine resize behavior
- maximize / restore / snap interaction timing
- titlebar drag behavior after maximize transitions
