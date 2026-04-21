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
pub struct SemanticInkTheme {
    pub fg: Option<u32>,
    pub tint: Option<u32>,
    pub underline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticHighlightTheme {
    pub input_prompt: SemanticInkTheme,
    pub input_command: SemanticInkTheme,
    pub input_path: SemanticInkTheme,
    pub input_option: SemanticInkTheme,
    pub input_operator: SemanticInkTheme,
    pub input_variable: SemanticInkTheme,
    pub input_string: SemanticInkTheme,
    pub input_invalid: SemanticInkTheme,
    pub output_accent: SemanticInkTheme,
    pub output_muted: SemanticInkTheme,
    pub output_info: SemanticInkTheme,
    pub output_warn: SemanticInkTheme,
    pub output_error: SemanticInkTheme,
    pub output_success: SemanticInkTheme,
    pub output_failure: SemanticInkTheme,
    pub output_added: SemanticInkTheme,
    pub output_removed: SemanticInkTheme,
    pub output_json_key: SemanticInkTheme,
    pub output_json_value: SemanticInkTheme,
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

pub fn app_theme_spec_from_terminal_background(default_bg_rgba: u32) -> AppThemeSpec {
    match default_bg_rgba & 0x00ff_ffff {
        0x08_13_1d => app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault),
        0xf4_f6_f8 => app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault),
        0x05_0b_08 => app_theme_spec(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen),
        0xef_f6_f1 => app_theme_spec(ThemeMode::Light, ThemeVariant::LegacyHackerGreen),
        _ => app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault),
    }
}

fn semantic_ink(fg: Option<u32>, tint: Option<u32>, underline: bool) -> SemanticInkTheme {
    SemanticInkTheme {
        fg,
        tint,
        underline,
    }
}

