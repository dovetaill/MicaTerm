use std::collections::{BTreeMap, BTreeSet};

use crate::app::keychain::model::{
    KeychainCatalog, KeychainNode, KeychainNodeMergeMetadata, KeychainNodePayload,
};
use crate::app::ssh::credentials::StoredSshSecretBundle;
use crate::app::vault::model::{
    VaultAssetCatalog, VaultAssetNode, VaultAssetPayload, VaultKnownHostEntry, VaultNodeMergeMetadata,
    VaultSnapshot,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeInput {
    pub base: VaultSnapshot,
    pub local: VaultSnapshot,
    pub remote: VaultSnapshot,
    pub device_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeEntityKind {
    Asset,
    KeychainNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeConflict {
    pub entity: MergeEntityKind,
    pub node_id: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeRecoveryAction {
    ConflictCopyRequired {
        entity: MergeEntityKind,
        node_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResult {
    pub merged: VaultSnapshot,
    pub conflicts: Vec<MergeConflict>,
    pub recovery_actions: Vec<MergeRecoveryAction>,
}

pub fn merge_snapshots(input: MergeInput) -> MergeResult {
    let mut conflicts = Vec::new();
    let mut recovery_actions = Vec::new();
    let asset_catalog = merge_asset_catalog(
        &input.base.asset_catalog,
        &input.local.asset_catalog,
        &input.remote.asset_catalog,
        &mut conflicts,
        &mut recovery_actions,
    );
    let keychain_catalog = merge_keychain_catalog(
        &input.base.keychain_catalog,
        &input.local.keychain_catalog,
        &input.remote.keychain_catalog,
        &mut conflicts,
        &mut recovery_actions,
    );

    let merged = VaultSnapshot {
        schema_version: input
            .base
            .schema_version
            .max(input.local.schema_version)
            .max(input.remote.schema_version),
        ssh_secret_bundles: merge_asset_secret_bundles(
            &asset_catalog,
            &input.base.ssh_secret_bundles,
            &input.local.ssh_secret_bundles,
            &input.remote.ssh_secret_bundles,
        ),
        keychain_identity_secret_bundles: merge_keychain_secret_bundles(
            &keychain_catalog,
            &input.base.keychain_identity_secret_bundles,
            &input.local.keychain_identity_secret_bundles,
            &input.remote.keychain_identity_secret_bundles,
            keychain_identity_node_ids,
        ),
        keychain_key_secret_bundles: merge_keychain_secret_bundles(
            &keychain_catalog,
            &input.base.keychain_key_secret_bundles,
            &input.local.keychain_key_secret_bundles,
            &input.remote.keychain_key_secret_bundles,
            keychain_ssh_key_node_ids,
        ),
        asset_catalog,
        keychain_catalog,
        known_hosts: merge_known_hosts(
            &input.base.known_hosts,
            &input.local.known_hosts,
            &input.remote.known_hosts,
        ),
        sync_preferences: input.local.sync_preferences,
        ui_preferences: input.local.ui_preferences,
    };

    MergeResult {
        merged,
        conflicts,
        recovery_actions,
    }
}

fn merge_asset_catalog(
    base: &VaultAssetCatalog,
    local: &VaultAssetCatalog,
    remote: &VaultAssetCatalog,
    conflicts: &mut Vec<MergeConflict>,
    recovery_actions: &mut Vec<MergeRecoveryAction>,
) -> VaultAssetCatalog {
    let ordered_ids = ordered_ids([
        local.nodes.keys().cloned().collect(),
        remote.nodes.keys().cloned().collect(),
        base.nodes.keys().cloned().collect(),
        local.merge_metadata.keys().cloned().collect(),
        remote.merge_metadata.keys().cloned().collect(),
        base.merge_metadata.keys().cloned().collect(),
    ]);
    let mut nodes = BTreeMap::new();
    let mut merge_metadata = BTreeMap::new();

    for node_id in ordered_ids {
        let decision = decide_asset_node(
            node_id.as_str(),
            base.nodes.get(&node_id),
            local.nodes.get(&node_id),
            remote.nodes.get(&node_id),
            base.merge_metadata.get(&node_id),
            local.merge_metadata.get(&node_id),
            remote.merge_metadata.get(&node_id),
        );

        if let Some(conflict) = decision.conflict {
            conflicts.push(conflict);
            recovery_actions.push(MergeRecoveryAction::ConflictCopyRequired {
                entity: MergeEntityKind::Asset,
                node_id: node_id.clone(),
            });
        }
        let has_live_node = decision.node.is_some();
        if let Some(node) = decision.node {
            nodes.insert(node_id.clone(), node);
        }
        if let Some(metadata) = decision.metadata
            && (has_live_node || metadata.is_deleted())
        {
            merge_metadata.insert(node_id, metadata);
        }
    }

    let (root_ids, rebuilt_nodes) = rebuild_asset_relationships(nodes, local, remote, base);
    VaultAssetCatalog {
        root_ids,
        nodes: rebuilt_nodes,
        merge_metadata,
    }
}

fn merge_keychain_catalog(
    base: &KeychainCatalog,
    local: &KeychainCatalog,
    remote: &KeychainCatalog,
    conflicts: &mut Vec<MergeConflict>,
    recovery_actions: &mut Vec<MergeRecoveryAction>,
) -> KeychainCatalog {
    let ordered_ids = ordered_ids([
        local.nodes.keys().cloned().collect(),
        remote.nodes.keys().cloned().collect(),
        base.nodes.keys().cloned().collect(),
        local.merge_metadata.keys().cloned().collect(),
        remote.merge_metadata.keys().cloned().collect(),
        base.merge_metadata.keys().cloned().collect(),
    ]);
    let mut nodes = BTreeMap::new();
    let mut merge_metadata = BTreeMap::new();

    for node_id in ordered_ids {
        let decision = decide_keychain_node(
            node_id.as_str(),
            base.nodes.get(&node_id),
            local.nodes.get(&node_id),
            remote.nodes.get(&node_id),
            base.merge_metadata.get(&node_id),
            local.merge_metadata.get(&node_id),
            remote.merge_metadata.get(&node_id),
        );

        if let Some(conflict) = decision.conflict {
            conflicts.push(conflict);
            recovery_actions.push(MergeRecoveryAction::ConflictCopyRequired {
                entity: MergeEntityKind::KeychainNode,
                node_id: node_id.clone(),
            });
        }
        let has_live_node = decision.node.is_some();
        if let Some(node) = decision.node {
            nodes.insert(node_id.clone(), node);
        }
        if let Some(metadata) = decision.metadata
            && (has_live_node || metadata.is_deleted())
        {
            merge_metadata.insert(node_id, metadata);
        }
    }

    let (root_ids, rebuilt_nodes) = rebuild_keychain_relationships(nodes, local, remote, base);
    KeychainCatalog {
        root_ids,
        nodes: rebuilt_nodes,
        merge_metadata,
    }
}

fn merge_asset_secret_bundles(
    catalog: &VaultAssetCatalog,
    base: &BTreeMap<String, StoredSshSecretBundle>,
    local: &BTreeMap<String, StoredSshSecretBundle>,
    remote: &BTreeMap<String, StoredSshSecretBundle>,
) -> BTreeMap<String, StoredSshSecretBundle> {
    let mut bundles = BTreeMap::new();
    for (node_id, node) in &catalog.nodes {
        if !matches!(node.payload, VaultAssetPayload::SshConnection(_)) {
            continue;
        }
        if let Some(bundle) = local
            .get(node_id)
            .or_else(|| remote.get(node_id))
            .or_else(|| base.get(node_id))
        {
            bundles.insert(node_id.clone(), bundle.clone());
        }
    }
    bundles
}

fn merge_keychain_secret_bundles(
    catalog: &KeychainCatalog,
    base: &BTreeMap<String, StoredSshSecretBundle>,
    local: &BTreeMap<String, StoredSshSecretBundle>,
    remote: &BTreeMap<String, StoredSshSecretBundle>,
    filter: fn(&KeychainNode) -> bool,
) -> BTreeMap<String, StoredSshSecretBundle> {
    let mut bundles = BTreeMap::new();
    for (node_id, node) in &catalog.nodes {
        if !filter(node) {
            continue;
        }
        if let Some(bundle) = local
            .get(node_id)
            .or_else(|| remote.get(node_id))
            .or_else(|| base.get(node_id))
        {
            bundles.insert(node_id.clone(), bundle.clone());
        }
    }
    bundles
}

fn keychain_identity_node_ids(node: &KeychainNode) -> bool {
    matches!(node.payload, KeychainNodePayload::Identity(_))
}

fn keychain_ssh_key_node_ids(node: &KeychainNode) -> bool {
    matches!(node.payload, KeychainNodePayload::SshKey(_))
}

fn merge_known_hosts(
    base: &[VaultKnownHostEntry],
    local: &[VaultKnownHostEntry],
    remote: &[VaultKnownHostEntry],
) -> Vec<VaultKnownHostEntry> {
    let mut seen = BTreeSet::new();
    let mut merged = Vec::new();

    for entry in local.iter().chain(remote.iter()).chain(base.iter()) {
        let key = format!("{}\u{0}{}", entry.host_pattern, entry.public_key);
        if seen.insert(key) {
            merged.push(entry.clone());
        }
    }

    merged
}

struct NodeDecision<T, M> {
    node: Option<T>,
    metadata: Option<M>,
    conflict: Option<MergeConflict>,
}

fn decide_asset_node(
    node_id: &str,
    base: Option<&VaultAssetNode>,
    local: Option<&VaultAssetNode>,
    remote: Option<&VaultAssetNode>,
    base_meta: Option<&VaultNodeMergeMetadata>,
    local_meta: Option<&VaultNodeMergeMetadata>,
    remote_meta: Option<&VaultNodeMergeMetadata>,
) -> NodeDecision<VaultAssetNode, VaultNodeMergeMetadata> {
    let base_meta = base_meta.cloned().unwrap_or_default();
    let local_meta = local_meta.cloned().unwrap_or_default();
    let remote_meta = remote_meta.cloned().unwrap_or_default();
    let local_deleted = is_asset_deleted(base, local, &local_meta);
    let remote_deleted = is_asset_deleted(base, remote, &remote_meta);

    match (local, remote, local_deleted, remote_deleted) {
        (Some(local), Some(remote), false, false) if local == remote => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_asset_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), _, false, true) => {
            if base.is_some() && base != Some(local) {
                conflict_asset(node_id, local.clone(), local_meta, "local modified while remote deleted")
            } else {
                NodeDecision {
                    node: None,
                    metadata: Some(remote_meta),
                    conflict: None,
                }
            }
        }
        (_, Some(remote), true, false) => {
            if base.is_some() && base != Some(remote) {
                conflict_asset(node_id, remote.clone(), remote_meta, "local deleted while remote modified")
            } else {
                NodeDecision {
                    node: None,
                    metadata: Some(local_meta),
                    conflict: None,
                }
            }
        }
        (None, None, true, true) => NodeDecision {
            node: None,
            metadata: Some(prefer_deleted_asset_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), None, false, false) => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_asset_metadata(&local_meta, &base_meta, &remote_meta)),
            conflict: None,
        },
        (None, Some(remote), false, false) => NodeDecision {
            node: Some(remote.clone()),
            metadata: Some(prefer_asset_metadata(&remote_meta, &base_meta, &local_meta)),
            conflict: None,
        },
        (Some(local), Some(remote), false, false) if base == Some(local) => NodeDecision {
            node: Some(remote.clone()),
            metadata: Some(prefer_asset_metadata(&remote_meta, &local_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), Some(remote), false, false) if base == Some(remote) => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_asset_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), Some(_remote), false, false) => conflict_asset(
            node_id,
            local.clone(),
            local_meta,
            "local and remote both modified the same asset",
        ),
        _ => NodeDecision {
            node: None,
            metadata: None,
            conflict: None,
        },
    }
}

fn decide_keychain_node(
    node_id: &str,
    base: Option<&KeychainNode>,
    local: Option<&KeychainNode>,
    remote: Option<&KeychainNode>,
    base_meta: Option<&KeychainNodeMergeMetadata>,
    local_meta: Option<&KeychainNodeMergeMetadata>,
    remote_meta: Option<&KeychainNodeMergeMetadata>,
) -> NodeDecision<KeychainNode, KeychainNodeMergeMetadata> {
    let base_meta = base_meta.cloned().unwrap_or_default();
    let local_meta = local_meta.cloned().unwrap_or_default();
    let remote_meta = remote_meta.cloned().unwrap_or_default();
    let local_deleted = is_keychain_deleted(base, local, &local_meta);
    let remote_deleted = is_keychain_deleted(base, remote, &remote_meta);

    match (local, remote, local_deleted, remote_deleted) {
        (Some(local), Some(remote), false, false) if local == remote => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_keychain_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), _, false, true) => {
            if base.is_some() && base != Some(local) {
                conflict_keychain(
                    node_id,
                    local.clone(),
                    local_meta,
                    "local modified while remote deleted",
                )
            } else {
                NodeDecision {
                    node: None,
                    metadata: Some(remote_meta),
                    conflict: None,
                }
            }
        }
        (_, Some(remote), true, false) => {
            if base.is_some() && base != Some(remote) {
                conflict_keychain(
                    node_id,
                    remote.clone(),
                    remote_meta,
                    "local deleted while remote modified",
                )
            } else {
                NodeDecision {
                    node: None,
                    metadata: Some(local_meta),
                    conflict: None,
                }
            }
        }
        (None, None, true, true) => NodeDecision {
            node: None,
            metadata: Some(prefer_deleted_keychain_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), None, false, false) => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_keychain_metadata(&local_meta, &base_meta, &remote_meta)),
            conflict: None,
        },
        (None, Some(remote), false, false) => NodeDecision {
            node: Some(remote.clone()),
            metadata: Some(prefer_keychain_metadata(&remote_meta, &base_meta, &local_meta)),
            conflict: None,
        },
        (Some(local), Some(remote), false, false) if base == Some(local) => NodeDecision {
            node: Some(remote.clone()),
            metadata: Some(prefer_keychain_metadata(&remote_meta, &local_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), Some(remote), false, false) if base == Some(remote) => NodeDecision {
            node: Some(local.clone()),
            metadata: Some(prefer_keychain_metadata(&local_meta, &remote_meta, &base_meta)),
            conflict: None,
        },
        (Some(local), Some(_remote), false, false) => conflict_keychain(
            node_id,
            local.clone(),
            local_meta,
            "local and remote both modified the same keychain node",
        ),
        _ => NodeDecision {
            node: None,
            metadata: None,
            conflict: None,
        },
    }
}

