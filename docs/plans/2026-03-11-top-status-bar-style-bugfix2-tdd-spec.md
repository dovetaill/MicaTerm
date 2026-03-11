# Top Status Bar Style Bugfix2 TDD Handoff

## Scope

This document captures the implemented 2026-03-11 `top status bar style bugfix2` baseline in the `Rust + Slint` shell. It is the handoff input for the next `test-driven-development` phase.

## Implemented Surface

### Core Rust structs and functions

- `mica_term::theme::ThemeMode`
  - `Dark`
  - `Light`
  - `toggled() -> ThemeMode`
- `mica_term::app::ui_preferences::UiPreferences`
  - `theme_mode: ThemeMode`
  - `always_on_top: bool`
  - `Default` returns `Dark` and `false`
- `mica_term::app::ui_preferences::UiPreferencesStore`
  - `new(path: PathBuf) -> Self`
  - `for_app() -> anyhow::Result<Self>`
  - `load_or_default() -> anyhow::Result<UiPreferences>`
  - `save(prefs: &UiPreferences) -> anyhow::Result<()>`
- `impl From<&ShellViewModel> for UiPreferences`
- `mica_term::shell::view_model::ShellViewModel`
  - `show_welcome: bool`
  - `show_right_panel: bool`
  - `show_global_menu: bool`
  - `is_window_maximized: bool`
  - `is_window_active: bool`
  - `theme_mode: ThemeMode`
  - `is_always_on_top: bool`
- `ShellViewModel::toggle_right_panel()`
- `ShellViewModel::toggle_global_menu()`
- `ShellViewModel::close_global_menu()`
- `ShellViewModel::set_window_maximized(value: bool)`
- `ShellViewModel::set_window_active(value: bool)`
- `ShellViewModel::toggle_theme_mode()`
- `ShellViewModel::toggle_always_on_top()`
- `mica_term::app::bootstrap::bind_top_status_bar(window: &AppWindow)`
- `mica_term::app::bootstrap::bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>)`
- `mica_term::app::bootstrap::run() -> anyhow::Result<()>`
- `mica_term::app::bootstrap::app_title() -> &'static str`
- `mica_term::app::bootstrap::default_window_size() -> (u32, u32)`

### Windowing interfaces

- No new trait was introduced by this bugfix.
- `mica_term::app::windowing::WindowController<C: ComponentHandle>`
  - Holds `slint::Weak<C>`
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
  - `supports_always_on_top: true`
- `next_maximize_state(is_maximized: bool) -> bool`

### Shell metrics

- `mica_term::shell::metrics::ShellMetrics`
  - `TITLEBAR_HEIGHT`
  - `TITLEBAR_NAV_WIDTH`
  - `TITLEBAR_BRAND_WIDTH`
  - `TITLEBAR_UTILITY_WIDTH`
  - `TITLEBAR_WINDOW_CONTROL_WIDTH`
  - `TITLEBAR_MIN_DRAG_WIDTH`
  - `TITLEBAR_TOOL_BUTTON_SIZE`
  - `TITLEBAR_TOOL_ICON_SIZE`
  - `ACTIVITY_BAR_WIDTH`
  - `ASSETS_SIDEBAR_WIDTH`
  - `TAB_BAR_HEIGHT`
  - `RIGHT_PANEL_WIDTH`
  - `BASE_SPACING`

## Slint Contract

### Root window

- `AppWindow`
  - `in-out property <bool> dark-mode <=> ThemeTokens.dark-mode`
  - `in-out property <bool> show-right-panel`
  - `in-out property <bool> show-global-menu`
  - `in-out property <bool> is-window-maximized`
  - `in-out property <bool> is-window-active`
  - `in-out property <bool> is-window-always-on-top`
  - Root window binding:
    - `always-on-top: root.is-window-always-on-top`
  - Callback surface:
    - `drag-requested()`
    - `drag-double-clicked()`
    - `minimize-requested()`
    - `maximize-toggle-requested()`
    - `close-requested()`
    - `toggle-right-panel-requested()`
    - `toggle-global-menu-requested()`
    - `close-global-menu-requested()`
    - `toggle-theme-mode-requested()`
    - `toggle-window-always-on-top-requested()`

### Titlebar

