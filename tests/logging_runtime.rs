//! Tracing runtime coverage for log writer setup and metadata emission.

use std::fs;

use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::{
    MemoryDiagnosticsEvent, build_test_logging_runtime, emit_app_root_metadata,
    emit_memory_diagnostics_event_with_config, emit_runtime_profile_metadata,
    emit_startup_memory_snapshot_with_config,
};
use mica_term::app::memory::ProcessMemorySnapshot;
use mica_term::app::runtime_profile::AppRuntimeProfile;
use mica_term::app::ssh::session_manager::SessionRegistryDiagnosticsSnapshot;
use mica_term::app::terminal_presenter::TerminalPresenterCacheStats;

#[test]
fn logging_runtime_writes_error_but_filters_debug_by_default() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-default");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::ErrorOnly);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        tracing::debug!(target: "ui.tooltip", "debug event should be filtered");
        tracing::error!(target: "app.lifecycle", "error event should be persisted");
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("error event should be persisted"));
    assert!(!content.contains("debug event should be filtered"));
}

#[test]
fn logging_runtime_keeps_debug_events_when_debug_mode_is_enabled() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-debug");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        tracing::debug!(target: "app.logging", "debug event should survive");
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("debug event should survive"));
}

#[test]
fn debug_logging_ignores_runtime_profile_metadata_helpers() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-profile");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_runtime_profile_metadata(AppRuntimeProfile::mainline());
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!content.contains("initialized runtime profile"));
    assert!(!content.contains("WindowsMainline"));
    assert!(!content.contains("winit-skia"));
}

#[test]
fn debug_logging_ignores_app_root_metadata_helpers() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-app-root");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();
    fs::create_dir_all(temp_root.join("data")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::PortableMarker,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_app_root_metadata(&paths);
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!content.contains("resolved app root directories"));
    assert!(!content.contains("PortableMarker"));
    assert!(!content.contains(paths.root_dir.to_string_lossy().as_ref()));
    assert!(!content.contains(paths.data_dir.to_string_lossy().as_ref()));
    assert!(!content.contains(paths.logs_dir.to_string_lossy().as_ref()));
    assert!(!content.contains(paths.crash_dir.to_string_lossy().as_ref()));
}

fn sample_process_memory_snapshot(
    working_set_bytes: usize,
    pagefile_usage_bytes: usize,
    private_usage_bytes: usize,
) -> ProcessMemorySnapshot {
    ProcessMemorySnapshot {
        working_set_bytes,
        peak_working_set_bytes: working_set_bytes.saturating_add(1024),
        pagefile_usage_bytes,
        private_usage_bytes,
    }
}

fn sample_cache_stats(entries: usize) -> TerminalPresenterCacheStats {
    TerminalPresenterCacheStats {
        previous_frame_rows: 48,
        previous_shaped_rows: 48,
        shaped_row_cache_entries: entries,
        shaped_row_cache_capacity: 256,
        mono_glyph_cache_entries: entries / 2,
        color_glyph_cache_entries: entries / 4,
        glyph_raster_cache_entries: entries / 2,
        prepared_row_cache_entries: entries / 3,
    }
}

fn sample_session_registry_snapshot(entries: usize) -> SessionRegistryDiagnosticsSnapshot {
    SessionRegistryDiagnosticsSnapshot {
        session_count: entries,
        open_order_count: entries,
        asset_session_count: entries,
        terminal_surface_count: entries,
        runtime_control_count: entries,
        pending_disconnect_count: entries.saturating_sub(1),
        pending_resize_count: entries / 2,
        current_working_directory_count: entries,
        disabled_enhancement_count: entries / 3,
        sftp_binding_count: entries / 2,
    }
}

#[test]
fn memory_diagnostics_helpers_stay_silent_when_toggle_is_disabled() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-memory-disabled");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_startup_memory_snapshot_with_config(AppRuntimeProfile::mainline(), config);
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::mainline(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "close-shrink",
                trigger_reason: Some("surface-disappeared"),
                before_memory: Some(sample_process_memory_snapshot(200, 300, 400)),
                after_memory: Some(sample_process_memory_snapshot(150, 300, 400)),
                cache_stats_before: Some(sample_cache_stats(96)),
                cache_stats_after: Some(sample_cache_stats(0)),
                ..MemoryDiagnosticsEvent::default()
            },
        );
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!content.contains("app.memory"));
    assert!(!content.contains("startup-snapshot"));
    assert!(!content.contains("close-shrink"));
}

