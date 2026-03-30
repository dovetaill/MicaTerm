use anyhow::Result;

use crate::app::vault::crypto::EncryptedSnapshot;
use crate::app::vault::model::{PackLayout, ProviderKind, VaultHead, VaultManifest};

pub mod gitee_gist;
pub mod github_gist;
pub mod gitlab_snippet;
pub mod mock;
pub mod s3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderCapabilities {
    pub supports_conditional_head_write: bool,
    pub max_pack_count: usize,
    pub max_pack_bytes: usize,
    pub preferred_pack_strategy: PackLayout,
}

impl ProviderCapabilities {
    pub fn s3_like() -> Self {
        Self {
            supports_conditional_head_write: true,
            max_pack_count: 64,
            max_pack_bytes: 16 * 1024 * 1024,
            preferred_pack_strategy: PackLayout::ObjectSet,
        }
    }

    pub fn bundled_files_like() -> Self {
        Self {
            supports_conditional_head_write: false,
            max_pack_count: 4,
            max_pack_bytes: 1024 * 1024,
            preferred_pack_strategy: PackLayout::BundledFiles,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProviderReadResult {
    pub head: Option<VaultHead>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderWriteRequest {
    pub head: VaultHead,
    pub manifest: VaultManifest,
    pub encrypted_snapshot: EncryptedSnapshot,
    pub expected_parent_revision: Option<String>,
    pub conditional_head_write: bool,
}

pub trait VaultProvider: Send + Sync {
    fn remote_id(&self) -> &str;
    fn capabilities(&self) -> ProviderCapabilities;
    fn read_head(&self) -> Result<ProviderReadResult>;
    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()>;
}

pub fn first_release_formal_provider_kind() -> ProviderKind {
    ProviderKind::GiteeGist
}

pub fn first_release_formal_provider_label() -> &'static str {
    "Gitee"
}

pub fn first_release_formal_auth_label() -> &'static str {
    "Personal Access Token"
}
