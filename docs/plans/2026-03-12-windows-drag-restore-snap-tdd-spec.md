# Windows Drag Restore Snap TDD Spec

Date: 2026-03-12
Status: implementation landed, Linux and Windows GNU automated verification passed, Windows runtime behavior still needs native execution evidence

## Purpose

This document is the handoff input for the next `test-driven-development` phase of the Windows drag restore / snap work.

The next phase should not redesign the solution. It should harden the implementation that now exists around:

- platform-agnostic window placement modeling
- shell chrome shape binding
- restore recovery state transitions
- Windows frame adapter contract points
- frameless resize contract

## Implemented Architecture Snapshot

- `WindowPlacementKind` is now the single semantic source of truth for shell window placement state.
- `ShellViewModel` no longer stores a standalone maximized boolean as the primary state source.
- Slint shell chrome now derives rounded vs. flat geometry from placement-derived state through `use-flat-window-chrome`.
- restore recovery logic was extracted from `bootstrap` into `WindowRecoveryController`.
- the old theme-only offscreen recovery path is now a general recovery controller used by both theme-triggered visibility recovery and maximize/restore transitions.
- Windows-specific frame integration is isolated into `src/app/windows_frame.rs`.
- `query_true_window_placement(...)` now reads real Win32 placement, window rect, and monitor work area before classifying `WindowPlacementKind`.
- `install_window_frame_adapter(...)` now installs a real subclass procedure that maps the exported maximize button geometry to `HTMAXBUTTON`.
- maximize button geometry is refreshed into the native frame adapter when shell layout invalidation runs.
- frameless resize capability is now explicitly declared through `resize-border-width: 6px` and `WindowCommandSpec.resize_border_width`.

## Core Files

