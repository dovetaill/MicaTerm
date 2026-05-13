//! Terminal theme presets and palette conversion helpers.

use wezterm_term::color::{ColorPalette, RgbColor, SrgbaTuple};

use crate::theme::{ThemeMode, ThemeVariant, app_theme_spec, terminal_palette_spec, terminal_palette_spec_for};

#[derive(Debug, Clone, Copy)]
pub struct TerminalThemePreset {
    pub name: &'static str,
    pub background: u32,
    pub foreground: u32,
    pub viewport_bg_top: u32,
    pub viewport_bg_bottom: u32,
    pub cursor_bg: u32,
    pub cursor_fg: u32,
    pub selection_bg: (u8, u8, u8, f32),
    pub scrollbar_track: (u8, u8, u8),
    pub ansi: [(u8, u8, u8); 16],
    pub frame_bg: u32,
    pub scrollbar_thumb: (u8, u8, u8),
    pub scrollbar_thumb_active: (u8, u8, u8),
    pub split: (u8, u8, u8),
}

#[derive(Debug, Clone, Copy)]
pub struct ProjectedThemePreset {
    pub terminal: TerminalThemePreset,
    pub app_background: u32,
    pub titlebar_background: u32,
    pub tabbar_background: u32,
    pub sidebar_background: u32,
    pub sidebar_panel_background: u32,
    pub right_panel_background: u32,
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

fn terminal_preset_from_app_spec(
    spec: crate::theme::AppThemeSpec,
) -> TerminalThemePreset {
    TerminalThemePreset {
        name: spec.terminal.name,
        background: spec.terminal.background.base,
        foreground: spec.terminal.foreground.default,
        viewport_bg_top: spec.terminal.background.gradient_top,
        viewport_bg_bottom: spec.terminal.background.gradient_bottom,
        cursor_bg: spec.terminal.cursor.background,
        cursor_fg: spec.terminal.cursor.foreground,
        selection_bg: rgba_components(spec.terminal.selection.rgb, spec.terminal.selection.alpha),
        scrollbar_track: rgb_components(spec.terminal.scrollbar.track),
        ansi: spec.terminal.ansi.map(rgb_components),
        frame_bg: spec.shell.terminal_frame_background,
        scrollbar_thumb: rgb_components(spec.terminal.scrollbar.thumb),
        scrollbar_thumb_active: rgb_components(spec.terminal.scrollbar.thumb_active),
        split: rgb_components(spec.shell.separator),
    }
}

fn preset_from_spec(theme_mode: ThemeMode, variant: ThemeVariant) -> TerminalThemePreset {
    let spec = if variant == ThemeVariant::PremiumDefault {
        terminal_palette_spec(theme_mode)
    } else {
        terminal_palette_spec_for(theme_mode, variant)
    };
    TerminalThemePreset {
        name: spec.name,
        background: spec.default_bg,
        foreground: spec.default_fg,
        viewport_bg_top: spec.row_bg_even,
        viewport_bg_bottom: spec.row_bg_odd,
        cursor_bg: spec.cursor_bg,
        cursor_fg: spec.cursor_fg,
        selection_bg: rgba_components(spec.selection_rgb, spec.selection_alpha),
        scrollbar_track: rgb_components(spec.scrollbar_track),
        ansi: spec.ansi.map(rgb_components),
        frame_bg: spec.frame_bg,
        scrollbar_thumb: rgb_components(spec.scrollbar_thumb),
        scrollbar_thumb_active: rgb_components(spec.scrollbar_thumb_active),
        split: rgb_components(spec.split),
    }
}

pub fn preset_for_theme(theme_mode: ThemeMode, variant: ThemeVariant) -> TerminalThemePreset {
    preset_from_spec(theme_mode, variant)
}

pub fn preset_for_theme_mode(theme_mode: ThemeMode) -> TerminalThemePreset {
    preset_for_theme(theme_mode, ThemeVariant::PremiumDefault)
}

pub fn projected_theme_for(theme_mode: ThemeMode, variant: ThemeVariant) -> ProjectedThemePreset {
    let spec = app_theme_spec(theme_mode, variant);
    ProjectedThemePreset {
        terminal: terminal_preset_from_app_spec(spec),
        app_background: spec.shell.app_background,
        titlebar_background: spec.shell.titlebar_background,
        tabbar_background: spec.shell.tabbar_background,
        sidebar_background: spec.shell.sidebar_background,
        sidebar_panel_background: spec.shell.sidebar_panel_background,
        right_panel_background: spec.shell.right_panel_background,
        separator: spec.shell.separator,
        border: spec.shell.border,
        hairline: spec.shell.hairline,
        text_primary: spec.shell.text_primary,
        text_secondary: spec.shell.text_secondary,
        text_muted: spec.shell.text_muted,
        text_inactive: spec.shell.text_inactive,
        accent: spec.shell.accent,
        link_accent: spec.shell.link_accent,
        focus_ring: spec.shell.focus_ring,
        tab_active: spec.shell.tab_active,
        tab_inactive: spec.shell.tab_inactive,
        tab_hover: spec.shell.tab_hover,
        tab_active_indicator: spec.shell.tab_active_indicator,
        sidebar_item_hover: spec.shell.sidebar_item_hover,
        sidebar_item_selected: spec.shell.sidebar_item_selected,
        sidebar_item_selected_border: spec.shell.sidebar_item_selected_border,
    }
}

pub fn projected_theme_for_mode(theme_mode: ThemeMode) -> ProjectedThemePreset {
    projected_theme_for(theme_mode, ThemeVariant::PremiumDefault)
}

pub fn palette_for_theme(theme_mode: ThemeMode, variant: ThemeVariant) -> ColorPalette {
    preset_for_theme(theme_mode, variant).to_color_palette()
}

pub fn palette_for_theme_mode(theme_mode: ThemeMode) -> ColorPalette {
    palette_for_theme(theme_mode, ThemeVariant::PremiumDefault)
}

pub fn selection_overlay_rgba_for(theme_mode: ThemeMode, variant: ThemeVariant) -> u32 {
    let preset = preset_for_theme(theme_mode, variant);
    rgba_hex(preset.selection_bg)
}

pub fn selection_overlay_rgba(theme_mode: ThemeMode) -> u32 {
    selection_overlay_rgba_for(theme_mode, ThemeVariant::PremiumDefault)
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
