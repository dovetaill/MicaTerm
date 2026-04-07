//! Supported theme modes for the shell and native window appearance synchronization.

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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTerminalPaletteSpec {
    pub name: &'static str,
    pub default_bg: u32,
    pub default_fg: u32,
    pub row_bg_even: u32,
    pub row_bg_odd: u32,
    pub cursor_bg: u32,
    pub cursor_fg: u32,
    pub selection_rgb: u32,
    pub selection_alpha: f32,
    pub scrollbar_thumb: u32,
    pub scrollbar_thumb_active: u32,
    pub split: u32,
    pub ansi: [u32; 16],
}

pub fn terminal_palette_spec(theme_mode: ThemeMode) -> ThemeTerminalPaletteSpec {
    match theme_mode {
        ThemeMode::Dark => ThemeTerminalPaletteSpec {
            name: "Catppuccin Mocha",
            default_bg: 0x1e_1e2e,
            default_fg: 0xcd_d6f4,
            row_bg_even: 0x1e_1e2e,
            row_bg_odd: 0x18_1825,
            cursor_bg: 0xcd_d6f4,
            cursor_fg: 0x1e_1e2e,
            selection_rgb: 0x58_5b70,
            selection_alpha: 0.36,
            scrollbar_thumb: 0x58_5b70,
            scrollbar_thumb_active: 0x6c_7086,
            split: 0x31_3244,
            ansi: [
                0x45_475a,
                0xf3_8ba8,
                0xa6_e3a1,
                0xf9_e2af,
                0x89_b4fa,
                0xf5_c2e7,
                0x94_e2d5,
                0xba_c2de,
                0x58_5b70,
                0xf3_8ba8,
                0xa6_e3a1,
                0xf9_e2af,
                0x89_b4fa,
                0xf5_c2e7,
                0x94_e2d5,
                0xa6_adc8,
            ],
        },
        ThemeMode::Light => ThemeTerminalPaletteSpec {
            name: "Catppuccin Latte",
            default_bg: 0xef_f1f5,
            default_fg: 0x4c_4f69,
            row_bg_even: 0xef_f1f5,
            row_bg_odd: 0xe6_e9ef,
            cursor_bg: 0x4c_4f69,
            cursor_fg: 0xef_f1f5,
            selection_rgb: 0xac_b0be,
            selection_alpha: 0.44,
            scrollbar_thumb: 0xac_b0be,
            scrollbar_thumb_active: 0x9c_a0b0,
            split: 0xcc_d0da,
            ansi: [
                0x5c_5f77,
                0xd2_0f39,
                0x40_a02b,
                0xdf_8e1d,
                0x1e_66f5,
                0xea_76cb,
                0x17_9299,
                0xac_b0be,
                0x6c_6f85,
                0xd2_0f39,
                0x40_a02b,
                0xdf_8e1d,
                0x1e_66f5,
                0xea_76cb,
                0x17_9299,
                0x7c_7f93,
            ],
        },
    }
}
