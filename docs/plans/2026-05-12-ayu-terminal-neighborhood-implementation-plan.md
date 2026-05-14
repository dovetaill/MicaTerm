# Ayu Terminal Neighborhood Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Refine MicaTerm's default Ayu experience so dark and light mode project one coherent terminal-and-shell neighborhood palette across Rust theme truth, runtime projection, fallback, Slint host surfaces, and renderer-adjacent terminal chrome.

**Architecture:** Keep `src/theme/spec.rs` as the authored source of truth for both terminal and shell-neighborhood colors. Project those values through `src/app/terminal_theme.rs` into a runtime-facing combined preset, publish the active preset from `src/app/bootstrap.rs` and `src/app/bootstrap/shell_chrome.rs`, and make Slint consumers read runtime shell/session properties instead of detached hardcoded token colors. Preserve flat viewport backgrounds so bitmap and native terminal paths stay identical.

**Tech Stack:** Rust, Slint, `wezterm-term`, existing bootstrap shell-chrome sync path, bitmap atlas presenter, native terminal renderer, Rust source-contract and smoke tests.

---

### Task 1: Freeze the approved Ayu dark/light targets and wording cleanup in tests

**Files:**
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/theme_terminal_redesign_spec.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Modify: `tests/theme_semantic_token_contract_spec.rs`
- Modify: `tests/ui_preferences.rs`
- Reference: `docs/plans/2026-05-12-ayu-terminal-neighborhood-design.md`

**Step 1: Write the failing preset-value tests**

Add or update assertions that require the new approved values:

```rust
#[test]
fn dark_theme_maps_terminal_palette_to_ayu_dark() {
    let preset = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    assert_eq!(preset.name, "Ayu Dark");
    assert_eq!(preset.background, 0x0a_0e14);
    assert_eq!(preset.foreground, 0xc5_c1b8);
    assert_eq!(preset.cursor_bg, 0xe6_b450);
    assert_eq!(preset.cursor_fg, 0x0a_0e14);
    assert_eq!(preset.selection_bg, (0x2a, 0x35, 0x41, 0.78));
}
```

Also add light-mode assertions for:

- `background = 0xf8_f9fa`
- `foreground = 0x5c_6166`
- `cursor_bg = 0xff_aa33`
- `cursor_fg = 0xf8_f9fa`
- `selection_bg = (0x55, 0xb4, 0xd4, 0.20)`

**Step 2: Add the shell-neighborhood contract expectations**

In `tests/theme_terminal_redesign_spec.rs`, replace the current shell ladder assertions with the approved Ayu neighborhood values. Lock at least these fields:

- dark shell:
  - `app_background = 0x0a_0e14`
  - `titlebar_background = 0x10_151d`
  - `tabbar_background = 0x10_151d`
  - `sidebar_background = 0x10_151d`
  - `sidebar_panel_background = 0x11_1821`
  - `right_panel_background = 0x11_1821`
  - `terminal_frame_background = 0x14_1b24`
  - `border = 0x1b_2530`
  - `text_primary = 0xc5_c1b8`
  - `text_secondary = 0x9a_a4ae`
  - `text_muted = 0x7d_8790`
  - `accent = 0xe6_b450`
- light shell:
  - `app_background = 0xf4_f6f8`
  - `titlebar_background = 0xee_f2f5`
  - `tabbar_background = 0xee_f2f5`
  - `sidebar_background = 0xee_f2f5`
  - `sidebar_panel_background = 0xf0_f3f6`
  - `right_panel_background = 0xf0_f3f6`
  - `terminal_frame_background = 0xfa_fafa`
  - `border = 0xd8_dee6`
  - `text_primary = 0x5c_6166`
  - `text_secondary = 0x7a_838c`
  - `text_muted = 0x8a_939c`
  - `accent = 0xff_aa33`

**Step 3: Add fallback and wording cleanup assertions**

