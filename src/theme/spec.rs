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
pub const TERMINAL_BG_BASE_DARK: u32 = 0x0c_141c;
pub const TERMINAL_BG_GRADIENT_TOP_DARK: u32 = 0x10_1924;
pub const TERMINAL_BG_GRADIENT_BOTTOM_DARK: u32 = 0x0c_141c;
pub const TERMINAL_BG_BASE_LIGHT: u32 = 0xf8_fafc;
pub const TERMINAL_BG_GRADIENT_TOP_LIGHT: u32 = 0xfb_fcfd;
pub const TERMINAL_BG_GRADIENT_BOTTOM_LIGHT: u32 = 0xf6_f8fb;

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

impl AppThemeSpec {
    pub fn semantic_style(self, role: SemanticStyleRole) -> SemanticRoleStyle {
        let shell = self.shell;
        let terminal = self.terminal;
        let decoration = self.decoration;
        let semantic = self.semantic;
        match role {
            SemanticStyleRole::InputPrompt => SemanticRoleStyle {
                foreground: shell.text_secondary,
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
                underline: true,
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
                name: "Mica Graphite",
                background: TerminalBackgroundTheme {
                    base: TERMINAL_BG_BASE_DARK,
                    gradient_top: TERMINAL_BG_GRADIENT_TOP_DARK,
                    gradient_bottom: TERMINAL_BG_GRADIENT_BOTTOM_DARK,
                },
                foreground: TerminalForegroundTheme {
                    default: 0xe3_eaf2,
                    soft: 0xc8_d1dc,
                    dim: 0x94_a1b2,
                    inactive: 0xbc_c6d2,
                },
                cursor: TerminalCursorTheme {
                    background: 0xdc_e6f3,
                    foreground: TERMINAL_BG_BASE_DARK,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x6c_88ae,
                    alpha: 0.26,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0x7b_6840,
                    alpha: 0.18,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0x8e_79b8,
                    alpha: 0.27,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0x1b_232c,
                    thumb: 0x53_6274,
                    thumb_active: 0x66_788e,
                },
                ansi: [
                    0x4a_5260, 0xc3_7a86, 0x86_b48f, 0xc6_a56a, 0x7d_9bc2, 0xa7_8cbf, 0x78_afae,
                    0xc8_d1dc, 0x66_7180, 0xd6_949f, 0x9b_c6a4, 0xd8_ba83, 0x94_aed0, 0xb7_9ccb,
                    0x8e_c0c0, 0xe7_edf4,
                ],
            },
            decoration: DecorationTheme {
                success: 0x7f_b08d,
                warning: 0xc9_a86a,
                error: 0xc9_8a94,
                info: 0x7d_9bc2,
                running: 0x7b_96b8,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x7d_9bc2,
                output_accent: 0x89_a9cf,
            },
        },
        ThemeMode::Light => AppThemeSpec {
            id: "premium-default-light",
            name: "Premium Default",
            variant: ThemeVariant::PremiumDefault,
            mode,
            shell: premium_shell_light(),
            terminal: TerminalTheme {
                name: "Mica Canvas",
                background: TerminalBackgroundTheme {
                    base: TERMINAL_BG_BASE_LIGHT,
                    gradient_top: TERMINAL_BG_GRADIENT_TOP_LIGHT,
                    gradient_bottom: TERMINAL_BG_GRADIENT_BOTTOM_LIGHT,
                },
                foreground: TerminalForegroundTheme {
                    default: 0x26_3240,
                    soft: 0x4b_596a,
                    dim: 0x75_8395,
                    inactive: 0x5f_6e80,
                },
                cursor: TerminalCursorTheme {
                    background: 0x2c_3948,
                    foreground: TERMINAL_BG_BASE_LIGHT,
                },
                selection: TerminalOverlayTheme {
                    rgb: 0x7f_9bc2,
                    alpha: 0.20,
                },
                search_match: TerminalOverlayTheme {
                    rgb: 0xd8_c79a,
                    alpha: 0.32,
                },
                current_search_match: TerminalOverlayTheme {
                    rgb: 0xa9_8dda,
                    alpha: 0.32,
                },
                scrollbar: TerminalScrollbarTheme {
                    track: 0xe7_ebf0,
                    thumb: 0xb7_c3d0,
                    thumb_active: 0x9f_afbe,
                },
                ansi: [
                    0x5a_6573, 0xb8_6470, 0x5f_8d69, 0x9d_7c41, 0x5b_80ae, 0x8f_73aa, 0x4e_9090,
                    0xaa_b5c1, 0x73_8090, 0xc7_7a85, 0x76_9e7e, 0xb0_8d53, 0x72_95bf, 0xa2_86bb,
                    0x68_a4a3, 0xd5_dce4,
                ],
            },
            decoration: DecorationTheme {
                success: 0x5e_8a68,
                warning: 0x9b_7a3c,
                error: 0xa5_5c67,
                info: 0x5e_81ae,
                running: 0x6b_85a9,
            },
            semantic: SemanticHighlightTheme {
                input_command: 0x5b_80ae,
                output_accent: 0x50_77a7,
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
        app_background: 0x14_1b23,
        titlebar_background: 0x18_1f27,
        tabbar_background: 0x1a_222c,
        sidebar_background: 0x18_212b,
        sidebar_panel_background: 0x1b_2430,
        right_panel_background: 0x1c_2431,
        terminal_frame_background: 0x10_1824,
        separator: 0x26_303b,
        border: 0x31_3d4b,
        hairline: 0x3a_4857,
        text_primary: 0xe6_ecf3,
        text_secondary: 0xba_c4d0,
        text_muted: 0x8d_9aa9,
        text_inactive: 0x76_8296,
        accent: 0x6f_8fb7,
        link_accent: 0x89_a9cf,
        focus_ring: 0x7d_9bc2,
        tab_active: 0x22_3040,
        tab_inactive: 0x1a_222c,
        tab_hover: 0x20_2b38,
        tab_active_indicator: 0x7a_97bc,
        sidebar_item_hover: 0x22_303d,
        sidebar_item_selected: 0x29_3846,
        sidebar_item_selected_border: 0x6c_88ae,
    }
}

fn premium_shell_light() -> ShellChromeTheme {
    ShellChromeTheme {
        app_background: 0xf2_f5f8,
        titlebar_background: 0xf7_f9fc,
        tabbar_background: 0xee_f2f7,
        sidebar_background: 0xeb_f0f5,
        sidebar_panel_background: 0xf1_f5f9,
        right_panel_background: 0xf3_f6fa,
        terminal_frame_background: 0xed_f2f6,
        separator: 0xd8_e0e8,
        border: 0xc7_d2de,
        hairline: 0xb7_c4d1,
        text_primary: 0x24_303d,
        text_secondary: 0x49_586a,
        text_muted: 0x67_7789,
        text_inactive: 0x7b_8a9c,
        accent: 0x58_7daa,
        link_accent: 0x50_77a7,
        focus_ring: 0x7e_9ec5,
        tab_active: 0xff_ffff,
        tab_inactive: 0xee_f2f7,
        tab_hover: 0xe8_edf4,
        tab_active_indicator: 0x63_88b4,
        sidebar_item_hover: 0xe4_ebf3,
        sidebar_item_selected: 0xdc_e6f2,
        sidebar_item_selected_border: 0x8e_a3bb,
    }
}
