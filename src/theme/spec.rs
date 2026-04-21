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
                app_background: 0x17_1c24,
                border: 0x34_47_5c,
            },
            terminal: TerminalTheme {
                name: "Mica Graphite",
                background: TerminalBackgroundTheme {
                    base: 0x0c_14_1c,
                    gradient_top: 0x10_19_24,
                    gradient_bottom: 0x0c_14_1c,
                },
                foreground: TerminalForegroundTheme {
                    default: 0xe3_ea_f2,
                    dim: 0x93_a0_b2,
                    soft: 0xc8_d1_dc,
                },
                cursor: TerminalCursorTheme {
                    background: 0xdc_e6_f3,
                    foreground: 0x0c_14_1c,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x6c_88_ae,
                    alpha: 66.0 / 255.0,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0x53_62_74,
                    thumb_active: 0x66_78_8e,
                },
                ansi: [
                    0x4a_52_60, 0xc3_7a_86, 0x86_b4_8f, 0xc6_a5_6a, 0x7d_9b_c2, 0xa7_8c_bf,
                    0x78_af_ae, 0xc8_d1_dc, 0x66_71_80, 0xd6_94_9f, 0x9b_c6_a4, 0xd8_ba_83,
                    0x94_ae_d0, 0xb7_9c_cb, 0x8e_c0_c0, 0xe7_ed_f4,
                ],
            },
            decoration: DecorationTheme {
                running: 0x7d_a8_d9,
                success: 0x7b_c5_93,
                failure: 0xde_8b_95,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x4f_c3_f7,
                output_accent: 0x28_7d_ff,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "premium-default-light",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: ShellChromeTheme {
                app_background: 0xf6_f8_fb,
                border: 0xc7_d4_e6,
            },
            terminal: TerminalTheme {
                name: "Mica Canvas",
                background: TerminalBackgroundTheme {
                    base: 0xf8_fa_fc,
                    gradient_top: 0xfb_fc_fd,
                    gradient_bottom: 0xf6_f8_fb,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x26_32_40,
                    dim: 0x75_83_95,
                    soft: 0x4b_59_6a,
                },
                cursor: TerminalCursorTheme {
                    background: 0x2c_39_48,
                    foreground: 0xf8_fa_fc,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x7f_9b_c2,
                    alpha: 51.0 / 255.0,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0xb7_c3_d0,
                    thumb_active: 0x9f_af_be,
                },
                ansi: [
                    0x5a_65_73, 0xb8_64_70, 0x5f_8d_69, 0x9d_7c_41, 0x5b_80_ae, 0x8f_73_aa,
                    0x4e_90_90, 0xaa_b5_c1, 0x73_80_90, 0xc7_7a_85, 0x76_9e_7e, 0xb0_8d_53,
                    0x72_95_bf, 0xa2_86_bb, 0x68_a4_a3, 0xd5_dc_e4,
                ],
            },
            decoration: DecorationTheme {
                running: 0x5f_87_c0,
                success: 0x4d_8f_66,
                failure: 0xc3_64_73,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x3d_77_bf,
                output_accent: 0x1e_66_f5,
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
