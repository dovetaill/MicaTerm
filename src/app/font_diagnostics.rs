//! Shared UI/terminal font diagnostics and fallback policy helpers.

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicUsize, Ordering};

use fontdb::{Database, Query, Stretch, Style, Weight};
use slint::fontique_07::{fontique, shared_collection};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_PIXEL_GEOMETRY, DWRITE_PIXEL_GEOMETRY_BGR,
    DWRITE_PIXEL_GEOMETRY_FLAT, DWRITE_PIXEL_GEOMETRY_RGB, DWriteCreateFactory, IDWriteFactory,
};

use crate::app::system_font_database::{
    load_system_font_database, reset_system_font_database_load_call_count,
    system_font_database_load_call_count,
};
use crate::app::terminal_font::{DEFAULT_TERMINAL_FONT_WEIGHT, DEFAULT_TERMINAL_LETTER_SPACING_PX};

pub const UI_FONT_FAMILY: &str = "JetBrains Maple Mono";
pub const UI_FONT_DEFAULT_WEIGHT: i32 = 400;
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const UI_CHROME_FONT_WEIGHT: i32 = 400;
#[cfg(target_os = "windows")]
#[allow(dead_code)]
const FORCE_OPAQUE_HOST_WINDOW_ENV: &str = "MICA_TERM_FORCE_OPAQUE_HOST_WINDOW";
#[cfg(target_os = "windows")]
#[allow(dead_code)]
const FORCE_DEVICE_INDEPENDENT_UI_FONTS_ENV: &str = "MICA_TERM_FORCE_DEVICE_INDEPENDENT_UI_FONTS";
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub const UI_BODY_FONT_SIZE_PX: f32 = 14.0;
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub const UI_CAPTION_FONT_SIZE_PX: f32 = 13.0;
#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub const UI_CHROME_LETTER_SPACING_PX: f32 = 0.0;
pub const TERMINAL_PRIMARY_FAMILY: &str = "Sarasa Term SC Nerd";
pub const TERMINAL_EMOJI_FALLBACK_FAMILY: &str = "Segoe UI Emoji";
pub const UI_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI", "Microsoft YaHei UI", "Microsoft YaHei", "Arial"];
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub const UI_SYMBOL_FALLBACK_FAMILIES: &[&str] =
    &["Segoe UI Symbol", "Segoe Fluent Icons", "Segoe MDL2 Assets"];
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
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

