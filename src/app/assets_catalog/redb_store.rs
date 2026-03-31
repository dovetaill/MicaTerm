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
    PersistedAssetPayload, PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec,
    PersistedSnippetSpec, PersistedSshConnectionSpec,
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
    SnippetPackage,
    Snippet,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
enum StoredPersistedAssetPayload {
    Folder,
    SshConnection(StoredPersistedSshConnectionSpec),
    SnippetPackage,
    Snippet(StoredPersistedSnippetSpec),
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredPersistedAssetSocks5ProxySpec {
    host: String,
    port: String,
    username: String,
    password_credential_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
enum StoredPersistedAssetSshProxySpec {
    #[default]
    None,
    Socks5(StoredPersistedAssetSocks5ProxySpec),
    Http(StoredPersistedAssetSocks5ProxySpec),
    SshAsset {
        asset_id: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredPersistedSshConnectionSpec {
    host: String,
    user: String,
    port: String,
    #[serde(default)]
    auth_method: String,
    #[serde(default = "default_stored_ssh_auth_source")]
    auth_source: String,
    #[serde(default)]
    keychain_identity_id: Option<String>,
    #[serde(default)]
    private_key_source: String,
    #[serde(default)]
    private_key_path: String,
    environment: String,
    proxy: StoredPersistedAssetSshProxySpec,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    credential_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct StoredPersistedSnippetSpec {
    script: String,
    package_id: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyStoredPersistedAssetNode {
    id: String,
    parent_id: Option<String>,
    title: String,
    kind: LegacyStoredPersistedAssetKind,
    child_ids: Vec<String>,
    payload: LegacyStoredPersistedAssetPayload,
}

#[derive(Debug, Serialize, Deserialize)]
enum LegacyStoredPersistedAssetKind {
    Folder,
    SshConnection,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
enum LegacyStoredPersistedAssetPayload {
    Folder,
    SshConnection(LegacyStoredPersistedSshConnectionSpec),
}

#[derive(Debug, Serialize, Deserialize)]
struct CompatStoredPersistedAssetNodeV4 {
    id: String,
    parent_id: Option<String>,
    title: String,
    kind: CompatStoredPersistedAssetKindV4,
    child_ids: Vec<String>,
    payload: CompatStoredPersistedAssetPayloadV4,
}

#[derive(Debug, Serialize, Deserialize)]
enum CompatStoredPersistedAssetKindV4 {
    Folder,
    SshConnection,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Serialize, Deserialize)]
enum CompatStoredPersistedAssetPayloadV4 {
    Folder,
    SshConnection(CompatStoredPersistedSshConnectionSpecV4),
}

#[derive(Debug, Serialize, Deserialize)]
struct CompatStoredPersistedSshConnectionSpecV4 {
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
    proxy: StoredPersistedAssetSshProxySpec,
    #[serde(default)]
    remark: String,
    #[serde(default)]
    credential_ref: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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
    if let Ok(stored) = bincode::deserialize::<StoredPersistedAssetNode>(bytes) {
        return Ok(stored.into());
    }

    if let Ok(compat) = bincode::deserialize::<CompatStoredPersistedAssetNodeV4>(bytes) {
        return Ok(compat.into());
    }

    let legacy: LegacyStoredPersistedAssetNode = bincode::deserialize(bytes)?;
    Ok(legacy.into())
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
                PersistedAssetKind::SnippetPackage => StoredPersistedAssetKind::SnippetPackage,
                PersistedAssetKind::Snippet => StoredPersistedAssetKind::Snippet,
            },
            child_ids: node.child_ids.clone(),
            payload: match &node.payload {
                PersistedAssetPayload::Folder => StoredPersistedAssetPayload::Folder,
                PersistedAssetPayload::SshConnection(spec) => {
                    StoredPersistedAssetPayload::SshConnection(StoredPersistedSshConnectionSpec {
                        host: spec.host.clone(),
                        user: spec.user.clone(),
                        port: spec.port.clone(),
                        auth_method: spec.auth_method.clone(),
                        auth_source: spec.auth_source.clone(),
                        keychain_identity_id: spec.keychain_identity_id.clone(),
                        private_key_source: spec.private_key_source.clone(),
                        private_key_path: spec.private_key_path.clone(),
                        environment: spec.environment.clone(),
                        proxy: stored_proxy(&spec.proxy),
                        remark: spec.remark.clone(),
                        credential_ref: spec.credential_ref.clone(),
                    })
                }
                PersistedAssetPayload::SnippetPackage => {
                    StoredPersistedAssetPayload::SnippetPackage
                }
                PersistedAssetPayload::Snippet(spec) => {
                    StoredPersistedAssetPayload::Snippet(StoredPersistedSnippetSpec {
                        script: spec.script.clone(),
                        package_id: spec.package_id.clone(),
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
                StoredPersistedAssetKind::SnippetPackage => PersistedAssetKind::SnippetPackage,
                StoredPersistedAssetKind::Snippet => PersistedAssetKind::Snippet,
            },
            child_ids: node.child_ids,
            payload: match node.payload {
                StoredPersistedAssetPayload::Folder => PersistedAssetPayload::Folder,
                StoredPersistedAssetPayload::SshConnection(spec) => {
                    PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: spec.host,
                        user: spec.user,
                        port: spec.port,
                        auth_method: default_ssh_auth_method(spec.auth_method),
                        auth_source: default_ssh_auth_source(spec.auth_source),
                        keychain_identity_id: spec.keychain_identity_id,
                        private_key_source: default_private_key_source(spec.private_key_source),
                        private_key_path: spec.private_key_path,
                        environment: spec.environment,
                        proxy: persisted_proxy(spec.proxy),
                        remark: spec.remark,
                        credential_ref: spec.credential_ref,
                    })
                }
                StoredPersistedAssetPayload::SnippetPackage => {
                    PersistedAssetPayload::SnippetPackage
                }
                StoredPersistedAssetPayload::Snippet(spec) => {
                    PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: spec.script,
                        package_id: spec.package_id,
                    })
                }
            },
        }
    }
}

impl From<CompatStoredPersistedAssetNodeV4> for PersistedAssetNode {
    fn from(node: CompatStoredPersistedAssetNodeV4) -> Self {
        Self {
            id: node.id,
            parent_id: node.parent_id,
            title: node.title,
            kind: match node.kind {
                CompatStoredPersistedAssetKindV4::Folder => PersistedAssetKind::Folder,
                CompatStoredPersistedAssetKindV4::SshConnection => {
                    PersistedAssetKind::SshConnection
                }
            },
            child_ids: node.child_ids,
            payload: match node.payload {
                CompatStoredPersistedAssetPayloadV4::Folder => PersistedAssetPayload::Folder,
                CompatStoredPersistedAssetPayloadV4::SshConnection(spec) => {
                    PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: spec.host,
                        user: spec.user,
                        port: spec.port,
                        auth_method: default_ssh_auth_method(spec.auth_method),
                        auth_source: default_ssh_auth_source(String::new()),
                        keychain_identity_id: None,
                        private_key_source: default_private_key_source(spec.private_key_source),
                        private_key_path: spec.private_key_path,
                        environment: spec.environment,
                        proxy: persisted_proxy(spec.proxy),
                        remark: spec.remark,
                        credential_ref: spec.credential_ref,
                    })
                }
            },
        }
    }
}

