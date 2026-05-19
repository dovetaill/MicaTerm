use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use mica_term::app::assets_catalog::{
    asset_tree_to_catalog, asset_tree_to_vault_catalog, asset_trees_to_catalog,
    catalog_to_asset_tree, catalog_to_asset_trees,
};
use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, StoredSshSecretBundle,
    load_secret_bundle, persist_secret_bundle, ssh_credential_ref,
};
use mica_term::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use mica_term::app::ui_preferences::UiPreferences;
use mica_term::app::vault::model::{
    SnapshotSyncPreferences, SnapshotUiPreferences, VaultAssetCatalog, VaultAssetDomain,
    VaultAssetKind, VaultAssetNode, VaultAssetPayload, VaultKnownHostEntry, VaultNodeMergeMetadata,
    VaultSnapshot, VaultSnippetSpec,
};
use mica_term::app::vault::snapshot::{apply_vault_snapshot, export_vault_snapshot};
use mica_term::shell::assets::{
    AssetDomain, AssetNodePayload, AssetSnippetSpec, AssetSshConnectionSpec, AssetSshProxySpec,
    AssetTree, ConsoleAssetKind,
};
use mica_term::theme::ThemeMode;
use russh::keys::{HashAlg, PublicKey};
use uuid::Uuid;

fn temp_known_hosts_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-snapshot-{}-{}.txt",
        label,
        Uuid::new_v4()
    ))
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti snapshot@example.com",
    )
    .expect("parse sample public key")
}

fn sample_console_asset_tree(credential_ref: &str) -> AssetTree {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Production");
    tree.insert_child_with_payload(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "prod.example.com".into(),
            user: "deploy".into(),
            port: "22".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "critical path".into(),
            credential_ref: Some(credential_ref.into()),
        }),
    );
    tree.insert_child_with_payload(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "Ops Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "ops.example.com".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            auth_source: "keychain-identity".into(),
            keychain_identity_id: Some("identity-ops".into()),
            private_key_source: String::new(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "identity-linked".into(),
            credential_ref: None,
        }),
    );
    tree.insert_child(&folder_id, ConsoleAssetKind::Folder, "Nested Folder");
    tree
}

fn sample_snippet_tree() -> AssetTree {
    let mut tree = AssetTree::new();
    let package_id = tree.insert_root(ConsoleAssetKind::SnippetPackage, "Deploy");
    tree.insert_child_with_payload(
        &package_id,
        ConsoleAssetKind::Snippet,
        "Deploy prod",
        AssetNodePayload::Snippet(AssetSnippetSpec {
            script: "kubectl apply -f prod.yaml".into(),
            package_id: Some(package_id.clone()),
        }),
    );
    tree.insert_root_with_payload(
        ConsoleAssetKind::Snippet,
        "Restart API",
        AssetNodePayload::Snippet(AssetSnippetSpec {
            script: "kubectl rollout restart deploy/api".into(),
            package_id: None,
        }),
    );
    tree
}

fn sample_combined_asset_tree(credential_ref: &str) -> AssetTree {
    catalog_to_asset_tree(&asset_trees_to_catalog(
        &sample_console_asset_tree(credential_ref),
        &sample_snippet_tree(),
    ))
}

fn sample_keychain_catalog(
    identity_credential_ref: &str,
    key_credential_ref: &str,
) -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-team".into(), "identity-ops".into()],
        nodes: BTreeMap::from([
            (
                "folder-team".into(),
                KeychainNode {
                    id: "folder-team".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: None,
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::Password,
                        ssh_key_id: None,
                        credential_ref: Some(identity_credential_ref.into()),
                        remark: "shared ops login".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Prod Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:key-prod".into(),
                        public_key: "ssh-ed25519 AAAAC3NzaKeyProd".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some(key_credential_ref.into()),
                        remark: "generated".into(),
                    }),
                },
            ),
        ]),
        merge_metadata: BTreeMap::new(),
    }
}

