use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use mica_term::app::assets_catalog::asset_tree_to_vault_catalog;
use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, StoredSshSecretBundle, persist_secret_bundle,
};
use mica_term::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use mica_term::app::ui_preferences::UiPreferences;
use mica_term::app::vault::model::{
    SnapshotSyncPreferences, SnapshotUiPreferences, VaultAssetCatalog, VaultAssetDomain,
    VaultAssetKind, VaultAssetNode, VaultAssetPayload, VaultKnownHostEntry, VaultSnapshot,
    VaultSnippetSpec,
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

fn sample_asset_tree(credential_ref: &str) -> AssetTree {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Production");
    tree.insert_root_with_payload(
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
    tree.insert_child(&folder_id, ConsoleAssetKind::Folder, "Nested Folder");
    tree
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
    }
}

#[test]
fn export_vault_snapshot_excludes_ui_preferences_for_first_release() {
    let credential_ref = "ssh/saved-secrets/asset-2";
    let identity_credential_ref = "keychain/identity/identity-ops";
    let key_credential_ref = "keychain/key/key-prod";
    let store = MemoryCredentialStore::default();
    let tree = sample_asset_tree(credential_ref);
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

    assert_eq!(snapshot.asset_catalog.root_ids.len(), 2);
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
    assert_eq!(snapshot.known_hosts.len(), 1);
    assert_eq!(
        snapshot.known_hosts[0].host_pattern,
        "[prod.example.com]:22"
    );
    assert_eq!(snapshot.sync_preferences, sync_preferences);
    assert_eq!(snapshot.ui_preferences, SnapshotUiPreferences::default());

    let _ = fs::remove_file(known_hosts_path);
}

#[test]
fn apply_vault_snapshot_recreates_asset_catalog_secret_store_known_hosts_and_defaults_ui_preferences()
 {
    let credential_ref = "ssh/saved-secrets/gateway";
    let identity_credential_ref = "keychain/identity/identity-ops";
    let key_credential_ref = "keychain/key/key-prod";
    let store = MemoryCredentialStore::default();
    let known_hosts_path = temp_known_hosts_path("import");
    let public_key = sample_public_key();
    let asset_catalog = VaultAssetCatalog {
        root_ids: vec!["folder-prod".into(), "asset-gateway".into()],
        nodes: BTreeMap::from([
            (
                "folder-prod".into(),
                VaultAssetNode {
                    id: "folder-prod".into(),
                    parent_id: None,
                    title: "Production".into(),
                    kind: VaultAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: VaultAssetPayload::Folder,
                },
            ),
            (
                "asset-gateway".into(),
                VaultAssetNode {
                    id: "asset-gateway".into(),
                    parent_id: None,
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
        ]),
    };
    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog,
        ssh_secret_bundles: BTreeMap::from([(
            "asset-gateway".into(),
            StoredSshSecretBundle {
                password: None,
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
    };

    let applied =
        apply_vault_snapshot(&snapshot, &store, &known_hosts_path).expect("apply vault snapshot");

    assert_eq!(applied.asset_tree.root_ids().len(), 2);
    assert_eq!(applied.keychain_catalog.root_ids.len(), 2);
    assert_eq!(
        applied
            .asset_tree
            .node("asset-gateway")
            .expect("gateway node")
            .title,
        "Gateway"
    );
    assert_eq!(
        applied
            .asset_tree
            .ssh_connection_spec("asset-gateway")
            .expect("ssh payload")
            .credential_ref
            .as_deref(),
        Some(credential_ref)
    );
    assert_eq!(
        store
            .get_secret(credential_ref)
            .expect("load secret")
            .as_deref(),
        Some(
            "{\"password\":null,\"private_key_content\":\"-----BEGIN OPENSSH PRIVATE KEY-----\",\"passphrase\":\"hunter2\",\"proxy_socks5_password\":null}"
        )
    );
    assert_eq!(
        store
            .get_secret(identity_credential_ref)
            .expect("load identity secret")
            .as_deref(),
        Some(
            "{\"password\":\"ops-password\",\"private_key_content\":null,\"passphrase\":null,\"proxy_socks5_password\":null}"
        )
    );
    assert_eq!(
        store
            .get_secret(key_credential_ref)
            .expect("load key secret")
            .as_deref(),
        Some(
            "{\"password\":null,\"private_key_content\":\"-----BEGIN OPENSSH PRIVATE KEY-----\",\"passphrase\":\"key-passphrase\",\"proxy_socks5_password\":null}"
        )
    );
    assert_eq!(applied.sync_preferences, snapshot.sync_preferences);
    assert_eq!(applied.ui_preferences.theme_mode, ThemeMode::Dark);
    assert!(!applied.ui_preferences.always_on_top);

    let known_hosts = KnownHostsService::new(&known_hosts_path);
    let result = known_hosts
        .check("prod.example.com", 22, &public_key)
        .expect("check imported known_hosts entry");
    assert!(matches!(result, KnownHostCheck::Trusted));
    assert_eq!(
        public_key.fingerprint(HashAlg::Sha256).to_string().len() > 0,
        true
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