fn conflict_asset(
    node_id: &str,
    node: VaultAssetNode,
    metadata: VaultNodeMergeMetadata,
    message: &str,
) -> NodeDecision<VaultAssetNode, VaultNodeMergeMetadata> {
    NodeDecision {
        node: Some(node),
        metadata: Some(metadata),
        conflict: Some(MergeConflict {
            entity: MergeEntityKind::Asset,
            node_id: node_id.to_string(),
            message: message.to_string(),
        }),
    }
}

fn conflict_keychain(
    node_id: &str,
    node: KeychainNode,
    metadata: KeychainNodeMergeMetadata,
    message: &str,
) -> NodeDecision<KeychainNode, KeychainNodeMergeMetadata> {
    NodeDecision {
        node: Some(node),
        metadata: Some(metadata),
        conflict: Some(MergeConflict {
            entity: MergeEntityKind::KeychainNode,
            node_id: node_id.to_string(),
            message: message.to_string(),
        }),
    }
}

fn is_asset_deleted(
    base: Option<&VaultAssetNode>,
    current: Option<&VaultAssetNode>,
    metadata: &VaultNodeMergeMetadata,
) -> bool {
    current.is_none() && (base.is_some() || metadata.is_deleted())
}

fn is_keychain_deleted(
    base: Option<&KeychainNode>,
    current: Option<&KeychainNode>,
    metadata: &KeychainNodeMergeMetadata,
) -> bool {
    current.is_none() && (base.is_some() || metadata.is_deleted())
}

