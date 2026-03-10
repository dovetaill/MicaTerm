# Overall Style Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build the initial Rust + Slint application shell and implement the approved Windows 11 Fluent overall style system for Mica Term, including theme tokens, frameless shell layout, signature surfaces, motion, and accessibility floor.

**Architecture:** Start from an empty repository and create a small Rust workspace with a binary entrypoint plus a library crate for theme, shell, windowing, and status state. Keep the UI declarative in `ui/` Slint modules, keep the testable style rules in Rust modules under `src/`, and wire them together through generated Slint components plus a thin app bootstrap layer. Implement only the approved first-release style surfaces; keep high-contrast completeness and premium-card variants out of scope.

**Tech Stack:** Rust, Cargo, Slint, slint-build, Tokio, window-vibrancy, raw-window-handle, serde, cargo fmt, cargo clippy

---

**Execution Notes**

- Use @superpowers:test-driven-development before each task and keep every task green before moving on.
- If any compile or test failure is not obvious, stop and use @superpowers:systematic-debugging instead of guessing.
- Before claiming the feature complete, use @superpowers:verification-before-completion and capture the exact command outputs.
- Execute this plan in a dedicated worktree rooted at `/home/wwwroot/mica-term`.

### Task 1: Bootstrap the Rust + Slint shell workspace

**Files:**
- Create: `.gitignore`
- Create: `Cargo.toml`
- Create: `build.rs`
- Create: `src/lib.rs`
- Create: `src/main.rs`
- Create: `src/app/mod.rs`
- Create: `src/app/bootstrap.rs`
- Create: `ui/app-window.slint`
- Test: `tests/bootstrap_smoke.rs`

**Step 1: Write the failing test**

Create `tests/bootstrap_smoke.rs`:

```rust
use mica_term::app::bootstrap::{app_title, default_window_size};

#[test]
fn bootstrap_exposes_app_title_and_default_window_size() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(default_window_size(), (1440, 900));
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test bootstrap_smoke -q`
Expected: FAIL with `could not find Cargo.toml` or unresolved crate `mica_term`.

**Step 3: Write minimal implementation**

Run:

```bash
cargo init --bin . --vcs none --name mica-term
cargo add slint tokio anyhow serde --features serde/derive
cargo add --build slint-build
mkdir -p src/app ui tests
```

Create `.gitignore`:

```gitignore
/target
/.idea
/.vscode
/.DS_Store
```

Replace `build.rs`:

```rust
fn main() {
    slint_build::compile("ui/app-window.slint").expect("failed to compile Slint UI");
}
```

Replace `src/lib.rs`:

```rust
pub mod app;

slint::include_modules!();
```

Create `src/app/mod.rs`:

```rust
pub mod bootstrap;
```

Create `src/app/bootstrap.rs`:

```rust
use anyhow::Result;

use crate::AppWindow;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn default_window_size() -> (u32, u32) {
    (1440, 900)
}

pub fn run() -> Result<()> {
    let window = AppWindow::new()?;
    window.run()?;
    Ok(())
}
```

Replace `src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    mica_term::app::bootstrap::run()
}
```

Create `ui/app-window.slint`:

```slint
export component AppWindow inherits Window {
    title: "Mica Term";
    preferred-width: 1440px;
    preferred-height: 900px;

    Text {
        text: "Mica Term";
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test bootstrap_smoke -q`
Expected: PASS with `1 passed; 0 failed`.

**Step 5: Commit**

```bash
git add .gitignore Cargo.toml build.rs src/lib.rs src/main.rs src/app/mod.rs src/app/bootstrap.rs ui/app-window.slint tests/bootstrap_smoke.rs
git commit -m "feat: bootstrap rust slint shell workspace"
```

### Task 2: Implement theme spec and design tokens

**Files:**
- Create: `src/theme/mod.rs`
- Create: `src/theme/spec.rs`
- Create: `ui/theme/tokens.slint`
- Modify: `src/lib.rs`
- Modify: `src/app/mod.rs`
- Modify: `ui/app-window.slint`
- Test: `tests/theme_spec.rs`

**Step 1: Write the failing test**

Create `tests/theme_spec.rs`:

