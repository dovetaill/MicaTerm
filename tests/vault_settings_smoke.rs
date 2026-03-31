//! Smoke coverage for the formal titlebar Sync entry and non-vault Settings contract.

use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::UiPreferencesStore;

#[test]
fn settings_action_no_longer_routes_right_panel_to_sync_and_vault() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_open_settings_panel_requested();

    assert_ne!(app.get_right_panel_view().as_str(), "vault");
}

#[test]
fn sync_modal_starts_closed_until_the_titlebar_sync_action_is_invoked() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());

    app.invoke_open_sync_modal_requested();

    assert!(app.get_sync_modal_open());
}

#[test]
fn selecting_settings_panel_does_not_persist_the_legacy_vault_preference() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("vault-settings-ui-preferences.json");
    let _ = std::fs::remove_file(&temp_path);

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));
    app.invoke_open_settings_panel_requested();

    let content = fs::read_to_string(&temp_path).expect("read persisted ui preferences");

    assert!(!content.contains("\"right_panel_view\": \"vault\""));
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn formal_ui_no_longer_contains_vault_right_panel_entry() {
    let source = fs::read_to_string("ui/shell/right-panel.slint").unwrap();

    assert!(!source.contains("text: \"Sync & Vault\""));
    assert!(!source.contains("panel-view == \"vault\""));
}
