use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use chacha20poly1305::{
    KeyInit, XChaCha20Poly1305, XNonce,
    aead::{Aead, Payload},
};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};

use crate::app::vault::model::{CipherKind, VaultSnapshot};

const RECOVERY_RECORD_FORMAT_VERSION: u32 = 1;
const RECOVERY_ENVELOPE_FORMAT_VERSION: u32 = 1;
const RECOVERY_KEY_LEN: usize = 32;
const RECOVERY_NONCE_LEN: usize = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EncryptedRecoverySnapshotRecord {
    format_version: u32,
    cipher: CipherKind,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
}

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
    vault_key: &[u8; RECOVERY_KEY_LEN],
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
    let encoded = encrypt_recovery_record(record, vault_key)?;
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
    vault_key: &[u8; RECOVERY_KEY_LEN],
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
        let Some(extension) = path.extension().and_then(|extension| extension.to_str()) else {
            continue;
        };
        let encoded = fs::read(&path).with_context(|| {
            format!(
                "failed to read recovery snapshot record `{}`",
                path.display()
            )
        })?;
        let record = match extension {
            "bin" => decrypt_recovery_record(encoded.as_slice(), vault_key).with_context(|| {
                format!(
                    "failed to decode encrypted recovery snapshot record `{}`",
                    path.display()
                )
            })?,
            "json" => serde_json::from_slice(encoded.as_slice()).with_context(|| {
                format!(
                    "failed to decode legacy recovery snapshot record `{}`",
                    path.display()
                )
            })?,
            _ => continue,
        };
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
    format!("{}-{}-{}.bin", record.captured_at, source, revision)
}

fn encrypt_recovery_record(
    record: &RecoverySnapshotRecord,
    vault_key: &[u8; RECOVERY_KEY_LEN],
) -> Result<Vec<u8>> {
    let plaintext =
        serde_json::to_vec(record).context("failed to encode recovery snapshot record")?;
    let mut nonce = [0u8; RECOVERY_NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher =
        XChaCha20Poly1305::new_from_slice(vault_key).context("invalid recovery encryption key")?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_slice(),
                aad: b"mica-term-vault-recovery",
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to encrypt recovery snapshot payload"))?;
    let envelope = EncryptedRecoverySnapshotRecord {
        format_version: RECOVERY_ENVELOPE_FORMAT_VERSION,
        cipher: CipherKind::XChaCha20Poly1305,
        nonce: nonce.to_vec(),
        ciphertext,
    };

    bincode::serialize(&envelope).context("failed to encode encrypted recovery snapshot record")
}

fn decrypt_recovery_record(
    encoded: &[u8],
    vault_key: &[u8; RECOVERY_KEY_LEN],
) -> Result<RecoverySnapshotRecord> {
    let envelope: EncryptedRecoverySnapshotRecord = bincode::deserialize(encoded)
        .context("failed to decode encrypted recovery snapshot envelope")?;
    ensure!(
        envelope.nonce.len() == RECOVERY_NONCE_LEN,
        "invalid recovery snapshot nonce length"
    );
    let cipher =
        XChaCha20Poly1305::new_from_slice(vault_key).context("invalid recovery encryption key")?;
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(envelope.nonce.as_slice()),
            Payload {
                msg: envelope.ciphertext.as_slice(),
                aad: b"mica-term-vault-recovery",
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to decrypt recovery snapshot payload"))?;

    serde_json::from_slice(plaintext.as_slice())
        .context("failed to decode decrypted recovery snapshot record")
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
            format_version: RECOVERY_RECORD_FORMAT_VERSION,
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
