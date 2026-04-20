# Windows Startup Window Placement Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make the Windows main window open with a larger default size, center cleanly on first launch, restore the user's last restored bounds on later launches, and recover gracefully when saved bounds are off-screen.

**Architecture:** Persist restored main-window bounds in `UiPreferences`, add a small pure geometry resolver for centering and off-screen recovery, and wire startup plus move/resize persistence through the existing bootstrap/winit event flow. Use Windows monitor work areas for first-launch centering and fallback placement so the taskbar does not push the window partly off-screen.

**Tech Stack:** Rust, Slint, winit 0.30 via `slint::winit_030`, `windows-sys`/Win32 monitor APIs, serde JSON preferences, cargo test.

---

### Task 1: Persist main-window bounds in UI preferences

**Files:**
- Modify: `src/app/ui_preferences.rs`
- Test: `src/app/ui_preferences.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn ui_preferences_round_trip_preserves_window_bounds() {
    let prefs = UiPreferences {
        window_bounds: Some(PersistedWindowBounds {
            x: 160,
            y: 120,
            width: 1680,
            height: 980,
        }),
        ..UiPreferences::default()
    };

    let json = serde_json::to_string(&prefs).expect("serialize preferences");
    let decoded: UiPreferences = serde_json::from_str(&json).expect("deserialize preferences");

    assert_eq!(decoded.window_bounds, prefs.window_bounds);
}

#[test]
fn ui_preferences_defaults_to_no_window_bounds() {
    let decoded: UiPreferences = serde_json::from_str("{}")
        .expect("deserialize default preferences");

    assert_eq!(decoded.window_bounds, None);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test ui_preferences_round_trip_preserves_window_bounds --lib`
Expected: FAIL because `PersistedWindowBounds` and `window_bounds` do not exist yet.

**Step 3: Write minimal implementation**

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedWindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPreferences {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_right_panel_view")]
    pub right_panel_view: String,
    #[serde(default = "default_terminal_scrollback_limit")]
    pub terminal_scrollback_limit: usize,
    #[serde(default = "default_terminal_active_idle_shrink_enabled")]
    pub terminal_active_idle_shrink_enabled: bool,
    #[serde(default)]
    pub download_conflict_default: DownloadConflictDefault,
    #[serde(default)]
    pub window_bounds: Option<PersistedWindowBounds>,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            always_on_top: false,
            right_panel_view: default_right_panel_view(),
            terminal_scrollback_limit: default_terminal_scrollback_limit(),
            terminal_active_idle_shrink_enabled: default_terminal_active_idle_shrink_enabled(),
            download_conflict_default: DownloadConflictDefault::Ask,
            window_bounds: None,
        }
    }
}
```

Keep `From<&ShellViewModel> for UiPreferences` from overwriting existing bounds later by handling merge in bootstrap instead of trying to source bounds from the view model.

**Step 4: Run test to verify it passes**

Run: `cargo test ui_preferences_round_trip_preserves_window_bounds --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/ui_preferences.rs
git commit -m "feat: persist window bounds in ui preferences"
```

### Task 2: Add a pure startup geometry resolver

**Files:**
- Create: `src/app/window_geometry.rs`
- Modify: `src/app/mod.rs`
- Test: `src/app/window_geometry.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn resolve_startup_bounds_centers_first_launch() {
    let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];

    let resolved = resolve_startup_bounds(None, (1600, 960), &monitors)
        .expect("resolved bounds");

    assert_eq!(resolved.width, 1600);
    assert_eq!(resolved.height, 960);
    assert_eq!(resolved.x, 160);
    assert_eq!(resolved.y, 40);
}

#[test]
fn resolve_startup_bounds_rehomes_offscreen_saved_bounds() {
    let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
    let saved = PersistedWindowBounds {
        x: 4000,
        y: 2800,
        width: 1600,
        height: 960,
    };

    let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
        .expect("resolved bounds");

    assert_eq!(resolved.x, 160);
    assert_eq!(resolved.y, 40);
}

