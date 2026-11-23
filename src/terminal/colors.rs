//! Terminal color palettes (Dark & Light) and ANSI/256/TrueColor -> GPUI Hsla resolution.

use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::color::Colors;
use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
use gpui_kit::{Hsla, Rgba};

#[derive(Debug, Clone)]
pub struct TerminalPalette {
    pub foreground: Rgb,
    pub background: Rgb,
    pub cursor: Rgb,
    pub cursor_text: Rgb,
    pub selection_bg: Hsla,
    pub search_match_bg: Hsla,
    pub search_active_bg: Hsla,
    pub ansi: [Rgb; 16],
}

impl TerminalPalette {
    pub fn dark() -> Self {
        Self {
            foreground: Rgb { r: 0xd8, g: 0xde, b: 0xe9 },
            background: Rgb { r: 0x11, g: 0x14, b: 0x1a },
            cursor: Rgb { r: 0x61, g: 0xaf, b: 0xef },
            cursor_text: Rgb { r: 0x11, g: 0x14, b: 0x1a },
            selection_bg: rgba_u8(0x3b, 0x82, 0xf6, 0.30),
            search_match_bg: rgba_u8(0xf5, 0x9e, 0x0b, 0.28),
            search_active_bg: rgba_u8(0xf5, 0x9e, 0x0b, 0.62),
            ansi: [
                Rgb { r: 0x1e, g: 0x22, b: 0x2a }, // 0 Black
                Rgb { r: 0xe0, g: 0x6c, b: 0x75 }, // 1 Red
                Rgb { r: 0x98, g: 0xc3, b: 0x79 }, // 2 Green
                Rgb { r: 0xe5, g: 0xc0, b: 0x7b }, // 3 Yellow
                Rgb { r: 0x61, g: 0xaf, b: 0xef }, // 4 Blue
                Rgb { r: 0xc6, g: 0x78, b: 0xdd }, // 5 Magenta
                Rgb { r: 0x56, g: 0xb6, b: 0xc2 }, // 6 Cyan
                Rgb { r: 0xab, g: 0xb2, b: 0xbf }, // 7 White
                Rgb { r: 0x5c, g: 0x63, b: 0x70 }, // 8 Bright Black
                Rgb { r: 0xf0, g: 0x80, b: 0x89 }, // 9 Bright Red
                Rgb { r: 0xa9, g: 0xd4, b: 0x8b }, // 10 Bright Green
                Rgb { r: 0xf0, g: 0xd1, b: 0x92 }, // 11 Bright Yellow
                Rgb { r: 0x79, g: 0xc0, b: 0xff }, // 12 Bright Blue
                Rgb { r: 0xd8, g: 0x92, b: 0xef }, // 13 Bright Magenta
                Rgb { r: 0x70, g: 0xc8, b: 0xd4 }, // 14 Bright Cyan
                Rgb { r: 0xe6, g: 0xed, b: 0xf3 }, // 15 Bright White
            ],
        }
    }

    pub fn light() -> Self {
        Self {
            foreground: Rgb { r: 0x24, g: 0x29, b: 0x2f },
            background: Rgb { r: 0xf9, g: 0xfa, b: 0xfb },
            cursor: Rgb { r: 0x25, g: 0x63, b: 0xeb },
            cursor_text: Rgb { r: 0xff, g: 0xff, b: 0xff },
            selection_bg: rgba_u8(0x25, 0x63, 0xeb, 0.22),
            search_match_bg: rgba_u8(0xd9, 0x77, 0x06, 0.24),
            search_active_bg: rgba_u8(0xd9, 0x77, 0x06, 0.52),
            ansi: [
                Rgb { r: 0x24, g: 0x29, b: 0x2f }, // 0 Black
                Rgb { r: 0xcf, g: 0x22, b: 0x2e }, // 1 Red
                Rgb { r: 0x1a, g: 0x7f, b: 0x37 }, // 2 Green
                Rgb { r: 0x9a, g: 0x67, b: 0x00 }, // 3 Yellow
                Rgb { r: 0x09, g: 0x69, b: 0xda }, // 4 Blue
                Rgb { r: 0x82, g: 0x50, b: 0xdf }, // 5 Magenta
                Rgb { r: 0x1b, g: 0x7c, b: 0x83 }, // 6 Cyan
                Rgb { r: 0x6e, g: 0x77, b: 0x81 }, // 7 White
                Rgb { r: 0x57, g: 0x60, b: 0x6a }, // 8 Bright Black
                Rgb { r: 0xa4, g: 0x0e, b: 0x26 }, // 9 Bright Red
                Rgb { r: 0x2d, g: 0xa4, b: 0x4e }, // 10 Bright Green
                Rgb { r: 0xbf, g: 0x87, b: 0x00 }, // 11 Bright Yellow
                Rgb { r: 0x21, g: 0x8b, b: 0xff }, // 12 Bright Blue
                Rgb { r: 0xa4, g: 0x75, b: 0xf9 }, // 13 Bright Magenta
                Rgb { r: 0x31, g: 0x92, b: 0xaa }, // 14 Bright Cyan
                Rgb { r: 0x1f, g: 0x23, b: 0x28 }, // 15 Bright White
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
