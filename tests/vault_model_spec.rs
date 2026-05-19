use std::collections::BTreeMap;

use mica_term::app::keychain::model::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::app::ssh::credentials::StoredSshSecretBundle;
use mica_term::app::vault::bootstrap::{
    LocalVaultBootstrapState, load_local_vault_bootstrap_state, save_local_vault_bootstrap_state,
};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind,
    GitHostKind, GitRemoteSafetyStatus, GitRepoRemoteDraft, KdfConfig, PackLayout, PackRef,
    ProviderAuthKind, ProviderKind, RemoteHealth, RemoteHealthStatus, RemoteRole,
    SnapshotSyncPreferences, SnapshotUiPreferences, VaultAssetCatalog, VaultAssetKind,
    VaultAssetNode, VaultAssetPayload, VaultHead, VaultKnownHostEntry, VaultManifest,
    VaultSnapshot, VaultSocks5ProxySpec, VaultSshConnectionSpec, VaultSshProxySpec,
};
use serde_json::json;

fn sample_keychain_catalog() -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-prod".into(), "identity-prod".into()],
        nodes: BTreeMap::from([
            (
                "folder-prod".into(),
                KeychainNode {
                    id: "folder-prod".into(),
                    parent_id: None,
                    title: "Production".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-prod".into(),
                KeychainNode {
                    id: "identity-prod".into(),
                    parent_id: None,
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::SshKey,
                        ssh_key_id: Some("key-prod".into()),
                        credential_ref: None,
                        remark: "prod identity".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-prod".into()),
                    title: "Prod SSH Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:key-prod".into(),
                        public_key: "ssh-ed25519 AAAAC3NzaKeyProd".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some("keychain/key/key-prod".into()),
                        remark: "generated key".into(),
                    }),
                },
            ),
        ]),
        merge_metadata: BTreeMap::new(),
    }
}

#[test]
fn vault_head_roundtrip_preserves_core_revision_fields() {
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: "rev-0001".into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-laptop".into(),
        committed_at: "2026-03-28T16:12:00Z".into(),
        committed_by_device: "device-laptop".into(),
        payload_hash: "sha256:payload-001".into(),
        manifest_ref: "manifest/rev-0001.bin".into(),
        wrapped_vault_key: "base64:wrapped-key".into(),
        kdf: KdfConfig::Argon2id {
            memory_cost_kib: 65_536,
            time_cost: 3,
            parallelism: 1,
            salt_b64: "c2FsdC0wMDE=".into(),
        },
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::ObjectSet,
    };

    let encoded = serde_json::to_string_pretty(&head).unwrap();
    let decoded: VaultHead = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.vault_revision, "rev-0001");
    assert_eq!(decoded.parent_revision.as_deref(), Some("rev-0000"));
    assert_eq!(decoded.committed_at, "2026-03-28T16:12:00Z");
    assert_eq!(decoded.committed_by_device, "device-laptop");
    assert_eq!(decoded.payload_hash, "sha256:payload-001");
    assert_eq!(decoded.manifest_ref, "manifest/rev-0001.bin");
    assert_eq!(decoded.wrapped_vault_key, "base64:wrapped-key");
}

#[test]
fn vault_head_serializes_real_commit_metadata() {
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: "rev-0042".into(),
        parent_revision: Some("rev-0041".into()),
        device_id: "device-laptop".into(),
        committed_at: "2026-03-31T09:30:15Z".into(),
        committed_by_device: "device-workstation".into(),
        payload_hash: "sha256:payload-0042".into(),
        manifest_ref: "manifest/rev-0042.bin".into(),
        wrapped_vault_key: "base64:wrapped-key".into(),
        kdf: KdfConfig::Argon2id {
            memory_cost_kib: 65_536,
            time_cost: 3,
            parallelism: 1,
            salt_b64: "c2FsdC0wNDI=".into(),
        },
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::ObjectSet,
    };

    let encoded = serde_json::to_value(&head).unwrap();
    let decoded: VaultHead = serde_json::from_value(encoded.clone()).unwrap();

    assert_eq!(encoded["committed_at"], "2026-03-31T09:30:15Z");
    assert_eq!(encoded["committed_by_device"], "device-workstation");
    assert!(encoded.get("created_at").is_none());
    assert_eq!(decoded.committed_at, "2026-03-31T09:30:15Z");
    assert_eq!(decoded.committed_by_device, "device-workstation");
}

