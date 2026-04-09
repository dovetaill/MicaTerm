use std::{fs, path::Path};

#[test]
fn app_window_has_no_legacy_terminal_font_imports() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !content.contains("SarasaTermSCNerd-Unhinted.ttf"),
        "Sarasa should stay owned by the Rust terminal renderer instead of a Slint startup import"
    );
    assert!(
        !content.contains("IosevkaTerm-Regular.ttf"),
        "Iosevka should stay out of the startup path"
    );
}

#[test]
fn terminal_font_assets_switch_to_windows_terminal_bundle() {
    assert!(
        Path::new("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf").exists(),
        "the Windows terminal default bundle should ship a JetBrains Mono medium face"
    );
    assert!(
        Path::new("assets/fonts/JetBrainsMono/OFL.txt").exists(),
        "the JetBrains Mono bundle should ship the upstream OFL text"
    );
    assert!(
        Path::new("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf").exists(),
        "the Windows terminal default bundle should ship a Sarasa Term SC regular face"
    );
    assert!(
        Path::new("assets/fonts/SarasaTermSC/LICENSE.txt").exists(),
        "the Sarasa Term SC bundle should ship the upstream license text"
    );
    assert!(
        Path::new("assets/fonts/SarasaUiSC/SarasaUiSC-Regular.ttf").exists(),
        "the Slint UI bundle should ship a Sarasa UI SC regular face"
    );
    assert!(
        Path::new("assets/fonts/SarasaUiSC/LICENSE.txt").exists(),
        "the Sarasa UI SC bundle should ship the upstream license text"
    );
}

#[test]
fn terminal_host_font_contract_drops_legacy_faces() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !content.contains("Iosevka Term"),
        "terminal host should not expose the retired Iosevka face"
    );
}

#[test]
fn bitmap_and_native_font_sources_point_at_windows_terminal_defaults() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");

    assert!(
        atlas_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"),
        "bitmap atlas should load the bundled JetBrains Mono medium face as the default Latin terminal font"
    );
    assert!(
        backend_source
            .contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"JetBrains Mono\";"),
        "font backend should move the default terminal family to JetBrains Mono"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"Medium\";"),
        "font backend should move the shared terminal default weight to Medium"
    );
    assert!(
        backend_source.contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())"),
        "default font requests should explicitly target the shared terminal family contract"
    );
    assert!(
        (fallback_source.contains("\"Sarasa Term SC\"")
            || fallback_source.contains("DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY"))
            && (fallback_source.contains("\"Segoe UI Emoji\"")
                || fallback_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY")),
        "Windows fallback resolution should explicitly include Sarasa Term SC and Segoe UI Emoji behind the primary JetBrains Mono face"
    );
    assert!(
        presenter_source.contains("let request = FontRequest::windows_default();"),
        "Windows presenters should source their default typography from the Windows FontRequest contract instead of hard-coding per-path font sizes or changing Linux/macOS defaults"
    );
}

#[test]
fn bootstrap_no_longer_uses_lazy_terminal_font_registration() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !content.contains("ensure_terminal_font_registered"),
        "bootstrap should not rely on lazy terminal font registration"
    );
}

#[test]
fn legacy_terminal_font_module_is_removed() {
    assert!(
        !Path::new("src/app/terminal_font.rs").exists(),
        "the legacy lazy-registration terminal font module should stay removed"
    );
}

#[test]
fn atlas_renderer_stays_on_ab_glyph() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");

    assert!(cargo_toml.contains("ab_glyph"));
    assert!(!cargo_toml.contains("fontdue"));
    assert!(atlas_source.contains("ab_glyph"));
    assert!(!atlas_source.contains("fontdue"));
}
