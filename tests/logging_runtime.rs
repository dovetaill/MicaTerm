//! Tracing runtime coverage for log writer setup and metadata emission.

use std::fs;

use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::{
    build_test_logging_runtime, emit_app_root_metadata, emit_runtime_profile_metadata,
};
use mica_term::app::runtime_profile::AppRuntimeProfile;

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