#[test]
fn local_bootstrap_state_persists_durable_sync_state_fields() {
    let path = std::env::temp_dir().join(format!(
        "mica-term-local-vault-state-{}.json",
        uuid::Uuid::new_v4()
    ));
    let state = LocalVaultBootstrapState {
        bundle: BootstrapBundle {
            format_version: 1,
            vault_id: "vault-main".into(),
            remotes: vec![BootstrapRemoteConfig {
                remote_id: "remote-primary".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::GiteeGist,
                locator: BootstrapRemoteLocator::GiteeGist {
                    gist_id: "gist-main".into(),
                },
                credential_ref: Some("vault/bootstrap/remote-primary".into()),
                auth_kind: ProviderAuthKind::Pat,
                last_health: None,
            }],
            auto_sync_enabled: true,
            bootstrap_cipher: CipherKind::XChaCha20Poly1305,
            bootstrap_kdf: Some(KdfConfig::Argon2id {
                memory_cost_kib: 19_456,
                time_cost: 2,
                parallelism: 1,
                salt_b64: "bootstrap-salt".into(),
            }),
        },
        wrapped_vault_key: "wrapped-key".into(),
        kdf: KdfConfig::Argon2id {
            memory_cost_kib: 19_456,
            time_cost: 2,
            parallelism: 1,
            salt_b64: "vault-salt".into(),
        },
        device_id: "device-laptop".into(),
        logical_revision: Some("logical-merge-0042".into()),
        transport_revision_hint: Some("git:0123456789abcdef".into()),
        base_revision: Some("rev-0041".into()),
        current_revision: Some("rev-0042".into()),
        local_snapshot_hash: Some("sha256:local-snapshot".into()),
        last_local_change_at: Some("2026-03-31T09:40:00Z".into()),
        last_successful_push_at: Some("2026-03-31T09:41:00Z".into()),
        last_successful_pull_at: Some("2026-03-31T09:39:30Z".into()),
        last_sync_error: Some("mirror degraded".into()),
        remote_safety_status: GitRemoteSafetyStatus::Paused,
    };

    save_local_vault_bootstrap_state(&path, &state).unwrap();
    let loaded = load_local_vault_bootstrap_state(&path)
        .unwrap()
        .expect("persisted local bootstrap state");

    assert_eq!(loaded.device_id, "device-laptop");
    assert_eq!(
        loaded.logical_revision.as_deref(),
        Some("logical-merge-0042")
    );
    assert_eq!(
        loaded.transport_revision_hint.as_deref(),
        Some("git:0123456789abcdef")
    );
    assert_eq!(loaded.base_revision.as_deref(), Some("rev-0041"));
    assert_eq!(loaded.current_revision.as_deref(), Some("rev-0042"));
    assert_eq!(
        loaded.local_snapshot_hash.as_deref(),
        Some("sha256:local-snapshot")
    );
    assert_eq!(
        loaded.last_local_change_at.as_deref(),
        Some("2026-03-31T09:40:00Z")
    );
    assert_eq!(
        loaded.last_successful_push_at.as_deref(),
        Some("2026-03-31T09:41:00Z")
    );
    assert_eq!(
        loaded.last_successful_pull_at.as_deref(),
        Some("2026-03-31T09:39:30Z")
    );
    assert_eq!(loaded.last_sync_error.as_deref(), Some("mirror degraded"));
    assert_eq!(loaded.remote_safety_status, GitRemoteSafetyStatus::Paused);

    let _ = std::fs::remove_file(path);
}