```rust
use mica_term::theme::{theme_spec, ThemeMode};

#[test]
fn dark_theme_matches_tinted_console_rules() {
    let spec = theme_spec(ThemeMode::Dark);
    assert!(spec.terminal_is_neutral);
    assert!(spec.panel_uses_tint);
    assert_eq!(spec.accent_name, "electric-blue");
}

#[test]
fn light_theme_is_supported_from_day_one() {
    let spec = theme_spec(ThemeMode::Light);
    assert!(spec.supports_light);
    assert!(spec.supports_dark);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test theme_spec -q`
Expected: FAIL with `could not find theme in mica_term`.

**Step 3: Write minimal implementation**

Create `src/theme/mod.rs`:

```rust
mod spec;

pub use spec::{theme_spec, ThemeMode, ThemeSpec};
```

Create `src/theme/spec.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpec {
    pub accent_name: &'static str,
    pub terminal_is_neutral: bool,
    pub panel_uses_tint: bool,
    pub supports_dark: bool,
    pub supports_light: bool,
}

pub fn theme_spec(mode: ThemeMode) -> ThemeSpec {
    match mode {
        ThemeMode::Dark => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
        ThemeMode::Light => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
    }
}
```

Modify `src/app/mod.rs`:

```rust
pub mod bootstrap;
pub mod theme;
```

Modify `src/lib.rs`:

```rust
pub mod app;
pub mod theme;

slint::include_modules!();
```

Create `ui/theme/tokens.slint`:

```slint
export global ThemeTokens {
    in property <bool> dark-mode: true;

    property <brush> shell-surface: dark-mode ? #181a20 : #f5f7fb;
    property <brush> shell-stroke: dark-mode ? #ffffff12 : #0f172a12;
    property <brush> command-tint: dark-mode ? #1c2430ee : #eef5ffea;
    property <brush> panel-tint: dark-mode ? #1a2230ee : #f2f7ffee;
    property <brush> terminal-surface: dark-mode ? #14161b : #f8f8f9;
    property <brush> accent: #4ea1ff;
    property <brush> text-primary: dark-mode ? #f5f7fb : #101418;
}
```

Modify `ui/app-window.slint`:

```slint
import { ThemeTokens } from "theme/tokens.slint";

export component AppWindow inherits Window {
    in property <bool> dark-mode: true;
    title: "Mica Term";
    preferred-width: 1440px;
    preferred-height: 900px;

    ThemeTokens {
        dark-mode: root.dark-mode;
    }

    Rectangle {
        background: ThemeTokens.shell-surface;
        Text {
            text: "Mica Term";
            color: ThemeTokens.text-primary;
            horizontal-alignment: center;
            vertical-alignment: center;
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test theme_spec -q`
Expected: PASS with `2 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/lib.rs src/app/mod.rs src/theme/mod.rs src/theme/spec.rs ui/theme/tokens.slint ui/app-window.slint tests/theme_spec.rs
git commit -m "feat: add overall theme spec and tokens"
```

### Task 3: Implement frameless window shell and balanced desktop metrics

**Files:**
- Create: `src/app/windowing.rs`
- Create: `src/shell/mod.rs`
- Create: `src/shell/metrics.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/lib.rs`
- Modify: `ui/app-window.slint`
- Test: `tests/window_shell.rs`

**Step 1: Write the failing test**

Create `tests/window_shell.rs`:

```rust
use mica_term::app::windowing::{window_appearance, MaterialKind};
use mica_term::shell::metrics::ShellMetrics;

#[test]
fn balanced_desktop_metrics_match_the_design_doc() {
    assert_eq!(ShellMetrics::TITLEBAR_HEIGHT, 48);
    assert_eq!(ShellMetrics::ACTIVITY_BAR_WIDTH, 48);
    assert_eq!(ShellMetrics::ASSETS_SIDEBAR_WIDTH, 256);
    assert_eq!(ShellMetrics::TAB_BAR_HEIGHT, 38);
    assert_eq!(ShellMetrics::RIGHT_PANEL_WIDTH, 392);
}

#[test]
fn window_shell_prefers_frameless_mica_alt() {
    let appearance = window_appearance();
    assert!(appearance.no_frame);
    assert_eq!(appearance.material, MaterialKind::MicaAlt);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_shell -q`
Expected: FAIL with unresolved imports for `windowing` and `shell`.

**Step 3: Write minimal implementation**

Run:

```bash
cargo add window-vibrancy raw-window-handle
mkdir -p src/shell
```

Modify `src/app/mod.rs`:

```rust
pub mod bootstrap;
pub mod windowing;
```

