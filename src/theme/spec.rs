//! Supported theme modes and theme variants shared across shell and terminal surfaces.

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ThemeVariant {
    #[default]
    PremiumDefault,
    LegacyHackerGreen,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellChromeTheme {
    pub app_background: u32,
    pub border: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBackgroundTheme {
    pub base: u32,
    pub gradient_top: u32,
    pub gradient_bottom: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalForegroundTheme {
    pub default: u32,
    pub dim: u32,
    pub soft: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalCursorTheme {
    pub background: u32,
    pub foreground: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalOverlayTheme {
    pub rgb: u32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalScrollbarTheme {
    pub thumb: u32,
    pub thumb_active: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalTheme {
    pub name: &'static str,
    pub background: TerminalBackgroundTheme,
    pub foreground: TerminalForegroundTheme,
    pub cursor: TerminalCursorTheme,
    pub selection: TerminalOverlayTheme,
    pub scrollbar: TerminalScrollbarTheme,
    pub ansi: [u32; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationTheme {
    pub running: u32,
    pub success: u32,
    pub failure: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticHighlightTheme {
    pub input_command: u32,
    pub output_accent: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppThemeSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub variant: ThemeVariant,
    pub mode: ThemeMode,
    pub shell: ShellChromeTheme,
    pub terminal: TerminalTheme,
    pub decoration: DecorationTheme,
    pub semantic: SemanticHighlightTheme,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThemeTerminalPaletteSpec {
    pub name: &'static str,
    pub default_bg: u32,
    pub default_fg: u32,
    // Historical compatibility transport: renderer backends should interpret these as
    // viewport background endpoints, not alternating row stripe colors.
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

pub fn app_theme_spec(mode: ThemeMode, variant: ThemeVariant) -> AppThemeSpec {
    match variant {
        ThemeVariant::PremiumDefault => premium_default_spec(mode),
        ThemeVariant::LegacyHackerGreen => legacy_hacker_green_spec(mode),
    }
}

pub fn terminal_palette_spec(theme_mode: ThemeMode) -> ThemeTerminalPaletteSpec {
    terminal_palette_spec_for(theme_mode, ThemeVariant::PremiumDefault)
}

pub fn terminal_palette_spec_for(
    theme_mode: ThemeMode,
    variant: ThemeVariant,
) -> ThemeTerminalPaletteSpec {
    let spec = app_theme_spec(theme_mode, variant);
    ThemeTerminalPaletteSpec {
        name: spec.terminal.name,
        default_bg: spec.terminal.background.base,
        default_fg: spec.terminal.foreground.default,
        row_bg_even: spec.terminal.background.gradient_top,
        row_bg_odd: spec.terminal.background.gradient_bottom,
        cursor_bg: spec.terminal.cursor.background,
        cursor_fg: spec.terminal.cursor.foreground,
        selection_rgb: spec.terminal.selection.rgb,
        selection_alpha: spec.terminal.selection.alpha,
        scrollbar_thumb: spec.terminal.scrollbar.thumb,
        scrollbar_thumb_active: spec.terminal.scrollbar.thumb_active,
        split: spec.shell.border,
        ansi: spec.terminal.ansi,
    }
}

fn premium_default_spec(mode: ThemeMode) -> AppThemeSpec {
    match mode {
        ThemeMode::Dark => AppThemeSpec {
            id: "premium-default-dark",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: ShellChromeTheme {
                app_background: 0x0f_16_1d,
                border: 0x2d_3a_48,
            },
            terminal: TerminalTheme {
                name: "Mica Graphite",
                background: TerminalBackgroundTheme {
                    base: 0x08_13_1d,
                    gradient_top: 0x0d_18_22,
                    gradient_bottom: 0x08_13_1d,
                },
                foreground: TerminalForegroundTheme {
                    default: 0xd7_e0_e8,
                    dim: 0x8f_a0_ae,
                    soft: 0xc0_cad4,
                },
                cursor: TerminalCursorTheme {
                    background: 0xdc_e6_ee,
                    foreground: 0x08_13_1d,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x7a_8f_a9,
                    alpha: 74.0 / 255.0,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0x5a_6a_79,
                    thumb_active: 0x72_84_95,
                },
                ansi: [
                    0x3e_4a_57, 0xc9_7d_88, 0x7f_b0_8d, 0xc6_a0_66, 0x7f_9e_c4, 0xa8_8d_bf,
                    0x74_b1_b7, 0xcb_d5_df, 0x5f_6d_7c, 0xd9_93_9d, 0x97_c3_a1, 0xd8_b7_80,
                    0x9a_b5_d6, 0xbe_a2_d1, 0x90_c8_cc, 0xec_f2_f7,
                ],
            },
            decoration: DecorationTheme {
                running: 0x7d_97_b8,
                success: 0x7f_b0_8d,
                failure: 0xc9_7d_88,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x7d_97_b8,
                output_accent: 0x7f_9e_c4,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "premium-default-light",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: ShellChromeTheme {
                app_background: 0xe8_ed_f1,
                border: 0xc9_d3_dd,
            },
            terminal: TerminalTheme {
                name: "Mica Canvas",
                background: TerminalBackgroundTheme {
                    base: 0xf4_f6_f8,
                    gradient_top: 0xf8_f9_fb,
                    gradient_bottom: 0xf1_f4_f7,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x1f_29_33,
                    dim: 0x6c_7a_86,
                    soft: 0x4756_63,
                },
                cursor: TerminalCursorTheme {
                    background: 0x24_31_3c,
                    foreground: 0xf4_f6_f8,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x78_95_b3,
                    alpha: 58.0 / 255.0,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0xb6_c0_ca,
                    thumb_active: 0x9f_ac_b8,
                },
                ansi: [
                    0x4e_5c_6a, 0xb7_64_70, 0x5f_89_69, 0x9b_7a_40, 0x56_7c_a8, 0x86_6e_a2,
                    0x4c_8d_8f, 0xa7_b4_bf, 0x6c_7b_89, 0xc8_79_84, 0x76_9d_7d, 0xad_8b_54,
                    0x70_95_bf, 0x9c_83_b6, 0x66_a4_a7, 0xd9_e0_e6,
                ],
            },
            decoration: DecorationTheme {
                running: 0x6b_87_ab,
                success: 0x5f_89_69,
                failure: 0xb7_64_70,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x6b_87_ab,
                output_accent: 0x56_7c_a8,
            },
        },
    }
}

fn legacy_hacker_green_spec(mode: ThemeMode) -> AppThemeSpec {
    match mode {
        ThemeMode::Dark => AppThemeSpec {
            id: "legacy-hacker-green-dark",
            name: "Legacy Hacker Green",
            variant: ThemeVariant::LegacyHackerGreen,
            mode,
            shell: ShellChromeTheme {
                app_background: 0x0c_12_0f,
                border: 0x1f_57_36,
            },
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0x05_0b_08,
                    gradient_top: 0x08_12_0d,
                    gradient_bottom: 0x05_0b_08,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x98_f5_b3,
                    dim: 0x4b_8d_64,
                    soft: 0x74_c0_8b,
                },
                cursor: TerminalCursorTheme {
                    background: 0x98_f5_b3,
                    foreground: 0x05_0b_08,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x2d_80_55,
                    alpha: 0.32,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0x25_53_3a,
                    thumb_active: 0x31_68_48,
                },
                ansi: [
                    0x1b_26_20, 0xc8_74_74, 0x72_d7_91, 0xc9_c7_6d, 0x5e_b5_f5, 0xb1_8b_f2,
                    0x57_d0_c8, 0xb9_f0_c8, 0x2e_4d_3b, 0xde_8b_95, 0x8e_e5_ab, 0xe0_d7_84,
                    0x89_b4_fa, 0xc2_a8_f7, 0x7b_dc_d4, 0xda_f7_e2,
                ],
            },
            decoration: DecorationTheme {
                running: 0x72_d7_91,
                success: 0x98_f5_b3,
                failure: 0xc8_74_74,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x72_d7_91,
                output_accent: 0x57_d0_c8,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "legacy-hacker-green-light",
            name: "Legacy Hacker Green",
            variant: ThemeVariant::LegacyHackerGreen,
            mode,
            shell: ShellChromeTheme {
                app_background: 0xf1_f7_f2,
                border: 0x9d_c2_a6,
            },
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0xee_f6_ef,
                    gradient_top: 0xf6_fb_f7,
                    gradient_bottom: 0xee_f6_ef,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x1e_32_26,
                    dim: 0x5b_73_67,
                    soft: 0x3c_54_47,
                },
                cursor: TerminalCursorTheme {
                    background: 0x1e_32_26,
                    foreground: 0xee_f6_ef,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x8d_b9_98,
                    alpha: 0.28,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0xaf_c8_b6,
                    thumb_active: 0x92_b2_9d,
                },
                ansi: [
                    0x5a_68_60, 0xb4_65_67, 0x53_8b_64, 0x96_8b_43, 0x4c_7b_b0, 0x8d_70_b0,
                    0x4b_8e_88, 0xa9_b9_b0, 0x73_85_7b, 0xc5_78_7b, 0x69_9b_78, 0xac_a0_5c,
                    0x66_92_c1, 0xa0_82_c1, 0x63_a2_9b, 0xd2_dd_d7,
                ],
            },
            decoration: DecorationTheme {
                running: 0x69_9b_78,
                success: 0x53_8b_64,
                failure: 0xb4_65_67,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x53_8b_64,
                output_accent: 0x4c_7b_b0,
            },
        },
    }
}
