//! Terminal color palettes (Dark, Light & Full Theme Catalog), ANSI/256/TrueColor -> GPUI Hsla
//! resolution, and theme-adaptive semantic developer overlay colors (OSC 133 command blocks,
//! output rules, input tokens, and terminal code highlighting).

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::{Hsla, Rgba};

use crate::domain::{DEFAULT_DARK_THEME_ID, DEFAULT_LIGHT_THEME_ID};
use crate::syntax::CodeTokenRole;
use crate::terminal::semantic::{CommandBlockStatus, OverviewMarkerKind, SemanticRole};
use crate::ui::theme_catalog::{
    ThemeRegistry, ThemeSpec, contrasting_ink, ensure_readable_fg, mix_hex, u32_to_rgb,
};

#[derive(Debug, Clone)]
pub struct TerminalPalette {
    pub dark: bool,
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub cursor_text: Rgb,
    pub selection_bg: Hsla,
    pub search_match_bg: Hsla,
    pub search_active_bg: Hsla,
    pub block_running: Hsla,
    pub block_success: Hsla,
    pub block_failure: Hsla,
    pub block_divider: Hsla,
    pub search_marker: Hsla,
    pub ansi: [Rgb; 16],
    syn_comment: Hsla,
    syn_comment_doc: Hsla,
    syn_string: Hsla,
    syn_cyan: Hsla,
    syn_number: Hsla,
    syn_yellow: Hsla,
    syn_keyword: Hsla,
    syn_blue: Hsla,
    syn_input_cmd: Hsla,
    syn_red: Hsla,
    syn_green: Hsla,
    syn_orange: Hsla,
    syn_variable: Hsla,
}

impl TerminalPalette {
    pub fn from_theme(spec: &ThemeSpec) -> Self {
        let dark = spec.dark;
        let bg = spec.term_bg;
        let min_c = if dark { 3.6 } else { 3.4 };

        let rd = |hex: u32| ensure_readable_fg(hex, bg, dark, min_c);
        let to_hsla = |hex: u32, alpha: f32| {
            let rgb = u32_to_rgb(hex);
            rgba_u8(rgb.r, rgb.g, rgb.b, alpha)
        };

        let comment_hex = ensure_readable_fg(
            mix_hex(spec.muted_fg, spec.ansi[8], 0.35),
            bg,
            dark,
            if dark { 2.7 } else { 2.6 },
        );
        let comment_doc_hex = ensure_readable_fg(mix_hex(comment_hex, spec.term_fg, 0.25), bg, dark, 3.0);

        let red_hex = rd(if dark { spec.ansi[9] } else { spec.ansi[1] });
        let green_hex = rd(spec.ansi[2]);
        let yellow_hex = rd(if dark { spec.ansi[11] } else { spec.ansi[3] });
        let blue_hex = rd(spec.ansi[4]);
        let magenta_hex = rd(spec.ansi[5]);
        let cyan_hex = rd(spec.ansi[6]);
        let orange_hex = rd(mix_hex(yellow_hex, red_hex, 0.42));
        let var_hex = rd(mix_hex(magenta_hex, spec.term_fg, 0.28));

        // Guarantee distinctness from raw ANSI[1] so semantic/syntax tokens never accidentally collide
        let fn_blue_hex = if blue_hex == spec.ansi[1] {
            rd(spec.accent)
        } else {
            blue_hex
        };
        let mut input_cmd_hex = rd(if dark {
            mix_hex(spec.ansi[12], 0xFFFFFF, 0.16)
        } else {
            mix_hex(spec.ansi[12], 0x000000, 0.16)
        });
        if input_cmd_hex == fn_blue_hex {
            input_cmd_hex = if dark {
                mix_hex(fn_blue_hex, 0xFFFFFF, 0.20)
            } else {
                mix_hex(fn_blue_hex, 0x000000, 0.20)
            };
        }

        let warn_marker_hex = spec.ansi[3];
        let fail_hex = spec.ansi[1];
        let run_hex = spec.accent;

        Self {
            dark,
            foreground: u32_to_rgb(spec.term_fg),
            background: u32_to_rgb(spec.term_bg),
            cursor: u32_to_rgb(spec.cursor),
            cursor_text: u32_to_rgb(
                spec.cursor_text
                    .unwrap_or_else(|| contrasting_ink(spec.cursor)),
            ),
            selection_bg: to_hsla(spec.selection, if dark { 0.30 } else { 0.20 }),
            search_match_bg: to_hsla(warn_marker_hex, if dark { 0.28 } else { 0.24 }),
            search_active_bg: to_hsla(warn_marker_hex, if dark { 0.66 } else { 0.56 }),
            block_running: to_hsla(run_hex, 0.88),
            block_success: if dark {
                rgba_u8(0xff, 0xff, 0xff, 0.22)
            } else {
                rgba_u8(0x00, 0x00, 0x00, 0.18)
            },
            block_failure: to_hsla(fail_hex, 0.90),
            block_divider: if dark {
                rgba_u8(0xff, 0xff, 0xff, 0.065)
            } else {
                rgba_u8(0x00, 0x00, 0x00, 0.065)
            },
            search_marker: to_hsla(warn_marker_hex, 0.90),
            ansi: spec.ansi.map(u32_to_rgb),
            syn_comment: to_hsla(comment_hex, 1.0),
            syn_comment_doc: to_hsla(comment_doc_hex, 1.0),
            syn_string: to_hsla(green_hex, 1.0),
            syn_cyan: to_hsla(cyan_hex, 1.0),
            syn_number: to_hsla(orange_hex, 1.0),
            syn_yellow: to_hsla(yellow_hex, 1.0),
            syn_keyword: to_hsla(magenta_hex, 1.0),
            syn_blue: to_hsla(fn_blue_hex, 1.0),
            syn_input_cmd: to_hsla(input_cmd_hex, 1.0),
            syn_red: to_hsla(red_hex, 1.0),
            syn_green: to_hsla(rd(spec.ansi[10]), 1.0),
            syn_orange: to_hsla(orange_hex, 1.0),
            syn_variable: to_hsla(var_hex, 1.0),
        }
    }

