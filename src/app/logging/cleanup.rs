use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupPolicy {
    pub max_age: Duration,
    pub max_total_bytes: u64,
}

pub fn cleanup_logging_dirs(
    logs_dir: &Path,
    crash_dir: &Path,
    policy: CleanupPolicy,
) -> Result<()> {
    let cutoff = SystemTime::now() - policy.max_age;
    let mut entries = collect_entries(logs_dir)?;
    entries.extend(collect_entries(crash_dir)?);

    for (path, modified, _) in &entries {
        if *modified < cutoff {
            let _ = fs::remove_file(path);
        }
    }

    let mut entries = collect_entries(logs_dir)?;
    entries.extend(collect_entries(crash_dir)?);
    entries.sort_by_key(|(_, modified, _)| *modified);

    let mut total_size: u64 = entries.iter().map(|(_, _, len)| *len).sum();
    for (path, _, len) in entries {
        if total_size <= policy.max_total_bytes {
            break;
        }

        let _ = fs::remove_file(path);
        total_size = total_size.saturating_sub(len);
    }

    Ok(())
}

fn collect_entries(dir: &Path) -> Result<Vec<(PathBuf, SystemTime, u64)>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut out = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_file() {
            out.push((entry.path(), metadata.modified()?, metadata.len()));
        }
    }

    Ok(out)
}
