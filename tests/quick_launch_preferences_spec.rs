use mica_term::app::quick_launch_preferences::{
    QuickLaunchPreferences, QuickLaunchPreferencesStore, QuickLaunchRecentAsset,
    record_recent_asset_opened, retain_known_ssh_asset_ids,
};
use std::collections::BTreeSet;

#[test]
fn quick_launch_preferences_roundtrip_preserves_recent_history() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("quick-launch-preferences-roundtrip.json");
    let store = QuickLaunchPreferencesStore::new(temp_path.clone());
    let prefs = QuickLaunchPreferences {
        recent_asset_ids: vec![
            QuickLaunchRecentAsset::new("asset-db", 1_700_000_000),
            QuickLaunchRecentAsset::new("asset-prod", 1_699_999_900),
        ],
    };

    store.save(&prefs).unwrap();

    assert_eq!(store.load_or_default().unwrap(), prefs);

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn record_recent_opened_tracks_timestamp_and_caps_history() {
    let updated = record_recent_asset_opened(
        vec![
            QuickLaunchRecentAsset::new("a", 10),
            QuickLaunchRecentAsset::new("b", 9),
            QuickLaunchRecentAsset::new("c", 8),
        ],
        "b",
        20,
        2,
    );

    assert_eq!(updated.len(), 2);
    assert_eq!(updated[0].asset_id, "b");
    assert_eq!(updated[0].opened_at_unix_seconds, 20);
    assert_eq!(updated[1].asset_id, "a");
}

#[test]
fn quick_launch_preferences_loads_legacy_recent_asset_id_arrays() {
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("quick-launch-preferences-legacy-recent.json");
    if let Some(parent) = temp_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(
        &temp_path,
        r#"{
  "favorite_asset_ids": ["asset-prod"],
  "recent_asset_ids": ["asset-db", "asset-prod"],
  "last_selected_asset_id": "asset-db"
}"#,
    )
    .unwrap();
    let store = QuickLaunchPreferencesStore::new(temp_path.clone());

    let prefs = store.load_or_default().unwrap();

    assert_eq!(prefs.recent_asset_ids.len(), 2);
    assert_eq!(prefs.recent_asset_ids[0].asset_id, "asset-db");
    assert_eq!(prefs.recent_asset_ids[0].opened_at_unix_seconds, 0);

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn retain_known_ssh_asset_ids_drops_unknown_recent_entries() {
    let prefs = QuickLaunchPreferences {
        recent_asset_ids: vec![
            QuickLaunchRecentAsset::new("asset-db", 30),
            QuickLaunchRecentAsset::new("asset-old", 20),
            QuickLaunchRecentAsset::new("asset-prod", 10),
        ],
    };
    let known_asset_ids = BTreeSet::from(["asset-db".to_string(), "asset-prod".to_string()]);

    let retained = retain_known_ssh_asset_ids(&prefs, &known_asset_ids);

    assert_eq!(
        retained
            .recent_asset_ids
            .iter()
            .map(|entry| entry.asset_id.as_str())
            .collect::<Vec<_>>(),
        vec!["asset-db", "asset-prod"]
    );
}
