//! Terminal color palettes (Dark & Light), ANSI/256/TrueColor -> GPUI Hsla resolution,
//! and semantic developer overlay colors (OSC 133 command blocks, output rules, input tokens).

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::{Hsla, Rgba};

use crate::syntax::CodeTokenRole;
use crate::terminal::semantic::{CommandBlockStatus, OverviewMarkerKind, SemanticRole};

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
}

impl TerminalPalette {
    pub fn dark() -> Self {
        Self {
            dark: true,
            foreground: Rgb {
                r: 0xe5,
                g: 0xe5,
                b: 0xea,
            },
            background: Rgb {
                r: 0x19,
                g: 0x19,
                b: 0x1c,
            },
            cursor: Rgb {
                r: 0x0a,
                g: 0x84,
                b: 0xff,
            },
            cursor_text: Rgb {
                r: 0xff,
                g: 0xff,
                b: 0xff,
            },
            selection_bg: rgba_u8(0x0a, 0x84, 0xff, 0.30),
            search_match_bg: rgba_u8(0xff, 0x9f, 0x0a, 0.28),
            search_active_bg: rgba_u8(0xff, 0x9f, 0x0a, 0.68),
            block_running: rgba_u8(0x0a, 0x84, 0xff, 0.90),
            block_success: rgba_u8(0xff, 0xff, 0xff, 0.24),
            block_failure: rgba_u8(0xff, 0x45, 0x3a, 0.92),
            block_divider: rgba_u8(0xff, 0xff, 0xff, 0.065),
            search_marker: rgba_u8(0xff, 0x9f, 0x0a, 0.92),
            ansi: [
                Rgb { r: 0x2c, g: 0x2c, b: 0x2e }, // 0 Black
                Rgb { r: 0xff, g: 0x45, b: 0x3a }, // 1 Red (Apple Dark Red)
                Rgb { r: 0x30, g: 0xd1, b: 0x58 }, // 2 Green (Apple Dark Green)
                Rgb { r: 0xff, g: 0xd6, b: 0x0a }, // 3 Yellow
                Rgb { r: 0x0a, g: 0x84, b: 0xff }, // 4 Blue (Apple Dark Blue)
                Rgb { r: 0xbf, g: 0x5a, b: 0xf2 }, // 5 Magenta (Apple Dark Purple)
                Rgb { r: 0x64, g: 0xd2, b: 0xff }, // 6 Cyan (Apple Dark Cyan)
                Rgb { r: 0xd1, g: 0xd1, b: 0xd6 }, // 7 White
                Rgb { r: 0x63, g: 0x63, b: 0x66 }, // 8 Bright Black
                Rgb { r: 0xff, g: 0x69, b: 0x61 }, // 9 Bright Red
                Rgb { r: 0x4c, g: 0xd9, b: 0x64 }, // 10 Bright Green
                Rgb { r: 0xff, g: 0xe0, b: 0x66 }, // 11 Bright Yellow
                Rgb { r: 0x5e, g: 0xb0, b: 0xff }, // 12 Bright Blue
                Rgb { r: 0xd9, g: 0x82, b: 0xff }, // 13 Bright Magenta
                Rgb { r: 0x85, g: 0xe0, b: 0xff }, // 14 Bright Cyan
                Rgb { r: 0xf2, g: 0xf2, b: 0xf7 }, // 15 Bright White
            ],
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            foreground: Rgb {
                r: 0x1d,
                g: 0x1d,
                b: 0x1f,
            },
            background: Rgb {
                r: 0xff,
                g: 0xff,
                b: 0xff,
            },
            cursor: Rgb {
                r: 0x00,
                g: 0x7a,
                b: 0xff,
            },
            cursor_text: Rgb {
                r: 0xff,
                g: 0xff,
                b: 0xff,
            },
            selection_bg: rgba_u8(0x00, 0x7a, 0xff, 0.20),
            search_match_bg: rgba_u8(0xff, 0x95, 0x00, 0.24),
            search_active_bg: rgba_u8(0xff, 0x95, 0x00, 0.58),
            block_running: rgba_u8(0x00, 0x7a, 0xff, 0.86),
            block_success: rgba_u8(0x00, 0x00, 0x00, 0.20),
            block_failure: rgba_u8(0xff, 0x3b, 0x30, 0.90),
            block_divider: rgba_u8(0x00, 0x00, 0x00, 0.065),
            search_marker: rgba_u8(0xff, 0x95, 0x00, 0.90),
            ansi: [
                Rgb { r: 0x1d, g: 0x1d, b: 0x1f }, // 0 Black
                Rgb { r: 0xd7, g: 0x00, b: 0x15 }, // 1 Red
                Rgb { r: 0x1d, g: 0x83, b: 0x30 }, // 2 Green
                Rgb { r: 0x9e, g: 0x63, b: 0x00 }, // 3 Yellow
                Rgb { r: 0x00, g: 0x66, b: 0xd6 }, // 4 Blue
                Rgb { r: 0x89, g: 0x44, b: 0xab }, // 5 Magenta
                Rgb { r: 0x00, g: 0x71, b: 0x85 }, // 6 Cyan
                Rgb { r: 0x6e, g: 0x6e, b: 0x73 }, // 7 White
                Rgb { r: 0x8e, g: 0x8e, b: 0x93 }, // 8 Bright Black
                Rgb { r: 0xff, g: 0x3b, b: 0x30 }, // 9 Bright Red
                Rgb { r: 0x28, g: 0xcd, b: 0x41 }, // 10 Bright Green
                Rgb { r: 0xc9, g: 0x82, b: 0x00 }, // 11 Bright Yellow
                Rgb { r: 0x00, g: 0x7a, b: 0xff }, // 12 Bright Blue
                Rgb { r: 0xaf, g: 0x52, b: 0xde }, // 13 Bright Magenta
                Rgb { r: 0x32, g: 0xad, b: 0xe6 }, // 14 Bright Cyan
                Rgb { r: 0x1d, g: 0x1d, b: 0x1f }, // 15 Bright White
            ],
        }
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
        if self.dark {
            match role {
                SemanticRole::OutputUrl => (rgba_u8(0x6c, 0xb6, 0xff, 1.0), false, false),
                SemanticRole::OutputFileLine => (rgba_u8(0x56, 0xc2, 0xc6, 1.0), false, false),
                SemanticRole::OutputPath => (rgba_u8(0x7d, 0xcf, 0xff, 1.0), false, false),
                SemanticRole::OutputEndpoint => (rgba_u8(0xc6, 0x78, 0xdd, 1.0), false, false),
                SemanticRole::DiagnosticError => (rgba_u8(0xf0, 0x80, 0x89, 1.0), true, false),
                SemanticRole::DiagnosticWarn => (rgba_u8(0xe5, 0xc0, 0x7b, 1.0), true, false),
                SemanticRole::InputCommand => (rgba_u8(0x79, 0xc0, 0xff, 1.0), true, false),
                SemanticRole::InputSubcommand => (rgba_u8(0x70, 0xc8, 0xd4, 1.0), false, false),
                SemanticRole::InputOption => (rgba_u8(0xf0, 0xd1, 0x92, 1.0), false, false),
                SemanticRole::InputString => (rgba_u8(0xa9, 0xd4, 0x8b, 1.0), false, false),
                SemanticRole::InputOperator => (rgba_u8(0xff, 0x9e, 0x64, 1.0), true, false),
                SemanticRole::InputVariable => (rgba_u8(0xd8, 0x92, 0xef, 1.0), false, false),
            }
        } else {
            match role {
                SemanticRole::OutputUrl => (rgba_u8(0x09, 0x69, 0xda, 1.0), false, false),
                SemanticRole::OutputFileLine => (rgba_u8(0x1b, 0x7c, 0x83, 1.0), false, false),
                SemanticRole::OutputPath => (rgba_u8(0x05, 0x50, 0xae, 1.0), false, false),
                SemanticRole::OutputEndpoint => (rgba_u8(0x82, 0x50, 0xdf, 1.0), false, false),
                SemanticRole::DiagnosticError => (rgba_u8(0xcf, 0x22, 0x2e, 1.0), true, false),
                SemanticRole::DiagnosticWarn => (rgba_u8(0x9a, 0x67, 0x00, 1.0), true, false),
                SemanticRole::InputCommand => (rgba_u8(0x09, 0x69, 0xda, 1.0), true, false),
                SemanticRole::InputSubcommand => (rgba_u8(0x1b, 0x7c, 0x83, 1.0), false, false),
                SemanticRole::InputOption => (rgba_u8(0x9a, 0x67, 0x00, 1.0), false, false),
                SemanticRole::InputString => (rgba_u8(0x1a, 0x7f, 0x37, 1.0), false, false),
                SemanticRole::InputOperator => (rgba_u8(0xbc, 0x4c, 0x00, 1.0), true, false),
                SemanticRole::InputVariable => (rgba_u8(0x82, 0x50, 0xdf, 1.0), false, false),
            }
        }
    }

