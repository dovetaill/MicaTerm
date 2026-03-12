use std::fs;

use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::{build_test_logging_runtime, emit_runtime_profile_metadata};
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
fn debug_logging_can_emit_runtime_profile_metadata() {
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
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        emit_runtime_profile_metadata(AppRuntimeProfile::formal());
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("initialized runtime profile"));
    assert!(content.contains("Formal"));
    assert!(content.contains("Software"));
    assert!(!content.contains("SkiaExperimental"));
    assert!(!content.contains("SkiaSoftware"));
}
