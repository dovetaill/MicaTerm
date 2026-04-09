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
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");

    assert!(
        backend_source
            .contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"JetBrains Mono\";"),
        "backend should set JetBrains Mono as the shared terminal default family"
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
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 15.0;"),
        "backend should expose a Windows-only 15px terminal font size override so the native presenter lands half to one notch larger without changing Linux/macOS defaults"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX: u32 = 23;")
            && backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT: f32 =")
            && backend_source.contains(
                "WINDOWS_DEFAULT_TERMINAL_CELL_HEIGHT_PX as f32 / WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX;"
            ),
        "backend should keep the Windows line-height contract explicit from a 23px cell box so the larger 15px body text stays readable without drifting into a loose terminal"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_LETTER_SPACING_PX: f32 = 0.0;"),
        "backend should keep terminal letter spacing at zero"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"Medium\";"),
        "backend should move terminal weight to Medium"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[")
            && backend_source.contains("\"JetBrains Mono\"")
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
        backend_source.contains("pub fn windows_default() -> Self")
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
        fallback_source.contains("CJK_FALLBACK_CANDIDATES")
            && (fallback_source.contains("\"Sarasa Term SC\"")
                || fallback_source.contains("DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY"))
            && (fallback_source.contains("\"Segoe UI Emoji\"")
                || fallback_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY")),
        "Windows fallback source should align with the JetBrains Mono -> Sarasa -> Segoe UI Emoji chain"
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
    assert!(
        dwrite_source.contains("WINDOWS_DEFAULT_TERMINAL_FONT_SIZE_PX")
            && dwrite_source.contains("WINDOWS_DEFAULT_TERMINAL_LINE_HEIGHT")
            && dwrite_source.contains("let cell_height_px = line_height.max(MIN_CELL_HEIGHT_PX);"),
        "DirectWrite metrics should keep the native line box aligned with the Windows-specific minimum readability floor so retained-native and scene-image paths do not diverge on dense text spacing"
    );
    assert!(
        presenter_source.contains("let request = FontRequest::windows_default();"),
        "Windows presenters should source their default request from the Windows-only typography contract instead of changing global defaults"
    );
}
