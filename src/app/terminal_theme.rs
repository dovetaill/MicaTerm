//! Terminal theme presets and palette conversion helpers.

use wezterm_term::color::{ColorPalette, RgbColor, SrgbaTuple};

use crate::theme::{ThemeMode, terminal_palette_spec};

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
    pub scrollbar_thumb_active: (u8, u8, u8),
    pub jump_to_latest_bg: u32,
    pub jump_to_latest_hover_bg: u32,
    pub jump_to_latest_pressed_bg: u32,
    pub jump_to_latest_border: u32,
    pub jump_to_latest_fg: u32,
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

fn preset_from_theme_mode(theme_mode: ThemeMode) -> TerminalThemePreset {
    let spec = terminal_palette_spec(theme_mode);
    TerminalThemePreset {
        name: spec.name,
        background: spec.default_bg,
        foreground: spec.default_fg,
        row_band_even: spec.row_bg_even,
        row_band_odd: spec.row_bg_odd,
        cursor_bg: spec.cursor_bg,
        cursor_fg: spec.cursor_fg,
        selection_bg: rgba_components(spec.selection_rgb, spec.selection_alpha),
        ansi: spec.ansi.map(rgb_components),
        scrollbar_thumb: rgb_components(spec.scrollbar_thumb),
        scrollbar_thumb_active: rgb_components(spec.scrollbar_thumb_active),
        jump_to_latest_bg: spec.jump_to_latest_bg,
        jump_to_latest_hover_bg: spec.jump_to_latest_hover_bg,
        jump_to_latest_pressed_bg: spec.jump_to_latest_pressed_bg,
        jump_to_latest_border: spec.jump_to_latest_border,
        jump_to_latest_fg: spec.jump_to_latest_fg,
        split: rgb_components(spec.split),
    }
}

pub fn preset_for_theme_mode(theme_mode: ThemeMode) -> TerminalThemePreset {
    preset_from_theme_mode(theme_mode)
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

fn rgb_components(rgb: u32) -> (u8, u8, u8) {
    (
        ((rgb >> 16) & 0xff) as u8,
        ((rgb >> 8) & 0xff) as u8,
        (rgb & 0xff) as u8,
    )
}

fn rgba_components(rgb: u32, alpha: f32) -> (u8, u8, u8, f32) {
    let (red, green, blue) = rgb_components(rgb);
    (red, green, blue, alpha)
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
