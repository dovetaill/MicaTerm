//! Persists the small set of user-facing window and theme preferences between launches.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::app::vault::model::SnapshotUiPreferences;
use crate::shell::view_model::{RightPanelView, ShellViewModel};
use crate::theme::ThemeMode;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPreferences {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: ThemeMode,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_right_panel_view")]
    pub right_panel_view: String,
    #[serde(default = "default_terminal_scrollback_limit")]
    pub terminal_scrollback_limit: usize,
    #[serde(default = "default_terminal_active_idle_shrink_enabled")]
    pub terminal_active_idle_shrink_enabled: bool,
}

fn default_theme_mode() -> ThemeMode {
    ThemeMode::Dark
}

fn default_right_panel_view() -> String {
    RightPanelView::Sftp.id().into()
}

fn default_terminal_scrollback_limit() -> usize {
    1500
}

fn default_terminal_active_idle_shrink_enabled() -> bool {
    true
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            always_on_top: false,
            right_panel_view: default_right_panel_view(),
            terminal_scrollback_limit: default_terminal_scrollback_limit(),
            terminal_active_idle_shrink_enabled: default_terminal_active_idle_shrink_enabled(),
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
            right_panel_view: value.right_panel_view_id().into(),
            terminal_scrollback_limit: value.settings_modal_terminal_scrollback_limit(),
            terminal_active_idle_shrink_enabled: value
                .settings_modal_terminal_active_idle_shrink_enabled(),
        }
    }
}

impl From<&UiPreferences> for SnapshotUiPreferences {
    fn from(value: &UiPreferences) -> Self {
        Self {
            theme_mode: Some(match value.theme_mode {
                ThemeMode::Dark => "dark".into(),
                ThemeMode::Light => "light".into(),
            }),
            always_on_top: Some(value.always_on_top),
        }
    }
}

pub fn ui_preferences_from_snapshot(snapshot: &SnapshotUiPreferences) -> UiPreferences {
    let theme_mode = match snapshot.theme_mode.as_deref() {
        Some("light") => ThemeMode::Light,
        _ => ThemeMode::Dark,
    };

    UiPreferences {
        theme_mode,
        always_on_top: snapshot.always_on_top.unwrap_or(false),
        right_panel_view: default_right_panel_view(),
        terminal_scrollback_limit: default_terminal_scrollback_limit(),
        terminal_active_idle_shrink_enabled: default_terminal_active_idle_shrink_enabled(),
    }
}
