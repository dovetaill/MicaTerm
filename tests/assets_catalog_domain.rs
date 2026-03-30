use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, PersistedAssetKind, PersistedAssetPayload,
    PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec, PersistedSshConnectionSpec,
    asset_tree_to_catalog, catalog_to_asset_tree,
};
use mica_term::shell::assets::{
    AssetNodePayload, AssetSocks5ProxySpec, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree,
    ConsoleAssetKind,
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
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
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
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "ops".into(),
            proxy: AssetSshProxySpec::SshAsset {
                asset_id: ssh_id.clone(),
            },
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
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
fn ssh_connection_spec_defaults_to_no_proxy() {
    assert_eq!(
        AssetSshConnectionSpec::default().proxy,
        AssetSshProxySpec::None
    );
}

#[test]
fn persisted_catalog_preserves_socks5_proxy_fields() {
    let mut tree = AssetTree::new();
    let ssh_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
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
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            remark: String::new(),
            credential_ref: None,
        })
    );

    let round_tripped = catalog_to_asset_tree(&catalog);
    assert_eq!(
        round_tripped.ssh_connection_spec(&ssh_id),
        Some(&AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
        })
    );
}

#[test]
fn persisted_catalog_preserves_http_proxy_fields() {
    let mut tree = AssetTree::new();
    let ssh_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Http(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "8080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
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
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: PersistedAssetSshProxySpec::Http(PersistedAssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "8080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            remark: String::new(),
            credential_ref: None,
        })
    );

    let round_tripped = catalog_to_asset_tree(&catalog);
    assert_eq!(
        round_tripped.ssh_connection_spec(&ssh_id),
        Some(&AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Http(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "8080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-a".into()),
            }),
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
        })
    );
}

#[test]
fn persisted_ssh_connection_spec_round_trips_ssh_upstream_proxy_reference() {
    let mut tree = AssetTree::new();
    let ssh_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::SshAsset {
                asset_id: "asset-upstream".into(),
            },
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/private-key/asset-gateway".into()),
        }),
    );

    let catalog = asset_tree_to_catalog(&tree);
    let persisted = catalog.nodes.get(&ssh_id).expect("persisted ssh node");

    assert_eq!(
        persisted.payload,
        PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: "prod".into(),
            proxy: PersistedAssetSshProxySpec::SshAsset {
                asset_id: "asset-upstream".into(),
            },
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/private-key/asset-gateway".into()),
        })
    );

    let round_tripped = catalog_to_asset_tree(&catalog);
    assert_eq!(
        round_tripped.ssh_connection_spec(&ssh_id),
        Some(&AssetSshConnectionSpec {
            host: "gateway.example.com".into(),
            user: "mica".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::SshAsset {
                asset_id: "asset-upstream".into(),
            },
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/private-key/asset-gateway".into()),
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
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
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
