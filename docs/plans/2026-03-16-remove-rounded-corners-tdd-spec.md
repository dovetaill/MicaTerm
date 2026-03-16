# Remove Rounded Corners TDD Spec

Date: 2026-03-16
Scope: shell chrome square geometry, Windows native corner preference, rounded-corner contract cleanup
Status: implementation complete, ready for test-driven follow-up

## Source Inputs

- Design: `docs/plans/2026-03-16-remove-rounded-corners-design.md`
- Implementation Plan: `docs/plans/2026-03-16-remove-rounded-corners-implementation-plan.md`
- Verification: `verification.md` -> `2026-03-16 Remove Rounded Corners`

## Core Rust Surfaces

### `src/app/window_state.rs`

- `WindowPlacementKind`
  - Variants:
    - `Restored`
    - `Maximized`
    - `SnappedLeft`
    - `SnappedRight`
    - `SnappedTop`
    - `SnappedBottom`
    - `Unknown`
  - Method:
    - `is_maximized(self) -> bool`
- `Rect`
  - Fields:
    - `x: i32`
    - `y: i32`
    - `width: u32`
    - `height: u32`
  - Method:
    - `new(x, y, width, height) -> Rect`
- `classify_window_placement(window_rect, work_area, maximized) -> WindowPlacementKind`
- Removed surface:
  - `WindowChromeMode`
  - `chrome_mode()`

### `src/shell/view_model.rs`

- `ShellViewModel`
  - Relevant window-state methods:
    - `window_placement() -> WindowPlacementKind`
    - `set_window_placement(WindowPlacementKind)`
    - `is_window_maximized() -> bool`
  - No flat/rounded chrome state remains in the view model.
- Existing UI state methods for titlebar/sidebar remain unchanged.

### `src/app/window_effects.rs`

- `NativeWindowTheme`
  - `Dark`
  - `Light`
- `BackdropPreference`
  - `None`
  - `Mica`
  - `MicaAlt`
- `NativeWindowCornerPreference`
  - `Default`
  - `DoNotRound`
- `NativeWindowAppearanceRequest`
  - Fields:
    - `theme: NativeWindowTheme`
    - `backdrop: BackdropPreference`
    - `corner_preference: NativeWindowCornerPreference`
    - `request_redraw: bool`
- `build_native_window_appearance_request(mode, appearance) -> NativeWindowAppearanceRequest`
  - Current contract always returns `corner_preference: DoNotRound`
- `PlatformWindowEffects`
  - `apply_to_app_window(&AppWindow, &NativeWindowAppearanceRequest) -> WindowAppearanceSyncReport`
- Windows-specific path:
  - `WindowsWindowEffects::apply_to_app_window()`
  - Calls:
    - `window.set_theme(...)`
    - `window.set_corner_preference(CornerPreference::DoNotRound | Default)`
    - `window_vibrancy::apply_tabbed(...)` / `apply_mica(...)`
    - `window.request_redraw()`

### `src/app/bootstrap.rs`

- `sync_top_status_bar_state(window, state, effects)`
  - Still syncs:
    - `show_right_panel`
    - `show_global_menu`
    - `is_window_maximized`
    - `is_window_active`
    - `is_window_always_on_top`
  - No longer syncs any flat/rounded chrome boolean into Slint.
- `bind_top_status_bar_with_store_and_effects(...)`
  - Existing appearance-sync path now propagates `corner_preference` through `PlatformWindowEffects`.

## Core Slint Surfaces

### `ui/app-window.slint`

- Removed property:
  - `use-flat-window-chrome`
- Geometry contract:
  - `shell-frame.border-radius: 0px`
  - `chrome-host.border-radius: 0px`
- Existing exported diagnostics still relevant:
  - `layout-shell-frame-radius`
  - `layout-titlebar-radius`
  - `layout-right-panel-radius`
  - `layout-resize-border-width`
- Existing callbacks remain the interaction backbone:
  - `drag-requested()`
  - `drag-double-clicked()`
  - `maximize-toggle-requested()`
  - `toggle-right-panel-requested()`
  - `toggle-global-menu-requested()`
  - `toggle-theme-mode-requested()`
  - `toggle-window-always-on-top-requested()`
  - `shell-layout-invalidated(length, length)`