#[test]
fn resolve_startup_bounds_keeps_visible_saved_bounds() {
    let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
    let saved = PersistedWindowBounds {
        x: 120,
        y: 80,
        width: 1500,
        height: 900,
    };

    let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
        .expect("resolved bounds");

    assert_eq!(resolved, saved);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test resolve_startup_bounds_centers_first_launch --lib`
Expected: FAIL because `window_geometry` does not exist yet.

**Step 3: Write minimal implementation**

```rust
use crate::app::ui_preferences::PersistedWindowBounds;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorWorkArea {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl MonitorWorkArea {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

pub fn resolve_startup_bounds(
    saved: Option<PersistedWindowBounds>,
    desired_size: (u32, u32),
    monitors: &[MonitorWorkArea],
) -> Option<PersistedWindowBounds> {
    let target = monitors.first().copied()?;

    if let Some(saved) = saved
        && bounds_intersects_any_monitor(saved, monitors)
    {
        return Some(clamp_bounds_to_monitor(saved, target));
    }

    Some(center_bounds_in_monitor(desired_size, target))
}

fn center_bounds_in_monitor(
    desired_size: (u32, u32),
    monitor: MonitorWorkArea,
) -> PersistedWindowBounds {
    let width = desired_size.0.min(monitor.width);
    let height = desired_size.1.min(monitor.height);
    let x = monitor.x + ((monitor.width.saturating_sub(width)) / 2) as i32;
    let y = monitor.y + ((monitor.height.saturating_sub(height)) / 2) as i32;

    PersistedWindowBounds { x, y, width, height }
}
```

Also add helpers for intersection checks and clamping inside the same file, plus `pub mod window_geometry;` in `src/app/mod.rs`.

**Step 4: Run test to verify it passes**

Run: `cargo test resolve_startup_bounds_centers_first_launch --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/window_geometry.rs
git commit -m "feat: add startup window geometry resolver"
```

### Task 3: Preserve bounds when generic UI preference saves happen

**Files:**
- Modify: `src/app/bootstrap.rs`
- Test: `src/app/bootstrap.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn merge_ui_preferences_preserves_existing_window_bounds() {
    let existing = UiPreferences {
        window_bounds: Some(PersistedWindowBounds {
            x: 100,
            y: 80,
            width: 1500,
            height: 920,
        }),
        ..UiPreferences::default()
    };

    let next = UiPreferences {
        theme_mode: ThemeMode::Light,
        ..UiPreferences::default()
    };

    let merged = merge_ui_preferences(existing, next);

    assert_eq!(merged.theme_mode, ThemeMode::Light);
    assert_eq!(merged.window_bounds, existing.window_bounds);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test merge_ui_preferences_preserves_existing_window_bounds --lib`
Expected: FAIL because `merge_ui_preferences` does not exist.

**Step 3: Write minimal implementation**

```rust
fn merge_ui_preferences(existing: UiPreferences, mut next: UiPreferences) -> UiPreferences {
    if next.window_bounds.is_none() {
        next.window_bounds = existing.window_bounds;
    }
    next
}

fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store {
        let existing = store.load_or_default().unwrap_or_default();
        let next = merge_ui_preferences(existing, UiPreferences::from(state));
        if let Err(err) = store.save(&next) {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to save ui preferences"
            );
        }
    }
}
```

Add a small companion helper:

```rust
fn save_window_bounds_preference(
    store: &Option<Rc<UiPreferencesStore>>,
    bounds: PersistedWindowBounds,
) {
    if let Some(store) = store {
        let mut prefs = store.load_or_default().unwrap_or_default();
        prefs.window_bounds = Some(bounds);
        let _ = store.save(&prefs);
    }
}
```

**Step 4: Run test to verify it passes**

Run: `cargo test merge_ui_preferences_preserves_existing_window_bounds --lib`
Expected: PASS

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs
git commit -m "refactor: preserve window bounds across preference saves"
```

### Task 4: Apply startup bounds and persist restored bounds from window events

**Files:**
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/bootstrap/windowing.rs`
- Modify: `src/app/windows_frame.rs`
- Test: `src/app/window_geometry.rs`

**Step 1: Write the failing test**

```rust
#[test]
fn resolve_startup_bounds_clamps_saved_bounds_into_work_area() {
    let monitors = [MonitorWorkArea::new(0, 0, 1920, 1040)];
    let saved = PersistedWindowBounds {
        x: 600,
        y: 400,
        width: 1800,
        height: 980,
    };

    let resolved = resolve_startup_bounds(Some(saved), (1600, 960), &monitors)
        .expect("resolved bounds");

    assert_eq!(resolved.x, 120);
    assert_eq!(resolved.y, 60);
    assert_eq!(resolved.width, 1800);
    assert_eq!(resolved.height, 980);
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test resolve_startup_bounds_clamps_saved_bounds_into_work_area --lib`
Expected: FAIL because clamping logic is not complete yet.

**Step 3: Write minimal implementation**

```rust
#[cfg(target_os = "windows")]
fn windows_monitor_work_areas(
    window: &AppWindow,
) -> Vec<crate::app::window_geometry::MonitorWorkArea> {
    use slint::winit_030::WinitWindowAccessor;
    use slint::winit_030::winit::platform::windows::MonitorHandleExtWindows;

    let mut monitors = Vec::new();
    let _ = window.window().with_winit_window(|winit_window| {
        for monitor in winit_window.available_monitors() {
            if let Some(work_area) = work_area_from_hmonitor(monitor.hmonitor()) {
                monitors.push(work_area);
            }
        }
    });
    monitors
}

#[cfg(target_os = "windows")]
fn apply_startup_window_bounds(
    window: &AppWindow,
    prefs: &UiPreferences,
) {
    let desired_size = default_window_size();
    let monitors = windows_monitor_work_areas(window);
    let Some(bounds) = resolve_startup_bounds(prefs.window_bounds, desired_size, &monitors) else {
        apply_restored_window_size(window, desired_size);
        return;
    };

    window.window().set_size(slint::PhysicalSize::new(bounds.width, bounds.height));
    window.window().set_position(slint::WindowPosition::Physical(
        slint::PhysicalPosition::new(bounds.x, bounds.y),
    ));
}
```

In `src/app/windows_frame.rs`, add:

```rust
#[cfg(target_os = "windows")]
pub fn work_area_from_hmonitor(
    hmonitor: windows_sys::Win32::Graphics::Gdi::HMONITOR,
) -> Option<crate::app::window_geometry::MonitorWorkArea> {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO};

    unsafe {
        let mut monitor_info = MONITORINFO {
            cbSize: core::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: core::mem::zeroed(),
            rcWork: core::mem::zeroed(),
            dwFlags: 0,
        };
        if GetMonitorInfoW(hmonitor, &mut monitor_info) == 0 {
            return None;
        }
        let work = rect_from_win32_rect(monitor_info.rcWork)?;
        Some(crate::app::window_geometry::MonitorWorkArea::new(
            work.x,
            work.y,
            work.width,
            work.height,
        ))
    }
}
```

Then replace the current startup call in `src/app/bootstrap.rs`:

```rust
apply_startup_window_bounds(window, &prefs);
```

Finally, persist restored bounds from `Moved`, `Resized`, and `ScaleFactorChanged` events in
`src/app/bootstrap/windowing.rs` by querying the current outer position/size and skipping saves
while `query_true_window_placement(winit_window)` reports a maximized placement.

**Step 4: Run test to verify it passes**

Run: `cargo test resolve_startup_bounds_clamps_saved_bounds_into_work_area --lib`
Expected: PASS

Then run:

`cargo test --lib`

Expected: PASS for the touched modules.

**Step 5: Commit**

```bash
git add src/app/bootstrap.rs src/app/bootstrap/windowing.rs src/app/windows_frame.rs src/app/window_geometry.rs
git commit -m "feat: restore and center windows startup bounds on windows"
```

### Task 5: Final verification and Windows-focused manual sanity check

**Files:**
- Modify: none
- Test: existing Rust tests plus a Windows desktop build/manual run

**Step 1: Run the relevant automated verification**

```bash
cargo test --lib
```

**Step 2: Run a Windows build to catch integration issues**

```bash
./build-win-x64.sh
```

Expected: successful Windows build artifact generation.

**Step 3: Manually verify the user-visible behavior on Windows**

- Delete or rename the existing `ui-preferences.json` file and launch the app.
- Confirm first launch opens larger and centered within the work area.
- Move the window near a screen edge, close the app, relaunch, and confirm the location is restored.
- Simulate a monitor-layout change or force invalid saved coordinates, relaunch, and confirm the window re-centers into a visible work area.

**Step 4: Record any follow-up fixes only if verification fails**

If any check fails, return to the smallest relevant task above, add or tighten a test first, and then patch the implementation.

**Step 5: Commit**

```bash
git status
```

Expected: clean working tree after the implementation commits above.
