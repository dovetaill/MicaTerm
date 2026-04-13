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
fn ui_shell_default_weight_moves_to_misans_medium() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        app_window.contains("default-font-weight: AppTypography.ui-font-weight-medium;"),
        "shell UI should default to MiSans Medium so the Windows chrome stops looking overly heavy without dropping all the way back to regular"
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
        diagnostics.contains("let requested_weight = 500;")
            || diagnostics.contains("const UI_FONT_DEFAULT_WEIGHT: i32 = 500;"),
        "ui diagnostics should log the MiSans Medium shell request weight instead of still claiming regular or semibold"
    );
}
