use std::{fs, path::Path};

#[test]
fn startup_path_drops_legacy_terminal_font_imports() {
    let content = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !content.contains("IosevkaTerm-Regular.ttf"),
        "startup should not import the retired Iosevka terminal font"
    );
    assert!(
        !content.contains("SarasaTermSCNerd-Unhinted.ttf"),
        "the terminal font should stay owned by the Rust renderer instead of a Slint import path"
    );
}

#[test]
fn bundled_terminal_font_contract_uses_sarasa_assets() {
    assert!(
        Path::new("assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf").exists(),
        "the bundled terminal font contract should ship Sarasa Term SC Nerd for the shared terminal path"
    );
    assert!(
        !Path::new("ui/fonts/IosevkaTerm-Regular.ttf").exists(),
        "the old Iosevka terminal font should stay removed from bundled assets"
    );
}

#[test]
fn readme_describes_current_bundled_shell_and_terminal_fonts() {
    let content = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        content.contains("MiSans"),
        "readme should describe MiSans as the bundled shell UI family"
    );
    assert!(
        content.contains("Sarasa Term SC Nerd"),
        "readme should describe Sarasa Term SC Nerd as the bundled terminal family"
    );
    assert!(
        !content.contains("ui/fonts/SarasaTermSCNerd-Regular.ttf"),
        "readme should stop describing the retired SarasaTermSCNerd startup asset as the live terminal font"
    );
}

#[test]
fn terminal_host_drops_legacy_font_stack_strings() {
    let content =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !content.contains("Iosevka Term"),
        "terminal host should stop advertising the Iosevka fallback stack"
    );
}

#[test]
fn atlas_renderer_dependency_contract_uses_ab_glyph_instead_of_fontdue() {
    let cargo_toml = fs::read_to_string("Cargo.toml").expect("read Cargo.toml");
    let atlas_source =
        fs::read_to_string("src/app/terminal_atlas.rs").expect("read terminal atlas");

    assert!(
        cargo_toml.contains("ab_glyph"),
        "atlas renderer should depend on ab_glyph for lazy glyph loading"
    );
    assert!(
        !cargo_toml.contains("fontdue"),
        "fontdue should stay removed from the atlas renderer path"
    );
    assert!(
        atlas_source.contains("ab_glyph"),
        "terminal atlas source should use ab_glyph APIs directly"
    );
    assert!(
        !atlas_source.contains("fontdue"),
        "terminal atlas source should no longer build on fontdue"
    );
}