impl From<LegacyStoredPersistedAssetNode> for PersistedAssetNode {
    fn from(node: LegacyStoredPersistedAssetNode) -> Self {
        Self {
            id: node.id,
            parent_id: node.parent_id,
            title: node.title,
            kind: match node.kind {
                LegacyStoredPersistedAssetKind::Folder => PersistedAssetKind::Folder,
                LegacyStoredPersistedAssetKind::SshConnection => PersistedAssetKind::SshConnection,
            },
            child_ids: node.child_ids,
            payload: match node.payload {
                LegacyStoredPersistedAssetPayload::Folder => PersistedAssetPayload::Folder,
                LegacyStoredPersistedAssetPayload::SshConnection(spec) => {
                    let _ = spec.proxy_method;
                    PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: spec.host,
                        user: spec.user,
                        port: spec.port,
                        auth_method: default_ssh_auth_method(spec.auth_method),
                        auth_source: default_ssh_auth_source(String::new()),
                        keychain_identity_id: None,
                        private_key_source: default_private_key_source(spec.private_key_source),
                        private_key_path: spec.private_key_path,
                        environment: spec.environment,
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: spec.remark,
                        credential_ref: spec.credential_ref,
                    })
                }
            },
        }
    }
}

fn stored_proxy(proxy: &PersistedAssetSshProxySpec) -> StoredPersistedAssetSshProxySpec {
    match proxy {
        PersistedAssetSshProxySpec::None => StoredPersistedAssetSshProxySpec::None,
        PersistedAssetSshProxySpec::Socks5(spec) => {
            StoredPersistedAssetSshProxySpec::Socks5(StoredPersistedAssetSocks5ProxySpec {
                host: spec.host.clone(),
                port: spec.port.clone(),
                username: spec.username.clone(),
                password_credential_ref: spec.password_credential_ref.clone(),
            })
        }
        PersistedAssetSshProxySpec::Http(spec) => {
            StoredPersistedAssetSshProxySpec::Http(StoredPersistedAssetSocks5ProxySpec {
                host: spec.host.clone(),
                port: spec.port.clone(),
                username: spec.username.clone(),
                password_credential_ref: spec.password_credential_ref.clone(),
            })
        }
        PersistedAssetSshProxySpec::SshAsset { asset_id } => {
            StoredPersistedAssetSshProxySpec::SshAsset {
                asset_id: asset_id.clone(),
            }
        }
    }
}

fn persisted_proxy(proxy: StoredPersistedAssetSshProxySpec) -> PersistedAssetSshProxySpec {
    match proxy {
        StoredPersistedAssetSshProxySpec::None => PersistedAssetSshProxySpec::None,
        StoredPersistedAssetSshProxySpec::Socks5(spec) => {
            PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                host: spec.host,
                port: spec.port,
                username: spec.username,
                password_credential_ref: spec.password_credential_ref,
            })
        }
        StoredPersistedAssetSshProxySpec::Http(spec) => {
            PersistedAssetSshProxySpec::Http(PersistedAssetSocks5ProxySpec {
                host: spec.host,
                port: spec.port,
                username: spec.username,
                password_credential_ref: spec.password_credential_ref,
            })
        }
        StoredPersistedAssetSshProxySpec::SshAsset { asset_id } => {
            PersistedAssetSshProxySpec::SshAsset { asset_id }
        }
    }
}

fn default_ssh_auth_method(value: String) -> String {
    if value.trim().is_empty() {
        "password".into()
    } else {
        value
    }
}

fn default_private_key_source(value: String) -> String {
    if value.trim().is_empty() {
        "content".into()
    } else {
        value
    }
}

fn default_stored_ssh_auth_source() -> String {
    "manual".into()
}

fn default_ssh_auth_source(value: String) -> String {
    if value.trim().is_empty() {
        "manual".into()
    } else {
        value
    }
}
