#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use mica_term::app::runtime_profile::AppRuntimeProfile;

fn select_runtime_profile() -> AppRuntimeProfile {
    AppRuntimeProfile::mainline()
}

fn apply_renderer_selector(_profile: AppRuntimeProfile) -> anyhow::Result<()> {
    use anyhow::Context;
    use slint::{BackendSelector, wgpu_28::WGPUConfiguration};

    #[cfg(target_os = "windows")]
    let wgpu_configuration = {
        let mut settings = slint::wgpu_28::WGPUSettings::default();
        settings.backends = slint::wgpu_28::wgpu::Backends::DX12;
        tracing::info!(
            target: "app.renderer",
            requested_backends = ?settings.backends,
            "configuring wgpu backend preferences for femtovg renderer"
        );
        WGPUConfiguration::Automatic(settings)
    };

    #[cfg(not(target_os = "windows"))]
    let wgpu_configuration = WGPUConfiguration::default();

    let selector = BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg-wgpu".into())
        .require_wgpu_28(wgpu_configuration);

    #[cfg(target_os = "windows")]
    let selector = {
        tracing::info!(
            target: "app.renderer",
            transparent_window = false,
            reason = "wgpu_surface_reports_opaque_alpha_only",
            "configuring winit window attributes for femtovg renderer"
        );
        selector.with_winit_window_attributes_hook(|attributes| attributes.with_transparent(false))
    };

    selector
        .select()
        .map_err(anyhow::Error::from)
        .context("failed to select winit-femtovg-wgpu backend for mainline runtime")
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
