//! Minimal platform URL opener with a test hook for integration coverage.

use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};

type UrlOpenHandler = dyn Fn(&str) -> Result<()> + Send + Sync + 'static;

static URL_OPEN_HANDLER: OnceLock<Mutex<Arc<UrlOpenHandler>>> = OnceLock::new();

fn url_open_handler() -> &'static Mutex<Arc<UrlOpenHandler>> {
    URL_OPEN_HANDLER.get_or_init(|| Mutex::new(Arc::new(open_url_with_platform_shell)))
}

pub fn open_url(url: &str) -> Result<()> {
    let handler = url_open_handler()
        .lock()
        .expect("lock URL open handler")
        .clone();
    handler(url)
}

#[doc(hidden)]
pub struct UrlOpenHandlerGuard {
    previous: Option<Arc<UrlOpenHandler>>,
}

impl Drop for UrlOpenHandlerGuard {
    fn drop(&mut self) {
        let Some(previous) = self.previous.take() else {
            return;
        };
        *url_open_handler().lock().expect("lock URL open handler") = previous;
    }
}

#[doc(hidden)]
pub fn install_open_url_handler_for_test<F>(handler: F) -> UrlOpenHandlerGuard
where
    F: Fn(&str) -> Result<()> + Send + Sync + 'static,
{
    let mut slot = url_open_handler().lock().expect("lock URL open handler");
    let previous = slot.clone();
    *slot = Arc::new(handler);
    UrlOpenHandlerGuard {
        previous: Some(previous),
    }
}

fn open_url_with_platform_shell(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .with_context(|| format!("failed to open URL `{url}` with the Windows shell"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .with_context(|| format!("failed to open URL `{url}` with `open`"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .with_context(|| format!("failed to open URL `{url}` with `xdg-open`"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}
