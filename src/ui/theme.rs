//! Cohesive Application & Terminal Theme Engine for Mica Term.
//!
//! Inspired by Superlogical Rex & Termius: synchronizes window chrome (TitleBar, Sidebar,
//! Inset Cards, Tabs, Inputs, Popovers, Semantic Badges) and Code Editor `HighlightTheme`
//! with the active `ThemeSpec`, or falls back to neutral Cupertino chrome when `tint_ui_with_theme`
//! is disabled.

use std::sync::Arc;

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    highlighter::HighlightTheme,
    theme::{Theme, ThemeMode, ThemeToken},
    v_flex,
};
use gpui_kit::{
    App, Div, Hsla, ParentElement as _, Rgba, SharedString, Styled as _, Window, div, px,
};
use serde_json::json;

use crate::domain::AppSettings;
use crate::ui::fonts::BUNDLED_MAPLE_FONT_FAMILY;
use crate::ui::theme_catalog::{
    ThemeRegistry, ThemeSpec, contrasting_ink, ensure_readable_fg, mix_hex, relative_luminance,
};

#[inline]
pub fn macos_hex(rgb: u32, alpha: f32) -> Hsla {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    Hsla::from(Rgba { r, g, b, a: alpha })
}

/// Applies the active `AppSettings` theme (both UI chrome and code editor highlight theme).
pub fn apply_workspace_theme(settings: &AppSettings, window: Option<&mut Window>, cx: &mut App) {
    let terminal_spec =
        ThemeRegistry::global().resolve(settings.active_theme_id(), settings.dark_mode);
    let chrome_spec = if settings.tint_ui_with_theme {
        terminal_spec.clone()
    } else {
        let neutral_id = if terminal_spec.dark {
            "cupertino-dark"
        } else {
            "cupertino-light"
        };
        ThemeRegistry::global().resolve(neutral_id, terminal_spec.dark)
    };
    apply_theme_specs(&chrome_spec, &terminal_spec, window, cx);
}

/// Legacy compatibility entry point for applying default Dark/Light theme.
pub fn apply_macos_theme(dark_mode: bool, window: Option<&mut Window>, cx: &mut App) {
    let settings = AppSettings {
        dark_mode,
        ..Default::default()
    };
    apply_workspace_theme(&settings, window, cx);
}

