use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::shell::view_model::ShellViewModel;
use crate::theme::ThemeMode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPreferences {
    pub theme_mode: ThemeMode,
    pub always_on_top: bool,
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            always_on_top: false,
        }
    }
}

pub struct UiPreferencesStore {
    path: PathBuf,
}

impl UiPreferencesStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn for_app() -> Result<Self> {
        let dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
            .context("project directories are unavailable")?;
        Ok(Self::new(dirs.config_dir().join("ui-preferences.json")))
    }

    pub fn load_or_default(&self) -> Result<UiPreferences> {
        if !self.path.exists() {
            return Ok(UiPreferences::default());
        }

        let content = fs::read_to_string(&self.path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save(&self, prefs: &UiPreferences) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(prefs)?;
        fs::write(&self.path, content)?;
        Ok(())
    }
}

impl From<&ShellViewModel> for UiPreferences {
    fn from(value: &ShellViewModel) -> Self {
        Self {
            theme_mode: value.theme_mode,
            always_on_top: value.is_always_on_top,
        }
    }
}
