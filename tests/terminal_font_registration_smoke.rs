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
fn bundled_font_assets_cover_terminal_and_shell_contracts() {
    assert!(
        Path::new("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf").exists(),
        "the terminal bundle should ship a Sarasa Term SC Nerd regular face"
    );
    assert!(
        Path::new("assets/fonts/SarasaTermSCNerd/LICENSE.txt").exists(),
        "the terminal bundle should ship the upstream Sarasa license text"
    );
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Regular.ttf").exists(),
        "the shell UI bundle should ship a MiSans regular face"
    );
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Medium.ttf").exists(),
        "the shell UI bundle should ship a MiSans medium face"
    );
    assert!(
        Path::new("assets/fonts/MiSans/LICENSE.txt").exists(),
        "the shell UI bundle should ship the upstream MiSans license text"
    );
    for retired_family in [
        "assets/fonts/JetBrainsMono",
        "assets/fonts/CascadiaMono",
        "assets/fonts/SarasaUiSC",
        "assets/fonts/Fusion-JetBrainsMapleMono",
    ] {
        assert!(
            !Path::new(retired_family).exists(),
            "retired bundled font family `{retired_family}` should be removed from the repository"
        );
    }
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
fn terminal_shared_font_contract_switches_to_sarasa() {
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let mock_source = fs::read_to_string("src/app/terminal_font/mock.rs").expect("read mock");
    let fallback_source =
        fs::read_to_string("src/app/terminal_font/windows_fallback.rs").expect("read fallback");
    let presenter_source =
        fs::read_to_string("src/app/terminal_presenter.rs").expect("read presenter");
    let windows_renderer_source =
        fs::read_to_string("src/app/terminal_renderer/platform/windows.rs")
            .expect("read windows renderer");

    assert!(
        atlas_source.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf"),
        "bitmap atlas should load the bundled Sarasa Term SC Nerd SemiBold face as the shared terminal font"
    );
    assert!(
        !atlas_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"),
        "bitmap atlas should stop loading the bundled JetBrains Mono atlas face"
    );
    assert!(
        backend_source
            .contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"Sarasa Term SC Nerd\";"),
        "font backend should set Sarasa Term SC Nerd as the shared terminal default family"
    );
    assert!(
        backend_source.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"SemiBold\";"),
        "font backend should keep the shared terminal request aligned with the packaged SemiBold face"
    );
    assert!(
        backend_source.contains(
            "pub const DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY: &str = DEFAULT_TERMINAL_FONT_FAMILY;"
        ),
        "the CJK fallback constant should collapse into the shared Sarasa terminal family"
    );
    assert!(
        backend_source.contains("pub const WINDOWS_DEFAULT_TERMINAL_FONT_CHAIN: &[&str] = &[")
            && backend_source.contains("DEFAULT_TERMINAL_FONT_FAMILY")
            && backend_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY"),
        "the Windows terminal font chain should keep the Sarasa primary family plus emoji fallback"
    );
    assert!(
        backend_source.contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())"),
        "font requests should explicitly target the shared default family contract"
    );
    assert!(
        !backend_source.contains("\"JetBrains Mono\""),
        "font backend should stop advertising JetBrains Mono as the shared terminal primary family"
    );
    assert!(
        mock_source.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf"),
        "mock font shaping should use the same bundled Sarasa terminal font as production defaults"
    );
    assert!(
        !mock_source.contains("assets/fonts/JetBrainsMono/JetBrainsMono-Medium.ttf"),
        "mock font shaping should stop loading the bundled JetBrains Mono face"
    );
    assert!(
        windows_renderer_source
            .contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf"),
        "Windows native text rendering should resolve the bundled Sarasa Term SC Nerd SemiBold face for DirectWrite"
    );
    assert!(
        !windows_renderer_source.contains("assets/fonts/JetBrainsMono"),
        "Windows native text rendering should stop referencing the retired JetBrains Mono bundle"
    );
    assert!(
        (fallback_source.contains("\"Sarasa Term SC\"")
            || fallback_source.contains("DEFAULT_TERMINAL_CJK_FALLBACK_FAMILY")
            || fallback_source.contains("DEFAULT_TERMINAL_FONT_FAMILY"))
            && (fallback_source.contains("\"Segoe UI Emoji\"")
                || fallback_source.contains("DEFAULT_TERMINAL_EMOJI_FALLBACK_FAMILY")),
        "Windows fallback resolution should continue to reference Sarasa Term SC and Segoe UI Emoji while the shared terminal family changes"
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
fn build_script_watches_only_active_font_assets() {
    let source = fs::read_to_string("build.rs").expect("read build script");

    assert!(
        source.contains("assets/fonts/MiSans/MiSans-Regular.ttf"),
        "build script should watch the bundled MiSans regular asset"
    );
    assert!(
        source.contains("assets/fonts/MiSans/MiSans-Medium.ttf"),
        "build script should watch the bundled MiSans medium asset"
    );
    assert!(
        source.contains("assets/fonts/MiSans/MiSans-Semibold.ttf"),
        "build script should watch the bundled MiSans semibold asset"
    );
    assert!(
        source.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf"),
        "build script should watch the bundled Sarasa Term SC Nerd asset used by the terminal renderer"
    );
    assert!(
        !source.contains("assets/fonts/JetBrainsMono"),
        "build script should stop watching the retired JetBrains Mono bundle"
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