#[test]
fn vault_snapshot_includes_all_user_sync_assets() {
    let credential_ref = "ssh/saved-secrets/asset-2";
    let identity_credential_ref = "keychain/identity/identity-ops";
    let key_credential_ref = "keychain/key/key-prod";
    let store = MemoryCredentialStore::default();
    let tree = sample_combined_asset_tree(credential_ref);
    let keychain_catalog = sample_keychain_catalog(identity_credential_ref, key_credential_ref);
    let known_hosts_path = temp_known_hosts_path("export");
    let known_hosts = KnownHostsService::new(&known_hosts_path);
    let public_key = sample_public_key();

    persist_secret_bundle(
        &store,
        credential_ref,
        &StoredSshSecretBundle {
            password: None,
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist secret bundle");
    persist_secret_bundle(
        &store,
        identity_credential_ref,
        &StoredSshSecretBundle {
            password: Some("ops-password".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist identity secret bundle");
    persist_secret_bundle(
        &store,
        key_credential_ref,
        &StoredSshSecretBundle {
            password: None,
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("key-passphrase".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist key secret bundle");
    known_hosts
        .accept_unknown("prod.example.com", 22, &public_key)
        .expect("persist known_hosts entry");

    let sync_preferences = SnapshotSyncPreferences {
        auto_sync_enabled: true,
        selected_primary_remote_id: Some("remote-s3".into()),
        selected_mirror_remote_ids: vec!["remote-github".into()],
        last_sync_result: Some("success".into()),
    };
    let ui_preferences = UiPreferences {
        theme_mode: ThemeMode::Light,
        always_on_top: true,
        right_panel_view: "appearance".into(),
        ..UiPreferences::default()
    };

    let snapshot = export_vault_snapshot(
        &tree,
        &keychain_catalog,
        &store,
        &known_hosts_path,
        sync_preferences.clone(),
        &ui_preferences,
    )
    .expect("export vault snapshot");

    assert_eq!(snapshot.asset_catalog, asset_tree_to_vault_catalog(&tree));
    assert_eq!(snapshot.ssh_secret_bundles.len(), 1);
    assert_eq!(snapshot.keychain_catalog.root_ids.len(), 2);
    assert_eq!(snapshot.keychain_identity_secret_bundles.len(), 1);
    assert_eq!(snapshot.keychain_key_secret_bundles.len(), 1);
    assert_eq!(
        snapshot
            .ssh_secret_bundles
            .values()
            .next()
            .expect("exported secret bundle")
            .passphrase
            .as_deref(),
        Some("hunter2")
    );
    assert_eq!(
        snapshot.keychain_identity_secret_bundles["identity-ops"]
            .password
            .as_deref(),
        Some("ops-password")
    );
    assert_eq!(
        snapshot.keychain_key_secret_bundles["key-prod"]
            .private_key_content
            .as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert!(snapshot.known_hosts.is_empty());
    assert_eq!(
        snapshot.sync_preferences,
        SnapshotSyncPreferences::default()
    );
    assert_eq!(snapshot.ui_preferences, SnapshotUiPreferences::default());

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn vault_snapshot_excludes_ui_preferences() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("ui-preferences");
    let snapshot = export_vault_snapshot(
        &sample_combined_asset_tree("ssh/saved-secrets/asset-2"),
        &sample_keychain_catalog("keychain/identity/identity-ops", "keychain/key/key-prod"),
        &store,
        &known_hosts_path,
        SnapshotSyncPreferences::default(),
        &UiPreferences {
            theme_mode: ThemeMode::Light,
            always_on_top: true,
            right_panel_view: "appearance".into(),
            ..UiPreferences::default()
        },
    )
    .expect("export vault snapshot");

    assert_eq!(snapshot.ui_preferences, SnapshotUiPreferences::default());

    let _ = fs::remove_file(known_hosts_path);
}

fn sample_sync_vault_snapshot(
    credential_ref: &str,
    identity_credential_ref: &str,
    key_credential_ref: &str,
) -> VaultSnapshot {
    VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec![
                "folder-prod".into(),
                "snippet-package-1".into(),
                "snippet-root".into(),
            ],
            nodes: BTreeMap::from([
                (
                    "folder-prod".into(),
                    VaultAssetNode {
                        id: "folder-prod".into(),
                        parent_id: None,
                        title: "Production".into(),
                        kind: VaultAssetKind::Folder,
                        child_ids: vec![
                            "asset-gateway".into(),
                            "asset-identity".into(),
                            "folder-nested".into(),
                        ],
                        payload: VaultAssetPayload::Folder,
                    },
                ),
                (
                    "asset-gateway".into(),
                    VaultAssetNode {
                        id: "asset-gateway".into(),
                        parent_id: Some("folder-prod".into()),
                        title: "Gateway".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            mica_term::app::vault::model::VaultSshConnectionSpec {
                                host: "prod.example.com".into(),
                                user: "deploy".into(),
                                port: "22".into(),
                                auth_method: "private-key".into(),
                                auth_source: "manual".into(),
                                keychain_identity_id: None,
                                private_key_source: "content".into(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                                remark: "critical path".into(),
                                credential_ref: Some(credential_ref.into()),
                            },
                        )),
                    },
                ),
                (
                    "asset-identity".into(),
                    VaultAssetNode {
                        id: "asset-identity".into(),
                        parent_id: Some("folder-prod".into()),
                        title: "Ops Bastion".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            mica_term::app::vault::model::VaultSshConnectionSpec {
                                host: "ops.example.com".into(),
                                user: "ops".into(),
                                port: "22".into(),
                                auth_method: "password".into(),
                                auth_source: "keychain-identity".into(),
                                keychain_identity_id: Some("identity-ops".into()),
                                private_key_source: String::new(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                                remark: "identity-linked".into(),
                                credential_ref: None,
                            },
                        )),
                    },
                ),
                (
                    "folder-nested".into(),
                    VaultAssetNode {
                        id: "folder-nested".into(),
                        parent_id: Some("folder-prod".into()),
                        title: "Nested Folder".into(),
                        kind: VaultAssetKind::Folder,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::Folder,
                    },
                ),
                (
                    "snippet-package-1".into(),
                    VaultAssetNode {
                        id: "snippet-package-1".into(),
                        parent_id: None,
                        title: "Deploy".into(),
                        kind: VaultAssetKind::SnippetPackage,
                        child_ids: vec!["snippet-child".into()],
                        payload: VaultAssetPayload::SnippetPackage,
                    },
                ),
                (
                    "snippet-child".into(),
                    VaultAssetNode {
                        id: "snippet-child".into(),
                        parent_id: Some("snippet-package-1".into()),
                        title: "Deploy prod".into(),
                        kind: VaultAssetKind::Snippet,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::Snippet(VaultSnippetSpec {
                            script: "kubectl apply -f prod.yaml".into(),
                            package_id: Some("snippet-package-1".into()),
                        }),
                    },
                ),
                (
                    "snippet-root".into(),
                    VaultAssetNode {
                        id: "snippet-root".into(),
                        parent_id: None,
                        title: "Restart API".into(),
                        kind: VaultAssetKind::Snippet,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::Snippet(VaultSnippetSpec {
                            script: "kubectl rollout restart deploy/api".into(),
                            package_id: None,
                        }),
                    },
                ),
            ]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::from([(
            "asset-gateway".into(),
            StoredSshSecretBundle {
                password: Some("deploy-password".into()),
                private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                passphrase: Some("hunter2".into()),
                proxy_socks5_password: None,
            },
        )]),
        keychain_catalog: sample_keychain_catalog(identity_credential_ref, key_credential_ref),
        keychain_identity_secret_bundles: BTreeMap::from([(
            "identity-ops".into(),
            StoredSshSecretBundle {
                password: Some("ops-password".into()),
                private_key_content: None,
                passphrase: None,
                proxy_socks5_password: None,
            },
        )]),
        keychain_key_secret_bundles: BTreeMap::from([(
            "key-prod".into(),
            StoredSshSecretBundle {
                password: None,
                private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                passphrase: Some("key-passphrase".into()),
                proxy_socks5_password: None,
            },
        )]),
        known_hosts: vec![VaultKnownHostEntry {
            host_pattern: "[prod.example.com]:22".into(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti snapshot@example.com".into(),
        }],
        sync_preferences: SnapshotSyncPreferences {
            auto_sync_enabled: true,
            selected_primary_remote_id: Some("remote-s3".into()),
            selected_mirror_remote_ids: vec!["remote-github".into()],
            last_sync_result: Some("success".into()),
        },
        ui_preferences: SnapshotUiPreferences {
            theme_mode: Some("light".into()),
            always_on_top: Some(true),
        },
    }
}

#[test]
fn known_hosts_is_excluded_until_trust_policy_exists() {
    let credential_ref = "ssh/saved-secrets/gateway";
    let identity_credential_ref = "keychain/identity/identity-ops";
    let key_credential_ref = "keychain/key/key-prod";
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("import");
    let public_key = sample_public_key();
    let snapshot =
        sample_sync_vault_snapshot(credential_ref, identity_credential_ref, key_credential_ref);

    let applied =
        apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply vault snapshot");

    assert_eq!(applied.sync_preferences, SnapshotSyncPreferences::default());
    assert_eq!(applied.ui_preferences.theme_mode, ThemeMode::Dark);
    assert!(!applied.ui_preferences.always_on_top);

    let known_hosts = KnownHostsService::new(&known_hosts_path);
    let result = known_hosts
        .check("prod.example.com", 22, &public_key)
        .expect("check imported known_hosts entry");
    assert!(matches!(result, KnownHostCheck::Unknown { .. }));
    assert_eq!(
        public_key.fingerprint(HashAlg::Sha256).to_string().len() > 0,
        true
    );

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn restore_rebuilds_asset_snippet_keychain_projection() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("projection-restore");
    let snapshot = sample_sync_vault_snapshot(
        "ssh/saved-secrets/gateway",
        "keychain/identity/identity-ops",
        "keychain/key/key-prod",
    );
    let applied = apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply");
    let (console_tree, snippet_tree) =
        catalog_to_asset_trees(&asset_tree_to_catalog(&applied.asset_tree));
    let mut state = mica_term::shell::view_model::ShellViewModel::default();

    state.replace_vault_projection(console_tree, snippet_tree, applied.keychain_catalog.clone());

    assert_eq!(state.console_asset_tree().root_ids().len(), 1);
    assert_eq!(state.snippet_asset_tree().root_ids().len(), 2);
    assert_eq!(state.keychain_catalog().nodes.len(), 3);
    assert_eq!(
        state
            .snippet_asset_tree()
            .snippet_spec("snippet-child")
            .expect("snippet child")
            .script,
        "kubectl apply -f prod.yaml"
    );
    assert!(state.keychain_catalog().nodes.contains_key("identity-ops"));

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn lock_clears_decrypted_asset_snippet_keychain_projection() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("projection-clear");
    let snapshot = sample_sync_vault_snapshot(
        "ssh/saved-secrets/gateway",
        "keychain/identity/identity-ops",
        "keychain/key/key-prod",
    );
    let applied = apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply");
    let (console_tree, snippet_tree) =
        catalog_to_asset_trees(&asset_tree_to_catalog(&applied.asset_tree));
    let mut state = mica_term::shell::view_model::ShellViewModel::default();

    state.replace_vault_projection(console_tree, snippet_tree, applied.keychain_catalog);
    state.clear_vault_projection();

    assert_eq!(state.console_asset_tree().root_ids().len(), 0);
    assert_eq!(state.snippet_asset_tree().root_ids().len(), 0);
    assert_eq!(state.keychain_catalog().nodes.len(), 0);

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn round_trip_ssh_connection_password_identity_keychain_snippet_folder_structure() {
    let credential_ref = "ssh/saved-secrets/gateway";
    let identity_credential_ref = "keychain/identity/identity-ops";
    let key_credential_ref = "keychain/key/key-prod";
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("round-trip");
    let snapshot =
        sample_sync_vault_snapshot(credential_ref, identity_credential_ref, key_credential_ref);
    let applied = apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply");

    let round_tripped = export_vault_snapshot(
        &applied.asset_tree,
        &applied.keychain_catalog,
        &store,
        &known_hosts_path,
        SnapshotSyncPreferences {
            auto_sync_enabled: true,
            selected_primary_remote_id: Some("remote-primary".into()),
            selected_mirror_remote_ids: vec!["remote-mirror".into()],
            last_sync_result: Some("degraded".into()),
        },
        &UiPreferences {
            theme_mode: ThemeMode::Light,
            always_on_top: true,
            right_panel_view: "appearance".into(),
            ..UiPreferences::default()
        },
    )
    .expect("export round-tripped snapshot");

    assert_eq!(round_tripped.asset_catalog, snapshot.asset_catalog);
    assert_eq!(
        round_tripped.ssh_secret_bundles["asset-gateway"]
            .password
            .as_deref(),
        Some("deploy-password")
    );
    assert_eq!(
        round_tripped
            .asset_catalog
            .nodes
            .get("asset-identity")
            .expect("identity-linked asset")
            .payload,
        snapshot
            .asset_catalog
            .nodes
            .get("asset-identity")
            .expect("original identity-linked asset")
            .payload
    );
    assert_eq!(round_tripped.keychain_catalog, snapshot.keychain_catalog);
    assert_eq!(
        round_tripped.keychain_identity_secret_bundles["identity-ops"]
            .password
            .as_deref(),
        Some("ops-password")
    );
    assert_eq!(
        round_tripped.keychain_key_secret_bundles["key-prod"]
            .passphrase
            .as_deref(),
        Some("key-passphrase")
    );
    assert!(round_tripped.known_hosts.is_empty());
    assert_eq!(
        round_tripped.sync_preferences,
        SnapshotSyncPreferences::default()
    );
    assert_eq!(
        round_tripped.ui_preferences,
        SnapshotUiPreferences::default()
    );

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn apply_vault_snapshot_canonicalizes_remapped_keychain_secret_refs_and_preserves_links() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("keychain-remap");
    let identity_id = "identity-ops-remote-merge-1";
    let key_id = "key-prod-remote-merge-1";
    let canonical_identity_ref = format!("keychain/identity/{identity_id}");
    let canonical_key_ref = format!("keychain/key/{key_id}");

    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["asset-remote".into()],
            nodes: BTreeMap::from([(
                "asset-remote".into(),
                VaultAssetNode {
                    id: "asset-remote".into(),
                    parent_id: None,
                    title: "Remote Gateway".into(),
                    kind: VaultAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: VaultAssetPayload::SshConnection(Box::new(
                        mica_term::app::vault::model::VaultSshConnectionSpec {
                            host: "remote.example.com".into(),
                            user: "remote-ops".into(),
                            port: "22".into(),
                            auth_method: "private-key".into(),
                            auth_source: "keychain-identity".into(),
                            keychain_identity_id: Some(identity_id.into()),
                            private_key_source: String::new(),
                            private_key_path: String::new(),
                            environment: "prod".into(),
                            proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                            remark: "restored from attach merge".into(),
                            credential_ref: None,
                        },
                    )),
                },
            )]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::new(),
        keychain_catalog: KeychainCatalog {
            root_ids: vec![identity_id.into(), key_id.into()],
            nodes: BTreeMap::from([
                (
                    identity_id.into(),
                    KeychainNode {
                        id: identity_id.into(),
                        parent_id: None,
                        title: "Remote Ops".into(),
                        kind: KeychainNodeKind::Identity,
                        child_ids: Vec::new(),
                        payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                            username: "remote-ops".into(),
                            auth_kind: KeychainIdentityAuthKind::SshKey,
                            ssh_key_id: Some(key_id.into()),
                            credential_ref: Some("keychain/identity/identity-ops".into()),
                            remark: "needs canonical ref after remap".into(),
                        }),
                    },
                ),
                (
                    key_id.into(),
                    KeychainNode {
                        id: key_id.into(),
                        parent_id: None,
                        title: "Remote Key".into(),
                        kind: KeychainNodeKind::SshKey,
                        child_ids: Vec::new(),
                        payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                            algorithm: "ed25519".into(),
                            fingerprint: "SHA256:remote-key".into(),
                            public_key: "ssh-ed25519 AAAAREMOTE remote@example".into(),
                            comment: "remote@example".into(),
                            credential_ref: Some("keychain/key/key-prod".into()),
                            remark: "needs canonical ref after remap".into(),
                        }),
                    },
                ),
            ]),
            merge_metadata: BTreeMap::new(),
        },
        keychain_identity_secret_bundles: BTreeMap::from([(
            identity_id.into(),
            StoredSshSecretBundle {
                password: Some("remote-password".into()),
                ..StoredSshSecretBundle::default()
            },
        )]),
        keychain_key_secret_bundles: BTreeMap::from([(
            key_id.into(),
            StoredSshSecretBundle {
                private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                passphrase: Some("remote-passphrase".into()),
                ..StoredSshSecretBundle::default()
            },
        )]),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    };

    let applied = apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply");
    let restored_host = applied
        .asset_tree
        .ssh_connection_spec("asset-remote")
        .expect("restored host");

    assert_eq!(
        restored_host.keychain_identity_id.as_deref(),
        Some(identity_id)
    );

    let identity = applied
        .keychain_catalog
        .nodes
        .get(identity_id)
        .expect("restored identity node");
    match &identity.payload {
        KeychainNodePayload::Identity(spec) => {
            assert_eq!(spec.ssh_key_id.as_deref(), Some(key_id));
            assert_eq!(
                spec.credential_ref.as_deref(),
                Some(canonical_identity_ref.as_str())
            );
        }
        other => panic!("unexpected identity payload: {other:?}"),
    }

    let key = applied
        .keychain_catalog
        .nodes
        .get(key_id)
        .expect("restored key node");
    match &key.payload {
        KeychainNodePayload::SshKey(spec) => {
            assert_eq!(
                spec.credential_ref.as_deref(),
                Some(canonical_key_ref.as_str())
            );
        }
        other => panic!("unexpected key payload: {other:?}"),
    }

    assert!(
        store
            .get_secret(canonical_identity_ref.as_str())
            .expect("load canonical identity ref")
            .is_some()
    );
    assert!(
        store
            .get_secret(canonical_key_ref.as_str())
            .expect("load canonical key ref")
            .is_some()
    );
    assert_eq!(
        store
            .get_secret("keychain/identity/identity-ops")
            .expect("load stale identity ref"),
        None
    );
    assert_eq!(
        store
            .get_secret("keychain/key/key-prod")
            .expect("load stale key ref"),
        None
    );

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn vault_snapshot_catalog_preserves_snippets_alongside_console_assets() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("snippets");
    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec![
                "folder-prod".into(),
                "snippet-package-1".into(),
                "snippet-root".into(),
            ],
            nodes: BTreeMap::from([
                (
                    "folder-prod".into(),
                    VaultAssetNode {
                        id: "folder-prod".into(),
                        parent_id: None,
                        title: "Production".into(),
                        kind: VaultAssetKind::Folder,
                        child_ids: vec!["asset-gateway".into()],
                        payload: VaultAssetPayload::Folder,
                    },
                ),
                (
                    "asset-gateway".into(),
                    VaultAssetNode {
                        id: "asset-gateway".into(),
                        parent_id: Some("folder-prod".into()),
                        title: "Gateway".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            mica_term::app::vault::model::VaultSshConnectionSpec {
                                host: "prod.example.com".into(),
                                user: "deploy".into(),
                                port: "22".into(),
                                auth_method: "private-key".into(),
                                auth_source: "manual".into(),
                                keychain_identity_id: None,
                                private_key_source: "content".into(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                                remark: String::new(),
                                credential_ref: None,
                            },
                        )),
                    },
                ),
                (
                    "snippet-package-1".into(),
                    VaultAssetNode {
                        id: "snippet-package-1".into(),
                        parent_id: None,
                        title: "Deploy".into(),
                        kind: VaultAssetKind::SnippetPackage,
                        child_ids: vec!["snippet-child".into()],
                        payload: VaultAssetPayload::SnippetPackage,
                    },
                ),
                (
                    "snippet-child".into(),
                    VaultAssetNode {
                        id: "snippet-child".into(),
                        parent_id: Some("snippet-package-1".into()),
                        title: "Deploy prod".into(),
                        kind: VaultAssetKind::Snippet,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::Snippet(VaultSnippetSpec {
                            script: "kubectl apply -f prod.yaml".into(),
                            package_id: Some("snippet-package-1".into()),
                        }),
                    },
                ),
                (
                    "snippet-root".into(),
                    VaultAssetNode {
                        id: "snippet-root".into(),
                        parent_id: None,
                        title: "Restart API".into(),
                        kind: VaultAssetKind::Snippet,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::Snippet(VaultSnippetSpec {
                            script: "kubectl rollout restart deploy/api".into(),
                            package_id: None,
                        }),
                    },
                ),
            ]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::new(),
        keychain_catalog: KeychainCatalog::default(),
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    };

    let applied = apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply");
    let package_node = applied
        .asset_tree
        .node("snippet-package-1")
        .expect("snippet package");

    assert_eq!(package_node.kind, ConsoleAssetKind::SnippetPackage);
    assert_eq!(package_node.kind.domain(), AssetDomain::Snippets);
    assert_eq!(
        applied.asset_tree.snippet_spec("snippet-child"),
        Some(&AssetSnippetSpec {
            script: "kubectl apply -f prod.yaml".into(),
            package_id: Some("snippet-package-1".into()),
        })
    );

    let round_tripped = asset_tree_to_vault_catalog(&applied.asset_tree);
    assert_eq!(
        round_tripped
            .nodes
            .get("snippet-root")
            .expect("root snippet")
            .kind
            .domain(),
        VaultAssetDomain::Snippets
    );
    assert_eq!(
        round_tripped
            .nodes
            .get("snippet-root")
            .expect("root snippet")
            .payload,
        VaultAssetPayload::Snippet(VaultSnippetSpec {
            script: "kubectl rollout restart deploy/api".into(),
            package_id: None,
        })
    );

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn vault_snapshot_round_trip_preserves_merge_metadata_and_tombstones() {
    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["asset-live".into()],
            nodes: BTreeMap::from([(
                "asset-live".into(),
                VaultAssetNode {
                    id: "asset-live".into(),
                    parent_id: None,
                    title: "Live".into(),
                    kind: VaultAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: VaultAssetPayload::Folder,
                },
            )]),
            merge_metadata: BTreeMap::from([
                (
                    "asset-live".into(),
                    VaultNodeMergeMetadata {
                        last_modified_at: Some("2026-04-01T09:00:00Z".into()),
                        last_modified_by_device: Some("device-live".into()),
                        deleted_at: None,
                    },
                ),
                (
                    "asset-deleted".into(),
                    VaultNodeMergeMetadata {
                        last_modified_at: Some("2026-04-01T08:00:00Z".into()),
                        last_modified_by_device: Some("device-delete".into()),
                        deleted_at: Some("2026-04-01T08:30:00Z".into()),
                    },
                ),
            ]),
        },
        ssh_secret_bundles: BTreeMap::new(),
        keychain_catalog: KeychainCatalog {
            root_ids: vec!["identity-ops".into()],
            nodes: BTreeMap::from([(
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: None,
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::Password,
                        ssh_key_id: None,
                        credential_ref: Some("keychain/identity/identity-ops".into()),
                        remark: String::new(),
                    }),
                },
            )]),
            merge_metadata: BTreeMap::from([
                (
                    "identity-ops".into(),
                    mica_term::app::keychain::model::KeychainNodeMergeMetadata {
                        last_modified_at: Some("2026-04-01T09:10:00Z".into()),
                        last_modified_by_device: Some("device-live".into()),
                        deleted_at: None,
                    },
                ),
                (
                    "identity-old".into(),
                    mica_term::app::keychain::model::KeychainNodeMergeMetadata {
                        last_modified_at: Some("2026-04-01T07:00:00Z".into()),
                        last_modified_by_device: Some("device-legacy".into()),
                        deleted_at: Some("2026-04-01T07:30:00Z".into()),
                    },
                ),
            ]),
        },
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    };

    let encoded = serde_json::to_vec_pretty(&snapshot).expect("encode snapshot");
    let decoded: VaultSnapshot = serde_json::from_slice(&encoded).expect("decode snapshot");

    assert_eq!(
        decoded.asset_catalog.merge_metadata["asset-live"]
            .last_modified_by_device
            .as_deref(),
        Some("device-live")
    );
    assert_eq!(
        decoded.asset_catalog.merge_metadata["asset-deleted"]
            .deleted_at
            .as_deref(),
        Some("2026-04-01T08:30:00Z")
    );
    assert_eq!(
        decoded.keychain_catalog.merge_metadata["identity-ops"]
            .last_modified_at
            .as_deref(),
        Some("2026-04-01T09:10:00Z")
    );
    assert_eq!(
        decoded.keychain_catalog.merge_metadata["identity-old"]
            .deleted_at
            .as_deref(),
        Some("2026-04-01T07:30:00Z")
    );
}

