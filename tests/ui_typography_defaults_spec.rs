use std::{fs, path::Path};

#[test]
fn bundled_ui_font_assets_exist() {
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Regular.ttf").exists(),
        "the shell UI bundle should ship a MiSans regular face"
    );
    assert!(
        Path::new("assets/fonts/MiSans/MiSans-Semibold.ttf").exists(),
        "the shell UI bundle should ship a MiSans semibold face for emphasis"
    );
    assert!(
        Path::new("assets/fonts/MiSans/LICENSE.txt").exists(),
        "the shell UI bundle should ship the upstream MiSans license text"
    );
}

#[test]
fn typography_theme_exposes_the_ui_font_contract() {
    let source = fs::read_to_string("ui/theme/typography.slint").expect("read typography theme");

    assert!(source.contains("ui-font-family: \"MiSans\";"));
    assert!(source.contains("ui-font-weight-regular: 400;"));
    assert!(source.contains("ui-font-weight-semibold: 600;"));
    assert!(!source.contains("SarasaUiSC"));
}

#[test]
fn app_window_uses_misans_as_the_shell_default() {
    let source = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Regular.ttf\";"));
    assert!(source.contains("import \"../assets/fonts/MiSans/MiSans-Semibold.ttf\";"));
    assert!(source.contains("import { AppTypography } from \"theme/typography.slint\";"));
    assert!(source.contains("default-font-family: AppTypography.ui-font-family;"));
    assert!(source.contains("default-font-weight: AppTypography.ui-font-weight-regular;"));
    assert!(!source.contains("SarasaUiSC"));
}

#[test]
fn popup_menu_uses_the_shared_ui_font_family() {
    let source =
        fs::read_to_string("ui/components/titlebar-menu.slint").expect("read titlebar menu");

    assert!(source.contains("import { AppTypography } from \"../theme/typography.slint\";"));
    assert!(source.contains("font-family: AppTypography.ui-font-family;"));
    assert!(source.contains("font-weight: AppTypography.ui-font-weight-regular;"));
}

#[test]
fn build_script_tracks_ui_typography_assets() {
    let source = fs::read_to_string("build.rs").expect("read build script");

    assert!(source.contains("assets/fonts/MiSans/MiSans-Regular.ttf"));
    assert!(source.contains("assets/fonts/MiSans/MiSans-Semibold.ttf"));
    assert!(!source.contains("assets/fonts/SarasaUiSC"));
}