Create `src/app/windowing.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaterialKind {
    MicaAlt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowAppearance {
    pub no_frame: bool,
    pub material: MaterialKind,
}

pub fn window_appearance() -> WindowAppearance {
    WindowAppearance {
        no_frame: true,
        material: MaterialKind::MicaAlt,
    }
}
```

Create `src/shell/mod.rs`:

```rust
pub mod metrics;
```

Create `src/shell/metrics.rs`:

```rust
pub struct ShellMetrics;

impl ShellMetrics {
    pub const TITLEBAR_HEIGHT: u32 = 48;
    pub const ACTIVITY_BAR_WIDTH: u32 = 48;
    pub const ASSETS_SIDEBAR_WIDTH: u32 = 256;
    pub const TAB_BAR_HEIGHT: u32 = 38;
    pub const RIGHT_PANEL_WIDTH: u32 = 392;
    pub const BASE_SPACING: u32 = 8;
}
```

Modify `src/lib.rs`:

```rust
pub mod app;
pub mod shell;
pub mod theme;

slint::include_modules!();
```

Modify `ui/app-window.slint`:

```slint
import { ThemeTokens } from "theme/tokens.slint";

export component AppWindow inherits Window {
    in property <bool> dark-mode: true;
    title: "Mica Term";
    no-frame: true;
    background: transparent;
    preferred-width: 1440px;
    preferred-height: 900px;

    ThemeTokens {
        dark-mode: root.dark-mode;
    }

    Rectangle {
        border-radius: 14px;
        border-width: 1px;
        border-color: ThemeTokens.shell-stroke;
        background: ThemeTokens.shell-surface;

        Text {
            text: "Mica Term";
            color: ThemeTokens.text-primary;
            horizontal-alignment: center;
            vertical-alignment: center;
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_shell -q`
Expected: PASS with `2 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/windowing.rs src/shell/mod.rs src/shell/metrics.rs src/lib.rs ui/app-window.slint tests/window_shell.rs
git commit -m "feat: add frameless shell metrics and window appearance config"
```

### Task 4: Build the shell layout and welcome workspace scaffolding

**Files:**
- Create: `src/shell/view_model.rs`
- Modify: `src/shell/mod.rs`
- Create: `ui/shell/titlebar.slint`
- Create: `ui/shell/sidebar.slint`
- Create: `ui/shell/tabbar.slint`
- Create: `ui/shell/right-panel.slint`
- Create: `ui/welcome/welcome-view.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/shell_view_model.rs`

**Step 1: Write the failing test**

Create `tests/shell_view_model.rs`:

```rust
use mica_term::shell::view_model::{welcome_actions, ShellViewModel, WelcomeAction};

#[test]
fn welcome_actions_match_the_approved_order() {
    assert_eq!(
        welcome_actions(),
        &[
            WelcomeAction::NewConnection,
            WelcomeAction::OpenRecent,
            WelcomeAction::Snippets,
            WelcomeAction::Sftp,
        ]
    );
}

#[test]
fn shell_view_model_starts_in_welcome_mode_with_right_panel_hidden() {
    let view_model = ShellViewModel::default();
    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test shell_view_model -q`
Expected: FAIL with unresolved import `view_model`.

**Step 3: Write minimal implementation**

Modify `src/shell/mod.rs`:

```rust
pub mod metrics;
pub mod view_model;
```

Create `src/shell/view_model.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    NewConnection,
    OpenRecent,
    Snippets,
    Sftp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
        }
    }
}

pub fn welcome_actions() -> &'static [WelcomeAction] {
    &[
        WelcomeAction::NewConnection,
        WelcomeAction::OpenRecent,
        WelcomeAction::Snippets,
        WelcomeAction::Sftp,
    ]
}
```

Create `ui/shell/titlebar.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component Titlebar inherits Rectangle {
    height: 48px;
    background: ThemeTokens.command-tint;
    border-radius: 12px;
}
```

Create `ui/shell/sidebar.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component Sidebar inherits Rectangle {
    width: 48px;
    background: ThemeTokens.shell-surface;
    border-right-width: 1px;
    border-right-color: ThemeTokens.shell-stroke;
}
```

Create `ui/shell/tabbar.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component TabBar inherits Rectangle {
    height: 38px;
    background: transparent;
    border-bottom-width: 1px;
    border-bottom-color: ThemeTokens.shell-stroke;
}
```

