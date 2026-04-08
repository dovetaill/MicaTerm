//! Creates the tracing runtime, log writer guard, and startup metadata emission helpers.

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

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

pub fn emit_terminal_memory_startup_snapshot(
    enabled: bool,
    profile: AppRuntimeProfile,
    stats: TerminalPresenterCacheStats,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "startup-snapshot",
        build_flavor = ?profile.build_flavor,
        terminal_render_mode = profile.terminal_render_mode_label(),
        terminal_subsystem_mode = profile.terminal_subsystem_mode_label(),
        previous_frame_rows = stats.previous_frame_rows,
        previous_shaped_rows = stats.previous_shaped_rows,
        shaped_row_cache_entries = stats.shaped_row_cache_entries,
        shaped_row_cache_capacity = stats.shaped_row_cache_capacity,
        mono_glyph_cache_entries = stats.mono_glyph_cache_entries,
        color_glyph_cache_entries = stats.color_glyph_cache_entries,
        glyph_raster_cache_entries = stats.glyph_raster_cache_entries,
        prepared_row_cache_entries = stats.prepared_row_cache_entries,
        scene_image_mono_glyph_cache_entries = stats.scene_image_mono_glyph_cache_entries,
        scene_image_color_glyph_cache_entries = stats.scene_image_color_glyph_cache_entries,
        scene_image_last_base_pixels_bytes = stats.scene_image_last_base_pixels_bytes,
        scene_image_working_pixels_bytes = stats.scene_image_working_pixels_bytes,
        "terminal memory startup snapshot"
    );
}

pub fn emit_terminal_memory_surface_refresh(
    enabled: bool,
    reason: &str,
    render_mode: &str,
    seqno: u64,
    stats: TerminalPresenterCacheStats,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "surface-refresh",
        reason,
        render_mode,
        seqno,
        previous_frame_rows = stats.previous_frame_rows,
        previous_shaped_rows = stats.previous_shaped_rows,
        shaped_row_cache_entries = stats.shaped_row_cache_entries,
        shaped_row_cache_capacity = stats.shaped_row_cache_capacity,
        mono_glyph_cache_entries = stats.mono_glyph_cache_entries,
        color_glyph_cache_entries = stats.color_glyph_cache_entries,
        glyph_raster_cache_entries = stats.glyph_raster_cache_entries,
        prepared_row_cache_entries = stats.prepared_row_cache_entries,
        scene_image_mono_glyph_cache_entries = stats.scene_image_mono_glyph_cache_entries,
        scene_image_color_glyph_cache_entries = stats.scene_image_color_glyph_cache_entries,
        scene_image_last_base_pixels_bytes = stats.scene_image_last_base_pixels_bytes,
        scene_image_working_pixels_bytes = stats.scene_image_working_pixels_bytes,
        "terminal memory surface refresh"
    );
}

pub fn emit_terminal_memory_trim_request(enabled: bool, pending_output_bytes: usize) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-request",
        pending_output_bytes,
        "terminal memory trim request"
    );
}

pub fn emit_terminal_memory_trim_executed(
    enabled: bool,
    pending_output_bytes: usize,
    trim_succeeded: bool,
) {
    if !enabled {
        return;
    }

    tracing::debug!(
        target: "app.memory",
        event = "trim-executed",
        pending_output_bytes,
        trim_succeeded,
        "terminal memory trim executed"
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
