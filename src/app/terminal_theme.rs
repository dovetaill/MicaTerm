//! Terminal theme presets and palette conversion helpers.

use wezterm_term::color::{ColorPalette, RgbColor, SrgbaTuple};

use crate::theme::ThemeMode;

#[derive(Debug, Clone, Copy)]
pub struct TerminalThemePreset {
    pub name: &'static str,
    pub background: u32,
    pub foreground: u32,
    pub cursor_bg: u32,
    pub cursor_fg: u32,
    pub selection_bg: (u8, u8, u8, f32),
    pub ansi: [(u8, u8, u8); 16],
    pub scrollbar_thumb: (u8, u8, u8),
    pub split: (u8, u8, u8),
}

impl TerminalThemePreset {
    pub fn to_color_palette(self) -> ColorPalette {
        let mut palette = ColorPalette::default();

        for (index, color) in self.ansi.into_iter().enumerate() {
            palette.colors.0[index] = rgb_tuple(color);
        }

        palette.foreground = rgb_hex(self.foreground);
        palette.background = rgb_hex(self.background);
        palette.cursor_fg = rgb_hex(self.cursor_fg);
        palette.cursor_bg = rgb_hex(self.cursor_bg);
        palette.cursor_border = rgb_hex(self.cursor_bg);
        palette.selection_bg = rgba_tuple(self.selection_bg);
        palette.scrollbar_thumb = rgb_tuple(self.scrollbar_thumb);
        palette.split = rgb_tuple(self.split);

        palette
    }
}

pub fn mica_code_dark() -> TerminalThemePreset {
    TerminalThemePreset {
        name: "Mica Code Dark",
        background: 0x11_16_1d,
        foreground: 0xd7_de_e9,
        cursor_bg: 0xcd_d2_db,
        cursor_fg: 0x11_16_1d,
        selection_bg: (0x6b, 0x76, 0x92, 1.0),
        ansi: [
            (0x2a, 0x31, 0x3c),
            (0xe0, 0x6c, 0x75),
            (0x98, 0xc3, 0x79),
            (0xd7, 0xba, 0x7d),
            (0x61, 0xaf, 0xef),
            (0xc7, 0x92, 0xea),
            (0x56, 0xb6, 0xc2),
            (0xb8, 0xc2, 0xd1),
            (0x5d, 0x68, 0x77),
            (0xf0, 0x8b, 0x92),
            (0xb2, 0xd9, 0x8c),
            (0xe6, 0xcb, 0x8b),
            (0x7c, 0xc3, 0xff),
            (0xd8, 0xa6, 0xff),
            (0x74, 0xca, 0xd6),
            (0xee, 0xf2, 0xf7),
        ],
        scrollbar_thumb: (0x39, 0x41, 0x4d),
        split: (0x24, 0x2c, 0x37),
    }
}

pub fn mica_code_light() -> TerminalThemePreset {
    TerminalThemePreset {
        name: "Mica Code Light",
        background: 0xf7_f9_fc,
        foreground: 0x1f_23_28,
        cursor_bg: 0x4b_50_58,
        cursor_fg: 0xf7_f9_fc,
        selection_bg: (0xc7, 0xd1, 0xe3, 1.0),
        ansi: [
            (0x1f, 0x23, 0x28),
            (0xc7, 0x4e, 0x39),
            (0x2f, 0x85, 0x5a),
            (0xa1, 0x62, 0x07),
            (0x25, 0x63, 0xeb),
            (0x7c, 0x3a, 0xed),
            (0x0f, 0x76, 0x6e),
            (0xd8, 0xde, 0xe8),
            (0x6b, 0x72, 0x80),
            (0xdd, 0x6b, 0x55),
            (0x3c, 0x9c, 0x6a),
            (0xb7, 0x79, 0x1f),
            (0x3b, 0x82, 0xf6),
            (0x8b, 0x5c, 0xf6),
            (0x0f, 0x8b, 0x83),
            (0xff, 0xff, 0xff),
        ],
        scrollbar_thumb: (0xc8, 0xd0, 0xdc),
        split: (0xdc, 0xe3, 0xed),
    }
}

pub fn preset_for_theme_mode(theme_mode: ThemeMode) -> TerminalThemePreset {
    match theme_mode {
        ThemeMode::Dark => mica_code_dark(),
        ThemeMode::Light => mica_code_light(),
    }
}

pub fn palette_for_theme_mode(theme_mode: ThemeMode) -> ColorPalette {
    preset_for_theme_mode(theme_mode).to_color_palette()
}

fn rgb_hex(rgb: u32) -> SrgbaTuple {
    let red = ((rgb >> 16) & 0xff) as u8;
    let green = ((rgb >> 8) & 0xff) as u8;
    let blue = (rgb & 0xff) as u8;

    RgbColor::new_8bpc(red, green, blue).into()
}

fn rgb_tuple((red, green, blue): (u8, u8, u8)) -> SrgbaTuple {
    RgbColor::new_8bpc(red, green, blue).into()
}

fn rgba_tuple((red, green, blue, alpha): (u8, u8, u8, f32)) -> SrgbaTuple {
    SrgbaTuple(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    )
}
