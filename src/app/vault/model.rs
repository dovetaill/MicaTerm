use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::app::keychain::model::KeychainCatalog;
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
    #[serde(alias = "created_at")]
    pub committed_at: String,
    #[serde(default)]
    pub committed_by_device: String,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    #[serde(default = "default_vault_ssh_auth_source")]
    pub auth_source: String,
    #[serde(default)]
    pub keychain_identity_id: Option<String>,
    pub private_key_source: String,
    pub private_key_path: String,
    pub environment: String,
    pub proxy: VaultSshProxySpec,
    pub remark: String,
    pub credential_ref: Option<String>,
}

impl Default for VaultSshConnectionSpec {
    fn default() -> Self {
        Self {
            host: String::new(),
            user: String::new(),
            port: String::new(),
            auth_method: String::new(),
            auth_source: default_vault_ssh_auth_source(),
            keychain_identity_id: None,
            private_key_source: String::new(),
            private_key_path: String::new(),
            environment: String::new(),
            proxy: VaultSshProxySpec::None,
            remark: String::new(),
            credential_ref: None,
        }
    }
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
#[serde(default)]
pub struct VaultNodeMergeMetadata {
    pub last_modified_at: Option<String>,
    pub last_modified_by_device: Option<String>,
    pub deleted_at: Option<String>,
}

impl VaultNodeMergeMetadata {
    pub fn is_deleted(&self) -> bool {
        self.deleted_at
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct VaultAssetCatalog {
    #[serde(default)]
    pub root_ids: Vec<String>,
    #[serde(default)]
    pub nodes: BTreeMap<String, VaultAssetNode>,
    #[serde(default)]
    pub merge_metadata: BTreeMap<String, VaultNodeMergeMetadata>,
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
    pub keychain_catalog: KeychainCatalog,
    #[serde(default)]
    pub keychain_identity_secret_bundles: BTreeMap<String, StoredSshSecretBundle>,
    #[serde(default)]
    pub keychain_key_secret_bundles: BTreeMap<String, StoredSshSecretBundle>,
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
            keychain_catalog: KeychainCatalog::default(),
            keychain_identity_secret_bundles: BTreeMap::new(),
            keychain_key_secret_bundles: BTreeMap::new(),
            known_hosts: Vec::new(),
            sync_preferences: SnapshotSyncPreferences::default(),
            ui_preferences: SnapshotUiPreferences::default(),
        }
    }
}

fn default_vault_ssh_auth_source() -> String {
    "manual".into()
}

const fn default_create_new_gist() -> bool {
    true
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
    GitRepo,
    GitHubGist,
    GitLabSnippet,
    GiteeGist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderAuthKind {
    AwsStandardChain,
    HttpsCredentials,
    SshKey,
    DeviceFlow,
    Pkce,
    Pat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GitHostKind {
    #[default]
    Gitee,
    GitHub,
    GitLab,
    Generic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GitRepositoryVisibility {
    Private,
    Public,
    Internal,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GitRepositoryWritePermission {
    Writable,
    ReadOnly,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GitRemoteSafetyStatus {
    Safe,
    #[default]
    Unknown,
    Stale,
    Paused,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiteeRemoteDraft {
    #[serde(default)]
    pub personal_access_token: String,
    #[serde(default)]
    pub gist_id: String,
    #[serde(default = "default_create_new_gist")]
    pub create_new_gist: bool,
}

impl GiteeRemoteDraft {
    pub fn setup_summary(&self) -> String {
        if self.create_new_gist {
            "Configure a Gitee Personal Access Token and a Gist ID target, or create a new Gist, to enable sync.".into()
        } else {
            "Configure a Gitee Personal Access Token and Gist ID to enable sync.".into()
        }
    }
}

impl Default for GiteeRemoteDraft {
    fn default() -> Self {
        Self {
            personal_access_token: String::new(),
            gist_id: String::new(),
            create_new_gist: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitRepoRemoteDraft {
    #[serde(default)]
    pub host_kind: GitHostKind,
    #[serde(default)]
    pub remote_url: String,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_base_url: String,
    #[serde(default)]
    pub namespace: String,
    #[serde(default)]
    pub repository: String,
    #[serde(default = "default_git_repo_branch")]
    pub branch: String,
    #[serde(default)]
    pub root_path: String,
    #[serde(default = "default_git_repo_auth_kind")]
    pub auth_kind: ProviderAuthKind,
    #[serde(default)]
    pub https_username: String,
    #[serde(default)]
    pub https_secret: String,
    #[serde(default)]
    pub personal_access_token: String,
    #[serde(default)]
    pub ssh_private_key: String,
    #[serde(default)]
    pub ssh_passphrase: String,
}

impl GitRepoRemoteDraft {
    pub fn setup_summary(&self) -> String {
        format!(
            "Configure a {} Git remote URL, branch, and either HTTPS credentials or SSH key to enable sync.",
            self.host_kind.label()
        )
    }
}

impl Default for GitRepoRemoteDraft {
    fn default() -> Self {
        Self {
            host_kind: GitHostKind::default(),
            remote_url: String::new(),
            base_url: "https://gitee.com".into(),
            api_base_url: "https://gitee.com/api/v5".into(),
            namespace: String::new(),
            repository: String::new(),
            branch: default_git_repo_branch(),
            root_path: ".mica-term-sync".into(),
            auth_kind: ProviderAuthKind::Pat,
            https_username: String::new(),
            https_secret: String::new(),
            personal_access_token: String::new(),
            ssh_private_key: String::new(),
            ssh_passphrase: String::new(),
        }
    }
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
    GitRepo {
        host_kind: GitHostKind,
        remote_url: String,
        branch: String,
        #[serde(default)]
        base_url: Option<String>,
        #[serde(default)]
        api_base_url: Option<String>,
        #[serde(default)]
        namespace: Option<String>,
        #[serde(default)]
        repository: Option<String>,
        #[serde(default)]
        root_path: Option<String>,
        #[serde(default)]
        display_name: Option<String>,
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

impl BootstrapBundle {
    pub fn primary_remote(&self) -> Option<&BootstrapRemoteConfig> {
        self.remotes
            .iter()
            .find(|remote| remote.role == RemoteRole::Primary)
    }
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

fn default_git_repo_branch() -> String {
    "main".into()
}

const fn default_git_repo_auth_kind() -> ProviderAuthKind {
    ProviderAuthKind::Pat
}

impl GitHostKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Gitee => "gitee",
            Self::GitHub => "github",
            Self::GitLab => "gitlab",
            Self::Generic => "generic",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "github" => Self::GitHub,
            "gitlab" => Self::GitLab,
            "generic" => Self::Generic,
            _ => Self::Gitee,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Gitee => "Gitee",
            Self::GitHub => "GitHub",
            Self::GitLab => "GitLab",
            Self::Generic => "Git",
        }
    }
}