fn prefer_asset_metadata(
    primary: &VaultNodeMergeMetadata,
    secondary: &VaultNodeMergeMetadata,
    fallback: &VaultNodeMergeMetadata,
) -> VaultNodeMergeMetadata {
    if !is_empty_asset_metadata(primary) {
        primary.clone()
    } else if !is_empty_asset_metadata(secondary) {
        secondary.clone()
    } else {
        fallback.clone()
    }
}

fn prefer_deleted_asset_metadata(
    primary: &VaultNodeMergeMetadata,
    secondary: &VaultNodeMergeMetadata,
    fallback: &VaultNodeMergeMetadata,
) -> VaultNodeMergeMetadata {
    if primary.is_deleted() {
        primary.clone()
    } else if secondary.is_deleted() {
        secondary.clone()
    } else {
        fallback.clone()
    }
}

fn prefer_keychain_metadata(
    primary: &KeychainNodeMergeMetadata,
    secondary: &KeychainNodeMergeMetadata,
    fallback: &KeychainNodeMergeMetadata,
) -> KeychainNodeMergeMetadata {
    if !is_empty_keychain_metadata(primary) {
        primary.clone()
    } else if !is_empty_keychain_metadata(secondary) {
        secondary.clone()
    } else {
        fallback.clone()
    }
}

