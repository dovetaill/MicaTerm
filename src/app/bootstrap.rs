use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::AppWindow;
use crate::app::runtime_profile::AppRuntimeProfile;
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
use crate::app::windowing::{WindowController, apply_restored_window_size, window_appearance};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for};
use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn runtime_window_title(_profile: AppRuntimeProfile) -> String {
    app_title().to_owned()
}

pub fn startup_failure_message(_profile: AppRuntimeProfile, _err: &str) -> Option<String> {
    None
}

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
impl WindowRect {
    fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn area(self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
impl MonitorRect {
    fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    fn right(self) -> i64 {
        i64::from(self.x) + i64::from(self.width)
    }

    fn bottom(self) -> i64 {
        i64::from(self.y) + i64::from(self.height)
    }
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowVisibilitySnapshot {
    total_area: u64,
    visible_area: u64,
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
impl WindowVisibilitySnapshot {
    fn from_rects(window: WindowRect, monitors: &[MonitorRect]) -> Self {
        let visible_area = monitors
            .iter()
            .map(|monitor| intersection_area(window, *monitor))
            .sum();

        Self {
            total_area: window.area(),
            visible_area,
        }
    }
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeRecoveryAction {
    None,
    NudgeWindowSize { width: u32, height: u32 },
    RestoreWindowSize { width: u32, height: u32 },
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingWindowSizeRestore {
    nudged_width: u32,
    nudged_height: u32,
    restore_width: u32,
    restore_height: u32,
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct ThemeRedrawRecovery {
    pending_visible_area: Option<u64>,
    pending_restore_size: Option<PendingWindowSizeRestore>,
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
impl ThemeRedrawRecovery {
    fn mark_theme_toggle(&mut self, snapshot: WindowVisibilitySnapshot) {
        self.pending_restore_size = None;
        self.pending_visible_area = (snapshot.total_area > 0
            && snapshot.visible_area < snapshot.total_area)
            .then_some(snapshot.visible_area);
    }

    fn next_action(
        &mut self,
        snapshot: WindowVisibilitySnapshot,
        window_width: u32,
        window_height: u32,
        window_maximized: bool,
    ) -> ThemeRecoveryAction {
        if let Some(pending_restore) = self.pending_restore_size {
            if window_width == pending_restore.nudged_width
                && window_height == pending_restore.nudged_height
            {
                self.pending_restore_size = None;
                return ThemeRecoveryAction::RestoreWindowSize {
                    width: pending_restore.restore_width,
                    height: pending_restore.restore_height,
                };
            }

            if window_width != pending_restore.restore_width
                || window_height != pending_restore.restore_height
            {
                self.pending_restore_size = None;
            }

            return ThemeRecoveryAction::None;
        }

        let Some(previous_visible_area) = self.pending_visible_area else {
            return ThemeRecoveryAction::None;
        };

        if snapshot.visible_area <= previous_visible_area {
            return ThemeRecoveryAction::None;
        }

        if window_maximized {
            self.pending_visible_area =
                (snapshot.visible_area < snapshot.total_area).then_some(snapshot.visible_area);
            return ThemeRecoveryAction::None;
        }

        let Some((nudged_width, nudged_height)) = nudged_window_size(window_width, window_height)
        else {
            self.pending_visible_area =
                (snapshot.visible_area < snapshot.total_area).then_some(snapshot.visible_area);
            return ThemeRecoveryAction::None;
        };

        self.pending_visible_area =
            (snapshot.visible_area < snapshot.total_area).then_some(snapshot.visible_area);

        self.pending_restore_size = Some(PendingWindowSizeRestore {
            nudged_width,
            nudged_height,
            restore_width: window_width,
            restore_height: window_height,
        });

        ThemeRecoveryAction::NudgeWindowSize {
            width: nudged_width,
            height: nudged_height,
        }
    }
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn nudged_window_size(window_width: u32, window_height: u32) -> Option<(u32, u32)> {
    if let Some(width) = window_width.checked_add(1) {
        return Some((width, window_height));
    }

    window_height
        .checked_add(1)
        .map(|height| (window_width, height))
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn intersection_area(window: WindowRect, monitor: MonitorRect) -> u64 {
    let left = i64::from(window.x).max(i64::from(monitor.x));
    let top = i64::from(window.y).max(i64::from(monitor.y));
    let right = window.right().min(monitor.right());
    let bottom = window.bottom().min(monitor.bottom());

    if right <= left || bottom <= top {
        return 0;
    }

    let width = u64::try_from(right - left).unwrap_or_default();
    let height = u64::try_from(bottom - top).unwrap_or_default();
    width * height
}

#[cfg(target_os = "windows")]
fn current_window_visibility_snapshot(
    window: &slint::winit_030::winit::window::Window,
) -> WindowVisibilitySnapshot {
    let position = window
        .outer_position()
        .unwrap_or(slint::winit_030::winit::dpi::PhysicalPosition::new(0, 0));
    let size = window.outer_size();
    let monitors: Vec<_> = window
        .available_monitors()
        .map(|monitor| {
            let position = monitor.position();
            let size = monitor.size();
            MonitorRect::new(position.x, position.y, size.width, size.height)
        })
        .collect();

    WindowVisibilitySnapshot::from_rects(
        WindowRect::new(position.x, position.y, size.width, size.height),
        &monitors,
    )
}

#[cfg(target_os = "windows")]
fn mark_windows_theme_redraw_recovery(
    window: &AppWindow,
    recovery: &Rc<RefCell<ThemeRedrawRecovery>>,
) {
    use slint::winit_030::WinitWindowAccessor;

    let _ = window.window().with_winit_window(|winit_window| {
        recovery
            .borrow_mut()
            .mark_theme_toggle(current_window_visibility_snapshot(winit_window));
    });
}

#[cfg(not(target_os = "windows"))]
fn mark_windows_theme_redraw_recovery(
    _window: &AppWindow,
    _recovery: &Rc<RefCell<ThemeRedrawRecovery>>,
) {
}

#[cfg(target_os = "windows")]
fn bind_windows_theme_redraw_recovery(
    window: &AppWindow,
    recovery: Rc<RefCell<ThemeRedrawRecovery>>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    window
        .window()
        .on_winit_window_event(move |slint_window, event| {
            let should_check_visibility = matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            );

            if should_check_visibility {
                let action = slint_window
                    .with_winit_window(|winit_window| {
                        let size = winit_window.inner_size();
                        recovery.borrow_mut().next_action(
                            current_window_visibility_snapshot(winit_window),
                            size.width,
                            size.height,
                            winit_window.is_maximized(),
                        )
                    })
                    .unwrap_or(ThemeRecoveryAction::None);

                match action {
                    ThemeRecoveryAction::None => {}
                    ThemeRecoveryAction::NudgeWindowSize { width, height } => {
                        let window = handle.unwrap();
                        bump_render_revision(&window);
                        slint_window.request_redraw();
                        let _ = slint_window.with_winit_window(|winit_window| {
                            winit_window.request_redraw();
                            let _ = winit_window
                                .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
                        });
                    }
                    ThemeRecoveryAction::RestoreWindowSize { width, height } => {
                        slint_window.request_redraw();
                        let _ = slint_window.with_winit_window(|winit_window| {
                            winit_window.request_redraw();
                            let _ = winit_window
                                .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
                        });
                    }
                }
            }

            EventResult::Propagate
        });
}

#[cfg(not(target_os = "windows"))]
fn bind_windows_theme_redraw_recovery(
    _window: &AppWindow,
    _recovery: Rc<RefCell<ThemeRedrawRecovery>>,
) {
}

#[cfg_attr(not(any(target_os = "windows", test)), allow(dead_code))]
fn bump_render_revision(window: &AppWindow) {
    let next_revision = window.get_render_revision().wrapping_add(1);
    window.set_render_revision(next_revision);
}

fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    window.window().request_redraw();

    let request = build_native_window_appearance_request(state.theme_mode, window_appearance());
    let report = effects.apply_to_app_window(window, &request);

    if matches!(
        report.backdrop_status,
        crate::app::window_effects::BackdropApplyStatus::Failed
    ) {
        tracing::error!(
            target: "app.window",
            theme = ?request.theme,
            backdrop = ?request.backdrop,
            backdrop_error = %report.backdrop_error.as_deref().unwrap_or("unknown"),
            "failed to apply native window appearance"
        );
    }
}

fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized);
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
}

fn sync_sidebar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_assets_sidebar(state.show_assets_sidebar);
    window.set_active_sidebar_destination(state.active_sidebar_destination.id().into());
    window.set_sidebar_items(ModelRc::new(VecModel::from(sidebar_items_for(state))));
}

fn sync_shell_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_top_status_bar_state(window, state, effects);
    sync_sidebar_state(window, state);
}

fn sync_shell_layout(
    window: &AppWindow,
    state: &ShellViewModel,
    logical_width: u32,
    logical_height: u32,
) {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: logical_width.max(ShellMetrics::WINDOW_MIN_WIDTH),
        request_assets_sidebar: state.requested_assets_sidebar(),
        request_right_panel: state.requested_right_panel(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
    window.set_shell_body_height_cache(
        logical_height.saturating_sub(ShellMetrics::TITLEBAR_HEIGHT) as f32,
    );
}

fn current_window_size(window: &AppWindow) -> (u32, u32) {
    let size = window.window().size();
    (size.width, size.height)
}

fn load_ui_preferences(store: &Option<Rc<UiPreferencesStore>>) -> UiPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                tracing::error!(
                    target: "config.preferences",
                    error = %err,
                    "failed to load ui preferences"
                );
                UiPreferences::default()
            }
        },
        None => UiPreferences::default(),
    }
}

fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store
        && let Err(err) = store.save(&UiPreferences::from(state))
    {
        tracing::error!(
            target: "config.preferences",
            error = %err,
            "failed to save ui preferences"
        );
    }
}

