//! `redb`-backed asset catalog repository with basic recovery and upgrade support.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use redb::{Database, ReadableTable, TableDefinition};
use serde::{Deserialize, Serialize};

use crate::app::assets_catalog::model::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetCatalog, PersistedAssetKind, PersistedAssetNode,
    PersistedAssetPayload, PersistedSshConnectionSpec,
};
use crate::app::assets_catalog::repository::AssetCatalogRepository;

pub const METADATA_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("asset_catalog_metadata");
pub const ASSET_RECORDS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("asset_catalog_records");

pub const METADATA_SCHEMA_VERSION_KEY: &str = "schema_version";
pub const METADATA_ROOT_IDS_KEY: &str = "root_ids";

pub struct RedbAssetCatalogStore {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl RedbAssetCatalogStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            database_path: data_dir.join("assets.redb"),
            data_dir,
        }
    }

    fn empty_catalog() -> PersistedAssetCatalog {
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        }
    }

    fn open_existing_database(&self) -> Result<Database> {
        Database::create(&self.database_path).context("failed to open asset catalog database")
    }

    fn read_catalog(&self, database: &Database) -> Result<PersistedAssetCatalog> {
        let read_txn = database.begin_read()?;
        let metadata = read_txn.open_table(METADATA_TABLE)?;
        let asset_records = read_txn.open_table(ASSET_RECORDS_TABLE)?;

        let schema_version = metadata
            .get(METADATA_SCHEMA_VERSION_KEY)?
            .map(|value| decode_schema_version(value.value()))
            .transpose()?
            .unwrap_or(ASSET_CATALOG_SCHEMA_VERSION);

        let root_ids = metadata
            .get(METADATA_ROOT_IDS_KEY)?
            .map(|value| decode_root_ids(value.value()))
            .transpose()?
            .unwrap_or_default();

        let mut nodes = BTreeMap::new();
        for entry in asset_records.iter()? {
            let (key, value) = entry?;
            let record_key = key.value().to_string();
            let node = decode_node(value.value())?;
            if node.id != record_key {
                bail!("asset record key does not match encoded node id");
            }
            nodes.insert(record_key, node);
        }

        Ok(PersistedAssetCatalog {
            schema_version,
            root_ids,
            nodes,
        })
    }

    fn backup_existing_database(&self) -> Result<()> {
        if !self.database_path.exists() {
            return Ok(());
        }

        let backup_path = self
            .data_dir
            .join(format!("assets.backup-{}.redb", timestamp_suffix()));
        fs::copy(&self.database_path, &backup_path)
            .with_context(|| format!("failed to create backup at {}", backup_path.display()))?;
        Ok(())
    }

    fn quarantine_existing_database(&self) -> Result<()> {
        if !self.database_path.exists() {
            return Ok(());
        }

        let quarantine_path = self
            .data_dir
            .join(format!("assets.corrupt-{}.redb", timestamp_suffix()));
        fs::rename(&self.database_path, &quarantine_path).with_context(|| {
            format!(
                "failed to quarantine corrupt asset catalog at {}",
                quarantine_path.display()
            )
        })?;
        Ok(())
    }
}

impl AssetCatalogRepository for RedbAssetCatalogStore {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        if !self.database_path.exists() {
            return Ok(Self::empty_catalog());
        }

        let database = match self.open_existing_database() {
            Ok(database) => database,
            Err(_) => {
                self.quarantine_existing_database()?;
                return Ok(Self::empty_catalog());
            }
        };

        let catalog_result = self.read_catalog(&database);
        drop(database);

        let catalog = match catalog_result {
            Ok(catalog) => catalog,
            Err(_) => {
                self.quarantine_existing_database()?;
                return Ok(Self::empty_catalog());
            }
        };

        if catalog.schema_version > ASSET_CATALOG_SCHEMA_VERSION {
            bail!(
                "asset catalog schema {} is newer than supported schema {}",
                catalog.schema_version,
                ASSET_CATALOG_SCHEMA_VERSION
            );
        }

        if catalog.schema_version < ASSET_CATALOG_SCHEMA_VERSION {
            self.backup_existing_database()?;
            let upgraded = PersistedAssetCatalog {
                schema_version: ASSET_CATALOG_SCHEMA_VERSION,
                root_ids: catalog.root_ids.clone(),
                nodes: catalog.nodes.clone(),
            };
            self.save(&upgraded)?;
            return Ok(upgraded);
        }

        Ok(catalog)
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        let database = Database::create(&self.database_path)
            .context("failed to create asset catalog database")?;
        let write_txn = database.begin_write()?;