- `Titlebar`
  - Reads:
    - `dark-mode`
    - `show-right-panel`
    - `show-global-menu`
    - `is-window-maximized`
    - `is-window-active`
    - `is-window-always-on-top`
  - Emits:
    - `drag-requested`
    - `drag-double-clicked`
    - `toggle-theme-mode-requested`
    - `toggle-right-panel-requested`
    - `toggle-global-menu-requested`
    - `close-global-menu-requested`
    - `toggle-window-always-on-top-requested`
    - `minimize-requested`
    - `maximize-toggle-requested`
    - `close-requested`
  - Final layout zones:
    - `nav-zone`
    - `brand-zone`
    - `drag-zone`
    - `utility-zone`
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
  - Current visual baseline:
    - `36px x 36px` hit area
    - `20px x 20px` icon display
- `WindowControlButton`
  - `icon-source`
  - `danger`
  - `tooltip-text`
  - `clicked`
  - `tooltip-open-requested(string, length, length)`
  - `tooltip-close-requested()`
  - `has-hover`
  - Current visual baseline:
    - `46px x 36px` control box
    - `20px x 20px` icon display
- `TitlebarMenu`
  - `settings-selected`
  - `appearance-selected`
  - `close-requested`
  - Anchored under `nav-button.absolute-position`
- `TitlebarTooltip`
  - `text`
  - `anchor-x`
  - `anchor-y`
  - Shared `PopupWindow`
  - `close-policy: PopupClosePolicy.no-auto-close`

## Visual and Asset Baseline

- Leftmost navigation entry:
  - `nav-button := TitlebarIconButton`
  - Uses `assets/icons/fluent/navigation-24-regular.svg`
- Header branding:
  - `brand-logotype := Image`
  - Uses `assets/icons/mica-term-header-logotype.svg`
  - `currentColor`-driven for theme tinting
- Utility cluster order:
  - `theme-button`
  - `panel-toggle-button`
  - `divider-line`
  - `pin-button`
- Window controls order:
  - `minimize-button`
  - `maximize-button`
  - `close-button`
- All titlebar buttons use Fluent SVG assets.
- Theme state icons:
  - `dark-theme-20-regular.svg`
  - `weather-sunny-20-regular.svg`
- Pin state icons:
  - `pin-20-regular.svg`
  - `pin-off-20-regular.svg`
- Caption control icons:
  - `subtract-20-regular.svg`
  - `maximize-20-regular.svg`
  - `restore-20-regular.svg`
  - `dismiss-20-regular.svg`
- Removed from the titlebar source:
  - `Workspace`
  - `SSH`
  - Old `MT + Text` brand composition

## State and Event Flow

1. `bind_top_status_bar()` resolves the application `UiPreferencesStore`.
2. `bind_top_status_bar_with_store()` loads persisted preferences, falling back to defaults if the file is absent or unreadable.
3. Bootstrap seeds `ShellViewModel` with:
   - `theme_mode`
   - `is_always_on_top`
   - all previous top bar state defaults
4. `sync_top_status_bar_state()` pushes current Rust state into `AppWindow`:
   - `dark-mode`
   - `show-right-panel`
   - `show-global-menu`
   - `is-window-maximized`
   - `is-window-active`
   - `is-window-always-on-top`
5. Slint callbacks update Rust state:
   - `toggle-theme-mode-requested` flips `theme_mode`, updates `dark-mode`, persists `UiPreferences`
   - `toggle-window-always-on-top-requested` flips `is_always_on_top`, updates `is-window-always-on-top`, persists `UiPreferences`
   - `toggle-right-panel-requested` flips `show_right_panel`
   - `toggle-global-menu-requested` flips `show_global_menu`
   - `close-global-menu-requested` clears `show_global_menu`
   - `maximize-toggle-requested` and `drag-double-clicked` share the maximize toggle path
6. `Titlebar` owns tooltip scheduling:
   - shared tooltip state properties
   - one `Timer` with `280ms` delay
   - one shared `TitlebarTooltip`
7. Button click always closes tooltip before forwarding action.
8. `changed show-global-menu` closes tooltip before opening or closing the shared `TitlebarMenu`.

## Persistence Contract

- Storage file:
  - Standard app config directory
  - `ui-preferences.json`
- Stored fields:
  - `theme_mode`
  - `always_on_top`
- Fallback rules:
  - Missing file -> use `UiPreferences::default()`
  - Read failure -> log to stderr and use default
  - Save failure -> log to stderr and keep UI interactive