pub fn bind_top_status_bar_with_store_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::formal(),
        effects,
    );
}

pub fn bind_top_status_bar_with_store_and_profile_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    let store = store.map(Rc::new);
    let prefs = load_ui_preferences(&store);
    let view_model = Rc::new(RefCell::new(ShellViewModel {
        theme_mode: prefs.theme_mode,
        is_always_on_top: prefs.always_on_top,
        ..ShellViewModel::default()
    }));
    let controller = Rc::new(WindowController::new(window));
    let redraw_recovery = Rc::new(RefCell::new(ThemeRedrawRecovery::default()));

    apply_restored_window_size(window, default_window_size());
    if profile.uses_theme_redraw_recovery() {
        bind_windows_theme_redraw_recovery(window, Rc::clone(&redraw_recovery));
    }
    sync_shell_state(window, &view_model.borrow(), effects.as_ref());
    sync_shell_layout(
        window,
        &view_model.borrow(),
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    );

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let redraw_recovery_ref = Rc::clone(&redraw_recovery);
    let profile_for_theme_toggle = profile;
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if profile_for_theme_toggle.uses_theme_redraw_recovery() {
            mark_windows_theme_redraw_recovery(&window, &redraw_recovery_ref);
        }
        state.toggle_theme_mode();
        sync_theme_and_window_effects(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    window.on_toggle_window_always_on_top_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_always_on_top();
        window.set_is_window_always_on_top(state.is_always_on_top);
        save_ui_preferences(&store_ref, &state);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_minimize_requested(move || {
        controller_ref.minimize();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_sidebar_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_assets_sidebar();
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sidebar_destination_selected(move |destination_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let destination = SidebarDestination::from_id(destination_id.as_str())
            .unwrap_or(SidebarDestination::Console);
        state.select_sidebar_destination(destination);
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, height| {
        let window = handle.unwrap();
        let state = state.borrow();
        sync_shell_layout(&window, &state, width as u32, height as u32);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_close_requested(move || {
        let _ = controller_ref.close();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_requested(move || {
        let _ = controller_ref.drag();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized);
        state.set_window_maximized(next);
        window.set_is_window_maximized(next);
    });
}

pub fn bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::formal(),
        default_platform_window_effects(),
    );
}

pub fn bind_top_status_bar_with_profile(window: &AppWindow, profile: AppRuntimeProfile) {
    let store = match UiPreferencesStore::for_app() {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to resolve ui preferences store"
            );
            None
        }
    };

    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        profile,
        default_platform_window_effects(),
    );
}

