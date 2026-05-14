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

impl ThemeVariant {
    pub fn id(self) -> &'static str {
        match self {
            Self::PremiumDefault => "premium_default",
            Self::LegacyHackerGreen => "legacy_hacker_green",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "legacy_hacker_green" => Self::LegacyHackerGreen,
            _ => Self::PremiumDefault,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SearchMatchHighlightStrength {
    Subtle,
    #[default]
    Balanced,
    Strong,
}

impl SearchMatchHighlightStrength {
    pub fn id(self) -> &'static str {
        match self {
            Self::Subtle => "subtle",
            Self::Balanced => "balanced",
            Self::Strong => "strong",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "subtle" => Self::Subtle,
            "strong" => Self::Strong,
            _ => Self::Balanced,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SemanticStyleRole {
    InputPrompt,
    InputCommand,
    InputSubcommand,
    InputOption,
    InputArgument,
    InputString,
    InputPath,
    InputVariable,
    InputOperator,
    InputInvalidCommand,
    OutputUrl,
    OutputUnixPath,
    OutputWindowsPath,
    OutputLineReference,
    OutputNetworkEndpoint,
    OutputTimestamp,
    OutputSeverityError,
    OutputSeverityWarning,
    OutputSeverityInfo,
    OutputSeverityDebug,
    OutputSuccessKeyword,
    OutputFailureKeyword,
    OutputGrepMatch,
    OutputDiffAdded,
    OutputDiffRemoved,
    OutputDiffHunk,
    OutputJsonKey,
    OutputJsonString,
    OutputJsonNumber,
    OutputJsonBoolean,
    CommandStatusRunning,
    CommandStatusSuccess,
    CommandStatusFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemanticRoleStyle {
    pub foreground: u32,
    pub background: Option<u32>,
    pub bold: bool,
    pub underline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellChromeTheme {
    pub app_background: u32,
    pub titlebar_background: u32,
    pub tabbar_background: u32,
    pub sidebar_background: u32,
    pub sidebar_panel_background: u32,
    pub right_panel_background: u32,
    pub terminal_frame_background: u32,
    pub separator: u32,
    pub border: u32,
    pub hairline: u32,
    pub text_primary: u32,
    pub text_secondary: u32,
    pub text_muted: u32,
    pub text_inactive: u32,
    pub accent: u32,
    pub link_accent: u32,
    pub focus_ring: u32,
    pub tab_active: u32,
    pub tab_inactive: u32,
    pub tab_hover: u32,
    pub tab_active_indicator: u32,
    pub sidebar_item_hover: u32,
    pub sidebar_item_selected: u32,
    pub sidebar_item_selected_border: u32,
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
    pub soft: u32,
    pub dim: u32,
    pub inactive: u32,
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
    pub track: u32,
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
    pub search_match: TerminalOverlayTheme,
    pub current_search_match: TerminalOverlayTheme,
    pub scrollbar: TerminalScrollbarTheme,
    pub ansi: [u32; 16],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecorationTheme {
    pub success: u32,
    pub warning: u32,
    pub error: u32,
    pub info: u32,
    pub running: u32,
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
    pub scrollbar_track: u32,
    pub scrollbar_thumb: u32,
    pub scrollbar_thumb_active: u32,
    pub frame_bg: u32,
    pub split: u32,
    pub ansi: [u32; 16],
}

#[allow(dead_code)]
pub const TERMINAL_ROW_BANDING_ENABLED: bool = false;
#[allow(dead_code)]
pub const TERMINAL_ROW_BANDING_ALPHA: f32 = 0.0;
#[allow(dead_code)]
pub const TERMINAL_BG_GRAIN_ALPHA: f32 = 0.0;
pub const TERMINAL_BG_BASE_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_GRADIENT_TOP_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_GRADIENT_BOTTOM_DARK: u32 = 0x0a_0e14;
pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xf8_f9fa;
pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xf8_f9fa;

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
        scrollbar_track: spec.terminal.scrollbar.track,
        scrollbar_thumb: spec.terminal.scrollbar.thumb,
        scrollbar_thumb_active: spec.terminal.scrollbar.thumb_active,
        frame_bg: spec.shell.terminal_frame_background,
        split: spec.shell.border,
        ansi: spec.terminal.ansi,
    }
}

impl AppThemeSpec {
    pub fn semantic_style(self, role: SemanticStyleRole) -> SemanticRoleStyle {
        let shell = self.shell;
        let terminal = self.terminal;
        let decoration = self.decoration;
        let semantic = self.semantic;
        match role {
            SemanticStyleRole::InputPrompt => SemanticRoleStyle {
                foreground: terminal.foreground.default,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::InputCommand => SemanticRoleStyle {
                foreground: semantic.input_command,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::InputSubcommand => SemanticRoleStyle {
                foreground: semantic.output_accent,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::InputOption
            | SemanticStyleRole::OutputJsonNumber
            | SemanticStyleRole::OutputJsonBoolean => SemanticRoleStyle {
                foreground: decoration.warning,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::InputArgument => SemanticRoleStyle {
                foreground: terminal.foreground.default,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::InputString | SemanticStyleRole::OutputJsonString => {
                SemanticRoleStyle {
                    foreground: terminal.foreground.soft,
                    background: None,
                    bold: false,
                    underline: false,
                }
            }
            SemanticStyleRole::InputPath
            | SemanticStyleRole::OutputUnixPath
            | SemanticStyleRole::OutputWindowsPath
            | SemanticStyleRole::OutputNetworkEndpoint => SemanticRoleStyle {
                foreground: semantic.output_accent,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::OutputUrl => SemanticRoleStyle {
                foreground: semantic.output_accent,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::OutputLineReference => SemanticRoleStyle {
                foreground: semantic.output_accent,
                background: None,
                bold: false,
                underline: false,
            },
            SemanticStyleRole::InputVariable | SemanticStyleRole::OutputTimestamp => {
                SemanticRoleStyle {
                    foreground: decoration.info,
                    background: None,
                    bold: false,
                    underline: false,
                }
            }
            SemanticStyleRole::InputOperator | SemanticStyleRole::OutputJsonKey => {
                SemanticRoleStyle {
                    foreground: shell.text_secondary,
                    background: None,
                    bold: true,
                    underline: false,
                }
            }
            SemanticStyleRole::InputInvalidCommand
            | SemanticStyleRole::OutputSeverityError
            | SemanticStyleRole::OutputFailureKeyword
            | SemanticStyleRole::CommandStatusFailure
            | SemanticStyleRole::OutputDiffRemoved => SemanticRoleStyle {
                foreground: decoration.error,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::OutputSeverityWarning => SemanticRoleStyle {
                foreground: decoration.warning,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::OutputSeverityInfo
            | SemanticStyleRole::OutputSeverityDebug
            | SemanticStyleRole::OutputDiffHunk => SemanticRoleStyle {
                foreground: decoration.info,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::OutputSuccessKeyword
            | SemanticStyleRole::CommandStatusSuccess
            | SemanticStyleRole::OutputDiffAdded => SemanticRoleStyle {
                foreground: decoration.success,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::CommandStatusRunning => SemanticRoleStyle {
                foreground: decoration.running,
                background: None,
                bold: true,
                underline: false,
            },
            SemanticStyleRole::OutputGrepMatch => SemanticRoleStyle {
                foreground: semantic.output_accent,
                background: Some(terminal.search_match.rgb),
                bold: true,
                underline: false,
            },
        }
    }
}

fn premium_default_spec(mode: ThemeMode) -> AppThemeSpec {
    match mode {
        ThemeMode::Dark => AppThemeSpec {
            id: "premium-default-dark",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: premium_shell_dark(),
            terminal: TerminalTheme {
                name: "Ayu Dark",
                background: TerminalBackgroundTheme {
                    base: TERMINAL_BG_BASE_DARK,
                    gradient_top: TERMINAL_BG_GRADIENT_TOP_DARK,
                    gradient_bottom: TERMINAL_BG_GRADIENT_BOTTOM_DARK,
                },
                foreground: TerminalForegroundTheme {
                    default: 0xc5_c1b8,
                    soft: 0x99_a1a8,
                    dim: 0x5c_6773,
                    inactive: 0x82_8c99,
                },
                cursor: TerminalCursorTheme {
                    background: 0xe6_b450,
                    foreground: TERMINAL_BG_BASE_DARK,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x2a_3541,
                    alpha: 0.78,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0x4c_4126,
                    alpha: 0.34,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0x6b_5300,
                    alpha: 0.36,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0x11_1821,
                    thumb: 0x2f_3944,
                    thumb_active: 0x3c_4856,
                },
                ansi: [
                    0x01_060e, 0xea_6c73, 0x91_b362, 0xf9_af4f, 0x53_bdfa, 0xfa_e994, 0x90_e1c6,
                    0xc7_c7c7, 0x68_6868, 0xf0_7178, 0xc2_d94c, 0xff_b454, 0x59_c2ff, 0xff_ee99,
                    0x95_e6cb, 0xff_ffff,
                ],
            },
            decoration: DecorationTheme {
                success: 0x91_b362,
                warning: 0xff_b454,
                error: 0xf0_7178,
                info: 0x59_c2ff,
                running: 0xe6_b450,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x59_c2ff,
                output_accent: 0x95_e6cb,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "premium-default-light",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: premium_shell_light(),
            terminal: TerminalTheme {
                name: "Ayu Light",
                background: TerminalBackgroundTheme {
                    base: TERMINAL_BG_BASE_LIGHT,
                    gradient_top: TERMINAL_BG_GRADIENT_TOP_LIGHT,
                    gradient_bottom: TERMINAL_BG_GRADIENT_BOTTOM_LIGHT,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x5c_6166,
                    soft: 0x6c_7680,
                    dim: 0x82_8c99,
                    inactive: 0x6b_7480,
                },
                cursor: TerminalCursorTheme {
                    background: 0xff_aa33,
                    foreground: TERMINAL_BG_BASE_LIGHT,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x55_b4d4,
                    alpha: 0.20,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0xf8_dfa0,
                    alpha: 0.34,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0xff_aa33,
                    alpha: 0.28,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0xf4_f6f8,
                    thumb: 0xd6_dce3,
                    thumb_active: 0xc6_cdd6,
                },
                ansi: [
                    0x00_0000, 0xea_6c6d, 0x6c_bf43, 0xec_a944, 0x31_99e1, 0x9e_75c7, 0x46_ba94,
                    0xc7_c7c7, 0x68_6868, 0xf0_7171, 0x86_b300, 0xf2_ae49, 0x39_9ee6, 0xa3_7acc,
                    0x4c_bf99, 0xd1_d1d1,
                ],
            },
            decoration: DecorationTheme {
                success: 0x86_b300,
                warning: 0xf2_ae49,
                error: 0xf0_7171,
                info: 0x39_9ee6,
                running: 0xff_aa33,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x39_9ee6,
                output_accent: 0x55_b4d4,
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
            shell: premium_shell_dark(),
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0x07_100b,
                    gradient_top: 0x0a_140e,
                    gradient_bottom: 0x07_100b,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x9d_e7b6,
                    soft: 0x78_c695,
                    dim: 0x4f_8c67,
                    inactive: 0x8c_c9a3,
                },
                cursor: TerminalCursorTheme {
                    background: 0xa6_f2bf,
                    foreground: 0x07_100b,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x2d_8055,
                    alpha: 0.27,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0x7a_8e45,
                    alpha: 0.16,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0x6a_a883,
                    alpha: 0.23,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0x12_1d18,
                    thumb: 0x36_5d4b,
                    thumb_active: 0x43_735e,
                },
                ansi: [
                    0x27_4033, 0xb9_6c72, 0x71_b488, 0xb8_a15f, 0x5e_91c2, 0x92_7cb5, 0x63_aaa4,
                    0xbf_e2cc, 0x40_614f, 0xc9_858b, 0x8b_c8a0, 0xcc_b978, 0x79_a7d4, 0xa7_91c5,
                    0x7c_beb8, 0xe2_f5e9,
                ],
            },
            decoration: DecorationTheme {
                success: 0x71_b488,
                warning: 0xb8_a15f,
                error: 0xb9_6c72,
                info: 0x5e_91c2,
                running: 0x6e_b98b,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x6e_b98b,
                output_accent: 0x63_aaa4,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "legacy-hacker-green-light",
            name: "Legacy Hacker Green",
            variant: ThemeVariant::LegacyHackerGreen,
            mode,
            shell: premium_shell_light(),
            terminal: TerminalTheme {
                name: "Legacy Hacker Green",
                background: TerminalBackgroundTheme {
                    base: 0xf5_fbf7,
                    gradient_top: 0xfa_fdfc,
                    gradient_bottom: 0xf2_f8f4,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x1f_3a2d,
                    soft: 0x3b_6250,
                    dim: 0x6a_8d7a,
                    inactive: 0x59_7766,
                },
                cursor: TerminalCursorTheme {
                    background: 0x2e_5b47,
                    foreground: 0xf5_fbf7,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x6a_a883,
                    alpha: 0.19,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0xc7_d79c,
                    alpha: 0.24,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0x82_a98f,
                    alpha: 0.26,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0xe9_f1ec,
                    thumb: 0xab_c6b6,
                    thumb_active: 0x8f_b29e,
                },
                ansi: [
                    0x37_5645, 0xb0_6f76, 0x4c_8b67, 0x9a_8650, 0x5e_8db6, 0x8b_76a9, 0x58_9b95,
                    0xb5_d7c4, 0x58_7967, 0xbf_8087, 0x63_9c76, 0xaf_995f, 0x75_9fc3, 0x9b_87b8,
                    0x6e_ada6, 0xda_eee1,
                ],
            },
            decoration: DecorationTheme {
                success: 0x4c_8b67,
                warning: 0x9a_8650,
                error: 0xb0_6f76,
                info: 0x5e_8db6,
                running: 0x5f_966f,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x4c_8b67,
                output_accent: 0x58_9b95,
            },
        },
    }
}

fn premium_shell_dark() -> ShellChromeTheme {
    ShellChromeTheme {
        app_background: 0x0a_0e14,
        titlebar_background: 0x10_151d,
        tabbar_background: 0x10_151d,
        sidebar_background: 0x10_151d,
        sidebar_panel_background: 0x11_1821,
        right_panel_background: 0x11_1821,
        terminal_frame_background: 0x14_1b24,
        separator: 0x1b_2530,
        border: 0x1b_2530,
        hairline: 0x1b_2530,
        text_primary: 0xc5_c1b8,
        text_secondary: 0x9a_a4ae,
        text_muted: 0x7d_8790,
        text_inactive: 0x7d_8790,
        accent: 0xe6_b450,
        link_accent: 0xe6_b450,
        focus_ring: 0xe6_b450,
        tab_active: 0x14_1b24,
        tab_inactive: 0x10_151d,
        tab_hover: 0x11_1821,
        tab_active_indicator: 0xe6_b450,
        sidebar_item_hover: 0x11_1821,
        sidebar_item_selected: 0x14_1b24,
        sidebar_item_selected_border: 0xe6_b450,
    }
}

fn premium_shell_light() -> ShellChromeTheme {
    ShellChromeTheme {
        app_background: 0xf4_f6f8,
        titlebar_background: 0xee_f2f5,
        tabbar_background: 0xee_f2f5,
        sidebar_background: 0xee_f2f5,
        sidebar_panel_background: 0xf0_f3f6,
        right_panel_background: 0xf0_f3f6,
        terminal_frame_background: 0xfa_fafa,
        separator: 0xd8_dee6,
        border: 0xd8_dee6,
        hairline: 0xd8_dee6,
        text_primary: 0x5c_6166,
        text_secondary: 0x7a_838c,
        text_muted: 0x8a_939c,
        text_inactive: 0x8a_939c,
        accent: 0xff_aa33,
        link_accent: 0xff_aa33,
        focus_ring: 0xff_aa33,
        tab_active: 0xfa_fafa,
        tab_inactive: 0xee_f2f5,
        tab_hover: 0xf0_f3f6,
        tab_active_indicator: 0xff_aa33,
        sidebar_item_hover: 0xf0_f3f6,
        sidebar_item_selected: 0xfa_fafa,
        sidebar_item_selected_border: 0xff_aa33,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_line_references_are_not_underlined() {
        let dark = app_theme_spec(ThemeMode::Dark, ThemeVariant::PremiumDefault);
        let light = app_theme_spec(ThemeMode::Light, ThemeVariant::PremiumDefault);

        assert!(
            !dark
                .semantic_style(SemanticStyleRole::OutputLineReference)
                .underline
        );
        assert!(
            !light
                .semantic_style(SemanticStyleRole::OutputLineReference)
                .underline
        );
    }
}
