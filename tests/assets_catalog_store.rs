use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, ASSET_RECORDS_TABLE, AssetCatalogRepository,
    METADATA_ROOT_IDS_KEY, METADATA_SCHEMA_VERSION_KEY, METADATA_TABLE, PersistedAssetCatalog,
    PersistedAssetDomain, PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload,
    PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec, PersistedSnippetSpec,
    PersistedSshConnectionSpec, RedbAssetCatalogStore,
};
use redb::{Database, ReadableTable};
use serde::Serialize;

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

fn sample_catalog() -> PersistedAssetCatalog {
    let folder_id = "folder-1".to_string();
    let ssh_id = "ssh-1".to_string();

    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec![folder_id.clone()],
        nodes: BTreeMap::from([
            (
                folder_id.clone(),
                PersistedAssetNode {
                    id: folder_id,
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: vec![ssh_id.clone()],
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                ssh_id.clone(),
                PersistedAssetNode {
                    id: ssh_id,
                    parent_id: Some("folder-1".into()),
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "2022".into(),
                        auth_method: "password".into(),
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                            host: "proxy.example.net".into(),
                            port: "1080".into(),
                            username: "ops-proxy".into(),
                            password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
                        }),
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
        ]),
    }
}

fn sample_catalog_with_ssh_upstream_proxy() -> PersistedAssetCatalog {
    let mut catalog = sample_catalog();
    let ssh_node = catalog.nodes.get_mut("ssh-1").expect("ssh asset");
    ssh_node.payload = PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
        host: "gateway.example.com".into(),
        user: "ops".into(),
        port: "2022".into(),
        auth_method: "password".into(),
        private_key_source: "content".into(),
        private_key_path: String::new(),
        environment: "prod".into(),
        proxy: PersistedAssetSshProxySpec::SshAsset {
            asset_id: "asset-upstream".into(),
        },
        remark: String::new(),
        credential_ref: None,
    });
    catalog
}

fn sample_catalog_with_snippets() -> PersistedAssetCatalog {
    let folder_id = "folder-1".to_string();
    let ssh_id = "ssh-1".to_string();
    let package_id = "snippet-package-1".to_string();
    let package_snippet_id = "snippet-1".to_string();
    let root_snippet_id = "snippet-2".to_string();

    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec![
            folder_id.clone(),
            package_id.clone(),
            root_snippet_id.clone(),
        ],
        nodes: BTreeMap::from([
            (
                folder_id.clone(),
                PersistedAssetNode {
                    id: folder_id,
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: vec![ssh_id.clone()],
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                ssh_id.clone(),
                PersistedAssetNode {
                    id: ssh_id,
                    parent_id: Some("folder-1".into()),
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "22".into(),
                        auth_method: "password".into(),
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
            (
                package_id.clone(),
                PersistedAssetNode {
                    id: package_id.clone(),
                    parent_id: None,
                    title: "Deploy".into(),
                    kind: PersistedAssetKind::SnippetPackage,
                    child_ids: vec![package_snippet_id.clone()],
                    payload: PersistedAssetPayload::SnippetPackage,
                },
            ),
            (
                package_snippet_id.clone(),
                PersistedAssetNode {
                    id: package_snippet_id,
                    parent_id: Some(package_id.clone()),
                    title: "Deploy prod".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl apply -f prod.yaml".into(),
                        package_id: Some(package_id),
                    }),
                },
            ),
            (
                root_snippet_id.clone(),
                PersistedAssetNode {
                    id: root_snippet_id,
                    parent_id: None,
                    title: "Restart API".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl rollout restart deploy/api".into(),
                        package_id: None,
                    }),
                },
            ),
        ]),
    }
}

fn matching_files(data_dir: &PathBuf, prefix: &str) -> Vec<String> {
    let mut matches = fs::read_dir(data_dir)
        .unwrap()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(prefix))
        .collect::<Vec<_>>();
    matches.sort();
    matches
}

