# Top Status Bar Style Bugfix TDD Handoff

## Scope

This document captures the implemented 2026-03-11 top status bar style bugfix baseline for the `Rust + Slint` shell. It is the handoff input for the next `test-driven-development` phase.

## Implemented Surface

### Core Rust structs and functions

- `mica_term::shell::view_model::ShellViewModel`
  - `show_welcome: bool`
  - `show_right_panel: bool`
  - `show_global_menu: bool`
  - `is_window_maximized: bool`
  - `is_window_active: bool`
- `ShellViewModel::toggle_right_panel()`
- `ShellViewModel::toggle_global_menu()`
- `ShellViewModel::close_global_menu()`
- `ShellViewModel::set_window_maximized(value: bool)`
- `ShellViewModel::set_window_active(value: bool)`
- `mica_term::app::bootstrap::bind_top_status_bar(window: &AppWindow)`
- `mica_term::app::bootstrap::run() -> anyhow::Result<()>`
- `mica_term::app::bootstrap::app_title() -> &'static str`
- `mica_term::app::bootstrap::default_window_size() -> (u32, u32)`
- `mica_term::shell::metrics::ShellMetrics`
  - `TITLEBAR_HEIGHT`
  - `TITLEBAR_BRAND_WIDTH`
  - `TITLEBAR_UTILITY_WIDTH`
  - `TITLEBAR_WINDOW_CONTROL_WIDTH`
  - `TITLEBAR_MIN_DRAG_WIDTH`
  - `ACTIVITY_BAR_WIDTH`
  - `ASSETS_SIDEBAR_WIDTH`
  - `TAB_BAR_HEIGHT`
  - `RIGHT_PANEL_WIDTH`
  - `BASE_SPACING`

### Windowing interfaces

- No new trait was introduced by this bugfix.
- `mica_term::app::windowing::WindowController<C: ComponentHandle>`
  - Keeps a `slint::Weak<C>` handle instead of owning the window directly.
  - `new(component: &C) -> Self`
  - `minimize()`
  - `toggle_maximize(current: bool) -> bool`
  - `close() -> anyhow::Result<()>`
  - `drag() -> anyhow::Result<()>`
- `window_appearance() -> WindowAppearance`
  - `no_frame: true`
  - `material: MaterialKind::MicaAlt`
- `window_command_spec() -> WindowCommandSpec`
  - `uses_winit_drag: true`
  - `self_drawn_controls: true`
  - `supports_double_click_maximize: true`
- `next_maximize_state(is_maximized: bool) -> bool`

## Slint Contract

### Root window

- `AppWindow`
  - `in-out property <bool> show-right-panel`
  - `in-out property <bool> show-global-menu`
  - `in-out property <bool> is-window-maximized`
  - `in-out property <bool> is-window-active`
  - Callbacks:
    - `drag-requested()`
    - `drag-double-clicked()`
    - `minimize-requested()`
    - `maximize-toggle-requested()`
    - `close-requested()`
    - `toggle-right-panel-requested()`
    - `toggle-global-menu-requested()`
    - `close-global-menu-requested()`

### Titlebar

- `Titlebar`
  - Reads:
    - `show-right-panel`
    - `show-global-menu`
    - `is-window-maximized`
    - `is-window-active`
  - Emits:
    - `drag-requested`
    - `drag-double-clicked`
    - `toggle-right-panel-requested`
    - `toggle-global-menu-requested`
    - `close-global-menu-requested`
    - `minimize-requested`
    - `maximize-toggle-requested`
    - `close-requested`
  - Layout zones:
    - `brand-zone`
    - `drag-zone`
    - `actions-zone`
    - `drag-safety-zone`
    - `window-controls`

### Titlebar sub-components

- `TitlebarIconButton`
  - `icon-source`
  - `active-icon-source`
  - `active`
  - `tooltip-text`
  - `clicked`
  - `tooltip-open-requested(string, length, length)`
  - `tooltip-close-requested()`
  - `has-hover`
- `WindowControlButton`
  - `icon-source`
  - `danger`
  - `tooltip-text`
  - `clicked`
  - `tooltip-open-requested(string, length, length)`
  - `tooltip-close-requested()`
  - `has-hover`
- `TitlebarMenu`
  - `settings-selected`
  - `appearance-selected`
  - `close-requested`
  - Anchored under `menu-button.absolute-position`
- `TitlebarTooltip`
  - `text`
  - `anchor-x`
  - `anchor-y`
  - Shared `PopupWindow` instance
  - `close-policy: PopupClosePolicy.no-auto-close`

## Visual and Asset Baseline

