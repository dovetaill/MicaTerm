//! `redb`-backed keychain catalog repository.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use redb::{Database, ReadableTable, TableDefinition};

use crate::app::keychain::model::{KeychainCatalog, KeychainNode};
use crate::app::keychain::repository::KeychainCatalogRepository;

pub const KEYCHAIN_CATALOG_SCHEMA_VERSION: u32 = 1;

pub const KEYCHAIN_METADATA_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("keychain_catalog_metadata");
pub const KEYCHAIN_RECORDS_TABLE: TableDefinition<&str, &[u8]> =
    TableDefinition::new("keychain_catalog_records");

pub const KEYCHAIN_METADATA_SCHEMA_VERSION_KEY: &str = "schema_version";
pub const KEYCHAIN_METADATA_ROOT_IDS_KEY: &str = "root_ids";

pub struct RedbKeychainCatalogStore {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl RedbKeychainCatalogStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self {
            database_path: data_dir.join("keychain.redb"),
            data_dir,
        }
    }

    fn read_catalog(&self, database: &Database) -> Result<KeychainCatalog> {
        let read_txn = database.begin_read()?;
        let metadata = read_txn.open_table(KEYCHAIN_METADATA_TABLE)?;
        let records = read_txn.open_table(KEYCHAIN_RECORDS_TABLE)?;

        let _schema_version = metadata
            .get(KEYCHAIN_METADATA_SCHEMA_VERSION_KEY)?
            .map(|value| decode_schema_version(value.value()))
            .transpose()?
            .unwrap_or(KEYCHAIN_CATALOG_SCHEMA_VERSION);
        let root_ids = metadata
            .get(KEYCHAIN_METADATA_ROOT_IDS_KEY)?
            .map(|value| decode_root_ids(value.value()))
            .transpose()?
            .unwrap_or_default();

        let mut nodes = BTreeMap::new();
        for entry in records.iter()? {
            let (key, value) = entry?;
            let node_id = key.value().to_string();
            let node: KeychainNode = bincode::deserialize(value.value())
                .with_context(|| format!("failed to decode keychain node `{node_id}`"))?;
            nodes.insert(node_id, node);
        }

        Ok(KeychainCatalog {
            root_ids,
            nodes,
            merge_metadata: BTreeMap::new(),
        })
    }
}

impl KeychainCatalogRepository for RedbKeychainCatalogStore {
    fn load(&self) -> Result<KeychainCatalog> {
        if !self.database_path.exists() {
            return Ok(KeychainCatalog::default());
        }

        let database = Database::create(&self.database_path)
            .context("failed to open keychain catalog database")?;
        self.read_catalog(&database)
    }

    fn save(&self, catalog: &KeychainCatalog) -> Result<()> {
        fs::create_dir_all(&self.data_dir)?;
        let database = Database::create(&self.database_path)
            .context("failed to create keychain catalog database")?;
        let write_txn = database.begin_write()?;

        {
            let mut metadata = write_txn.open_table(KEYCHAIN_METADATA_TABLE)?;
            let schema_version_bytes = encode_schema_version(KEYCHAIN_CATALOG_SCHEMA_VERSION);
            metadata.insert(
                KEYCHAIN_METADATA_SCHEMA_VERSION_KEY,
                schema_version_bytes.as_slice(),
            )?;
            let root_ids_bytes = encode_root_ids(&catalog.root_ids)?;
            metadata.insert(KEYCHAIN_METADATA_ROOT_IDS_KEY, root_ids_bytes.as_slice())?;
        }

        {
            let mut records = write_txn.open_table(KEYCHAIN_RECORDS_TABLE)?;
            let existing_keys = records
                .iter()?
                .map(|entry| entry.map(|(key, _)| key.value().to_string()))
                .collect::<std::result::Result<Vec<_>, _>>()?;

            for key in existing_keys {
                records.remove(key.as_str())?;
            }

            for (node_id, node) in &catalog.nodes {
                let encoded = bincode::serialize(node)
                    .with_context(|| format!("failed to encode keychain node `{node_id}`"))?;
                records.insert(node_id.as_str(), encoded.as_slice())?;
            }
        }

        write_txn.commit()?;
        Ok(())
    }
}

fn encode_schema_version(schema_version: u32) -> [u8; 4] {
    schema_version.to_le_bytes()
}

fn decode_schema_version(bytes: &[u8]) -> Result<u32> {
    let raw: [u8; 4] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("invalid keychain schema version payload"))?;
    Ok(u32::from_le_bytes(raw))
}

fn encode_root_ids(root_ids: &[String]) -> Result<Vec<u8>> {
    Ok(bincode::serialize(root_ids)?)
}

fn decode_root_ids(bytes: &[u8]) -> Result<Vec<String>> {
    Ok(bincode::deserialize(bytes)?)
}
