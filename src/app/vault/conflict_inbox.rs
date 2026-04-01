use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictInboxEntry {
    pub vault_id: String,
    pub target_id: String,
    pub conflict_kind: String,
    pub local_device_id: String,
    pub remote_device_id: String,
    pub captured_at: String,
}

pub fn persist_conflict_entries(
    conflict_root: &Path,
    entries: &[ConflictInboxEntry],
) -> Result<Vec<PathBuf>> {
    let mut persisted_paths = Vec::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        ensure!(
            !entry.vault_id.trim().is_empty(),
            "conflict inbox entry requires a non-empty vault_id"
        );
        ensure!(
            !entry.target_id.trim().is_empty(),
            "conflict inbox entry requires a non-empty target_id"
        );

        let vault_dir = conflict_root.join(entry.vault_id.as_str());
        fs::create_dir_all(&vault_dir).with_context(|| {
            format!(
                "failed to create conflict inbox directory `{}`",
                vault_dir.display()
            )
        })?;

        let path = vault_dir.join(conflict_file_name(entry, index));
        let encoded =
            serde_json::to_vec_pretty(entry).context("failed to encode conflict inbox entry")?;
        fs::write(&path, encoded).with_context(|| {
            format!(
                "failed to persist conflict inbox entry `{}`",
                path.display()
            )
        })?;
        persisted_paths.push(path);
    }

    Ok(persisted_paths)
}

pub fn load_conflict_entries(conflict_root: &Path, vault_id: &str) -> Result<Vec<ConflictInboxEntry>> {
    let vault_dir = conflict_root.join(vault_id);
    if !vault_dir.exists() {
        return Ok(Vec::new());
    }

    let mut directory_entries = fs::read_dir(&vault_dir)
        .with_context(|| {
            format!(
                "failed to read conflict inbox directory `{}`",
                vault_dir.display()
            )
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to enumerate conflict inbox directory `{}`",
                vault_dir.display()
            )
        })?;
    directory_entries.sort_by_key(|entry| entry.path());

    let mut entries = Vec::with_capacity(directory_entries.len());
    for directory_entry in directory_entries {
        let path = directory_entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let encoded = fs::read(&path)
            .with_context(|| format!("failed to read conflict inbox entry `{}`", path.display()))?;
        let entry: ConflictInboxEntry = serde_json::from_slice(encoded.as_slice()).with_context(|| {
            format!("failed to decode conflict inbox entry `{}`", path.display())
        })?;
        entries.push(entry);
    }

    entries.sort_by(|left, right| {
        right
            .captured_at
            .cmp(&left.captured_at)
            .then_with(|| left.target_id.cmp(&right.target_id))
            .then_with(|| left.conflict_kind.cmp(&right.conflict_kind))
    });
    Ok(entries)
}

fn conflict_file_name(entry: &ConflictInboxEntry, index: usize) -> String {
    format!(
        "{}-{}-{:04}.json",
        entry.captured_at,
        sanitize_path_component(entry.target_id.as_str()),
        index
    )
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|character| match character {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => character,
            _ => '_',
        })
        .collect()
}