fn prefer_deleted_keychain_metadata(
    primary: &KeychainNodeMergeMetadata,
    secondary: &KeychainNodeMergeMetadata,
    fallback: &KeychainNodeMergeMetadata,
) -> KeychainNodeMergeMetadata {
    if primary.is_deleted() {
        primary.clone()
    } else if secondary.is_deleted() {
        secondary.clone()
    } else {
        fallback.clone()
    }
}

fn is_empty_asset_metadata(metadata: &VaultNodeMergeMetadata) -> bool {
    metadata.last_modified_at.is_none()
        && metadata.last_modified_by_device.is_none()
        && metadata.deleted_at.is_none()
}

fn is_empty_keychain_metadata(metadata: &KeychainNodeMergeMetadata) -> bool {
    metadata.last_modified_at.is_none()
        && metadata.last_modified_by_device.is_none()
        && metadata.deleted_at.is_none()
}

fn rebuild_asset_relationships(
    mut nodes: BTreeMap<String, VaultAssetNode>,
    local: &VaultAssetCatalog,
    remote: &VaultAssetCatalog,
    base: &VaultAssetCatalog,
) -> (Vec<String>, BTreeMap<String, VaultAssetNode>) {
    let ordered = ordered_ids([
        local.root_ids.clone(),
        remote.root_ids.clone(),
        base.root_ids.clone(),
        local.nodes.keys().cloned().collect(),
        remote.nodes.keys().cloned().collect(),
        base.nodes.keys().cloned().collect(),
    ]);
    rebuild_relationships(
        &mut nodes,
        ordered,
        |node| node.parent_id.clone(),
        |node, children| node.child_ids = children,
    )
}

