//! Maps between the persisted asset catalog schema and the runtime asset tree.

use std::collections::{BTreeMap, HashMap};

use crate::app::assets_catalog::model::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetCatalog, PersistedAssetKind, PersistedAssetNode,
    PersistedAssetPayload, PersistedSshConnectionSpec,
};
use crate::shell::assets::{
    AssetNode, AssetNodePayload, AssetSshConnectionSpec, AssetTree, ConsoleAssetKind,
};

pub fn catalog_to_asset_tree(catalog: &PersistedAssetCatalog) -> AssetTree {
    let nodes = catalog
        .nodes
        .iter()
        .map(|(id, node)| {
            (
                id.clone(),
                AssetNode {
                    id: node.id.clone(),
                    kind: runtime_kind(node.kind),
                    title: node.title.clone(),
                    parent_id: node.parent_id.clone(),
                    children: node.child_ids.clone(),
                    expanded: false,
                    payload: runtime_payload(&node.payload),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    AssetTree::from_parts(catalog.root_ids.clone(), nodes)
}

pub fn asset_tree_to_catalog(tree: &AssetTree) -> PersistedAssetCatalog {
    let mut nodes = BTreeMap::new();
    collect_catalog_nodes(tree, tree.root_ids(), &mut nodes);

    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: tree.root_ids().to_vec(),
        nodes,
    }
}

fn collect_catalog_nodes(
    tree: &AssetTree,
    node_ids: &[String],
    output: &mut BTreeMap<String, PersistedAssetNode>,
) {
    for node_id in node_ids {
        let Some(node) = tree.node(node_id) else {
            continue;
        };

        output.insert(
            node.id.clone(),
            PersistedAssetNode {
                id: node.id.clone(),
                parent_id: node.parent_id.clone(),
                title: node.title.clone(),
                kind: persisted_kind(node.kind),
                child_ids: node.children.clone(),
                payload: persisted_payload(node),
            },
        );

        collect_catalog_nodes(tree, &node.children, output);
    }
}

fn runtime_kind(kind: PersistedAssetKind) -> ConsoleAssetKind {
    match kind {
        PersistedAssetKind::Folder => ConsoleAssetKind::Folder,
        PersistedAssetKind::SshConnection => ConsoleAssetKind::SshConnection,
    }
}

fn persisted_kind(kind: ConsoleAssetKind) -> PersistedAssetKind {
    match kind {
        ConsoleAssetKind::Folder => PersistedAssetKind::Folder,
        ConsoleAssetKind::SshConnection => PersistedAssetKind::SshConnection,
    }
}

fn runtime_payload(payload: &PersistedAssetPayload) -> AssetNodePayload {
    match payload {
        PersistedAssetPayload::Folder => AssetNodePayload::Folder,
        PersistedAssetPayload::SshConnection(spec) => {
            AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                host: spec.host.clone(),
                user: spec.user.clone(),
                port: spec.port.clone(),
                environment: spec.environment.clone(),
                proxy_method: spec.proxy_method.clone(),
            })
        }
    }
}

fn persisted_payload(node: &AssetNode) -> PersistedAssetPayload {
    match (&node.kind, &node.payload) {
        (ConsoleAssetKind::Folder, _) => PersistedAssetPayload::Folder,
        (ConsoleAssetKind::SshConnection, AssetNodePayload::SshConnection(spec)) => {
            PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                host: spec.host.clone(),
                user: spec.user.clone(),
                port: spec.port.clone(),
                environment: spec.environment.clone(),
                proxy_method: spec.proxy_method.clone(),
            })
        }
        (ConsoleAssetKind::SshConnection, AssetNodePayload::Folder) => {
            unreachable!("ssh runtime nodes must carry ssh payload")
        }
    }
}
