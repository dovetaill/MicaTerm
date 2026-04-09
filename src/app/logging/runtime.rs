//! Creates the tracing runtime, log writer guard, and startup metadata emission helpers.

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

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

pub fn emit_terminal_memory_cleanup_result(
    enabled: bool,
    event: &str,
    reason: &str,
    no_surface_idle_ms: u128,
    backend_purge_attempted: bool,
    backend_purge_succeeded: bool,
    process_trim_attempted: bool,
    process_trim_succeeded: bool,
    before: Option<ProcessMemorySnapshot>,
    after: Option<ProcessMemorySnapshot>,
) {
    if !enabled {
        return;
    }

    let before = before.unwrap_or_default();
    let after = after.unwrap_or_default();

    tracing::debug!(
        target: "app.memory",
        event,
        reason,
        no_surface_idle_ms,
        before_snapshot_available = before != ProcessMemorySnapshot::default(),
        after_snapshot_available = after != ProcessMemorySnapshot::default(),
        backend_purge_attempted,
        backend_purge_succeeded,
        process_trim_attempted,
        process_trim_succeeded,
        before_working_set_bytes = before.working_set_bytes,
        before_peak_working_set_bytes = before.peak_working_set_bytes,
        before_pagefile_usage_bytes = before.pagefile_usage_bytes,
        before_private_usage_bytes = before.private_usage_bytes,
        after_working_set_bytes = after.working_set_bytes,
        after_peak_working_set_bytes = after.peak_working_set_bytes,
        after_pagefile_usage_bytes = after.pagefile_usage_bytes,
        after_private_usage_bytes = after.private_usage_bytes,
        "terminal memory cleanup result"
    );
}

pub fn emit_terminal_memory_idle_transition(
    enabled: bool,
    event: &str,
    reason: &str,
    no_surface_idle_ms: u128,
    idle_threshold_ms: u64,
    renderer_resources_retained: bool,
    idle_cache_shrunk: bool,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event,
        reason,
        no_surface_idle_ms,
        idle_threshold_ms,
        renderer_resources_retained,
        idle_cache_shrunk,
        "terminal memory idle transition"
    );
}

pub fn emit_terminal_memory_surface_transition(
    enabled: bool,
    event: &str,
    reason: &str,
    had_active_surface: bool,
    has_active_surface: bool,
    surface_disappeared: bool,
    session_id: Option<&str>,
) {
    if !enabled {
        return;
    }

    if let Some(session_id) = session_id {
        tracing::debug!(
            target: "app.memory",
            event,
            reason,
            had_active_surface,
            has_active_surface,
            surface_disappeared,
            session_id,
            "terminal memory surface transition"
        );
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event,
        reason,
        had_active_surface,
        has_active_surface,
        surface_disappeared,
        "terminal memory surface transition"
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