#[test]
fn git_repo_remote_draft_defaults_to_gitee_https_contract() {
    let draft = GitRepoRemoteDraft::default();

    assert_eq!(draft.host_kind, GitHostKind::Gitee);
    assert_eq!(draft.remote_url, "");
    assert_eq!(draft.branch, "main");
    assert_eq!(draft.auth_kind, ProviderAuthKind::HttpsCredentials);
}

#[test]
fn vault_manifest_roundtrip_preserves_multiple_pack_refs_and_defaults() {
    let manifest = VaultManifest {
        format_version: 1,
        snapshot_schema_version: 1,
        packs: vec![
            PackRef {
                pack_id: "pack-0001".into(),
                object_name: "packs/pack-0001.bin".into(),
                size_bytes: 4096,
                digest: "sha256:pack-0001".into(),
            },
            PackRef {
                pack_id: "pack-0002".into(),
                object_name: "packs/pack-0002.bin".into(),
                size_bytes: 8192,
                digest: "sha256:pack-0002".into(),
            },
        ],
        feature_flags: vec!["ssh-proxy-chain".into(), "known-hosts".into()],
        provider_capability_fallbacks: BTreeMap::from([(
            "github-gist".into(),
            "bundled-files".into(),
        )]),
    };

    let encoded = serde_json::to_string(&manifest).unwrap();
    let decoded: VaultManifest = serde_json::from_str(&encoded).unwrap();
    let defaults: VaultManifest = serde_json::from_value(json!({ "packs": [] })).unwrap();

    assert_eq!(decoded.packs.len(), 2);
    assert_eq!(decoded.packs[0].pack_id, "pack-0001");
    assert_eq!(
        decoded.feature_flags,
        vec!["ssh-proxy-chain", "known-hosts"]
    );
    assert_eq!(
        decoded.provider_capability_fallbacks["github-gist"],
        "bundled-files"
    );
    assert_eq!(defaults.format_version, 1);
    assert_eq!(defaults.snapshot_schema_version, 1);
    assert!(defaults.feature_flags.is_empty());
    assert!(defaults.provider_capability_fallbacks.is_empty());
}

