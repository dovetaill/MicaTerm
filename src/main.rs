#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Binary entrypoint that selects the runtime profile, initializes logging, and launches the UI.

use mica_term::app::{async_runtime::AppAsyncRuntime, runtime_profile::AppRuntimeProfile};

fn select_runtime_profile() -> AppRuntimeProfile {
    AppRuntimeProfile::mainline()
}

fn apply_renderer_selector(_profile: AppRuntimeProfile) -> anyhow::Result<()> {
    use anyhow::Context;
    use slint::BackendSelector;

    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("software".into())
        .select()
        .map_err(anyhow::Error::from)
        .context("failed to select winit-software backend for mainline runtime")
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
