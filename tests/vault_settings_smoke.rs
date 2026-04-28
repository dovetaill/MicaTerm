//! Smoke coverage for the formal titlebar Sync entry and non-vault Settings contract.

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use std::fs;

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
fn settings_action_opens_settings_modal_without_touching_sftp_panel() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_settings_modal_open());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_open_settings_panel_requested();

    assert!(app.get_settings_modal_open());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");
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
fn settings_modal_exposes_default_terminal_preferences() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    let defaults = UiPreferences::default();

    app.invoke_open_settings_panel_requested();

    assert_eq!(
        app.get_settings_modal_terminal_scrollback_limit(),
        defaults.terminal_scrollback_limit as i32
    );
    assert_eq!(
        app.get_settings_modal_terminal_active_idle_shrink_enabled(),
        defaults.terminal_active_idle_shrink_enabled
    );
    assert_eq!(
        app.get_settings_modal_terminal_input_highlighting_enabled(),
        defaults.terminal_input_highlighting_enabled
    );
    assert_eq!(
        app.get_settings_modal_terminal_output_rule_highlighting_enabled(),
        defaults.terminal_output_rule_highlighting_enabled
    );
    assert_eq!(
        app.get_settings_modal_terminal_command_decorations_enabled(),
        defaults.terminal_command_decorations_enabled
    );
    assert_eq!(
        app.get_settings_modal_terminal_overview_markers_enabled(),
        defaults.terminal_overview_markers_enabled
    );
    assert_eq!(
        app.get_settings_modal_terminal_output_rule_profile()
            .as_str(),
        defaults.terminal_output_rule_profile.id()
    );
    assert_eq!(
        app.get_settings_modal_terminal_search_match_highlight()
            .as_str(),
        defaults.terminal_search_match_highlight.id()
    );
    assert_eq!(
        app.get_settings_modal_theme_variant().as_str(),
        defaults.theme_variant.id()
    );
}

#[test]
fn settings_modal_terminal_preferences_update_window_state_and_persist() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("settings-modal-terminal-preferences.json");
    let _ = std::fs::remove_file(&temp_path);

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_open_settings_panel_requested();
    app.invoke_settings_modal_terminal_scrollback_limit_changed(3000);
    app.invoke_settings_modal_terminal_active_idle_shrink_enabled_changed(false);
    app.invoke_settings_modal_terminal_input_highlighting_enabled_changed(false);
    app.invoke_settings_modal_terminal_output_rule_highlighting_enabled_changed(false);
    app.invoke_settings_modal_terminal_command_decorations_enabled_changed(false);
    app.invoke_settings_modal_terminal_overview_markers_enabled_changed(false);
    app.invoke_settings_modal_terminal_output_rule_profile_changed("focused".into());
    app.invoke_settings_modal_terminal_search_match_highlight_changed("strong".into());
    app.invoke_settings_modal_theme_variant_changed("legacy_hacker_green".into());

    assert_eq!(app.get_settings_modal_terminal_scrollback_limit(), 3000);
    assert!(!app.get_settings_modal_terminal_active_idle_shrink_enabled());
    assert!(!app.get_settings_modal_terminal_input_highlighting_enabled());
    assert!(!app.get_settings_modal_terminal_output_rule_highlighting_enabled());
    assert!(!app.get_settings_modal_terminal_command_decorations_enabled());
    assert!(!app.get_settings_modal_terminal_overview_markers_enabled());
    assert_eq!(
        app.get_settings_modal_terminal_output_rule_profile()
            .as_str(),
        "focused"
    );
    assert_eq!(
        app.get_settings_modal_terminal_search_match_highlight()
            .as_str(),
        "strong"
    );
    assert_eq!(
        app.get_settings_modal_theme_variant().as_str(),
        "legacy_hacker_green"
    );

    let content = fs::read_to_string(&temp_path).expect("read persisted ui preferences");
    assert!(content.contains("\"terminal_scrollback_limit\": 3000"));
    assert!(content.contains("\"terminal_active_idle_shrink_enabled\": false"));
    assert!(content.contains("\"terminal_input_highlighting_enabled\": false"));
    assert!(content.contains("\"terminal_output_rule_highlighting_enabled\": false"));
    assert!(content.contains("\"terminal_command_decorations_enabled\": false"));
    assert!(content.contains("\"terminal_overview_markers_enabled\": false"));
    assert!(content.contains("\"terminal_output_rule_profile\": \"focused\""));
    assert!(content.contains("\"terminal_search_match_highlight\": \"strong\""));
    assert!(content.contains("\"theme_variant\": \"legacy_hacker_green\""));

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn settings_modal_download_conflict_preference_updates_window_state_and_persist() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("settings-modal-download-conflict-default.json");
    let _ = std::fs::remove_file(&temp_path);

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    app.invoke_open_settings_panel_requested();

    assert_eq!(
        app.get_settings_modal_download_conflict_default().as_str(),
        "ask"
    );

    app.invoke_settings_modal_download_conflict_default_changed("auto-rename".into());

    assert_eq!(
        app.get_settings_modal_download_conflict_default().as_str(),
        "auto-rename"
    );

    let content = fs::read_to_string(&temp_path).expect("read persisted ui preferences");
    assert!(content.contains("\"download_conflict_default\": \"auto-rename\""));

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
