use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, ASSET_RECORDS_TABLE, AssetCatalogRepository,
    METADATA_ROOT_IDS_KEY, METADATA_SCHEMA_VERSION_KEY, METADATA_TABLE, PersistedAssetCatalog,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedSshConnectionSpec,
    RedbAssetCatalogStore,
};
use redb::{Database, ReadableTable};

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
                        proxy_method: "jump-host".into(),
                        remark: String::new(),
                        credential_ref: None,
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
