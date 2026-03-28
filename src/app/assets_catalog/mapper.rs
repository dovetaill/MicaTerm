//! Maps between the persisted asset catalog schema and the runtime asset tree.

use std::collections::{BTreeMap, HashMap};

use crate::app::assets_catalog::model::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetCatalog, PersistedAssetKind, PersistedAssetNode,
    PersistedAssetPayload, PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec,
    PersistedSshConnectionSpec,
};
use crate::app::vault::model::{
    VaultAssetCatalog, VaultAssetKind, VaultAssetNode, VaultAssetPayload, VaultSocks5ProxySpec,
    VaultSshConnectionSpec, VaultSshProxySpec,
};
use crate::shell::assets::{
    AssetNode, AssetNodePayload, AssetSocks5ProxySpec, AssetSshConnectionSpec, AssetSshProxySpec,
    AssetTree, ConsoleAssetKind,
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

pub fn vault_catalog_to_asset_tree(catalog: &VaultAssetCatalog) -> AssetTree {
    let nodes = catalog
        .nodes
        .iter()
        .map(|(id, node)| {
            (
                id.clone(),
                AssetNode {
                    id: node.id.clone(),
                    kind: runtime_vault_kind(node.kind),
                    title: node.title.clone(),
                    parent_id: node.parent_id.clone(),
                    children: node.child_ids.clone(),
                    expanded: false,
                    payload: runtime_vault_payload(&node.payload),
                },
            )
        })
        .collect::<HashMap<_, _>>();

    AssetTree::from_parts(catalog.root_ids.clone(), nodes)
}

pub fn asset_tree_to_vault_catalog(tree: &AssetTree) -> VaultAssetCatalog {
    let mut nodes = BTreeMap::new();
    collect_vault_nodes(tree, tree.root_ids(), &mut nodes);

    VaultAssetCatalog {
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

fn collect_vault_nodes(
    tree: &AssetTree,
    node_ids: &[String],
    output: &mut BTreeMap<String, VaultAssetNode>,
) {
    for node_id in node_ids {
        let Some(node) = tree.node(node_id) else {
            continue;
        };

        output.insert(
            node.id.clone(),
            VaultAssetNode {
                id: node.id.clone(),
                parent_id: node.parent_id.clone(),
                title: node.title.clone(),
                kind: persisted_vault_kind(node.kind),
                child_ids: node.children.clone(),
                payload: persisted_vault_payload(node),
            },
        );

        collect_vault_nodes(tree, &node.children, output);
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

fn runtime_vault_kind(kind: VaultAssetKind) -> ConsoleAssetKind {
    match kind {
        VaultAssetKind::Folder => ConsoleAssetKind::Folder,
        VaultAssetKind::SshConnection => ConsoleAssetKind::SshConnection,
    }
}

fn persisted_vault_kind(kind: ConsoleAssetKind) -> VaultAssetKind {
    match kind {
        ConsoleAssetKind::Folder => VaultAssetKind::Folder,
        ConsoleAssetKind::SshConnection => VaultAssetKind::SshConnection,
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
                auth_method: spec.auth_method.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: runtime_proxy(&spec.proxy),
                proxy_method: String::new(),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
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
                auth_method: spec.auth_method.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: persisted_proxy(&spec.proxy),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
            })
        }
        (ConsoleAssetKind::SshConnection, AssetNodePayload::Folder) => {
            unreachable!("ssh runtime nodes must carry ssh payload")
        }
    }
}

fn runtime_vault_payload(payload: &VaultAssetPayload) -> AssetNodePayload {
    match payload {
        VaultAssetPayload::Folder => AssetNodePayload::Folder,
        VaultAssetPayload::SshConnection(spec) => {
            AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                host: spec.host.clone(),
                user: spec.user.clone(),
                port: spec.port.clone(),
                auth_method: spec.auth_method.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: runtime_vault_proxy(&spec.proxy),
                proxy_method: String::new(),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
            })
        }
    }
}

fn persisted_vault_payload(node: &AssetNode) -> VaultAssetPayload {
    match (&node.kind, &node.payload) {
        (ConsoleAssetKind::Folder, _) => VaultAssetPayload::Folder,
        (ConsoleAssetKind::SshConnection, AssetNodePayload::SshConnection(spec)) => {
            VaultAssetPayload::SshConnection(Box::new(VaultSshConnectionSpec {
                host: spec.host.clone(),
                user: spec.user.clone(),
                port: spec.port.clone(),
                auth_method: spec.auth_method.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: persisted_vault_proxy(&spec.proxy),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
            }))
        }
        (ConsoleAssetKind::SshConnection, AssetNodePayload::Folder) => {
            unreachable!("ssh runtime nodes must carry ssh payload")
        }
    }
}

fn runtime_proxy(proxy: &PersistedAssetSshProxySpec) -> AssetSshProxySpec {
    match proxy {
        PersistedAssetSshProxySpec::None => AssetSshProxySpec::None,
        PersistedAssetSshProxySpec::Socks5(spec) => {
            AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: spec.host.clone(),
                port: spec.port.clone(),
                username: spec.username.clone(),
                password_credential_ref: spec.password_credential_ref.clone(),
            })
        }
        PersistedAssetSshProxySpec::SshAsset { asset_id } => AssetSshProxySpec::SshAsset {
            asset_id: asset_id.clone(),
        },
    }
}

fn persisted_proxy(proxy: &AssetSshProxySpec) -> PersistedAssetSshProxySpec {
    match proxy {
        AssetSshProxySpec::None => PersistedAssetSshProxySpec::None,
        AssetSshProxySpec::Socks5(spec) => {
            PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                host: spec.host.clone(),
                port: spec.port.clone(),
                username: spec.username.clone(),
                password_credential_ref: spec.password_credential_ref.clone(),
            })
        }
        AssetSshProxySpec::SshAsset { asset_id } => PersistedAssetSshProxySpec::SshAsset {
            asset_id: asset_id.clone(),
        },
    }
}

fn runtime_vault_proxy(proxy: &VaultSshProxySpec) -> AssetSshProxySpec {
    match proxy {
        VaultSshProxySpec::None => AssetSshProxySpec::None,
        VaultSshProxySpec::Socks5(spec) => AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
            host: spec.host.clone(),
            port: spec.port.clone(),
            username: spec.username.clone(),
            password_credential_ref: spec.password_credential_ref.clone(),
        }),
        VaultSshProxySpec::SshAsset { asset_id } => AssetSshProxySpec::SshAsset {
            asset_id: asset_id.clone(),
        },
    }
}

fn persisted_vault_proxy(proxy: &AssetSshProxySpec) -> VaultSshProxySpec {
    match proxy {
        AssetSshProxySpec::None => VaultSshProxySpec::None,
        AssetSshProxySpec::Socks5(spec) => VaultSshProxySpec::Socks5(VaultSocks5ProxySpec {
            host: spec.host.clone(),
            port: spec.port.clone(),
            username: spec.username.clone(),
            password_credential_ref: spec.password_credential_ref.clone(),
        }),
        AssetSshProxySpec::SshAsset { asset_id } => VaultSshProxySpec::SshAsset {
            asset_id: asset_id.clone(),
        },
    }
}
