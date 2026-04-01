use std::sync::Arc;

use mica_term::app::vault::crypto::generate_vault_key;
use mica_term::app::vault::engine::{SyncEngine, SyncError, SyncRequest};
use mica_term::app::vault::model::{
    CipherKind, CompressionKind, KdfConfig, PackLayout, ProviderKind, VaultAssetCatalog, VaultHead,
    VaultSnapshot,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, VaultProvider};

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "sync-engine-salt".into(),
    }
}

fn sample_snapshot() -> VaultSnapshot {
    VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog::default(),
        ..VaultSnapshot::default()
    }
}

fn sample_remote_head(revision: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-a".into(),
        committed_at: "2026-03-28T09:00:00Z".into(),
        committed_by_device: "device-a".into(),
        payload_hash: "sha256:payload-prev".into(),
        manifest_ref: format!("manifest/{revision}.bin"),
        wrapped_vault_key: "wrapped-key-prev".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::ObjectSet,
    }
}

fn sample_request(next_revision: &str, parent_revision: Option<&str>) -> SyncRequest {
    SyncRequest {
        vault_id: "vault-main".into(),
        snapshot: sample_snapshot(),
        next_revision: next_revision.into(),
        parent_revision: parent_revision.map(ToOwned::to_owned),
        device_id: "device-b".into(),
        committed_at: "2026-03-28T10:00:00Z".into(),
        committed_by_device: "device-b".into(),
        wrapped_vault_key: "wrapped-key-next".into(),
        kdf: sample_kdf(),
        provider_kind: ProviderKind::S3Compatible,
        vault_key: generate_vault_key(),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[test]
fn sync_engine_writes_primary_then_fans_out_to_mirrors() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let engine = SyncEngine::new(
        primary.clone() as Arc<dyn VaultProvider>,
        vec![mirror.clone() as Arc<dyn VaultProvider>],
    );

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("sync succeeds");

    assert_eq!(result.primary_revision, "rev-0002");
    assert!(result.mirror_failures.is_empty());
    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(mirror.recorded_writes().len(), 1);
    assert_eq!(
        mirror.recorded_writes()[0].head.vault_revision,
        "rev-0002",
        "mirror should receive the committed primary revision"
    );
}

#[test]
fn sync_engine_reports_mirror_failure_without_rolling_back_primary_commit() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    mirror.set_write_error(Some("mirror unavailable"));
    let engine = SyncEngine::new(
        primary.clone() as Arc<dyn VaultProvider>,
        vec![mirror.clone() as Arc<dyn VaultProvider>],
    );

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("primary should still commit");

    assert_eq!(result.primary_revision, "rev-0002");
    assert_eq!(result.mirror_failures.len(), 1);
    assert_eq!(result.mirror_failures[0].remote_id, "remote-mirror");
    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(mirror.recorded_writes().len(), 0);
}

#[test]
fn sync_engine_surfaces_conflict_when_primary_parent_revision_mismatches() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0009")));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let err = engine
        .sync(sample_request("rev-0010", Some("rev-0001")))
        .expect_err("parent mismatch should conflict");

    assert_eq!(
        err,
        SyncError::Conflict {
            remote_id: "remote-primary".into(),
            expected_parent_revision: Some("rev-0001".into()),
            actual_primary_revision: Some("rev-0009".into()),
        }
    );
    assert!(primary.recorded_writes().is_empty());
}

#[test]
fn sync_engine_enables_conditional_head_write_for_s3_like_primary() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("conditional sync succeeds");

    assert_eq!(result.primary_revision, "rev-0002");
    let writes = primary.recorded_writes();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].conditional_head_write);
    assert_eq!(
        writes[0].expected_parent_revision.as_deref(),
        Some("rev-0001")
    );
    assert_eq!(writes[0].head.pack_layout, PackLayout::ObjectSet);
}

#[test]
fn sync_engine_disables_conditional_head_write_for_git_repo_primary() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::git_repo_primary(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("git repo sync succeeds");

    assert_eq!(result.primary_revision, "rev-0002");
    let writes = primary.recorded_writes();
    assert_eq!(writes.len(), 1);
    assert!(!writes[0].conditional_head_write);
    assert_eq!(writes[0].head.pack_layout, PackLayout::BundledFiles);
}