    /// Returns `(fg_color, add_bold, add_underline)` for a syntax-highlighted code token in terminal output.
    pub fn code_token_style(&self, role: CodeTokenRole) -> (Hsla, bool, bool) {
        if self.dark {
            match role {
                CodeTokenRole::Comment => (rgba_u8(0x7f, 0x84, 0x8e, 1.0), false, false),
                CodeTokenRole::CommentDoc => (rgba_u8(0x8b, 0x94, 0x9e, 1.0), false, false),
                CodeTokenRole::String => (rgba_u8(0x98, 0xc3, 0x79, 1.0), false, false),
                CodeTokenRole::StringEscape => (rgba_u8(0x56, 0xb6, 0xc2, 1.0), false, false),
                CodeTokenRole::Number | CodeTokenRole::Boolean => {
                    (rgba_u8(0xd1, 0x9a, 0x66, 1.0), false, false)
                }
                CodeTokenRole::Constant => (rgba_u8(0xe5, 0xc0, 0x7b, 1.0), false, false),
                CodeTokenRole::Keyword => (rgba_u8(0xc6, 0x78, 0xdd, 1.0), false, false),
                CodeTokenRole::Operator => (rgba_u8(0x56, 0xb6, 0xc2, 1.0), false, false),
                CodeTokenRole::Function => (rgba_u8(0x61, 0xaf, 0xef, 1.0), false, false),
                CodeTokenRole::Type => (rgba_u8(0xe5, 0xc0, 0x7b, 1.0), false, false),
                CodeTokenRole::Tag => (rgba_u8(0xe0, 0x6c, 0x75, 1.0), false, false),
                CodeTokenRole::Attribute | CodeTokenRole::Property => {
                    (rgba_u8(0x7d, 0xcf, 0xff, 1.0), false, false)
                }
                CodeTokenRole::Variable => (rgba_u8(0xd8, 0x92, 0xef, 1.0), false, false),
                CodeTokenRole::Heading => (rgba_u8(0x61, 0xaf, 0xef, 1.0), true, false),
                CodeTokenRole::Inserted => (rgba_u8(0x30, 0xd1, 0x58, 1.0), false, false),
                CodeTokenRole::Deleted => (rgba_u8(0xff, 0x45, 0x3a, 1.0), false, false),
            }
        } else {
            match role {
                CodeTokenRole::Comment => (rgba_u8(0x6e, 0x77, 0x81, 1.0), false, false),
                CodeTokenRole::CommentDoc => (rgba_u8(0x57, 0x60, 0x6a, 1.0), false, false),
                CodeTokenRole::String => (rgba_u8(0x1a, 0x7f, 0x37, 1.0), false, false),
                CodeTokenRole::StringEscape => (rgba_u8(0x05, 0x50, 0xae, 1.0), false, false),
                CodeTokenRole::Number | CodeTokenRole::Boolean => {
                    (rgba_u8(0x95, 0x38, 0x00, 1.0), false, false)
                }
                CodeTokenRole::Constant => (rgba_u8(0x05, 0x50, 0xae, 1.0), false, false),
                CodeTokenRole::Keyword => (rgba_u8(0x82, 0x50, 0xdf, 1.0), false, false),
                CodeTokenRole::Operator => (rgba_u8(0x05, 0x50, 0xae, 1.0), false, false),
                CodeTokenRole::Function => (rgba_u8(0x09, 0x69, 0xda, 1.0), false, false),
                CodeTokenRole::Type => (rgba_u8(0x95, 0x38, 0x00, 1.0), false, false),
                CodeTokenRole::Tag => (rgba_u8(0x11, 0x63, 0x29, 1.0), false, false),
                CodeTokenRole::Attribute | CodeTokenRole::Property => {
                    (rgba_u8(0x05, 0x50, 0xae, 1.0), false, false)
                }
                CodeTokenRole::Variable => (rgba_u8(0x82, 0x50, 0xdf, 1.0), false, false),
                CodeTokenRole::Heading => (rgba_u8(0x09, 0x69, 0xda, 1.0), true, false),
                CodeTokenRole::Inserted => (rgba_u8(0x1a, 0x7f, 0x37, 1.0), false, false),
                CodeTokenRole::Deleted => (rgba_u8(0xcf, 0x22, 0x2e, 1.0), false, false),
            }
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
