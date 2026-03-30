use mica_term::app::quick_launch_preferences::{
    QuickLaunchPreferences, QuickLaunchPreferencesStore, record_recent_asset_id,
    retain_known_ssh_asset_ids,
};
use std::collections::BTreeSet;

#[test]
fn quick_launch_preferences_roundtrip_preserves_recent_and_favorites() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("quick-launch-preferences-roundtrip.json");
    let store = QuickLaunchPreferencesStore::new(temp_path.clone());
    let prefs = QuickLaunchPreferences {
        favorite_asset_ids: vec!["asset-prod".into()],
        recent_asset_ids: vec!["asset-db".into(), "asset-prod".into()],
        last_selected_asset_id: Some("asset-db".into()),
    };

    store.save(&prefs).unwrap();

    assert_eq!(store.load_or_default().unwrap(), prefs);

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn record_recent_moves_asset_to_front_and_caps_history() {
    let updated = record_recent_asset_id(vec!["a".into(), "b".into()], "a", 2);

    assert_eq!(updated, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn retain_known_ssh_asset_ids_drops_unknown_ids_and_invalid_selection() {
    let prefs = QuickLaunchPreferences {
        favorite_asset_ids: vec!["asset-prod".into(), "asset-old".into()],
        recent_asset_ids: vec!["asset-db".into(), "asset-old".into(), "asset-prod".into()],
        last_selected_asset_id: Some("asset-old".into()),
    };
    let known_asset_ids = BTreeSet::from(["asset-db".to_string(), "asset-prod".to_string()]);

    let retained = retain_known_ssh_asset_ids(&prefs, &known_asset_ids);

    assert_eq!(retained.favorite_asset_ids, vec!["asset-prod".to_string()]);
    assert_eq!(
        retained.recent_asset_ids,
        vec!["asset-db".to_string(), "asset-prod".to_string()]
    );
    assert_eq!(retained.last_selected_asset_id, None);
}
