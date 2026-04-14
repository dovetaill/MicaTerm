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

#[allow(dead_code)]
pub const TERMINAL_ROW_BANDING_ENABLED: bool = false;
#[allow(dead_code)]
pub const TERMINAL_ROW_BANDING_ALPHA: f32 = 0.0;
#[allow(dead_code)]
pub const TERMINAL_BG_GRAIN_ALPHA: f32 = 0.0;

pub const TERMINAL_BG_BASE_DARK: u32 = 0x07_111a;
pub const TERMINAL_BG_GRADIENT_TOP_DARK: u32 = 0x0a_1621;
pub const TERMINAL_BG_GRADIENT_BOTTOM_DARK: u32 = 0x07_111a;

pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xfb_fcfe;
pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xfb_fcfe;
pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xfb_fcfe;

pub fn terminal_palette_spec(theme_mode: ThemeMode) -> ThemeTerminalPaletteSpec {
    match theme_mode {
        ThemeMode::Dark => ThemeTerminalPaletteSpec {
            name: "Mica Graphite",
            default_bg: TERMINAL_BG_BASE_DARK,
            default_fg: 0xe5_ebf5,
            row_bg_even: TERMINAL_BG_GRADIENT_TOP_DARK,
            row_bg_odd: TERMINAL_BG_GRADIENT_BOTTOM_DARK,
            cursor_bg: 0xe5_ebf5,
            cursor_fg: TERMINAL_BG_BASE_DARK,
            selection_rgb: 0x7c_92af,
            selection_alpha: 0.25,
            scrollbar_thumb: 0x4a_586a,
            scrollbar_thumb_active: 0x5c_6d82,
            split: 0x34_475c,
            ansi: [
                0x45_475a, 0xf3_8ba8, 0xa6_e3a1, 0xf9_e2af, 0x89_b4fa, 0xf5_c2e7, 0x94_e2d5,
                0xba_c2de, 0x58_5b70, 0xf3_8ba8, 0xa6_e3a1, 0xf9_e2af, 0x89_b4fa, 0xf5_c2e7,
                0x94_e2d5, 0xa6_adc8,
            ],
        },
        ThemeMode::Light => ThemeTerminalPaletteSpec {
            name: "Mica Canvas",
            default_bg: TERMINAL_BG_BASE_LIGHT,
            default_fg: 0x24_3142,
            row_bg_even: TERMINAL_BG_GRADIENT_TOP_LIGHT,
            row_bg_odd: TERMINAL_BG_GRADIENT_BOTTOM_LIGHT,
            cursor_bg: 0x24_3142,
            cursor_fg: TERMINAL_BG_BASE_LIGHT,
            selection_rgb: 0x95_add3,
            selection_alpha: 0.30,
            scrollbar_thumb: 0xbc_c8da,
            scrollbar_thumb_active: 0xa8_b8ce,
            split: 0xc7_d4e6,
            ansi: [
                0x5c_5f77, 0xd2_0f39, 0x40_a02b, 0xdf_8e1d, 0x1e_66f5, 0xea_76cb, 0x17_9299,
                0xac_b0be, 0x6c_6f85, 0xd2_0f39, 0x40_a02b, 0xdf_8e1d, 0x1e_66f5, 0xea_76cb,
                0x17_9299, 0x7c_7f93,
            ],
        },
    }
}
