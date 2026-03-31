use anyhow::{Context, Result, anyhow};

use crate::app::vault::crypto::EncryptedSnapshot;
use crate::app::vault::model::{PackLayout, ProviderKind, VaultHead, VaultManifest};

pub mod gitee_gist;
pub mod github_gist;
pub mod gitlab_snippet;
pub mod mock;
pub mod s3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_conditional_head_write: bool,
    pub max_pack_count: usize,
    pub max_pack_bytes: usize,
    pub preferred_pack_strategy: PackLayout,
}

impl ProviderCapabilities {
    pub fn s3_like() -> Self {
        Self {
            supports_conditional_head_write: true,
            max_pack_count: 64,
            max_pack_bytes: 16 * 1024 * 1024,
            preferred_pack_strategy: PackLayout::ObjectSet,
        }
    }

    pub fn bundled_files_like() -> Self {
        Self {
            supports_conditional_head_write: false,
            max_pack_count: 4,
            max_pack_bytes: 1024 * 1024,
            preferred_pack_strategy: PackLayout::BundledFiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderReadResult {
    pub head: Option<VaultHead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderWriteRequest {
    pub head: VaultHead,
    pub manifest: VaultManifest,
    pub encrypted_snapshot: EncryptedSnapshot,
    pub expected_parent_revision: Option<String>,
    pub conditional_head_write: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRevision {
    pub head: VaultHead,
    pub manifest: VaultManifest,
    pub encrypted_snapshot: EncryptedSnapshot,
}

const SNAPSHOT_NONCE_HEX_KEY: &str = "snapshot.nonce_hex";
const SNAPSHOT_PLAINTEXT_LEN_KEY: &str = "snapshot.plaintext_len";
const SNAPSHOT_COMPRESSED_LEN_KEY: &str = "snapshot.compressed_len";
const SNAPSHOT_PAYLOAD_SHA256_KEY: &str = "snapshot.payload_sha256";

pub trait VaultProvider: Send + Sync {
    fn remote_id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn read_head(&self) -> Result<ProviderReadResult>;
    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision>;
    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()>;
}

pub fn first_release_formal_provider_kind() -> ProviderKind {
    ProviderKind::GiteeGist
}

pub fn first_release_formal_provider_label() -> &'static str {
    "Gitee"
}

pub fn first_release_formal_auth_label() -> &'static str {
    "Personal Access Token"
}

pub fn attach_snapshot_recovery_metadata(
    manifest: &mut VaultManifest,
    encrypted_snapshot: &EncryptedSnapshot,
) {
    manifest.provider_capability_fallbacks.insert(
        SNAPSHOT_NONCE_HEX_KEY.into(),
        encode_hex(encrypted_snapshot.nonce.as_slice()),
    );
    manifest.provider_capability_fallbacks.insert(
        SNAPSHOT_PLAINTEXT_LEN_KEY.into(),
        encrypted_snapshot.plaintext_len.to_string(),
    );
    manifest.provider_capability_fallbacks.insert(
        SNAPSHOT_COMPRESSED_LEN_KEY.into(),
        encrypted_snapshot.compressed_len.to_string(),
    );
    manifest.provider_capability_fallbacks.insert(
        SNAPSHOT_PAYLOAD_SHA256_KEY.into(),
        encrypted_snapshot.payload_sha256.clone(),
    );
}

pub fn rebuild_snapshot_from_manifest(
    head: &VaultHead,
    manifest: &VaultManifest,
    ciphertext: Vec<u8>,
) -> Result<EncryptedSnapshot> {
    let nonce = decode_hex(required_snapshot_metadata(
        manifest,
        SNAPSHOT_NONCE_HEX_KEY,
        &head.vault_revision,
    )?)
    .with_context(|| {
        format!(
            "remote revision `{}` has invalid bundled snapshot nonce metadata",
            head.vault_revision
        )
    })?;
    let plaintext_len =
        required_snapshot_metadata(manifest, SNAPSHOT_PLAINTEXT_LEN_KEY, &head.vault_revision)?
            .parse::<usize>()
            .with_context(|| {
                format!(
                    "remote revision `{}` has invalid bundled snapshot plaintext length metadata",
                    head.vault_revision
                )
            })?;
    let compressed_len =
        required_snapshot_metadata(manifest, SNAPSHOT_COMPRESSED_LEN_KEY, &head.vault_revision)?
            .parse::<usize>()
            .with_context(|| {
                format!(
                    "remote revision `{}` has invalid bundled snapshot compressed length metadata",
                    head.vault_revision
                )
            })?;
    let payload_sha256 = manifest
        .provider_capability_fallbacks
        .get(SNAPSHOT_PAYLOAD_SHA256_KEY)
        .cloned()
        .or_else(|| head.payload_hash.strip_prefix("sha256:").map(ToOwned::to_owned))
        .ok_or_else(|| {
            anyhow!(
                "legacy remote revision `{}` is missing bundled snapshot recovery metadata. Sync once from another device on the current build before using remote recovery.",
                head.vault_revision
            )
        })?;

    Ok(EncryptedSnapshot {
        cipher: head.cipher,
        compression: head.compression,
        nonce,
        ciphertext,
        plaintext_len,
        compressed_len,
        payload_sha256,
    })
}

fn required_snapshot_metadata<'a>(
    manifest: &'a VaultManifest,
    key: &'static str,
    revision: &str,
) -> Result<&'a str> {
    manifest
        .provider_capability_fallbacks
        .get(key)
        .map(String::as_str)
        .ok_or_else(|| {
            anyhow!(
                "legacy remote revision `{revision}` is missing bundled snapshot recovery metadata. Sync once from another device on the current build before using remote recovery."
            )
        })
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(anyhow!("hex string must have an even number of characters"));
    }

    let mut bytes = Vec::with_capacity(value.len() / 2);
    let chars = value.as_bytes().chunks_exact(2);
    for pair in chars {
        let hex = std::str::from_utf8(pair).context("hex metadata is not valid UTF-8")?;
        let byte = u8::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hex byte `{hex}` in metadata"))?;
        bytes.push(byte);
    }

    Ok(bytes)
}
