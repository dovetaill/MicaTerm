use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::AppWindow;
use crate::app::window_recovery::WindowRecoveryController;
use crate::app::window_recovery::WindowVisibilitySnapshot;
#[cfg(target_os = "windows")]
use crate::app::window_recovery::WindowRecoveryAction;
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{WindowController, apply_restored_window_size, window_appearance};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for};
use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WindowRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MonitorRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

    let window = WindowRect::new(position.x, position.y, size.width, size.height);
    let visible_area = monitors
        .iter()
        .map(|monitor| intersection_area(window, *monitor))
        .sum();

    WindowVisibilitySnapshot::new(window.area(), visible_area)
}

#[cfg(target_os = "windows")]
fn arm_windows_window_recovery(
    window: &AppWindow,
    recovery: &Rc<RefCell<WindowRecoveryController>>,
) {
    use slint::winit_030::WinitWindowAccessor;

    let _ = window.window().with_winit_window(|winit_window| {
        recovery
            .borrow_mut()
            .arm_visibility_recovery(current_window_visibility_snapshot(winit_window));
    });
}

#[cfg(not(target_os = "windows"))]
fn arm_windows_window_recovery(
    _window: &AppWindow,
    _recovery: &Rc<RefCell<WindowRecoveryController>>,
) {
}

#[cfg(target_os = "windows")]
fn apply_window_recovery_action(
    handle: &slint::Weak<AppWindow>,
    slint_window: &slint::Window,
    action: WindowRecoveryAction,
) {
    use slint::winit_030::{WinitWindowAccessor, winit};

    match action {
        WindowRecoveryAction::None => {}
        WindowRecoveryAction::RequestRedraw => {
            let window = handle.unwrap();
            bump_render_revision(&window);
            slint_window.request_redraw();
            let _ = slint_window.with_winit_window(|winit_window| {
                winit_window.request_redraw();
            });
        }
        WindowRecoveryAction::NudgeWindowSize { width, height } => {
            let window = handle.unwrap();
            bump_render_revision(&window);
            slint_window.request_redraw();
            let _ = slint_window.with_winit_window(|winit_window| {
                winit_window.request_redraw();
                let _ = winit_window
                    .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
            });
        }
        WindowRecoveryAction::RestoreWindowSize { width, height } => {
            slint_window.request_redraw();
            let _ = slint_window.with_winit_window(|winit_window| {
                winit_window.request_redraw();
                let _ = winit_window
                    .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
            });
        }
    }
}

