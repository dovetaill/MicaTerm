# Theme Toggle Window Appearance TDD Spec

Date: 2026-03-11
Status: implementation landed, automated verification passed, Windows 11 manual validation pending

## Purpose

This document is the handoff input for the next `test-driven-development` phase of the theme toggle window appearance work.

The scope of that phase is not to redesign the feature. The scope is to harden the current implementation with regression coverage around:

- dual-layer theme sync
- Windows native appearance application
- persistence behavior
- platform downgrade behavior
- manual Windows 11 edge-case validation

## Implemented Architecture Snapshot

- `ThemeMode` remains the single source of truth for UI theme selection.
- Slint content theme sync still happens through `AppWindow.dark-mode <=> ThemeTokens.dark-mode`.
- Native window shell sync now flows through a thin `PlatformWindowEffects` abstraction.
- `bootstrap` performs native sync both:
- during initial binding
- when `toggle-theme-mode-requested` fires
- Windows uses `winit::window::Window::set_theme(...)`, `window_vibrancy::apply_tabbed(...)` or `apply_mica(...)`, and `request_redraw()`.
- Non-Windows platforms currently downgrade to `NoopWindowEffects`.
- Top status bar buttons remain on Fluent SVG assets. This is an invariant and should not regress during later test work.

## Core Files

- `src/app/bootstrap.rs`
- `src/app/window_effects.rs`
- `src/app/windowing.rs`
- `src/app/ui_preferences.rs`
- `src/shell/view_model.rs`
- `ui/app-window.slint`
- `ui/shell/titlebar.slint`
- `tests/window_effects.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/window_shell.rs`
- `tests/window_theme_contract_smoke.sh`
- `verification.md`

## Core Types And Interfaces

### `ThemeMode`

Location: `src/theme/spec.rs`

- `ThemeMode::Dark`
- `ThemeMode::Light`
- `ThemeMode::toggled()`

Testing importance:

- theme toggle behavior must always start from and write back to this type
- no second source of truth should be introduced in later refactors

### `ShellViewModel`

Location: `src/shell/view_model.rs`

Relevant fields:

- `theme_mode: ThemeMode`
- `is_always_on_top: bool`
- `show_right_panel: bool`
- `show_global_menu: bool`
- `is_window_maximized: bool`
- `is_window_active: bool`

Relevant methods:

- `toggle_theme_mode()`
- `toggle_always_on_top()`
- `toggle_right_panel()`
- `toggle_global_menu()`
- `close_global_menu()`
- `set_window_maximized(bool)`

Testing importance:

- next-phase tests should verify only theme-related actions trigger native appearance sync
- non-theme callbacks must not accidentally call the native appearance bridge

### `UiPreferences` And `UiPreferencesStore`

Location: `src/app/ui_preferences.rs`

Relevant fields:

- `theme_mode`
- `always_on_top`

Relevant methods:

- `load_or_default()`
- `save(&UiPreferences)`

Testing importance:

- persisted theme must round-trip correctly
- storage failure should fall back safely without panicking

### `NativeWindowAppearanceRequest`

Location: `src/app/window_effects.rs`

Fields:

- `theme: NativeWindowTheme`
- `backdrop: BackdropPreference`
- `request_redraw: bool`

Construction:

- `build_native_window_appearance_request(ThemeMode, WindowAppearance)`

Testing importance:

- dark and light both map to `BackdropPreference::MicaAlt`
- redraw intent must remain `true`
- any future backdrop expansion must preserve current default behavior unless design docs change

### `PlatformWindowEffects`

Location: `src/app/window_effects.rs`

Trait contract:

```rust
fn apply_to_app_window(
    &self,
    window: &AppWindow,
    request: &NativeWindowAppearanceRequest,
) -> WindowAppearanceSyncReport;
```

Implementations:

- `NoopWindowEffects`
- `WindowsWindowEffects` behind `#[cfg(target_os = "windows")]`

Testing importance:

- `NoopWindowEffects` must remain safe on unsupported platforms
- Windows implementation must not panic when backdrop application fails
- return values should stay explicit enough for later logging or telemetry if needed

### `WindowAppearanceSyncReport`

Location: `src/app/window_effects.rs`

Fields:

- `theme_applied`
- `backdrop_status`
- `redraw_requested`

Testing importance:

- `skipped()` must stay explicit
- future tests should cover failure and skipped branches, not only success branches

## Slint Callback Chain

### Initial Bind

