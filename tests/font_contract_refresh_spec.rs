use std::fs;
use std::path::Path;

#[test]
fn terminal_font_contract_switches_to_sarasa_nerd_assets() {
    let backend = fs::read_to_string("src/app/terminal_font/backend.rs").expect("read backend");
    let atlas = fs::read_to_string("src/app/terminal_atlas.rs").expect("read atlas");
    let dwrite =
        fs::read_to_string("src/app/terminal_font/windows_dwrite.rs").expect("read dwrite");
    let diagnostics = fs::read_to_string("src/app/font_diagnostics.rs").expect("read diagnostics");

    assert!(
        Path::new("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf").exists(),
        "terminal bundle should ship the actual Sarasa Term SC Nerd regular face so Windows text and Nerd icons use the same font"
    );
    assert!(
        Path::new("assets/fonts/SarasaTermSCNerd/LICENSE.txt").exists(),
        "terminal bundle should ship the Sarasa Term SC Nerd license text"
    );
    assert!(
        backend.contains("pub const DEFAULT_TERMINAL_FONT_FAMILY: &str = \"Sarasa Term SC Nerd\";"),
        "terminal backend should request the Nerd family explicitly instead of plain Sarasa Term SC"
    );
    assert!(
        backend.contains("pub const DEFAULT_TERMINAL_FONT_WEIGHT: &str = \"SemiBold\";"),
        "terminal backend should request SemiBold once the bundled face switches away from the older Regular default"
    );
    assert!(
        atlas.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf"),
        "atlas renderer should use the packaged Sarasa Term SC Nerd SemiBold face"
    );
    assert!(
        dwrite.contains("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf")
            && dwrite.contains("post_script_name: \"SarasaTermSCNerd-SemiBold\".to_string()"),
        "DirectWrite path should use the packaged Sarasa Term SC Nerd SemiBold face metadata"
    );
    assert!(
        diagnostics.contains("pub const TERMINAL_PRIMARY_FAMILY: &str = \"Sarasa Term SC Nerd\";"),
        "font diagnostics should log the Nerd family as the requested terminal primary family"
    );
}

#[test]
fn ui_shell_default_weight_stays_regular_for_small_shell_text() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        app_window.contains("default-font-weight: AppTypography.ui-font-weight-regular;"),
        "shell UI should keep the global default on JetBrains Maple Mono regular so text inputs do not all become unnecessarily heavy"
    );
    assert!(
        app_window.contains("default-font-size: AppTypography.ui-font-size-body;"),
        "shell UI should lift the default font size a notch so the chrome stops looking undersized on Windows"
    );
}

#[test]
fn ui_shell_diagnostics_ignore_expected_symbol_and_emoji_fallbacks() {
    let diagnostics = fs::read_to_string("src/app/font_diagnostics.rs").expect("read diagnostics");

    assert!(
        diagnostics.contains("let shell_probe_matches = [latin.as_ref(), cjk.as_ref()]")
            || diagnostics.contains("shell_probe_matches"),
        "ui diagnostics should derive shell-family drift only from latin/cjk shell probes instead of counting expected symbol or emoji fallback families"
    );
}

#[test]
fn ui_shell_diagnostics_report_medium_request_weight() {
    let diagnostics = fs::read_to_string("src/app/font_diagnostics.rs").expect("read diagnostics");

    assert!(
        diagnostics.contains("let requested_weight = UI_FONT_DEFAULT_WEIGHT;")
            || diagnostics.contains("pub const UI_FONT_DEFAULT_WEIGHT: i32 = 400;"),
        "ui diagnostics should keep the default family probe on JetBrains Maple Mono regular so generic text/input inheritance stays truthful"
    );
    assert!(
        diagnostics.contains("ui_chrome_font_weight")
            || diagnostics.contains("UI_CHROME_FONT_WEIGHT"),
        "ui diagnostics should also expose the dedicated chrome weight so packaged Windows runs do not force people to infer why tabs and menus look heavier than generic body text"
    );
    assert!(
        diagnostics.contains("chrome_requested_weight")
            || diagnostics.contains("chrome_resolved_weight")
            || diagnostics.contains("chrome_resolved_family"),
        "ui diagnostics should log the actual chrome probe so packaged Windows runs can verify the visible shell labels are really hitting the intended JetBrains Maple Mono face"
    );
}

#[test]
fn slint_renderer_overrides_jetbrains_maple_mono_bundle_weights_to_css_values() {
    let skia_renderer =
        fs::read_to_string("vendor/i-slint-renderer-skia/lib.rs").expect("read skia renderer");

    for expected in [
        "FontInfoOverride",
        "JetBrainsMapleMono-Regular",
        "FontWeight::new(400.0)",
        "family_name: Some(\"JetBrains Maple Mono\")",
    ] {
        assert!(
            skia_renderer.contains(expected),
            "the vendored Slint renderer should keep `{expected}` so bundled JetBrains Maple Mono fonts are registered with CSS-like weights instead of their raw embedded metadata"
        );
    }

    assert!(
        !skia_renderer.contains("JetBrainsMapleMono-Medium"),
        "the vendored Slint renderer should stop carrying a separate JetBrains Maple Mono medium override once the shell becomes regular-only"
    );
    assert!(
        !skia_renderer.contains("JetBrainsMapleMono-SemiBold"),
        "the vendored Slint renderer should stop carrying a separate JetBrains Maple Mono semibold override once the shell becomes regular-only"
    );
}

#[test]
fn font_diagnostics_distinguish_effective_and_embedded_weights() {
    let diagnostics = fs::read_to_string("src/app/font_diagnostics.rs").expect("read diagnostics");

    for expected in [
        "embedded_weight",
        "embedded_post_script_name",
        "resolved_weight = requested_match.weight.as_str()",
        "chrome_resolved_weight = chrome_requested_match.weight.as_str()",
        "resolved_primary_embedded_weight",
        "cjk_embedded_weight",
        "symbol_embedded_weight",
        "icon_embedded_weight",
        "emoji_embedded_weight",
    ] {
        assert!(
            diagnostics.contains(expected),
            "font diagnostics should keep `{expected}` so packaged Windows logs reveal the effective matched weight separately from the font file's embedded metadata"
        );
    }
}
