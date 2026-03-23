use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetKind, PersistedAssetPayload,
    PersistedSshConnectionSpec, asset_tree_to_catalog, catalog_to_asset_tree,
};
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetTree, ConsoleAssetKind,
};

#[test]
fn persisted_catalog_round_trips_tree_order_and_node_kind() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    let ssh_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            environment: "prod".into(),
            proxy_method: "none".into(),
        }),
    );
    let child_id = tree.insert_child_with_payload(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.13".into(),
            user: "root".into(),
            port: "2222".into(),
            environment: "ops".into(),
            proxy_method: "jump".into(),
        }),
    );

    let catalog = asset_tree_to_catalog(&tree);

    assert_eq!(catalog.schema_version, ASSET_CATALOG_SCHEMA_VERSION);
    assert_eq!(catalog.root_ids, vec![folder_id.clone(), ssh_id.clone()]);
    assert_eq!(
        catalog.nodes.get(&folder_id).unwrap().kind,
        PersistedAssetKind::Folder
    );
    assert_eq!(
        catalog.nodes.get(&folder_id).unwrap().child_ids,
        vec![child_id.clone()]
    );
    assert_eq!(
        catalog.nodes.get(&ssh_id).unwrap().kind,
        PersistedAssetKind::SshConnection
    );

    let round_tripped = catalog_to_asset_tree(&catalog);
    assert_eq!(
        round_tripped.root_ids(),
        &[folder_id.clone(), ssh_id.clone()]
    );
    assert_eq!(
        round_tripped.node(&folder_id).unwrap().kind,
        ConsoleAssetKind::Folder
    );
    assert_eq!(
        round_tripped.node(&folder_id).unwrap().children,
        vec![child_id.clone()]
    );
    assert_eq!(
        round_tripped.node(&ssh_id).unwrap().kind,
        ConsoleAssetKind::SshConnection
    );
}

#[test]
fn persisted_catalog_preserves_ssh_connection_fields() {
    let mut tree = AssetTree::new();
    let ssh_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            environment: "prod".into(),
            proxy_method: "jump-host".into(),
        }),
    );

    let catalog = asset_tree_to_catalog(&tree);
    let persisted = catalog.nodes.get(&ssh_id).unwrap();

    assert_eq!(
        persisted.payload,
        PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            environment: "prod".into(),
            proxy_method: "jump-host".into(),
        })
    );

    let round_tripped = catalog_to_asset_tree(&catalog);
    assert_eq!(
        round_tripped.ssh_connection_spec(&ssh_id),
        Some(&AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            environment: "prod".into(),
            proxy_method: "jump-host".into(),
        })
    );
}

#[test]
fn persisted_catalog_excludes_ui_session_state() {
    let mut tree = AssetTree::new();
    let folder_id = tree.insert_root(ConsoleAssetKind::Folder, "Team");
    tree.insert_child_with_payload(
        &folder_id,
        ConsoleAssetKind::SshConnection,
        "Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            environment: "".into(),
            proxy_method: "".into(),
        }),
    );
    tree.set_expanded(&folder_id, true);

    let catalog = asset_tree_to_catalog(&tree);
    let round_tripped = catalog_to_asset_tree(&catalog);

    assert_eq!(tree.is_expanded(&folder_id), Some(true));
    assert_eq!(round_tripped.is_expanded(&folder_id), Some(false));
}

#[test]
fn empty_catalog_maps_to_empty_runtime_tree() {
    let tree = catalog_to_asset_tree(&mica_term::app::assets_catalog::PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: Vec::new(),
        nodes: Default::default(),
    });

    assert!(tree.root_ids().is_empty());
}
