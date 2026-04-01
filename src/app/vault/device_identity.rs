use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use uuid::Uuid;

const DEVICE_ID_FILE_NAME: &str = "device-id";
const GIT_REMOTE_CACHE_DIR_NAME: &str = "git-remotes";

pub fn load_or_create_device_id(root: &Path) -> Result<String> {
    fs::create_dir_all(root).with_context(|| {
        format!(
            "failed to create vault device identity root `{}`",
            root.display()
        )
    })?;

    let path = root.join(DEVICE_ID_FILE_NAME);
    if path.exists() {
        let existing = fs::read_to_string(&path)
            .with_context(|| format!("failed to read vault device identity `{}`", path.display()))?;
        let existing = existing.trim().to_string();
        if !existing.is_empty() {
            return Ok(existing);
        }
    }

    let device_id = format!("device-{}", Uuid::new_v4().simple());
    fs::write(&path, device_id.as_bytes())
        .with_context(|| format!("failed to persist vault device identity `{}`", path.display()))?;
    Ok(device_id)
}

pub fn git_remote_cache_dir(root: &Path, remote_id: &str) -> PathBuf {
    root.join(GIT_REMOTE_CACHE_DIR_NAME).join(remote_id)
}
