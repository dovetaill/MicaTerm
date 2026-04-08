use std::collections::BTreeMap;

use mica_term::app::keychain::model::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodeMergeMetadata, KeychainNodePayload,
};
use mica_term::app::ssh::credentials::StoredSshSecretBundle;
use mica_term::app::vault::merge::{
    MergeEntityKind, MergeInput, MergeRecoveryAction, merge_snapshots,
};
use mica_term::app::vault::model::{
    SnapshotSyncPreferences, SnapshotUiPreferences, VaultAssetCatalog, VaultAssetKind,
    VaultAssetNode, VaultAssetPayload, VaultKnownHostEntry, VaultNodeMergeMetadata, VaultSnapshot,
    VaultSshConnectionSpec, VaultSshProxySpec,
};

fn empty_snapshot() -> VaultSnapshot {
    VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog::default(),
        ssh_secret_bundles: BTreeMap::new(),
        keychain_catalog: KeychainCatalog::default(),
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    }
}

fn ssh_asset(id: &str, title: &str, keychain_identity_id: Option<&str>) -> VaultAssetNode {
    VaultAssetNode {
        id: id.into(),
        parent_id: None,
        title: title.into(),
        kind: VaultAssetKind::SshConnection,
        child_ids: Vec::new(),
        payload: VaultAssetPayload::SshConnection(Box::new(VaultSshConnectionSpec {
            host: format!("{id}.example.com"),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: keychain_identity_id.map(ToOwned::to_owned),
            private_key_source: String::new(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: VaultSshProxySpec::None,
            remark: String::new(),
            credential_ref: Some(format!("ssh/saved-secrets/{id}")),
        })),
    }
}

fn identity_node(id: &str) -> KeychainNode {
    KeychainNode {
        id: id.into(),
        parent_id: None,
        title: "Ops".into(),
        kind: KeychainNodeKind::Identity,
        child_ids: Vec::new(),
        payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
            username: "ops".into(),
            auth_kind: KeychainIdentityAuthKind::Password,
            ssh_key_id: None,
            credential_ref: Some(format!("keychain/identity/{id}")),
            remark: "shared ops login".into(),
        }),
    }
}

#[test]
fn merge_unions_assets_added_on_different_devices() {
    let mut local = empty_snapshot();
    local
        .asset_catalog
        .nodes
        .insert("asset-a".into(), ssh_asset("asset-a", "Asset A", None));
    local.asset_catalog.root_ids.push("asset-a".into());
    local.asset_catalog.merge_metadata.insert(
        "asset-a".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T09:00:00Z".into()),
            last_modified_by_device: Some("device-a".into()),
            deleted_at: None,
        },
    );

    let mut remote = empty_snapshot();
    remote
        .asset_catalog
        .nodes
        .insert("asset-b".into(), ssh_asset("asset-b", "Asset B", None));
    remote.asset_catalog.root_ids.push("asset-b".into());
    remote.asset_catalog.merge_metadata.insert(
        "asset-b".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T09:05:00Z".into()),
            last_modified_by_device: Some("device-b".into()),
            deleted_at: None,
        },
    );

    let result = merge_snapshots(MergeInput {
        base: empty_snapshot(),
        local,
        remote,
        device_id: "device-a".into(),
    });

    assert!(result.merged.asset_catalog.nodes.contains_key("asset-a"));
    assert!(result.merged.asset_catalog.nodes.contains_key("asset-b"));
    assert!(result.conflicts.is_empty());
}

#[test]
fn merge_records_conflict_when_local_deletes_and_remote_modifies_same_asset() {
    let mut base = empty_snapshot();
    base.asset_catalog.nodes.insert(
        "asset-gateway".into(),
        ssh_asset("asset-gateway", "Gateway", None),
    );
    base.asset_catalog.root_ids.push("asset-gateway".into());
    base.asset_catalog.merge_metadata.insert(
        "asset-gateway".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T08:00:00Z".into()),
            last_modified_by_device: Some("device-base".into()),
            deleted_at: None,
        },
    );

    let mut local = base.clone();
    local.asset_catalog.nodes.remove("asset-gateway");
    local.asset_catalog.root_ids.clear();
    local.asset_catalog.merge_metadata.insert(
        "asset-gateway".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T08:10:00Z".into()),
            last_modified_by_device: Some("device-local".into()),
            deleted_at: Some("2026-04-01T08:10:00Z".into()),
        },
    );

    let mut remote = base.clone();
    remote.asset_catalog.nodes.insert(
        "asset-gateway".into(),
        ssh_asset("asset-gateway", "Gateway v2", None),
    );
    remote.asset_catalog.merge_metadata.insert(
        "asset-gateway".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T08:20:00Z".into()),
            last_modified_by_device: Some("device-remote".into()),
            deleted_at: None,
        },
    );

    let result = merge_snapshots(MergeInput {
        base,
        local,
        remote,
        device_id: "device-local".into(),
    });

    assert!(
        result
            .conflicts
            .iter()
            .any(|conflict| conflict.entity == MergeEntityKind::Asset
                && conflict.node_id == "asset-gateway"
                && conflict.message.contains("deleted"))
    );
    assert!(matches!(
        result.recovery_actions.first(),
        Some(MergeRecoveryAction::ConflictCopyRequired {
            entity: MergeEntityKind::Asset,
            node_id,
        }) if node_id == "asset-gateway"
    ));
    assert_eq!(
        result
            .merged
            .asset_catalog
            .nodes
            .get("asset-gateway")
            .expect("conflicted asset kept")
            .title,
        "Gateway v2"
    );
}

