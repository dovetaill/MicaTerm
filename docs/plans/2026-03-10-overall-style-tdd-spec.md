# Overall Style TDD Handoff

## Scope

This document captures the current Rust + Slint shell baseline after the overall style implementation tasks. It is intended as the starting point for the next `test-driven-development` phase.

## Core Rust API Surface

### App bootstrap

- `mica_term::app::bootstrap::app_title() -> &'static str`
- `mica_term::app::bootstrap::default_window_size() -> (u32, u32)`
- `mica_term::app::bootstrap::run() -> anyhow::Result<()>`

### Windowing

- `MaterialKind`
  - `MicaAlt`
- `WindowAppearance`
  - `no_frame: bool`
  - `material: MaterialKind`
- `window_appearance() -> WindowAppearance`

### Theme

- `ThemeMode`
  - `Dark`
  - `Light`
- `ThemeSpec`
  - `accent_name: &'static str`
  - `terminal_is_neutral: bool`
  - `panel_uses_tint: bool`
  - `supports_dark: bool`
  - `supports_light: bool`
- `theme_spec(mode: ThemeMode) -> ThemeSpec`

### Shell metrics

- `ShellMetrics`
  - `TITLEBAR_HEIGHT`
  - `ACTIVITY_BAR_WIDTH`
  - `ASSETS_SIDEBAR_WIDTH`
  - `TAB_BAR_HEIGHT`
  - `RIGHT_PANEL_WIDTH`
  - `BASE_SPACING`

### Welcome and signature state

- `WelcomeAction`
  - `NewConnection`
  - `OpenRecent`
  - `Snippets`
  - `Sftp`
- `ShellViewModel`
  - `show_welcome: bool`
  - `show_right_panel: bool`
- `welcome_actions() -> &'static [WelcomeAction]`
- `SignatureSurface`
  - `CommandEntry`
  - `ActiveTab`
  - `RightPanelSegmentedControl`
  - `WelcomeState`
  - `CommandPalette`
- `signature_surfaces() -> &'static [SignatureSurface]`

### Status and accessibility

- `ConnectionState`
  - `Connecting`
  - `Connected`
  - `Disconnected`
  - `Error`
- `StatusSpec`
  - `animated: bool`
  - `escalates_to_page_overlay: bool`
- `MotionSpec`
  - `drawer_open_ms: u32`
  - `welcome_transition_ms: u32`
- `status_spec(state: ConnectionState) -> StatusSpec`
- `motion_spec() -> MotionSpec`
- `AccessibilityFloor`
  - `keyboard_reachable: bool`
  - `dark_light_focus_clear: bool`
  - `high_contrast_safe: bool`
- `accessibility_floor() -> AccessibilityFloor`

## Slint Components

### Root shell

- `AppWindow`
  - Frameless shell container
  - Imports `ThemeTokens`, `MotionTokens`, `StatusPill`, `Titlebar`, `Sidebar`, `TabBar`, `RightPanel`, `WelcomeView`
  - Exposes `in property <bool> dark-mode`

### Theme globals

- `ThemeTokens`
  - `dark-mode`
  - `shell-surface`
  - `shell-stroke`
  - `command-tint`
  - `panel-tint`
  - `terminal-surface`
  - `accent`
  - `text-primary`
- `MotionTokens`
  - `drawer-duration`
  - `welcome-duration`

### Signature and scaffold components

- `Titlebar`
- `Sidebar`
- `TabBar`
- `RightPanel`
- `WelcomeView`
- `CommandEntry`
- `ActiveTab`
- `SegmentedControl`
- `CommandPalette`
- `StatusPill`

## Slint Callbacks and Bindings

- No custom Slint callbacks are implemented yet.
- No `ModelRc` usage is implemented yet.
- No `slint::invoke_from_event_loop` integration is implemented yet.
- Current bindings are static property reads from global tokens and fixed layout values.

## Current Testing Baseline

- `bootstrap_smoke.rs`
- `theme_spec.rs`
- `window_shell.rs`
- `shell_view_model.rs`
- `signature_surfaces.rs`
- `status_motion.rs`
- `accessibility_floor.rs`

## Edge Cases and Risks To Cover Next

- Frameless window behavior is only declared in Slint and Rust metadata; no platform-specific Mica application logic exists yet.
- `ThemeTokens.dark-mode` is currently left at its default path; runtime theme switching is not wired.
- `StatusPill` animation is present as a token-based opacity animation, but connection state is not connected to async runtime events.
- `CommandPalette` exists as a component file but is not mounted into the shell yet.
- The right panel is always visible in the current Slint tree even though `ShellViewModel.show_right_panel` defaults to `false`; state wiring is not implemented.
- Welcome actions are encoded in Rust only; they are not yet bound into interactive Slint models.
- No Tokio channel flow, actor boundary, or data race surface exists yet, but future tests should cover channel backpressure, shutdown ordering, and UI-thread handoff safety.
- Accessibility is currently a declared policy object only; keyboard traversal, focus ring visibility, and high-contrast rendering still need runtime assertions.

## Recommended Next TDD Targets

1. Wire `ShellViewModel` state into `AppWindow` with deterministic tests for right-panel visibility and welcome-to-terminal transitions.
2. Add runtime theme switching tests that verify dark/light token propagation without regressions in terminal neutrality.
3. Introduce explicit command palette visibility state and test its mount/unmount behavior.
4. Add async status propagation tests once Tokio channels or actor-style state updates are introduced.
5. Add visual-contract tests or snapshot-oriented checks for shell metrics and signature surface presence.
