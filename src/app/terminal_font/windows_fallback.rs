//! Windows text fallback helper that discovers installed system families for mixed text.

use crate::app::terminal_font::windows_locator::WindowsFontLocator;

const EMOJI_FALLBACK_CANDIDATES: &[&str] = &[
    "Segoe UI Emoji",
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
const CJK_FALLBACK_CANDIDATES: &[&str] = &[
    "Microsoft YaHei UI",
    "Microsoft JhengHei UI",
    "Sarasa Term SC Nerd",
    "Noto Sans CJK SC",
    "WenQuanYi Zen Hei",
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
    ) -> Vec<String> {
        let mut families = vec![primary_family.to_string()];

        if contains_color_glyph_text(text)
            && let Some(family_name) = locator.resolve_family(EMOJI_FALLBACK_CANDIDATES)
        {
            push_unique_family(&mut families, family_name);
        }

        if contains_symbol_text(text)
            && let Some(family_name) = locator.resolve_family(SYMBOL_FALLBACK_CANDIDATES)
        {
            push_unique_family(&mut families, family_name);
        }

        if contains_cjk_text(text)
            && let Some(family_name) = locator.resolve_family(CJK_FALLBACK_CANDIDATES)
        {
            push_unique_family(&mut families, family_name);
        }

        if families.len() == 1
            && let Some(family_name) = locator.first_distinct_family(&families)
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
    ) -> String {
        if contains_color_glyph_text(text) {
            return resolve_fallback_family(locator, primary_family, EMOJI_FALLBACK_CANDIDATES);
        }
        if contains_symbol_text(text) {
            return resolve_fallback_family(locator, primary_family, SYMBOL_FALLBACK_CANDIDATES);
        }
        if contains_cjk_text(text) {
            return resolve_fallback_family(locator, primary_family, CJK_FALLBACK_CANDIDATES);
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
    text.chars().any(|ch| {
        matches!(
            ch,
            '\u{2600}'..='\u{27bf}' | '\u{1f300}'..='\u{1faff}'
        )
    })
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
            '\u{2e80}'..='\u{9fff}' | '\u{f900}'..='\u{faff}'
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

    locator
        .first_distinct_family(&[primary_family.to_string()])
        .unwrap_or_else(|| primary_family.to_string())
}