pub fn apply_theme_specs(
    chrome: &ThemeSpec,
    syntax_spec: &ThemeSpec,
    window: Option<&mut Window>,
    cx: &mut App,
) {
    let dark = chrome.dark;
    Theme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );

    let hl_theme = build_editor_highlight_theme(syntax_spec);

    Theme::update(cx, |theme| {
        theme.mono_font_family = SharedString::from(BUNDLED_MAPLE_FONT_FAMILY);
        theme.radius = px(7.0);
        theme.radius_lg = px(11.0);
        theme.shadow = true;
        theme.highlight_theme = hl_theme;

        let pole = if dark { 0xFFFFFF } else { 0x000000 };
        let inv_pole = if dark { 0x000000 } else { 0xFFFFFF };

        let accent = chrome.accent;
        let accent_hover = if dark {
            mix_hex(accent, 0xFFFFFF, 0.14)
        } else {
            mix_hex(accent, 0x000000, 0.12)
        };
        let accent_active = if dark {
            mix_hex(accent, 0x000000, 0.16)
        } else {
            mix_hex(accent, 0x000000, 0.22)
        };
        let primary_fg = macos_hex(contrasting_ink(accent), 1.0);

        let red_hex = ensure_readable_fg(chrome.ansi[1], chrome.ui_bg, dark, 3.1);
        let green_hex = ensure_readable_fg(chrome.ansi[2], chrome.ui_bg, dark, 2.9);
        let amber_hex = ensure_readable_fg(chrome.ansi[3], chrome.ui_bg, dark, 2.9);
        let blue_hex = ensure_readable_fg(chrome.ansi[4], chrome.ui_bg, dark, 3.1);
        let magenta_hex = ensure_readable_fg(chrome.ansi[5], chrome.ui_bg, dark, 3.0);
        let cyan_hex = ensure_readable_fg(chrome.ansi[6], chrome.ui_bg, dark, 3.0);

        let shell = if dark {
            chrome.sidebar_bg
        } else {
            chrome.titlebar_bg
        };
        let surface = if dark {
            chrome.ui_bg
        } else {
            chrome.surface_bg
        };
        // The same secondary text appears on both the shell and the canvas.
        let contrast_bg = if (relative_luminance(shell) > relative_luminance(surface)) == dark {
            shell
        } else {
            surface
        };
        let secondary_bg = shell;
        let secondary_hover = mix_hex(shell, pole, 0.06);
        let secondary_active = mix_hex(shell, pole, 0.11);

        theme.background = macos_hex(surface, 1.0);
        theme.foreground = macos_hex(chrome.term_fg, 1.0);
        theme.muted = macos_hex(shell, 1.0);
        theme.muted_foreground = macos_hex(
            ensure_readable_fg(chrome.muted_fg, contrast_bg, dark, 4.5),
            1.0,
        );

        theme.title_bar = macos_hex(shell, 1.0);
        theme.title_bar_border = theme.transparent;
        theme.sidebar = theme.title_bar;
        theme.sidebar_foreground = macos_hex(chrome.term_fg, 1.0);
        theme.border = macos_hex(pole, if dark { 0.14 } else { 0.15 });
        theme.sidebar_border = theme.border;
        theme.sidebar_accent = macos_hex(accent, if dark { 0.20 } else { 0.14 });
        theme.sidebar_accent_foreground = if dark {
            macos_hex(0xFFFFFF, 1.0)
        } else {
            macos_hex(ensure_readable_fg(accent, shell, false, 3.5), 1.0)
        };
        theme.sidebar_primary = macos_hex(accent, 1.0);
        theme.sidebar_primary_foreground = primary_fg;

        theme.popover = theme.background;
        theme.popover_foreground = macos_hex(chrome.term_fg, 1.0);
        theme.group_box = theme.background;
        theme.group_box_foreground = macos_hex(chrome.term_fg, 1.0);

        theme.input = macos_hex(pole, if dark { 0.135 } else { 0.125 });
        theme.ring = macos_hex(accent, if dark { 0.50 } else { 0.42 });
        theme.caret = macos_hex(accent, 1.0);
        theme.selection = macos_hex(chrome.selection, if dark { 0.30 } else { 0.20 });

        theme.primary = macos_hex(accent, 1.0);
        theme.primary_hover = macos_hex(accent_hover, 1.0);
        theme.primary_active = macos_hex(accent_active, 1.0);
        theme.primary_foreground = primary_fg;

        theme.button_primary = theme.primary;
        theme.button_primary_hover = theme.primary_hover;
        theme.button_primary_active = theme.primary_active;
        theme.button_primary_foreground = primary_fg;

        theme.secondary = macos_hex(secondary_bg, 1.0);
        theme.secondary_hover = macos_hex(secondary_hover, 1.0);
        theme.secondary_active = macos_hex(secondary_active, 1.0);
        theme.secondary_foreground = macos_hex(chrome.term_fg, 1.0);

        theme.button_secondary = theme.secondary;
        theme.button_secondary_hover = theme.secondary_hover;
        theme.button_secondary_active = theme.secondary_active;
        theme.button_secondary_foreground = theme.secondary_foreground;

        theme.button = theme.secondary;
        theme.button_hover = theme.secondary_hover;
        theme.button_active = theme.secondary_active;
        theme.button_foreground = theme.secondary_foreground;

        theme.accent = macos_hex(accent, if dark { 0.22 } else { 0.13 });
        theme.accent_foreground = if dark {
            macos_hex(chrome.term_fg, 1.0)
        } else {
            macos_hex(ensure_readable_fg(accent, chrome.ui_bg, false, 3.5), 1.0)
        };

        theme.colors.list = theme.sidebar;
        theme.list_active = macos_hex(accent, if dark { 0.22 } else { 0.15 });
        theme.list_active_border = macos_hex(accent, if dark { 0.42 } else { 0.32 });
        theme.list_hover = macos_hex(pole, if dark { 0.055 } else { 0.048 });

        theme.tab_bar = theme.title_bar;
        theme.tab_bar_segmented = macos_hex(pole, 0.06);
        theme.tab = macos_hex(inv_pole, 0.0);
        theme.tab_active = theme.background;
        theme.tab_foreground = theme.muted_foreground;
        theme.tab_active_foreground = macos_hex(chrome.term_fg, 1.0);

        theme.switch = macos_hex(secondary_bg, 1.0);
        theme.switch_thumb = macos_hex(0xFFFFFF, 1.0);

        theme.scrollbar = macos_hex(pole, 0.0);
        theme.scrollbar_thumb = macos_hex(pole, if dark { 0.22 } else { 0.20 });
        theme.scrollbar_thumb_hover = macos_hex(pole, if dark { 0.38 } else { 0.36 });

        theme.success = macos_hex(green_hex, 1.0);
        theme.success_foreground = macos_hex(contrasting_ink(green_hex), 1.0);
        theme.warning = macos_hex(amber_hex, 1.0);
        theme.warning_foreground = macos_hex(contrasting_ink(amber_hex), 1.0);
        theme.info = macos_hex(blue_hex, 1.0);
        theme.info_foreground = macos_hex(contrasting_ink(blue_hex), 1.0);

        let danger_hover = if dark {
            mix_hex(red_hex, 0xFFFFFF, 0.14)
        } else {
            mix_hex(red_hex, 0x000000, 0.10)
        };
        let danger_active = mix_hex(red_hex, 0x000000, 0.18);
        let danger_fg = macos_hex(contrasting_ink(red_hex), 1.0);
        theme.danger = macos_hex(red_hex, 1.0);
        theme.danger_hover = macos_hex(danger_hover, 1.0);
        theme.danger_active = macos_hex(danger_active, 1.0);
        theme.danger_foreground = danger_fg;
        theme.button_danger = theme.danger;
        theme.button_danger_hover = theme.danger_hover;
        theme.button_danger_active = theme.danger_active;
        theme.button_danger_foreground = danger_fg;

        theme.red = macos_hex(red_hex, 1.0);
        theme.red_light = macos_hex(chrome.ansi[9], 1.0);
        theme.green = macos_hex(green_hex, 1.0);
        theme.green_light = macos_hex(chrome.ansi[10], 1.0);
        theme.yellow = macos_hex(amber_hex, 1.0);
        theme.yellow_light = macos_hex(chrome.ansi[11], 1.0);
        theme.blue = macos_hex(blue_hex, 1.0);
        theme.blue_light = macos_hex(chrome.ansi[12], 1.0);
        theme.magenta = macos_hex(magenta_hex, 1.0);
        theme.magenta_light = macos_hex(chrome.ansi[13], 1.0);
        theme.cyan = macos_hex(cyan_hex, 1.0);
        theme.cyan_light = macos_hex(chrome.ansi[14], 1.0);

        theme.overlay = macos_hex(0x000000, if dark { 0.48 } else { 0.28 });

        // Replace default TitleBar / Button gradients with clean solid tokens
        theme.tokens.title_bar = ThemeToken::from(theme.colors.title_bar);
        theme.tokens.button = ThemeToken::from(theme.colors.button);
        theme.tokens.button_primary = ThemeToken::from(theme.colors.button_primary);
        theme.tokens.button_secondary = ThemeToken::from(theme.colors.button_secondary);
        theme.tokens.button_danger = ThemeToken::from(theme.colors.button_danger);
    });
}

