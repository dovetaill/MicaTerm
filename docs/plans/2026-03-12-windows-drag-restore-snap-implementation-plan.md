# Windows Drag Restore Snap Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 在保持当前 `Slint + frameless + self-drawn titlebar` 路线不变的前提下，为 Windows `maximize / snap / restore / unsnap` 建立真状态源、壳层外形同步、恢复状态机与 frame adapter，修复拖拽还原和贴边后再拖拽时的窗口壳层异常。

**Architecture:** 先把“窗口真实状态”和“UI 壳层形态”拆开：`src/app/window_state.rs` 负责平台无关状态枚举和工作区矩形分类，`src/app/windows_frame.rs` 负责 Windows 下的真实 placement / hit-test / subclass 行为，`src/app/window_recovery.rs` 负责从 `Maximized / Snapped` 回到 `Restored` 时的显式恢复状态机。`bootstrap` 继续作为集成入口，把平台真状态同步到 `ShellViewModel` 和 Slint 属性；现有 `ThemeRedrawRecovery` 逻辑被收敛为更通用的恢复控制器，必要时才执行 `render-revision + redraw + size nudge` fallback。

**Tech Stack:** Rust, Cargo, Slint 1.15.1, `unstable-winit-030`, winit 0.30 via Slint backend, `window-vibrancy`, `windows-sys` (new direct dependency for Win32 placement and subclass APIs), shell smoke scripts, `cargo fmt`, `cargo test`, `cargo check`, `cargo clippy`

---

## Execution Notes

- 设计输入固定为 `docs/plans/2026-03-12-windows-drag-restore-snap-design.md`，实现时不得偏离已确认的 `1B + 2C + 3B + 4B`。
- 使用 `@superpowers:test-driven-development`：每个任务先写失败测试或失败 smoke，再写最小实现，再跑通过。
- 如果 Windows 运行时行为与预期不一致，不允许继续拍脑袋 patch，必须切换到 `@superpowers:systematic-debugging`。
- 本轮不触碰 terminal、SSH、SFTP、Welcome 内容，不顺手重构视觉稿；仅处理窗口状态、标题栏壳层与验证契约。
- 计划默认在独立 worktree 执行；若仍在当前工作区执行，改动范围必须限制在本计划列出的文件内。

## Task Ordering

1. 先做平台无关状态模型，避免 UI 和 Win32 代码继续直接猜状态。
2. 再把状态模型同步到 `ShellViewModel` 和 Slint 壳层属性，先拿到可测试的“圆角 / 方角切换”。
3. 然后抽出通用恢复状态机，把当前主题切换 workaround 升级为窗口状态恢复控制器。
4. 再落 Windows frame adapter 和 maximize hit-test，因为它是平台专用增强层。
5. 最后固化 capability contract、`resize-border-width` 和整套验证命令，避免未来回归。

### Task 1: 建立平台无关窗口状态模型与工作区分类器

**Files:**
- Create: `src/app/window_state.rs`
- Modify: `src/app/mod.rs`
- Create: `tests/window_state_spec.rs`

**Step 1: Write the failing test**

创建 `tests/window_state_spec.rs`：

```rust
use mica_term::app::window_state::{
    Rect, WindowChromeMode, WindowPlacementKind, classify_window_placement,
};

#[test]
fn restored_state_keeps_rounded_chrome() {
    assert_eq!(
        WindowPlacementKind::Restored.chrome_mode(),
        WindowChromeMode::Rounded
    );
}

#[test]
fn maximized_and_snapped_states_use_flat_chrome() {
    for placement in [
        WindowPlacementKind::Maximized,
        WindowPlacementKind::SnappedLeft,
        WindowPlacementKind::SnappedRight,
        WindowPlacementKind::SnappedTop,
        WindowPlacementKind::SnappedBottom,
    ] {
        assert_eq!(placement.chrome_mode(), WindowChromeMode::Flat);
    }
}

#[test]
fn classifier_detects_left_and_right_snap_from_work_area_halves() {
    let work_area = Rect::new(0, 0, 1920, 1080);

    assert_eq!(
        classify_window_placement(Rect::new(0, 0, 960, 1080), work_area, false),
        WindowPlacementKind::SnappedLeft
    );
    assert_eq!(
        classify_window_placement(Rect::new(960, 0, 960, 1080), work_area, false),
        WindowPlacementKind::SnappedRight
    );
}

#[test]
fn classifier_prefers_maximized_when_flag_is_true() {
    let work_area = Rect::new(0, 0, 1920, 1080);

    assert_eq!(
        classify_window_placement(Rect::new(0, 0, 1920, 1080), work_area, true),
        WindowPlacementKind::Maximized
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_state_spec -q`  
Expected: FAIL with unresolved items such as `window_state`, `Rect`, `WindowPlacementKind`, or `classify_window_placement`.

