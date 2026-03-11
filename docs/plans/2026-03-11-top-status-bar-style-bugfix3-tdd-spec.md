# Top Status Bar Style Bugfix3 TDD Spec

Date: 2026-03-11

## Scope

This handoff covers the tooltip stabilization work for the top status bar:

- Tooltip rendering moved from `PopupWindow` to an in-window overlay
- `Titlebar` owns the shared tooltip state machine
- Tooltip lifecycle events are bridged into Rust and written to `logs/titlebar-tooltip.log`
- All titlebar buttons continue to use Fluent SVG assets

## Code Surfaces

- `src/app/tooltip_debug_log.rs`
- `src/app/bootstrap.rs`
- `src/shell/metrics.rs`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/shell/titlebar.slint`
- `ui/app-window.slint`
- `tests/tooltip_debug_log.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/titlebar_layout_spec.rs`
- `tests/top_status_bar_ui_contract_smoke.sh`

## Core Rust Interfaces

### `TooltipDebugEvent<'a>`

Location: `src/app/tooltip_debug_log.rs`

Fields:

- `phase: &'a str`
- `source_id: &'a str`
- `text: &'a str`
- `anchor_x: f32`
- `anchor_y: f32`

Purpose:

- Carries one tooltip lifecycle event from the UI bridge to the file logger.

### `TooltipDebugLog`

Location: `src/app/tooltip_debug_log.rs`

Public API:

- `TooltipDebugLog::in_directory(directory: PathBuf) -> anyhow::Result<Self>`
- `TooltipDebugLog::for_current_dir() -> anyhow::Result<Self>`
- `TooltipDebugLog::log_path(&self) -> PathBuf`
- `TooltipDebugLog::append(&self, event: TooltipDebugEvent<'_>) -> anyhow::Result<()>`

Purpose:

- Creates `logs/` on demand and appends one line per tooltip lifecycle event.

### `bind_top_status_bar_with_store_and_log_dir`

Location: `src/app/bootstrap.rs`

Signature:

```rust
pub fn bind_top_status_bar_with_store_and_log_dir(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    log_root: Option<PathBuf>,
)
```

Purpose:

- Keeps the existing titlebar state binding behavior
- Initializes the optional tooltip debug logger
- Registers `window.on_tooltip_debug_event_requested(...)`
- Downgrades logger init/write failures to `eprintln!`

Compatibility:

- `bind_top_status_bar_with_store(...)` now delegates to this function
- `bind_top_status_bar(...)` still remains the public default entry point

## Slint Callback Contract

### Button-level intent callbacks

Locations:

- `ui/components/titlebar-icon-button.slint`
- `ui/components/window-control-button.slint`

Callbacks:

```slint
callback tooltip-open-requested(string, string, length, length);
callback tooltip-close-requested(string);
```

Argument meaning:

- `string #1`: `tooltip-source-id`
- `string #2`: tooltip text
- `length #1`: anchor x
- `length #2`: anchor y

### Titlebar debug bridge callback

Locations:

- `ui/shell/titlebar.slint`
- `ui/app-window.slint`

Callback:

```slint
callback tooltip-debug-event-requested(string, string, string, length, length);
```

Argument meaning:

- `string #1`: source id
- `string #2`: phase (`schedule-tooltip`, `show-tooltip`, `close-tooltip`)
- `string #3`: tooltip text
- `length #1`: anchor x in titlebar-local coordinates
- `length #2`: anchor y in titlebar-local coordinates

## Shared Tooltip State Machine

Location: `ui/shell/titlebar.slint`

State fields:

- `tooltip-text-value`
- `tooltip-source-id-value`
- `tooltip-anchor-x-value`
- `tooltip-anchor-y-value`
- `tooltip-visible-value`
- `tooltip-delay := Timer { interval: 280ms }`

Behavior:

1. Hover enter from a button emits `tooltip-open-requested(...)`
2. `Titlebar.schedule-tooltip(...)` stores source/text/anchor, emits `schedule-tooltip`, hides the overlay, and restarts the timer
3. Timer completion emits `show-tooltip` and sets `tooltip-visible-value = true`
4. Hover leave, click, or menu-open flows through `close-tooltip(source-id)` and emits `close-tooltip`
5. Overlay rendering is handled by a single `tooltip-overlay := TitlebarTooltip`

## Existing Automated Coverage

- `tests/tooltip_debug_log.rs`
  - Verifies log directory creation and event append behavior
- `tests/top_status_bar_smoke.rs`
  - Verifies bootstrap binding still syncs top status bar state
  - Verifies tooltip debug callback writes a real log file
- `tests/titlebar_layout_spec.rs`
  - Verifies tooltip budget constants (`delay`, `offset`, `min width`)
- `tests/top_status_bar_ui_contract_smoke.sh`
  - Verifies overlay contract exists in source and `PopupWindow` usage is gone

## Recommended Next TDD Targets

1. Add a focused test around logger fallback behavior when the log directory cannot be created.
2. Add a source-level smoke assertion for emitted tooltip phases so the bridge contract cannot silently drift.
3. Add a GUI-capable smoke path on Windows 11 for:
   - hover on each top status bar button
   - rapid hover switching between adjacent buttons
   - opening `global-menu` while tooltip is visible
   - verifying `logs/titlebar-tooltip.log` event order
4. If Slint test support allows it later, add a UI integration test asserting the overlay x-clamp logic against narrow window widths.

## Edge Cases And Risks

### Current implementation risks

- Fast hover switching can emit a stale close from the previous button; current mitigation is the `source-id` gate in `close-tooltip(source-id)`.
- Overlay x-position is clamped against `host-width`; narrow widths need GUI coverage to confirm no visual clipping.
- File logging is synchronous; frequent hover activity can create noisy logs, but it does not block startup because init/write failures only downgrade to `eprintln!`.

### Explicit non-risks in the current implementation

- No Tokio actor, channel, or background task was introduced for this feature.
- No `ModelRc` state sharing was introduced for this tooltip path.
- No new trait abstraction was introduced; the change is callback-based and UI-thread-local.

### Future risks if the logging path becomes async later

- Channel backpressure could delay tooltip lifecycle recording.
- Cross-thread logging would require careful `slint::invoke_from_event_loop` boundaries.
- Shared logger mutation would need explicit synchronization to avoid data races.

## Manual Validation Still Required

The following items are still outside automated coverage and should be the first manual TDD follow-up on a Windows 11 desktop session:

- Tooltip appears below each button with stable positioning
- Tooltip disappears immediately on button leave and click
- Tooltip closes when `global-menu` opens
- No tooltip residue remains after fast horizontal sweeps
- `logs/titlebar-tooltip.log` contains matching source ids and visible phase order
