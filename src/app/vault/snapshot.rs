use std::collections::BTreeSet;
use std::path::Path;

use anyhow::Result;

use crate::app::assets_catalog::{asset_tree_to_vault_catalog, vault_catalog_to_asset_tree};
use crate::app::keychain::{
    KeychainCatalog, KeychainIdentitySpec, KeychainNode, KeychainNodePayload, KeychainSshKeySpec,
};
use crate::app::ssh::credentials::{
    CredentialStore, SshCredentialKind, StoredSshSecretBundle, keychain_identity_credential_ref,
    keychain_key_credential_ref, restore_snapshot_secret_bundle, snapshot_secret_bundle,
    ssh_credential_ref,
};
use crate::app::ui_preferences::UiPreferences;
use crate::app::vault::model::{
    SnapshotSyncPreferences, VaultAssetPayload, VaultSnapshot, VaultSshProxySpec,
};
use crate::shell::assets::AssetTree;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedVaultSnapshot {
    pub asset_tree: AssetTree,
    pub keychain_catalog: KeychainCatalog,
    pub sync_preferences: SnapshotSyncPreferences,
    pub ui_preferences: UiPreferences,
}

pub fn export_vault_snapshot(
    asset_tree: &AssetTree,
    keychain_catalog: &KeychainCatalog,
    credential_store: &dyn CredentialStore,
    _known_hosts_path: &Path,
    _sync_preferences: SnapshotSyncPreferences,
    _ui_preferences: &UiPreferences,
) -> Result<VaultSnapshot> {
    let asset_catalog = asset_tree_to_vault_catalog(asset_tree);
    let keychain_catalog = normalize_keychain_merge_metadata(keychain_catalog.clone());
    let mut ssh_secret_bundles = std::collections::BTreeMap::new();
    let mut keychain_identity_secret_bundles = std::collections::BTreeMap::new();
    let mut keychain_key_secret_bundles = std::collections::BTreeMap::new();

    for node in asset_catalog.nodes.values() {
        let VaultAssetPayload::SshConnection(spec) = &node.payload else {
            continue;
        };

        if let Some(bundle) =
            snapshot_secret_bundle(credential_store, spec.credential_ref.as_deref())?
        {
            ssh_secret_bundles.insert(node.id.clone(), bundle);
        }
    }

    for node in keychain_catalog.nodes.values() {
        match &node.payload {
            KeychainNodePayload::Folder => {}
            KeychainNodePayload::Identity(spec) => {
                insert_keychain_bundle(
                    &mut keychain_identity_secret_bundles,
                    credential_store,
                    &node.id,
                    spec,
                )?;
            }
            KeychainNodePayload::SshKey(spec) => {
                insert_keychain_bundle(
                    &mut keychain_key_secret_bundles,
                    credential_store,
                    &node.id,
                    spec,
                )?;
            }
        }
    }

    Ok(normalize_snapshot_secret_refs(VaultSnapshot {
        schema_version: 1,
        asset_catalog,
        ssh_secret_bundles,
        keychain_catalog,
        keychain_identity_secret_bundles,
        keychain_key_secret_bundles,
        known_hosts: Vec::new(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: Default::default(),
    }))
}

fn normalize_keychain_merge_metadata(catalog: KeychainCatalog) -> KeychainCatalog {
    let mut catalog = catalog;
    catalog
        .merge_metadata
        .retain(|node_id, metadata| catalog.nodes.contains_key(node_id) || metadata.is_deleted());
    catalog
}

pub fn apply_vault_snapshot(
    snapshot: &VaultSnapshot,
    credential_store: &dyn CredentialStore,
    _known_hosts_path: &Path,
) -> Result<AppliedVaultSnapshot> {
    let normalized_snapshot = normalize_snapshot_secret_refs(snapshot.clone());
    let obsolete_secret_refs = obsolete_ssh_secret_refs(snapshot, &normalized_snapshot);
    let obsolete_keychain_refs = obsolete_keychain_secret_refs(snapshot, &normalized_snapshot);
    let asset_tree = vault_catalog_to_asset_tree(&normalized_snapshot.asset_catalog);

    for node in normalized_snapshot.asset_catalog.nodes.values() {
        let VaultAssetPayload::SshConnection(spec) = &node.payload else {
            continue;
        };

        restore_snapshot_secret_bundle(
            credential_store,
            spec.credential_ref.as_deref(),
            normalized_snapshot.ssh_secret_bundles.get(&node.id),
        )?;
    }

    for credential_ref in obsolete_secret_refs {
        credential_store.delete_secret(credential_ref.as_str())?;
    }

    for node in normalized_snapshot.keychain_catalog.nodes.values() {
        match &node.payload {
            KeychainNodePayload::Folder => {}
            KeychainNodePayload::Identity(spec) => restore_keychain_bundle(
                credential_store,
                spec,
                normalized_snapshot
                    .keychain_identity_secret_bundles
                    .get(&node.id),
            )?,
            KeychainNodePayload::SshKey(spec) => restore_keychain_bundle(
                credential_store,
                spec,
                normalized_snapshot
                    .keychain_key_secret_bundles
                    .get(&node.id),
            )?,
        }
    }

    for credential_ref in obsolete_keychain_refs {
        credential_store.delete_secret(credential_ref.as_str())?;
    }

    Ok(AppliedVaultSnapshot {
        asset_tree,
        keychain_catalog: normalized_snapshot.keychain_catalog.clone(),
        sync_preferences: SnapshotSyncPreferences::default(),
        ui_preferences: UiPreferences::default(),
    })
}

pub fn normalize_snapshot_secret_refs(snapshot: VaultSnapshot) -> VaultSnapshot {
    let mut snapshot = snapshot;
    let duplicated_refs = duplicated_ssh_secret_refs(&snapshot.asset_catalog);
    let identity_bundle_ids = snapshot
        .keychain_identity_secret_bundles
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let key_bundle_ids = snapshot
        .keychain_key_secret_bundles
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();

    for (node_id, node) in &mut snapshot.asset_catalog.nodes {
        let VaultAssetPayload::SshConnection(spec) = &mut node.payload else {
            continue;
        };
        let saved_bundle = snapshot.ssh_secret_bundles.get(node_id);
        let bundle_has_proxy_secret = saved_bundle
            .and_then(|bundle| bundle.proxy_socks5_password.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        let proxy_needs_saved_secret = match &spec.proxy {
            VaultSshProxySpec::Socks5(proxy) | VaultSshProxySpec::Http(proxy) => {
                proxy.password_credential_ref.is_some() || bundle_has_proxy_secret
            }
            VaultSshProxySpec::None | VaultSshProxySpec::SshAsset { .. } => false,
        };
        let needs_canonical_secret_ref =
            spec.credential_ref.is_some() || proxy_needs_saved_secret || saved_bundle.is_some();
        let duplicated_secret_ref = spec
            .credential_ref
            .as_ref()
            .is_some_and(|credential_ref| duplicated_refs.contains(credential_ref));
        let canonical_ref = if needs_canonical_secret_ref {
            Some(if duplicated_secret_ref {
                ssh_credential_ref(node_id, SshCredentialKind::SavedSecrets)
            } else {
                spec.credential_ref
                    .clone()
                    .unwrap_or_else(|| ssh_credential_ref(node_id, SshCredentialKind::SavedSecrets))
            })
        } else {
            None
        };
        spec.credential_ref = canonical_ref.clone();

        match &mut spec.proxy {
            VaultSshProxySpec::Socks5(proxy) | VaultSshProxySpec::Http(proxy) => {
                proxy.password_credential_ref = if proxy_needs_saved_secret {
                    canonical_ref.clone()
                } else {
                    None
                };
            }
            VaultSshProxySpec::None | VaultSshProxySpec::SshAsset { .. } => {}
        }
    }

    for (node_id, node) in &mut snapshot.keychain_catalog.nodes {
        match &mut node.payload {
            KeychainNodePayload::Folder => {}
            KeychainNodePayload::Identity(spec) => {
                let needs_canonical_ref =
                    spec.credential_ref.is_some() || identity_bundle_ids.contains(node_id);
                spec.credential_ref =
                    needs_canonical_ref.then(|| keychain_identity_credential_ref(node_id.as_str()));
            }
            KeychainNodePayload::SshKey(spec) => {
                let needs_canonical_ref =
                    spec.credential_ref.is_some() || key_bundle_ids.contains(node_id);
                spec.credential_ref =
                    needs_canonical_ref.then(|| keychain_key_credential_ref(node_id.as_str()));
            }
        }
    }

    snapshot
}

fn duplicated_ssh_secret_refs(
    catalog: &crate::app::vault::model::VaultAssetCatalog,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut duplicated = BTreeSet::new();

    for node in catalog.nodes.values() {
        let VaultAssetPayload::SshConnection(spec) = &node.payload else {
            continue;
        };
        let Some(credential_ref) = spec.credential_ref.as_deref() else {
            continue;
        };
        if !seen.insert(credential_ref.to_string()) {
            duplicated.insert(credential_ref.to_string());
        }
    }

    duplicated
}

fn obsolete_ssh_secret_refs(
    original: &VaultSnapshot,
    normalized: &VaultSnapshot,
) -> BTreeSet<String> {
    let normalized_refs = normalized
        .asset_catalog
        .nodes
        .values()
        .filter_map(|node| match &node.payload {
            VaultAssetPayload::SshConnection(spec) => spec.credential_ref.clone(),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let mut obsolete = BTreeSet::new();

    for (node_id, node) in &original.asset_catalog.nodes {
        let VaultAssetPayload::SshConnection(spec) = &node.payload else {
            continue;
        };
        let Some(previous_ref) = spec.credential_ref.as_ref() else {
            continue;
        };
        let Some(normalized_node) = normalized.asset_catalog.nodes.get(node_id) else {
            continue;
        };
        let VaultAssetPayload::SshConnection(normalized_spec) = &normalized_node.payload else {
            continue;
        };
        if normalized_spec.credential_ref.as_ref() != Some(previous_ref)
            && !normalized_refs.contains(previous_ref)
        {
            obsolete.insert(previous_ref.clone());
        }
    }

    obsolete
}

fn obsolete_keychain_secret_refs(
    original: &VaultSnapshot,
    normalized: &VaultSnapshot,
) -> BTreeSet<String> {
    let normalized_refs = normalized
        .keychain_catalog
        .nodes
        .values()
        .filter_map(keychain_node_credential_ref)
        .collect::<BTreeSet<_>>();
    let mut obsolete = BTreeSet::new();

    for (node_id, node) in &original.keychain_catalog.nodes {
        let Some(previous_ref) = keychain_node_credential_ref(node) else {
            continue;
        };
        let Some(normalized_node) = normalized.keychain_catalog.nodes.get(node_id) else {
            continue;
        };
        if keychain_node_credential_ref(normalized_node).as_ref() != Some(&previous_ref)
            && !normalized_refs.contains(&previous_ref)
        {
            obsolete.insert(previous_ref);
        }
    }

    obsolete
}

fn keychain_node_credential_ref(node: &KeychainNode) -> Option<String> {
    match &node.payload {
        KeychainNodePayload::Folder => None,
        KeychainNodePayload::Identity(spec) => spec.credential_ref.clone(),
        KeychainNodePayload::SshKey(spec) => spec.credential_ref.clone(),
    }
}

fn insert_keychain_bundle<T>(
    bundles: &mut std::collections::BTreeMap<String, StoredSshSecretBundle>,
    credential_store: &dyn CredentialStore,
    node_id: &str,
    secret_holder: &T,
) -> Result<()>
where
    T: KeychainSecretHolder,
{
    if let Some(bundle) = snapshot_secret_bundle(credential_store, secret_holder.credential_ref())?
    {
        bundles.insert(node_id.to_string(), bundle);
    }

    Ok(())
}

fn restore_keychain_bundle<T>(
    credential_store: &dyn CredentialStore,
    secret_holder: &T,
    bundle: Option<&StoredSshSecretBundle>,
) -> Result<()>
where
    T: KeychainSecretHolder,
{
    restore_snapshot_secret_bundle(credential_store, secret_holder.credential_ref(), bundle)
}

trait KeychainSecretHolder {
    fn credential_ref(&self) -> Option<&str>;
}

impl KeychainSecretHolder for KeychainIdentitySpec {
    fn credential_ref(&self) -> Option<&str> {
        self.credential_ref.as_deref()
    }
}

impl KeychainSecretHolder for KeychainSshKeySpec {
    fn credential_ref(&self) -> Option<&str> {
        self.credential_ref.as_deref()
    }
}