        {
            let mut metadata = write_txn.open_table(METADATA_TABLE)?;
            let schema_version_bytes = encode_schema_version(catalog.schema_version);
            metadata.insert(METADATA_SCHEMA_VERSION_KEY, schema_version_bytes.as_slice())?;
            let root_ids_bytes = encode_root_ids(&catalog.root_ids)?;
            metadata.insert(METADATA_ROOT_IDS_KEY, root_ids_bytes.as_slice())?;
        }

        {
            let mut asset_records = write_txn.open_table(ASSET_RECORDS_TABLE)?;
            let existing_keys = asset_records
                .iter()?
                .map(|entry| entry.map(|(key, _)| key.value().to_string()))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for key in existing_keys {
                asset_records.remove(key.as_str())?;
            }

            for (asset_id, node) in &catalog.nodes {
                let node_bytes = encode_node(node)?;
                asset_records.insert(asset_id.as_str(), node_bytes.as_slice())?;
            }
        }

        write_txn.commit()?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPersistedAssetNode {
    id: String,
    parent_id: Option<String>,
    title: String,
    kind: StoredPersistedAssetKind,
    child_ids: Vec<String>,
    payload: StoredPersistedAssetPayload,
}

#[derive(Debug, Serialize, Deserialize)]
enum StoredPersistedAssetKind {
    Folder,
    SshConnection,
}

#[derive(Debug, Serialize, Deserialize)]
enum StoredPersistedAssetPayload {
    Folder,
    SshConnection(StoredPersistedSshConnectionSpec),
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPersistedSshConnectionSpec {
    host: String,
    user: String,
    port: String,
    environment: String,
    proxy_method: String,
}

fn encode_schema_version(schema_version: u32) -> [u8; 4] {
    schema_version.to_le_bytes()
}

fn decode_schema_version(bytes: &[u8]) -> Result<u32> {
    let raw: [u8; 4] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid schema version payload"))?;
    Ok(u32::from_le_bytes(raw))
}

fn encode_root_ids(root_ids: &[String]) -> Result<Vec<u8>> {
    Ok(bincode::serialize(root_ids)?)
}

fn decode_root_ids(bytes: &[u8]) -> Result<Vec<String>> {
    Ok(bincode::deserialize(bytes)?)
}

fn encode_node(node: &PersistedAssetNode) -> Result<Vec<u8>> {
    Ok(bincode::serialize(&StoredPersistedAssetNode::from(node))?)
}

fn decode_node(bytes: &[u8]) -> Result<PersistedAssetNode> {
    let stored: StoredPersistedAssetNode = bincode::deserialize(bytes)?;
    Ok(stored.into())
}

fn timestamp_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

impl From<&PersistedAssetNode> for StoredPersistedAssetNode {
    fn from(node: &PersistedAssetNode) -> Self {
        Self {
            id: node.id.clone(),
            parent_id: node.parent_id.clone(),
            title: node.title.clone(),
            kind: match node.kind {
                PersistedAssetKind::Folder => StoredPersistedAssetKind::Folder,
                PersistedAssetKind::SshConnection => StoredPersistedAssetKind::SshConnection,
            },
            child_ids: node.child_ids.clone(),
            payload: match &node.payload {
                PersistedAssetPayload::Folder => StoredPersistedAssetPayload::Folder,
                PersistedAssetPayload::SshConnection(spec) => {
                    StoredPersistedAssetPayload::SshConnection(StoredPersistedSshConnectionSpec {
                        host: spec.host.clone(),
                        user: spec.user.clone(),
                        port: spec.port.clone(),
                        environment: spec.environment.clone(),
                        proxy_method: spec.proxy_method.clone(),
                    })
                }
            },
        }
    }
}

impl From<StoredPersistedAssetNode> for PersistedAssetNode {
    fn from(node: StoredPersistedAssetNode) -> Self {
        Self {
            id: node.id,
            parent_id: node.parent_id,
            title: node.title,
            kind: match node.kind {
                StoredPersistedAssetKind::Folder => PersistedAssetKind::Folder,
                StoredPersistedAssetKind::SshConnection => PersistedAssetKind::SshConnection,
            },
            child_ids: node.child_ids,
            payload: match node.payload {
                StoredPersistedAssetPayload::Folder => PersistedAssetPayload::Folder,
                StoredPersistedAssetPayload::SshConnection(spec) => {
                    PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: spec.host,
                        user: spec.user,
                        port: spec.port,
                        environment: spec.environment,
                        proxy_method: spec.proxy_method,
                    })
                }
            },
        }
    }
}
