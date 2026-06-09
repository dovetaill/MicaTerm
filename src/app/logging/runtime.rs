//! Creates the tracing runtime, log writer guard, and startup metadata emission helpers.

use anyhow::Result;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

use crate::app::memory::{ProcessMemorySnapshot, current_process_memory_snapshot};
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MemoryDiagnosticsEvent {
    pub event_name: &'static str,
    pub trigger_reason: Option<&'static str>,
    pub active_renderer_mode: Option<&'static str>,
    pub has_active_surface: Option<bool>,
    pub pending_output_bytes: Option<usize>,
    pub idle_interval_ms: Option<u64>,
    pub no_surface_idle_ms: Option<u64>,
    pub trim_succeeded: Option<bool>,
    pub backend_purge_succeeded: Option<bool>,
    pub retained_renderer_resources_before: Option<bool>,
    pub retained_renderer_resources_after: Option<bool>,
    pub before_memory: Option<ProcessMemorySnapshot>,
    pub after_memory: Option<ProcessMemorySnapshot>,
    pub cache_stats_before: Option<TerminalPresenterCacheStats>,
    pub cache_stats_after: Option<TerminalPresenterCacheStats>,
}

pub fn emit_startup_memory_snapshot(profile: AppRuntimeProfile) {
    emit_startup_memory_snapshot_with_config(profile, AppLoggingConfig::from_env());
}

pub fn emit_startup_memory_snapshot_with_config(
    profile: AppRuntimeProfile,
    config: AppLoggingConfig,
) {
    if !config.memory_diagnostics_enabled() {
        return;
    }

    let snapshot = current_process_memory_snapshot();
    let snapshot_available = snapshot.is_some();
    let snapshot = snapshot.unwrap_or_default();

    tracing::debug!(
        target: "app.memory",
        event = "startup-snapshot",
        requested_build_flavor = profile.build_flavor_label(),
        requested_terminal_render_mode = profile.terminal_render_mode_label(),
        requested_native_present_path = profile.native_present_path_label(),
        active_renderer_mode = profile.terminal_render_mode_label(),
        memory_snapshot_available = snapshot_available,
        working_set_bytes = snapshot.working_set_bytes,
        peak_working_set_bytes = snapshot.peak_working_set_bytes,
        pagefile_usage_bytes = snapshot.pagefile_usage_bytes,
        private_usage_bytes = snapshot.private_usage_bytes,
        "startup memory snapshot"
    );
}

pub fn emit_memory_diagnostics_event(profile: AppRuntimeProfile, event: MemoryDiagnosticsEvent) {
    emit_memory_diagnostics_event_with_config(profile, AppLoggingConfig::from_env(), event);
}

pub fn emit_memory_diagnostics_event_with_config(
    profile: AppRuntimeProfile,
    config: AppLoggingConfig,
    event: MemoryDiagnosticsEvent,
) {
    if !config.memory_diagnostics_enabled() {
        return;
    }

    let before_memory_snapshot_available = event.before_memory.is_some();
    let after_memory_snapshot_available = event.after_memory.is_some();
    let before_memory = event.before_memory.unwrap_or_default();
    let after_memory = event.after_memory.unwrap_or_default();
    let cache_stats_before = event.cache_stats_before.unwrap_or_default();
    let cache_stats_after = event.cache_stats_after.unwrap_or_default();

    tracing::debug!(
        target: "app.memory",
        event = event.event_name,
        requested_build_flavor = profile.build_flavor_label(),
        requested_terminal_render_mode = profile.terminal_render_mode_label(),
        requested_native_present_path = profile.native_present_path_label(),
        active_renderer_mode = event
            .active_renderer_mode
            .unwrap_or(profile.terminal_render_mode_label()),
        trigger_reason = event.trigger_reason.unwrap_or("unspecified"),
        has_active_surface = ?event.has_active_surface,
        pending_output_bytes = ?event.pending_output_bytes,
        idle_interval_ms = ?event.idle_interval_ms,
        no_surface_idle_ms = ?event.no_surface_idle_ms,
        trim_succeeded = ?event.trim_succeeded,
        backend_purge_succeeded = ?event.backend_purge_succeeded,
        retained_renderer_resources_before = ?event.retained_renderer_resources_before,
        retained_renderer_resources_after = ?event.retained_renderer_resources_after,
        before_memory_snapshot_available,
        before_working_set_bytes = before_memory.working_set_bytes,
        before_peak_working_set_bytes = before_memory.peak_working_set_bytes,
        before_pagefile_usage_bytes = before_memory.pagefile_usage_bytes,
        before_private_usage_bytes = before_memory.private_usage_bytes,
        after_memory_snapshot_available,
        after_working_set_bytes = after_memory.working_set_bytes,
        after_peak_working_set_bytes = after_memory.peak_working_set_bytes,
        after_pagefile_usage_bytes = after_memory.pagefile_usage_bytes,
        after_private_usage_bytes = after_memory.private_usage_bytes,
        cache_before_previous_frame_rows = cache_stats_before.previous_frame_rows,
        cache_before_previous_shaped_rows = cache_stats_before.previous_shaped_rows,
        cache_before_shaped_row_cache_entries = cache_stats_before.shaped_row_cache_entries,
        cache_before_shaped_row_cache_capacity = cache_stats_before.shaped_row_cache_capacity,
        cache_before_mono_glyph_cache_entries = cache_stats_before.mono_glyph_cache_entries,
        cache_before_color_glyph_cache_entries = cache_stats_before.color_glyph_cache_entries,
        cache_before_glyph_raster_cache_entries = cache_stats_before.glyph_raster_cache_entries,
        cache_before_prepared_row_cache_entries = cache_stats_before.prepared_row_cache_entries,
        cache_after_previous_frame_rows = cache_stats_after.previous_frame_rows,
        cache_after_previous_shaped_rows = cache_stats_after.previous_shaped_rows,
        cache_after_shaped_row_cache_entries = cache_stats_after.shaped_row_cache_entries,
        cache_after_shaped_row_cache_capacity = cache_stats_after.shaped_row_cache_capacity,
        cache_after_mono_glyph_cache_entries = cache_stats_after.mono_glyph_cache_entries,
        cache_after_color_glyph_cache_entries = cache_stats_after.color_glyph_cache_entries,
        cache_after_glyph_raster_cache_entries = cache_stats_after.glyph_raster_cache_entries,
        cache_after_prepared_row_cache_entries = cache_stats_after.prepared_row_cache_entries,
        "terminal memory diagnostic event"
    );
}

pub fn emit_runtime_profile_metadata(_profile: AppRuntimeProfile) {
    // Startup profile details were useful while stabilizing renderer selection,
    // but they now duplicate the final renderer-selection summary without adding
    // actionable signal to packaged logs.
}

pub fn emit_app_root_metadata(_paths: &LoggingPaths) {
    // Packaged app-root discovery is stable enough that repeating the resolved
    // directories on every launch no longer helps routine diagnostics.
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
