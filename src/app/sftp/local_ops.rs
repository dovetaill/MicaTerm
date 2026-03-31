//! Local filesystem helpers for SFTP upload and download requests.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTransferEntry {
    pub local_path: PathBuf,
    pub relative_path: PathBuf,
    pub bytes_total: u64,
}

pub fn scan_local_sources(paths: &[PathBuf]) -> Result<Vec<LocalTransferEntry>> {
    let mut entries = Vec::new();
    for path in paths {
        let metadata = fs::metadata(path)
            .with_context(|| format!("failed to inspect local path `{}`", path.display()))?;
        if metadata.is_dir() {
            scan_directory(path.as_path(), path.as_path(), &mut entries)?;
        } else if metadata.is_file() {
            entries.push(LocalTransferEntry {
                local_path: path.clone(),
                relative_path: PathBuf::from(
                    path.file_name()
                        .map(|value| value.to_string_lossy().to_string())
                        .unwrap_or_default(),
                ),
                bytes_total: metadata.len(),
            });
        }
    }

    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(entries)
}

pub fn build_remote_upload_path(target_dir: &str, relative_path: &Path) -> String {
    let suffix = relative_path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy().replace('\\', "/")),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/");

    if suffix.is_empty() {
        return normalize_remote_dir(target_dir);
    }

    let prefix = normalize_remote_dir(target_dir);
    if prefix == "/" {
        format!("/{}", suffix.trim_start_matches('/'))
    } else {
        format!(
            "{}/{}",
            prefix.trim_end_matches('/'),
            suffix.trim_start_matches('/')
        )
    }
}

pub fn build_local_download_path(local_root: &Path, remote_path: &str) -> PathBuf {
    let name = remote_path
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or("downloaded-file");
    local_root.join(name)
}

fn scan_directory(
    root: &Path,
    current: &Path,
    entries: &mut Vec<LocalTransferEntry>,
) -> Result<()> {
    let directory = fs::read_dir(current)
        .with_context(|| format!("failed to read local directory `{}`", current.display()))?;

    for child in directory {
        let child = child.with_context(|| {
            format!(
                "failed to read local directory entry in `{}`",
                current.display()
            )
        })?;
        let path = child.path();
        let metadata = child
            .metadata()
            .with_context(|| format!("failed to inspect local path `{}`", path.display()))?;
        if metadata.is_dir() {
            scan_directory(root, path.as_path(), entries)?;
        } else if metadata.is_file() {
            let relative_path = path
                .strip_prefix(root)
                .with_context(|| {
                    format!(
                        "failed to compute local relative path for `{}` against `{}`",
                        path.display(),
                        root.display()
                    )
                })?
                .to_path_buf();
            entries.push(LocalTransferEntry {
                local_path: path,
                relative_path,
                bytes_total: metadata.len(),
            });
        }
    }

    Ok(())
}

fn normalize_remote_dir(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        "/".into()
    } else {
        format!("/{}", trimmed.trim_matches('/'))
    }
}
