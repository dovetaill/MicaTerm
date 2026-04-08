//! Tracing runtime coverage for log writer setup and metadata emission.

use std::fs;

use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::{
    build_test_logging_runtime, emit_app_root_metadata, emit_runtime_profile_metadata,
    emit_terminal_memory_cache_clear, emit_terminal_memory_cache_reset,
    emit_terminal_memory_startup_snapshot, emit_terminal_memory_surface_refresh,
    emit_terminal_memory_trim_executed, emit_terminal_memory_trim_request,
};
use mica_term::app::runtime_profile::AppRuntimeProfile;
use mica_term::app::terminal_presenter::TerminalPresenterCacheStats;
use uuid::Uuid;

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
fn debug_logging_can_emit_windows_mainline_runtime_profile_metadata() {
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
    assert!(content.contains("initialized runtime profile"));
    assert!(content.contains("WindowsMainline"));
    assert!(content.contains("Skia"));
    assert!(content.contains("winit-skia"));
    assert!(content.contains("prefers_direct3d=true"));
    assert!(content.contains("requested_graphics_api=Some(Direct3D)"));
    assert!(content.contains("renderer_fallback_chain=[Skia, SkiaSoftware, Software]"));
    assert!(content.contains("Some(\"winit\")"));
    assert!(content.contains("Some(\"skia\")"));
}

#[test]
fn debug_logging_emits_app_root_metadata() {
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
    assert!(content.contains("resolved app root directories"));
    assert!(content.contains("PortableMarker"));
    assert!(content.contains(paths.root_dir.to_string_lossy().as_ref()));
    assert!(content.contains(paths.data_dir.to_string_lossy().as_ref()));
    assert!(content.contains(paths.logs_dir.to_string_lossy().as_ref()));
    assert!(content.contains(paths.crash_dir.to_string_lossy().as_ref()));
}

#[test]
fn terminal_memory_diagnostics_remain_quiet_when_disabled() {
    let session_id = Uuid::new_v4();
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
        emit_terminal_memory_startup_snapshot(
            false,
            AppRuntimeProfile::mainline(),
            TerminalPresenterCacheStats::default(),
        );
        emit_terminal_memory_surface_refresh(
            false,
            session_id,
            "startup-idle",
            "bitmap",
            0,
            TerminalPresenterCacheStats::default(),
        );
        emit_terminal_memory_cache_clear(
            false,
            "idle-shrink",
            "surface-disappeared",
            "bitmap",
            TerminalPresenterCacheStats::default(),
            TerminalPresenterCacheStats::default(),
        );
        emit_terminal_memory_cache_reset(
            false,
            session_id,
            "glyph-cache-cap",
            "bitmap",
            1,
            TerminalPresenterCacheStats::default(),
        );
        emit_terminal_memory_trim_request(false, 1024 * 1024);
        emit_terminal_memory_trim_executed(false, 1024 * 1024, true);
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!content.contains("terminal memory startup snapshot"));
    assert!(!content.contains("terminal memory surface refresh"));
    assert!(!content.contains("terminal memory cache shrink"));
    assert!(!content.contains("terminal memory cache reset"));
    assert!(!content.contains("terminal memory trim request"));
    assert!(!content.contains("terminal memory trim executed"));
}

#[test]
fn terminal_memory_diagnostics_emit_when_enabled() {
    let session_id = Uuid::new_v4();
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
    let stats = TerminalPresenterCacheStats {
        shaped_row_cache_entries: 12,
        glyph_raster_cache_entries: 24,
        scene_image_working_pixels_bytes: 4096,
        ..TerminalPresenterCacheStats::default()
    };
    let shrunk_stats = TerminalPresenterCacheStats {
        shaped_row_cache_entries: 1,
        glyph_raster_cache_entries: 2,
        ..TerminalPresenterCacheStats::default()
    };

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_terminal_memory_startup_snapshot(true, AppRuntimeProfile::mainline(), stats);
        emit_terminal_memory_surface_refresh(
            true,
            session_id,
            "surface-refresh",
            "bitmap",
            7,
            stats,
        );
        emit_terminal_memory_cache_clear(
            true,
            "idle-shrink",
            "no-active-surface-idle",
            "bitmap",
            stats,
            shrunk_stats,
        );
        emit_terminal_memory_cache_reset(
            true,
            session_id,
            "glyph-cache-cap",
            "bitmap",
            1,
            shrunk_stats,
        );
        emit_terminal_memory_trim_request(true, 1024 * 1024);
        emit_terminal_memory_trim_executed(true, 1024 * 1024, true);
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("terminal memory startup snapshot"));
    assert!(content.contains("terminal memory surface refresh"));
    assert!(content.contains("terminal memory cache shrink"));
    assert!(content.contains("terminal memory cache reset"));
    assert!(content.contains("cache-reset"));
    assert!(content.contains("reason=\"glyph-cache-cap\""));
    assert!(content.contains(&format!("session_id={session_id}")));
    assert!(content.contains("generation=1"));
    assert!(content.contains("idle-shrink"));
    assert!(content.contains("reason=\"no-active-surface-idle\""));
    assert!(content.contains("terminal memory trim request"));
    assert!(content.contains("terminal memory trim executed"));
    assert!(content.contains("before_shaped_row_cache_entries=12"));
    assert!(content.contains("before_glyph_raster_cache_entries=24"));
    assert!(content.contains("before_scene_image_working_pixels_bytes=4096"));
    assert!(content.contains("after_shaped_row_cache_entries=1"));
    assert!(content.contains("after_glyph_raster_cache_entries=2"));
}