Create `ui/shell/right-panel.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component RightPanel inherits Rectangle {
    width: 392px;
    background: ThemeTokens.panel-tint;
    border-radius: 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
}
```

Create `ui/welcome/welcome-view.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component WelcomeView inherits Rectangle {
    background: transparent;

    VerticalLayout {
        alignment: center;
        spacing: 12px;

        Text {
            text: "Welcome to Mica Term";
            color: ThemeTokens.text-primary;
            font-size: 28px;
        }

        Text {
            text: "Command-first SSH and SFTP workspace";
            color: ThemeTokens.text-primary;
        }
    }
}
```

Modify `ui/app-window.slint`:

```slint
import { ThemeTokens } from "theme/tokens.slint";
import { Titlebar } from "shell/titlebar.slint";
import { Sidebar } from "shell/sidebar.slint";
import { TabBar } from "shell/tabbar.slint";
import { RightPanel } from "shell/right-panel.slint";
import { WelcomeView } from "welcome/welcome-view.slint";

export component AppWindow inherits Window {
    in property <bool> dark-mode: true;
    title: "Mica Term";
    no-frame: true;
    background: transparent;
    preferred-width: 1440px;
    preferred-height: 900px;

    ThemeTokens {
        dark-mode: root.dark-mode;
    }

    Rectangle {
        border-radius: 14px;
        border-width: 1px;
        border-color: ThemeTokens.shell-stroke;
        background: ThemeTokens.shell-surface;

        VerticalLayout {
            spacing: 0px;

            Titlebar {}

            HorizontalLayout {
                spacing: 0px;

                Sidebar {}

                VerticalLayout {
                    spacing: 0px;
                    TabBar {}
                    WelcomeView {}
                }

                RightPanel {}
            }
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test shell_view_model -q`
Expected: PASS with `2 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/shell/mod.rs src/shell/view_model.rs ui/shell/titlebar.slint ui/shell/sidebar.slint ui/shell/tabbar.slint ui/shell/right-panel.slint ui/welcome/welcome-view.slint ui/app-window.slint tests/shell_view_model.rs
git commit -m "feat: add shell layout and guided welcome scaffold"
```

### Task 5: Implement the first-release signature surfaces

**Files:**
- Create: `src/shell/signature.rs`
- Modify: `src/shell/mod.rs`
- Create: `ui/components/command-entry.slint`
- Create: `ui/components/active-tab.slint`
- Create: `ui/components/segmented-control.slint`
- Create: `ui/components/command-palette.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/shell/tabbar.slint`
- Modify: `ui/shell/right-panel.slint`
- Test: `tests/signature_surfaces.rs`

**Step 1: Write the failing test**

Create `tests/signature_surfaces.rs`:

```rust
use mica_term::shell::signature::{signature_surfaces, SignatureSurface};

#[test]
fn signature_surfaces_match_the_curated_highlights_set() {
    assert_eq!(
        signature_surfaces(),
        &[
            SignatureSurface::CommandEntry,
            SignatureSurface::ActiveTab,
            SignatureSurface::RightPanelSegmentedControl,
            SignatureSurface::WelcomeState,
            SignatureSurface::CommandPalette,
        ]
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test signature_surfaces -q`
Expected: FAIL with unresolved import `signature`.

**Step 3: Write minimal implementation**

Modify `src/shell/mod.rs`:

```rust
pub mod metrics;
pub mod signature;
pub mod view_model;
```

Create `src/shell/signature.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureSurface {
    CommandEntry,
    ActiveTab,
    RightPanelSegmentedControl,
    WelcomeState,
    CommandPalette,
}

pub fn signature_surfaces() -> &'static [SignatureSurface] {
    &[
        SignatureSurface::CommandEntry,
        SignatureSurface::ActiveTab,
        SignatureSurface::RightPanelSegmentedControl,
        SignatureSurface::WelcomeState,
        SignatureSurface::CommandPalette,
    ]
}
```

Create `ui/components/command-entry.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component CommandEntry inherits Rectangle {
    border-radius: 10px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.command-tint;
}
```

Create `ui/components/active-tab.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component ActiveTab inherits Rectangle {
    border-radius: 8px;
    background: ThemeTokens.accent;
}
```

Create `ui/components/segmented-control.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component SegmentedControl inherits Rectangle {
    border-radius: 10px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.panel-tint;
}
```