#[derive(Debug, Serialize)]
struct LegacyStoredPersistedAssetNode {
    id: String,
    parent_id: Option<String>,
    title: String,
    kind: LegacyStoredPersistedAssetKind,
    child_ids: Vec<String>,
    payload: LegacyStoredPersistedAssetPayload,
}

#[allow(dead_code)]
#[derive(Debug, Serialize)]
enum LegacyStoredPersistedAssetKind {
    Folder,
    SshConnection,
}

#[allow(clippy::large_enum_variant)]
#[allow(dead_code)]
#[derive(Debug, Serialize)]
enum LegacyStoredPersistedAssetPayload {
    Folder,
    SshConnection(LegacyStoredPersistedSshConnectionSpec),
}

#[derive(Debug, Serialize)]
struct LegacyStoredPersistedSshConnectionSpec {
    host: String,
    user: String,
    port: String,
    #[serde(default)]
    auth_method: String,
    #[serde(default)]
    private_key_source: String,
    #[serde(default)]
    private_key_path: String,
    environment: String,
    proxy_method: String,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    credential_ref: Option<String>,
}

#[test]
fn load_returns_empty_catalog_when_assets_file_is_missing() {
    let data_dir = temp_data_dir("assets-store-missing");
    let store = RedbAssetCatalogStore::new(data_dir.clone());

    let catalog = store.load().unwrap();

    assert_eq!(catalog.schema_version, ASSET_CATALOG_SCHEMA_VERSION);
    assert!(catalog.root_ids.is_empty());
    assert!(catalog.nodes.is_empty());
    assert!(!store.database_path.exists());
}

#[test]
fn save_and_reload_preserves_tree_structure_and_ssh_fields() {
    let data_dir = temp_data_dir("assets-store-roundtrip");
    let store = RedbAssetCatalogStore::new(data_dir);
    let catalog = sample_catalog();

    store.save(&catalog).unwrap();
    let loaded = store.load().unwrap();

    assert_eq!(loaded, catalog);
}

#[test]
fn save_and_reload_preserves_ssh_upstream_asset_reference() {
    let data_dir = temp_data_dir("assets-store-ssh-upstream-roundtrip");
    let store = RedbAssetCatalogStore::new(data_dir);
    let catalog = sample_catalog_with_ssh_upstream_proxy();

    store.save(&catalog).unwrap();
    let loaded = store.load().unwrap();

    assert_eq!(loaded, catalog);
}

#[test]
fn save_and_reload_preserves_snippets_package_and_root_snippet() {
    let data_dir = temp_data_dir("assets-store-snippets-roundtrip");
    let store = RedbAssetCatalogStore::new(data_dir);
    let catalog = sample_catalog_with_snippets();

    store.save(&catalog).unwrap();
    let loaded = store.load().unwrap();

    assert_eq!(loaded, catalog);
    assert_eq!(
        loaded
            .nodes
            .get("snippet-package-1")
            .expect("snippet package")
            .kind
            .domain(),
        PersistedAssetDomain::Snippets
    );
    assert_eq!(
        loaded.nodes.get("snippet-2").expect("root snippet").payload,
        PersistedAssetPayload::Snippet(PersistedSnippetSpec {
            script: "kubectl rollout restart deploy/api".into(),
            package_id: None,
        })
    );
}

#[test]
fn open_failure_quarantines_corrupt_file_with_timestamp_suffix() {
    let data_dir = temp_data_dir("assets-store-corrupt");
    let store = RedbAssetCatalogStore::new(data_dir.clone());
    fs::write(&store.database_path, b"not-a-redb-file").unwrap();

    let catalog = store.load().unwrap();

    assert_eq!(catalog.schema_version, ASSET_CATALOG_SCHEMA_VERSION);
    assert!(catalog.root_ids.is_empty());
    assert!(catalog.nodes.is_empty());
    assert!(!store.database_path.exists());
    let corrupt_files = matching_files(&data_dir, "assets.corrupt-");
    assert_eq!(corrupt_files.len(), 1);
}