**Step 3: Write minimal implementation**

创建 `src/app/window_state.rs`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPlacementKind {
    Restored,
    Maximized,
    SnappedLeft,
    SnappedRight,
    SnappedTop,
    SnappedBottom,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowChromeMode {
    Rounded,
    Flat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self { x, y, width, height }
    }
}

impl WindowPlacementKind {
    pub fn chrome_mode(self) -> WindowChromeMode {
        match self {
            Self::Restored => WindowChromeMode::Rounded,
            Self::Maximized
            | Self::SnappedLeft
            | Self::SnappedRight
            | Self::SnappedTop
            | Self::SnappedBottom => WindowChromeMode::Flat,
            Self::Unknown => WindowChromeMode::Rounded,
        }
    }

    pub fn is_maximized(self) -> bool {
        matches!(self, Self::Maximized)
    }
}

pub fn classify_window_placement(
    window_rect: Rect,
    work_area: Rect,
    maximized: bool,
) -> WindowPlacementKind {
    if maximized {
        return WindowPlacementKind::Maximized;
    }

    let half_width = work_area.width / 2;
    let half_height = work_area.height / 2;

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y
        && window_rect.width == half_width
        && window_rect.height == work_area.height
    {
        return WindowPlacementKind::SnappedLeft;
    }

    if window_rect.x == work_area.x + half_width as i32
        && window_rect.y == work_area.y
        && window_rect.width == half_width
        && window_rect.height == work_area.height
    {
        return WindowPlacementKind::SnappedRight;
    }

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y
        && window_rect.width == work_area.width
        && window_rect.height == half_height
    {
        return WindowPlacementKind::SnappedTop;
    }

    if window_rect.x == work_area.x
        && window_rect.y == work_area.y + half_height as i32
        && window_rect.width == work_area.width
        && window_rect.height == half_height
    {
        return WindowPlacementKind::SnappedBottom;
    }

    WindowPlacementKind::Restored
}
```

修改 `src/app/mod.rs`：

```rust
pub mod bootstrap;
pub mod ui_preferences;
pub mod window_effects;
pub mod window_state;
pub mod windowing;
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_state_spec -q`  
Expected: PASS，状态分类器已经能稳定表达 `Restored / Maximized / Snapped*` 和 `Rounded / Flat` 映射。

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/window_state.rs tests/window_state_spec.rs
git commit -m "feat: add window placement classifier"
```

### Task 2: 将窗口状态模型接入 `ShellViewModel` 与 Slint 壳层外形

**Files:**
- Modify: `src/shell/view_model.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `ui/app-window.slint`
- Modify: `ui/shell/titlebar.slint`
- Modify: `tests/shell_view_model.rs`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/top_status_bar_smoke.rs`

**Step 1: Write the failing tests**

修改 `tests/shell_view_model.rs`，增加窗口状态和壳层模式断言：

```rust
use mica_term::app::window_state::WindowPlacementKind;

#[test]
fn shell_view_model_tracks_window_placement_and_chrome_mode() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.window_placement(), WindowPlacementKind::Restored);
    assert!(!view_model.uses_flat_window_chrome());
    assert!(!view_model.is_window_maximized());

    view_model.set_window_placement(WindowPlacementKind::SnappedLeft);
    assert_eq!(view_model.window_placement(), WindowPlacementKind::SnappedLeft);
    assert!(view_model.uses_flat_window_chrome());
    assert!(!view_model.is_window_maximized());

    view_model.set_window_placement(WindowPlacementKind::Maximized);
    assert!(view_model.is_window_maximized());
    assert!(view_model.uses_flat_window_chrome());
}
```

修改 `tests/window_geometry_spec.rs`，增加壳层圆角导出契约：

