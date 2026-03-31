//! Persistence coverage for saved UI preferences.

use mica_term::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use mica_term::shell::view_model::RightPanelView;
use mica_term::theme::ThemeMode;

#[test]
fn ui_preferences_default_to_dark_and_not_pinned() {
    let prefs = UiPreferences::default();

    assert_eq!(prefs.theme_mode, ThemeMode::Dark);
    assert!(!prefs.always_on_top);
    assert_eq!(prefs.right_panel_view, "appearance");
}

#[test]
fn ui_preferences_roundtrip_theme_and_pin_state() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ui-preferences-roundtrip.json");

    let store = UiPreferencesStore::new(temp_path.clone());
    let prefs = UiPreferences {
        theme_mode: ThemeMode::Light,
        always_on_top: true,
        right_panel_view: "vault".into(),
    };

    store.save(&prefs).unwrap();
    let loaded = store.load_or_default().unwrap();

    assert_eq!(loaded, prefs);
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn ui_preferences_accept_sftp_right_panel_view() {
    let prefs = UiPreferences {
        right_panel_view: "sftp".into(),
        ..UiPreferences::default()
    };

    assert_eq!(prefs.right_panel_view, "sftp");
    assert_eq!(RightPanelView::from_id(&prefs.right_panel_view).id(), "sftp");
}