#[test]
fn apply_vault_snapshot_keeps_empty_secret_entries_absent() {
    let credential_ref = "ssh/saved-secrets/asset-empty";
    let identity_credential_ref = "keychain/identity/identity-empty";
    let key_credential_ref = "keychain/key/key-empty";
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("empty-secrets");

    store
        .put_secret(credential_ref, "stale-secret")
        .expect("persist stale secret");
    store
        .put_secret(identity_credential_ref, "stale-identity-secret")
        .expect("persist stale identity secret");
    store
        .put_secret(key_credential_ref, "stale-key-secret")
        .expect("persist stale key secret");

    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["asset-empty".into()],
            nodes: BTreeMap::from([(
                "asset-empty".into(),
                VaultAssetNode {
                    id: "asset-empty".into(),
                    parent_id: None,
                    title: "Empty Secret Asset".into(),
                    kind: VaultAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: VaultAssetPayload::SshConnection(Box::new(
                        mica_term::app::vault::model::VaultSshConnectionSpec {
                            host: "empty.example.com".into(),
                            user: "deploy".into(),
                            port: "22".into(),
                            auth_method: "password".into(),
                            auth_source: "manual".into(),
                            keychain_identity_id: None,
                            private_key_source: "content".into(),
                            private_key_path: String::new(),
                            environment: "prod".into(),
                            proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                            remark: String::new(),
                            credential_ref: Some(credential_ref.into()),
                        },
                    )),
                },
            )]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::new(),
        keychain_catalog: sample_keychain_catalog(identity_credential_ref, key_credential_ref),
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    };

    apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply snapshot");

    assert_eq!(store.get_secret(credential_ref).unwrap(), None);
    assert_eq!(store.get_secret(identity_credential_ref).unwrap(), None);
    assert_eq!(store.get_secret(key_credential_ref).unwrap(), None);

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn apply_vault_snapshot_rekeys_duplicate_saved_ssh_secret_refs_per_asset() {
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("duplicate-ssh-secret-refs");
    let shared_ref = "ssh/saved-secrets/shared";
    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["asset-a".into(), "asset-b".into()],
            nodes: BTreeMap::from([
                (
                    "asset-a".into(),
                    VaultAssetNode {
                        id: "asset-a".into(),
                        parent_id: None,
                        title: "A".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            mica_term::app::vault::model::VaultSshConnectionSpec {
                                host: "a.example.com".into(),
                                user: "ops".into(),
                                port: "22".into(),
                                auth_method: "password".into(),
                                auth_source: "manual".into(),
                                keychain_identity_id: None,
                                private_key_source: "content".into(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                                remark: String::new(),
                                credential_ref: Some(shared_ref.into()),
                            },
                        )),
                    },
                ),
                (
                    "asset-b".into(),
                    VaultAssetNode {
                        id: "asset-b".into(),
                        parent_id: None,
                        title: "B".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            mica_term::app::vault::model::VaultSshConnectionSpec {
                                host: "b.example.com".into(),
                                user: "ops".into(),
                                port: "22".into(),
                                auth_method: "password".into(),
                                auth_source: "manual".into(),
                                keychain_identity_id: None,
                                private_key_source: "content".into(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: mica_term::app::vault::model::VaultSshProxySpec::None,
                                remark: String::new(),
                                credential_ref: Some(shared_ref.into()),
                            },
                        )),
                    },
                ),
            ]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::from([
            (
                "asset-a".into(),
                StoredSshSecretBundle {
                    password: Some("alpha".into()),
                    ..StoredSshSecretBundle::default()
                },
            ),
            (
                "asset-b".into(),
                StoredSshSecretBundle {
                    password: Some("bravo".into()),
                    ..StoredSshSecretBundle::default()
                },
            ),
        ]),
        keychain_catalog: KeychainCatalog::default(),
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: SnapshotUiPreferences::default(),
    };

    let applied =
        apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply vault snapshot");
    let asset_a_ref = applied
        .asset_tree
        .ssh_connection_spec("asset-a")
        .expect("asset a spec")
        .credential_ref
        .clone()
        .expect("asset a credential ref");
    let asset_b_ref = applied
        .asset_tree
        .ssh_connection_spec("asset-b")
        .expect("asset b spec")
        .credential_ref
        .clone()
        .expect("asset b credential ref");

    assert_eq!(
        asset_a_ref,
        ssh_credential_ref("asset-a", SshCredentialKind::SavedSecrets)
    );
    assert_eq!(
        asset_b_ref,
        ssh_credential_ref("asset-b", SshCredentialKind::SavedSecrets)
    );
    assert_ne!(asset_a_ref, asset_b_ref);
    assert_eq!(
        load_secret_bundle(&store, asset_a_ref.as_str())
            .expect("load asset a secret")
            .password
            .as_deref(),
        Some("alpha")
    );
    assert_eq!(
        load_secret_bundle(&store, asset_b_ref.as_str())
            .expect("load asset b secret")
            .password
            .as_deref(),
        Some("bravo")
    );
    assert_eq!(store.get_secret(shared_ref).unwrap(), None);

    let _ = fs::remove_file(known_hosts_path);
}