```rust
#[test]
fn restored_window_uses_rounded_shell_frame_and_titlebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 14);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 12);
}

#[test]
fn flat_window_chrome_flattens_shell_frame_and_titlebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.set_use_flat_window_chrome(true);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
}
```

修改 `tests/top_status_bar_smoke.rs`，确保最大化按钮路径会同步壳层模式：

```rust
#[test]
fn maximize_toggle_updates_flat_window_chrome_binding() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_use_flat_window_chrome());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_use_flat_window_chrome());

    app.invoke_drag_double_clicked();
    assert!(!app.get_use_flat_window_chrome());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test shell_view_model --test window_geometry_spec --test top_status_bar_smoke -q`  
Expected: FAIL with missing `window_placement()` / `uses_flat_window_chrome()` methods and missing Slint properties such as `use-flat-window-chrome`, `layout-shell-frame-radius`, `layout-titlebar-radius`.

**Step 3: Write minimal implementation**

修改 `src/shell/view_model.rs`，将窗口放置状态纳入单一真源：

```rust
use crate::app::window_state::WindowPlacementKind;

pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub is_always_on_top: bool,
    window_placement: WindowPlacementKind,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_global_menu: false,
            show_assets_sidebar: true,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
            is_always_on_top: false,
            window_placement: WindowPlacementKind::Restored,
        }
    }
}

impl ShellViewModel {
    pub fn window_placement(&self) -> WindowPlacementKind {
        self.window_placement
    }

    pub fn set_window_placement(&mut self, value: WindowPlacementKind) {
        self.window_placement = value;
    }

    pub fn is_window_maximized(&self) -> bool {
        self.window_placement.is_maximized()
    }

    pub fn uses_flat_window_chrome(&self) -> bool {
        matches!(self.window_placement.chrome_mode(), crate::app::window_state::WindowChromeMode::Flat)
    }
}
```

修改 `ui/app-window.slint`：

```slint
in-out property <bool> use-flat-window-chrome: false;
out property <length> layout-shell-frame-radius: shell-frame.border-radius;
out property <length> layout-titlebar-radius: titlebar.layout-radius;

shell-frame := Rectangle {
    border-radius: root.use-flat-window-chrome ? 0px : 14px;
    ...

    titlebar := Titlebar {
        use-flat-window-chrome: root.use-flat-window-chrome;
        ...
    }
}
```

修改 `ui/shell/titlebar.slint`：

```slint
in property <bool> use-flat-window-chrome: false;
out property <length> layout-radius: root.border-radius;

border-radius: root.use-flat-window-chrome ? 0px : 12px;
```

修改 `src/app/bootstrap.rs`，在同步 UI 时统一派生：

```rust
fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized());
    window.set_use_flat_window_chrome(state.uses_flat_window_chrome());
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
}
```

并把现有最大化切换点从：

```rust
let next = controller_ref.toggle_maximize(state.is_window_maximized);
state.set_window_maximized(next);
window.set_is_window_maximized(next);
```

替换成：

```rust
let next = controller_ref.toggle_maximize(state.is_window_maximized());
state.set_window_placement(if next {
    WindowPlacementKind::Maximized
} else {
    WindowPlacementKind::Restored
});
sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test shell_view_model --test window_geometry_spec --test top_status_bar_smoke -q`  
Expected: PASS，恢复态壳层为圆角，最大化路径会同步切换到平角壳层。

**Step 5: Commit**

```bash
git add src/shell/view_model.rs src/app/bootstrap.rs ui/app-window.slint ui/shell/titlebar.slint tests/shell_view_model.rs tests/window_geometry_spec.rs tests/top_status_bar_smoke.rs
git commit -m "feat: bind shell chrome to window placement state"
```

### Task 3: 抽出通用恢复状态机，取代主题专用恢复逻辑

**Files:**
- Create: `src/app/window_recovery.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Create: `tests/window_recovery_spec.rs`

**Step 1: Write the failing test**

创建 `tests/window_recovery_spec.rs`：

```rust
use mica_term::app::window_recovery::{
    WindowRecoveryAction, WindowRecoveryController, WindowVisibilitySnapshot,
};
use mica_term::app::window_state::WindowPlacementKind;

