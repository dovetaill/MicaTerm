//! Minimal platform URL opener with a test hook for integration coverage.

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
        spawn_windows_shell_open(url)
            .with_context(|| format!("failed to open URL `{url}` with the Windows shell"))?;
        return Ok(());
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .with_context(|| format!("failed to open URL `{url}` with `open`"))?;
        return Ok(());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .with_context(|| format!("failed to open URL `{url}` with `xdg-open`"))?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[cfg(target_os = "windows")]
fn spawn_windows_shell_open(url: &str) -> std::io::Result<()> {
    let operation = windows_wide("open");
    let target = windows_wide(url);
    let result = unsafe {
        ShellExecuteW(
            0,
            operation.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as usize <= 32 {
        return Err(std::io::Error::other(format!(
            "ShellExecuteW returned status code {result}"
        )));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

#[cfg(target_os = "windows")]
const SW_SHOWNORMAL: i32 = 1;

#[cfg(target_os = "windows")]
#[link(name = "shell32")]
unsafe extern "system" {
    fn ShellExecuteW(
        hwnd: isize,
        operation: *const u16,
        file: *const u16,
        parameters: *const u16,
        directory: *const u16,
        show_cmd: i32,
    ) -> isize;
}
