use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_log_dir,
};
use mica_term::app::ui_preferences::UiPreferencesStore;

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
fn bootstrap_routes_tooltip_debug_events_to_log_file() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("tooltip-debug-bridge");
    let _ = fs::remove_dir_all(&temp_root);

    bind_top_status_bar_with_store_and_log_dir(
        &app,
        Some(UiPreferencesStore::new(
            temp_root.join("ui-preferences.json"),
        )),
        Some(temp_root.clone()),
    );

    app.invoke_tooltip_debug_event_requested(
        "nav-button".into(),
        "show-tooltip".into(),
        "Open menu".into(),
        24.0,
        44.0,
    );

    let content = fs::read_to_string(temp_root.join("logs").join("titlebar-tooltip.log")).unwrap();
    assert!(content.contains("show-tooltip"));
    assert!(content.contains("nav-button"));

    let _ = fs::remove_dir_all(temp_root);
}