- Test isolation:
  - `top_status_bar_smoke.rs` injects a temporary `UiPreferencesStore`
  - This prevents local user config from polluting headless test expectations

## Current Test and Smoke Coverage

- [ui_preferences.rs](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/ui_preferences.rs)
  - Verifies default values and roundtrip persistence for `theme_mode` and `always_on_top`.
- [shell_view_model.rs](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/shell_view_model.rs)
  - Verifies `ShellViewModel` top bar state defaults plus `theme/pin` toggle behavior.
- [window_shell.rs](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/window_shell.rs)
  - Verifies frameless shell strategy and `supports_always_on_top`.
- [titlebar_layout_spec.rs](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/titlebar_layout_spec.rs)
  - Verifies bugfix2 titlebar budget constants.
- [top_status_bar_smoke.rs](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/top_status_bar_smoke.rs)
  - Verifies callback wiring for `theme`, `pin`, panel toggle, menu toggle, and maximize state.
- [fluent_titlebar_assets_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/fluent_titlebar_assets_smoke.sh)
  - Verifies required Fluent SVG assets exist.
- [icon_svg_assets_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/icon_svg_assets_smoke.sh)
  - Verifies core SVG assets and the new header logotype contract.
- [top_status_bar_ui_contract_smoke.sh](/home/wwwroot/mica-term/.worktrees/feature-top-status-bar-style-bugfix2/tests/top_status_bar_ui_contract_smoke.sh)
  - Verifies source-level titlebar contract for final bugfix2 layout and asset wiring.

## Verification Evidence

- `cargo fmt --all`
- `cargo check --workspace`
- `cargo test -q`
- `bash tests/fluent_titlebar_assets_smoke.sh`
- `bash tests/icon_svg_assets_smoke.sh`
- `bash tests/top_status_bar_ui_contract_smoke.sh`
- `cargo clippy --workspace -- -D warnings`

Desktop GUI smoke status in this environment:

- `cargo run` was not executed as a completion claim.
- The current Linux session reported:
  - `DISPLAY=`
  - `WAYLAND_DISPLAY=`
- Real desktop interaction still requires a GUI-capable session.

## Edge Cases For Next TDD Phase

- `always-on-top` support is declared in `WindowCommandSpec`, but real backend behavior can still differ across Windows, Linux, and future macOS targets.
- Persistence is intentionally non-blocking:
  - load failures silently fall back to defaults after stderr logging
  - save failures do not interrupt interaction
  - next TDD phase should verify expected stderr/logging behavior if that becomes a contract
- `bind_top_status_bar_with_store()` uses `Rc<RefCell<ShellViewModel>>`.
  - Any future Tokio task, actor, or channel-driven state update must marshal UI changes back with `slint::invoke_from_event_loop`.
  - Direct background-thread mutation would risk borrow panics or UI-thread violations.
- Future async state propagation should explicitly test:
  - channel sender blocked or backpressured
  - dropped receiver during shutdown
  - event-loop ordering when multiple state updates are queued
  - stale state writes after the window is already gone
- `WindowController::drag()` and `WindowController::close()` still return error paths when the window backend is unavailable; current callback sites intentionally ignore those results.
- Tooltip behavior is source-contract tested, not end-to-end interaction tested.
  - Hover delay
  - click-to-close timing
  - menu-open overlap
  - multi-monitor / high-DPI anchor correctness
  all still need GUI-backed verification.
- `is_window_active` remains a visual input only; no platform focus event source updates it yet.
- `TitlebarMenu` still exposes `settings-selected` and `appearance-selected`, but the current implementation only closes the popup and does not route to deeper feature state.
- The new header logotype is a hand-authored `currentColor` wordmark.
  - Future TDD should verify visual legibility at real DPI scales and across dark/light themes.

## Recommended Next TDD Targets

1. Add GUI-capable interaction tests for `theme` and `pin`, including immediate state change plus persistence after restart.
2. Add runtime tests for the shared tooltip lifecycle, including delay, hide-on-click, and suppression when the global menu opens.
3. Add integration tests for global menu positioning under `nav-button` and close behavior on outside click or `Esc`.
4. Add backend-aware tests or smoke coverage for real always-on-top behavior on Windows 11.
5. Add async safety tests that route future Tokio or actor state updates through `slint::invoke_from_event_loop`.