In `tests/bootstrap_smoke.rs` and `tests/theme_semantic_token_contract_spec.rs`, add or update assertions that require:

- no-surface fallback fg/bg/cursor to match the active preset after theme toggles
- terminal frame / host / scrollbar values to match the runtime-projected preset
- no default-theme wording still describes the active preset as Catppuccin,
  Graphite, or Canvas

Example wording check:

```rust
assert!(
    !theme_spec.contains("Catppuccin")
        && !theme_spec.contains("Graphite")
        && !theme_spec.contains("Canvas"),
    "default Ayu terminal tests should stop using retired premium-palette names"
);
```

**Step 4: Run the focused tests to confirm they fail**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test ui_preferences -- --nocapture
cargo test bootstrap_smoke -- --nocapture
```

Expected: FAIL because the current preset and shell chrome values are still the
older post-migration values, and the new wording/parity assertions are not yet
satisfied.

**Step 5: Commit**

```bash
git add tests/terminal_theme_selection_spec.rs tests/theme_terminal_redesign_spec.rs tests/bootstrap_smoke.rs tests/theme_semantic_token_contract_spec.rs tests/ui_preferences.rs
git commit -m "test: freeze ayu terminal neighborhood targets"
```

### Task 2: Update `src/theme/spec.rs` to the approved Ayu dark/light authored values

**Files:**
- Modify: `src/theme/spec.rs`
- Test: `tests/terminal_theme_selection_spec.rs`
- Test: `tests/theme_terminal_redesign_spec.rs`

**Step 1: Change the shared terminal background constants**

Update:

```rust
pub const TERMINAL_BG_BASE_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_GRADIENT_TOP_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_GRADIENT_BOTTOM_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xf8_f9fa;
```

Keep row banding disabled and keep top/bottom equal to base so renderers remain
flat and identical.

**Step 2: Update the `PremiumDefault` terminal fg/cursor/selection/scrollbar values**

In `premium_default_spec(mode)`, change:

- dark terminal foreground to `0xc5_c1b8`
- dark selection to `rgb = 0x2a_3541`, `alpha = 0.78`
- dark scrollbar track/thumb/active to `0x11_1821`, `0x2f_3944`, `0x3c_4856`
- light background base to `0xf8_f9fa`
- light cursor foreground to `TERMINAL_BG_BASE_LIGHT`
- light selection to `rgb = 0x55_b4d4`, `alpha = 0.20`
- light scrollbar track/thumb/active to `0xf0_f3f6`, `0xd1_d7de`, `0xc1_c8d1`

Do not change the Ayu ANSI 16 values unless a targeted test explicitly proves a
specific mismatch.

**Step 3: Update the `PremiumDefault` shell chrome values**

Replace `premium_shell_dark()` and `premium_shell_light()` with the approved
authored values:

```rust
fn premium_shell_dark() -> ShellChromeTheme {
    ShellChromeTheme {
        app_background: 0x0a_0e14,
        titlebar_background: 0x10_151d,
        tabbar_background: 0x10_151d,
        sidebar_background: 0x10_151d,
        sidebar_panel_background: 0x11_1821,
        right_panel_background: 0x11_1821,
        terminal_frame_background: 0x14_1b24,
        separator: 0x1b_2530,
        border: 0x1b_2530,
        text_primary: 0xc5_c1b8,
        text_secondary: 0x9a_a4ae,
        text_muted: 0x7d_8790,
        accent: 0xe6_b450,
        // ...keep the remaining state fields in the same Ayu family
    }
}
```

Mirror the approved light values in `premium_shell_light()`.

**Step 4: Run the focused tests to verify the authored source now matches**

Run:

```bash
cargo test terminal_theme -- --nocapture
```

Expected: PASS for the terminal-theme and shell-ladder expectations that only
depend on `src/theme/spec.rs`.

**Step 5: Commit**

```bash
git add src/theme/spec.rs tests/terminal_theme_selection_spec.rs tests/theme_terminal_redesign_spec.rs
git commit -m "feat: update ayu terminal neighborhood theme spec"
```

### Task 3: Expand the runtime projection layer to carry both terminal and shell-neighborhood colors

**Files:**
- Modify: `src/app/terminal_theme.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/shell_chrome.rs`
- Modify: `ui/app-window.slint`
- Test: `tests/bootstrap_smoke.rs`
- Reference: `src/theme/spec.rs`

**Step 1: Add a combined projected preset in `src/app/terminal_theme.rs`**

Keep `TerminalThemePreset`, and add a second runtime-facing wrapper that carries
the shell fields alongside the terminal preset:

```rust
#[derive(Debug, Clone, Copy)]
pub struct ProjectedThemePreset {
    pub terminal: TerminalThemePreset,
    pub app_background: u32,
    pub titlebar_background: u32,
    pub tabbar_background: u32,
    pub sidebar_background: u32,
    pub sidebar_panel_background: u32,
    pub right_panel_background: u32,
    pub separator: u32,
    pub border: u32,
    pub hairline: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_inactive: u32,
    pub accent: u32,
    pub link_accent: u32,
    pub focus_ring: u32,
    pub tab_active: u32,
    pub tab_inactive: u32,
    pub tab_hover: u32,
    pub tab_active_indicator: u32,
    pub sidebar_item_hover: u32,
    pub sidebar_item_selected: u32,
    pub sidebar_item_selected_border: u32,
}
```

Add `projected_theme_for(theme_mode, variant)` and
`projected_theme_for_mode(theme_mode)` that call `app_theme_spec(theme_mode,
variant)` and populate both `terminal` and shell fields from one source.

**Step 2: Keep existing terminal helpers intact**

Retain:

- `preset_for_theme()`
- `preset_for_theme_mode()`
- `palette_for_theme()`
- `selection_overlay_rgba_for()`

These helpers still serve runtime/renderer code. The new wrapper exists so
bootstrap and shell chrome can publish the same active preset family into Slint.

**Step 3: Add `AppWindow` shell properties and runtime publish helpers**

In `ui/app-window.slint`, declare the runtime shell palette properties so the
generated setters exist before bootstrap tries to publish them:

```slint
in-out property <color> shell-app-background: ThemeTokens.window-surface;
in-out property <color> shell-titlebar-background: ThemeTokens.titlebar-background;
in-out property <color> shell-tabbar-background: ThemeTokens.tabbar-background;
in-out property <color> shell-sidebar-background: ThemeTokens.sidebar-background;
in-out property <color> shell-sidebar-panel-background: ThemeTokens.sidebar-panel-background;
in-out property <color> shell-right-panel-background: ThemeTokens.right-panel-background;
// ...mirror the remaining shell state colors here
```

Then, in bootstrap, add the runtime publish helper:

```rust
fn sync_shell_runtime_palette(window: &AppWindow, preset: ProjectedThemePreset) {
    window.set_shell_app_background(slint_color_from_rgba(0xff00_0000 | preset.app_background));
    window.set_shell_titlebar_background(slint_color_from_rgba(0xff00_0000 | preset.titlebar_background));
    window.set_shell_tabbar_background(slint_color_from_rgba(0xff00_0000 | preset.tabbar_background));
    // ...
}
```

Keep the existing `sync_workspace_terminal_shell_chrome()` helper for
session-scoped terminal host values, but change it to read
`preset.terminal.frame_bg`, `preset.terminal.scrollbar_*`, and
`preset.terminal.selection_bg` from the combined projection.

**Step 4: Publish the shell runtime palette on every relevant lifecycle**

Call the new helper:

- during initial bind
- after theme mode toggles
- after theme variant changes
- whenever shell chrome is re-synced because runtime state changed

Do not add a second source of shell colors anywhere else.

**Step 5: Run the focused tests to verify runtime projection behavior**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
```

