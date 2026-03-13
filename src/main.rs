#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use mica_term::app::runtime_profile::AppRuntimeProfile;

fn select_runtime_profile() -> AppRuntimeProfile {
    #[cfg(feature = "femtovg-wgpu-experimental")]
    {
        return AppRuntimeProfile::femtovg_wgpu_experimental();
    }

    #[cfg(not(feature = "femtovg-wgpu-experimental"))]
    {
        AppRuntimeProfile::formal()
    }
}

#[cfg(feature = "femtovg-wgpu-experimental")]
fn apply_renderer_selector(profile: AppRuntimeProfile) -> anyhow::Result<()> {
    use anyhow::Context;
    use slint::{BackendSelector, wgpu_28::WGPUConfiguration};

    if !profile.is_experimental() {
        return Ok(());
    }

    BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_28(WGPUConfiguration::default())
        .select()
        .map_err(anyhow::Error::from)
        .context("failed to select winit-femtovg-wgpu backend for femtovg-wgpu-experimental")
}

#[cfg(not(feature = "femtovg-wgpu-experimental"))]
fn apply_renderer_selector(_profile: AppRuntimeProfile) -> anyhow::Result<()> {
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let profile = select_runtime_profile();
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

    if let Err(err) = mica_term::app::bootstrap::run_with_profile(profile) {
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
