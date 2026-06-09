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
        content.contains("session-close"),
        "README should document the dedicated session-close event so packaged diagnostics can separate runtime/session release from later surface-clear and no-surface shrink phases"
    );
    assert!(
        content.contains("startup-snapshot")
            && content.contains("startup-checkpoint")
            && content.contains("close-shrink")
            && content.contains("idle-shrink")
            && content.contains("trim-request")
            && content.contains("trim-executed"),
        "README should document startup-snapshot, startup-checkpoint, close-shrink, idle-shrink, trim-request, and trim-executed so packaged diagnostics can map each runtime memory transition"
    );
    assert!(
        content.contains("private_usage_bytes") && content.contains("pagefile_usage_bytes"),
        "README should explain that runtime memory diagnostics must surface private_usage_bytes and pagefile_usage_bytes so field runs can distinguish real release from a working-set-only trim"
    );
    assert!(
        content.contains("startup_stage")
            && content.contains("after-ui-font-fallbacks")
            && content.contains("after-window-new")
            && content.contains("after-ui-font-diagnostics")
            && content.contains("after-bootstrap-bind")
            && content.contains("ui_shared_collection_configure_calls")
            && content.contains("ui_shared_collection_diagnostics_calls")
            && content.contains("system_font_database_load_calls"),
        "README should document startup-checkpoint stage names plus font/system catalog counters so packaged runs can separate UI font fallback, AppWindow creation, and bootstrap service costs"
    );
    assert!(
        content.contains("before_session_count")
            && content.contains("after_session_count")
            && content.contains("terminal_memory_release_succeeded")
            && content.contains("runtime_disconnect_succeeded"),
        "README should explain that close-path diagnostics must surface before/after session counts plus terminal-memory/disconnect outcomes so field runs can tell whether session close really released runtime state"
    );
}

#[test]
fn runtime_memory_diagnostics_source_wires_all_required_event_families() {
    let logging_runtime =
        fs::read_to_string("src/app/logging/runtime.rs").expect("read logging runtime");
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let ssh_pump = fs::read_to_string("src/app/ssh/runtime/pump.rs").expect("read ssh pump");
    let session_manager =
        fs::read_to_string("src/app/ssh/session_manager.rs").expect("read session manager");

    assert!(
        logging_runtime.contains("startup-snapshot"),
        "logging runtime should define a startup-snapshot memory event so packaged baseline runs capture startup private/commit counters before later optimizations"
    );
    assert!(
        bootstrap.contains("\"startup-checkpoint\"")
            && bootstrap.contains("\"after-ui-font-fallbacks\"")
            && bootstrap.contains("\"after-window-new\"")
            && bootstrap.contains("\"after-ui-font-diagnostics\"")
            && bootstrap.contains("\"after-bootstrap-bind\""),
        "bootstrap should emit staged startup-checkpoint events around UI font fallback, AppWindow creation, UI font diagnostics, and bootstrap service binding so startup private/commit spikes can be attributed instead of guessed"
    );
    assert!(
        bootstrap.contains("\"session-close\"")
            && bootstrap.contains("\"close-shrink\"")
            && bootstrap.contains("\"idle-shrink\""),
        "bootstrap should emit session-close, close-shrink, and idle-shrink events so field diagnostics can distinguish session/runtime release, immediate surface cleanup, and delayed no-surface release"
    );
    assert!(
        ssh_pump.contains("\"trim-request\"") && ssh_pump.contains("\"trim-executed\""),
        "the SSH output pump should emit trim-request and trim-executed events so large-output trims record both the request cause and the post-trim counters"
    );
    assert!(
        logging_runtime.contains("before_session_count")
            && logging_runtime.contains("after_session_count")
            && logging_runtime.contains("terminal_memory_release_succeeded")
            && logging_runtime.contains("runtime_disconnect_succeeded"),
        "logging runtime should surface session-registry counters and runtime release outcomes so close-path logs can prove whether session close actually released memory-bearing state"
    );
    assert!(
        logging_runtime.contains("startup_stage")
            && logging_runtime.contains("ui_shared_collection_configure_calls")
            && logging_runtime.contains("ui_shared_collection_diagnostics_calls")
            && logging_runtime.contains("system_font_database_load_calls"),
        "logging runtime should surface startup_stage plus font/system-catalog counters so packaged startup runs can separate UI font fallback work from AppWindow and bootstrap service costs"
    );
    assert!(
        session_manager.contains("pub struct SessionRegistryDiagnosticsSnapshot")
            && session_manager.contains("pub struct ClosedSessionDiagnostics")
            && session_manager.contains("pub fn close_session_with_diagnostics"),
        "session manager should expose a diagnostics snapshot and close_session_with_diagnostics helper so close-path tests and bootstrap logging can prove which registry/runtime state was released"
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
