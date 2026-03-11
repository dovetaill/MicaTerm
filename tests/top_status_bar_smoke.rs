use std::cell::RefCell;
use std::rc::Rc;

use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_effects,
};
use mica_term::app::ui_preferences::UiPreferencesStore;
use mica_term::app::window_effects::{
    BackdropApplyStatus, NativeWindowAppearanceRequest, NativeWindowTheme, PlatformWindowEffects,
    WindowAppearanceSyncReport,
};

#[derive(Clone)]
struct RecordingWindowEffects {
    requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>,
}

impl RecordingWindowEffects {
    fn new(requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>) -> Self {
        Self { requests }
    }
}

impl PlatformWindowEffects for RecordingWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        self.requests.borrow_mut().push(*request);
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Applied,
            redraw_requested: request.request_redraw,
        }
    }
}

#[test]
fn bootstrap_binds_top_status_bar_callbacks_to_window_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-ui-preferences.json");
    let _ = std::fs::remove_file(&temp_path);

    app.set_dark_mode(false);
    app.set_show_right_panel(true);
    app.set_show_global_menu(true);
    app.set_is_window_maximized(true);
    app.set_is_window_active(false);
    app.set_is_window_always_on_top(true);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_dark_mode());
    assert!(!app.get_show_right_panel());
    assert!(!app.get_show_global_menu());
    assert!(!app.get_is_window_maximized());
    assert!(app.get_is_window_active());
    assert!(!app.get_is_window_always_on_top());

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_right_panel());

    app.invoke_toggle_global_menu_requested();
    assert!(app.get_show_global_menu());

    app.invoke_close_global_menu_requested();
    assert!(!app.get_show_global_menu());

    app.invoke_toggle_theme_mode_requested();
    assert!(!app.get_dark_mode());

    app.invoke_toggle_window_always_on_top_requested();
    assert!(app.get_is_window_always_on_top());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn bootstrap_syncs_native_window_effects_on_bind_and_theme_toggle() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-window-effects.json");
    let _ = std::fs::remove_file(&temp_path);

    let requests = Rc::new(RefCell::new(Vec::new()));
    let effects = Rc::new(RecordingWindowEffects::new(Rc::clone(&requests)));

    bind_top_status_bar_with_store_and_effects(
        &app,
        Some(UiPreferencesStore::new(temp_path.clone())),
        effects,
    );

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].theme, NativeWindowTheme::Dark);
        assert!(requests[0].request_redraw);
    }

    app.invoke_toggle_theme_mode_requested();

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].theme, NativeWindowTheme::Light);
        assert!(requests[1].request_redraw);
    }

    let _ = std::fs::remove_file(temp_path);
}
