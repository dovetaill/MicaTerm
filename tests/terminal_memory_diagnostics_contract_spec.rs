//! Source-level contract coverage for opt-in terminal memory diagnostics.

use std::fs;

#[test]
fn logging_config_exposes_opt_in_memory_diagnostics_toggle() {
    let content = fs::read_to_string("src/app/logging/config.rs").expect("read logging config");

    assert!(
        content.contains("MICA_TERM_MEMORY_DIAGNOSTICS"),
        "logging config should recognize the MICA_TERM_MEMORY_DIAGNOSTICS env toggle so field diagnostics stay opt-in"
    );
    assert!(
        content.contains("memory_diagnostics"),
        "logging config should carry a memory diagnostics flag instead of forcing terminal memory logs on every debug run"
    );
}

#[test]
fn readme_documents_windows_memory_diagnostics_repro_flow() {
    let content = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        content.contains("$env:MICA_TERM_MEMORY_DIAGNOSTICS = \"1\""),
        "README should document how to enable terminal memory diagnostics in the packaged Windows reproduction flow"
    );
    assert!(
        content.contains("MICA_TERM_MEMORY_DIAGNOSTICS=1"),
        "README should mention the dedicated memory diagnostics toggle by name"
    );
    assert!(
        content.contains("close-shrink") && content.contains("idle-shrink"),
        "README should document the close-shrink and idle-shrink memory events so field diagnostics can distinguish immediate surface-clear cleanup from delayed no-surface cleanup"
    );
}

#[test]
fn native_renderer_field_verification_defaults_lower_glyph_caps_to_256() {
    let content = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read wgpu renderer source");

    assert!(
        content.contains("const DEFAULT_MONO_GLYPH_CACHE_LIMIT: usize = 256;"),
        "field verification builds should lower the default mono glyph cache cap to 256 so Windows repros can hit the deferred reset path without synthetic stress tooling"
    );
    assert!(
        content.contains("const DEFAULT_GLYPH_RASTER_CACHE_LIMIT: usize = 256;"),
        "field verification builds should lower the default glyph raster cache cap to 256 so real-world repro logs can surface cache-reset events sooner"
    );
}
