use std::marker::PhantomData;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::app::vault::crypto::EncryptedSnapshot;
use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, ProviderAuthKind, ProviderKind, VaultHead,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderRevision, ProviderWriteRequest, VaultProvider,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum S3CredentialMode {
    StandardChain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3VaultProviderConfig {
    pub remote_id: String,
    pub bucket: String,
    pub prefix: String,
    pub endpoint: Option<String>,
    pub region: Option<String>,
    pub force_path_style: bool,
    pub credential_mode: S3CredentialMode,
}

impl TryFrom<&BootstrapRemoteConfig> for S3VaultProviderConfig {
    type Error = anyhow::Error;

    fn try_from(remote: &BootstrapRemoteConfig) -> Result<Self> {
        if remote.provider != ProviderKind::S3Compatible {
            return Err(anyhow!(
                "bootstrap remote `{}` is not an S3-compatible provider",
                remote.remote_id
            ));
        }

        let BootstrapRemoteLocator::S3 {
            bucket,
            prefix,
            endpoint,
            region,
            force_path_style,
        } = &remote.locator
        else {
            return Err(anyhow!(
                "bootstrap remote `{}` is missing an S3 locator",
                remote.remote_id
            ));
        };

        if remote.auth_kind != ProviderAuthKind::AwsStandardChain {
            return Err(anyhow!(
                "bootstrap remote `{}` does not use the supported S3 auth mode",
                remote.remote_id
            ));
        }

        Ok(Self {
            remote_id: remote.remote_id.clone(),
            bucket: bucket.clone(),
            prefix: normalize_prefix(prefix),
            endpoint: endpoint.clone(),
            region: region.clone(),
            force_path_style: *force_path_style,
            credential_mode: S3CredentialMode::StandardChain,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3ObjectKeySet {
    pub head_key: String,
    pub manifest_key: String,
    pub pack_keys: Vec<String>,
}

impl S3ObjectKeySet {
    pub fn for_revision(prefix: &str, revision: &str, pack_count: usize) -> Self {
        let normalized_prefix = normalize_prefix(prefix);
        let head_key = join_s3_key(&normalized_prefix, "head.json");
        let manifest_key = join_s3_key(
            &normalized_prefix,
            format!("revisions/{revision}/manifest.bin").as_str(),
        );
        let pack_keys = (0..pack_count)
            .map(|index| {
                join_s3_key(
                    &normalized_prefix,
                    format!("revisions/{revision}/packs/pack-{index:04}.bin").as_str(),
                )
            })
            .collect();

        Self {
            head_key,
            manifest_key,
            pack_keys,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct S3PutObjectRequest {
    pub bucket: String,
    pub key: String,
    pub body: Vec<u8>,
    pub expected_current_revision: Option<String>,
}

pub trait S3ObjectStoreAdapter: Send + Sync {
    fn get_object(&self, bucket: &str, key: &str) -> Result<Option<Vec<u8>>>;
    fn put_object(&self, request: &S3PutObjectRequest) -> Result<()>;
    fn list_objects(&self, bucket: &str, prefix: &str) -> Result<Vec<String>>;
    fn delete_object(&self, bucket: &str, key: &str) -> Result<()>;
}

#[derive(Debug, Default)]
struct UnconfiguredS3ObjectStoreAdapter;

impl S3ObjectStoreAdapter for UnconfiguredS3ObjectStoreAdapter {
    fn get_object(&self, _bucket: &str, _key: &str) -> Result<Option<Vec<u8>>> {
        Err(anyhow!("S3 object store adapter is not configured"))
    }

    fn put_object(&self, _request: &S3PutObjectRequest) -> Result<()> {
        Err(anyhow!("S3 object store adapter is not configured"))
    }

    fn list_objects(&self, _bucket: &str, _prefix: &str) -> Result<Vec<String>> {
        Err(anyhow!("S3 object store adapter is not configured"))
    }

    fn delete_object(&self, _bucket: &str, _key: &str) -> Result<()> {
        Err(anyhow!("S3 object store adapter is not configured"))
    }
}

pub struct S3VaultProvider {
    config: S3VaultProviderConfig,
    object_store: Arc<dyn S3ObjectStoreAdapter>,
    _aws_behavior_marker: PhantomData<aws_config::BehaviorVersion>,
    _aws_client_marker: PhantomData<aws_sdk_s3::Client>,
}

impl S3VaultProvider {
    pub fn new(config: S3VaultProviderConfig) -> Result<Self> {
        Self::with_adapter(config, Arc::new(UnconfiguredS3ObjectStoreAdapter))
    }

    pub fn with_adapter(
        config: S3VaultProviderConfig,
        object_store: Arc<dyn S3ObjectStoreAdapter>,
    ) -> Result<Self> {
        if config.bucket.trim().is_empty() {
            return Err(anyhow!("S3 bucket must not be empty"));
        }

        Ok(Self {
            config,
            object_store,
            _aws_behavior_marker: PhantomData,
            _aws_client_marker: PhantomData,
        })
    }

    pub fn config(&self) -> &S3VaultProviderConfig {
        &self.config
    }

    pub fn object_keys_for_revision(&self, revision: &str, pack_count: usize) -> S3ObjectKeySet {
        S3ObjectKeySet::for_revision(&self.config.prefix, revision, pack_count)
    }
}

impl VaultProvider for S3VaultProvider {
    fn remote_id(&self) -> &str {
        self.config.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::s3_like()
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        let head_key = join_s3_key(&self.config.prefix, "head.json");
        let Some(bytes) = self
            .object_store
            .get_object(&self.config.bucket, &head_key)?
        else {
            return Ok(ProviderReadResult::default());
        };

        let head = serde_json::from_slice::<VaultHead>(bytes.as_slice()).with_context(|| {
            format!(
                "failed to decode S3 head object for remote `{}`",
                self.config.remote_id
            )
        })?;

        Ok(ProviderReadResult { head: Some(head) })
    }

    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision> {
        let keys = self.object_keys_for_revision(head.vault_revision.as_str(), 1);
        let manifest_bytes = self
            .object_store
            .get_object(&self.config.bucket, &keys.manifest_key)?
            .ok_or_else(|| {
                anyhow!(
                    "S3 manifest object is missing for remote `{}` revision `{}`",
                    self.config.remote_id,
                    head.vault_revision
                )
            })?;
        let manifest = bincode::deserialize::<crate::app::vault::model::VaultManifest>(
            manifest_bytes.as_slice(),
        )
        .with_context(|| {
            format!(
                "failed to decode S3 manifest for remote `{}` revision `{}`",
                self.config.remote_id, head.vault_revision
            )
        })?;

        let keys = self
            .object_keys_for_revision(head.vault_revision.as_str(), manifest.packs.len().max(1));
        let mut stored_packs = Vec::with_capacity(keys.pack_keys.len());
        for key in &keys.pack_keys {
            let pack_bytes = self
                .object_store
                .get_object(&self.config.bucket, key)?
                .ok_or_else(|| {
                    anyhow!(
                        "S3 pack object `{key}` is missing for remote `{}` revision `{}`",
                        self.config.remote_id,
                        head.vault_revision
                    )
                })?;
            stored_packs.push(
                bincode::deserialize::<S3StoredPack>(pack_bytes.as_slice()).with_context(|| {
                    format!(
                        "failed to decode S3 pack `{key}` for remote `{}` revision `{}`",
                        self.config.remote_id, head.vault_revision
                    )
                })?,
            );
        }

        let mut encrypted_snapshot = stored_packs
            .first()
            .map(|pack| pack.encrypted_snapshot.clone())
            .ok_or_else(|| {
                anyhow!(
                    "S3 revision `{}` for remote `{}` does not contain any stored packs",
                    head.vault_revision,
                    self.config.remote_id
                )
            })?;
        stored_packs.sort_by_key(|pack| pack.pack_index);
        encrypted_snapshot.ciphertext = stored_packs
            .into_iter()
            .flat_map(|pack| pack.chunk.into_iter())
            .collect();

        Ok(ProviderRevision {
            head: head.clone(),
            manifest,
            encrypted_snapshot,
        })
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let pack_count = request.manifest.packs.len().max(1);
        let keys = self.object_keys_for_revision(&request.head.vault_revision, pack_count);

        let manifest_bytes =
            bincode::serialize(&request.manifest).context("failed to encode S3 vault manifest")?;
        self.object_store.put_object(&S3PutObjectRequest {
            bucket: self.config.bucket.clone(),
            key: keys.manifest_key.clone(),
            body: manifest_bytes,
            expected_current_revision: None,
        })?;

        let pack_payloads = pack_payloads_for_snapshot(&request.encrypted_snapshot, pack_count)?;
        for (key, body) in keys.pack_keys.iter().zip(pack_payloads.into_iter()) {
            self.object_store.put_object(&S3PutObjectRequest {
                bucket: self.config.bucket.clone(),
                key: key.clone(),
                body,
                expected_current_revision: None,
            })?;
        }

        let head_bytes =
            serde_json::to_vec(&request.head).context("failed to encode S3 vault head")?;
        self.object_store.put_object(&S3PutObjectRequest {
            bucket: self.config.bucket.clone(),
            key: keys.head_key,
            body: head_bytes,
            expected_current_revision: request
                .conditional_head_write
                .then(|| request.expected_parent_revision.clone())
                .flatten(),
        })?;

        Ok(())
    }

    fn prune_revisions(&self, keep_latest: usize, live_head: &VaultHead) -> Result<()> {
        let revision_prefix = format!("{}/", join_s3_key(&self.config.prefix, "revisions"));
        let listed_keys = self
            .object_store
            .list_objects(&self.config.bucket, revision_prefix.as_str())?;
        let retained = retained_revision_ids(
            listed_keys
                .iter()
                .filter_map(|key| revision_from_object_key(&self.config.prefix, key)),
            keep_latest,
            live_head.vault_revision.as_str(),
        );

        for key in listed_keys {
            let Some(revision) = revision_from_object_key(&self.config.prefix, key.as_str()) else {
                continue;
            };
            if retained.contains(revision) {
                continue;
            }

            self.object_store
                .delete_object(&self.config.bucket, key.as_str())?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct S3StoredPack {
    pack_index: usize,
    pack_count: usize,
    encrypted_snapshot: EncryptedSnapshot,
    chunk: Vec<u8>,
}

fn pack_payloads_for_snapshot(
    encrypted_snapshot: &EncryptedSnapshot,
    pack_count: usize,
) -> Result<Vec<Vec<u8>>> {
    if pack_count == 0 {
        return Err(anyhow!("S3 pack count must be greater than zero"));
    }

    let chunks = split_bytes(encrypted_snapshot.ciphertext.as_slice(), pack_count);
    let mut payloads = Vec::with_capacity(chunks.len());
    for (index, chunk) in chunks.into_iter().enumerate() {
        payloads.push(
            bincode::serialize(&S3StoredPack {
                pack_index: index,
                pack_count,
                encrypted_snapshot: encrypted_snapshot.clone(),
                chunk,
            })
            .context("failed to encode S3 vault pack")?,
        );
    }

    Ok(payloads)
}

fn split_bytes(bytes: &[u8], chunk_count: usize) -> Vec<Vec<u8>> {
    let chunk_size = bytes.len().div_ceil(chunk_count);
    let mut chunks = Vec::with_capacity(chunk_count);
    for index in 0..chunk_count {
        let start = index.saturating_mul(chunk_size);
        let end = std::cmp::min(start.saturating_add(chunk_size), bytes.len());
        chunks.push(bytes.get(start..end).unwrap_or_default().to_vec());
    }
    chunks
}

fn normalize_prefix(prefix: &str) -> String {
    prefix.trim_matches('/').to_string()
}

fn join_s3_key(prefix: &str, suffix: &str) -> String {
    if prefix.is_empty() {
        suffix.to_string()
    } else {
        format!("{prefix}/{suffix}")
    }
}

fn revision_from_object_key<'a>(prefix: &str, key: &'a str) -> Option<&'a str> {
    let revisions_prefix = format!("{}/", join_s3_key(prefix, "revisions"));
    let suffix = key.strip_prefix(revisions_prefix.as_str())?;
    let (revision, _) = suffix.split_once('/')?;
    if revision.is_empty() {
        return None;
    }

    Some(revision)
}

fn retained_revision_ids<'a>(
    revisions: impl Iterator<Item = &'a str>,
    keep_latest: usize,
    live_revision: &str,
) -> std::collections::BTreeSet<String> {
    let mut revisions = revisions
        .map(ToOwned::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    revisions.sort();
    revisions.reverse();

    let mut retained = revisions
        .into_iter()
        .take(keep_latest)
        .collect::<std::collections::BTreeSet<_>>();
    retained.insert(live_revision.to_string());
    retained
}
