use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn toggled(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeSpec {
    pub accent_name: &'static str,
    pub terminal_is_neutral: bool,
    pub panel_uses_tint: bool,
    pub supports_dark: bool,
    pub supports_light: bool,
}

pub fn theme_spec(mode: ThemeMode) -> ThemeSpec {
    match mode {
        ThemeMode::Dark => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
        ThemeMode::Light => ThemeSpec {
            accent_name: "electric-blue",
            terminal_is_neutral: true,
            panel_uses_tint: true,
            supports_dark: true,
            supports_light: true,
        },
    }
}
