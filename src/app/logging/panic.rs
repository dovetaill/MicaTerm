//! Panic hook installation and crash-record formatting for post-mortem diagnostics.

use std::backtrace::Backtrace;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;

pub fn install_panic_hook(crash_dir: PathBuf) -> Result<()> {
    fs::create_dir_all(&crash_dir)?;
    let previous = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        if let Err(err) = write_panic_record(&crash_dir, info) {
            eprintln!("failed to write panic record: {err}");
        }
        previous(info);
    }));

    Ok(())
}

pub fn write_fatal_record(crash_dir: &Path, phase: &str, error_text: &str) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("fatal-{}-{}.log", std::process::id(), unix_nanos()));
    let mut file = File::create(&file_path)?;
    writeln!(
        file,
        "pid={}\nphase={phase}\nerror={error_text}",
        std::process::id()
    )?;
    file.sync_data()?;
    writeln!(file, "backtrace={}", Backtrace::force_capture())?;
    file.sync_data()?;
    Ok(file_path)
}

fn write_panic_record(crash_dir: &Path, info: &std::panic::PanicHookInfo<'_>) -> Result<PathBuf> {
    fs::create_dir_all(crash_dir)?;
    let file_path = crash_dir.join(format!("panic-{}-{}.log", std::process::id(), unix_nanos()));
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
    // Persist the essentials before symbolization, which can itself fail or stall.
    let mut file = File::create(&file_path)?;
    writeln!(
        file,
        "pid={}\nmessage={payload}\nlocation={location}\nthread={thread}",
        std::process::id()
    )?;
    file.sync_data()?;
    writeln!(file, "backtrace={}", Backtrace::force_capture())?;
    file.sync_data()?;
    Ok(file_path)
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}
