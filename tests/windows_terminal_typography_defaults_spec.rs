use std::fs;

#[test]
fn backend_source_exposes_windows_terminal_typography_defaults() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let font_mod_source =
        fs::read_to_string("src/app/terminal_font/mod.rs").expect("read font mod");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");
    let dwrite_source =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite");

    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"Cascadia Mono\";"),
        "backend should set Cascadia Mono as the shared terminal default family"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 14.0;"),
        "backend should expose a 14px default terminal font size"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_LINE_HEIGHT: f32 = 1.4;"),
        "backend should expose a compact 1.4 default line height"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_LETTER_SPACING_PX: f32 = 0.0;"),
        "backend should keep terminal letter spacing at zero"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"Regular\";"),
        "backend should keep terminal weight on Regular"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[")
            && backend_source.contains("\"Cascadia Mono\"")
            && backend_source.contains("\"Sarasa Term SC\"")
            && backend_source.contains("\"Segoe UI Emoji\""),
        "backend should expose the explicit Windows terminal font chain contract"
    );
    assert!(
        backend_source.contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())")
            && backend_source.contains("px_size: DEFAULT_TERMINAL_FONT_SIZE_PX,"),
        "default font requests should flow through the shared typography constants"
    );
    assert!(
        font_mod_source.contains("DEFAULT_TERMINAL_FONT_SIZE_PX")
            && font_mod_source.contains("DEFAULT_TERMINAL_LINE_HEIGHT")
            && font_mod_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN"),
        "terminal font module should re-export the shared typography defaults"
    );
    assert!(
        fallback_source.contains("CJK_FALLBACK_CANDIDATES")
            && (fallback_source.contains("\"Sarasa Term SC\"")
                || fallback_source.contains("DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY"))
            && (fallback_source.contains("\"Segoe UI Emoji\"")
                || fallback_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY")),
        "Windows fallback source should align with the Cascadia -> Sarasa -> Segoe UI Emoji chain"
    );
    assert!(
        fallback_source.contains("'\\u{ff00}'..='\\u{ffef}'"),
        "Windows fallback source should treat fullwidth punctuation as part of the CJK fallback range so Chinese punctuation does not fall back to tofu squares"
    );
    assert!(
        dwrite_source.contains("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf")
            && dwrite_source.contains("DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY"),
        "DirectWrite fallback should keep a bundled Sarasa Term SC face available when packaged Windows builds cannot resolve the system CJK family"
    );
}