1. `AppWindow` is created.
2. `bind_top_status_bar_with_store_and_effects(...)` loads persisted preferences.
3. `ShellViewModel` is seeded from `UiPreferences`.
4. `sync_top_status_bar_state(...)` runs immediately.
5. `sync_theme_and_window_effects(...)` sets `window.dark-mode`.
6. `build_native_window_appearance_request(...)` builds native request.
7. `PlatformWindowEffects::apply_to_app_window(...)` is called.

### Theme Toggle Path

1. `Titlebar.toggle-theme-mode-requested`
2. `AppWindow.toggle-theme-mode-requested`
3. `bootstrap` callback `window.on_toggle_theme_mode_requested(...)`
4. `ShellViewModel::toggle_theme_mode()`
5. `sync_theme_and_window_effects(...)`
6. `UiPreferencesStore::save(...)`

## Existing Regression Coverage

### Rust tests already present

- `tests/window_effects.rs`
- `tests/top_status_bar_smoke.rs`
- `tests/window_shell.rs`

Current assertions already cover:

- `ThemeMode -> NativeWindowTheme` mapping
- `ThemeMode -> BackdropPreference::MicaAlt` mapping
- skipped sync report contract
- default provider construction
- initial bind emits a native appearance request
- theme toggle emits a second native appearance request
- shell metadata still prefers frameless `MicaAlt`

### Source contract smoke already present

- `tests/window_theme_contract_smoke.sh`

Current smoke assertions cover source presence of:

- `window.set_theme(Some(...))`
- `window.request_redraw();`
- `window_vibrancy::apply_tabbed`
- `#[cfg(target_os = "windows")]`
- `NoopWindowEffects`

## Required Next-Phase TDD Targets

### Unit tests to add first

- test that a non-theme callback such as `toggle-right-panel-requested` does not emit a native appearance request
- test that `toggle-window-always-on-top-requested` persists state without emitting a theme sync request
- test that `build_native_window_appearance_request(...)` remains stable if `WindowAppearance` later gets more materials
- test that `WindowAppearanceSyncReport::skipped()` remains the fallback for unsupported environments

### Failure-path tests to add

- fake provider returns `BackdropApplyStatus::Failed` and bootstrap path still completes without panic
- `UiPreferencesStore::load_or_default()` returns default state when file is missing
- malformed preferences file path should be covered with an explicit error-path test if the persistence layer is expanded

### Manual Windows validation to convert into repeatable checks

- partially off-screen bottom toggle
- partially off-screen left toggle
- partially off-screen right toggle
- partially off-screen top toggle
- maximized toggle
- restored toggle
- restart and persistence confirmation
- transparent effects disabled in Windows settings

### Optional future integration work

- if a Windows-capable CI runner becomes available, add platform-gated integration coverage around `WindowsWindowEffects`
- if native sync ever moves off the Slint/UI thread, add explicit `slint::invoke_from_event_loop(...)` handoff coverage

## Edge Cases And Risks

### Platform downgrade behavior

- On non-Windows systems the default provider is `NoopWindowEffects`.
- This is intentional.
- Future tests must not treat lack of native shell sync on Linux/macOS as a failure unless the design document changes.

### Missing native window handle

- `WindowsWindowEffects` uses `with_winit_window(...)`.
- If the native window is unavailable, the closure does not run and the report remains effectively skipped.
- This branch is not directly asserted yet and should be covered in the next TDD phase if the abstraction is widened.

### Backdrop failure

- `apply_tabbed(...)` and `apply_mica(...)` map failure to `BackdropApplyStatus::Failed`.
- They do not panic.
- Later tests should keep this non-panicking downgrade contract intact.

### Persistence failure

- bootstrap currently logs preference load/save errors with `eprintln!` and continues.
- Later work must preserve this non-fatal behavior unless a product decision explicitly changes startup policy.

### Threading and data race boundaries

- This feature currently stays on the Slint/UI thread and uses `Rc<RefCell<...>>`.
- No Tokio channel, actor mailbox, or cross-thread state handoff was introduced in this scope.
- Because of that, no new Rust data race surface was added by this feature itself.
- If later work introduces Tokio tasks, channels, or background window sync, `AppWindow`, `Rc`, and `RefCell` must not be moved across threads directly.
- Any future async bridge must marshal back through `slint::invoke_from_event_loop(...)` or an equivalent UI-thread handoff.

### Fluent SVG invariant

- The top status bar currently uses Fluent SVG assets for the button icons.
- Theme-related follow-up tests must not replace or regress those asset paths while chasing appearance bugs.

## Current Verification State

- Automated verification passed and is recorded in `verification.md`.
- Windows 11 manual checklist is still pending.
- Do not claim the off-screen theme-toggle bug is fully validated on real Windows hardware until that manual checklist is executed.
