# Top Status Bar Corner Background TDD Spec

Date: 2026-03-16

## Scope

This handoff covers the shell geometry ownership cleanup for the top status bar corner background issue:

- `AppWindow.shell-frame` remains the only owner of outer rounded window geometry
- `Titlebar` is permanently flattened into internal chrome
- `chrome-host` clips internal chrome to the outer shell geometry
- `RightPanel` is flattened into a square docked pane
- geometry diagnostics and smoke contracts now lock the final shell contract

## Code Surfaces

- `src/app/bootstrap.rs`
- `src/app/windowing.rs`
- `src/app/window_effects.rs`
- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `ui/shell/right-panel.slint`
- `tests/window_geometry_spec.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/top_status_bar_ui_contract_smoke.sh`
- `tests/shell_layout_ui_contract_smoke.sh`
- `tests/windows_frame_contract_smoke.sh`

## Core Rust Interfaces

### `WindowController<C>`

Location: `src/app/windowing.rs`

Public methods used by this shell path:

- `WindowController::new(component: &C) -> Self`
- `WindowController::minimize(&self)`
- `WindowController::toggle_maximize(&self, current: bool) -> bool`
- `WindowController::close(&self) -> anyhow::Result<()>`
- `WindowController::drag(&self) -> anyhow::Result<()>`
- `WindowController::drag_resize(&self, direction: WindowResizeDirection) -> anyhow::Result<()>`

Purpose:

- Bridges Slint titlebar/window callbacks into native window actions.
- Remains the control point for maximize, drag, and resize behavior that must not regress while shell chrome is flattened.

### `PlatformWindowEffects`

Location: `src/app/window_effects.rs`

Trait method:

```rust
fn apply_to_app_window(
    &self,
    window: &AppWindow,
    request: &NativeWindowAppearanceRequest,
) -> WindowAppearanceSyncReport;
```

Purpose:

- Applies theme and backdrop requests without owning geometry.
- Must stay compatible with the flattened internal chrome and rounded outer shell split.

### `bind_top_status_bar_with_store(...)`

Location: `src/app/bootstrap.rs`

Entry points:

- `bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>)`
- `bind_top_status_bar_with_store_and_effects(...)`
- `bind_top_status_bar_with_store_and_profile_and_effects(...)`

Responsibilities relevant to this feature:

- binds `toggle-right-panel-requested`
- binds `maximize-toggle-requested`
- binds `drag-double-clicked`
- binds `shell-layout-invalidated`
- syncs `use-flat-window-chrome`, `show-right-panel`, and layout-dependent shell state

## Slint Contracts

### `AppWindow`

Location: `ui/app-window.slint`

Geometry owners:

- `shell-frame := Rectangle`
- `chrome-host := Rectangle`
- `titlebar := Titlebar`
- `body-host := Rectangle`
- `right-panel := RightPanel`

Relevant exported properties:

- `layout-shell-frame-radius`
- `layout-titlebar-radius`
- `layout-titlebar-border-width`
- `layout-right-panel-radius`
- `layout-right-panel-border-width`

Contract:

- `shell-frame` owns the outer rounded geometry
- `chrome-host` mirrors `parent.border-radius` and uses `clip: true`
- `use-flat-window-chrome` only changes `shell-frame.border-radius`

### `Titlebar`

Location: `ui/shell/titlebar.slint`

Contract:

- `border-radius: 0px`
- `border-width: 0px`
- no `use-flat-window-chrome` input
- still emits drag, maximize, close, and utility callbacks

### `RightPanel`

Location: `ui/shell/right-panel.slint`

Contract:

- `border-radius: 0px`
- `border-width: 0px`
- `left-divider := Rectangle` provides the only explicit edge treatment
- `layout-radius` and `layout-border-width` expose geometry diagnostics for tests

## Slint Callback Coverage

Callbacks exercised or protected by this feature:

- `toggle-right-panel-requested()`
- `maximize-toggle-requested()`
- `drag-requested()`
- `drag-double-clicked()`
- `drag-resize-requested(string)`
- `shell-layout-invalidated(length, length)`

Why they matter:

- `toggle-right-panel-requested()` must not change panel geometry semantics back to a rounded card
- `maximize-toggle-requested()` and `drag-double-clicked()` must only flip outer shell geometry, not the internal titlebar chrome
- `drag-resize-requested(string)` and the Windows frame adapter must keep their current hit-test contract after the shell containment change

## Existing Automated Coverage

- `tests/window_geometry_spec.rs`
  - verifies outer shell radius
  - verifies titlebar flat geometry
  - verifies right panel flat geometry
  - verifies maximize button and resize budget exports
- `tests/top_status_bar_smoke.rs`
  - verifies maximize toggle only changes outer shell chrome
  - verifies top status bar bootstrap state binding remains consistent
- `tests/top_status_bar_ui_contract_smoke.sh`
  - verifies titlebar flat source contract
  - verifies `chrome-host` exists in `AppWindow`
- `tests/shell_layout_ui_contract_smoke.sh`
  - verifies shell containment host contract
  - verifies right panel docked pane contract
- `tests/windows_frame_contract_smoke.sh`
  - guards native frame adapter and maximize geometry contract from regressions

## Recommended Next TDD Targets

1. Add a rendering-oriented test that validates `chrome-host` clipping against restored-state rounded corners once Slint test support can observe non-rectangular clipping.
2. Add a focused integration test for `RightPanel` expand/collapse transitions so divider geometry and width transitions cannot drift.
3. Add a Windows-capable smoke path for restored / maximized / snapped screenshots to catch corner bleed regressions visually.
4. Add coverage for theme switching while `RightPanel` is expanded to confirm no border bleed reappears through color changes.

## Edge Cases And Risks

### Current geometry risks

- Restored state relies on `chrome-host` clipping to keep flat internal chrome inside rounded outer corners; any future removal of `clip: true` will reintroduce corner bleed.
- `RightPanel` now uses a single divider instead of a full border; future theme token changes can make the divider visually disappear if `ThemeTokens.shell-stroke` loses enough contrast.
- `shell-layout-invalidated` still drives responsive width collapse; future changes must preserve the current collapse order for assets sidebar and right panel.

### Platform interaction risks

- `WindowController::toggle_maximize(...)` and Windows state tracking must keep `use-flat-window-chrome` synchronized with actual placement state, or outer shell geometry can drift from native window state.
- `install_windows_frame_adapter(...)` and `tests/windows_frame_contract_smoke.sh` remain the guardrail for maximize hit-test geometry; any future titlebar refactor needs to rerun that contract first.

### Concurrency and async notes

- This feature did not introduce Tokio actors, channels, or cross-thread state sharing.
- Current shell geometry updates stay on the UI thread; if future work moves layout or window-state syncing off-thread, `slint::invoke_from_event_loop(...)` will be required to avoid UI-thread violations.
- No `ModelRc` state was introduced here. If a future design exposes shell diagnostics or panel state via shared models, tests should cover update ordering and stale model reads.

## Manual Validation Still Required

- restored window on Windows 11 shows only outer rounded corners
- no square background is visible behind the top-left and top-right corners in dark mode
- no square background is visible behind the top-left and top-right corners in light mode
- maximized and snapped windows keep both outer and inner chrome flat
- right panel appears as a docked square pane with a single divider
- drag zone, maximize button, and resize band remain behaviorally unchanged on a real desktop
