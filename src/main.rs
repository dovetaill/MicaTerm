#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Binary entrypoint that selects the runtime profile, initializes logging, and launches the UI.

use mica_term::app::{
    async_runtime::AppAsyncRuntime,
    runtime_profile::{AppRuntimeProfile, GraphicsApiRequirement, RendererMode},
};

#[derive(Clone, Copy, Debug)]
struct RendererSelectionAttempt {
    renderer_mode: RendererMode,
    graphics_api: Option<GraphicsApiRequirement>,
}

fn select_runtime_profile() -> AppRuntimeProfile {
    // Packaged wrappers now carry the native-only terminal contract through AppRuntimeProfile.
    AppRuntimeProfile::packaged()
}

fn renderer_selection_attempts(profile: AppRuntimeProfile) -> Vec<RendererSelectionAttempt> {
    profile
        .renderer_fallback_chain()
        .iter()
        .copied()
        .enumerate()
        .map(|(index, renderer_mode)| RendererSelectionAttempt {
            renderer_mode,
            graphics_api: if index == 0 {
                profile.preferred_graphics_api()
            } else {
                None
            },
        })
        .collect()
}

fn apply_renderer_selector(profile: AppRuntimeProfile) -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};
    use slint::BackendSelector;

    let backend_name = profile.forced_backend().unwrap_or("winit");
    let mut failures = Vec::new();

    for (fallback_level, attempt) in renderer_selection_attempts(profile).into_iter().enumerate() {
        let mut selector = BackendSelector::new()
            .backend_name(backend_name.into())
            .renderer_name(attempt.renderer_mode.renderer_name().into());
        if matches!(attempt.graphics_api, Some(GraphicsApiRequirement::Direct3D)) {
            selector = selector.require_d3d();
        }

        match selector.select() {
            Ok(()) => {
                tracing::info!(
                    target: "app.renderer",
                    selected_backend = backend_name,
                    selected_renderer = attempt.renderer_mode.renderer_name(),
                    selected_graphics_api = ?attempt.graphics_api.map(GraphicsApiRequirement::as_str),
                    fallback_level,
                    profile_selector = profile.selector_label(),
                    "selected renderer backend"
                );
                return Ok(());
            }
            Err(err) => {
                tracing::warn!(
                    target: "app.renderer",
                    attempted_backend = backend_name,
                    attempted_renderer = attempt.renderer_mode.renderer_name(),
                    attempted_graphics_api = ?attempt.graphics_api.map(GraphicsApiRequirement::as_str),
                    fallback_level,
                    error = %err,
                    "renderer selection attempt failed"
                );
                failures.push(format!(
                    "{}{}: {err}",
                    attempt.renderer_mode.renderer_name(),
                    if matches!(attempt.graphics_api, Some(GraphicsApiRequirement::Direct3D)) {
                        "+direct3d"
                    } else {
                        ""
                    }
                ));
            }
        }
    }

    Err(anyhow!(
        "renderer selection attempts exhausted: {}",
        failures.join(" | ")
    ))
    .with_context(|| {
        format!(
            "failed to select {} backend for packaged runtime with {} terminal rendering",
            profile.selector_label(),
            profile.terminal_render_mode_label(),
        )
    })
}

fn main() -> anyhow::Result<()> {
    let profile = select_runtime_profile();
    let async_runtime = AppAsyncRuntime::new()?;
    // Logging starts before UI initialization so startup failures and panic hooks always have a
    // stable place to write diagnostics.
    let logging = match mica_term::app::logging::runtime::try_init_global_logging() {
        Ok(runtime) => {
            if let Err(err) =
                mica_term::app::logging::panic::install_panic_hook(runtime.paths.crash_dir.clone())
            {
                tracing::error!(
                    target: "app.logging",
                    error = %err,
                    "failed to install panic hook"
                );
            }
            Some(runtime)
        }
        Err(err) => {
            eprintln!("failed to initialize system logging: {err}");
            None
        }
    };

    if let Some(runtime) = &logging {
        mica_term::app::logging::runtime::emit_app_root_metadata(&runtime.paths);
    }
    mica_term::app::logging::runtime::emit_runtime_profile_metadata(profile);
    apply_renderer_selector(profile)?;

    if let Err(err) = mica_term::app::bootstrap::run_with_profile(profile, async_runtime.handle()) {
        // Mirror fatal startup errors to stderr and the crash directory so failures remain visible
        // in both interactive and packaged launches.
        if let Some(message) =
            mica_term::app::bootstrap::startup_failure_message(profile, &err.to_string())
        {
            eprintln!("{message}");
        }

        if let Some(runtime) = &logging {
            let _ = mica_term::app::logging::panic::write_fatal_record(
                &runtime.paths.crash_dir,
                "bootstrap.run_with_profile",
                &err.to_string(),
            );
        } else {
            eprintln!("fatal bootstrap error: {err}");
        }
        return Err(err);
    }

    Ok(())
}