- `src/app/window_state.rs`
- `src/app/window_recovery.rs`
- `src/app/windows_frame.rs`
- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/shell/view_model.rs`
- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `tests/window_state_spec.rs`
- `tests/window_recovery_spec.rs`
- `tests/shell_view_model.rs`
- `tests/window_geometry_spec.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/window_shell.rs`
- `tests/windows_frame_spec.rs`
- `tests/windows_frame_contract_smoke.sh`
- `tests/windows_drag_restore_contract_smoke.sh`

## Core Types And Interfaces

### `WindowPlacementKind`

Location: `src/app/window_state.rs`

Values:

- `Restored`
- `Maximized`
- `SnappedLeft`
- `SnappedRight`
- `SnappedTop`
- `SnappedBottom`
- `Unknown`

Relevant methods:

- `chrome_mode() -> WindowChromeMode`
- `is_maximized() -> bool`

Testing importance:

- future refactors must not reintroduce a second placement truth source
- `Restored` must continue to map to rounded chrome
- `Maximized` and `Snapped*` must continue to map to flat chrome

### `Rect` And `classify_window_placement(...)`

Location: `src/app/window_state.rs`

Purpose:

- express platform-neutral geometry
- classify work-area-aligned rects into restored vs. snapped placements

Testing importance:

- left/right snap classification is covered now
- top/bottom snap and partial monitor edge cases still need broader coverage

### `ShellViewModel`

Location: `src/shell/view_model.rs`

Relevant fields:

- `show_right_panel`
- `show_global_menu`
- `show_assets_sidebar`
- `active_sidebar_destination`
- `is_window_active`
- `theme_mode`
- `is_always_on_top`
- `window_placement`

Relevant methods:

- `window_placement()`
- `set_window_placement(...)`
- `is_window_maximized()`
- `uses_flat_window_chrome()`
- `toggle_theme_mode()`
- `toggle_always_on_top()`

Testing importance:

- maximize state must stay derived, not independently stored
- shell chrome flattening must continue to follow placement state

### `WindowRecoveryController`

Location: `src/app/window_recovery.rs`

Relevant methods:

- `arm_visibility_recovery(...)`
- `on_placement_changed(...)`
- `on_visibility_changed(...)`
- `on_resize_ack(...)`

Relevant actions:

- `None`
- `RequestRedraw`
- `NudgeWindowSize { width, height }`
- `RestoreWindowSize { width, height }`

Testing importance:

- restore transitions from `Maximized` or `Snapped*` into `Restored` must request redraw
- visibility growth should only produce a single size nudge / restore pair per armed cycle
- repeated nudge loops must not appear without a new transition or new armed recovery

### `CaptionButtonGeometry`

Location: `src/app/windows_frame.rs`

Fields:

- `x`
- `y`
- `width`
- `height`

Purpose:

- export the maximize button hot zone from Slint into the Windows adapter layer
- provide `contains_window_point(...)` so hit-test bounds stay unit-testable outside Win32

Testing importance:

- geometry contract must remain stable enough for `HTMAXBUTTON` integration work
- current contract exports a 36x36 hot zone, intentionally narrower than the full 46px button container

### Windows Frame Bridge

Location: `src/app/windows_frame.rs`

Relevant functions:

- `query_true_window_placement(...)`
- `install_window_frame_adapter(...)`
- `window_frame_subclass_proc(...)`

Purpose:

- translate the native `HWND` into true placement state via `GetWindowPlacement`, `GetWindowRect`, `MonitorFromWindow`, and `GetMonitorInfoW`
- install and update a `SetWindowSubclass` hook that can return `HTMAXBUTTON` for the exported maximize-button rect, but the hit-test is currently rolled back to disabled mode after runtime conflicts with the custom restore button
- keep the maximize-button geometry in `user32` window properties so the adapter does not depend on `GetWindowSubclass`
- native `HTMAXBUTTON` hit-testing is currently disabled for all placements; the subclass shell remains in place for a later Windows-only reintroduction
- release subclass state on `WM_NCDESTROY` so the stored geometry box does not leak

Testing importance:

- future refactors must preserve the actual Win32 state query path instead of falling back to UI-only placement guesses
- subclass updates must remain idempotent when layout invalidation re-installs the adapter with new geometry
- native maximize hit-testing is intentionally disabled right now because it steals restore-button input from Slint after maximize/restore cycles
- `WM_NCHITTEST` handling must not override non-client results that Windows already resolved to something other than `HTCLIENT`

### `WindowCommandSpec`

Location: `src/app/windowing.rs`

Relevant fields:

- `uses_winit_drag`
- `self_drawn_controls`
- `supports_double_click_maximize`
- `supports_always_on_top`
- `supports_true_window_state_tracking`
- `supports_native_frame_adapter`
- `resize_border_width`

Testing importance:

- this struct is now the high-level source contract for shell/window capability expectations
- future work should extend it deliberately instead of introducing duplicate capability booleans elsewhere

### `PlatformWindowEffects`

Location: `src/app/window_effects.rs`

Relevant trait method:

- `apply_to_app_window(...) -> WindowAppearanceSyncReport`

Testing importance:

- placement sync and recovery changes must not break the existing theme/backdrop synchronization path
- future Windows-specific rendering fixes still need to keep `request_redraw` and backdrop failure reporting intact

## Slint Callback Chain

### Maximize / Restore Path

1. `Titlebar.maximize-toggle-requested`
2. `AppWindow.maximize-toggle-requested`
3. `bootstrap` callback `window.on_maximize_toggle_requested(...)`
4. `WindowController::toggle_maximize(...)`
5. `ShellViewModel::set_window_placement(...)`
6. `notify_windows_window_recovery_transition(...)`
7. `sync_top_status_bar_state(...)`
8. `AppWindow.is-window-maximized` and `AppWindow.use-flat-window-chrome` update

### Double-Click Restore Path

1. `Titlebar.drag-double-clicked`
2. `AppWindow.drag-double-clicked`
3. `bootstrap` callback `window.on_drag_double_clicked(...)`
4. same placement/recovery path as maximize toggle

### Native Placement Sync Path

1. `winit` emits `WindowEvent::Moved`, `WindowEvent::Resized`, or `WindowEvent::ScaleFactorChanged`
2. `bootstrap` callback `bind_windows_window_recovery(...)` receives the event
3. `sync_windows_true_window_placement(...)` calls `query_true_window_placement(...)`
4. `ShellViewModel::set_window_placement(...)` updates the semantic placement state
5. `sync_top_status_bar_state(...)` updates `AppWindow.is-window-maximized` and `AppWindow.use-flat-window-chrome`
6. `notify_windows_window_recovery_transition_with_snapshot(...)` feeds the recovery controller with the same native snapshot

### Theme Toggle Recovery Arming Path

1. `Titlebar.toggle-theme-mode-requested`
2. `AppWindow.toggle-theme-mode-requested`
3. `bootstrap` callback `window.on_toggle_theme_mode_requested(...)`
4. `arm_windows_window_recovery(...)`
5. `ShellViewModel::toggle_theme_mode()`
6. `sync_theme_and_window_effects(...)`

### Frame Adapter Install Path

1. `bind_top_status_bar_with_store_and_effects(...)`
2. `sync_shell_state(...)`
3. `sync_shell_layout(...)`
4. `install_windows_frame_adapter(...)`
5. `CaptionButtonGeometry` is built from exported Slint maximize button geometry
6. later `AppWindow.shell-layout-invalidated(...)` re-runs `install_windows_frame_adapter(...)` to refresh the geometry after layout changes

## Existing Regression Coverage

### Rust tests already present

- `tests/window_state_spec.rs`
- `tests/window_recovery_spec.rs`
- `tests/shell_view_model.rs`
- `tests/window_geometry_spec.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/window_shell.rs`
- `tests/windows_frame_spec.rs`

Current assertions already cover:

- placement-to-chrome mapping
- left/right snap classification
- restore recovery redraw and nudge transitions
- `ShellViewModel` placement-derived chrome state
- rounded vs. flat shell/titlebar geometry export
- maximize button geometry export
- maximize button hit-test rect containment
- native maximize hit-test rollback flagging
- resize border export
- frame adapter capability contract

### Source contract smoke already present

- `tests/windows_frame_contract_smoke.sh`
- `tests/windows_drag_restore_contract_smoke.sh`

Current smoke assertions already cover source presence of:

- `WM_NCHITTEST`
- `HTMAXBUTTON`
- `SetWindowSubclass`
- `DefSubclassProc`
- `RemoveWindowSubclass`
- `GetPropW`
- `SetPropW`
- `RemovePropW`
- `WM_NCDESTROY`
- maximize button geometry export properties
- `resize-border-width: 6px;`
- `WindowRecoveryController` presence in `bootstrap`
- native placement sync hooks in `bootstrap`

## Required Next-Phase TDD Targets

### Unit tests to add first

- test `SnappedTop` and `SnappedBottom` classification on real work-area-aligned rects
- test `Unknown` and irregular-rect cases so `classify_window_placement(...)` does not silently overclassify
- test that non-recoverable placement transitions clear pending recovery state
- test that `on_resize_ack(...)` ignores unrelated resize events after the restore target changes

### Windows adapter tests to add

- test `query_true_window_placement(...)` through an extracted Win32 fixture seam so irregular monitor work areas can be replayed without a live `HWND`
- test maximize button geometry translation from Slint logical coords into native hit-test coords
- test subclass install lifecycle and geometry refresh on a real Windows runner if an abstraction seam is introduced

### Integration tests to add

- verify maximize toggle on the UI path also updates `use-flat-window-chrome` under repeated toggles
- verify theme toggle recovery arming does not force placement changes
- verify `resize-border-width` remains exported after shell/titlebar refactors

### Manual Windows validation to convert into repeatable checks

- maximize -> drag restore on Windows 11
- snapped left -> drag unsnap
- snapped right -> drag unsnap
- snapped top / bottom if supported by the active Windows version and monitor layout
- maximize hover / snap layout behavior if `HTMAXBUTTON` is reintroduced behind a Windows-only guard
- visual validation that restored chrome returns to rounded corners without titlebar clipping

## Edge Cases And Risks

### Restore recovery is still visibility-driven

- current recovery fallback still depends on visible-area growth and size nudge behavior
- this is intentional as a fallback, even though placement truth is now sourced from Win32

### `windows_frame.rs` now has real Win32 hooks, but native coverage is still thin

- `query_true_window_placement(...)` now executes real Win32 calls, but its edge coverage is only compile-tested on Linux and Windows GNU targets
- `install_window_frame_adapter(...)` now owns native subclass state and updates it on repeat installs through `user32` window properties
- there is still no automated Windows runner proving `WM_NCHITTEST` behavior end-to-end on an actual desktop session
- the adapter currently trades native snap-layout hover away completely so the custom maximize/restore button remains clickable

### UI-thread-only state is an intentional invariant

- current state is held under `Rc<RefCell<...>>` and accessed from the Slint/UI thread
- no Tokio channel or cross-thread mutation was introduced in this round
- if future Windows subclass callbacks or background tasks cross threads, they must marshal back with `slint::invoke_from_event_loop(...)` before mutating UI state

### Geometry contracts are now externally depended upon

- `layout-titlebar-maximize-button-*`
- `layout-resize-border-width`
- `use-flat-window-chrome`

These properties are now part of the internal integration contract between Slint UI and Rust window orchestration. Later UI cleanup must preserve them or deliberately migrate all consumers together.

### Native subclass memory ownership is now a risk surface

- `install_window_frame_adapter(...)` stores `CaptionButtonGeometry` in `dwrefdata`
- `window_frame_subclass_proc(...)` frees that box on `WM_NCDESTROY`
- future changes must not introduce double-install / double-free paths or move this state across threads

## Verification Evidence Captured In This Phase

Commands to re-run after the current runtime fixes:

- `cargo test --test window_state_spec --test window_recovery_spec --test shell_view_model --test window_geometry_spec --test top_status_bar_smoke --test window_shell --test windows_frame_spec -q`
- `bash tests/windows_frame_contract_smoke.sh`
- `bash tests/windows_drag_restore_contract_smoke.sh`
- `cargo check --workspace`
- `cargo check --workspace --target x86_64-pc-windows-gnu`
- `cargo clippy --workspace -- -D warnings`
- `cargo clippy --workspace --all-targets -- -D warnings`
