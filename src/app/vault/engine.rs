use std::fmt;
use std::sync::Arc;

use crate::app::vault::crypto::{EncryptedSnapshot, encrypt_snapshot};
use crate::app::vault::model::{
    CipherKind, KdfConfig, PackLayout, PackRef, ProviderKind, VaultHead, VaultManifest,
    VaultSnapshot,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderWriteRequest, VaultProvider, attach_snapshot_recovery_metadata,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncRequest {
    pub vault_id: String,
    pub snapshot: VaultSnapshot,
    pub next_revision: String,
    pub parent_revision: Option<String>,
    pub device_id: String,
    pub created_at: String,
    pub wrapped_vault_key: String,
    pub kdf: KdfConfig,
    pub provider_kind: ProviderKind,
    pub vault_key: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncMirrorFailure {
    pub remote_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub primary_remote_id: String,
    pub primary_revision: String,
    pub head: VaultHead,
    pub manifest: VaultManifest,
    pub encrypted_snapshot: EncryptedSnapshot,
    pub mirror_failures: Vec<SyncMirrorFailure>,
}

impl SyncReport {
    pub fn is_mirror_degraded(&self) -> bool {
        !self.mirror_failures.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Conflict {
        remote_id: String,
        expected_parent_revision: Option<String>,
        actual_primary_revision: Option<String>,
    },
    PrimaryReadFailed {
        remote_id: String,
        message: String,
    },
    PrimaryWriteFailed {
        remote_id: String,
        message: String,
    },
    PayloadAssemblyFailed {
        message: String,
    },
}

impl fmt::Display for SyncError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict {
                remote_id,
                expected_parent_revision,
                actual_primary_revision,
            } => write!(
                f,
                "primary remote `{remote_id}` revision conflict: expected parent {:?}, found {:?}",
                expected_parent_revision, actual_primary_revision
            ),
            Self::PrimaryReadFailed { remote_id, message } => {
                write!(f, "failed to read primary remote `{remote_id}`: {message}")
            }
            Self::PrimaryWriteFailed { remote_id, message } => {
                write!(f, "failed to write primary remote `{remote_id}`: {message}")
            }
            Self::PayloadAssemblyFailed { message } => {
                write!(f, "failed to assemble encrypted vault payload: {message}")
            }
        }
    }
}

impl std::error::Error for SyncError {}

#[derive(Clone)]
pub struct SyncEngine {
    primary: Arc<dyn VaultProvider>,
    mirrors: Vec<Arc<dyn VaultProvider>>,
}

impl SyncEngine {
    pub fn new(primary: Arc<dyn VaultProvider>, mirrors: Vec<Arc<dyn VaultProvider>>) -> Self {
        Self { primary, mirrors }
    }

    pub fn sync(&self, request: SyncRequest) -> Result<SyncReport, SyncError> {
        let primary_remote_id = self.primary.remote_id().to_string();
        let primary_head = self
            .primary
            .read_head()
            .map_err(|err| SyncError::PrimaryReadFailed {
                remote_id: primary_remote_id.clone(),
                message: err.to_string(),
            })?
            .head;
        let actual_primary_revision = primary_head
            .as_ref()
            .map(|head| head.vault_revision.clone());

        if request.parent_revision != actual_primary_revision {
            return Err(SyncError::Conflict {
                remote_id: primary_remote_id,
                expected_parent_revision: request.parent_revision,
                actual_primary_revision,
            });
        }

        let encrypted_snapshot =
            encrypt_snapshot(&request.snapshot, &request.vault_key).map_err(|err| {
                SyncError::PayloadAssemblyFailed {
                    message: err.to_string(),
                }
            })?;
        let primary_request = build_provider_write_request(
            &request,
            &encrypted_snapshot,
            &self.primary.capabilities(),
            true,
        );
        self.primary
            .write_revision(&primary_request)
            .map_err(|err| SyncError::PrimaryWriteFailed {
                remote_id: self.primary.remote_id().to_string(),
                message: err.to_string(),
            })?;

        let mut mirror_failures = Vec::new();
        for mirror in &self.mirrors {
            let mirror_request = build_provider_write_request(
                &request,
                &encrypted_snapshot,
                &mirror.capabilities(),
                false,
            );
            if let Err(err) = mirror.write_revision(&mirror_request) {
                mirror_failures.push(SyncMirrorFailure {
                    remote_id: mirror.remote_id().to_string(),
                    message: err.to_string(),
                });
            }
        }

        Ok(SyncReport {
            primary_remote_id: self.primary.remote_id().to_string(),
            primary_revision: request.next_revision,
            head: primary_request.head,
            manifest: primary_request.manifest,
            encrypted_snapshot: primary_request.encrypted_snapshot,
            mirror_failures,
        })
    }
}

fn build_provider_write_request(
    request: &SyncRequest,
    encrypted_snapshot: &EncryptedSnapshot,
    capabilities: &ProviderCapabilities,
    is_primary: bool,
) -> ProviderWriteRequest {
    let pack_layout = capabilities.preferred_pack_strategy;
    let manifest_ref = manifest_ref_for(pack_layout, &request.next_revision);
    let pack_ref = PackRef {
        pack_id: format!("pack-{}", request.next_revision),
        object_name: pack_object_name_for(pack_layout, &request.next_revision),
        size_bytes: encrypted_snapshot.ciphertext.len() as u64,
        digest: format!("sha256:{}", encrypted_snapshot.payload_sha256),
    };
    let manifest = VaultManifest {
        packs: vec![pack_ref],
        ..VaultManifest::default()
    };
    let mut manifest = manifest;
    attach_snapshot_recovery_metadata(&mut manifest, encrypted_snapshot);
    let head = VaultHead {
        format_version: 1,
        vault_id: request.vault_id.clone(),
        vault_revision: request.next_revision.clone(),
        parent_revision: request.parent_revision.clone(),
        device_id: request.device_id.clone(),
        created_at: request.created_at.clone(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref,
        wrapped_vault_key: request.wrapped_vault_key.clone(),
        kdf: request.kdf.clone(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: encrypted_snapshot.compression,
        pack_layout,
    };

    ProviderWriteRequest {
        head,
        manifest,
        encrypted_snapshot: encrypted_snapshot.clone(),
        expected_parent_revision: request.parent_revision.clone(),
        conditional_head_write: is_primary && capabilities.supports_conditional_head_write,
    }
}

fn manifest_ref_for(pack_layout: PackLayout, revision: &str) -> String {
    match pack_layout {
        PackLayout::ObjectSet => format!("manifest/{revision}.bin"),
        PackLayout::BundledFiles => format!("bundle/{revision}/manifest.bin"),
    }
}

fn pack_object_name_for(pack_layout: PackLayout, revision: &str) -> String {
    match pack_layout {
        PackLayout::ObjectSet => format!("packs/{revision}-snapshot.bin"),
        PackLayout::BundledFiles => format!("bundle/{revision}/snapshot.bin"),
    }
}