#[cfg(target_os = "windows")]
fn notify_windows_window_recovery_transition_with_snapshot(
    window: &AppWindow,
    recovery: &Rc<RefCell<WindowRecoveryController>>,
    previous: WindowPlacementKind,
    next: WindowPlacementKind,
    snapshot: WindowVisibilitySnapshot,
    width: u32,
    height: u32,
) {
    let handle = window.as_weak();
    let action = recovery
        .borrow_mut()
        .on_placement_changed(previous, next, snapshot, width, height);

    apply_window_recovery_action(&handle, window.window(), action);
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
#[cfg(not(target_os = "windows"))]
fn notify_windows_window_recovery_transition_with_snapshot(
    _window: &AppWindow,
    _recovery: &Rc<RefCell<WindowRecoveryController>>,
    _previous: WindowPlacementKind,
    _next: WindowPlacementKind,
    _snapshot: WindowVisibilitySnapshot,
    _width: u32,
    _height: u32,
) {
}

#[cfg(target_os = "windows")]
fn notify_windows_window_recovery_transition(
    window: &AppWindow,
    recovery: &Rc<RefCell<WindowRecoveryController>>,
    previous: WindowPlacementKind,
    next: WindowPlacementKind,
) {
    use slint::winit_030::WinitWindowAccessor;

    let _ = window.window().with_winit_window(|winit_window| {
        let size = winit_window.inner_size();
        notify_windows_window_recovery_transition_with_snapshot(
            window,
            recovery,
            previous,
            next,
            current_window_visibility_snapshot(winit_window),
            size.width,
            size.height,
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn notify_windows_window_recovery_transition(
    _window: &AppWindow,
    _recovery: &Rc<RefCell<WindowRecoveryController>>,
    _previous: WindowPlacementKind,
    _next: WindowPlacementKind,
) {
}

#[cfg(target_os = "windows")]
fn sync_windows_true_window_placement(
    window: &AppWindow,
    state: &Rc<RefCell<ShellViewModel>>,
    effects: &dyn PlatformWindowEffects,
    recovery: &Rc<RefCell<WindowRecoveryController>>,
    winit_window: &slint::winit_030::winit::window::Window,
) {
    let Some(next) = query_true_window_placement(winit_window) else {
        return;
    };

    let previous = {
        let mut state = state.borrow_mut();
        let previous = state.window_placement();
        if previous == next {
            return;
        }

        state.set_window_placement(next);
        sync_top_status_bar_state(window, &state, effects);
        previous
    };

    let size = winit_window.inner_size();
    notify_windows_window_recovery_transition_with_snapshot(
        window,
        recovery,
        previous,
        next,
        current_window_visibility_snapshot(winit_window),
        size.width,
        size.height,
    );
}

#[cfg(target_os = "windows")]
fn bind_windows_window_recovery(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    effects: Rc<dyn PlatformWindowEffects>,
    recovery: Rc<RefCell<WindowRecoveryController>>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    window.window().on_winit_window_event(move |slint_window, event| {
        let should_check_visibility = matches!(
            event,
            winit::event::WindowEvent::Moved(_)
                | winit::event::WindowEvent::Resized(_)
                | winit::event::WindowEvent::ScaleFactorChanged { .. }
        );

        if should_check_visibility {
            let window = handle.unwrap();
            let action = slint_window
                .with_winit_window(|winit_window| {
                    sync_windows_true_window_placement(
                        &window,
                        &state,
                        effects.as_ref(),
                        &recovery,
                        winit_window,
                    );

                    let size = winit_window.inner_size();
                    let mut recovery = recovery.borrow_mut();
                    let action = recovery.on_resize_ack(size.width, size.height);
                    if action != WindowRecoveryAction::None {
                        return action;
                    }

                    recovery.on_visibility_changed(
                        current_window_visibility_snapshot(winit_window),
                        size.width,
                        size.height,
                    )
                })
                .unwrap_or(WindowRecoveryAction::None);

            apply_window_recovery_action(&handle, slint_window, action);
        }

        EventResult::Propagate
    });
}

#[cfg(not(target_os = "windows"))]
fn bind_windows_window_recovery(
    _window: &AppWindow,
    _state: Rc<RefCell<ShellViewModel>>,
    _effects: Rc<dyn PlatformWindowEffects>,
    _recovery: Rc<RefCell<WindowRecoveryController>>,
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
    window.set_is_window_maximized(state.is_window_maximized());
    window.set_use_flat_window_chrome(state.uses_flat_window_chrome());
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
        logical_height
            .saturating_sub(ShellMetrics::TITLEBAR_HEIGHT)
            as f32,
    );
}

fn current_window_size(window: &AppWindow) -> (u32, u32) {
    let size = window.window().size();
    (size.width, size.height)
}

#[cfg(target_os = "windows")]
fn install_windows_frame_adapter(window: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;

    let placement = query_true_window_placement_from_app(window);
    let maximize_button = CaptionButtonGeometry {
        x: window.get_layout_titlebar_maximize_button_x() as i32,
        y: window.get_layout_titlebar_maximize_button_y() as i32,
        width: window.get_layout_titlebar_maximize_button_width() as i32,
        height: window.get_layout_titlebar_maximize_button_height() as i32,
    };

    let _ = window.window().with_winit_window(|winit_window| {
        install_window_frame_adapter(winit_window, maximize_button, placement);
    });
}

#[cfg(not(target_os = "windows"))]
fn install_windows_frame_adapter(_window: &AppWindow) {
}

#[cfg(target_os = "windows")]
fn query_true_window_placement_from_app(window: &AppWindow) -> WindowPlacementKind {
    use slint::winit_030::WinitWindowAccessor;

    window
        .window()
        .with_winit_window(query_true_window_placement)
        .flatten()
        .unwrap_or(WindowPlacementKind::Unknown)
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
    let store = store.map(Rc::new);
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let controller = Rc::new(WindowController::new(window));
    let window_recovery = Rc::new(RefCell::new(WindowRecoveryController::default()));

    apply_restored_window_size(window, default_window_size());
    bind_windows_window_recovery(
        window,
        Rc::clone(&view_model),
        Rc::clone(&effects),
        Rc::clone(&window_recovery),
    );
    sync_shell_state(window, &view_model.borrow(), effects.as_ref());
    sync_shell_layout(
        window,
        &view_model.borrow(),
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    );
    install_windows_frame_adapter(window);

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
    let window_recovery_ref = Rc::clone(&window_recovery);
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        arm_windows_window_recovery(&window, &window_recovery_ref);
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
    let effects_ref = Rc::clone(&effects);
    let window_recovery_ref = Rc::clone(&window_recovery);
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let previous = state.window_placement();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        notify_windows_window_recovery_transition(&window, &window_recovery_ref, previous, next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
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
        install_windows_frame_adapter(&window);
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
    let effects_ref = Rc::clone(&effects);
    let window_recovery_ref = Rc::clone(&window_recovery);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let previous = state.window_placement();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        notify_windows_window_recovery_transition(&window, &window_recovery_ref, previous, next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });
}

pub fn bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>) {
    bind_top_status_bar_with_store_and_effects(window, store, default_platform_window_effects());
}

pub fn bind_top_status_bar(window: &AppWindow) {
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

    bind_top_status_bar_with_store(window, store);
}

pub fn run() -> Result<()> {
    let window = AppWindow::new()?;
    bind_top_status_bar(&window);
    window.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::AppWindow;

    #[test]
    fn bump_render_revision_increments_hidden_revision() {
        i_slint_backend_testing::init_no_event_loop();

        let window = AppWindow::new().unwrap();

        assert_eq!(window.get_render_revision(), 0);

        super::bump_render_revision(&window);

        assert_eq!(window.get_render_revision(), 1);
    }
}