Expected: PASS for the runtime projection and no-surface shell-palette checks
that only depend on the generated `AppWindow` setters and bootstrap wiring.

**Step 6: Commit**

```bash
git add src/app/terminal_theme.rs src/app/bootstrap.rs src/app/bootstrap/shell_chrome.rs ui/app-window.slint tests/bootstrap_smoke.rs
git commit -m "feat: project ayu shell and terminal runtime palette"
```

### Task 4: Add first-class Slint properties for the runtime shell palette and thread them through the window tree

**Files:**
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/components/active-tab.slint`
- Modify: `ui/shell/sidebar.slint`
- Modify: `ui/shell/assets-sidebar.slint`
- Modify: `ui/components/sidebar-nav-button.slint`
- Modify: `ui/components/asset-node-row.slint`
- Modify: `ui/shell/right-panel.slint`
- Modify: `ui/shell/workspace-pane.slint`
- Modify: `ui/shell/terminal-session-host.slint`
- Test: `tests/theme_semantic_token_contract_spec.rs`
- Test: `tests/native_terminal_surface_contract_spec.rs`

**Step 1: Thread the runtime shell palette properties into the actual shell components**

Pass the new properties into:

- `Titlebar`
- `TabBar`
- `Sidebar`
- `AssetsSidebar`
- `RightPanel`
- `WorkspacePane`

Keep the property names parallel all the way down so code review can confirm
the same runtime surface travels through the tree unchanged.

**Step 2: Switch shell consumers from `ThemeTokens` to runtime properties**

Update the shell components so the active surface colors come from the new
runtime properties instead of hardcoded token reads. Examples:

```slint
background: root.titlebar-background-surface;
border-color: root.separator-surface;
color: root.text-primary-color;
```

Do this at least for:

- titlebar background and text hierarchy
- tabbar background and active/inactive/hover tab surfaces
- sidebar / assets sidebar / right-panel primary surfaces
- sidebar selected and hover states
- workspace shell frame around the terminal session host

Do not leave any active terminal-neighborhood consumer on a detached token value
if there is now a matching runtime property.

**Step 3: Keep terminal-session host on session-scoped runtime values**

Continue to use:

- `session-frame-surface`
- `session-frame-border`
- `session-selection-surface`
- `session-scrollbar-track`
- `session-scrollbar-thumb`
- `session-scrollbar-thumb-active`

The session host should not read those active colors from generic shell props.

**Step 4: Run the source-contract tests to confirm the runtime chain exists**

Run:

```bash
cargo test theme_semantic_token_contract_spec -- --nocapture
cargo test native_terminal_surface_contract_spec -- --nocapture
```

Expected: PASS after the new runtime property chain replaces the detached token
paths in shell and terminal host consumers.

**Step 5: Commit**

```bash
git add ui/shell/titlebar.slint ui/shell/tabbar.slint ui/components/active-tab.slint ui/shell/sidebar.slint ui/shell/assets-sidebar.slint ui/components/sidebar-nav-button.slint ui/components/asset-node-row.slint ui/shell/right-panel.slint ui/shell/workspace-pane.slint ui/shell/terminal-session-host.slint tests/theme_semantic_token_contract_spec.rs tests/native_terminal_surface_contract_spec.rs
git commit -m "feat: thread runtime ayu shell palette through slint"
```

### Task 5: Align `ui/theme/tokens.slint` boot defaults and verify no-surface parity

**Files:**
- Modify: `ui/theme/tokens.slint`
- Modify: `tests/terminal_theme_selection_spec.rs`
- Modify: `tests/ui_preferences.rs`
- Modify: `tests/bootstrap_smoke.rs`
- Reference: `docs/plans/2026-05-12-ayu-terminal-neighborhood-design.md`

**Step 1: Update boot-time terminal defaults to the approved Ayu values**

Change `ui/theme/tokens.slint` boot defaults so they match the new authored
preset exactly:

- `terminal-canvas-surface: dark-mode ? #0a0e14 : #f8f9fa`
- `terminal-default-fg: dark-mode ? #c5c1b8 : #5c6166`
- `terminal-cursor-fg: dark-mode ? #0a0e14 : #f8f9fa`
- `terminal-cursor-bg: dark-mode ? #e6b450 : #ffaa33`
- `terminal-selection-surface: dark-mode ? #2a3541c7 : #55b4d433`
- `terminal-scrollbar-track-surface: dark-mode ? #111821 : #f0f3f6`
- `terminal-scrollbar-thumb-surface: dark-mode ? #2f3944 : #d1d7de`
- `terminal-scrollbar-thumb-active-surface: dark-mode ? #3c4856 : #c1c8d1`

