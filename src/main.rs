#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use mica_term::app::runtime_profile::AppRuntimeProfile;

const SKIA_SOFTWARE_BACKEND: &str = "winit-skia-software";

fn select_runtime_profile() -> AppRuntimeProfile {
    #[cfg(feature = "windows-skia-experimental")]
    {
        AppRuntimeProfile::skia_experimental()
    }

    #[cfg(not(feature = "windows-skia-experimental"))]
    {
        AppRuntimeProfile::formal()
    }
}

fn apply_renderer_lock(profile: AppRuntimeProfile) {
    if profile.requires_backend_lock() {
        debug_assert_eq!(profile.forced_backend(), Some(SKIA_SOFTWARE_BACKEND));

        let requested_backend = std::env::var("SLINT_BACKEND").ok();
        if requested_backend.as_deref() != Some(SKIA_SOFTWARE_BACKEND) {
            tracing::warn!(
                target: "app.renderer",
                requested = ?requested_backend,
                forced = SKIA_SOFTWARE_BACKEND,
                "overriding conflicting SLINT_BACKEND for experimental profile"
            );
        }

        // Safety: startup is still single-threaded here and no UI/runtime has been initialized yet.
        unsafe {
            std::env::set_var("SLINT_BACKEND", SKIA_SOFTWARE_BACKEND);
        }
    }
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

    apply_renderer_lock(profile);
    mica_term::app::logging::runtime::emit_runtime_profile_metadata(profile);

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
