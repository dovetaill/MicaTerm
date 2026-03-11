use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::AppWindow;
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

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
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

fn sync_shell_layout(window: &AppWindow, state: &ShellViewModel, logical_width: u32) {
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: logical_width.max(ShellMetrics::WINDOW_MIN_WIDTH),
        request_assets_sidebar: state.requested_assets_sidebar(),
        request_right_panel: state.requested_right_panel(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
}

fn current_window_width(window: &AppWindow) -> u32 {
    window.window().size().width
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
    let view_model = Rc::new(RefCell::new(ShellViewModel {
        theme_mode: prefs.theme_mode,
        is_always_on_top: prefs.always_on_top,
        ..ShellViewModel::default()
    }));
    let controller = Rc::new(WindowController::new(window));

    apply_restored_window_size(window, default_window_size());
    sync_shell_state(window, &view_model.borrow(), effects.as_ref());
    sync_shell_layout(
        window,
        &view_model.borrow(),
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
    );

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
        sync_shell_layout(&window, &state, current_window_width(&window));
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
        sync_shell_layout(&window, &state, current_window_width(&window));
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
        sync_shell_layout(&window, &state, current_window_width(&window));
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, _height| {
        let window = handle.unwrap();
        let state = state.borrow();
        sync_shell_layout(&window, &state, width as u32);
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
