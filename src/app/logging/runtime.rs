//! Creates the tracing runtime, log writer guard, and startup metadata emission helpers.

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::app::memory::ProcessMemorySnapshot;
use crate::app::runtime_profile::AppRuntimeProfile;
use crate::app::terminal_presenter::TerminalPresenterCacheStats;

use super::cleanup::{CleanupPolicy, cleanup_logging_dirs};
use super::config::AppLoggingConfig;
use super::paths::{LoggingPaths, resolve_logging_paths_for_app};

pub struct AppLoggingRuntime {
    pub paths: LoggingPaths,
    pub guard: WorkerGuard,
}

pub struct TestLoggingRuntime {
    pub dispatch: tracing::Dispatch,
    pub guard: WorkerGuard,
}

pub fn memory_diagnostics_enabled() -> bool {
    AppLoggingConfig::from_env().memory_diagnostics_enabled()
}

pub fn emit_runtime_profile_metadata(profile: AppRuntimeProfile) {
    tracing::info!(
        target: "app.renderer",
        build_flavor = ?profile.build_flavor,
        renderer_mode = ?profile.renderer_mode,
        terminal_render_mode = ?profile.terminal_render_mode(),
        selector_label = profile.selector_label(),
        prefers_direct3d = profile.prefers_direct3d(),
        requested_graphics_api = ?profile.preferred_graphics_api(),
        renderer_fallback_chain = ?profile.renderer_fallback_chain(),
        forced_backend = ?profile.forced_backend(),
        forced_renderer = ?profile.forced_renderer(),
        "initialized runtime profile"
    );
}

pub fn emit_app_root_metadata(paths: &LoggingPaths) {
    tracing::info!(
        target: "app.paths",
        root_source = ?paths.root_source,
        root_dir = %paths.root_dir.display(),
        data_dir = %paths.data_dir.display(),
        logs_dir = %paths.logs_dir.display(),
        crash_dir = %paths.crash_dir.display(),
        "resolved app root directories"
    );
}

pub fn emit_terminal_memory_cache_clear(
    enabled: bool,
    event: &str,
    reason: &str,
    render_mode: &str,
    before: TerminalPresenterCacheStats,
    after: TerminalPresenterCacheStats,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event,
        reason,
        render_mode,
        before_previous_frame_rows = before.previous_frame_rows,
        before_previous_shaped_rows = before.previous_shaped_rows,
        before_shaped_row_cache_entries = before.shaped_row_cache_entries,
        before_shaped_row_cache_capacity = before.shaped_row_cache_capacity,
        before_mono_glyph_cache_entries = before.mono_glyph_cache_entries,
        before_color_glyph_cache_entries = before.color_glyph_cache_entries,
        before_glyph_raster_cache_entries = before.glyph_raster_cache_entries,
        before_prepared_row_cache_entries = before.prepared_row_cache_entries,
        before_scene_image_mono_glyph_cache_entries = before.scene_image_mono_glyph_cache_entries,
        before_scene_image_color_glyph_cache_entries = before.scene_image_color_glyph_cache_entries,
        before_scene_image_last_base_pixels_bytes = before.scene_image_last_base_pixels_bytes,
        before_scene_image_working_pixels_bytes = before.scene_image_working_pixels_bytes,
        after_previous_frame_rows = after.previous_frame_rows,
        after_previous_shaped_rows = after.previous_shaped_rows,
        after_shaped_row_cache_entries = after.shaped_row_cache_entries,
        after_shaped_row_cache_capacity = after.shaped_row_cache_capacity,
        after_mono_glyph_cache_entries = after.mono_glyph_cache_entries,
        after_color_glyph_cache_entries = after.color_glyph_cache_entries,
        after_glyph_raster_cache_entries = after.glyph_raster_cache_entries,
        after_prepared_row_cache_entries = after.prepared_row_cache_entries,
        after_scene_image_mono_glyph_cache_entries = after.scene_image_mono_glyph_cache_entries,
        after_scene_image_color_glyph_cache_entries = after.scene_image_color_glyph_cache_entries,
        after_scene_image_last_base_pixels_bytes = after.scene_image_last_base_pixels_bytes,
        after_scene_image_working_pixels_bytes = after.scene_image_working_pixels_bytes,
        "terminal memory cache shrink"
    );
}