#[test]
fn vault_snapshot_roundtrip_preserves_assets_secrets_hosts_and_preferences() {
    let snapshot = VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["folder-prod".into()],
            nodes: BTreeMap::from([
                (
                    "folder-prod".into(),
                    VaultAssetNode {
                        id: "folder-prod".into(),
                        parent_id: None,
                        title: "Production".into(),
                        kind: VaultAssetKind::Folder,
                        child_ids: vec!["ssh-jump".into()],
                        payload: VaultAssetPayload::Folder,
                    },
                ),
                (
                    "ssh-jump".into(),
                    VaultAssetNode {
                        id: "ssh-jump".into(),
                        parent_id: Some("folder-prod".into()),
                        title: "Jump Host".into(),
                        kind: VaultAssetKind::SshConnection,
                        child_ids: Vec::new(),
                        payload: VaultAssetPayload::SshConnection(Box::new(
                            VaultSshConnectionSpec {
                                host: "jump.example.com".into(),
                                user: "ops".into(),
                                port: "22".into(),
                                auth_method: "private-key".into(),
                                auth_source: "manual".into(),
                                keychain_identity_id: None,
                                private_key_source: "content".into(),
                                private_key_path: String::new(),
                                environment: "prod".into(),
                                proxy: VaultSshProxySpec::Socks5(VaultSocks5ProxySpec {
                                    host: "proxy.internal".into(),
                                    port: "1080".into(),
                                    username: "relay".into(),
                                    password_credential_ref: Some("ssh/proxy-secrets/jump".into()),
                                }),
                                remark: "shared bastion".into(),
                                credential_ref: Some("ssh/saved-secrets/ssh-jump".into()),
                            },
                        )),
                    },
                ),
            ]),
            merge_metadata: BTreeMap::new(),
        },
        ssh_secret_bundles: BTreeMap::from([(
            "ssh-jump".into(),
            StoredSshSecretBundle {
                password: None,
                private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                passphrase: Some("vault-passphrase".into()),
                proxy_socks5_password: Some("proxy-pass".into()),
            },
        )]),
        keychain_catalog: sample_keychain_catalog(),
        keychain_identity_secret_bundles: BTreeMap::from([(
            "identity-prod".into(),
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
            host_pattern: "[jump.example.com]:22".into(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBastion".into(),
        }],
        sync_preferences: SnapshotSyncPreferences {
            auto_sync_enabled: true,
            selected_primary_remote_id: Some("remote-s3".into()),
            selected_mirror_remote_ids: vec!["remote-github".into()],
            last_sync_result: Some("success@2026-03-28T16:30:00Z".into()),
        },
        ui_preferences: SnapshotUiPreferences {
            theme_mode: Some("dark".into()),
            always_on_top: Some(false),
        },
    };

    let encoded = serde_json::to_string_pretty(&snapshot).unwrap();
    let decoded: VaultSnapshot = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.asset_catalog.root_ids, vec!["folder-prod"]);
    assert_eq!(decoded.asset_catalog.nodes["ssh-jump"].title, "Jump Host");
    assert_eq!(decoded.keychain_catalog.root_ids.len(), 2);
    assert_eq!(
        decoded.keychain_identity_secret_bundles["identity-prod"]
            .password
            .as_deref(),
        Some("ops-password")
    );
    assert_eq!(
        decoded.keychain_key_secret_bundles["key-prod"]
            .private_key_content
            .as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert_eq!(
        decoded.ssh_secret_bundles["ssh-jump"].passphrase.as_deref(),
        Some("vault-passphrase")
    );
    assert_eq!(decoded.known_hosts[0].host_pattern, "[jump.example.com]:22");
    assert!(decoded.sync_preferences.auto_sync_enabled);
    assert_eq!(
        decoded
            .sync_preferences
            .selected_primary_remote_id
            .as_deref(),
        Some("remote-s3")
    );
    assert_eq!(decoded.ui_preferences.theme_mode.as_deref(), Some("dark"));

    let defaults: VaultSnapshot = serde_json::from_value(json!({
        "asset_catalog": {
            "root_ids": [],
            "nodes": {}
        }
    }))
    .unwrap();
    assert!(defaults.keychain_catalog.root_ids.is_empty());
    assert!(defaults.keychain_identity_secret_bundles.is_empty());
    assert!(defaults.keychain_key_secret_bundles.is_empty());
}

#[test]
fn bootstrap_bundle_roundtrip_preserves_git_repo_locator_and_dual_auth_kinds() {
    let bundle = BootstrapBundle {
        format_version: 1,
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
                remote_id: "remote-primary".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::GitRepo,
                locator: BootstrapRemoteLocator::GitRepo {
                    host_kind: GitHostKind::Gitee,
                    remote_url: "https://gitee.com/demo/mica-vault.git".into(),
                    branch: "mica-vault".into(),
                    base_url: None,
                    api_base_url: None,
                    namespace: None,
                    repository: None,
                    root_path: None,
                    display_name: None,
                },
                credential_ref: Some("vault/bootstrap/remote-primary".into()),
                auth_kind: ProviderAuthKind::HttpsCredentials,
                last_health: None,
            },
            BootstrapRemoteConfig {
                remote_id: "remote-mirror".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitRepo,
                locator: BootstrapRemoteLocator::GitRepo {
                    host_kind: GitHostKind::Gitee,
                    remote_url: "git@gitee.com:demo/mica-vault.git".into(),
                    branch: "mirror".into(),
                    base_url: None,
                    api_base_url: None,
                    namespace: None,
                    repository: None,
                    root_path: None,
                    display_name: None,
                },
                credential_ref: Some("vault/bootstrap/remote-mirror".into()),
                auth_kind: ProviderAuthKind::SshKey,
                last_health: Some(RemoteHealth {
                    status: RemoteHealthStatus::Healthy,
                    checked_at: Some("2026-04-01T09:00:00Z".into()),
                    message: None,
                }),
            },
        ],
        auto_sync_enabled: true,
        bootstrap_cipher: CipherKind::XChaCha20Poly1305,
        bootstrap_kdf: Some(KdfConfig::Argon2id {
            memory_cost_kib: 65_536,
            time_cost: 3,
            parallelism: 1,
            salt_b64: "Ym9vdHN0cmFwLXNhbHQ=".into(),
        }),
    };

    let encoded = serde_json::to_string_pretty(&bundle).unwrap();
    let decoded: BootstrapBundle = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.remotes.len(), 2);
    assert_eq!(decoded.remotes[0].provider, ProviderKind::GitRepo);
    assert_eq!(
        decoded.remotes[0].auth_kind,
        ProviderAuthKind::HttpsCredentials
    );
    assert_eq!(decoded.remotes[1].auth_kind, ProviderAuthKind::SshKey);
    match &decoded.remotes[0].locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            ..
        } => {
            assert_eq!(*host_kind, GitHostKind::Gitee);
            assert_eq!(remote_url, "https://gitee.com/demo/mica-vault.git");
            assert_eq!(branch, "mica-vault");
        }
        other => panic!("unexpected locator: {other:?}"),
    }
}