Keep them as startup defaults only; do not treat them as the active runtime
truth once bootstrap publishes the real preset.

**Step 2: Update boot-time shell defaults to the approved Ayu values**

Replace the detached premium shell tokens so boot-time parity also matches the
approved Ayu neighborhood:

- `titlebar-background: dark-mode ? #10151d : #eef2f5`
- `tabbar-background: dark-mode ? #10151d : #eef2f5`
- `sidebar-background: dark-mode ? #10151d : #eef2f5`
- `sidebar-panel-background: dark-mode ? #111821 : #f0f3f6`
- `right-panel-background: dark-mode ? #111821 : #f0f3f6`
- `terminal-frame-background: dark-mode ? #141b24 : #fafafa`
- `separator: dark-mode ? #1b2530 : #d8dee6`
- `text-primary: dark-mode ? #c5c1b8 : #5c6166`
- `text-secondary: dark-mode ? #9aa4ae : #7a838c`
- `text-muted: dark-mode ? #7d8790 : #8a939c`
- `accent: dark-mode ? #e6b450 : #ffaa33`

**Step 3: Keep the token file as parity defaults only**

Do not add a second Ayu shell ladder or a parallel runtime system. The file
should remain a static boot snapshot of the same Rust-authored preset, nothing
more.

**Step 4: Run the fallback and token-parity tests**

