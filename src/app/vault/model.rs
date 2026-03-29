use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::ssh::credentials::StoredSshSecretBundle;

const fn default_format_version() -> u32 {
    1
}

const fn default_snapshot_schema_version() -> u32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KdfConfig {
    Argon2id {
        memory_cost_kib: u32,
        time_cost: u32,
        parallelism: u32,
        salt_b64: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CipherKind {
    #[default]
    XChaCha20Poly1305,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CompressionKind {
    #[default]
    Zstd,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PackLayout {
    #[default]
    ObjectSet,
    BundledFiles,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultHead {
    pub format_version: u32,
    pub vault_id: String,
    pub vault_revision: String,
    pub parent_revision: Option<String>,
    pub device_id: String,
    pub created_at: String,
    pub payload_hash: String,
    pub manifest_ref: String,
    pub wrapped_vault_key: String,
    pub kdf: KdfConfig,
    pub cipher: CipherKind,
    pub compression: CompressionKind,
    pub pack_layout: PackLayout,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackRef {
    pub pack_id: String,
    pub object_name: String,
    pub size_bytes: u64,
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultManifest {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    #[serde(default = "default_snapshot_schema_version")]
    pub snapshot_schema_version: u32,
    pub packs: Vec<PackRef>,
    #[serde(default)]
    pub feature_flags: Vec<String>,
    #[serde(default)]
    pub provider_capability_fallbacks: BTreeMap<String, String>,
}

impl Default for VaultManifest {
    fn default() -> Self {
        Self {
            format_version: default_format_version(),
            snapshot_schema_version: default_snapshot_schema_version(),
            packs: Vec::new(),
            feature_flags: Vec::new(),
            provider_capability_fallbacks: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultAssetKind {
    Folder,
    SshConnection,
    SnippetPackage,
    Snippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VaultAssetDomain {
    Console,
    Snippets,
}

impl VaultAssetKind {
    pub fn domain(self) -> VaultAssetDomain {
        match self {
            Self::Folder | Self::SshConnection => VaultAssetDomain::Console,
            Self::SnippetPackage | Self::Snippet => VaultAssetDomain::Snippets,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VaultSocks5ProxySpec {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password_credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum VaultSshProxySpec {
    #[default]
    None,
    Socks5(VaultSocks5ProxySpec),
    Http(VaultSocks5ProxySpec),
    SshAsset {
        asset_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VaultSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    pub private_key_source: String,
    pub private_key_path: String,
    pub environment: String,
    pub proxy: VaultSshProxySpec,
    pub remark: String,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VaultSnippetSpec {
    pub script: String,
    pub package_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VaultAssetPayload {
    Folder,
    SshConnection(Box<VaultSshConnectionSpec>),
    SnippetPackage,
    Snippet(VaultSnippetSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultAssetNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub kind: VaultAssetKind,
    pub child_ids: Vec<String>,
    pub payload: VaultAssetPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VaultAssetCatalog {
    #[serde(default)]
    pub root_ids: Vec<String>,
    #[serde(default)]
    pub nodes: BTreeMap<String, VaultAssetNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultKnownHostEntry {
    pub host_pattern: String,
    pub public_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SnapshotSyncPreferences {
    #[serde(default)]
    pub auto_sync_enabled: bool,
    pub selected_primary_remote_id: Option<String>,
    #[serde(default)]
    pub selected_mirror_remote_ids: Vec<String>,
    pub last_sync_result: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct SnapshotUiPreferences {
    pub theme_mode: Option<String>,
    pub always_on_top: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultSnapshot {
    #[serde(default = "default_snapshot_schema_version")]
    pub schema_version: u32,
    pub asset_catalog: VaultAssetCatalog,
    #[serde(default)]
    pub ssh_secret_bundles: BTreeMap<String, StoredSshSecretBundle>,
    #[serde(default)]
    pub known_hosts: Vec<VaultKnownHostEntry>,
    #[serde(default)]
    pub sync_preferences: SnapshotSyncPreferences,
    #[serde(default)]
    pub ui_preferences: SnapshotUiPreferences,
}

impl Default for VaultSnapshot {
    fn default() -> Self {
        Self {
            schema_version: default_snapshot_schema_version(),
            asset_catalog: VaultAssetCatalog::default(),
            ssh_secret_bundles: BTreeMap::new(),
            known_hosts: Vec::new(),
            sync_preferences: SnapshotSyncPreferences::default(),
            ui_preferences: SnapshotUiPreferences::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteRole {
    Primary,
    Mirror,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    S3Compatible,
    GitHubGist,
    GitLabSnippet,
    GiteeGist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderAuthKind {
    AwsStandardChain,
    DeviceFlow,
    Pkce,
    Pat,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BootstrapRemoteLocator {
    S3 {
        bucket: String,
        prefix: String,
        endpoint: Option<String>,
        region: Option<String>,
        force_path_style: bool,
    },
    GitHubGist {
        gist_id: String,
    },
    GitLabSnippet {
        base_url: Option<String>,
        project_id: Option<String>,
        snippet_id: String,
    },
    GiteeGist {
        gist_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteHealthStatus {
    Healthy,
    Degraded,
    Unreachable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteHealth {
    pub status: RemoteHealthStatus,
    pub checked_at: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapRemoteConfig {
    pub remote_id: String,
    pub role: RemoteRole,
    pub provider: ProviderKind,
    pub locator: BootstrapRemoteLocator,
    pub credential_ref: Option<String>,
    pub auth_kind: ProviderAuthKind,
    pub last_health: Option<RemoteHealth>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BootstrapBundle {
    #[serde(default = "default_format_version")]
    pub format_version: u32,
    pub vault_id: String,
    #[serde(default)]
    pub remotes: Vec<BootstrapRemoteConfig>,
    #[serde(default)]
    pub auto_sync_enabled: bool,
    #[serde(default)]
    pub bootstrap_cipher: CipherKind,
    pub bootstrap_kdf: Option<KdfConfig>,
}

impl Default for BootstrapBundle {
    fn default() -> Self {
        Self {
            format_version: default_format_version(),
            vault_id: String::new(),
            remotes: Vec::new(),
            auto_sync_enabled: false,
            bootstrap_cipher: CipherKind::default(),
            bootstrap_kdf: None,
        }
    }
}