#[test]
fn bootstrap_bundle_roundtrip_preserves_primary_mirror_and_credential_refs() {
    let bundle = BootstrapBundle {
        format_version: 1,
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
                remote_id: "remote-s3".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::S3Compatible,
                locator: BootstrapRemoteLocator::S3 {
                    bucket: "vault-bucket".into(),
                    prefix: "ssh-vault/main".into(),
                    endpoint: Some("https://s3.example.com".into()),
                    region: Some("us-east-1".into()),
                    force_path_style: true,
                },
                credential_ref: Some("vault/bootstrap/s3".into()),
                auth_kind: ProviderAuthKind::AwsStandardChain,
                last_health: Some(RemoteHealth {
                    status: RemoteHealthStatus::Healthy,
                    checked_at: Some("2026-03-28T16:35:00Z".into()),
                    message: None,
                }),
            },
            BootstrapRemoteConfig {
                remote_id: "remote-github".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitHubGist,
                locator: BootstrapRemoteLocator::GitHubGist {
                    gist_id: "gist-123".into(),
                },
                credential_ref: Some("vault/bootstrap/github".into()),
                auth_kind: ProviderAuthKind::DeviceFlow,
                last_health: Some(RemoteHealth {
                    status: RemoteHealthStatus::Degraded,
                    checked_at: Some("2026-03-28T16:40:00Z".into()),
                    message: Some("mirror lagging".into()),
                }),
            },
        ],
        auto_sync_enabled: true,
        bootstrap_cipher: CipherKind::XChaCha20Poly1305,
        bootstrap_kdf: Some(KdfConfig::Argon2id {
            memory_cost_kib: 65_536,
            time_cost: 3,
            parallelism: 1,
            salt_b64: "Ym9vdHN0cmFwLXNhbHQ=".into(),
        }),
    };

    let encoded = serde_json::to_string_pretty(&bundle).unwrap();
    let decoded: BootstrapBundle = serde_json::from_str(&encoded).unwrap();
    let defaults: BootstrapBundle =
        serde_json::from_value(json!({ "vault_id": "vault-main", "remotes": [] })).unwrap();

    assert_eq!(decoded.remotes.len(), 2);
    assert_eq!(decoded.remotes[0].role, RemoteRole::Primary);
    assert_eq!(decoded.remotes[1].role, RemoteRole::Mirror);
    assert_eq!(
        decoded.remotes[0].credential_ref.as_deref(),
        Some("vault/bootstrap/s3")
    );
    assert_eq!(defaults.format_version, 1);
    assert!(!defaults.auto_sync_enabled);
    assert!(defaults.remotes.is_empty());
}
