//! Persisted schema for the console asset catalog.

use std::collections::BTreeMap;

pub const ASSET_CATALOG_SCHEMA_VERSION: u32 = 1;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistedAssetPayload {
    Folder,
    SshConnection(PersistedSshConnectionSpec),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PersistedSshConnectionSpec {
    pub host: String,
    pub user: String,
    pub port: String,
    pub environment: String,
    pub proxy_method: String,
}
