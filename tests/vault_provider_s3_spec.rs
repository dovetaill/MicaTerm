use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind, KdfConfig,
    ProviderAuthKind, ProviderKind, RemoteRole, VaultHead,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::s3::{
    S3CredentialMode, S3ObjectKeySet, S3ObjectStoreAdapter, S3VaultProvider, S3VaultProviderConfig,
};

fn sample_s3_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-s3-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::S3Compatible,
        locator: BootstrapRemoteLocator::S3 {
            bucket: "vault-bucket".into(),
            prefix: "users/demo".into(),
            endpoint: Some("https://minio.example.internal:9000".into()),
            region: Some("cn-hangzhou".into()),
            force_path_style: true,
        },
        credential_ref: Some("vault/bootstrap/remote-s3-primary".into()),
        auth_kind: ProviderAuthKind::AwsStandardChain,
        last_health: None,
    }
}

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "s3-provider-salt".into(),
    }
}

fn sample_head(revision: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-a".into(),
        committed_at: "2026-03-31T08:00:00Z".into(),
        committed_by_device: "device-a".into(),
        payload_hash: "sha256:payload".into(),
        manifest_ref: format!("manifest/{revision}.bin"),
        wrapped_vault_key: "wrapped-key".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: mica_term::app::vault::model::PackLayout::ObjectSet,
    }
}

#[derive(Default)]
struct RecordingS3ObjectStoreAdapter {
    listed_keys: Mutex<Vec<String>>,
    deleted_keys: Mutex<Vec<(String, String)>>,
}

impl RecordingS3ObjectStoreAdapter {
    fn with_keys(keys: Vec<String>) -> Self {
        Self {
            listed_keys: Mutex::new(keys),
            deleted_keys: Mutex::new(Vec::new()),
        }
    }
}

impl S3ObjectStoreAdapter for RecordingS3ObjectStoreAdapter {
    fn get_object(&self, _bucket: &str, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    fn put_object(
        &self,
        _request: &mica_term::app::vault::provider::s3::S3PutObjectRequest,
    ) -> Result<()> {
        Ok(())
    }

    fn list_objects(&self, _bucket: &str, prefix: &str) -> Result<Vec<String>> {
        Ok(self
            .listed_keys
            .lock()
            .map_err(|_| anyhow!("listed keys lock poisoned"))?
            .iter()
            .filter(|key| key.starts_with(prefix))
            .cloned()
            .collect())
    }

    fn delete_object(&self, bucket: &str, key: &str) -> Result<()> {
        self.deleted_keys
            .lock()
            .map_err(|_| anyhow!("deleted keys lock poisoned"))?
            .push((bucket.to_string(), key.to_string()));
        Ok(())
    }
}

#[test]
fn s3_provider_config_parses_bucket_prefix_region_endpoint_and_path_style() {
    let remote = sample_s3_remote();

    let config = S3VaultProviderConfig::try_from(&remote).expect("parse s3 remote");

    assert_eq!(config.remote_id, "remote-s3-primary");
    assert_eq!(config.bucket, "vault-bucket");
    assert_eq!(config.prefix, "users/demo");
    assert_eq!(
        config.endpoint.as_deref(),
        Some("https://minio.example.internal:9000")
    );
    assert_eq!(config.region.as_deref(), Some("cn-hangzhou"));
    assert!(config.force_path_style);
}

#[test]
fn s3_provider_uses_standard_credential_chain_by_default() {
    let remote = sample_s3_remote();

    let config = S3VaultProviderConfig::try_from(&remote).expect("parse s3 remote");

    assert_eq!(config.credential_mode, S3CredentialMode::StandardChain);
}

#[test]
fn s3_provider_exposes_conditional_head_write_capability() {
    let provider = S3VaultProvider::new(
        S3VaultProviderConfig::try_from(&sample_s3_remote()).expect("parse s3 remote"),
    )
    .expect("build s3 provider");

    let capabilities = provider.capabilities();

    assert!(capabilities.supports_conditional_head_write);
}

#[test]
fn s3_provider_generates_stable_deterministic_object_names_for_head_manifest_and_packs() {
    let keys = S3ObjectKeySet::for_revision("users/demo", "rev-0002", 3);

    assert_eq!(keys.head_key, "users/demo/head.json");
    assert_eq!(
        keys.manifest_key,
        "users/demo/revisions/rev-0002/manifest.bin"
    );
    assert_eq!(
        keys.pack_keys,
        vec![
            "users/demo/revisions/rev-0002/packs/pack-0000.bin".to_string(),
            "users/demo/revisions/rev-0002/packs/pack-0001.bin".to_string(),
            "users/demo/revisions/rev-0002/packs/pack-0002.bin".to_string(),
        ]
    );
}

#[test]
fn s3_provider_prune_revision_objects_older_than_keep_latest_limit() {
    let config = S3VaultProviderConfig::try_from(&sample_s3_remote()).expect("parse s3 remote");
    let keys = (1..=12)
        .flat_map(|revision| {
            let revision = format!("rev-{revision:04}");
            let key_set = S3ObjectKeySet::for_revision(&config.prefix, revision.as_str(), 1);
            [key_set.manifest_key, key_set.pack_keys[0].clone()]
        })
        .collect::<Vec<_>>();
    let adapter = Arc::new(RecordingS3ObjectStoreAdapter::with_keys(keys));
    let provider =
        S3VaultProvider::with_adapter(config, adapter.clone()).expect("build s3 provider");

    provider
        .prune_revisions(10, &sample_head("rev-0012"))
        .expect("prune old s3 revisions");

    assert_eq!(
        adapter
            .deleted_keys
            .lock()
            .expect("lock deleted keys")
            .as_slice(),
        &[
            (
                "vault-bucket".to_string(),
                "users/demo/revisions/rev-0001/manifest.bin".to_string(),
            ),
            (
                "vault-bucket".to_string(),
                "users/demo/revisions/rev-0001/packs/pack-0000.bin".to_string(),
            ),
            (
                "vault-bucket".to_string(),
                "users/demo/revisions/rev-0002/manifest.bin".to_string(),
            ),
            (
                "vault-bucket".to_string(),
                "users/demo/revisions/rev-0002/packs/pack-0000.bin".to_string(),
            ),
        ]
    );
}
