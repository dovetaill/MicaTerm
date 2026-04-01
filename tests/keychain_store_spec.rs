use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mica_term::app::app_paths::{AppRootPathInputs, AppRootSource, resolve_app_root_paths};
use mica_term::app::keychain::redb_store::RedbKeychainCatalogStore;
use mica_term::app::keychain::repository::KeychainCatalogRepository;
use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};

fn temp_data_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join(format!("{name}-{unique}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample_catalog() -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-team".into()],
        nodes: BTreeMap::from([
            (
                "folder-team".into(),
                KeychainNode {
                    id: "folder-team".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["identity-ops".into(), "key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::SshKey,
                        ssh_key_id: Some("key-prod".into()),
                        credential_ref: None,
                        remark: "shared login".into(),
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
                        credential_ref: Some("keychain/key/key-prod".into()),
                        remark: "generated".into(),
                    }),
                },
            ),
        ]),
        merge_metadata: BTreeMap::new(),
    }
}

#[test]
fn app_root_exposes_keychain_catalog_database_path_under_data_dir() {
    let temp_root = temp_data_dir("app-paths-keychain-db");
    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: Some(temp_root.join("override-root")),
        executable_dir: temp_root.join("bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, AppRootSource::EnvOverride);
    assert_eq!(
        paths.keychain_catalog_database_path(),
        temp_root
            .join("override-root")
            .join("data")
            .join("keychain.redb")
    );
}

#[test]
fn load_returns_empty_catalog_when_keychain_store_is_missing() {
    let data_dir = temp_data_dir("keychain-store-missing");
    let store = RedbKeychainCatalogStore::new(data_dir.clone());

    let catalog = store.load().unwrap();

    assert!(catalog.root_ids.is_empty());
    assert!(catalog.nodes.is_empty());
    assert!(!store.database_path.exists());
}

#[test]
fn save_and_reload_preserves_folder_identity_key_order_and_links() {
    let data_dir = temp_data_dir("keychain-store-roundtrip");
    let store = RedbKeychainCatalogStore::new(data_dir);
    let catalog = sample_catalog();

    store.save(&catalog).unwrap();
    let reloaded = store.load().unwrap();

    assert_eq!(reloaded.root_ids, vec!["folder-team"]);
    assert_eq!(reloaded.nodes.len(), 3);
    assert_eq!(reloaded.nodes["folder-team"].child_ids.len(), 2);
    assert_eq!(
        reloaded.nodes["identity-ops"].parent_id.as_deref(),
        Some("folder-team")
    );
    match &reloaded.nodes["identity-ops"].payload {
        KeychainNodePayload::Identity(identity) => {
            assert_eq!(identity.username, "ops");
            assert_eq!(identity.ssh_key_id.as_deref(), Some("key-prod"));
        }
        other => panic!("expected identity payload, got {other:?}"),
    }
    match &reloaded.nodes["key-prod"].payload {
        KeychainNodePayload::SshKey(ssh_key) => {
            assert_eq!(ssh_key.algorithm, "ed25519");
            assert_eq!(ssh_key.comment, "prod@example");
        }
        other => panic!("expected ssh key payload, got {other:?}"),
    }
}
