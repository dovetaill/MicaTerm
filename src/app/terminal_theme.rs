//! Terminal theme presets and palette conversion helpers.

use wezterm_term::color::{ColorPalette, RgbColor, SrgbaTuple};

use crate::theme::ThemeMode;

#[derive(Debug, Clone, Copy)]
pub struct TerminalThemePreset {
    pub name: &'static str,
    pub background: u32,
    pub foreground: u32,
    pub row_band_even: u32,
    pub row_band_odd: u32,
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
        background: 0x0c_10_14,
        foreground: 0xe6_ed_f5,
        row_band_even: 0x0c_10_14,
        row_band_odd: 0x13_18_1e,
        cursor_bg: 0xe6_ed_f5,
        cursor_fg: 0x0c_10_14,
        selection_bg: (0x5d, 0x73, 0x8b, 1.0),
        ansi: [
            (0x1f, 0x24, 0x2b),
            (0xe8, 0x7f, 0x86),
            (0xa8, 0xdc, 0x8a),
            (0xe7, 0xc9, 0x8b),
            (0x7c, 0xc5, 0xff),
            (0xd9, 0xa8, 0xff),
            (0x74, 0xcd, 0xd8),
            (0xc8, 0xd2, 0xdf),
            (0x68, 0x75, 0x86),
            (0xff, 0x9a, 0xa2),
            (0xc0, 0xea, 0x9d),
            (0xf0, 0xd3, 0x91),
            (0x99, 0xd5, 0xff),
            (0xe3, 0xbb, 0xff),
            (0x8d, 0xd9, 0xe2),
            (0xf6, 0xfb, 0xff),
        ],
        scrollbar_thumb: (0x32, 0x38, 0x41),
        split: (0x1d, 0x22, 0x29),
    }
}

pub fn mica_code_light() -> TerminalThemePreset {
    TerminalThemePreset {
        name: "Mica Code Light",
        background: 0xfc_fd_ff,
        foreground: 0x17_1c_23,
        row_band_even: 0xfc_fd_ff,
        row_band_odd: 0xf4_f8_ff,
        cursor_bg: 0x4c_55_61,
        cursor_fg: 0xfc_fd_ff,
        selection_bg: (0xc6, 0xd8, 0xf5, 1.0),
        ansi: [
            (0x24, 0x29, 0x2f),
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
        scrollbar_thumb: (0xc6, 0xd0, 0xdd),
        split: (0xe0, 0xe8, 0xf3),
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

pub fn selection_overlay_rgba(theme_mode: ThemeMode) -> u32 {
    let preset = preset_for_theme_mode(theme_mode);
    rgba_hex(preset.selection_bg)
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

fn rgba_hex((red, green, blue, alpha): (u8, u8, u8, f32)) -> u32 {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (alpha << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

fn rgba_tuple((red, green, blue, alpha): (u8, u8, u8, f32)) -> SrgbaTuple {
    SrgbaTuple(
        f32::from(red) / 255.0,
        f32::from(green) / 255.0,
        f32::from(blue) / 255.0,
        alpha,
    )
}