fn build_editor_highlight_theme(spec: &ThemeSpec) -> Arc<HighlightTheme> {
    let dark = spec.dark;
    let bg = spec.term_bg;
    let rd = |hex: u32, min_c: f32| macos_hex(ensure_readable_fg(hex, bg, dark, min_c), 1.0);

    let comment = rd(mix_hex(spec.muted_fg, spec.ansi[8], 0.35), 2.7);
    let comment_doc = rd(mix_hex(spec.muted_fg, spec.term_fg, 0.25), 3.0);
    let red = rd(if dark { spec.ansi[9] } else { spec.ansi[1] }, 3.5);
    let green = rd(spec.ansi[2], 3.4);
    let yellow = rd(if dark { spec.ansi[11] } else { spec.ansi[3] }, 3.4);
    let blue = rd(if dark { spec.ansi[12] } else { spec.ansi[4] }, 3.5);
    let magenta = rd(spec.ansi[5], 3.4);
    let cyan = rd(spec.ansi[6], 3.4);
    let orange = rd(
        mix_hex(
            if dark { spec.ansi[11] } else { spec.ansi[3] },
            if dark { spec.ansi[9] } else { spec.ansi[1] },
            0.42,
        ),
        3.4,
    );
    let variable = rd(mix_hex(spec.ansi[5], spec.term_fg, 0.35), 3.5);

    let editor_bg = macos_hex(spec.term_bg, 1.0);
    let editor_fg = macos_hex(spec.term_fg, 1.0);
    let active_line = macos_hex(
        if dark {
            mix_hex(spec.term_bg, 0xFFFFFF, 0.045)
        } else {
            mix_hex(spec.term_bg, 0x000000, 0.035)
        },
        1.0,
    );
    let gutter_fg = macos_hex(ensure_readable_fg(spec.muted_fg, bg, dark, 2.2), 1.0);

    let val = json!({
        "name": spec.name,
        "appearance": if dark { "dark" } else { "light" },
        "style": {
            "editor.background": editor_bg,
            "editor.foreground": editor_fg,
            "editor.active_line.background": active_line,
            "editor.line_number": gutter_fg,
            "editor.active_line_number": editor_fg,
            "syntax": {
                "comment": { "color": comment },
                "comment.doc": { "color": comment_doc },
                "string": { "color": green },
                "string.escape": { "color": cyan },
                "string.regex": { "color": cyan },
                "number": { "color": orange },
                "boolean": { "color": orange },
                "constant": { "color": yellow },
                "keyword": { "color": magenta },
                "operator": { "color": cyan },
                "function": { "color": blue },
                "type": { "color": yellow },
                "tag": { "color": red },
                "attribute": { "color": cyan },
                "property": { "color": cyan },
                "variable": { "color": variable },
                "title": { "color": blue, "font_weight": 700 }
            }
        }
    });

    Arc::new(
        serde_json::from_value::<HighlightTheme>(val)
            .expect("internal HighlightTheme JSON schema must match gpui-component"),
    )
}