pub fn emit_terminal_memory_trim_threshold_crossed(
    enabled: bool,
    session_id: Uuid,
    chunk_bytes: usize,
    pending_output_bytes: usize,
    trim_threshold_bytes: usize,
    idle_interval_ms: u64,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-threshold-crossed",
        session_id = session_id.to_string(),
        chunk_bytes,
        pending_output_bytes,
        trim_threshold_bytes,
        idle_interval_ms,
        "terminal memory trim threshold crossed"
    );
}

pub fn emit_terminal_memory_trim_request(
    enabled: bool,
    session_id: Uuid,
    pending_output_bytes: usize,
    before: Option<ProcessMemorySnapshot>,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-request",
        session_id = session_id.to_string(),
        pending_output_bytes,
        before_working_set_bytes = before.map(|snapshot| snapshot.working_set_bytes),
        before_peak_working_set_bytes = before.map(|snapshot| snapshot.peak_working_set_bytes),
        before_pagefile_usage_bytes = before.map(|snapshot| snapshot.pagefile_usage_bytes),
        before_private_usage_bytes = before.map(|snapshot| snapshot.private_usage_bytes),
        "terminal memory trim request"
    );
}

pub fn emit_terminal_memory_trim_executed(
    enabled: bool,
    session_id: Uuid,
    pending_output_bytes: usize,
    trim_succeeded: bool,
    before: Option<ProcessMemorySnapshot>,
    after: Option<ProcessMemorySnapshot>,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-executed",
        session_id = session_id.to_string(),
        pending_output_bytes,
        trim_succeeded,
        before_working_set_bytes = before.map(|snapshot| snapshot.working_set_bytes),
        before_peak_working_set_bytes = before.map(|snapshot| snapshot.peak_working_set_bytes),
        before_pagefile_usage_bytes = before.map(|snapshot| snapshot.pagefile_usage_bytes),
        before_private_usage_bytes = before.map(|snapshot| snapshot.private_usage_bytes),
        after_working_set_bytes = after.map(|snapshot| snapshot.working_set_bytes),
        after_peak_working_set_bytes = after.map(|snapshot| snapshot.peak_working_set_bytes),
        after_pagefile_usage_bytes = after.map(|snapshot| snapshot.pagefile_usage_bytes),
        after_private_usage_bytes = after.map(|snapshot| snapshot.private_usage_bytes),
        "terminal memory trim executed"
    );
}

pub fn emit_terminal_memory_trim_skipped(
    enabled: bool,
    session_id: Uuid,
    pending_output_bytes: usize,
    trim_threshold_bytes: usize,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-skipped",
        session_id = session_id.to_string(),
        pending_output_bytes,
        trim_threshold_bytes,
        "terminal memory trim skipped"
    );
}

pub fn build_test_logging_runtime(
    paths: &LoggingPaths,
    config: &AppLoggingConfig,
) -> Result<TestLoggingRuntime> {
    let file_appender = tracing_appender::rolling::never(&paths.logs_dir, "system-error.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(EnvFilter::new(config.filter_directive()))
        .with_writer(writer)
        .finish();

    Ok(TestLoggingRuntime {
        dispatch: tracing::Dispatch::new(subscriber),
        guard,
    })
}

pub fn try_init_global_logging() -> Result<AppLoggingRuntime> {
    let paths = resolve_logging_paths_for_app()?;
    let config = AppLoggingConfig::from_env();
    let file_appender = tracing_appender::rolling::daily(&paths.logs_dir, "system-error.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);
    let subscriber = tracing_subscriber::fmt()
        .with_ansi(false)
        .with_target(true)
        .with_env_filter(EnvFilter::new(config.filter_directive()))
        .with_writer(writer)
        .finish();

    tracing::subscriber::set_global_default(subscriber)?;

    if let Err(err) = cleanup_logging_dirs(
        &paths.logs_dir,
        &paths.crash_dir,
        CleanupPolicy {
            max_age: std::time::Duration::from_secs(60 * 60 * 24 * 14),
            max_total_bytes: 64 * 1024 * 1024,
        },
    ) {
        tracing::error!(
            target: "app.logging",
            error = %err,
            "failed to cleanup logging directories"
        );
    }

    Ok(AppLoggingRuntime { paths, guard })
}
