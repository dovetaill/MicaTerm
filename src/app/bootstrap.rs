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
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{
    WindowController, apply_restored_window_size, parse_resize_direction, window_appearance,
};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for};
use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn runtime_window_title(profile: AppRuntimeProfile) -> String {
    if profile.is_experimental() {
        "Mica Term [FemtoVG WGPU Experimental]".into()
    } else {
        app_title().to_owned()
    }
}

pub fn startup_failure_message(profile: AppRuntimeProfile, err: &str) -> Option<String> {
    if profile.is_experimental() {
        Some(format!(
            "Mica Term FemtoVG WGPU Experimental failed to initialize winit-femtovg-wgpu: {err}"
        ))
    } else {
        None
    }
}

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

#[cfg(target_os = "windows")]
fn sync_windows_true_window_placement(
    window: &AppWindow,
    state: &Rc<RefCell<ShellViewModel>>,
    effects: &dyn PlatformWindowEffects,
    winit_window: &slint::winit_030::winit::window::Window,
) {
    let Some(next) = query_true_window_placement(winit_window) else {
        return;
    };

    let mut state = state.borrow_mut();
    if state.window_placement() == next {
        return;
    }

    state.set_window_placement(next);
    sync_top_status_bar_state(window, &state, effects);
}

#[cfg(target_os = "windows")]
fn bind_windows_window_state_tracking(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    window
        .window()
        .on_winit_window_event(move |_slint_window, event| {
            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) {
                let window = handle.unwrap();
                let _ = window.window().with_winit_window(|winit_window| {
                    sync_windows_true_window_placement(
                        &window,
                        &state,
                        effects.as_ref(),
                        winit_window,
                    );
                });
            }

            EventResult::Propagate
        });
}

#[cfg(not(target_os = "windows"))]
fn bind_windows_window_state_tracking(
    _window: &AppWindow,
    _state: Rc<RefCell<ShellViewModel>>,
    _effects: Rc<dyn PlatformWindowEffects>,
) {
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
        logical_height.saturating_sub(ShellMetrics::TITLEBAR_HEIGHT) as f32,
    );
}

fn current_window_size(window: &AppWindow) -> (u32, u32) {
    let size = window.window().size();
    (size.width, size.height)
}

#[cfg(target_os = "windows")]
const WINDOW_FRAME_RESERVED_RESIZE_BAND: i32 = 10;

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
        install_window_frame_adapter(
            winit_window,
            maximize_button,
            placement,
            WINDOW_FRAME_RESERVED_RESIZE_BAND,
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn install_windows_frame_adapter(_window: &AppWindow) {}

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
    _profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    let store = store.map(Rc::new);
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let controller = Rc::new(WindowController::new(window));

    apply_restored_window_size(window, default_window_size());
    bind_windows_window_state_tracking(window, Rc::clone(&view_model), Rc::clone(&effects));
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
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
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
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
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

    let controller_ref = Rc::clone(&controller);
    window.on_drag_resize_requested(move |direction| {
        if let Some(direction) = parse_resize_direction(direction.as_str()) {
            let _ = controller_ref.drag_resize(direction);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    let effects_ref = Rc::clone(&effects);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
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
    let window = AppWindow::new()?;
    window.set_window_title(runtime_window_title(profile).into());
    bind_top_status_bar_with_profile(&window, profile);
    window.run()?;
    Ok(())
}