#[test]
fn memory_diagnostics_helpers_emit_structured_runtime_memory_events_when_enabled() {
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("logging-runtime-memory-enabled");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug).with_memory_diagnostics(true);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_startup_memory_snapshot_with_config(AppRuntimeProfile::software_compat(), config);
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::software_compat(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "trim-request",
                trigger_reason: Some("large-output-idle"),
                active_renderer_mode: Some("native"),
                pending_output_bytes: Some(1_048_577),
                idle_interval_ms: Some(2_000),
                before_memory: Some(sample_process_memory_snapshot(
                    205_586_432,
                    206_917_632,
                    206_917_632,
                )),
                ..MemoryDiagnosticsEvent::default()
            },
        );
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::software_compat(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "trim-executed",
                trigger_reason: Some("large-output-idle"),
                active_renderer_mode: Some("native"),
                pending_output_bytes: Some(1_048_577),
                idle_interval_ms: Some(2_000),
                trim_succeeded: Some(true),
                before_memory: Some(sample_process_memory_snapshot(
                    205_586_432,
                    206_917_632,
                    206_917_632,
                )),
                after_memory: Some(sample_process_memory_snapshot(
                    659_456,
                    206_917_632,
                    206_917_632,
                )),
                ..MemoryDiagnosticsEvent::default()
            },
        );
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::software_compat(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "session-close",
                trigger_reason: Some("session-close"),
                before_memory: Some(sample_process_memory_snapshot(
                    210_000_000,
                    212_000_000,
                    212_000_000,
                )),
                after_memory: Some(sample_process_memory_snapshot(
                    181_000_000,
                    212_000_000,
                    212_000_000,
                )),
                session_registry_before: Some(sample_session_registry_snapshot(3)),
                session_registry_after: Some(sample_session_registry_snapshot(2)),
                runtime_control_present_before: Some(true),
                terminal_surface_present_before: Some(true),
                sftp_binding_present_before: Some(true),
                terminal_memory_release_attempted: Some(true),
                terminal_memory_release_succeeded: Some(true),
                runtime_disconnect_attempted: Some(true),
                runtime_disconnect_succeeded: Some(true),
                ..MemoryDiagnosticsEvent::default()
            },
        );
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::software_compat(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "close-shrink",
                trigger_reason: Some("surface-disappeared"),
                active_renderer_mode: Some("native"),
                has_active_surface: Some(false),
                retained_renderer_resources_before: Some(true),
                retained_renderer_resources_after: Some(true),
                before_memory: Some(sample_process_memory_snapshot(
                    200_000_000,
                    210_000_000,
                    210_000_000,
                )),
                after_memory: Some(sample_process_memory_snapshot(
                    180_000_000,
                    210_000_000,
                    210_000_000,
                )),
                cache_stats_before: Some(sample_cache_stats(96)),
                cache_stats_after: Some(sample_cache_stats(0)),
                ..MemoryDiagnosticsEvent::default()
            },
        );
        emit_memory_diagnostics_event_with_config(
            AppRuntimeProfile::software_compat(),
            config,
            MemoryDiagnosticsEvent {
                event_name: "idle-shrink",
                trigger_reason: Some("no-active-surface-idle"),
                active_renderer_mode: Some("native"),
                has_active_surface: Some(false),
                no_surface_idle_ms: Some(30_000),
                trim_succeeded: Some(true),
                backend_purge_succeeded: Some(true),
                retained_renderer_resources_before: Some(true),
                retained_renderer_resources_after: Some(false),
                before_memory: Some(sample_process_memory_snapshot(
                    180_000_000,
                    210_000_000,
                    210_000_000,
                )),
                after_memory: Some(sample_process_memory_snapshot(
                    20_000_000, 60_000_000, 60_000_000,
                )),
                cache_stats_before: Some(sample_cache_stats(0)),
                ..MemoryDiagnosticsEvent::default()
            },
        );
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("app.memory"));
    assert!(content.contains("startup-snapshot"));
    assert!(content.contains("session-close"));
    assert!(content.contains("close-shrink"));
    assert!(content.contains("idle-shrink"));
    assert!(content.contains("trim-request"));
    assert!(content.contains("trim-executed"));
    assert!(content.contains("requested_build_flavor"));
    assert!(content.contains("requested_terminal_render_mode"));
    assert!(content.contains("requested_native_present_path"));
    assert!(content.contains("working_set_bytes"));
    assert!(content.contains("private_usage_bytes"));
    assert!(content.contains("pagefile_usage_bytes"));
    assert!(content.contains("before_private_usage_bytes"));
    assert!(content.contains("after_private_usage_bytes"));
    assert!(content.contains("before_session_count"));
    assert!(content.contains("after_session_count"));
    assert!(content.contains("before_runtime_control_count"));
    assert!(content.contains("after_runtime_control_count"));
    assert!(content.contains("terminal_memory_release_succeeded"));
    assert!(content.contains("runtime_disconnect_succeeded"));
}