fn semantic_tint(rgb: u32, alpha: u8) -> u32 {
    (u32::from(alpha) << 24) | (rgb & 0x00ff_ffff)
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
                input_prompt: semantic_ink(Some(0x8f_a0_ae), None, false),
                input_command: semantic_ink(
                    Some(0x7d_97_b8),
                    Some(semantic_tint(0x7d_97_b8, 0x18)),
                    true,
                ),
                input_path: semantic_ink(Some(0x7f_9e_c4), None, true),
                input_option: semantic_ink(Some(0xc0_ca_d4), None, false),
                input_operator: semantic_ink(Some(0x8f_a0_ae), None, false),
                input_variable: semantic_ink(
                    Some(0x96_af_ca),
                    Some(semantic_tint(0x96_af_ca, 0x14)),
                    false,
                ),
                input_string: semantic_ink(Some(0xc0_ca_d4), None, false),
                input_invalid: semantic_ink(
                    Some(0xc9_7d_88),
                    Some(semantic_tint(0xc9_7d_88, 0x18)),
                    true,
                ),
                output_accent: semantic_ink(
                    Some(0x7f_9e_c4),
                    Some(semantic_tint(0x7f_9e_c4, 0x14)),
                    true,
                ),
                output_muted: semantic_ink(Some(0xc0_ca_d4), None, false),
                output_info: semantic_ink(
                    Some(0xc0_ca_d4),
                    Some(semantic_tint(0x7f_9e_c4, 0x10)),
                    false,
                ),
                output_warn: semantic_ink(
                    Some(0xc6_a0_66),
                    Some(semantic_tint(0xc6_a0_66, 0x12)),
                    false,
                ),
                output_error: semantic_ink(
                    Some(0xc9_7d_88),
                    Some(semantic_tint(0xc9_7d_88, 0x16)),
                    true,
                ),
                output_success: semantic_ink(
                    Some(0x7f_b0_8d),
                    Some(semantic_tint(0x7f_b0_8d, 0x12)),
                    false,
                ),
                output_failure: semantic_ink(
                    Some(0xc9_7d_88),
                    Some(semantic_tint(0xc9_7d_88, 0x14)),
                    true,
                ),
                output_added: semantic_ink(
                    Some(0x97_c3_a1),
                    Some(semantic_tint(0x97_c3_a1, 0x10)),
                    false,
                ),
                output_removed: semantic_ink(
                    Some(0xd9_93_9d),
                    Some(semantic_tint(0xd9_93_9d, 0x10)),
                    false,
                ),
                output_json_key: semantic_ink(Some(0x96_af_ca), None, false),
                output_json_value: semantic_ink(Some(0xc0_ca_d4), None, false),
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
                input_prompt: semantic_ink(Some(0x6c_7a_86), None, false),
                input_command: semantic_ink(
                    Some(0x6b_87_ab),
                    Some(semantic_tint(0x6b_87_ab, 0x14)),
                    true,
                ),
                input_path: semantic_ink(Some(0x56_7c_a8), None, true),
                input_option: semantic_ink(Some(0x47_56_63), None, false),
                input_operator: semantic_ink(Some(0x6c_7a_86), None, false),
                input_variable: semantic_ink(
                    Some(0x6b_87_ab),
                    Some(semantic_tint(0x6b_87_ab, 0x10)),
                    false,
                ),
                input_string: semantic_ink(Some(0x47_56_63), None, false),
                input_invalid: semantic_ink(
                    Some(0xb7_64_70),
                    Some(semantic_tint(0xb7_64_70, 0x14)),
                    true,
                ),
                output_accent: semantic_ink(
                    Some(0x56_7c_a8),
                    Some(semantic_tint(0x56_7c_a8, 0x10)),
                    true,
                ),
                output_muted: semantic_ink(Some(0x47_56_63), None, false),
                output_info: semantic_ink(
                    Some(0x47_56_63),
                    Some(semantic_tint(0x56_7c_a8, 0x0e)),
                    false,
                ),
                output_warn: semantic_ink(
                    Some(0x9b_7a_40),
                    Some(semantic_tint(0x9b_7a_40, 0x10)),
                    false,
                ),
                output_error: semantic_ink(
                    Some(0xb7_64_70),
                    Some(semantic_tint(0xb7_64_70, 0x14)),
                    true,
                ),
                output_success: semantic_ink(
                    Some(0x5f_89_69),
                    Some(semantic_tint(0x5f_89_69, 0x10)),
                    false,
                ),
                output_failure: semantic_ink(
                    Some(0xb7_64_70),
                    Some(semantic_tint(0xb7_64_70, 0x12)),
                    true,
                ),
                output_added: semantic_ink(
                    Some(0x76_9d_7d),
                    Some(semantic_tint(0x76_9d_7d, 0x0e)),
                    false,
                ),
                output_removed: semantic_ink(
                    Some(0xc8_79_84),
                    Some(semantic_tint(0xc8_79_84, 0x0e)),
                    false,
                ),
                output_json_key: semantic_ink(Some(0x6b_87_ab), None, false),
                output_json_value: semantic_ink(Some(0x47_56_63), None, false),
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
                app_background: 0x0b_12_0f,
                border: 0x23_43_33,
            },
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0x05_0b_08,
                    gradient_top: 0x08_10_0c,
                    gradient_bottom: 0x05_0b_08,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x9b_e6_b3,
                    dim: 0x5e_8a_6d,
                    soft: 0x7d_bf_92,
                },
                cursor: TerminalCursorTheme {
                    background: 0xb4_f0_c6,
                    foreground: 0x05_0b_08,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x3f_7a_57,
                    alpha: 0.25,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0x30_58_41,
                    thumb_active: 0x3f_72_56,
                },
                ansi: [
                    0x24_31_29, 0xc0_7a_7a, 0x73_c0_8c, 0xb8_b9_6a, 0x6f_a6_d8, 0x9b_88_c8,
                    0x63_bd_b5, 0xbe_e7_c9, 0x39_51_43, 0xd2_8f_8f, 0x8e_db_a5, 0xd2_d4_7f,
                    0x8a_bc_f0, 0xb1_9b_e0, 0x7c_d4_cc, 0xe4_f8_ea,
                ],
            },
            decoration: DecorationTheme {
                running: 0x69_b3_7f,
                success: 0x73_c0_8c,
                failure: 0xc0_7a_7a,
            },
            semantic: SemanticHighlightTheme {
                input_prompt: semantic_ink(Some(0x5e_8a_6d), None, false),
                input_command: semantic_ink(
                    Some(0x69_b3_7f),
                    Some(semantic_tint(0x69_b3_7f, 0x18)),
                    true,
                ),
                input_path: semantic_ink(Some(0x63_bd_b5), None, true),
                input_option: semantic_ink(Some(0x7d_bf_92), None, false),
                input_operator: semantic_ink(Some(0x5e_8a_6d), None, false),
                input_variable: semantic_ink(
                    Some(0x8e_db_a5),
                    Some(semantic_tint(0x8e_db_a5, 0x14)),
                    false,
                ),
                input_string: semantic_ink(Some(0x7d_bf_92), None, false),
                input_invalid: semantic_ink(
                    Some(0xc0_7a_7a),
                    Some(semantic_tint(0xc0_7a_7a, 0x18)),
                    true,
                ),
                output_accent: semantic_ink(
                    Some(0x63_bd_b5),
                    Some(semantic_tint(0x63_bd_b5, 0x14)),
                    true,
                ),
                output_muted: semantic_ink(Some(0x7d_bf_92), None, false),
                output_info: semantic_ink(
                    Some(0x7d_bf_92),
                    Some(semantic_tint(0x63_bd_b5, 0x10)),
                    false,
                ),
                output_warn: semantic_ink(
                    Some(0xb8_b9_6a),
                    Some(semantic_tint(0xb8_b9_6a, 0x12)),
                    false,
                ),
                output_error: semantic_ink(
                    Some(0xc0_7a_7a),
                    Some(semantic_tint(0xc0_7a_7a, 0x16)),
                    true,
                ),
                output_success: semantic_ink(
                    Some(0x73_c0_8c),
                    Some(semantic_tint(0x73_c0_8c, 0x12)),
                    false,
                ),
                output_failure: semantic_ink(
                    Some(0xc0_7a_7a),
                    Some(semantic_tint(0xc0_7a_7a, 0x14)),
                    true,
                ),
                output_added: semantic_ink(
                    Some(0x8e_db_a5),
                    Some(semantic_tint(0x8e_db_a5, 0x10)),
                    false,
                ),
                output_removed: semantic_ink(
                    Some(0xd2_8f_8f),
                    Some(semantic_tint(0xd2_8f_8f, 0x10)),
                    false,
                ),
                output_json_key: semantic_ink(Some(0x8e_db_a5), None, false),
                output_json_value: semantic_ink(Some(0x7d_bf_92), None, false),
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "legacy-hacker-green-light",
            name: "Legacy Hacker Green",
            variant: ThemeVariant::LegacyHackerGreen,
            mode,
            shell: ShellChromeTheme {
                app_background: 0xee_f5_f0,
                border: 0xaf_c6_b5,
            },
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0xef_f6_f1,
                    gradient_top: 0xf7_fb_f8,
                    gradient_bottom: 0xec_f4_ee,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x21_31_28,
                    dim: 0x6b_7e_74,
                    soft: 0x45_5d_52,
                },
                cursor: TerminalCursorTheme {
                    background: 0x1e_30_26,
                    foreground: 0xef_f6_f1,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x9a_b9_a4,
                    alpha: 0.29,
                },
                scrollbar: TerminalScrollbarTheme {
                    thumb: 0xb4_cb_bb,
                    thumb_active: 0x98_b4_a2,
                },
                ansi: [
                    0x56_67_5d, 0xb8_67_67, 0x5e_95_6d, 0x9c_94_45, 0x5f_88_b4, 0x85_6e_a8,
                    0x4f_97_91, 0xaa_bc_b1, 0x74_86_7a, 0xca_7c_7c, 0x75_a8_84, 0xb0_a8_5d,
                    0x75_9d_c6, 0x9a_84_ba, 0x68_ae_a7, 0xd9_e5_dd,
                ],
            },
            decoration: DecorationTheme {
                running: 0x5e_94_6f,
                success: 0x5e_95_6d,
                failure: 0xb8_67_67,
            },
            semantic: SemanticHighlightTheme {
                input_prompt: semantic_ink(Some(0x6b_7e_74), None, false),
                input_command: semantic_ink(
                    Some(0x4e_8a_63),
                    Some(semantic_tint(0x4e_8a_63, 0x14)),
                    true,
                ),
                input_path: semantic_ink(Some(0x5f_88_b4), None, true),
                input_option: semantic_ink(Some(0x45_5d_52), None, false),
                input_operator: semantic_ink(Some(0x6b_7e_74), None, false),
                input_variable: semantic_ink(
                    Some(0x75_a8_84),
                    Some(semantic_tint(0x75_a8_84, 0x10)),
                    false,
                ),
                input_string: semantic_ink(Some(0x45_5d_52), None, false),
                input_invalid: semantic_ink(
                    Some(0xb8_67_67),
                    Some(semantic_tint(0xb8_67_67, 0x14)),
                    true,
                ),
                output_accent: semantic_ink(
                    Some(0x5f_88_b4),
                    Some(semantic_tint(0x5f_88_b4, 0x10)),
                    true,
                ),
                output_muted: semantic_ink(Some(0x45_5d_52), None, false),
                output_info: semantic_ink(
                    Some(0x45_5d_52),
                    Some(semantic_tint(0x5f_88_b4, 0x0e)),
                    false,
                ),
                output_warn: semantic_ink(
                    Some(0x9c_94_45),
                    Some(semantic_tint(0x9c_94_45, 0x10)),
                    false,
                ),
                output_error: semantic_ink(
                    Some(0xb8_67_67),
                    Some(semantic_tint(0xb8_67_67, 0x12)),
                    true,
                ),
                output_success: semantic_ink(
                    Some(0x5e_95_6d),
                    Some(semantic_tint(0x5e_95_6d, 0x10)),
                    false,
                ),
                output_failure: semantic_ink(
                    Some(0xb8_67_67),
                    Some(semantic_tint(0xb8_67_67, 0x12)),
                    true,
                ),
                output_added: semantic_ink(
                    Some(0x75_a8_84),
                    Some(semantic_tint(0x75_a8_84, 0x0e)),
                    false,
                ),
                output_removed: semantic_ink(
                    Some(0xca_7c_7c),
                    Some(semantic_tint(0xca_7c_7c, 0x0e)),
                    false,
                ),
                output_json_key: semantic_ink(Some(0x4e_8a_63), None, false),
                output_json_value: semantic_ink(Some(0x45_5d_52), None, false),
            },
        },
    }
}
