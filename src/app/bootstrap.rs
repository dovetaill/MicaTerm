use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;
use crate::app::tooltip_debug_log::{TooltipDebugEvent, TooltipDebugLog};
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::windowing::WindowController;
use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn default_window_size() -> (u32, u32) {
    (1440, 900)
}

fn sync_top_status_bar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized);
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
}

fn load_ui_preferences(store: &Option<Rc<UiPreferencesStore>>) -> UiPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                eprintln!("failed to load ui preferences: {err}");
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
        eprintln!("failed to save ui preferences: {err}");
    }
}

fn create_tooltip_debug_logger(log_root: Option<PathBuf>) -> Option<Rc<TooltipDebugLog>> {
    let logger = match log_root {
        Some(root) => TooltipDebugLog::in_directory(root.join("logs")),
        None => TooltipDebugLog::for_current_dir(),
    };

    match logger {
        Ok(logger) => Some(Rc::new(logger)),
        Err(err) => {
            eprintln!("failed to initialize tooltip debug log: {err}");
            None
        }
    }
}

pub fn bind_top_status_bar_with_store_and_log_dir(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    log_root: Option<PathBuf>,
) {
    let store = store.map(Rc::new);
    let logger = create_tooltip_debug_logger(log_root);
    let prefs = load_ui_preferences(&store);
    let view_model = Rc::new(RefCell::new(ShellViewModel {
        theme_mode: prefs.theme_mode,
        is_always_on_top: prefs.always_on_top,
        ..ShellViewModel::default()
    }));
    let controller = Rc::new(WindowController::new(window));

    sync_top_status_bar_state(window, &view_model.borrow());

    let logger_ref = logger.clone();
    window.on_tooltip_debug_event_requested(move |source_id, phase, text, anchor_x, anchor_y| {
        if let Some(logger) = &logger_ref
            && let Err(err) = logger.append(TooltipDebugEvent {
                phase: phase.as_str(),
                source_id: source_id.as_str(),
                text: text.as_str(),
                anchor_x,
                anchor_y,
            })
        {
            eprintln!("failed to append tooltip debug event: {err}");
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
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
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_theme_mode();
        window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
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
    bind_top_status_bar_with_store_and_log_dir(window, store, None);
}

pub fn bind_top_status_bar(window: &AppWindow) {
    let store = match UiPreferencesStore::for_app() {
        Ok(store) => Some(store),
        Err(err) => {
            eprintln!("failed to resolve ui preferences store: {err}");
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