/// Renders a macOS System Settings / Termius style rounded squircle icon badge.
pub fn macos_squircle_badge(icon: LucideIcon, tint: Hsla, box_px: f32, radius_px: f32) -> Div {
    let icon_el = if box_px >= 34.0 {
        Icon::new(icon).text_color(tint)
    } else if box_px >= 26.0 {
        Icon::new(icon).small().text_color(tint)
    } else {
        Icon::new(icon).xsmall().text_color(tint)
    };

    div()
        .flex_shrink_0()
        .w(px(box_px))
        .h(px(box_px))
        .rounded(px(radius_px))
        .bg(tint.opacity(0.15))
        .flex()
        .items_center()
        .justify_center()
        .child(icon_el)
}

/// Returns `(bg_color, fg_color)` for environment tags (`PROD`, `STG`, `DEV`, etc.).
pub fn macos_env_badge_colors(tag: &str, cx: &App) -> (Hsla, Hsla) {
    let upper = tag.trim().to_ascii_uppercase();
    if upper.contains("PROD") || upper.contains("ONLINE") || upper == "PRD" || tag.contains("生产")
    {
        let c = cx.theme().danger;
        (c.opacity(0.16), c)
    } else if upper.contains("STG")
        || upper.contains("PRE")
        || upper.contains("UAT")
        || tag.contains("预发")
    {
        let c = cx.theme().warning;
        (c.opacity(0.16), c)
    } else if upper.contains("DEV")
        || upper.contains("TEST")
        || upper.contains("LOCAL")
        || tag.contains("测试")
        || tag.contains("开发")
    {
        let c = cx.theme().success;
        (c.opacity(0.16), c)
    } else {
        (cx.theme().secondary, cx.theme().muted_foreground)
    }
}

/// Elevated Inset Grouped container (System Settings & Rex / Termius cards) driven by active theme.
pub fn macos_inset_group(cx: &App) -> Div {
    v_flex()
        .w_full()
        .min_w_0()
        .rounded(px(10.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().group_box)
        .overflow_hidden()
}

/// Recessed segmented control track (`NSSegmentedControl` background) driven by active theme.
pub fn macos_segmented_track(cx: &App) -> Div {
    div()
        .flex()
        .flex_row()
        .items_center()
        .p(px(2.0))
        .gap(px(2.0))
        .rounded(px(7.0))
        .bg(cx.theme().tab_bar_segmented)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::CodeTokenRole;
    use crate::terminal::{SemanticRole, TerminalPalette};
    use crate::ui::theme_catalog::builtin_themes;
    use gpui_kit::component::input::HighlightStyleResolver as _;

    #[test]
    fn every_builtin_theme_produces_valid_highlight_theme_and_terminal_palette() {
        let themes = builtin_themes();
        assert_eq!(themes.len(), 54);
        for spec in &themes {
            let hl = build_editor_highlight_theme(spec);
            assert!(hl.style("keyword").is_some());
            assert!(hl.style("string").is_some());
            assert!(hl.style("comment").is_some());
            assert!(hl.style("function").is_some());

            let pal = TerminalPalette::from_theme(spec);
            assert_eq!(pal.dark, spec.dark);
            assert_ne!(
                pal.semantic_style(SemanticRole::InputCommand).0,
                pal.code_token_style(CodeTokenRole::Function).0,
                "InputCommand and Function collided in theme {}",
                spec.id
            );
        }
    }
}