Create `ui/components/command-palette.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component CommandPalette inherits Rectangle {
    opacity: 0;
    border-radius: 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;
    background: ThemeTokens.command-tint;
}
```

Modify `ui/shell/titlebar.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";
import { CommandEntry } from "../components/command-entry.slint";

export component Titlebar inherits Rectangle {
    height: 48px;
    background: ThemeTokens.command-tint;
    border-radius: 12px;

    HorizontalLayout {
        padding: 8px;
        CommandEntry {
            width: 320px;
            height: 32px;
        }
    }
}
```

Modify `ui/shell/tabbar.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";
import { ActiveTab } from "../components/active-tab.slint";

export component TabBar inherits Rectangle {
    height: 38px;
    background: transparent;
    border-bottom-width: 1px;
    border-bottom-color: ThemeTokens.shell-stroke;

    HorizontalLayout {
        padding-left: 12px;
        ActiveTab {
            width: 140px;
            height: 28px;
        }
    }
}
```

Modify `ui/shell/right-panel.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";
import { SegmentedControl } from "../components/segmented-control.slint";

export component RightPanel inherits Rectangle {
    width: 392px;
    background: ThemeTokens.panel-tint;
    border-radius: 14px;
    border-width: 1px;
    border-color: ThemeTokens.shell-stroke;

    VerticalLayout {
        padding: 12px;
        SegmentedControl {
            height: 36px;
        }
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test signature_surfaces -q`
Expected: PASS with `1 passed; 0 failed`.

**Step 5: Commit**

```bash
git add src/shell/mod.rs src/shell/signature.rs ui/components/command-entry.slint ui/components/active-tab.slint ui/components/segmented-control.slint ui/components/command-palette.slint ui/shell/titlebar.slint ui/shell/tabbar.slint ui/shell/right-panel.slint tests/signature_surfaces.rs
git commit -m "feat: add curated highlight signature surfaces"
```

### Task 6: Add operational status, motion specs, and accessibility floor

**Files:**
- Create: `src/status/mod.rs`
- Create: `src/status/spec.rs`
- Create: `src/theme/accessibility.rs`
- Modify: `src/lib.rs`
- Create: `ui/theme/motion.slint`
- Create: `ui/components/status-pill.slint`
- Modify: `ui/app-window.slint`
- Test: `tests/status_motion.rs`
- Test: `tests/accessibility_floor.rs`

**Step 1: Write the failing tests**

Create `tests/status_motion.rs`:

```rust
use mica_term::status::{motion_spec, status_spec, ConnectionState};

#[test]
fn connecting_uses_low_noise_feedback() {
    let status = status_spec(ConnectionState::Connecting);
    assert!(status.animated);
    assert!(!status.escalates_to_page_overlay);
}

#[test]
fn right_panel_motion_matches_the_design_duration() {
    let motion = motion_spec();
    assert_eq!(motion.drawer_open_ms, 220);
    assert_eq!(motion.welcome_transition_ms, 160);
}
```

Create `tests/accessibility_floor.rs`:

```rust
use mica_term::theme::accessibility::accessibility_floor;

#[test]
fn accessibility_floor_requires_keyboard_reachability_and_readable_high_contrast() {
    let floor = accessibility_floor();
    assert!(floor.keyboard_reachable);
    assert!(floor.dark_light_focus_clear);
    assert!(floor.high_contrast_safe);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test status_motion --test accessibility_floor -q`
Expected: FAIL with unresolved imports for `status` and `accessibility`.

**Step 3: Write minimal implementation**

Create `src/status/mod.rs`:

```rust
mod spec;

pub use spec::{motion_spec, status_spec, ConnectionState, MotionSpec, StatusSpec};
```

Create `src/status/spec.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected,
    Disconnected,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusSpec {
    pub animated: bool,
    pub escalates_to_page_overlay: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionSpec {
    pub drawer_open_ms: u32,
    pub welcome_transition_ms: u32,
}

pub fn status_spec(state: ConnectionState) -> StatusSpec {
    match state {
        ConnectionState::Connecting => StatusSpec {
            animated: true,
            escalates_to_page_overlay: false,
        },
        ConnectionState::Connected => StatusSpec {
            animated: false,
            escalates_to_page_overlay: false,
        },
        ConnectionState::Disconnected | ConnectionState::Error => StatusSpec {
            animated: false,
            escalates_to_page_overlay: false,
        },
    }
}

pub fn motion_spec() -> MotionSpec {
    MotionSpec {
        drawer_open_ms: 220,
        welcome_transition_ms: 160,
    }
}
```

