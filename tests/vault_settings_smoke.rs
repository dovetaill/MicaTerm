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
fn sync_settings_starts_closed_until_explicitly_requested() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());

    app.invoke_open_sync_modal_requested();

    assert!(app.get_sync_modal_open());
    assert_eq!(app.get_sync_modal_provider_label().as_str(), "Gitee");
    assert_eq!(app.get_sync_modal_git_auth_mode().as_str(), "https");
}

#[test]
fn sync_settings_opens_before_remote_head_refresh_completes() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();

    assert!(app.get_sync_modal_open());
}

#[test]
fn titlebar_sync_action_falls_back_to_sync_settings_when_not_configured() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());

    app.invoke_sync_now_requested();

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

#[test]
fn formal_sync_settings_contract_no_longer_exposes_gitee_gist_primary() {
    let app_window = fs::read_to_string("ui/app-window.slint").unwrap();
    let provider_contract = fs::read_to_string("src/app/vault/provider/mod.rs").unwrap();

    assert!(!app_window.contains("sync-modal-primary-gist-id"));
    assert!(!app_window.contains("sync-modal-primary-pat"));
    assert!(!app_window.contains("Gitee Gist"));
    assert!(provider_contract.contains("ProviderKind::GitRepo"));
    assert!(
        !provider_contract
            .contains("first_release_formal_provider_kind() -> ProviderKind::GiteeGist")
    );
}
