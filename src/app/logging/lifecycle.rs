//! Synchronous, low-volume breadcrumbs, independent of tracing filters and its queue.
//! An absent shutdown-complete record is evidence of incomplete shutdown, not proof of a crash.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

static JOURNAL: OnceLock<Mutex<File>> = OnceLock::new();

pub fn install(logs_dir: &Path) -> std::io::Result<()> {
    fs::create_dir_all(logs_dir)?;
    let file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(logs_dir.join(format!("lifecycle-{}-{}.log", millis(), std::process::id())))?;
    JOURNAL.set(Mutex::new(file)).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "lifecycle journal already installed",
        )
    })?;
    record("process-start");
    Ok(())
}

/// Only called on ordinary control-flow paths, never from a native exception handler.
pub fn record(event: &str) {
    let Some(journal) = JOURNAL.get() else { return };
    let Ok(mut file) = journal.lock() else { return };
    if let Err(error) = writeln!(
        file,
        "time_ms={} pid={} event={event}",
        millis(),
        std::process::id()
    )
    .and_then(|_| file.sync_data())
    {
        eprintln!("failed to write lifecycle journal: {error}");
    }
}

fn millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}
