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
fn bundled_terminal_font_contract_uses_cascadia_and_sarasa_assets() {
    assert!(
        Path::new("assets/fonts/CascadiaMono/CascadiaMono-Regular.ttf").exists(),
        "the bundled terminal font contract should ship Cascadia Mono for the default Latin path"
    );
    assert!(
        Path::new("assets/fonts/SarasaTermSC/SarasaTermSC-Regular.ttf").exists(),
        "the bundled terminal font contract should ship Sarasa Term SC for the default CJK path"
    );
    assert!(
        !Path::new("ui/fonts/IosevkaTerm-Regular.ttf").exists(),
        "the old Iosevka terminal font should stay removed from bundled assets"
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

#[test]
fn slint_font_crates_are_vendored_for_startup_memory_work() {
    assert!(
        Path::new("vendor/i-slint-common/sharedfontique.rs").exists(),
        "startup font tuning should vendor i-slint-common locally so the shared font collection can be patched without relying on crates.io defaults"
    );
    assert!(
        Path::new("vendor/i-slint-core/graphics.rs").exists(),
        "startup font tuning should vendor i-slint-core locally so runtime font queries can switch to a primary-then-system lookup path"
    );
}

#[test]
fn startup_primary_font_collection_disables_eager_system_font_scan() {
    let source = fs::read_to_string("vendor/i-slint-common/sharedfontique.rs")
        .expect("read vendored sharedfontique source");

    assert!(
        source.contains("system_fonts: false"),
        "the startup primary font collection should disable eager system font enumeration so startup private/commit is not dominated by the system catalog"
    );
    assert!(
        source.contains("include_bytes!(\"sharedfontique/DejaVuSans.ttf\")"),
        "the startup primary font collection should seed a bundled DejaVu Sans face so common UI text can resolve before any system-font fallback work"
    );
    assert!(
        source.contains("GenericFamily::SystemUi"),
        "the startup primary font collection should map the bundled face onto generic UI families instead of depending on system fonts at startup"
    );
}

#[test]
fn startup_font_source_exposes_lazy_system_collection_helper() {
    let source = fs::read_to_string("vendor/i-slint-common/sharedfontique.rs")
        .expect("read vendored sharedfontique source");

    assert!(
        source.contains("pub static SYSTEM_COLLECTION"),
        "the startup font source should expose a dedicated lazy system collection so system font enumeration is deferred until an actual miss needs it"
    );
    assert!(
        source.contains("pub fn get_system_collection() -> Collection"),
        "the startup font source should expose an accessor for the lazy system collection instead of forcing every startup query through the eager shared collection"
    );
}

#[test]
fn startup_font_query_uses_primary_then_system_fallback() {
    let source =
        fs::read_to_string("vendor/i-slint-core/graphics.rs").expect("read vendored graphics");

    assert!(
        source.contains("let mut collection = sharedfontique::get_collection();"),
        "startup font queries should keep the lightweight primary collection as the first lookup path"
    );
    assert!(
        source.contains("let mut system_collection = sharedfontique::get_system_collection();"),
        "startup font queries should only touch the system-backed collection after the primary startup collection misses"
    );
}

#[test]
fn startup_sharedparley_context_stays_on_primary_collection() {
    let source = fs::read_to_string("vendor/i-slint-core/textlayout/sharedparley.rs")
        .expect("read vendored sharedparley source");

    assert!(
        source.contains("sharedfontique::COLLECTION.inner.clone()"),
        "the startup sharedparley font context should stay pinned to the lightweight primary collection so first-frame layout does not eagerly enumerate system fonts"
    );
    assert!(
        !source.contains("get_system_collection"),
        "the startup sharedparley font context should not wire the lazy system collection into the always-on layout context"
    );
}
