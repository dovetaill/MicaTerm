use std::backtrace::Backtrace;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

pub fn install_panic_hook(crash_dir: PathBuf) -> Result<()> {
    fs::create_dir_all(&crash_dir)?;
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        let _ = write_panic_record(&crash_dir, info);
        previous(info);
    }));

    Ok(())
}

pub fn write_fatal_record(crash_dir: &Path, phase: &str, error_text: &str) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("fatal-{}.log", unix_millis()));
    let backtrace = Backtrace::force_capture();
    let body = format!("phase={phase}\nerror={error_text}\nbacktrace={backtrace}\n");
    fs::write(&file_path, body)?;
    Ok(file_path)
}

fn write_panic_record(crash_dir: &Path, info: &std::panic::PanicHookInfo<'_>) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("panic-{}.log", unix_millis()));
    let location = info
        .location()
        .map(|loc| format!("{}:{}", loc.file(), loc.line()))
        .unwrap_or_else(|| "unknown".into());
    let payload = if let Some(text) = info.payload().downcast_ref::<&str>() {
        (*text).to_string()
    } else if let Some(text) = info.payload().downcast_ref::<String>() {
        text.clone()
    } else {
        "non-string panic payload".into()
    };
    let current_thread = std::thread::current();
    let thread = current_thread.name().unwrap_or("unnamed");
    let backtrace = Backtrace::force_capture();
    let body =
        format!("message={payload}\nlocation={location}\nthread={thread}\nbacktrace={backtrace}\n");
    fs::write(&file_path, body)?;
    Ok(file_path)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
