//! Shared UI/terminal font diagnostics and fallback policy helpers.

use std::collections::BTreeSet;

use fontdb::{Database, Query, Stretch, Style, Weight};
use slint::fontique_07::{fontique, shared_collection};

use crate::app::system_font_database::load_system_font_database;
use crate::app::terminal_font::DEFAULT_TERMINAL_FONT_WEIGHT;

pub const UI_FONT_FAMILY: &str = "MiSans";
pub const UI_FONT_DEFAULT_WEIGHT: i32 = 500;
pub const TERMINAL_PRIMARY_FAMILY: &str = "Sarasa Term SC Nerd";
pub const TERMINAL_EMOJI_FALLBACK_FAMILY: &str = "Segoe UI Emoji";
pub const UI_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", "Arial"];
pub const UI_SYMBOL_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI Symbol", "Segoe Fluent Icons", "Segoe MDL2 Assets"];
pub const UI_EMOJI_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI Emoji", "Noto Color Emoji", "Noto Emoji"];
pub const TERMINAL_NERD_FALLBACK_FAMILIES: &[&str] = &[
    "Sarasa Term SC Nerd",
    "Symbols Nerd Font Mono",
    "Symbols Nerd Font",
    "Maple Mono NF CN",
    "Maple Mono NF",
    "CaskaydiaCove Nerd Font Mono",
    "CaskaydiaMono Nerd Font Mono",
    "JetBrainsMono Nerd Font Mono",
    "MesloLGS NF",
];
pub const TERMINAL_SYMBOL_FALLBACK_FAMILIES: &[&str] = &[
    "Segoe UI Symbol",
    "Segoe Fluent Icons",
    "Segoe MDL2 Assets",
    "Noto Sans Symbols",
    "Noto Sans Symbols 2",
];
pub const TERMINAL_ICON_FALLBACK_FAMILIES: &[&str] =
    &["Segoe Fluent Icons", "Segoe MDL2 Assets", "Segoe UI Symbol"];

const BUNDLED_SARASA_TERM_SC_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf");

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FontFaceMatchDiagnostic {
    pub requested_family: String,
    pub resolved_family: String,
    pub fallback_family: Option<String>,
    pub post_script_name: Option<String>,
    pub weight: String,
    pub style: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalFontResolutionSnapshot {
    pub requested_family: String,
    pub requested_weight: String,
    pub requested_style: String,
    pub primary: FontFaceMatchDiagnostic,
    pub cjk: FontFaceMatchDiagnostic,
    pub symbol: FontFaceMatchDiagnostic,
    pub icon: FontFaceMatchDiagnostic,
    pub emoji: FontFaceMatchDiagnostic,
    pub mixes_multiple_unrelated_families: bool,
}

#[derive(Clone, Debug, Default)]
struct ParsedFaceMetadata {
    post_script_name: Option<String>,
    weight: Option<u16>,
    style: Option<String>,
}

pub(crate) fn configure_ui_font_fallbacks() {
    let mut collection = shared_collection();
    let fallback_ids = UI_FALLBACK_FAMILIES
        .iter()
        .filter_map(|family_name| collection.family_id(family_name))
        .collect::<Vec<_>>();

    if fallback_ids.is_empty() {
        tracing::warn!(
            target: "app.fonts",
            requested_family = UI_FONT_FAMILY,
            configured_candidates = ?UI_FALLBACK_FAMILIES,
            "ui shell fallback policy could not find any configured system fallback families"
        );
        return;
    }

    for generic_family in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::SystemUi,
        fontique::GenericFamily::UiSansSerif,
    ] {
        collection.set_generic_families(generic_family, fallback_ids.iter().copied());
    }

    let resolved_fallbacks = fallback_ids
        .iter()
        .filter_map(|id| collection.family_name(*id).map(str::to_string))
        .collect::<Vec<_>>();

    tracing::info!(
        target: "app.fonts",
        requested_family = UI_FONT_FAMILY,
        configured_fallback_families = ?resolved_fallbacks,
        "configured ui shell font fallback policy"
    );
}

