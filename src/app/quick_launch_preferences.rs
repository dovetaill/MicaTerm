//! Persists local quick launch state for the welcome dashboard.

use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::app::app_paths::app_root_paths_for_app;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickLaunchPreferences {
    #[serde(default)]
    pub favorite_asset_ids: Vec<String>,
    #[serde(default)]
    pub recent_asset_ids: Vec<String>,
    #[serde(default)]
    pub last_selected_asset_id: Option<String>,
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

pub fn record_recent_asset_id(existing: Vec<String>, asset_id: &str, cap: usize) -> Vec<String> {
    if cap == 0 {
        return Vec::new();
    }

    let mut updated = Vec::with_capacity(existing.len().saturating_add(1).min(cap));
    updated.push(asset_id.to_string());
    updated.extend(existing.into_iter().filter(|current| current != asset_id));
    updated.truncate(cap);
    updated
}

pub fn retain_known_ssh_asset_ids(
    prefs: &QuickLaunchPreferences,
    known_asset_ids: &BTreeSet<String>,
) -> QuickLaunchPreferences {
    let favorite_asset_ids = retain_known_ids(&prefs.favorite_asset_ids, known_asset_ids);
    let recent_asset_ids = retain_known_ids(&prefs.recent_asset_ids, known_asset_ids);
    let last_selected_asset_id = prefs
        .last_selected_asset_id
        .as_ref()
        .filter(|asset_id| known_asset_ids.contains(*asset_id))
        .cloned();

    QuickLaunchPreferences {
        favorite_asset_ids,
        recent_asset_ids,
        last_selected_asset_id,
    }
}

fn retain_known_ids(ids: &[String], known_asset_ids: &BTreeSet<String>) -> Vec<String> {
    ids.iter()
        .filter(|asset_id| known_asset_ids.contains(*asset_id))
        .cloned()
        .collect()
}