### Shared square-geometry components

The following surfaces now carry a literal `border-radius: 0px;` contract and should stay square unless design direction changes explicitly:

- `ui/components/status-pill.slint`
- `ui/components/assets-create-menu.slint`
- `ui/components/active-tab.slint`
- `ui/components/segmented-control.slint`
- `ui/components/command-entry.slint`
- `ui/components/titlebar-icon-button.slint`
- `ui/components/sidebar-nav-button.slint`
- `ui/components/command-palette.slint`
- `ui/components/titlebar-tooltip.slint`
- `ui/components/sidebar-toolbar-icon-button.slint`
- `ui/components/titlebar-menu.slint`
- `ui/shell/assets-sidebar.slint`
- `ui/shell/titlebar.slint`

## Existing Automated Coverage

- Rust:
  - `tests/window_state_spec.rs`
  - `tests/shell_view_model.rs`
  - `tests/window_effects.rs`
  - `tests/window_geometry_spec.rs`
  - `tests/top_status_bar_smoke.rs`
- Shell smoke:
  - `tests/window_chrome_contract_smoke.sh`
  - `tests/square_component_contract_smoke.sh`
  - `tests/top_status_bar_ui_contract_smoke.sh`
  - `tests/window_theme_contract_smoke.sh`

## Required Next-Stage TDD Focus

### 1. Window placement classification edges

- Verify `classify_window_placement()` under odd work-area dimensions where half-width / half-height truncation can affect snap detection.
- Add coverage for `Unknown` / unexpected rectangles if downstream code begins depending on that variant.
- Confirm maximize / restore / snap transitions never reintroduce geometry-dependent UI branching.

### 2. Native corner preference synchronization

- Add regression coverage for repeated theme toggles so each request keeps `corner_preference: DoNotRound`.
- On Windows-capable automation, verify `set_corner_preference()` is invoked alongside theme/backdrop changes for both dark and light flows.
- Preserve non-Windows behavior as a no-op path through `NoopWindowEffects`.

### 3. Square geometry visual regressions

- Extend UI tests or golden/screenshot checks for:
  - titlebar hover and divider readability
  - sidebar and command surface selected state visibility
  - tooltip / menu visual clarity after radius removal
- Keep future refactors from reintroducing non-zero radius via theme tokens or derived properties.

### 4. Contract cleanup stability

- Keep production-code smoke coverage focused on the removed symbols:
  - `WindowChromeMode`
  - `chrome_mode(`
  - `uses_flat_window_chrome`
  - `use-flat-window-chrome`
- If future refactors rename geometry fields, update smoke tests deliberately rather than weakening them.

## Edge Cases And Risks

- `CornerPreference::DoNotRound` behavior can still vary across Windows 11 builds; Linux CI cannot prove real HWND-level rendering.
- Current square-component smoke is literal-text based; if future Slint code generates radius indirectly, the smoke contract may need a parser-aware replacement.
- No new Tokio task, actor mailbox, or channel was introduced in this feature.
  - If future async code updates window placement, theme, or native appearance from background workers, it must marshal UI mutations through `slint::invoke_from_event_loop`.
  - Background threads must not touch Slint component state directly.
  - If actor/channel updates are introduced later, test for queue backpressure, stale request ordering, and data-race avoidance.
- Current feature does not change `ModelRc` ownership patterns; if geometry/theme state ever becomes model-driven, keep Slint model updates on the UI thread.

## Suggested Next Verification Commands

```bash
bash tests/window_chrome_contract_smoke.sh
bash tests/square_component_contract_smoke.sh
bash tests/top_status_bar_ui_contract_smoke.sh
bash tests/window_theme_contract_smoke.sh
cargo test --test window_state_spec --test shell_view_model --test window_effects --test window_geometry_spec --test top_status_bar_smoke -q
cargo check --workspace
cargo clippy --workspace -- -D warnings
```