pub(crate) fn log_ui_shell_font_diagnostics() {
    let mut collection = shared_collection();
    let requested_weight = UI_FONT_DEFAULT_WEIGHT;
    let requested_style = fontique::FontStyle::Normal;

    let latin = resolve_ui_probe(
        &mut collection,
        UI_FONT_FAMILY,
        requested_weight,
        requested_style,
        "A",
    );
    let cjk = resolve_ui_probe(
        &mut collection,
        UI_FONT_FAMILY,
        requested_weight,
        requested_style,
        "界",
    );
    let icon = resolve_ui_probe(
        &mut collection,
        UI_FONT_FAMILY,
        requested_weight,
        requested_style,
        "⚙",
    )
    .or_else(|| {
        resolve_system_face_diagnostic(
            UI_FONT_FAMILY,
            "Segoe UI Symbol",
            UI_SYMBOL_FALLBACK_FAMILIES,
        )
    });
    let emoji = resolve_ui_probe(
        &mut collection,
        UI_FONT_FAMILY,
        requested_weight,
        requested_style,
        "🙂",
    )
    .or_else(|| {
        resolve_system_face_diagnostic(
            UI_FONT_FAMILY,
            TERMINAL_EMOJI_FALLBACK_FAMILY,
            UI_EMOJI_FALLBACK_FAMILIES,
        )
    });

    let requested_match = latin.clone().unwrap_or_default();
    let shell_probe_matches = [latin.as_ref(), cjk.as_ref()];
    let fallback_family = shell_probe_matches
        .iter()
        .copied()
        .flatten()
        .find(|diagnostic| diagnostic.resolved_family != UI_FONT_FAMILY)
        .map(|diagnostic| diagnostic.resolved_family.clone());
    let resolved_families = shell_probe_matches
        .iter()
        .copied()
        .flatten()
        .map(|diagnostic| diagnostic.resolved_family.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let mixed_ui_families = resolved_families.len() > 1;

    tracing::info!(
        target: "app.fonts",
        requested_family = UI_FONT_FAMILY,
        resolved_family = requested_match.resolved_family,
        fallback_family = fallback_family.as_deref().unwrap_or("none"),
        requested_weight,
        resolved_weight = requested_match.weight,
        requested_style = "normal",
        resolved_style = requested_match.style,
        source = requested_match.source,
        latin_family = latin
            .as_ref()
            .map(|diagnostic| diagnostic.resolved_family.as_str())
            .unwrap_or("unresolved"),
        cjk_family = cjk
            .as_ref()
            .map(|diagnostic| diagnostic.resolved_family.as_str())
            .unwrap_or("unresolved"),
        emoji_family = emoji
            .as_ref()
            .map(|diagnostic| diagnostic.resolved_family.as_str())
            .unwrap_or("unresolved"),
        icon_family = icon
            .as_ref()
            .map(|diagnostic| diagnostic.resolved_family.as_str())
            .unwrap_or("unresolved"),
        mixed_ui_families,
        "ui shell font resolution established"
    );

    if requested_match.resolved_family != UI_FONT_FAMILY || mixed_ui_families {
        tracing::warn!(
            target: "app.fonts",
            requested_family = UI_FONT_FAMILY,
            resolved_family = requested_match.resolved_family,
            fallback_family = fallback_family.as_deref().unwrap_or("none"),
            mixed_ui_families,
            "ui shell font resolution is not locked to the requested family"
        );
    }
}

pub(crate) fn bitmap_terminal_font_resolution_snapshot() -> TerminalFontResolutionSnapshot {
    let primary = bundled_font_match_diagnostic(
        TERMINAL_PRIMARY_FAMILY,
        TERMINAL_PRIMARY_FAMILY,
        "bundled",
        BUNDLED_SARASA_TERM_SC_FONT_BYTES,
        0,
    );
    let emoji = resolve_system_face_diagnostic(
        TERMINAL_PRIMARY_FAMILY,
        TERMINAL_EMOJI_FALLBACK_FAMILY,
        &[TERMINAL_EMOJI_FALLBACK_FAMILY],
    )
    .unwrap_or_else(|| {
        unresolved_face_diagnostic(TERMINAL_PRIMARY_FAMILY, TERMINAL_EMOJI_FALLBACK_FAMILY)
    });

    TerminalFontResolutionSnapshot {
        requested_family: TERMINAL_PRIMARY_FAMILY.to_string(),
        requested_weight: DEFAULT_TERMINAL_FONT_WEIGHT.to_string(),
        requested_style: "normal".to_string(),
        primary: primary.clone(),
        cjk: primary.clone(),
        symbol: primary.clone(),
        icon: primary,
        emoji,
        mixes_multiple_unrelated_families: false,
    }
}

pub(crate) fn log_terminal_font_diagnostics(
    renderer_path: &str,
    requested_size_px: f32,
    snapshot: &TerminalFontResolutionSnapshot,
) {
    tracing::info!(
        target: "app.fonts",
        renderer_path,
        requested_family = snapshot.requested_family.as_str(),
        requested_weight = snapshot.requested_weight.as_str(),
        requested_style = snapshot.requested_style.as_str(),
        requested_size_px,
        resolved_primary_family = snapshot.primary.resolved_family.as_str(),
        resolved_primary_fallback_family = snapshot.primary.fallback_family.as_deref().unwrap_or("none"),
        resolved_primary_post_script_name = snapshot.primary.post_script_name.as_deref().unwrap_or("unknown"),
        resolved_primary_weight = snapshot.primary.weight.as_str(),
        resolved_primary_style = snapshot.primary.style.as_str(),
        resolved_primary_source = snapshot.primary.source.as_str(),
        cjk_family = snapshot.cjk.resolved_family.as_str(),
        cjk_fallback_family = snapshot.cjk.fallback_family.as_deref().unwrap_or("none"),
        cjk_post_script_name = snapshot.cjk.post_script_name.as_deref().unwrap_or("unknown"),
        cjk_weight = snapshot.cjk.weight.as_str(),
        cjk_style = snapshot.cjk.style.as_str(),
        cjk_source = snapshot.cjk.source.as_str(),
        symbol_family = snapshot.symbol.resolved_family.as_str(),
        symbol_fallback_family = snapshot.symbol.fallback_family.as_deref().unwrap_or("none"),
        symbol_post_script_name = snapshot.symbol.post_script_name.as_deref().unwrap_or("unknown"),
        symbol_weight = snapshot.symbol.weight.as_str(),
        symbol_style = snapshot.symbol.style.as_str(),
        symbol_source = snapshot.symbol.source.as_str(),
        icon_family = snapshot.icon.resolved_family.as_str(),
        icon_fallback_family = snapshot.icon.fallback_family.as_deref().unwrap_or("none"),
        icon_post_script_name = snapshot.icon.post_script_name.as_deref().unwrap_or("unknown"),
        icon_weight = snapshot.icon.weight.as_str(),
        icon_style = snapshot.icon.style.as_str(),
        icon_source = snapshot.icon.source.as_str(),
        emoji_family = snapshot.emoji.resolved_family.as_str(),
        emoji_fallback_family = snapshot.emoji.fallback_family.as_deref().unwrap_or("none"),
        emoji_post_script_name = snapshot.emoji.post_script_name.as_deref().unwrap_or("unknown"),
        emoji_weight = snapshot.emoji.weight.as_str(),
        emoji_style = snapshot.emoji.style.as_str(),
        emoji_source = snapshot.emoji.source.as_str(),
        mixes_multiple_unrelated_families = snapshot.mixes_multiple_unrelated_families,
        "terminal font resolution established"
    );

    if snapshot.primary.resolved_family != snapshot.requested_family
        || snapshot.mixes_multiple_unrelated_families
    {
        tracing::warn!(
            target: "app.fonts",
            renderer_path,
            requested_family = snapshot.requested_family.as_str(),
            resolved_primary_family = snapshot.primary.resolved_family.as_str(),
            symbol_family = snapshot.symbol.resolved_family.as_str(),
            icon_family = snapshot.icon.resolved_family.as_str(),
            emoji_family = snapshot.emoji.resolved_family.as_str(),
            mixes_multiple_unrelated_families = snapshot.mixes_multiple_unrelated_families,
            "terminal font resolution drifted away from the requested primary family"
        );
    }

    if snapshot.icon.resolved_family != snapshot.requested_family
        && !TERMINAL_NERD_FALLBACK_FAMILIES.iter().any(|candidate| {
            snapshot
                .icon
                .resolved_family
                .eq_ignore_ascii_case(candidate)
        })
    {
        tracing::warn!(
            target: "app.fonts",
            renderer_path,
            private_use_probe = "",
            requested_family = snapshot.requested_family.as_str(),
            resolved_icon_family = snapshot.icon.resolved_family.as_str(),
            resolved_icon_source = snapshot.icon.source.as_str(),
            "terminal private-use icon fallback is not using a Nerd Font family; prompt icons may still be missing on this machine"
        );
    }
}

pub(crate) fn terminal_chain_uses_unexpected_mix(chain: &[String]) -> bool {
    let unexpected = chain
        .iter()
        .filter(|family_name| !terminal_family_is_expected(family_name.as_str()))
        .count();
    let primary_like = chain
        .iter()
        .filter(|family_name| family_name.eq_ignore_ascii_case(TERMINAL_PRIMARY_FAMILY))
        .count();

    unexpected > 0 || (primary_like == 0 && !chain.is_empty())
}

pub(crate) fn bundled_font_match_diagnostic(
    requested_family: &str,
    resolved_family: &str,
    source: &str,
    font_bytes: &[u8],
    face_index: u32,
) -> FontFaceMatchDiagnostic {
    let parsed = parse_face_metadata(font_bytes, face_index);
    FontFaceMatchDiagnostic {
        requested_family: requested_family.to_string(),
        resolved_family: resolved_family.to_string(),
        fallback_family: (resolved_family != requested_family).then(|| resolved_family.to_string()),
        post_script_name: parsed.post_script_name,
        weight: parsed
            .weight
            .map(|value| value.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        style: parsed.style.unwrap_or_else(|| "normal".to_string()),
        source: source.to_string(),
    }
}

pub(crate) fn unresolved_face_diagnostic(
    requested_family: &str,
    resolved_family: &str,
) -> FontFaceMatchDiagnostic {
    FontFaceMatchDiagnostic {
        requested_family: requested_family.to_string(),
        resolved_family: resolved_family.to_string(),
        fallback_family: (resolved_family != requested_family).then(|| resolved_family.to_string()),
        post_script_name: None,
        weight: "unknown".to_string(),
        style: "normal".to_string(),
        source: "unresolved".to_string(),
    }
}

pub(crate) fn resolve_system_face_diagnostic(
    requested_family: &str,
    preferred_family: &str,
    candidates: &[&str],
) -> Option<FontFaceMatchDiagnostic> {
    let mut database = load_system_font_database();
    let resolved_family = candidates
        .iter()
        .find_map(|candidate| resolve_system_family_name(&mut database, candidate))?;
    let face = resolve_system_face(&mut database, resolved_family.as_str())?;
    Some(FontFaceMatchDiagnostic {
        requested_family: requested_family.to_string(),
        resolved_family: face
            .families
            .first()
            .map(|(family_name, _)| family_name.clone())
            .unwrap_or_else(|| preferred_family.to_string()),
        fallback_family: (resolved_family != requested_family).then_some(resolved_family),
        post_script_name: Some(face.post_script_name.clone()),
        weight: face.weight.0.to_string(),
        style: fontdb_style_label(face.style).to_string(),
        source: system_source_label(&face.source),
    })
}

fn resolve_ui_probe(
    collection: &mut fontique::Collection,
    requested_family: &str,
    requested_weight: i32,
    requested_style: fontique::FontStyle,
    text: &str,
) -> Option<FontFaceMatchDiagnostic> {
    let mut source_cache = fontique::SourceCache::new_shared();

    if let Some(diagnostic) = resolve_collection_candidate(
        collection,
        &mut source_cache,
        fontique::QueryFamily::Named(requested_family),
        requested_family,
        requested_weight,
        requested_style,
        text,
    ) {
        return Some(diagnostic);
    }

    for generic_family in [
        fontique::GenericFamily::SansSerif,
        fontique::GenericFamily::SystemUi,
        fontique::GenericFamily::UiSansSerif,
    ] {
        if let Some(diagnostic) = resolve_collection_candidate(
            collection,
            &mut source_cache,
            fontique::QueryFamily::Generic(generic_family),
            requested_family,
            requested_weight,
            requested_style,
            text,
        ) {
            return Some(diagnostic);
        }
    }

    None
}

fn resolve_collection_candidate(
    collection: &mut fontique::Collection,
    source_cache: &mut fontique::SourceCache,
    family: fontique::QueryFamily<'_>,
    requested_family: &str,
    requested_weight: i32,
    requested_style: fontique::FontStyle,
    text: &str,
) -> Option<FontFaceMatchDiagnostic> {
    let (matched_family_id, matched_font_index, matched) = {
        let mut query = collection.query(source_cache);
        query.set_families(core::iter::once(family));
        query.set_attributes(fontique::Attributes {
            weight: fontique::FontWeight::new(requested_weight as f32),
            style: requested_style,
            ..Default::default()
        });

        let mut matched = None;
        query.matches_with(|query_font| {
            if query_font_supports_text(query_font, text) {
                matched = Some((query_font.family.0, query_font.family.1, query_font.clone()));
                fontique::QueryStatus::Stop
            } else {
                fontique::QueryStatus::Continue
            }
        });
        matched?
    };

    let family_info = collection.family(matched_family_id)?;
    let font_info = family_info.fonts().get(matched_font_index)?;
    let parsed = parse_face_metadata(matched.blob.as_ref(), matched.index);
    let resolved_family = family_info.name().to_string();

    Some(FontFaceMatchDiagnostic {
        requested_family: requested_family.to_string(),
        resolved_family: resolved_family.clone(),
        fallback_family: (!resolved_family.eq_ignore_ascii_case(requested_family))
            .then_some(resolved_family),
        post_script_name: parsed.post_script_name,
        weight: parsed
            .weight
            .map(|value| value.to_string())
            .unwrap_or_else(|| (font_info.weight().value().round() as i32).to_string()),
        style: parsed
            .style
            .unwrap_or_else(|| fontique_style_label(font_info.style()).to_string()),
        source: fontique_source_label(font_info.source()),
    })
}

fn query_font_supports_text(query_font: &fontique::QueryFont, text: &str) -> bool {
    let Some(charmap) = query_font.charmap() else {
        return false;
    };

    text.chars()
        .all(|ch| is_ignorable_char(ch) || charmap.map(ch).is_some())
}

fn is_ignorable_char(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r' | '\t' | '\u{200c}' | '\u{200d}' | '\u{fe0e}' | '\u{fe0f}'
    )
}

fn parse_face_metadata(font_data: &[u8], face_index: u32) -> ParsedFaceMetadata {
    let mut database = Database::new();
    let ids = database.load_font_source(fontdb::Source::Binary(std::sync::Arc::new(
        font_data.to_vec(),
    )));
    let Some(face_id) = ids.get(face_index as usize) else {
        return ParsedFaceMetadata::default();
    };
    let Some(face) = database.face(*face_id) else {
        return ParsedFaceMetadata::default();
    };

    ParsedFaceMetadata {
        post_script_name: Some(face.post_script_name.clone()),
        weight: Some(face.weight.0),
        style: Some(fontdb_style_label(face.style).to_string()),
    }
}

fn resolve_system_family_name(database: &mut Database, family_name: &str) -> Option<String> {
    let families = [fontdb::Family::Name(family_name)];
    let face_id = database.query(&Query {
        families: &families,
        weight: Weight::NORMAL,
        stretch: Stretch::Normal,
        style: Style::Normal,
    })?;
    let face = database.face(face_id)?;
    face.families.first().map(|(name, _)| name.clone())
}

fn resolve_system_face<'a>(
    database: &'a mut Database,
    family_name: &str,
) -> Option<&'a fontdb::FaceInfo> {
    let families = [fontdb::Family::Name(family_name)];
    let face_id = database.query(&Query {
        families: &families,
        weight: Weight::NORMAL,
        stretch: Stretch::Normal,
        style: Style::Normal,
    })?;
    database.face(face_id)
}

