use std::collections::BTreeMap;
use std::fs;

use mica_term::app::ssh::credentials::StoredSshSecretBundle;
use mica_term::app::vault::cache::{cache_path_for_vault, load_encrypted_cache, store_encrypted_cache};
use mica_term::app::vault::crypto::{
    decrypt_snapshot, encrypt_snapshot, generate_vault_key, unwrap_vault_key, wrap_vault_key,
};
use mica_term::app::vault::model::{
    KdfConfig, SnapshotSyncPreferences, SnapshotUiPreferences, VaultAssetCatalog, VaultAssetKind,
    VaultAssetNode, VaultAssetPayload, VaultKnownHostEntry, VaultSshConnectionSpec,
    VaultSnapshot, VaultSshProxySpec,
};
use secrecy::SecretString;
use uuid::Uuid;

fn sample_snapshot() -> VaultSnapshot {
    VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog {
            root_ids: vec!["ssh-prod".into()],
            nodes: BTreeMap::from([(
                "ssh-prod".into(),
                VaultAssetNode {
                    id: "ssh-prod".into(),
                    parent_id: None,
                    title: "Prod SSH".into(),
                    kind: VaultAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: VaultAssetPayload::SshConnection(Box::new(VaultSshConnectionSpec {
                        host: "prod.example.com".into(),
                        user: "deploy".into(),
                        port: "22".into(),
                        auth_method: "private-key".into(),
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: VaultSshProxySpec::None,
                        remark: "critical path".into(),
                        credential_ref: Some("ssh/saved-secrets/ssh-prod".into()),
                    })),
                },
            )]),
        },
        ssh_secret_bundles: BTreeMap::from([(
            "ssh-prod".into(),
            StoredSshSecretBundle {
                password: None,
                private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
                passphrase: Some("master-key-passphrase".into()),
                proxy_socks5_password: None,
            },
        )]),
        keychain_catalog: Default::default(),
        keychain_identity_secret_bundles: BTreeMap::new(),
        keychain_key_secret_bundles: BTreeMap::new(),
        known_hosts: vec![VaultKnownHostEntry {
            host_pattern: "[prod.example.com]:22".into(),
            public_key: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPROD".into(),
        }],
        sync_preferences: SnapshotSyncPreferences {
            auto_sync_enabled: true,
            selected_primary_remote_id: Some("remote-s3".into()),
            selected_mirror_remote_ids: vec!["remote-github".into()],
            last_sync_result: Some("success".into()),
        },
        ui_preferences: SnapshotUiPreferences {
            theme_mode: Some("dark".into()),
            always_on_top: Some(false),
        },
    }
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|window| window == needle)
}

#[test]
fn argon2id_derived_kek_wraps_and_unwraps_a_random_vault_key() {
    let password = SecretString::new("correct horse battery staple".into());
    let kdf = KdfConfig::Argon2id {
        memory_cost_kib: 65_536,
        time_cost: 3,
        parallelism: 1,
        salt_b64: "c2FsdC12YXVsdC0wMDE=".into(),
    };
    let vault_key = generate_vault_key();

    let wrapped = wrap_vault_key(&password, &kdf, &vault_key).unwrap();
    let unwrapped = unwrap_vault_key(&password, &wrapped).unwrap();

    assert_eq!(vault_key.to_vec(), unwrapped.to_vec());
}

#[test]
fn wrong_password_fails_to_unwrap_cleanly() {
    let password = SecretString::new("correct horse battery staple".into());
    let wrong_password = SecretString::new("wrong-pass".into());
    let kdf = KdfConfig::Argon2id {
        memory_cost_kib: 65_536,
        time_cost: 3,
        parallelism: 1,
        salt_b64: "c2FsdC12YXVsdC0wMDI=".into(),
    };
    let vault_key = generate_vault_key();
    let wrapped = wrap_vault_key(&password, &kdf, &vault_key).unwrap();

    assert!(unwrap_vault_key(&wrong_password, &wrapped).is_err());
}

#[test]
fn vault_snapshot_content_is_compressed_before_encryption_and_decrypts_back() {
    let snapshot = sample_snapshot();
    let vault_key = generate_vault_key();
    let raw = serde_json::to_vec(&snapshot).unwrap();

    let encrypted = encrypt_snapshot(&snapshot, &vault_key).unwrap();
    let decrypted = decrypt_snapshot(&encrypted, &vault_key).unwrap();

    assert!(encrypted.compressed_len < raw.len());
    assert_eq!(decrypted, snapshot);
}

#[test]
fn encrypted_cache_roundtrips_through_disk_without_plaintext_markers() {
    let snapshot = sample_snapshot();
    let vault_key = generate_vault_key();
    let encrypted = encrypt_snapshot(&snapshot, &vault_key).unwrap();
    let temp_root = std::env::temp_dir().join(format!("mica-term-vault-cache-{}", Uuid::new_v4()));
    let expected_path = cache_path_for_vault(&temp_root, "vault-main");

    let stored_path = store_encrypted_cache(&temp_root, "vault-main", &encrypted).unwrap();
    let encoded = fs::read(&stored_path).unwrap();
    let loaded = load_encrypted_cache(&temp_root, "vault-main")
        .unwrap()
        .expect("encrypted cache should exist");
    let decrypted = decrypt_snapshot(&loaded, &vault_key).unwrap();

    assert_eq!(stored_path, expected_path);
    assert!(!contains_bytes(&encoded, b"private_key_content"));
    assert!(!contains_bytes(&encoded, b"BEGIN OPENSSH PRIVATE KEY"));
    assert_eq!(decrypted, snapshot);

    let _ = fs::remove_file(stored_path);
    let _ = fs::remove_dir_all(temp_root);
}
