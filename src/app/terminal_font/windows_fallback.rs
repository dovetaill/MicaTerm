//! Windows text fallback helper that discovers installed system families for mixed text.

use crate::app::font_diagnostics::{
    TERMINAL_ICON_FALLBACK_FAMILIES, TERMINAL_NERD_FALLBACK_FAMILIES,
};
use crate::app::terminal_emoji::{ClusterRenderKind, classify_cluster_render_kind};
use crate::app::terminal_font::backend::{
    DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY, DEFAULT_TERMINAL_FONT_FAMILY,
};
use crate::app::terminal_font::windows_locator::WindowsFontLocator;

const EMOJI_FALLBACK_CANDIDATES: &[&str] = &[
    DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY,
    "Noto Color Emoji",
    "Noto Emoji",
    "Apple Color Emoji",
];
const SYMBOL_FALLBACK_CANDIDATES: &[&str] = &[
    "Segoe UI Symbol",
    "Noto Sans Symbols",
    "Noto Sans Symbols 2",
    "Symbola",
    "DejaVu Sans",
];
pub struct WindowsFontFallbackResolver;

impl WindowsFontFallbackResolver {
    pub fn new() -> Self {
        Self
    }

    pub fn discover_fallback_families(
        &self,
        locator: &WindowsFontLocator,
        primary_family: &str,
        text: &str,
        primary_supports_text: bool,
    ) -> Vec<String> {
        let primary_family = normalize_primary_family(primary_family);
        let mut families = vec![primary_family.to_string()];

        if contains_color_glyph_text(text)
            && let Some(family_name) = locator.resolve_family(EMOJI_FALLBACK_CANDIDATES)
        {
            push_unique_family(&mut families, family_name);
        }

        if contains_private_use_text(text)
            && !primary_supports_text
            && let Some(family_name) = resolve_private_use_fallback_family(locator)
        {
            push_unique_family(&mut families, family_name);
        }

        if contains_symbol_text(text)
            && !primary_supports_text
            && let Some(family_name) = locator.resolve_family(SYMBOL_FALLBACK_CANDIDATES)
        {
            push_unique_family(&mut families, family_name);
        }

        families
    }

    pub fn resolve_family_for_text(
        &self,
        locator: &WindowsFontLocator,
        primary_family: &str,
        text: &str,
        primary_supports_text: bool,
    ) -> String {
        let primary_family = normalize_primary_family(primary_family);

        if contains_color_glyph_text(text) {
            return resolve_fallback_family(locator, primary_family, EMOJI_FALLBACK_CANDIDATES);
        }
        if primary_supports_text {
            return primary_family.to_string();
        }
        if contains_private_use_text(text) {
            return resolve_private_use_fallback_family(locator)
                .unwrap_or_else(|| primary_family.to_string());
        }
        if contains_symbol_text(text) {
            return resolve_fallback_family(locator, primary_family, SYMBOL_FALLBACK_CANDIDATES);
        }
        if contains_cjk_text(text) {
            return primary_family.to_string();
        }

        primary_family.to_string()
    }
}

impl Default for WindowsFontFallbackResolver {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn contains_color_glyph_text(text: &str) -> bool {
    classify_cluster_render_kind(text) == ClusterRenderKind::Emoji
}

fn contains_symbol_text(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{2190}'..='\u{21ff}'
                | '\u{2300}'..='\u{23ff}'
                | '\u{2460}'..='\u{24ff}'
                | '\u{25a0}'..='\u{25ff}'
        )
    })
}

fn contains_cjk_text(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{2e80}'..='\u{9fff}' | '\u{f900}'..='\u{faff}' | '\u{ff00}'..='\u{ffef}'
        )
    })
}

fn contains_private_use_text(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{e000}'..='\u{f8ff}'
                | '\u{f0000}'..='\u{ffffd}'
                | '\u{100000}'..='\u{10fffd}'
        )
    })
}

fn push_unique_family(families: &mut Vec<String>, family_name: String) {
    if families
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&family_name))
    {
        return;
    }

    families.push(family_name);
}

fn resolve_fallback_family(
    locator: &WindowsFontLocator,
    primary_family: &str,
    candidates: &[&str],
) -> String {
    if let Some(family_name) = locator.resolve_family(candidates) {
        return family_name;
    }

    primary_family.to_string()
}

fn resolve_private_use_fallback_family(locator: &WindowsFontLocator) -> Option<String> {
    locator
        .resolve_family(TERMINAL_NERD_FALLBACK_FAMILIES)
        .or_else(|| locator.resolve_family(TERMINAL_ICON_FALLBACK_FAMILIES))
}

fn normalize_primary_family(primary_family: &str) -> &str {
    if primary_family.is_empty() {
        DEFAULT_TERMINAL_FONT_FAMILY
    } else {
        primary_family
    }
}
