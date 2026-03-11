# Top Status Bar TDD Handoff

## Scope

This document captures the implemented top status bar baseline in the `Rust + Slint` shell. It is the handoff input for the next `test-driven-development` phase.

## Core Rust API Surface

### App bootstrap

- `mica_term::app::bootstrap::app_title() -> &'static str`
- `mica_term::app::bootstrap::default_window_size() -> (u32, u32)`
- `mica_term::app::bootstrap::bind_top_status_bar(window: &AppWindow)`
- `mica_term::app::bootstrap::run() -> anyhow::Result<()>`

### Shell state

- `ShellViewModel`
  - `show_welcome: bool`
  - `show_right_panel: bool`
  - `show_settings_menu: bool`
  - `is_window_maximized: bool`
  - `is_window_active: bool`
- `ShellViewModel::toggle_right_panel()`
- `ShellViewModel::toggle_settings_menu()`
- `ShellViewModel::close_settings_menu()`
- `ShellViewModel::set_window_maximized(value: bool)`
- `ShellViewModel::set_window_active(value: bool)`

### Windowing

- `MaterialKind`
  - `MicaAlt`
- `WindowAppearance`
  - `no_frame: bool`
  - `material: MaterialKind`
- `window_appearance() -> WindowAppearance`
- `WindowCommandSpec`
  - `uses_winit_drag: bool`
  - `self_drawn_controls: bool`
  - `supports_double_click_maximize: bool`
- `window_command_spec() -> WindowCommandSpec`
- `next_maximize_state(is_maximized: bool) -> bool`
- `WindowController<C: ComponentHandle>`
  - Holds `slint::Weak<C>` instead of owning `slint::Window`
  - `new(component: &C) -> Self`
  - `minimize()`
  - `toggle_maximize(current: bool) -> bool`
  - `close() -> anyhow::Result<()>`
  - `drag() -> anyhow::Result<()>`

### Shell metrics

- `ShellMetrics`
  - `TITLEBAR_HEIGHT`
  - `TITLEBAR_ACTIONS_WIDTH`
  - `TITLEBAR_WINDOW_CONTROL_WIDTH`
  - `TITLEBAR_MIN_DRAG_WIDTH`
  - `ACTIVITY_BAR_WIDTH`
  - `ASSETS_SIDEBAR_WIDTH`
  - `TAB_BAR_HEIGHT`
  - `RIGHT_PANEL_WIDTH`
  - `BASE_SPACING`

## Slint Components

### Root window contract

- `AppWindow`
  - Frameless root shell window
  - `in-out property <bool> show-right-panel`
  - `in-out property <bool> show-settings-menu`
  - `in-out property <bool> is-window-maximized`
  - `in-out property <bool> is-window-active`
  - Callback surface:
    - `drag-requested()`
    - `drag-double-clicked()`
    - `minimize-requested()`
    - `maximize-toggle-requested()`
    - `close-requested()`
    - `toggle-right-panel-requested()`
    - `toggle-settings-menu-requested()`
    - `close-settings-menu-requested()`

### Title bar contract

- `Titlebar`
  - Five-zone layout:
    - Brand zone
    - Main drag zone
    - Actions zone
    - Minimum drag safety zone
    - Window controls zone
  - Reads:
    - `show-right-panel`
    - `show-settings-menu`
    - `is-window-maximized`
    - `is-window-active`
  - Emits:
    - `drag-requested`
    - `drag-double-clicked`
    - `toggle-right-panel-requested`
    - `toggle-settings-menu-requested`
    - `close-settings-menu-requested`
    - `minimize-requested`
    - `maximize-toggle-requested`
    - `close-requested`

### New title bar sub-components

- `TitlebarIconButton`
  - Small chrome action button for menu and panel toggle
- `WindowControlButton`
  - Self-drawn minimize / maximize / close control
- `TitlebarMenu`
  - PopupWindow-based top bar menu
  - Emits `settings-selected`, `appearance-selected`, `close-requested`
- `RightPanel`
  - Adds `expanded: bool`
  - Width collapses to `0px` when hidden

## Current Binding Strategy

- Slint owns structure, visual states, popup presentation, and emits intent callbacks.
- `ShellViewModel` owns shell state values used by the title bar.
- `bind_top_status_bar()` allocates `Rc<RefCell<ShellViewModel>>` and registers all `AppWindow` callback handlers.
- `WindowController` isolates window minimize / maximize / close / drag behavior and keeps `winit` access inside `windowing`.
- Double-click maximize uses the same maximize toggle path as the window control button.

## Current Testing Baseline

- `tests/shell_view_model.rs`
  - Verifies top status bar state defaults and toggle/set methods.
- `tests/window_shell.rs`
  - Verifies frameless Mica route and approved window command strategy.
- `tests/titlebar_layout_spec.rs`
  - Verifies layout budget constants for the new title bar.
- `tests/top_status_bar_smoke.rs`
  - Verifies `bind_top_status_bar()` wires `invoke_*` callbacks to window state transitions.

## Verification Evidence

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -q`

Desktop smoke result in this environment:

- `cargo run` does not start because the current Linux session has no `WAYLAND_DISPLAY`, `WAYLAND_SOCKET`, or `DISPLAY`.
- Real drag / native caption behavior still requires a GUI-capable desktop session.

## Edge Cases and Risks To Cover Next

- `WindowController::drag()` depends on a live `winit` window. In headless tests or before the window backend is fully active, it returns an error path that is currently ignored by callbacks.
- `bind_top_status_bar()` uses `Rc<RefCell<ShellViewModel>>`. If future async updates arrive from Tokio tasks, UI mutations must be marshaled back with `slint::invoke_from_event_loop` instead of touching state directly from worker threads.
- `TitlebarMenu` currently closes through callback and popup close policy, but there is no dedicated assertion yet for click-outside and `Esc` behavior.
- `RightPanel` collapses by width and visibility, but there is no runtime visual assertion yet that narrow window layouts still preserve the minimum drag safety zone.
- `is_window_active` is wired as a state field and visual opacity input, but no platform event source updates it yet.
- `show_welcome` still exists in `ShellViewModel`, but the welcome/terminal content switch is not connected to the new top bar.
- Tokio channel backpressure, actor mailbox saturation, and shutdown ordering are not implemented yet; when they are introduced, tests must cover blocked senders, dropped receivers, and UI-thread handoff order.

## Recommended Next TDD Targets

1. Add deterministic tests for `toggle-right-panel-requested` changing both root property state and right-panel visibility contract.
2. Add popup interaction tests for `toggle-settings-menu-requested`, outside click close, and `Esc` close behavior.
3. Add a GUI-capable smoke test path for `drag-requested` and double-click maximize on a real backend.
4. Add tests for active/inactive window transitions once platform window focus events are wired into `ShellViewModel`.
5. Add integration tests for future Tokio-driven shell state updates using `slint::invoke_from_event_loop`.
