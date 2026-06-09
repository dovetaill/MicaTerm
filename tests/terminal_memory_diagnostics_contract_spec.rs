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
        content.contains("startup-snapshot")
            && content.contains("close-shrink")
            && content.contains("idle-shrink")
            && content.contains("trim-request")
            && content.contains("trim-executed"),
        "README should document startup-snapshot, close-shrink, idle-shrink, trim-request, and trim-executed so packaged diagnostics can map each runtime memory transition"
    );
    assert!(
        content.contains("private_usage_bytes") && content.contains("pagefile_usage_bytes"),
        "README should explain that runtime memory diagnostics must surface private_usage_bytes and pagefile_usage_bytes so field runs can distinguish real release from a working-set-only trim"
    );
}

#[test]
fn runtime_memory_diagnostics_source_wires_all_required_event_families() {
    let logging_runtime =
        fs::read_to_string("src/app/logging/runtime.rs").expect("read logging runtime");
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let ssh_pump = fs::read_to_string("src/app/ssh/runtime/pump.rs").expect("read ssh pump");

    assert!(
        logging_runtime.contains("startup-snapshot"),
        "logging runtime should define a startup-snapshot memory event so packaged baseline runs capture startup private/commit counters before later optimizations"
    );
    assert!(
        bootstrap.contains("\"close-shrink\"") && bootstrap.contains("\"idle-shrink\""),
        "bootstrap should emit both close-shrink and idle-shrink events so field diagnostics can distinguish immediate surface cleanup from delayed no-surface release"
    );
    assert!(
        ssh_pump.contains("\"trim-request\"") && ssh_pump.contains("\"trim-executed\""),
        "the SSH output pump should emit trim-request and trim-executed events so large-output trims record both the request cause and the post-trim counters"
    );
}

#[test]
fn native_renderer_defaults_restore_glyph_caps_to_1024() {
    let content = fs::read_to_string("src/app/terminal_renderer/wgpu_renderer.rs")
        .expect("read wgpu renderer source");

    assert!(
        content.contains("const DEFAULT_MONO_GLYPH_CACHE_LIMIT: usize = 1024;"),
        "the default mono glyph cache cap should return to 1024 once the field diagnostics run is complete"
    );
    assert!(
        content.contains("const DEFAULT_GLYPH_RASTER_CACHE_LIMIT: usize = 1024;"),
        "the default glyph raster cache cap should return to 1024 so production builds avoid overly aggressive deferred cache resets"
    );
}