pub fn bind_top_status_bar(window: &AppWindow) {
    bind_top_status_bar_with_profile(window, AppRuntimeProfile::formal());
}

pub fn run() -> Result<()> {
    run_with_profile(AppRuntimeProfile::formal())
}

pub fn run_with_profile(profile: AppRuntimeProfile) -> Result<()> {
    let _window_title = runtime_window_title(profile);
    let window = AppWindow::new()?;
    bind_top_status_bar_with_profile(&window, profile);
    window.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::AppWindow;

    use super::{
        MonitorRect, ThemeRecoveryAction, ThemeRedrawRecovery, WindowRect, WindowVisibilitySnapshot,
    };

    #[test]
    fn redraw_recovery_stays_idle_when_theme_toggles_fully_visible() {
        let mut recovery = ThemeRedrawRecovery::default();
        let window = WindowRect::new(100, 100, 1440, 900);
        let monitors = [MonitorRect::new(0, 0, 1920, 1080)];

        recovery.mark_theme_toggle(WindowVisibilitySnapshot::from_rects(window, &monitors));

        assert_eq!(
            recovery.next_action(
                WindowVisibilitySnapshot::from_rects(window, &monitors),
                window.width,
                window.height,
                false,
            ),
            ThemeRecoveryAction::None
        );
    }

    #[test]
    fn redraw_recovery_requests_size_nudge_once_when_window_reenters_visible_area() {
        let mut recovery = ThemeRedrawRecovery::default();
        let mostly_offscreen = WindowRect::new(-400, 120, 1440, 900);
        let restored = WindowRect::new(80, 120, 1440, 900);
        let monitors = [MonitorRect::new(0, 0, 1920, 1080)];

        recovery.mark_theme_toggle(WindowVisibilitySnapshot::from_rects(
            mostly_offscreen,
            &monitors,
        ));

        let restored_snapshot = WindowVisibilitySnapshot::from_rects(restored, &monitors);
        assert_eq!(
            recovery.next_action(restored_snapshot, restored.width, restored.height, false,),
            ThemeRecoveryAction::NudgeWindowSize {
                width: restored.width + 1,
                height: restored.height,
            }
        );
        assert_eq!(
            recovery.next_action(
                restored_snapshot,
                restored.width + 1,
                restored.height,
                false
            ),
            ThemeRecoveryAction::RestoreWindowSize {
                width: restored.width,
                height: restored.height,
            }
        );
        assert_eq!(
            recovery.next_action(restored_snapshot, restored.width, restored.height, false),
            ThemeRecoveryAction::None
        );
    }

    #[test]
    fn redraw_recovery_skips_size_nudge_for_maximized_windows() {
        let mut recovery = ThemeRedrawRecovery::default();
        let mostly_offscreen = WindowRect::new(-400, 120, 1440, 900);
        let restored = WindowRect::new(80, 120, 1440, 900);
        let monitors = [MonitorRect::new(0, 0, 1920, 1080)];

        recovery.mark_theme_toggle(WindowVisibilitySnapshot::from_rects(
            mostly_offscreen,
            &monitors,
        ));

        assert_eq!(
            recovery.next_action(
                WindowVisibilitySnapshot::from_rects(restored, &monitors),
                restored.width,
                restored.height,
                true,
            ),
            ThemeRecoveryAction::None
        );
    }

    #[test]
    fn redraw_recovery_can_nudge_again_while_window_keeps_becoming_more_visible() {
        let mut recovery = ThemeRedrawRecovery::default();
        let mostly_offscreen = WindowRect::new(-400, 120, 1440, 900);
        let partially_restored = WindowRect::new(-320, 120, 1440, 900);
        let more_visible = WindowRect::new(-160, 120, 1440, 900);
        let monitors = [MonitorRect::new(0, 0, 1920, 1080)];

        recovery.mark_theme_toggle(WindowVisibilitySnapshot::from_rects(
            mostly_offscreen,
            &monitors,
        ));

        let partially_restored_snapshot =
            WindowVisibilitySnapshot::from_rects(partially_restored, &monitors);
        assert_eq!(
            recovery.next_action(
                partially_restored_snapshot,
                partially_restored.width,
                partially_restored.height,
                false,
            ),
            ThemeRecoveryAction::NudgeWindowSize {
                width: partially_restored.width + 1,
                height: partially_restored.height,
            }
        );
        assert_eq!(
            recovery.next_action(
                partially_restored_snapshot,
                partially_restored.width + 1,
                partially_restored.height,
                false,
            ),
            ThemeRecoveryAction::RestoreWindowSize {
                width: partially_restored.width,
                height: partially_restored.height,
            }
        );

        assert_eq!(
            recovery.next_action(
                WindowVisibilitySnapshot::from_rects(more_visible, &monitors),
                more_visible.width,
                more_visible.height,
                false,
            ),
            ThemeRecoveryAction::NudgeWindowSize {
                width: more_visible.width + 1,
                height: more_visible.height,
            }
        );
    }

    #[test]
    fn bump_render_revision_increments_hidden_revision() {
        i_slint_backend_testing::init_no_event_loop();

        let window = AppWindow::new().unwrap();

        assert_eq!(window.get_render_revision(), 0);

        super::bump_render_revision(&window);

        assert_eq!(window.get_render_revision(), 1);
    }
}