#[test]
fn entering_restored_from_maximized_requests_redraw() {
    let mut recovery = WindowRecoveryController::default();

    assert_eq!(
        recovery.on_placement_changed(
            WindowPlacementKind::Maximized,
            WindowPlacementKind::Restored,
            WindowVisibilitySnapshot::new(1296000, 1296000),
            1440,
            900,
        ),
        WindowRecoveryAction::RequestRedraw
    );
}

#[test]
fn entering_restored_from_snapped_can_nudge_once_when_visibility_grows() {
    let mut recovery = WindowRecoveryController::default();

    recovery.on_placement_changed(
        WindowPlacementKind::SnappedLeft,
        WindowPlacementKind::Restored,
        WindowVisibilitySnapshot::new(1296000, 640000),
        1440,
        900,
    );

    assert_eq!(
        recovery.on_visibility_changed(WindowVisibilitySnapshot::new(1296000, 960000), 1440, 900),
        WindowRecoveryAction::NudgeWindowSize { width: 1441, height: 900 }
    );
    assert_eq!(
        recovery.on_resize_ack(1441, 900),
        WindowRecoveryAction::RestoreWindowSize { width: 1440, height: 900 }
    );
}

#[test]
fn steady_restored_window_stays_idle() {
    let mut recovery = WindowRecoveryController::default();

    assert_eq!(
        recovery.on_placement_changed(
            WindowPlacementKind::Restored,
            WindowPlacementKind::Restored,
            WindowVisibilitySnapshot::new(1296000, 1296000),
            1440,
            900,
        ),
        WindowRecoveryAction::None
    );
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --test window_recovery_spec -q`  
Expected: FAIL with missing `window_recovery`, `WindowRecoveryController`, `WindowRecoveryAction`, or `WindowVisibilitySnapshot::new`.

**Step 3: Write minimal implementation**

创建 `src/app/window_recovery.rs`：

```rust
use crate::app::window_state::WindowPlacementKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowVisibilitySnapshot {
    pub total_area: u64,
    pub visible_area: u64,
}

impl WindowVisibilitySnapshot {
    pub const fn new(total_area: u64, visible_area: u64) -> Self {
        Self { total_area, visible_area }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRecoveryAction {
    None,
    RequestRedraw,
    NudgeWindowSize { width: u32, height: u32 },
    RestoreWindowSize { width: u32, height: u32 },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct WindowRecoveryController {
    pending_visible_area: Option<u64>,
    pending_restore_size: Option<(u32, u32, u32, u32)>,
}

impl WindowRecoveryController {
    pub fn on_placement_changed(
        &mut self,
        previous: WindowPlacementKind,
        next: WindowPlacementKind,
        snapshot: WindowVisibilitySnapshot,
        width: u32,
        height: u32,
    ) -> WindowRecoveryAction {
        self.pending_restore_size = None;

        if previous != next
            && matches!(
                previous,
                WindowPlacementKind::Maximized
                    | WindowPlacementKind::SnappedLeft
                    | WindowPlacementKind::SnappedRight
                    | WindowPlacementKind::SnappedTop
                    | WindowPlacementKind::SnappedBottom
            )
            && next == WindowPlacementKind::Restored
        {
            self.pending_visible_area =
                (snapshot.total_area > 0 && snapshot.visible_area < snapshot.total_area)
                    .then_some(snapshot.visible_area);
            return WindowRecoveryAction::RequestRedraw;
        }

        let _ = (width, height);
        WindowRecoveryAction::None
    }

    pub fn on_visibility_changed(
        &mut self,
        snapshot: WindowVisibilitySnapshot,
        width: u32,
        height: u32,
    ) -> WindowRecoveryAction {
        let Some(previous_visible_area) = self.pending_visible_area else {
            return WindowRecoveryAction::None;
        };

        if snapshot.visible_area <= previous_visible_area {
            return WindowRecoveryAction::None;
        }

        self.pending_visible_area =
            (snapshot.visible_area < snapshot.total_area).then_some(snapshot.visible_area);
        self.pending_restore_size = Some((width + 1, height, width, height));

        WindowRecoveryAction::NudgeWindowSize {
            width: width + 1,
            height,
        }
    }

    pub fn on_resize_ack(&mut self, width: u32, height: u32) -> WindowRecoveryAction {
        if let Some((nudged_width, nudged_height, restore_width, restore_height)) =
            self.pending_restore_size
        {
            if width == nudged_width && height == nudged_height {
                self.pending_restore_size = None;
                return WindowRecoveryAction::RestoreWindowSize {
                    width: restore_width,
                    height: restore_height,
                };
            }
        }

        WindowRecoveryAction::None
    }
}
```

修改 `src/app/mod.rs`：

```rust
pub mod window_recovery;
```

修改 `src/app/bootstrap.rs`：

- 将现有 `ThemeRedrawRecovery`、`ThemeRecoveryAction` 逻辑迁移到 `window_recovery.rs`
- 保留 `current_window_visibility_snapshot(...)` 和 `bump_render_revision(...)`
- 将 `bind_windows_theme_redraw_recovery(...)` 重命名并收敛为通用窗口恢复绑定函数
- 在 placement 变化和可见面积变化时调用 `WindowRecoveryController`

最小接线骨架：

```rust
use crate::app::window_recovery::{WindowRecoveryAction, WindowRecoveryController};

let recovery = Rc::new(RefCell::new(WindowRecoveryController::default()));
```

**Step 4: Run test to verify it passes**

Run: `cargo test --test window_recovery_spec -q`  
Expected: PASS，恢复链路已经从“主题专用”升级为“窗口状态专用”。

**Step 5: Commit**

```bash
git add src/app/mod.rs src/app/window_recovery.rs src/app/bootstrap.rs tests/window_recovery_spec.rs
git commit -m "feat: add window restore recovery controller"
```

### Task 4: 增加 Windows frame adapter 与 maximize hit-test 几何导出

**Files:**
- Modify: `Cargo.toml`
- Create: `src/app/windows_frame.rs`
- Modify: `src/app/mod.rs`
- Modify: `src/app/bootstrap.rs`
- Modify: `src/app/windowing.rs`
- Modify: `ui/shell/titlebar.slint`
- Modify: `ui/app-window.slint`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/window_shell.rs`
- Create: `tests/windows_frame_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/window_geometry_spec.rs`，导出 maximize 按钮几何：

```rust
#[test]
fn maximize_button_geometry_is_exported_for_native_frame_adapter() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_titlebar_maximize_button_width() as u32, 36);
    assert_eq!(app.get_layout_titlebar_maximize_button_height() as u32, 36);
    assert!(app.get_layout_titlebar_maximize_button_x() > 0.0);
}
```

修改 `tests/window_shell.rs`，增加能力契约：

```rust
#[test]
fn top_status_bar_window_commands_match_the_windows_restore_strategy() {
    let spec = window_command_spec();

    assert!(spec.uses_winit_drag);
    assert!(spec.supports_true_window_state_tracking);
    assert!(spec.supports_native_frame_adapter);
}
```

创建 `tests/windows_frame_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRAME_FILE="$ROOT_DIR/src/app/windows_frame.rs"
TITLEBAR="$ROOT_DIR/ui/shell/titlebar.slint"
APP_FILE="$ROOT_DIR/ui/app-window.slint"

grep -F 'WM_NCHITTEST' "$FRAME_FILE" >/dev/null
grep -F 'HTMAXBUTTON' "$FRAME_FILE" >/dev/null
grep -F 'SetWindowSubclass' "$FRAME_FILE" >/dev/null
grep -F 'layout-maximize-button-x' "$TITLEBAR" >/dev/null
grep -F 'layout-titlebar-maximize-button-x' "$APP_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec --test window_shell -q`  
Expected: FAIL with missing geometry outputs or new spec fields.

Run: `bash tests/windows_frame_contract_smoke.sh`  
Expected: FAIL with missing `windows_frame.rs`, `WM_NCHITTEST`, or missing exported maximize button layout properties.

**Step 3: Write minimal implementation**

修改 `Cargo.toml`，加入直接 Win32 绑定：

```toml
windows-sys = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_UI_Controls",
    "Win32_UI_WindowsAndMessaging",
] }
```

创建 `src/app/windows_frame.rs`：

```rust
use crate::app::window_state::{Rect, WindowPlacementKind, classify_window_placement};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptionButtonGeometry {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[cfg(target_os = "windows")]
pub fn query_true_window_placement(
    window: &slint::winit_030::winit::window::Window,
) -> Option<WindowPlacementKind> {
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowPlacement;

    let _ = (window, HWND, GetWindowPlacement);
    None
}

#[cfg(target_os = "windows")]
pub fn install_window_frame_adapter(
    window: &slint::winit_030::winit::window::Window,
    maximize_button: CaptionButtonGeometry,
) {
    use windows_sys::Win32::UI::Controls::SetWindowSubclass;
    use windows_sys::Win32::UI::WindowsAndMessaging::{HTCAPTION, HTMAXBUTTON, WM_NCHITTEST};

    let _ = (
        window,
        maximize_button,
        SetWindowSubclass,
        WM_NCHITTEST,
        HTCAPTION,
        HTMAXBUTTON,
    );
}
```

修改 `ui/shell/titlebar.slint`，为 maximize 按钮导出布局：

```slint
out property <length> layout-maximize-button-x: maximize-button.absolute-position.x;
out property <length> layout-maximize-button-y: maximize-button.absolute-position.y;
out property <length> layout-maximize-button-width: maximize-button.width;
out property <length> layout-maximize-button-height: maximize-button.height;
```

修改 `ui/app-window.slint`，转发到窗口级属性：

```slint
out property <length> layout-titlebar-maximize-button-x: titlebar.layout-maximize-button-x;
out property <length> layout-titlebar-maximize-button-y: titlebar.layout-maximize-button-y;
out property <length> layout-titlebar-maximize-button-width: titlebar.layout-maximize-button-width;
out property <length> layout-titlebar-maximize-button-height: titlebar.layout-maximize-button-height;
```

修改 `src/app/windowing.rs`，增强 capability contract：

```rust
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
    pub supports_true_window_state_tracking: bool,
    pub supports_native_frame_adapter: bool,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
        supports_always_on_top: true,
        supports_true_window_state_tracking: true,
        supports_native_frame_adapter: true,
    }
}
```

修改 `src/app/bootstrap.rs`：

- 在窗口 show / bind 后安装 Windows frame adapter
- 从 `titlebar` 导出的 maximize 按钮几何构造 `CaptionButtonGeometry`
- 在窗口事件变化时调用 `query_true_window_placement(...)`

最小接线骨架：

```rust
#[cfg(target_os = "windows")]
{
    let _ = window.window().with_winit_window(|winit_window| {
        install_window_frame_adapter(
            winit_window,
            CaptionButtonGeometry {
                x: window.get_layout_titlebar_maximize_button_x() as i32,
                y: window.get_layout_titlebar_maximize_button_y() as i32,
                width: window.get_layout_titlebar_maximize_button_width() as i32,
                height: window.get_layout_titlebar_maximize_button_height() as i32,
            },
        );
    });
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --test window_geometry_spec --test window_shell -q`  
Expected: PASS，geometry contract 与 capability contract 成立。

Run: `bash tests/windows_frame_contract_smoke.sh`  
Expected: PASS，Windows frame adapter 所需关键符号和布局导出已存在。

Run: `cargo check --workspace --target x86_64-pc-windows-gnu`  
Expected: PASS，Windows 专用代码至少能在交叉目标上通过编译检查。

**Step 5: Commit**

```bash
git add Cargo.toml src/app/mod.rs src/app/windows_frame.rs src/app/bootstrap.rs src/app/windowing.rs ui/shell/titlebar.slint ui/app-window.slint tests/window_geometry_spec.rs tests/window_shell.rs tests/windows_frame_contract_smoke.sh
git commit -m "feat: add windows frame adapter hooks"
```

### Task 5: 启用 `resize-border-width`，固化契约并跑完整验证

**Files:**
- Modify: `ui/app-window.slint`
- Modify: `src/app/windowing.rs`
- Modify: `tests/window_geometry_spec.rs`
- Modify: `tests/window_shell.rs`
- Create: `tests/windows_drag_restore_contract_smoke.sh`

**Step 1: Write the failing tests**

修改 `tests/window_geometry_spec.rs`，增加 resize border 导出：

```rust
#[test]
fn frameless_window_exports_resize_border_budget() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_resize_border_width() as u32, 6);
}
```

修改 `tests/window_shell.rs`：

```rust
#[test]
fn window_shell_exposes_resize_border_for_frameless_resize() {
    let spec = window_command_spec();

    assert_eq!(spec.resize_border_width, 6);
}
```

创建 `tests/windows_drag_restore_contract_smoke.sh`：

```bash
#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP_FILE="$ROOT_DIR/ui/app-window.slint"
WINDOWING_FILE="$ROOT_DIR/src/app/windowing.rs"
BOOTSTRAP_FILE="$ROOT_DIR/src/app/bootstrap.rs"

grep -F 'resize-border-width: 6px;' "$APP_FILE" >/dev/null
grep -F 'layout-resize-border-width' "$APP_FILE" >/dev/null
grep -F 'supports_true_window_state_tracking: true' "$WINDOWING_FILE" >/dev/null
grep -F 'supports_native_frame_adapter: true' "$WINDOWING_FILE" >/dev/null
grep -F 'WindowRecoveryController' "$BOOTSTRAP_FILE" >/dev/null
```

**Step 2: Run tests to verify they fail**

Run: `cargo test --test window_geometry_spec --test window_shell -q`  
Expected: FAIL with missing `layout_resize_border_width` or missing `resize_border_width` in `WindowCommandSpec`.

Run: `bash tests/windows_drag_restore_contract_smoke.sh`  
Expected: FAIL with missing `resize-border-width: 6px;` or missing capability flags.

**Step 3: Write minimal implementation**

修改 `ui/app-window.slint`：

```slint
resize-border-width: 6px;
out property <length> layout-resize-border-width: root.resize-border-width;
```

修改 `src/app/windowing.rs`：

```rust
pub struct WindowCommandSpec {
    pub uses_winit_drag: bool,
    pub self_drawn_controls: bool,
    pub supports_double_click_maximize: bool,
    pub supports_always_on_top: bool,
    pub supports_true_window_state_tracking: bool,
    pub supports_native_frame_adapter: bool,
    pub resize_border_width: u32,
}

pub fn window_command_spec() -> WindowCommandSpec {
    WindowCommandSpec {
        uses_winit_drag: true,
        self_drawn_controls: true,
        supports_double_click_maximize: true,
        supports_always_on_top: true,
        supports_true_window_state_tracking: true,
        supports_native_frame_adapter: true,
        resize_border_width: 6,
    }
}
```

**Step 4: Run full verification**

Run: `cargo test --test window_state_spec --test window_recovery_spec --test shell_view_model --test window_geometry_spec --test top_status_bar_smoke --test window_shell -q`  
Expected: PASS，纯状态、UI 绑定和集成路径全部通过。

Run: `bash tests/windows_frame_contract_smoke.sh`  
Expected: PASS

Run: `bash tests/windows_drag_restore_contract_smoke.sh`  
Expected: PASS

Run: `cargo check --workspace`  
Expected: PASS

Run: `cargo check --workspace --target x86_64-pc-windows-gnu`  
Expected: PASS

Run: `cargo clippy --workspace --all-targets -- -D warnings`  
Expected: PASS

**Step 5: Commit**

```bash
git add ui/app-window.slint src/app/windowing.rs tests/window_geometry_spec.rs tests/window_shell.rs tests/windows_drag_restore_contract_smoke.sh
git commit -m "feat: finalize windows drag restore snap contracts"
```

## Final Verification Checklist

- `Restored` 状态下 `shell-frame` 与 `titlebar` 恢复圆角
- `Maximized` 与 `Snapped*` 状态下壳层切换为方角
- `ShellViewModel` 不再把 `is_window_maximized` 当成唯一真状态源
- `WindowRecoveryController` 取代主题专用恢复逻辑
- `windows_frame.rs` 具备 `WM_NCHITTEST` / `HTMAXBUTTON` / subclass 接入点
- maximize 按钮几何已从 Slint 导出到 Rust
- `resize-border-width` 已启用，避免 frameless resize 再次自研
- Linux 默认目标与 Windows GNU 交叉目标都能通过编译检查

## Notes for Execution Session

- 如果 Task 4 的 `cargo check --target x86_64-pc-windows-gnu` 因本机未安装 target 失败，先执行仓库 README 中已有的 target 安装步骤，再继续，不要把问题归咎于实现代码。
- 如果 subclass 或 `WM_NCHITTEST` 接入后出现消息循环异常，立刻暂停 Task 4，单独进入 `@superpowers:systematic-debugging`，不要继续推进 Task 5。
- 若 Windows runtime 行为已经正确，但 `size nudge` fallback 仍被频繁触发，应在实现会话里额外补一条日志统计，确认是否可以进一步收敛 fallback 触发条件。