#[test]
fn sync_engine_surfaces_primary_read_failures_without_touching_mirrors() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_read_error(Some("token expired"));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let engine = SyncEngine::new(
        primary.clone() as Arc<dyn VaultProvider>,
        vec![mirror.clone() as Arc<dyn VaultProvider>],
    );

    let err = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect_err("primary read error should fail sync");

    assert_eq!(
        err,
        SyncError::PrimaryReadFailed {
            remote_id: "remote-primary".into(),
            message: "token expired".into(),
        }
    );
    assert!(primary.recorded_writes().is_empty());
    assert!(mirror.recorded_writes().is_empty());
}

#[test]
fn sync_engine_allows_the_first_commit_against_an_empty_primary_head() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let result = engine
        .sync(sample_request("rev-0001", None))
        .expect("initial sync should succeed against an empty head");

    assert_eq!(result.primary_revision, "rev-0001");
    let writes = primary.recorded_writes();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].conditional_head_write);
    assert_eq!(writes[0].expected_parent_revision, None);
}

#[test]
fn sync_engine_embeds_recovery_metadata_into_written_manifest() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let result = engine
        .sync(sample_request("rev-0001", None))
        .expect("initial sync should succeed against an empty head");

    let writes = primary.recorded_writes();
    assert_eq!(writes.len(), 1);
    assert_eq!(
        writes[0]
            .manifest
            .provider_capability_fallbacks
            .get("snapshot.nonce_hex"),
        Some(&hex(result.encrypted_snapshot.nonce.as_slice()))
    );
    assert_eq!(
        writes[0]
            .manifest
            .provider_capability_fallbacks
            .get("snapshot.plaintext_len"),
        Some(&result.encrypted_snapshot.plaintext_len.to_string())
    );
    assert_eq!(
        writes[0]
            .manifest
            .provider_capability_fallbacks
            .get("snapshot.compressed_len"),
        Some(&result.encrypted_snapshot.compressed_len.to_string())
    );
    assert_eq!(
        writes[0]
            .manifest
            .provider_capability_fallbacks
            .get("snapshot.payload_sha256"),
        Some(&result.encrypted_snapshot.payload_sha256)
    );
}

#[test]
fn sync_engine_writes_committed_metadata_instead_of_legacy_created_at() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let result = engine
        .sync(sample_request("rev-0001", None))
        .expect("initial sync should succeed against an empty head");

    assert_eq!(result.head.committed_at, "2026-03-28T10:00:00Z");
    assert_eq!(result.head.committed_by_device, "device-b");
    let encoded = serde_json::to_value(&result.head).expect("serialize written head");
    assert!(encoded.get("created_at").is_none());
}

#[test]
fn retention_runs_after_successful_primary_and_mirror_syncs() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let engine = SyncEngine::new(
        primary.clone() as Arc<dyn VaultProvider>,
        vec![mirror.clone() as Arc<dyn VaultProvider>],
    );

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("sync succeeds");

    assert_eq!(
        primary.recorded_prunes(),
        vec![(10, result.head.clone())],
        "primary should prune retained revisions after committing"
    );
    assert_eq!(mirror.recorded_prunes().len(), 1);
    assert_eq!(mirror.recorded_prunes()[0].0, 10);
    assert_eq!(
        mirror.recorded_prunes()[0].1.vault_revision,
        result.primary_revision,
        "mirror should prune retained revisions for the committed revision"
    );
}

#[test]
fn retention_primary_prune_failure_surfaces_as_sync_error() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    primary.set_prune_error(Some("retention cleanup failed"));
    let engine = SyncEngine::new(primary.clone() as Arc<dyn VaultProvider>, Vec::new());

    let err = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect_err("primary prune failure should fail sync");

    assert_eq!(
        err,
        SyncError::PrimaryWriteFailed {
            remote_id: "remote-primary".into(),
            message: "retention cleanup failed".into(),
        }
    );
    assert_eq!(primary.recorded_writes().len(), 1);
}

#[test]
fn retention_mirror_prune_failure_is_reported_as_mirror_degradation() {
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0001")));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    mirror.set_prune_error(Some("mirror retention failed"));
    let engine = SyncEngine::new(
        primary.clone() as Arc<dyn VaultProvider>,
        vec![mirror.clone() as Arc<dyn VaultProvider>],
    );

    let result = engine
        .sync(sample_request("rev-0002", Some("rev-0001")))
        .expect("mirror prune failure should not roll back primary");

    assert_eq!(result.mirror_failures.len(), 1);
    assert_eq!(result.mirror_failures[0].remote_id, "remote-mirror");
    assert_eq!(result.mirror_failures[0].message, "mirror retention failed");
    assert_eq!(primary.recorded_prunes().len(), 1);
    assert_eq!(mirror.recorded_prunes().len(), 1);
}
