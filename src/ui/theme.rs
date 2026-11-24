//! Native macOS Sonoma / Sequoia visual design system for Mica Term.
//!
//! Provides neutral Cupertino surface hierarchy, vibrant Apple system accents,
//! unified toolbar tokens, and reusable macOS squircle badges & inset grouped containers.

use gpui_kit::assets::IconName as LucideIcon;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Sizable as _,
    theme::{Theme, ThemeMode, ThemeToken},
    v_flex,
};
use gpui_kit::{
    App, Div, Hsla, ParentElement as _, Rgba, SharedString, Styled as _, Window, div, px,
};

use crate::ui::fonts::BUNDLED_MAPLE_FONT_FAMILY;

#[inline]
pub fn macos_hex(rgb: u32, alpha: f32) -> Hsla {
    let r = ((rgb >> 16) & 0xff) as f32 / 255.0;
    let g = ((rgb >> 8) & 0xff) as f32 / 255.0;
    let b = (rgb & 0xff) as f32 / 255.0;
    Hsla::from(Rgba { r, g, b, a: alpha })
}

/// Applies the native macOS Sonoma / Sequoia color palette and geometry tokens.
///
/// Always switches `ThemeMode` first via `Theme::change`, then applies custom tokens via
/// `Theme::update` so GPUI Component's Base scrollbar/input/dialog projections stay in sync.
pub fn apply_macos_theme(dark_mode: bool, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(
        if dark_mode {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        window,
        cx,
    );

    Theme::update(cx, |theme| {
        theme.font_family = SharedString::from(BUNDLED_MAPLE_FONT_FAMILY);
        theme.mono_font_family = SharedString::from(BUNDLED_MAPLE_FONT_FAMILY);
        theme.radius = px(7.0);
        theme.radius_lg = px(11.0);
        theme.shadow = true;

        if dark_mode {
            let sys_blue = macos_hex(0x0A84FF, 1.0);
            let sys_blue_hover = macos_hex(0x2994FF, 1.0);
            let sys_blue_active = macos_hex(0x0070E0, 1.0);
            let sys_red = macos_hex(0xFF453A, 1.0);
            let sys_green = macos_hex(0x30D158, 1.0);
            let sys_amber = macos_hex(0xFF9F0A, 1.0);

            theme.background = macos_hex(0x1C1C1E, 1.0);
            theme.foreground = macos_hex(0xF2F2F7, 1.0);
            theme.muted = macos_hex(0x2C2C2E, 1.0);
            theme.muted_foreground = macos_hex(0x98989D, 1.0);

            theme.title_bar = macos_hex(0x28282B, 1.0);
            theme.title_bar_border = macos_hex(0xFFFFFF, 0.08);

            theme.sidebar = macos_hex(0x242427, 1.0);
            theme.sidebar_foreground = macos_hex(0xE5E5EA, 1.0);
            theme.sidebar_border = macos_hex(0xFFFFFF, 0.075);
            theme.sidebar_accent = macos_hex(0x0A84FF, 0.22);
            theme.sidebar_accent_foreground = macos_hex(0xFFFFFF, 1.0);
            theme.sidebar_primary = sys_blue;
            theme.sidebar_primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.popover = macos_hex(0x2C2C30, 1.0);
            theme.popover_foreground = macos_hex(0xF2F2F7, 1.0);
            theme.group_box = macos_hex(0x262629, 1.0);
            theme.group_box_foreground = macos_hex(0xF2F2F7, 1.0);

            theme.border = macos_hex(0xFFFFFF, 0.10);
            theme.input = macos_hex(0xFFFFFF, 0.13);
            theme.ring = macos_hex(0x0A84FF, 0.50);
            theme.caret = sys_blue;
            theme.selection = macos_hex(0x0A84FF, 0.32);

            theme.primary = sys_blue;
            theme.primary_hover = sys_blue_hover;
            theme.primary_active = sys_blue_active;
            theme.primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.button_primary = sys_blue;
            theme.button_primary_hover = sys_blue_hover;
            theme.button_primary_active = sys_blue_active;
            theme.button_primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.secondary = macos_hex(0x343438, 1.0);
            theme.secondary_hover = macos_hex(0x3F3F44, 1.0);
            theme.secondary_active = macos_hex(0x49494F, 1.0);
            theme.secondary_foreground = macos_hex(0xF2F2F7, 1.0);

            theme.button_secondary = macos_hex(0x343438, 1.0);
            theme.button_secondary_hover = macos_hex(0x3F3F44, 1.0);
            theme.button_secondary_active = macos_hex(0x49494F, 1.0);
            theme.button_secondary_foreground = macos_hex(0xF2F2F7, 1.0);

            theme.accent = macos_hex(0x0A84FF, 0.22);
            theme.accent_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.colors.list = macos_hex(0x242427, 1.0);
            theme.list_active = macos_hex(0x0A84FF, 0.24);
            theme.list_active_border = macos_hex(0x0A84FF, 0.42);
            theme.list_hover = macos_hex(0xFFFFFF, 0.055);

            theme.tab_bar = macos_hex(0x1E1E21, 1.0);
            theme.tab_bar_segmented = macos_hex(0x1B1B1E, 1.0);
            theme.tab = macos_hex(0xFFFFFF, 0.0);
            theme.tab_active = macos_hex(0x3A3A40, 1.0);
            theme.tab_foreground = macos_hex(0x98989D, 1.0);
            theme.tab_active_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.switch = macos_hex(0x39393D, 1.0);
            theme.switch_thumb = macos_hex(0xFFFFFF, 1.0);

            theme.scrollbar = macos_hex(0xFFFFFF, 0.0);
            theme.scrollbar_thumb = macos_hex(0xFFFFFF, 0.22);
            theme.scrollbar_thumb_hover = macos_hex(0xFFFFFF, 0.38);

            theme.success = sys_green;
            theme.warning = sys_amber;
            theme.danger = sys_red;
            theme.danger_hover = macos_hex(0xFF6259, 1.0);
            theme.danger_active = macos_hex(0xD93025, 1.0);
            theme.danger_foreground = macos_hex(0xFFFFFF, 1.0);
            theme.button_danger = sys_red;
            theme.button_danger_hover = macos_hex(0xFF6259, 1.0);
            theme.button_danger_active = macos_hex(0xD93025, 1.0);
            theme.button_danger_foreground = macos_hex(0xFFFFFF, 1.0);
            theme.overlay = macos_hex(0x000000, 0.46);
        } else {
            let sys_blue = macos_hex(0x007AFF, 1.0);
            let sys_blue_hover = macos_hex(0x006AE0, 1.0);
            let sys_blue_active = macos_hex(0x0059BF, 1.0);
            let sys_red = macos_hex(0xFF3B30, 1.0);
            let sys_green = macos_hex(0x28CD41, 1.0);
            let sys_amber = macos_hex(0xFF9500, 1.0);

            theme.background = macos_hex(0xFFFFFF, 1.0);
            theme.foreground = macos_hex(0x1D1D1F, 1.0);
            theme.muted = macos_hex(0xF2F2F7, 1.0);
            theme.muted_foreground = macos_hex(0x6E6E73, 1.0);

            theme.title_bar = macos_hex(0xF4F4F7, 1.0);
            theme.title_bar_border = macos_hex(0x000000, 0.085);

            theme.sidebar = macos_hex(0xF2F2F7, 1.0);
            theme.sidebar_foreground = macos_hex(0x1D1D1F, 1.0);
            theme.sidebar_border = macos_hex(0x000000, 0.08);
            theme.sidebar_accent = macos_hex(0x007AFF, 0.14);
            theme.sidebar_accent_foreground = sys_blue;
            theme.sidebar_primary = sys_blue;
            theme.sidebar_primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.popover = macos_hex(0xFFFFFF, 1.0);
            theme.popover_foreground = macos_hex(0x1D1D1F, 1.0);
            theme.group_box = macos_hex(0xF5F5F8, 1.0);
            theme.group_box_foreground = macos_hex(0x1D1D1F, 1.0);

            theme.border = macos_hex(0x000000, 0.09);
            theme.input = macos_hex(0x000000, 0.12);
            theme.ring = macos_hex(0x007AFF, 0.42);
            theme.caret = sys_blue;
            theme.selection = macos_hex(0x007AFF, 0.20);

            theme.primary = sys_blue;
            theme.primary_hover = sys_blue_hover;
            theme.primary_active = sys_blue_active;
            theme.primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.button_primary = sys_blue;
            theme.button_primary_hover = sys_blue_hover;
            theme.button_primary_active = sys_blue_active;
            theme.button_primary_foreground = macos_hex(0xFFFFFF, 1.0);

            theme.secondary = macos_hex(0xE5E5EA, 1.0);
            theme.secondary_hover = macos_hex(0xDCDCE2, 1.0);
            theme.secondary_active = macos_hex(0xD1D1D8, 1.0);
            theme.secondary_foreground = macos_hex(0x1D1D1F, 1.0);

            theme.button_secondary = macos_hex(0xE5E5EA, 1.0);
            theme.button_secondary_hover = macos_hex(0xDCDCE2, 1.0);
            theme.button_secondary_active = macos_hex(0xD1D1D8, 1.0);
            theme.button_secondary_foreground = macos_hex(0x1D1D1F, 1.0);

            theme.accent = macos_hex(0x007AFF, 0.12);
            theme.accent_foreground = sys_blue;

            theme.colors.list = macos_hex(0xF2F2F7, 1.0);
            theme.list_active = macos_hex(0x007AFF, 0.15);
            theme.list_active_border = macos_hex(0x007AFF, 0.30);
            theme.list_hover = macos_hex(0x000000, 0.045);

            theme.tab_bar = macos_hex(0xE5E5EA, 1.0);
            theme.tab_bar_segmented = macos_hex(0xE3E3E8, 1.0);
            theme.tab = macos_hex(0x000000, 0.0);
            theme.tab_active = macos_hex(0xFFFFFF, 1.0);
            theme.tab_foreground = macos_hex(0x6E6E73, 1.0);
            theme.tab_active_foreground = macos_hex(0x1D1D1F, 1.0);

            theme.switch = macos_hex(0xE5E5EA, 1.0);
            theme.switch_thumb = macos_hex(0xFFFFFF, 1.0);

            theme.scrollbar = macos_hex(0x000000, 0.0);
            theme.scrollbar_thumb = macos_hex(0x000000, 0.20);
            theme.scrollbar_thumb_hover = macos_hex(0x000000, 0.36);

            theme.success = sys_green;
            theme.warning = sys_amber;
            theme.danger = sys_red;
            theme.danger_hover = macos_hex(0xE6352B, 1.0);
            theme.danger_active = macos_hex(0xCC2E25, 1.0);
            theme.danger_foreground = macos_hex(0xFFFFFF, 1.0);
            theme.button_danger = sys_red;
            theme.button_danger_hover = macos_hex(0xE6352B, 1.0);
            theme.button_danger_active = macos_hex(0xCC2E25, 1.0);
            theme.button_danger_foreground = macos_hex(0xFFFFFF, 1.0);
            theme.overlay = macos_hex(0x000000, 0.28);
        }

        // Replace default TitleBar / Button gradients with clean solid Cupertino tokens
        theme.tokens.title_bar = ThemeToken::from(theme.colors.title_bar);
        theme.tokens.button_primary = ThemeToken::from(theme.colors.button_primary);
        theme.tokens.button_secondary = ThemeToken::from(theme.colors.button_secondary);
        theme.tokens.button_danger = ThemeToken::from(theme.colors.button_danger);
    });
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
    let dark = cx.theme().is_dark();
    if upper.contains("PROD") || upper.contains("ONLINE") || upper == "PRD" || tag.contains("生产")
    {
        let c = if dark {
            macos_hex(0xFF453A, 1.0)
        } else {
            macos_hex(0xD70015, 1.0)
        };
        (c.opacity(0.16), c)
    } else if upper.contains("STG")
        || upper.contains("PRE")
        || upper.contains("UAT")
        || tag.contains("预发")
    {
        let c = if dark {
            macos_hex(0xFF9F0A, 1.0)
        } else {
            macos_hex(0xB25000, 1.0)
        };
        (c.opacity(0.16), c)
    } else if upper.contains("DEV")
        || upper.contains("TEST")
        || upper.contains("LOCAL")
        || tag.contains("测试")
        || tag.contains("开发")
    {
        let c = if dark {
            macos_hex(0x30D158, 1.0)
        } else {
            macos_hex(0x1D8330, 1.0)
        };
        (c.opacity(0.16), c)
    } else {
        (cx.theme().secondary, cx.theme().muted_foreground)
    }
}

/// Elevated macOS Inset Grouped container (like System Settings & Finder cards).
pub fn macos_inset_group(cx: &App) -> Div {
    let surface = if cx.theme().is_dark() {
        macos_hex(0x252528, 1.0)
    } else {
        macos_hex(0xF7F7FA, 1.0)
    };
    v_flex()
        .rounded(px(10.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(surface)
        .overflow_hidden()
}

/// MacOS recessed segmented control track (`NSSegmentedControl` background).
pub fn macos_segmented_track(cx: &App) -> Div {
    let track_bg = if cx.theme().is_dark() {
        macos_hex(0x18181B, 0.85)
    } else {
        macos_hex(0xE3E3E8, 0.90)
    };
    div()
        .flex()
        .flex_row()
        .items_center()
        .p(px(2.0))
        .gap(px(2.0))
        .rounded(px(7.0))
        .bg(track_bg)
}