Create `src/theme/accessibility.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessibilityFloor {
    pub keyboard_reachable: bool,
    pub dark_light_focus_clear: bool,
    pub high_contrast_safe: bool,
}

pub fn accessibility_floor() -> AccessibilityFloor {
    AccessibilityFloor {
        keyboard_reachable: true,
        dark_light_focus_clear: true,
        high_contrast_safe: true,
    }
}
```

Modify `src/lib.rs`:

```rust
pub mod app;
pub mod shell;
pub mod status;
pub mod theme;

slint::include_modules!();
```

Create `ui/theme/motion.slint`:

```slint
export global MotionTokens {
    property <duration> drawer-duration: 220ms;
    property <duration> welcome-duration: 160ms;
}
```

Create `ui/components/status-pill.slint`:

```slint
import { ThemeTokens } from "../theme/tokens.slint";

export component StatusPill inherits Rectangle {
    in property <string> label: "Connected";
    border-radius: 999px;
    padding-left: 10px;
    padding-right: 10px;
    background: ThemeTokens.panel-tint;

    Text {
        text: root.label;
        color: ThemeTokens.text-primary;
    }
}
```

Modify `ui/app-window.slint` to import `MotionTokens` and `StatusPill`, then add:

```slint
import { MotionTokens } from "theme/motion.slint";
import { StatusPill } from "components/status-pill.slint";

// inside the main Rectangle content:
StatusPill {
    x: parent.width - 140px;
    y: 10px;
    label: "Connecting";
    animate opacity {
        duration: MotionTokens.welcome-duration;
    }
}
```

**Step 4: Run tests and verification commands**

Run:

```bash
cargo test --test status_motion --test accessibility_floor -q
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected:
- `cargo test`: PASS with `3 passed; 0 failed`
- `cargo fmt --check`: no diff output
- `cargo clippy`: no warnings

**Step 5: Run local visual smoke**

Run: `cargo run`
Expected:
- Window launches with frameless shell
- Titlebar height matches `48px`
- Right panel width matches `392px`
- Welcome state is visible
- Command-entry / active-tab / segmented-control surfaces are visibly differentiated

**Step 6: Commit**

```bash
git add src/status/mod.rs src/status/spec.rs src/theme/accessibility.rs src/lib.rs ui/theme/motion.slint ui/components/status-pill.slint ui/app-window.slint tests/status_motion.rs tests/accessibility_floor.rs
git commit -m "feat: add motion, status, and accessibility floor"
```

### Task 7: Capture the implementation verification notes

**Files:**
- Create: `docs/plans/2026-03-10-overall-style-verification.md`
- Modify: `readme.md`

**Step 1: Write the failing check**

Open `readme.md` and confirm it does not yet describe how to validate the style shell.

**Step 2: Run check to verify the gap exists**

Run: `rg -n "overall style|verification|mica term" readme.md docs/plans/2026-03-10-overall-style-verification.md`
Expected: FAIL or no matches because the verification note file does not exist yet.

**Step 3: Write minimal documentation**

Create `docs/plans/2026-03-10-overall-style-verification.md`:

```markdown
# Overall Style Verification

## Automated

- `cargo test`
- `cargo fmt --check`
- `cargo clippy --all-targets --all-features -- -D warnings`

## Visual Smoke

- Confirm frameless shell launches
- Confirm command-entry, active-tab, right-panel segmented control, welcome state, and command palette match the design doc
- Confirm dark mode is the primary polished path
- Confirm light mode remains legible and stable
- Confirm high-contrast mode does not break layout or erase focus visibility
```

Replace `readme.md`:

```markdown
# Mica Term

Project planning is in `docs/plans/`.

- Overall style design: `docs/plans/2026-03-10-overall-style-design.md`
- Overall style implementation plan: `docs/plans/2026-03-10-overall-style-implementation-plan.md`
- Overall style verification: `docs/plans/2026-03-10-overall-style-verification.md`
```

**Step 4: Run check to verify it passes**

Run: `rg -n "Overall style|verification" readme.md docs/plans/2026-03-10-overall-style-verification.md`
Expected: PASS with matches in both files.

**Step 5: Commit**

```bash
git add docs/plans/2026-03-10-overall-style-verification.md readme.md
git commit -m "docs: add overall style verification notes"
```
