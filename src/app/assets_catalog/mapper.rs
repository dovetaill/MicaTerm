//! Maps between the persisted asset catalog schema and the runtime asset tree.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::app::assets_catalog::model::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetCatalog, PersistedAssetDomain,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedAssetSocks5ProxySpec,
    PersistedAssetSshProxySpec, PersistedSnippetSpec, PersistedSshConnectionSpec,
};
use crate::app::vault::model::{
    VaultAssetCatalog, VaultAssetKind, VaultAssetNode, VaultAssetPayload, VaultSocks5ProxySpec,
    VaultSnippetSpec, VaultSshConnectionSpec, VaultSshProxySpec,
};
use crate::shell::assets::{
    AssetNode, AssetNodePayload, AssetSnippetSpec, AssetSocks5ProxySpec,
    AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
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

pub fn catalog_to_asset_trees(catalog: &PersistedAssetCatalog) -> (AssetTree, AssetTree) {
    let console_catalog = filter_catalog_by_domain(catalog, PersistedAssetDomain::Console);
    let snippet_catalog = filter_catalog_by_domain(catalog, PersistedAssetDomain::Snippets);
    (
        catalog_to_asset_tree(&console_catalog),
        catalog_to_asset_tree(&snippet_catalog),
    )
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

pub fn asset_trees_to_catalog(
    console_tree: &AssetTree,
    snippet_tree: &AssetTree,
) -> PersistedAssetCatalog {
    let console_catalog = asset_tree_to_catalog(console_tree);
    let snippet_catalog = remap_snippet_catalog_ids(
        asset_tree_to_catalog(snippet_tree),
        console_catalog.nodes.keys().cloned().collect(),
    );

    let mut root_ids = console_catalog.root_ids.clone();
    root_ids.extend(snippet_catalog.root_ids.clone());

    let mut nodes = console_catalog.nodes.clone();
    nodes.extend(snippet_catalog.nodes);

    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids,
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

fn filter_catalog_by_domain(
    catalog: &PersistedAssetCatalog,
    domain: PersistedAssetDomain,
) -> PersistedAssetCatalog {
    let allowed_ids = catalog
        .nodes
        .iter()
        .filter_map(|(id, node)| (node.kind.domain() == domain).then_some(id.clone()))
        .collect::<HashSet<_>>();
    let root_ids = catalog
        .root_ids
        .iter()
        .filter(|id| allowed_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let nodes = catalog
        .nodes
        .iter()
        .filter(|(_, node)| node.kind.domain() == domain)
        .map(|(id, node)| {
            let mut filtered = node.clone();
            filtered.parent_id = filtered
                .parent_id
                .filter(|parent_id| allowed_ids.contains(parent_id));
            filtered.child_ids = filtered
                .child_ids
                .iter()
                .filter(|child_id| allowed_ids.contains(*child_id))
                .cloned()
                .collect();
            (id.clone(), filtered)
        })
        .collect::<BTreeMap<_, _>>();

    PersistedAssetCatalog {
        schema_version: catalog.schema_version,
        root_ids,
        nodes,
    }
}

fn remap_snippet_catalog_ids(
    catalog: PersistedAssetCatalog,
    reserved_ids: HashSet<String>,
) -> PersistedAssetCatalog {
    let mut used_ids = reserved_ids;
    let mut id_map = HashMap::new();

    for node_id in catalog_node_order(&catalog) {
        let remapped = if used_ids.contains(&node_id) {
            next_available_snippet_id(&node_id, &used_ids)
        } else {
            node_id.clone()
        };
        used_ids.insert(remapped.clone());
        id_map.insert(node_id, remapped);
    }

    let root_ids = catalog
        .root_ids
        .into_iter()
        .map(|root_id| remap_id(&root_id, &id_map))
        .collect::<Vec<_>>();
    let nodes = catalog
        .nodes
        .into_iter()
        .map(|(node_id, node)| {
            let remapped_id = remap_id(&node_id, &id_map);
            (
                remapped_id.clone(),
                PersistedAssetNode {
                    id: remapped_id,
                    parent_id: node.parent_id.map(|parent_id| remap_id(&parent_id, &id_map)),
                    title: node.title,
                    kind: node.kind,
                    child_ids: node
                        .child_ids
                        .into_iter()
                        .map(|child_id| remap_id(&child_id, &id_map))
                        .collect(),
                    payload: remap_persisted_payload_ids(node.payload, &id_map),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    PersistedAssetCatalog {
        schema_version: catalog.schema_version,
        root_ids,
        nodes,
    }
}

fn catalog_node_order(catalog: &PersistedAssetCatalog) -> Vec<String> {
    let mut order = Vec::new();
    let mut seen = HashSet::new();
    collect_catalog_node_order(catalog, &catalog.root_ids, &mut seen, &mut order);
    for node_id in catalog.nodes.keys() {
        if seen.insert(node_id.clone()) {
            order.push(node_id.clone());
        }
    }
    order
}

fn collect_catalog_node_order(
    catalog: &PersistedAssetCatalog,
    node_ids: &[String],
    seen: &mut HashSet<String>,
    order: &mut Vec<String>,
) {
    for node_id in node_ids {
        if !seen.insert(node_id.clone()) {
            continue;
        }
        order.push(node_id.clone());
        if let Some(node) = catalog.nodes.get(node_id) {
            collect_catalog_node_order(catalog, &node.child_ids, seen, order);
        }
    }
}

fn next_available_snippet_id(base_id: &str, used_ids: &HashSet<String>) -> String {
    let normalized = if base_id.starts_with("snippet-") {
        base_id.to_string()
    } else {
        format!("snippet-{base_id}")
    };
    if !used_ids.contains(&normalized) {
        return normalized;
    }

    let mut suffix = 2_u64;
    loop {
        let candidate = format!("{normalized}-{suffix}");
        if !used_ids.contains(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

fn remap_id(id: &str, id_map: &HashMap<String, String>) -> String {
    id_map.get(id).cloned().unwrap_or_else(|| id.to_string())
}

fn remap_persisted_payload_ids(
    payload: PersistedAssetPayload,
    id_map: &HashMap<String, String>,
) -> PersistedAssetPayload {
    match payload {
        PersistedAssetPayload::Snippet(mut spec) => {
            spec.package_id = spec.package_id.map(|package_id| remap_id(&package_id, id_map));
            PersistedAssetPayload::Snippet(spec)
        }
        other => other,
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
        PersistedAssetKind::SnippetPackage => ConsoleAssetKind::SnippetPackage,
        PersistedAssetKind::Snippet => ConsoleAssetKind::Snippet,
    }
}

fn persisted_kind(kind: ConsoleAssetKind) -> PersistedAssetKind {
    match kind {
        ConsoleAssetKind::Folder => PersistedAssetKind::Folder,
        ConsoleAssetKind::SshConnection => PersistedAssetKind::SshConnection,
        ConsoleAssetKind::SnippetPackage => PersistedAssetKind::SnippetPackage,
        ConsoleAssetKind::Snippet => PersistedAssetKind::Snippet,
    }
}

fn runtime_vault_kind(kind: VaultAssetKind) -> ConsoleAssetKind {
    match kind {
        VaultAssetKind::Folder => ConsoleAssetKind::Folder,
        VaultAssetKind::SshConnection => ConsoleAssetKind::SshConnection,
        VaultAssetKind::SnippetPackage => ConsoleAssetKind::SnippetPackage,
        VaultAssetKind::Snippet => ConsoleAssetKind::Snippet,
    }
}

fn persisted_vault_kind(kind: ConsoleAssetKind) -> VaultAssetKind {
    match kind {
        ConsoleAssetKind::Folder => VaultAssetKind::Folder,
        ConsoleAssetKind::SshConnection => VaultAssetKind::SshConnection,
        ConsoleAssetKind::SnippetPackage => VaultAssetKind::SnippetPackage,
        ConsoleAssetKind::Snippet => VaultAssetKind::Snippet,
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
                auth_source: spec.auth_source.clone(),
                keychain_identity_id: spec.keychain_identity_id.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: runtime_proxy(&spec.proxy),
                proxy_method: String::new(),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
            })
        }
        PersistedAssetPayload::SnippetPackage => AssetNodePayload::SnippetPackage,
        PersistedAssetPayload::Snippet(spec) => AssetNodePayload::Snippet(AssetSnippetSpec {
            script: spec.script.clone(),
            package_id: spec.package_id.clone(),
        }),
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
                auth_source: spec.auth_source.clone(),
                keychain_identity_id: spec.keychain_identity_id.clone(),
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
        (ConsoleAssetKind::SshConnection, AssetNodePayload::SnippetPackage)
        | (ConsoleAssetKind::SshConnection, AssetNodePayload::Snippet(_)) => {
            unreachable!("ssh runtime nodes must not carry snippet payload")
        }
        (ConsoleAssetKind::SnippetPackage, _) => PersistedAssetPayload::SnippetPackage,
        (ConsoleAssetKind::Snippet, AssetNodePayload::Snippet(spec)) => {
            PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                script: spec.script.clone(),
                package_id: spec.package_id.clone(),
            })
        }
        (ConsoleAssetKind::Snippet, AssetNodePayload::Folder)
        | (ConsoleAssetKind::Snippet, AssetNodePayload::SshConnection(_))
        | (ConsoleAssetKind::Snippet, AssetNodePayload::SnippetPackage) => {
            unreachable!("snippet runtime nodes must carry snippet payload")
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
                auth_source: spec.auth_source.clone(),
                keychain_identity_id: spec.keychain_identity_id.clone(),
                private_key_source: spec.private_key_source.clone(),
                private_key_path: spec.private_key_path.clone(),
                environment: spec.environment.clone(),
                proxy: runtime_vault_proxy(&spec.proxy),
                proxy_method: String::new(),
                remark: spec.remark.clone(),
                credential_ref: spec.credential_ref.clone(),
            })
        }
        VaultAssetPayload::SnippetPackage => AssetNodePayload::SnippetPackage,
        VaultAssetPayload::Snippet(spec) => AssetNodePayload::Snippet(AssetSnippetSpec {
            script: spec.script.clone(),
            package_id: spec.package_id.clone(),
        }),
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
                auth_source: spec.auth_source.clone(),
                keychain_identity_id: spec.keychain_identity_id.clone(),
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
        (ConsoleAssetKind::SshConnection, AssetNodePayload::SnippetPackage)
        | (ConsoleAssetKind::SshConnection, AssetNodePayload::Snippet(_)) => {
            unreachable!("ssh runtime nodes must not carry snippet payload")
        }
        (ConsoleAssetKind::SnippetPackage, _) => VaultAssetPayload::SnippetPackage,
        (ConsoleAssetKind::Snippet, AssetNodePayload::Snippet(spec)) => {
            VaultAssetPayload::Snippet(VaultSnippetSpec {
                script: spec.script.clone(),
                package_id: spec.package_id.clone(),
            })
        }
        (ConsoleAssetKind::Snippet, AssetNodePayload::Folder)
        | (ConsoleAssetKind::Snippet, AssetNodePayload::SshConnection(_))
        | (ConsoleAssetKind::Snippet, AssetNodePayload::SnippetPackage) => {
            unreachable!("snippet runtime nodes must carry snippet payload")
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
        PersistedAssetSshProxySpec::Http(spec) => AssetSshProxySpec::Http(AssetSocks5ProxySpec {
            host: spec.host.clone(),
            port: spec.port.clone(),
            username: spec.username.clone(),
            password_credential_ref: spec.password_credential_ref.clone(),
        }),
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
        AssetSshProxySpec::Http(spec) => {
            PersistedAssetSshProxySpec::Http(PersistedAssetSocks5ProxySpec {
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
        VaultSshProxySpec::Http(spec) => AssetSshProxySpec::Http(AssetSocks5ProxySpec {
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
        AssetSshProxySpec::Http(spec) => VaultSshProxySpec::Http(VaultSocks5ProxySpec {
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
