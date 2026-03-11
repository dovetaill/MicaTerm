#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
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

    if let Err(err) = mica_term::app::bootstrap::run() {
        if let Some(runtime) = &logging {
            let _ = mica_term::app::logging::panic::write_fatal_record(
                &runtime.paths.crash_dir,
                "bootstrap.run",
                &err.to_string(),
            );
        } else {
            eprintln!("fatal bootstrap error: {err}");
        }
        return Err(err);
    }

    Ok(())
}
