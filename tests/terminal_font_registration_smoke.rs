use std::{fs, path::Path};

#[test]
fn app_window_has_no_legacy_terminal_font_imports() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !content.contains("SarasaTermSCNerd-Unhinted.ttf"),
        "Sarasa should stay owned by the Rust atlas renderer instead of a Slint startup import"
    );
    assert!(
        !content.contains("IosevkaTerm-Regular.ttf"),
        "Iosevka should be gone from the startup path"
    );
    assert!(
        !content.contains("MapleMonoNormalNL-NF-CN-Regular.ttf"),
        "Maple should be gone from the startup path"
    );
}

#[test]
fn terminal_font_assets_switch_to_fusion_jetbrains_maple_mono_bundle() {
    assert!(
        Path::new("assets/fonts/Fusion-JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf")
            .exists(),
        "the default terminal font bundle should ship a Fusion-JetBrainsMapleMono regular face"
    );
    assert!(
        Path::new("assets/fonts/Fusion-JetBrainsMapleMono/OFL.txt").exists(),
        "the default terminal font bundle should ship the upstream OFL license text"
    );
    assert!(!Path::new("ui/fonts/IosevkaTerm-Regular.ttf").exists());
    assert!(!Path::new("ui/fonts/MapleMonoNormalNL-NF-CN-Regular.ttf").exists());
}

#[test]
fn terminal_host_font_contract_drops_maple_and_legacy_faces() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !content.contains("Maple"),
        "terminal host should stop exposing Maple after the Sarasa follow-up"
    );
    assert!(
        !content.contains("Iosevka Term"),
        "terminal host should stop exposing the retired Iosevka face"
    );
}

#[test]
fn native_terminal_font_default_switches_to_fusion_jetbrains_maple_mono() {
    let backend_source =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");
    let dwrite_source = fs::read_to_string("src/app/terminal_font/windows_dwrite.rs")
        .expect("read windows dwrite font backend");
    let wezterm_source = fs::read_to_string("src/app/terminal_font/wezterm_font.rs")
        .expect("read wezterm font adapter");

    assert!(
        backend_source.contains("DEFAULT_TERMINAL_FONT_FAMILY"),
        "font backend should expose a shared default terminal font family contract"
    );
    assert!(
        backend_source.contains("Fusion JetBrains Maple Mono"),
        "font backend should set Fusion JetBrains Maple Mono as the default terminal font family"
    );
    assert!(
        backend_source.contains("family_name: Some(DEFAULT_TERMINAL_FONT_FAMILY.to_string())"),
        "default font requests should explicitly target the Fusion-JetBrainsMapleMono family"
    );
    assert!(
        dwrite_source.contains("assets/fonts/Fusion-JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf"),
        "Windows native font backend should load the bundled Fusion-JetBrainsMapleMono regular face by default"
    );
    assert!(
        !dwrite_source.contains("ui/fonts/SarasaTermSCNerd-Regular.ttf"),
        "the old Sarasa bundle path should no longer be the default native terminal font"
    );
    assert!(
        wezterm_source.contains("Fusion JetBrains Maple Mono"),
        "WezTerm font migration scaffold should track the new default terminal font family"
    );
}

#[test]
fn bootstrap_no_longer_uses_lazy_terminal_font_registration() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !content.contains("ensure_terminal_font_registered"),
        "bootstrap should not rely on lazy terminal font registration once the atlas renderer owns Maple directly"
    );
}

#[test]
fn legacy_terminal_font_module_is_removed() {
    assert!(
        !Path::new("src/app/terminal_font.rs").exists(),
        "the legacy lazy-registration terminal font module should be removed"
    );
}

#[test]
fn atlas_renderer_switches_off_fontdue() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");

    assert!(cargo_toml.contains("ab_glyph"));
    assert!(!cargo_toml.contains("fontdue"));
    assert!(atlas_source.contains("ab_glyph"));
    assert!(!atlas_source.contains("fontdue"));
}
