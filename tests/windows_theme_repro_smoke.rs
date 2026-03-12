use std::fs;
use std::path::Path;

#[test]
fn windows_theme_repro_sources_exist() {
    assert!(Path::new("src/bin/windows_theme_repro.rs").exists());
    assert!(Path::new("ui/windows-theme-repro.slint").exists());
}

#[test]
fn windows_theme_repro_does_not_use_production_bootstrap() {
    let content =
        fs::read_to_string("src/bin/windows_theme_repro.rs").expect("read windows theme repro");

    assert!(!content.contains("bootstrap::run_with_profile"));
    assert!(!content.contains("window_effects"));
    assert!(!content.contains("AppWindow::new"));
    assert!(content.contains("toggle_theme_requested"));
}