#[test]
fn load_migrates_legacy_proxy_method_field_to_no_proxy() {
    let data_dir = temp_data_dir("assets-store-legacy-proxy-method");
    let store = RedbAssetCatalogStore::new(data_dir.clone());
    fs::create_dir_all(&data_dir).unwrap();

    let database = Database::create(&store.database_path).unwrap();
    let write_txn = database.begin_write().unwrap();
    {
        let mut metadata = write_txn.open_table(METADATA_TABLE).unwrap();
        metadata
            .insert(
                METADATA_SCHEMA_VERSION_KEY,
                &(ASSET_CATALOG_SCHEMA_VERSION - 1).to_le_bytes()[..],
            )
            .unwrap();
        let root_ids = bincode::serialize(&vec!["ssh-legacy".to_string()]).unwrap();
        metadata
            .insert(METADATA_ROOT_IDS_KEY, root_ids.as_slice())
            .unwrap();
    }
    {
        let mut asset_records = write_txn.open_table(ASSET_RECORDS_TABLE).unwrap();
        let legacy_node = LegacyStoredPersistedAssetNode {
            id: "ssh-legacy".into(),
            parent_id: None,
            title: "Legacy Gateway".into(),
            kind: LegacyStoredPersistedAssetKind::SshConnection,
            child_ids: Vec::new(),
            payload: LegacyStoredPersistedAssetPayload::SshConnection(
                LegacyStoredPersistedSshConnectionSpec {
                    host: "legacy.example.com".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "legacy".into(),
                    proxy_method: "jump-host".into(),
                    remark: String::new(),
                    credential_ref: None,
                },
            ),
        };
        let encoded = bincode::serialize(&legacy_node).unwrap();
        asset_records
            .insert("ssh-legacy", encoded.as_slice())
            .unwrap();
    }
    write_txn.commit().unwrap();
    drop(database);

    let loaded = store.load().unwrap();
    let ssh_node = loaded.nodes.get("ssh-legacy").expect("legacy ssh node");

    assert_eq!(loaded.schema_version, ASSET_CATALOG_SCHEMA_VERSION);
    assert_eq!(
        ssh_node.payload,
        PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
            host: "legacy.example.com".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "legacy".into(),
            proxy: PersistedAssetSshProxySpec::None,
            remark: String::new(),
            credential_ref: None,
        })
    );
}

#[test]
fn schema_upgrade_creates_backup_before_rewrite() {
    let data_dir = temp_data_dir("assets-store-upgrade");
    let store = RedbAssetCatalogStore::new(data_dir.clone());
    let catalog = sample_catalog();
    store.save(&catalog).unwrap();

    {
        let database = Database::create(&store.database_path).unwrap();
        let write_txn = database.begin_write().unwrap();
        {
            let mut metadata = write_txn.open_table(METADATA_TABLE).unwrap();
            metadata
                .insert(METADATA_SCHEMA_VERSION_KEY, &[0_u8, 0_u8, 0_u8, 0_u8][..])
                .unwrap();
            assert!(metadata.get(METADATA_ROOT_IDS_KEY).unwrap().is_some());
        }
        {
            let asset_records = write_txn.open_table(ASSET_RECORDS_TABLE).unwrap();
            assert!(asset_records.get("folder-1").unwrap().is_some());
        }
        write_txn.commit().unwrap();
    }

    let loaded = store.load().unwrap();

    assert_eq!(loaded, catalog);
    let backup_files = matching_files(&data_dir, "assets.backup-");
    assert_eq!(backup_files.len(), 1);

    let reloaded = store.load().unwrap();
    assert_eq!(reloaded.schema_version, ASSET_CATALOG_SCHEMA_VERSION);
    assert_eq!(reloaded, catalog);
}
