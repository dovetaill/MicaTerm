use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::app::vault::crypto::EncryptedSnapshot;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct EncryptedCacheRecord {
    vault_id: String,
    snapshot: EncryptedSnapshot,
}

pub fn cache_path_for_vault(root: &Path, vault_id: &str) -> PathBuf {
    root.join(format!("vault-cache-{}.bin", short_vault_id_hash(vault_id)))
}

pub fn store_encrypted_cache(
    root: &Path,
    vault_id: &str,
    snapshot: &EncryptedSnapshot,
) -> Result<PathBuf> {
    let path = cache_path_for_vault(root, vault_id);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create vault cache directory `{}`", parent.display())
        })?;
    }

    let record = EncryptedCacheRecord {
        vault_id: vault_id.to_string(),
        snapshot: snapshot.clone(),
    };
    let encoded = bincode::serialize(&record).context("failed to encode encrypted cache record")?;
    fs::write(&path, encoded)
        .with_context(|| format!("failed to write encrypted cache file `{}`", path.display()))?;

    Ok(path)
}

pub fn load_encrypted_cache(root: &Path, vault_id: &str) -> Result<Option<EncryptedSnapshot>> {
    let path = cache_path_for_vault(root, vault_id);
    if !path.exists() {
        return Ok(None);
    }

    let encoded = fs::read(&path)
        .with_context(|| format!("failed to read encrypted cache file `{}`", path.display()))?;
    let record: EncryptedCacheRecord =
        bincode::deserialize(encoded.as_slice()).context("failed to decode encrypted cache record")?;

    if record.vault_id != vault_id {
        return Ok(None);
    }

    Ok(Some(record.snapshot))
}

fn short_vault_id_hash(vault_id: &str) -> String {
    let digest = Sha256::digest(vault_id.as_bytes());
    let mut output = String::with_capacity(16);
    for byte in digest.iter().take(8) {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