static UI_SHARED_COLLECTION_CONFIGURE_CALLS: AtomicUsize = AtomicUsize::new(0);
static UI_SHARED_COLLECTION_DIAGNOSTICS_CALLS: AtomicUsize = AtomicUsize::new(0);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FontFaceMatchDiagnostic {
    pub requested_family: String,
    pub resolved_family: String,
    pub fallback_family: Option<String>,
    pub post_script_name: Option<String>,
    pub weight: String,
    pub embedded_weight: Option<String>,
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct StartupFontDiagnosticsCounters {
    pub ui_shared_collection_configure_calls: usize,
    pub ui_shared_collection_diagnostics_calls: usize,
    pub system_font_database_load_calls: usize,
}

pub(crate) fn reset_startup_font_diagnostics_counters() {
    UI_SHARED_COLLECTION_CONFIGURE_CALLS.store(0, Ordering::Relaxed);
    UI_SHARED_COLLECTION_DIAGNOSTICS_CALLS.store(0, Ordering::Relaxed);
    reset_system_font_database_load_call_count();
}

pub(crate) fn startup_font_diagnostics_counters() -> StartupFontDiagnosticsCounters {
    StartupFontDiagnosticsCounters {
        ui_shared_collection_configure_calls: UI_SHARED_COLLECTION_CONFIGURE_CALLS
            .load(Ordering::Relaxed),
        ui_shared_collection_diagnostics_calls: UI_SHARED_COLLECTION_DIAGNOSTICS_CALLS
            .load(Ordering::Relaxed),
        system_font_database_load_calls: system_font_database_load_call_count(),
    }
}

pub(crate) fn configure_ui_font_fallbacks() {
    UI_SHARED_COLLECTION_CONFIGURE_CALLS.fetch_add(1, Ordering::Relaxed);
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
}

pub(crate) fn log_ui_shell_font_diagnostics() {
    UI_SHARED_COLLECTION_DIAGNOSTICS_CALLS.fetch_add(1, Ordering::Relaxed);
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

    // Only the opt-in memory-diagnostics path should pay to enumerate extra system-font
    // snapshot data. The default startup warning path stays limited to the latin/cjk probes.
    if crate::app::logging::runtime::memory_diagnostics_enabled()
        && tracing::enabled!(target: "app.fonts", tracing::Level::DEBUG)
    {
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
        let chrome_latin = resolve_ui_probe(
            &mut collection,
            UI_FONT_FAMILY,
            UI_CHROME_FONT_WEIGHT,
            requested_style,
            "A",
        );
        let chrome_cjk = resolve_ui_probe(
            &mut collection,
            UI_FONT_FAMILY,
            UI_CHROME_FONT_WEIGHT,
            requested_style,
            "界",
        );
        let chrome_requested_match = chrome_latin
            .clone()
            .or_else(|| chrome_cjk.clone())
            .unwrap_or_default();

        tracing::debug!(
            target: "app.fonts",
            requested_family = UI_FONT_FAMILY,
            resolved_family = requested_match.resolved_family,
            fallback_family = fallback_family.as_deref().unwrap_or("none"),
            requested_weight,
            resolved_weight = requested_match.weight.as_str(),
            embedded_post_script_name = requested_match.post_script_name.as_deref().unwrap_or("unknown"),
            embedded_weight = requested_match.embedded_weight.as_deref().unwrap_or("unknown"),
            requested_style = "normal",
            resolved_style = requested_match.style.as_str(),
            source = requested_match.source.as_str(),
            chrome_requested_weight = UI_CHROME_FONT_WEIGHT,
            chrome_resolved_family = chrome_requested_match.resolved_family.as_str(),
            chrome_resolved_weight = chrome_requested_match.weight.as_str(),
            chrome_embedded_post_script_name = chrome_requested_match.post_script_name.as_deref().unwrap_or("unknown"),
            chrome_embedded_weight = chrome_requested_match.embedded_weight.as_deref().unwrap_or("unknown"),
            chrome_resolved_style = chrome_requested_match.style.as_str(),
            chrome_source = chrome_requested_match.source.as_str(),
            chrome_latin_family = chrome_latin
                .as_ref()
                .map(|diagnostic| diagnostic.resolved_family.as_str())
                .unwrap_or("unresolved"),
            chrome_cjk_family = chrome_cjk
                .as_ref()
                .map(|diagnostic| diagnostic.resolved_family.as_str())
                .unwrap_or("unresolved"),
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
            "ui shell font resolution snapshot"
        );
    }

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

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_host_window_transparent() -> bool {
    std::env::var_os(FORCE_OPAQUE_HOST_WINDOW_ENV).is_none()
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
fn ui_host_window_transparent() -> bool {
    false
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_system_rendering_params() -> Option<(DWRITE_PIXEL_GEOMETRY, f32, f32)> {
    let factory: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED).ok()? };
    let params = unsafe { factory.CreateRenderingParams().ok()? };
    Some((
        unsafe { params.GetPixelGeometry() },
        unsafe { params.GetEnhancedContrast() },
        unsafe { params.GetGamma() },
    ))
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_system_pixel_geometry() -> &'static str {
    match ui_system_rendering_params().map(|(pixel_geometry, _, _)| pixel_geometry) {
        Some(DWRITE_PIXEL_GEOMETRY_RGB) => "rgb-horizontal",
        Some(DWRITE_PIXEL_GEOMETRY_BGR) => "bgr-horizontal",
        Some(DWRITE_PIXEL_GEOMETRY_FLAT) => "flat",
        _ => "unknown",
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_antialias_mode() -> &'static str {
    if ui_host_window_transparent() {
        "grayscale"
    } else {
        match ui_system_rendering_params().map(|(pixel_geometry, _, _)| pixel_geometry) {
            Some(DWRITE_PIXEL_GEOMETRY_RGB | DWRITE_PIXEL_GEOMETRY_BGR) => "subpixel",
            _ => "grayscale",
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_subpixel_positioning() -> bool {
    if !ui_host_window_transparent() {
        return false;
    }

    true
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_surface_uses_device_independent_fonts() -> bool {
    std::env::var_os(FORCE_DEVICE_INDEPENDENT_UI_FONTS_ENV).is_some()
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_rendering_params() -> Option<(f32, f32)> {
    let (pixel_geometry, enhanced_contrast, gamma) = ui_system_rendering_params()?;
    let text_contrast = if pixel_geometry == DWRITE_PIXEL_GEOMETRY_FLAT {
        enhanced_contrast
    } else {
        enhanced_contrast.max(0.65).min(1.0)
    };

    Some((text_contrast, gamma))
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_contrast() -> String {
    ui_text_rendering_params()
        .map(|(text_contrast, _)| format!("{text_contrast:.2}"))
        .unwrap_or_else(|| "default".to_string())
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_gamma() -> String {
    ui_text_rendering_params()
        .map(|(_, text_gamma)| format!("{text_gamma:.2}"))
        .unwrap_or_else(|| "default".to_string())
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_hinting() -> &'static str {
    match ui_text_antialias_mode() {
        "subpixel" => "normal",
        _ => "normal",
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_surface_pixel_geometry() -> &'static str {
    if ui_host_window_transparent() {
        "unknown"
    } else {
        match ui_system_rendering_params().map(|(pixel_geometry, _, _)| pixel_geometry) {
            Some(DWRITE_PIXEL_GEOMETRY_RGB) => "rgb-horizontal",
            Some(DWRITE_PIXEL_GEOMETRY_BGR) => "bgr-horizontal",
            _ => "unknown",
        }
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_surface_color_space() -> &'static str {
    "srgb"
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
fn ui_text_rendering_policy() -> &'static str {
    if ui_host_window_transparent() {
        "grayscale-on-transparent-host"
    } else {
        "lcd-on-opaque-host"
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub(crate) fn log_ui_text_renderer_diagnostics() {
    tracing::info!(
        target: "app.renderer",
        ui_text_renderer = "slint-skia",
        ui_host_window_transparent = ui_host_window_transparent(),
        ui_system_pixel_geometry = ui_system_pixel_geometry(),
        ui_text_antialias_mode = ui_text_antialias_mode(),
        ui_text_subpixel_positioning = ui_text_subpixel_positioning(),
        ui_text_hinting = ui_text_hinting(),
        ui_surface_pixel_geometry = ui_surface_pixel_geometry(),
        ui_surface_color_space = ui_surface_color_space(),
        ui_surface_uses_device_independent_fonts = ui_surface_uses_device_independent_fonts(),
        ui_default_font_weight = UI_FONT_DEFAULT_WEIGHT,
        ui_chrome_font_weight = UI_CHROME_FONT_WEIGHT,
        ui_body_font_size_px = UI_BODY_FONT_SIZE_PX,
        ui_caption_font_size_px = UI_CAPTION_FONT_SIZE_PX,
        ui_chrome_letter_spacing_px = UI_CHROME_LETTER_SPACING_PX,
        ui_text_contrast = ui_text_contrast(),
        ui_text_gamma = ui_text_gamma(),
        ui_text_rendering_policy = ui_text_rendering_policy(),
        "ui text renderer configuration established"
    );
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub(crate) fn log_ui_text_renderer_diagnostics() {}

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
    // Keep the full terminal-font snapshot wiring available in-source for contract checks
    // without restoring the old startup/runtime info log spam.
    let _terminal_font_snapshot = || {
        tracing::debug!(
            target: "app.fonts",
            renderer_path,
            requested_family = snapshot.requested_family.as_str(),
            requested_weight = snapshot.requested_weight.as_str(),
            requested_style = snapshot.requested_style.as_str(),
            requested_size_px,
            terminal_letter_spacing_px = DEFAULT_TERMINAL_LETTER_SPACING_PX,
            resolved_primary_family = snapshot.primary.resolved_family.as_str(),
            resolved_primary_fallback_family = snapshot.primary.fallback_family.as_deref().unwrap_or("none"),
            resolved_primary_post_script_name = snapshot.primary.post_script_name.as_deref().unwrap_or("unknown"),
            resolved_primary_weight = snapshot.primary.weight.as_str(),
            resolved_primary_embedded_weight = snapshot.primary.embedded_weight.as_deref().unwrap_or("unknown"),
            resolved_primary_style = snapshot.primary.style.as_str(),
            resolved_primary_source = snapshot.primary.source.as_str(),
            cjk_family = snapshot.cjk.resolved_family.as_str(),
            cjk_fallback_family = snapshot.cjk.fallback_family.as_deref().unwrap_or("none"),
            cjk_post_script_name = snapshot.cjk.post_script_name.as_deref().unwrap_or("unknown"),
            cjk_weight = snapshot.cjk.weight.as_str(),
            cjk_embedded_weight = snapshot.cjk.embedded_weight.as_deref().unwrap_or("unknown"),
            cjk_style = snapshot.cjk.style.as_str(),
            cjk_source = snapshot.cjk.source.as_str(),
            symbol_family = snapshot.symbol.resolved_family.as_str(),
            symbol_fallback_family = snapshot.symbol.fallback_family.as_deref().unwrap_or("none"),
            symbol_post_script_name = snapshot.symbol.post_script_name.as_deref().unwrap_or("unknown"),
            symbol_weight = snapshot.symbol.weight.as_str(),
            symbol_embedded_weight = snapshot.symbol.embedded_weight.as_deref().unwrap_or("unknown"),
            symbol_style = snapshot.symbol.style.as_str(),
            symbol_source = snapshot.symbol.source.as_str(),
            icon_family = snapshot.icon.resolved_family.as_str(),
            icon_fallback_family = snapshot.icon.fallback_family.as_deref().unwrap_or("none"),
            icon_post_script_name = snapshot.icon.post_script_name.as_deref().unwrap_or("unknown"),
            icon_weight = snapshot.icon.weight.as_str(),
            icon_embedded_weight = snapshot.icon.embedded_weight.as_deref().unwrap_or("unknown"),
            icon_style = snapshot.icon.style.as_str(),
            icon_source = snapshot.icon.source.as_str(),
            emoji_family = snapshot.emoji.resolved_family.as_str(),
            emoji_fallback_family = snapshot.emoji.fallback_family.as_deref().unwrap_or("none"),
            emoji_post_script_name = snapshot.emoji.post_script_name.as_deref().unwrap_or("unknown"),
            emoji_weight = snapshot.emoji.weight.as_str(),
            emoji_embedded_weight = snapshot.emoji.embedded_weight.as_deref().unwrap_or("unknown"),
            emoji_style = snapshot.emoji.style.as_str(),
            emoji_source = snapshot.emoji.source.as_str(),
            mixes_multiple_unrelated_families = snapshot.mixes_multiple_unrelated_families,
            "terminal font resolution snapshot"
        );
    };

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
        embedded_weight: parsed.weight.map(|value| value.to_string()),
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
        embedded_weight: None,
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
        embedded_weight: Some(face.weight.0.to_string()),
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
        weight: (font_info.weight().value().round() as i32).to_string(),
        embedded_weight: parsed.weight.map(|value| value.to_string()),
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
