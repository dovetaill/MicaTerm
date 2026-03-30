use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeychainCatalog {
    pub root_ids: Vec<String>,
    pub nodes: BTreeMap<String, KeychainNode>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeychainNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub title: String,
    pub kind: KeychainNodeKind,
    pub child_ids: Vec<String>,
    pub payload: KeychainNodePayload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeychainNodeKind {
    Folder,
    Identity,
    SshKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KeychainNodePayload {
    Folder,
    Identity(KeychainIdentitySpec),
    SshKey(KeychainSshKeySpec),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KeychainIdentityAuthKind {
    #[default]
    Password,
    SshKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeychainIdentitySpec {
    pub username: String,
    pub auth_kind: KeychainIdentityAuthKind,
    pub ssh_key_id: Option<String>,
    pub credential_ref: Option<String>,
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeychainSshKeySpec {
    pub algorithm: String,
    pub fingerprint: String,
    pub public_key: String,
    pub comment: String,
    pub credential_ref: Option<String>,
    pub remark: String,
}
