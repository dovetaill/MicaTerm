//! Persisted schema for the console asset catalog.

use std::collections::BTreeMap;

pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 5;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAssetCatalog {
    pub schema_version: u32,
    pub root_ids: Vec<String>,
    pub nodes: BTreeMap<String, PersistedAssetNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedAssetNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub kind: PersistedAssetKind,
    pub child_ids: Vec<String>,
    pub payload: PersistedAssetPayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedAssetKind {
    Folder,
    SshConnection,
    SnippetPackage,
    Snippet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistedAssetDomain {
    Console,
    Snippets,
}

impl PersistedAssetKind {
    pub fn domain(self) -> PersistedAssetDomain {
        match self {
            Self::Folder | Self::SshConnection => PersistedAssetDomain::Console,
            Self::SnippetPackage | Self::Snippet => PersistedAssetDomain::Snippets,
        }
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistedAssetPayload {
    Folder,
    SshConnection(PersistedSshConnectionSpec),
    SnippetPackage,
    Snippet(PersistedSnippetSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedAssetSocks5ProxySpec {
    pub host: String,
    pub port: String,
    pub username: String,
    pub password_credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PersistedAssetSshProxySpec {
    #[default]
    None,
    Socks5(PersistedAssetSocks5ProxySpec),
    Http(PersistedAssetSocks5ProxySpec),
    SshAsset {
        asset_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_method: String,
    pub private_key_source: String,
    pub private_key_path: String,
    pub environment: String,
    pub proxy: PersistedAssetSshProxySpec,
    pub remark: String,
    pub credential_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedSnippetSpec {
    pub script: String,
    pub package_id: Option<String>,
}
