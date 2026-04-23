//! Persists the small set of user-facing window and theme preferences between launches.

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::app::app_paths::app_root_paths_for_app;
use crate::app::terminal_semantic::OutputRuleProfile;
use crate::app::vault::model::SnapshotUiPreferences;
use crate::shell::view_model::{RightPanelView, ShellViewModel};
use crate::theme::{SearchMatchHighlightStrength, ThemeMode, ThemeVariant};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum DownloadConflictDefault {
    #[default]
    Ask,
    Overwrite,
    AutoRename,
}

impl DownloadConflictDefault {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Overwrite => "overwrite",
            Self::AutoRename => "auto-rename",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "overwrite" => Self::Overwrite,
            "auto-rename" => Self::AutoRename,
            _ => Self::Ask,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct PersistedWindowBounds {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiPreferences {
    #[serde(default = "default_theme_mode")]
    pub theme_mode: ThemeMode,
    #[serde(default = "default_theme_variant")]
    pub theme_variant: ThemeVariant,
    #[serde(default)]
    pub always_on_top: bool,
    #[serde(default = "default_right_panel_view")]
    pub right_panel_view: String,
    #[serde(default = "default_terminal_scrollback_limit")]
    pub terminal_scrollback_limit: usize,
    #[serde(default = "default_terminal_active_idle_shrink_enabled")]
    pub terminal_active_idle_shrink_enabled: bool,
    #[serde(default = "default_terminal_input_highlighting_enabled")]
    pub terminal_input_highlighting_enabled: bool,
    #[serde(default = "default_terminal_output_rule_highlighting_enabled")]
    pub terminal_output_rule_highlighting_enabled: bool,
    #[serde(default = "default_terminal_command_decorations_enabled")]
    pub terminal_command_decorations_enabled: bool,
    #[serde(default = "default_terminal_overview_markers_enabled")]
    pub terminal_overview_markers_enabled: bool,
    #[serde(default = "default_terminal_output_rule_profile")]
    pub terminal_output_rule_profile: OutputRuleProfile,
    #[serde(default = "default_terminal_search_match_highlight")]
    pub terminal_search_match_highlight: SearchMatchHighlightStrength,
    #[serde(default)]
    pub download_conflict_default: DownloadConflictDefault,
    #[serde(default)]
    pub window_bounds: Option<PersistedWindowBounds>,
}

fn default_theme_mode() -> ThemeMode {
    ThemeMode::Dark
}

fn default_theme_variant() -> ThemeVariant {
    ThemeVariant::PremiumDefault
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

fn default_terminal_input_highlighting_enabled() -> bool {
    true
}

fn default_terminal_output_rule_highlighting_enabled() -> bool {
    true
}

fn default_terminal_command_decorations_enabled() -> bool {
    true
}

fn default_terminal_overview_markers_enabled() -> bool {
    true
}

fn default_terminal_output_rule_profile() -> OutputRuleProfile {
    OutputRuleProfile::Default
}

fn default_terminal_search_match_highlight() -> SearchMatchHighlightStrength {
    SearchMatchHighlightStrength::Balanced
}

impl Default for UiPreferences {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::Dark,
            theme_variant: ThemeVariant::PremiumDefault,
            always_on_top: false,
            right_panel_view: default_right_panel_view(),
            terminal_scrollback_limit: default_terminal_scrollback_limit(),
            terminal_active_idle_shrink_enabled: default_terminal_active_idle_shrink_enabled(),
            terminal_input_highlighting_enabled: default_terminal_input_highlighting_enabled(),
            terminal_output_rule_highlighting_enabled:
                default_terminal_output_rule_highlighting_enabled(),
            terminal_command_decorations_enabled: default_terminal_command_decorations_enabled(),
            terminal_overview_markers_enabled: default_terminal_overview_markers_enabled(),
            terminal_output_rule_profile: default_terminal_output_rule_profile(),
            terminal_search_match_highlight: default_terminal_search_match_highlight(),
            download_conflict_default: DownloadConflictDefault::Ask,
            window_bounds: None,
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
        let app_paths = app_root_paths_for_app()?;
        Ok(Self::new(app_paths.config_dir.join("ui-preferences.json")))
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
            theme_variant: value.theme_variant,
            always_on_top: value.is_always_on_top,
            right_panel_view: value.right_panel_view_id().into(),
            terminal_scrollback_limit: value.settings_modal_terminal_scrollback_limit(),
            terminal_active_idle_shrink_enabled: value
                .settings_modal_terminal_active_idle_shrink_enabled(),
            terminal_input_highlighting_enabled: value
                .settings_modal_terminal_input_highlighting_enabled(),
            terminal_output_rule_highlighting_enabled: value
                .settings_modal_terminal_output_rule_highlighting_enabled(),
            terminal_command_decorations_enabled: value
                .settings_modal_terminal_command_decorations_enabled(),
            terminal_overview_markers_enabled: value
                .settings_modal_terminal_overview_markers_enabled(),
            terminal_output_rule_profile: value.settings_modal_terminal_output_rule_profile(),
            terminal_search_match_highlight: value.settings_modal_terminal_search_match_highlight(),
            download_conflict_default: value.settings_modal_download_conflict_default(),
            window_bounds: None,
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
        theme_variant: default_theme_variant(),
        always_on_top: snapshot.always_on_top.unwrap_or(false),
        right_panel_view: default_right_panel_view(),
        terminal_scrollback_limit: default_terminal_scrollback_limit(),
        terminal_active_idle_shrink_enabled: default_terminal_active_idle_shrink_enabled(),
        terminal_input_highlighting_enabled: default_terminal_input_highlighting_enabled(),
        terminal_output_rule_highlighting_enabled:
            default_terminal_output_rule_highlighting_enabled(),
        terminal_command_decorations_enabled: default_terminal_command_decorations_enabled(),
        terminal_overview_markers_enabled: default_terminal_overview_markers_enabled(),
        terminal_output_rule_profile: default_terminal_output_rule_profile(),
        terminal_search_match_highlight: default_terminal_search_match_highlight(),
        download_conflict_default: DownloadConflictDefault::Ask,
        window_bounds: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_preferences_round_trip_preserves_window_position_only() {
        let json = serde_json::json!({
            "theme_mode": "dark",
            "always_on_top": false,
            "right_panel_view": "sftp",
            "terminal_scrollback_limit": 1500,
            "terminal_active_idle_shrink_enabled": true,
            "terminal_input_highlighting_enabled": true,
            "terminal_output_rule_highlighting_enabled": true,
            "terminal_command_decorations_enabled": true,
            "terminal_overview_markers_enabled": true,
            "terminal_output_rule_profile": "default",
            "terminal_search_match_highlight": "balanced",
            "download_conflict_default": "ask",
            "window_bounds": {
                "x": 160,
                "y": 120,
                "width": 1680,
                "height": 980
            }
        })
        .to_string();

        let decoded: UiPreferences = serde_json::from_str(&json).expect("deserialize preferences");
        let reencoded = serde_json::to_value(&decoded).expect("serialize preferences");

        assert_eq!(
            reencoded["window_bounds"],
            serde_json::json!({ "x": 160, "y": 120 })
        );
    }

    #[test]
    fn ui_preferences_defaults_to_no_window_bounds() {
        let decoded: UiPreferences =
            serde_json::from_str("{}").expect("deserialize default preferences");

        assert_eq!(decoded.window_bounds, None);
    }
}
