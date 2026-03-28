//! Smoke coverage for the Sync & Vault right-panel contract.

use std::fs;

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::UiPreferencesStore;

#[test]
fn settings_action_routes_right_panel_to_sync_and_vault() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_right_panel_view().as_str(), "appearance");

    app.invoke_open_settings_panel_requested();

    assert!(app.get_show_right_panel());
    assert_eq!(app.get_right_panel_view().as_str(), "vault");
    assert_eq!(app.get_vault_panel_title().as_str(), "Sync & Vault");
}

#[test]
fn sync_and_vault_panel_exposes_default_status_and_actions() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_open_settings_panel_requested();

    assert_eq!(app.get_vault_lock_state_label().as_str(), "Locked");
    assert_eq!(app.get_vault_primary_status_label().as_str(), "Primary not configured");
    assert_eq!(app.get_vault_primary_action_label().as_str(), "Set");
    assert_eq!(app.get_vault_secondary_action_label().as_str(), "Change");
    assert_eq!(app.get_vault_tertiary_action_label().as_str(), "Lock now");
    assert_eq!(app.get_vault_sync_now_label().as_str(), "Sync now");
    assert_eq!(app.get_vault_export_bootstrap_label().as_str(), "Export bootstrap");
    assert_eq!(app.get_vault_import_bootstrap_label().as_str(), "Import bootstrap");
}

#[test]
fn selecting_settings_panel_persists_sync_and_vault_preference() {
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

    assert!(content.contains("\"right_panel_view\": \"vault\""));
    let _ = std::fs::remove_file(temp_path);
}