Run:

```bash
cargo test terminal_theme -- --nocapture
cargo test ui_preferences -- --nocapture
cargo test bootstrap_smoke -- --nocapture
```

Expected: PASS for token-parity, fallback, and no-surface theme-toggle
assertions.

**Step 5: Commit**

```bash
git add ui/theme/tokens.slint tests/terminal_theme_selection_spec.rs tests/ui_preferences.rs tests/bootstrap_smoke.rs
git commit -m "style: align ayu shell and terminal boot defaults"
```

### Task 6: Run the full Ayu terminal-neighborhood verification matrix

**Files:**
- No new source files required unless verification reveals a regression

**Step 1: Format the workspace**

Run:

```bash
cargo fmt
```

Expected: exit code `0`.

**Step 2: Run the focused terminal theme verification**

Run:

```bash
cargo test terminal_theme -- --nocapture
```

Expected: PASS with the new dark/light preset and token-parity coverage.

**Step 3: Run the UI preferences verification**

Run:

```bash
cargo test ui_preferences -- --nocapture
```

Expected: PASS with the no-surface and theme-toggle parity assertions.

**Step 4: Run the bootstrap smoke verification**

Run:

```bash
cargo test bootstrap_smoke -- --nocapture
```

Expected: PASS with runtime shell-palette and terminal-neighborhood projection
coverage.

**Step 5: Run the native terminal surface contract verification**

Run:

```bash
cargo test native_terminal_surface_contract_spec -- --nocapture
```

Expected: PASS with the session-host and runtime shell property contract
coverage.

**Step 6: Run the full test suite**

Run:

```bash
cargo test
```

Expected: PASS.

**Step 7: Commit**

```bash
git add .
git commit -m "feat: unify ayu terminal neighborhood palette"
```