    pub fn for_theme_id(theme_id: &str, dark_fallback: bool) -> Self {
        let spec = ThemeRegistry::global().resolve(theme_id, dark_fallback);
        Self::from_theme(&spec)
    }

    pub fn dark() -> Self {
        Self::for_theme_id(DEFAULT_DARK_THEME_ID, true)
    }

    pub fn light() -> Self {
        Self::for_theme_id(DEFAULT_LIGHT_THEME_ID, false)
    }

    pub fn for_mode(dark_mode: bool) -> Self {
        if dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    pub fn command_block_color(&self, status: CommandBlockStatus) -> Hsla {
        match status {
            CommandBlockStatus::Running => self.block_running,
            CommandBlockStatus::Success => self.block_success,
            CommandBlockStatus::Failure(_) => self.block_failure,
        }
    }

    pub fn overview_marker_color(&self, kind: OverviewMarkerKind) -> Hsla {
        match kind {
            OverviewMarkerKind::CommandRunning => self.block_running,
            OverviewMarkerKind::CommandSuccess => self.block_success.opacity(0.55),
            OverviewMarkerKind::CommandFailure => self.block_failure,
            OverviewMarkerKind::SearchMatch => self.search_marker,
        }
    }

    /// Returns `(fg_color, add_bold, add_underline)` for a semantic token role.
    pub fn semantic_style(&self, role: SemanticRole) -> (Hsla, bool, bool) {
        match role {
            SemanticRole::OutputUrl => (self.syn_blue, false, false),
            SemanticRole::OutputFileLine => (self.syn_cyan, false, false),
            SemanticRole::OutputPath => (self.syn_cyan, false, false),
            SemanticRole::OutputEndpoint => (self.syn_keyword, false, false),
            SemanticRole::DiagnosticError => (self.syn_red, true, false),
            SemanticRole::DiagnosticWarn => (self.syn_yellow, true, false),
            SemanticRole::InputCommand => (self.syn_input_cmd, true, false),
            SemanticRole::InputSubcommand => (self.syn_cyan, false, false),
            SemanticRole::InputOption => (self.syn_yellow, false, false),
            SemanticRole::InputString => (self.syn_string, false, false),
            SemanticRole::InputOperator => (self.syn_orange, true, false),
            SemanticRole::InputVariable => (self.syn_variable, false, false),
        }
    }

    /// Returns `(fg_color, add_bold, add_underline)` for a syntax-highlighted code token in terminal output.
    pub fn code_token_style(&self, role: CodeTokenRole) -> (Hsla, bool, bool) {
        match role {
            CodeTokenRole::Comment => (self.syn_comment, false, false),
            CodeTokenRole::CommentDoc => (self.syn_comment_doc, false, false),
            CodeTokenRole::String => (self.syn_string, false, false),
            CodeTokenRole::StringEscape => (self.syn_cyan, false, false),
            CodeTokenRole::Number | CodeTokenRole::Boolean => (self.syn_number, false, false),
            CodeTokenRole::Constant => (self.syn_yellow, false, false),
            CodeTokenRole::Keyword => (self.syn_keyword, false, false),
            CodeTokenRole::Operator => (self.syn_cyan, false, false),
            CodeTokenRole::Function => (self.syn_blue, false, false),
            CodeTokenRole::Type => (self.syn_yellow, false, false),
            CodeTokenRole::Tag => (self.syn_red, false, false),
            CodeTokenRole::Attribute | CodeTokenRole::Property => (self.syn_cyan, false, false),
            CodeTokenRole::Variable => (self.syn_variable, false, false),
            CodeTokenRole::Heading => (self.syn_blue, true, false),
            CodeTokenRole::Inserted => (self.syn_green, false, false),
            CodeTokenRole::Deleted => (self.syn_red, false, false),
        }
    }

    pub fn resolve_cell_colors(
        &self,
        fg: Color,
        bg: Color,
        flags: Flags,
        term_colors: &Colors,
    ) -> (Hsla, Option<Hsla>) {
        let mut fg_rgb = self.resolve_color(fg, flags.contains(Flags::BOLD), term_colors);
        let bg_rgb = self.resolve_color(bg, false, term_colors);

        let (final_fg, final_bg, bg_is_default) = if flags.contains(Flags::INVERSE) {
            (bg_rgb, fg_rgb, false)
        } else {
            let is_default_bg = matches!(bg, Color::Named(NamedColor::Background))
                && term_colors[NamedColor::Background].is_none();
            (fg_rgb, bg_rgb, is_default_bg)
        };

        fg_rgb = final_fg;
        if flags.contains(Flags::DIM) {
            fg_rgb = dim_rgb(fg_rgb);
        }

        let fg_hsla = if flags.contains(Flags::HIDDEN) {
            rgb_to_hsla(final_bg, 0.0)
        } else {
            rgb_to_hsla(fg_rgb, 1.0)
        };
        let bg_hsla = (!bg_is_default).then(|| rgb_to_hsla(final_bg, 1.0));
        (fg_hsla, bg_hsla)
    }

    pub fn resolve_color(&self, color: Color, bold_bright: bool, term_colors: &Colors) -> Rgb {
        match color {
            Color::Spec(rgb) => rgb,
            Color::Indexed(idx) => {
                if let Some(custom) = term_colors[idx as usize] {
                    return custom;
                }
                self.resolve_indexed(idx)
            }
            Color::Named(named) => {
                let effective = if bold_bright {
                    named.to_bright()
                } else {
                    named
                };
                if let Some(custom) = term_colors[effective] {
                    return custom;
                }
                self.resolve_named(effective)
            }
        }
    }

    fn resolve_named(&self, named: NamedColor) -> Rgb {
        match named {
            NamedColor::Black => self.ansi[0],
            NamedColor::Red => self.ansi[1],
            NamedColor::Green => self.ansi[2],
            NamedColor::Yellow => self.ansi[3],
            NamedColor::Blue => self.ansi[4],
            NamedColor::Magenta => self.ansi[5],
            NamedColor::Cyan => self.ansi[6],
            NamedColor::White => self.ansi[7],
            NamedColor::BrightBlack => self.ansi[8],
            NamedColor::BrightRed => self.ansi[9],
            NamedColor::BrightGreen => self.ansi[10],
            NamedColor::BrightYellow => self.ansi[11],
            NamedColor::BrightBlue => self.ansi[12],
            NamedColor::BrightMagenta => self.ansi[13],
            NamedColor::BrightCyan => self.ansi[14],
            NamedColor::BrightWhite => self.ansi[15],
            NamedColor::Foreground | NamedColor::BrightForeground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            NamedColor::DimBlack => dim_rgb(self.ansi[0]),
            NamedColor::DimRed => dim_rgb(self.ansi[1]),
            NamedColor::DimGreen => dim_rgb(self.ansi[2]),
            NamedColor::DimYellow => dim_rgb(self.ansi[3]),
            NamedColor::DimBlue => dim_rgb(self.ansi[4]),
            NamedColor::DimMagenta => dim_rgb(self.ansi[5]),
            NamedColor::DimCyan => dim_rgb(self.ansi[6]),
            NamedColor::DimWhite => dim_rgb(self.ansi[7]),
            NamedColor::DimForeground => dim_rgb(self.foreground),
        }
    }

    fn resolve_indexed(&self, idx: u8) -> Rgb {
        match idx {
            0..=15 => self.ansi[idx as usize],
            16..=231 => {
                let c = idx - 16;
                let r_idx = c / 36;
                let g_idx = (c % 36) / 6;
                let b_idx = c % 6;
                let map = |v: u8| if v == 0 { 0 } else { 55 + v * 40 };
                Rgb {
                    r: map(r_idx),
                    g: map(g_idx),
                    b: map(b_idx),
                }
            }
            232..=255 => {
                let gray = 8 + (idx - 232) * 10;
                Rgb {
                    r: gray,
                    g: gray,
                    b: gray,
                }
            }
        }
    }
}

pub fn rgb_to_hsla(rgb: Rgb, alpha: f32) -> Hsla {
    rgba_u8(rgb.r, rgb.g, rgb.b, alpha)
}

pub fn rgba_u8(r: u8, g: u8, b: u8, a: f32) -> Hsla {
    Hsla::from(Rgba {
        r: f32::from(r) / 255.0,
        g: f32::from(g) / 255.0,
        b: f32::from(b) / 255.0,
        a,
    })
}

fn dim_rgb(rgb: Rgb) -> Rgb {
    Rgb {
        r: ((u16::from(rgb.r) * 2) / 3) as u8,
        g: ((u16::from(rgb.g) * 2) / 3) as u8,
        b: ((u16::from(rgb.b) * 2) / 3) as u8,
    }
}