fn fontdb_style_label(style: Style) -> &'static str {
    match style {
        Style::Normal => "normal",
        Style::Italic => "italic",
        Style::Oblique => "oblique",
    }
}

fn fontique_style_label(style: fontique::FontStyle) -> &'static str {
    match style {
        fontique::FontStyle::Normal => "normal",
        fontique::FontStyle::Italic => "italic",
        fontique::FontStyle::Oblique(_) => "oblique",
    }
}

fn fontique_source_label(source: &fontique::SourceInfo) -> String {
    match source.kind() {
        fontique::SourceKind::Memory(_) => "custom-registration".to_string(),
        fontique::SourceKind::Path(path) => format!("system:{}", path.display()),
    }
}

fn system_source_label(source: &fontdb::Source) -> String {
    match source {
        fontdb::Source::Binary(_) => "bundled".to_string(),
        fontdb::Source::File(path) => format!("system:{}", path.display()),
        fontdb::Source::SharedFile(path, _) => format!("system:{}", path.display()),
    }
}

fn terminal_family_is_expected(family_name: &str) -> bool {
    family_name.eq_ignore_ascii_case(TERMINAL_PRIMARY_FAMILY)
        || family_name.starts_with("Sarasa")
        || family_name.eq_ignore_ascii_case(TERMINAL_EMOJI_FALLBACK_FAMILY)
        || TERMINAL_NERD_FALLBACK_FAMILIES
            .iter()
            .any(|candidate| family_name.eq_ignore_ascii_case(candidate))
        || TERMINAL_SYMBOL_FALLBACK_FAMILIES
            .iter()
            .any(|candidate| family_name.eq_ignore_ascii_case(candidate))
        || TERMINAL_ICON_FALLBACK_FAMILIES
            .iter()
            .any(|candidate| family_name.eq_ignore_ascii_case(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_bitmap_terminal_snapshot_stays_on_sarasa_for_non_emoji_text() {
        let snapshot = bitmap_terminal_font_resolution_snapshot();

        assert_eq!(snapshot.primary.resolved_family, TERMINAL_PRIMARY_FAMILY);
        assert_eq!(snapshot.cjk.resolved_family, TERMINAL_PRIMARY_FAMILY);
        assert_eq!(snapshot.symbol.resolved_family, TERMINAL_PRIMARY_FAMILY);
        assert_eq!(snapshot.icon.resolved_family, TERMINAL_PRIMARY_FAMILY);
    }
}
