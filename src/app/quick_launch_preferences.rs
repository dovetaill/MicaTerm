//! Persists local recent SSH launcher state for the New Tab surface.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Deserializer, Serialize};

use crate::app::app_paths::app_root_paths_for_app;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickLaunchPreferences {
    #[serde(default, deserialize_with = "deserialize_recent_assets")]
    pub recent_asset_ids: Vec<QuickLaunchRecentAsset>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickLaunchRecentAsset {
    pub asset_id: String,
    #[serde(default)]
    pub opened_at_unix_seconds: i64,
}

impl QuickLaunchRecentAsset {
    pub fn new(asset_id: impl Into<String>, opened_at_unix_seconds: i64) -> Self {
        Self {
            asset_id: asset_id.into(),
            opened_at_unix_seconds,
        }
    }
}

pub struct QuickLaunchPreferencesStore {
    path: PathBuf,
}

impl QuickLaunchPreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_app() -> Result<Self> {
        let app_paths = app_root_paths_for_app()?;
        Ok(Self::new(
            app_paths.config_dir.join("quick-launch-preferences.json"),
        ))
    }

    pub fn load_or_default(&self) -> Result<QuickLaunchPreferences> {
        if !self.path.exists() {
            return Ok(QuickLaunchPreferences::default());
        }

        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, prefs: &QuickLaunchPreferences) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(prefs)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

pub fn record_recent_asset_opened(
    existing: Vec<QuickLaunchRecentAsset>,
    asset_id: &str,
    opened_at_unix_seconds: i64,
    cap: usize,
) -> Vec<QuickLaunchRecentAsset> {
    if cap == 0 {
        return Vec::new();
    }

    let mut updated = Vec::with_capacity(existing.len().saturating_add(1).min(cap));
    updated.push(QuickLaunchRecentAsset::new(
        asset_id,
        opened_at_unix_seconds,
    ));
    updated.extend(
        existing
            .into_iter()
            .filter(|current| current.asset_id != asset_id),
    );
    updated.truncate(cap);
    updated
}

pub fn retain_known_ssh_asset_ids(
    prefs: &QuickLaunchPreferences,
    known_asset_ids: &BTreeSet<String>,
) -> QuickLaunchPreferences {
    let recent_asset_ids = prefs
        .recent_asset_ids
        .iter()
        .filter(|entry| known_asset_ids.contains(&entry.asset_id))
        .cloned()
        .collect();

    QuickLaunchPreferences { recent_asset_ids }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RecentAssetCompat {
    AssetId(String),
    Entry(QuickLaunchRecentAsset),
}

fn deserialize_recent_assets<'de, D>(
    deserializer: D,
) -> std::result::Result<Vec<QuickLaunchRecentAsset>, D::Error>
where
    D: Deserializer<'de>,
{
    let entries = Vec::<RecentAssetCompat>::deserialize(deserializer)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| match entry {
            RecentAssetCompat::AssetId(asset_id) => Some(QuickLaunchRecentAsset::new(asset_id, 0)),
            RecentAssetCompat::Entry(entry) => (!entry.asset_id.trim().is_empty()).then_some(entry),
        })
        .collect())
}