#[test]
fn merge_keeps_keychain_identity_references_intact_when_asset_and_identity_arrive_together() {
    let mut local = empty_snapshot();
    local.asset_catalog.nodes.insert(
        "asset-gateway".into(),
        ssh_asset("asset-gateway", "Gateway", Some("identity-ops")),
    );
    local.asset_catalog.root_ids.push("asset-gateway".into());
    local.asset_catalog.merge_metadata.insert(
        "asset-gateway".into(),
        VaultNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T09:00:00Z".into()),
            last_modified_by_device: Some("device-a".into()),
            deleted_at: None,
        },
    );
    local
        .keychain_catalog
        .nodes
        .insert("identity-ops".into(), identity_node("identity-ops"));
    local.keychain_catalog.root_ids.push("identity-ops".into());
    local.keychain_catalog.merge_metadata.insert(
        "identity-ops".into(),
        KeychainNodeMergeMetadata {
            last_modified_at: Some("2026-04-01T09:00:00Z".into()),
            last_modified_by_device: Some("device-a".into()),
            deleted_at: None,
        },
    );
    local.keychain_identity_secret_bundles.insert(
        "identity-ops".into(),
        StoredSshSecretBundle {
            password: Some("ops-password".into()),
            ..StoredSshSecretBundle::default()
        },
    );

    let result = merge_snapshots(MergeInput {
        base: empty_snapshot(),
        local,
        remote: empty_snapshot(),
        device_id: "device-a".into(),
    });

    let asset = result
        .merged
        .asset_catalog
        .nodes
        .get("asset-gateway")
        .expect("merged ssh asset");
    match &asset.payload {
        VaultAssetPayload::SshConnection(spec) => {
            assert_eq!(spec.keychain_identity_id.as_deref(), Some("identity-ops"));
        }
        other => panic!("unexpected payload: {other:?}"),
    }
    assert!(
        result
            .merged
            .keychain_catalog
            .nodes
            .contains_key("identity-ops")
    );
    assert_eq!(
        result
            .merged
            .keychain_identity_secret_bundles
            .get("identity-ops")
            .and_then(|bundle| bundle.password.as_deref()),
        Some("ops-password")
    );
}

#[test]
fn merge_known_hosts_uses_conservative_union_without_destructive_overwrite() {
    let mut base = empty_snapshot();
    base.known_hosts.push(VaultKnownHostEntry {
        host_pattern: "[base.example.com]:22".into(),
        public_key: "ssh-ed25519 AAAABase".into(),
    });

    let mut local = empty_snapshot();
    local.known_hosts.push(VaultKnownHostEntry {
        host_pattern: "[local.example.com]:22".into(),
        public_key: "ssh-ed25519 AAAALocal".into(),
    });

    let mut remote = empty_snapshot();
    remote.known_hosts.push(VaultKnownHostEntry {
        host_pattern: "[remote.example.com]:22".into(),
        public_key: "ssh-ed25519 AAAARemote".into(),
    });
    remote.known_hosts.push(VaultKnownHostEntry {
        host_pattern: "[local.example.com]:22".into(),
        public_key: "ssh-ed25519 AAAALocal".into(),
    });

    let result = merge_snapshots(MergeInput {
        base,
        local,
        remote,
        device_id: "device-a".into(),
    });

    assert_eq!(result.merged.known_hosts.len(), 3);
    assert!(
        result
            .merged
            .known_hosts
            .iter()
            .any(|entry| entry.host_pattern == "[base.example.com]:22")
    );
    assert!(
        result
            .merged
            .known_hosts
            .iter()
            .any(|entry| entry.host_pattern == "[local.example.com]:22")
    );
    assert!(
        result
            .merged
            .known_hosts
            .iter()
            .any(|entry| entry.host_pattern == "[remote.example.com]:22")
    );
}