- The left `menu-button` is the only global menu entry.
- The right `panel-toggle-button` is the only utility icon in the compact utility group.
- The previous duplicate right-side `S` entry is removed.
- Caption controls now use vendored Fluent SVG assets:
  - `menu-20-regular.svg`
  - `menu-20-filled.svg`
  - `panel-right-20-regular.svg`
  - `panel-right-20-filled.svg`
  - `subtract-20-regular.svg`
  - `maximize-20-regular.svg`
  - `restore-20-regular.svg`
  - `dismiss-20-regular.svg`
- `Regular` is the default icon style.
- `Filled` is used only for active menu and active right-panel toggle states.

## State and Event Flow

1. `bind_top_status_bar()` creates `Rc<RefCell<ShellViewModel>>` and `Rc<WindowController<AppWindow>>`.
2. `sync_top_status_bar_state()` pushes the initial shell state into `AppWindow`.
3. Slint callbacks update the Rust state:
   - `toggle-right-panel-requested` flips `show_right_panel`
   - `toggle-global-menu-requested` flips `show_global_menu`
   - `close-global-menu-requested` clears `show_global_menu`
   - `maximize-toggle-requested` and `drag-double-clicked` share the same maximize toggle path
4. `Titlebar` owns tooltip presentation:
   - shared tooltip state properties
   - one `Timer` with `280ms` delay
   - one `TitlebarTooltip` popup
5. Button hover emits tooltip-open requests with absolute anchor coordinates.
6. Button click closes tooltip before forwarding the action callback.
7. `changed show-global-menu` closes tooltip first, then opens or closes the shared `TitlebarMenu`.

## Current Test and Smoke Coverage

- [shell_view_model.rs](/home/wwwroot/mica-term/tests/shell_view_model.rs)
  - Verifies top status bar state defaults and mutation methods.
- [top_status_bar_smoke.rs](/home/wwwroot/mica-term/tests/top_status_bar_smoke.rs)
  - Verifies `bind_top_status_bar()` wires `AppWindow` callbacks to visible root property changes.
- [titlebar_layout_spec.rs](/home/wwwroot/mica-term/tests/titlebar_layout_spec.rs)
  - Verifies titlebar layout budget constants.
- [fluent_titlebar_assets_smoke.sh](/home/wwwroot/mica-term/tests/fluent_titlebar_assets_smoke.sh)
  - Verifies vendored Fluent SVG files exist.
- [top_status_bar_ui_contract_smoke.sh](/home/wwwroot/mica-term/tests/top_status_bar_ui_contract_smoke.sh)
  - Verifies source-level contract for menu anchor, Fluent icon wiring, and tooltip presence.

## Verification Evidence

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -q`
- `bash tests/fluent_titlebar_assets_smoke.sh`
- `bash tests/top_status_bar_ui_contract_smoke.sh`

Desktop GUI smoke status in this environment:

- `cargo run` was not executed as a verification claim.
- The current Linux session reported no `DISPLAY` and no `WAYLAND_DISPLAY`.
- Real desktop interaction still needs a GUI-capable session.

## Edge Cases For Next TDD Phase

- `WindowController::drag()` depends on a live `winit` window and currently returns an error path in headless or unavailable-window conditions.
- `WindowController::close()` and `drag()` errors are intentionally ignored at callback call sites; next TDD phase should decide whether to surface or log them.
- `bind_top_status_bar()` uses `Rc<RefCell<ShellViewModel>>`. Any future Tokio or actor-driven updates must be marshaled back onto the UI thread with `slint::invoke_from_event_loop`.
- Tooltip behavior is source-contract tested, not runtime-interaction tested. Delay timing, hide timing, and overlap with menu popup still need GUI-backed verification.
- `TitlebarMenu` still exposes `settings-selected` and `appearance-selected`, but the current implementation only closes the popup and does not route to deeper feature state.
- `is_window_active` is only a visual input today. No platform focus event source updates it yet.
- `show_welcome` remains in `ShellViewModel`, but the welcome/content switch is not driven by the titlebar changes from this bugfix.
- Future Tokio channel or actor mailbox integration must cover blocked senders, dropped receivers, shutdown ordering, and UI-thread handoff ordering.

## Recommended Next TDD Targets

1. Add interaction tests for the shared tooltip lifecycle, including hover delay, close-on-click, and menu-open suppression.
2. Add integration tests for the global menu open/close lifecycle, including outside click and `Esc` close behavior in a GUI-capable environment.
3. Add GUI smoke coverage for drag, double-click maximize, and caption control behavior on a real desktop backend.
4. Add tests for focus-driven `is_window_active` transitions once platform window activation events are wired.
5. Add tests for future async state updates using `slint::invoke_from_event_loop` so background tasks cannot mutate UI state directly.
