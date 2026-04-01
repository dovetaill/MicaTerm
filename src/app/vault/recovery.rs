use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::app::vault::model::VaultSnapshot;

const RECOVERY_FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RecoverySource {
    LocalBeforePull,
    RemoteBeforePush,
    LocalConflictCopy,
    RemoteConflictCopy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverySnapshotRecord {
    pub format_version: u32,
    pub vault_id: String,
    pub source: RecoverySource,
    pub captured_at: String,
    pub base_revision: Option<String>,
    pub losing_revision: Option<String>,
    pub payload_hash: Option<String>,
    pub snapshot: VaultSnapshot,
}

pub fn persist_recovery_snapshot(
    recovery_root: &Path,
    record: &RecoverySnapshotRecord,
) -> Result<PathBuf> {
    ensure!(
        !record.vault_id.trim().is_empty(),
        "recovery snapshot record requires a non-empty vault_id"
    );
    let vault_dir = recovery_root.join(record.vault_id.as_str());
    fs::create_dir_all(&vault_dir).with_context(|| {
        format!(
            "failed to create recovery snapshot directory `{}`",
            vault_dir.display()
        )
    })?;

    let path = vault_dir.join(recovery_file_name(record));
    let encoded =
        serde_json::to_vec_pretty(record).context("failed to encode recovery snapshot record")?;
    fs::write(&path, encoded).with_context(|| {
        format!(
            "failed to persist recovery snapshot record `{}`",
            path.display()
        )
    })?;

    Ok(path)
}

pub fn load_recovery_snapshots(
    recovery_root: &Path,
    vault_id: &str,
) -> Result<Vec<RecoverySnapshotRecord>> {
    let vault_dir = recovery_root.join(vault_id);
    if !vault_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&vault_dir)
        .with_context(|| {
            format!(
                "failed to read recovery snapshot directory `{}`",
                vault_dir.display()
            )
        })?
        .collect::<std::result::Result<Vec<_>, _>>()
        .with_context(|| {
            format!(
                "failed to enumerate recovery snapshot directory `{}`",
                vault_dir.display()
            )
        })?;
    entries.sort_by_key(|entry| entry.path());

    let mut snapshots = Vec::with_capacity(entries.len());
    for entry in entries {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let encoded = fs::read(&path).with_context(|| {
            format!("failed to read recovery snapshot record `{}`", path.display())
        })?;
        let record = serde_json::from_slice(encoded.as_slice()).with_context(|| {
            format!(
                "failed to decode recovery snapshot record `{}`",
                path.display()
            )
        })?;
        snapshots.push(record);
    }

    Ok(snapshots)
}

fn recovery_file_name(record: &RecoverySnapshotRecord) -> String {
    let source = match record.source {
        RecoverySource::LocalBeforePull => "local-before-pull",
        RecoverySource::RemoteBeforePush => "remote-before-push",
        RecoverySource::LocalConflictCopy => "local-conflict-copy",
        RecoverySource::RemoteConflictCopy => "remote-conflict-copy",
    };
    let revision = record
        .losing_revision
        .as_deref()
        .filter(|revision| !revision.trim().is_empty())
        .unwrap_or("unversioned");
    format!("{}-{}-{}.json", record.captured_at, source, revision)
}

impl RecoverySnapshotRecord {
    pub fn new(
        vault_id: String,
        source: RecoverySource,
        captured_at: String,
        base_revision: Option<String>,
        losing_revision: Option<String>,
        payload_hash: Option<String>,
        snapshot: VaultSnapshot,
    ) -> Self {
        Self {
            format_version: RECOVERY_FORMAT_VERSION,
            vault_id,
            source,
            captured_at,
            base_revision,
            losing_revision,
            payload_hash,
            snapshot,
        }
    }
}
