use std::path::Path;

use anyhow::Result;

use crate::app::keychain::{
    KeychainCatalog, KeychainIdentitySpec, KeychainNodePayload, KeychainSshKeySpec,
};
use crate::app::assets_catalog::{asset_tree_to_vault_catalog, vault_catalog_to_asset_tree};
use crate::app::ssh::credentials::{
    CredentialStore, StoredSshSecretBundle, restore_snapshot_secret_bundle,
    snapshot_secret_bundle,
};
use crate::app::ssh::known_hosts::KnownHostsService;
use crate::app::ui_preferences::{UiPreferences, ui_preferences_from_snapshot};
use crate::app::vault::model::{SnapshotSyncPreferences, VaultAssetPayload, VaultSnapshot};
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
    known_hosts_path: &Path,
    sync_preferences: SnapshotSyncPreferences,
    ui_preferences: &UiPreferences,
) -> Result<VaultSnapshot> {
    let asset_catalog = asset_tree_to_vault_catalog(asset_tree);
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

    let known_hosts = KnownHostsService::new(known_hosts_path).export_snapshot_entries()?;

    Ok(VaultSnapshot {
        schema_version: 1,
        asset_catalog,
        ssh_secret_bundles,
        keychain_catalog: keychain_catalog.clone(),
        keychain_identity_secret_bundles,
        keychain_key_secret_bundles,
        known_hosts,
        sync_preferences,
        ui_preferences: ui_preferences.into(),
    })
}

pub fn apply_vault_snapshot(
    snapshot: &VaultSnapshot,
    credential_store: &dyn CredentialStore,
    known_hosts_path: &Path,
) -> Result<AppliedVaultSnapshot> {
    let asset_tree = vault_catalog_to_asset_tree(&snapshot.asset_catalog);

    for node in snapshot.asset_catalog.nodes.values() {
        let VaultAssetPayload::SshConnection(spec) = &node.payload else {
            continue;
        };

        restore_snapshot_secret_bundle(
            credential_store,
            spec.credential_ref.as_deref(),
            snapshot.ssh_secret_bundles.get(&node.id),
        )?;
    }

    for node in snapshot.keychain_catalog.nodes.values() {
        match &node.payload {
            KeychainNodePayload::Folder => {}
            KeychainNodePayload::Identity(spec) => restore_keychain_bundle(
                credential_store,
                spec,
                snapshot.keychain_identity_secret_bundles.get(&node.id),
            )?,
            KeychainNodePayload::SshKey(spec) => restore_keychain_bundle(
                credential_store,
                spec,
                snapshot.keychain_key_secret_bundles.get(&node.id),
            )?,
        }
    }

    KnownHostsService::new(known_hosts_path).replace_snapshot_entries(&snapshot.known_hosts)?;

    Ok(AppliedVaultSnapshot {
        asset_tree,
        keychain_catalog: snapshot.keychain_catalog.clone(),
        sync_preferences: snapshot.sync_preferences.clone(),
        ui_preferences: ui_preferences_from_snapshot(&snapshot.ui_preferences),
    })
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
    if let Some(bundle) =
        snapshot_secret_bundle(credential_store, secret_holder.credential_ref())?
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
