#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;

use std::fs;

#[test]
fn backend_source_exposes_shared_terminal_typography_defaults() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let font_mod_source =
        fs::read_to_string("src/app/terminal_font/mod.rs").expect("read font mod");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");

    assert!(
        backend_source
            .contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"Sarasa Term SC Nerd\";"),
        "backend should set Sarasa Term SC Nerd as the shared terminal default family"
    );
    assert!(
        !backend_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_FAMILY"),
        "backend should not introduce a second Windows-only terminal family constant while unifying the shared terminal contract"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 14.0;"),
        "backend should expose a 14px default terminal font size"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_LINE_HEIGHT: f32 = 1.5;"),
        "backend should expose a slightly looser 1.5 default line height for denser CJK readability"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 16.0;"),
        "backend should expose a Windows-only 16px terminal font size override so the native presenter lands a full notch larger without changing Linux/macOS defaults"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX: u32 = 24;")
            && backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT: f32 =")
            && backend_source.contains(
                "WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX as f32 / WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX;"
            ),
        "backend should keep the Windows line-height contract explicit from a 24px cell box so the larger 16px Semibold body text stays readable without drifting into a loose terminal"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_LETTER_SPACING_PX: f32 = 1.0;"),
        "backend should keep an explicit slightly tightened terminal letter spacing so dense Windows prompt output stays readable without drifting back into over-tracked text"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"SemiBold\";"),
        "backend should keep the terminal request aligned with the packaged SemiBold face"
    );
    assert!(
        backend_source.contains(
            "pub const DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY: &str = DEFAULT_TERMINAL_FONT_FAMILY;"
        ),
        "backend should collapse the CJK fallback family into the shared Sarasa terminal family"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[")
            && backend_source.contains("DEFAULT_TERMINAL_FONT_FAMILY")
            && backend_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY"),
        "backend should expose a Windows terminal font chain rooted in the shared Sarasa family plus emoji fallback"
    );
    assert!(
        backend_source.contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())")
            && backend_source.contains("px_size: DEFAULT_TERMINAL_FONT_SIZE_PX,"),
        "default font requests should flow through the shared typography constants"
    );
    assert!(
        backend_source.contains("pub fn windows_default() -> Self")
            && backend_source
                .contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())")
            && backend_source.contains("px_size: WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX,"),
        "font requests should expose a Windows-specific default constructor so only the Windows presenters pick up the larger body size"
    );
    assert!(
        font_mod_source.contains("DEFAULT_TERMINAL_FONT_SIZE_PX")
            && font_mod_source.contains("DEFAULT_TERMINAL_LINE_HEIGHT")
            && font_mod_source.contains("WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX")
            && font_mod_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX")
            && font_mod_source.contains("WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT")
            && font_mod_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN"),
        "terminal font module should re-export the shared typography defaults"
    );
    assert!(
        fallback_source.contains("contains_cjk_text(text)")
            && fallback_source.contains("return primary_family.to_string();")
            && (fallback_source.contains("\"Segoe UI Emoji\"")
                || fallback_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY")),
        "Windows fallback source should keep CJK text on the shared primary family while retaining emoji fallback"
    );
    assert!(
        fallback_source.contains("'\\u{ff00}'..='\\u{ffef}'"),
        "Windows fallback source should treat fullwidth punctuation as part of the CJK fallback range so Chinese punctuation does not fall back to tofu squares"
    );
    assert!(
        presenter_source.contains("let request = FontRequest::windows_default();"),
        "Windows presenters should source their default request from the shared typography contract"
    );
}

#[test]
fn windows_dwrite_source_keeps_current_native_loader_contract() {
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite");

    assert!(
        dwrite_source.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf")
            && dwrite_source
                .contains("post_script_name: \"SarasaTermSCNerd-SemiBold\".to_string()"),
        "DirectWrite fallback should use the bundled Sarasa Term SC Nerd face as the primary packaged terminal font"
    );
    assert!(
        !dwrite_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Regular.ttf")
            && !dwrite_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"),
        "Windows DirectWrite body text should stop loading JetBrains Mono once Sarasa owns the primary terminal font contract"
    );
    assert!(
        dwrite_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX")
            && dwrite_source.contains("WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT")
            && dwrite_source.contains("let cell_height_px = line_height.max(MIN_CELL_HEIGHT_PX);"),
        "DirectWrite metrics should keep the native line box aligned with the Windows-specific minimum readability floor so the live and retired Windows paths never diverge on dense text spacing"
    );
    assert!(
        !dwrite_source.contains(&retired_windows_subsystem::retired_font_loader_name()),
        "DirectWrite defaults should stop exposing a retired Windows font-loading entrypoint once retained-native is the only supported Windows path"
    );
    assert!(
        dwrite_source.contains("fallback_face_data_for_family(family_name)")
            && dwrite_source
                .contains(".or_else(|| self.ensure_locator().resolve_face_data(family_name))"),
        "DirectWrite font loading should prefer bundled face data before consulting the system locator so shaping, rasterization, and DrawGlyphRun stay on the same font bytes and never decode ASCII glyph ids against a different Cascadia build"
    );
    assert!(
        !dwrite_source.contains("post_script_name: \"JetBrainsMono-Regular\".to_string()"),
        "Windows DirectWrite fallback metadata should stop advertising JetBrains Mono once Sarasa owns the primary terminal font contract"
    );
}
