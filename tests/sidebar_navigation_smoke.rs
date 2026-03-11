use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::UiPreferencesStore;

#[test]
fn bootstrap_initializes_sidebar_defaults() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-defaults.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn bootstrap_toggles_assets_sidebar_without_losing_destination() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-toggle.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_sidebar_destination_selected("snippets".into());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "snippets");

    let _ = fs::remove_file(temp_path);
}

#[test]
fn selecting_destination_auto_expands_assets_sidebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("sidebar-select.json");
    let _ = fs::remove_file(&temp_path);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_toggle_assets_sidebar_requested();
    assert!(!app.get_show_assets_sidebar());

    app.invoke_sidebar_destination_selected("keychain".into());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "keychain");

    let _ = fs::remove_file(temp_path);
}