fn rebuild_keychain_relationships(
    mut nodes: BTreeMap<String, KeychainNode>,
    local: &KeychainCatalog,
    remote: &KeychainCatalog,
    base: &KeychainCatalog,
) -> (Vec<String>, BTreeMap<String, KeychainNode>) {
    let ordered = ordered_ids([
        local.root_ids.clone(),
        remote.root_ids.clone(),
        base.root_ids.clone(),
        local.nodes.keys().cloned().collect(),
        remote.nodes.keys().cloned().collect(),
        base.nodes.keys().cloned().collect(),
    ]);
    rebuild_relationships(
        &mut nodes,
        ordered,
        |node| node.parent_id.clone(),
        |node, children| node.child_ids = children,
    )
}

fn rebuild_relationships<T>(
    nodes: &mut BTreeMap<String, T>,
    ordered: Vec<String>,
    parent_of: fn(&T) -> Option<String>,
    set_children: fn(&mut T, Vec<String>),
) -> (Vec<String>, BTreeMap<String, T>)
where
    T: Clone,
{
    let mut child_map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut root_ids = Vec::new();
    let available = nodes.keys().cloned().collect::<BTreeSet<_>>();

    for node_id in &ordered {
        let Some(node) = nodes.get(node_id) else {
            continue;
        };
        let parent_id = parent_of(node);
        if let Some(parent_id) = parent_id.filter(|parent_id| available.contains(parent_id)) {
            child_map.entry(parent_id).or_default().push(node_id.clone());
        } else {
            root_ids.push(node_id.clone());
        }
    }

    for (node_id, node) in nodes.iter_mut() {
        set_children(node, child_map.remove(node_id).unwrap_or_default());
    }

    (root_ids, nodes.clone())
}

fn ordered_ids<const N: usize>(groups: [Vec<String>; N]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for group in groups {
        for node_id in group {
            if seen.insert(node_id.clone()) {
                ordered.push(node_id);
            }
        }
    }
    ordered
}
