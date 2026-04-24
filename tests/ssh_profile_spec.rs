use std::collections::HashMap;

use mica_term::app::ssh::profile::{
    ConnectionProfile, ConnectionProxyProfile, ResolvedProxyHop, SshAuthMethod,
};
use mica_term::app::ssh::proxy::resolve_proxy_chain;
use mica_term::shell::assets::{
    AssetNode, AssetNodePayload, AssetSocks5ProxySpec, AssetSshConnectionSpec, AssetSshProxySpec,
    AssetTree, ConsoleAssetKind,
};
use mica_term::shell::view_model::AssetSshConnectionDraft;

fn base_draft() -> AssetSshConnectionDraft {
    AssetSshConnectionDraft {
        name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: "2022".into(),
        remark: "Primary entry point".into(),
        ..AssetSshConnectionDraft::default()
    }
}

fn saved_password_spec(proxy: AssetSshProxySpec) -> AssetSshConnectionSpec {
    AssetSshConnectionSpec {
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: "2022".into(),
        auth_method: "password".into(),
        auth_source: "manual".into(),
        keychain_identity_id: None,
        private_key_source: "content".into(),
        private_key_path: String::new(),
        environment: "prod".into(),
        proxy,
        proxy_method: String::new(),
        remark: "Primary entry point".into(),
        credential_ref: Some("ssh/password/asset".into()),
    }
}

fn saved_ssh_node(id: &str, title: &str, spec: AssetSshConnectionSpec) -> (String, AssetNode) {
    (
        id.to_string(),
        AssetNode {
            id: id.to_string(),
            kind: ConsoleAssetKind::SshConnection,
            title: title.to_string(),
            parent_id: None,
            children: Vec::new(),
            expanded: false,
            payload: AssetNodePayload::SshConnection(spec),
        },
    )
}

fn saved_ssh_tree(nodes: Vec<(&str, &str, AssetSshConnectionSpec)>) -> AssetTree {
    let root_ids = nodes
        .iter()
        .map(|(id, _, _)| (*id).to_string())
        .collect::<Vec<_>>();
    let nodes = nodes
        .into_iter()
        .map(|(id, title, spec)| saved_ssh_node(id, title, spec))
        .collect::<HashMap<_, _>>();
    AssetTree::from_parts(root_ids, nodes)
}

#[test]
fn ssh_profile_normalizes_password_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();
    draft.password_visible = true;
    draft.passphrase_visible = true;
    draft.proxy_socks5_password_visible = true;

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize password draft");

    assert_eq!(profile.asset_id, None);
    assert_eq!(profile.name, "Prod Bastion");
    assert_eq!(profile.host, "10.0.0.12");
    assert_eq!(profile.user, "ops");
    assert_eq!(profile.port, 2022);
    assert_eq!(profile.auth_method, SshAuthMethod::Password);
    assert!(profile.credential_ref.is_some());
    assert_eq!(profile.private_key_path, None);
    assert_eq!(profile.remark, "Primary entry point");
}

#[test]
fn ssh_profile_normalizes_private_key_path_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "private-key".into();
    draft.private_key_source = "path".into();
    draft.private_key_path = "/tmp/id_ed25519".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize key path draft");

    assert_eq!(profile.auth_method, SshAuthMethod::PrivateKeyPath);
    assert_eq!(profile.credential_ref, None);
    assert_eq!(profile.private_key_path.as_deref(), Some("/tmp/id_ed25519"));
    assert_eq!(profile.port, 2022);
}

#[test]
fn ssh_profile_normalizes_private_key_content_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "private-key".into();
    draft.private_key_source = "content".into();
    draft.private_key_content = "-----BEGIN OPENSSH PRIVATE KEY-----".into();
    draft.passphrase = "phrase".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize inline key draft");

    assert_eq!(profile.auth_method, SshAuthMethod::PrivateKeyContent);
    assert!(profile.credential_ref.is_some());
    assert_eq!(profile.private_key_path, None);
    assert_eq!(profile.remark, "Primary entry point");
}

#[test]
fn ssh_profile_normalizes_socks5_proxy_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();
    draft.proxy_type = "socks5".into();
    draft.proxy_socks5_host = "proxy.example.net".into();
    draft.proxy_socks5_port = "1080".into();
    draft.proxy_socks5_username = "ops-proxy".into();
    draft.proxy_socks5_password = "proxy-secret".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize socks5 proxy draft");

    assert_eq!(
        profile.proxy,
        ConnectionProxyProfile::Socks5 {
            host: "proxy.example.net".into(),
            port: 1080,
            username: Some("ops-proxy".into()),
            password: Some("proxy-secret".into()),
            credential_ref: None,
        }
    );
    assert!(profile.resolved_proxy_hops.is_empty());
}

#[test]
fn ssh_profile_can_be_built_from_saved_asset_and_credential_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: "".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: "jump-host".into(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/password/asset-prod".into()),
        },
    )
    .expect("build saved ssh profile");

    assert_eq!(profile.asset_id.as_deref(), Some("asset-prod"));
    assert_eq!(profile.name, "Prod Bastion");
    assert_eq!(profile.host, "10.0.0.12");
    assert_eq!(profile.user, "ops");
    assert_eq!(profile.port, 2022);
    assert_eq!(profile.auth_method, SshAuthMethod::Password);
    assert_eq!(
        profile.credential_ref.as_deref(),
        Some("ssh/password/asset-prod")
    );
    assert_eq!(profile.private_key_path, None);
    assert_eq!(profile.remark, "Primary entry point");
}

#[test]
fn ssh_profile_saved_manual_asset_accepts_explicit_manual_auth_source() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: "".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: "jump-host".into(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/password/asset-prod".into()),
        },
    )
    .expect("build saved ssh profile with explicit manual auth source");

    assert_eq!(profile.asset_id.as_deref(), Some("asset-prod"));
    assert_eq!(profile.user, "ops");
    assert_eq!(profile.auth_method, SshAuthMethod::Password);
    assert_eq!(
        profile.credential_ref.as_deref(),
        Some("ssh/password/asset-prod")
    );
}

#[test]
fn ssh_profile_preserves_saved_upstream_ssh_proxy_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: "".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::SshAsset {
                asset_id: "asset-upstream".into(),
            },
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/password/asset-prod".into()),
        },
    )
    .expect("build saved upstream proxy profile");

    assert_eq!(
        profile.proxy,
        ConnectionProxyProfile::SshAsset {
            asset_id: "asset-upstream".into(),
        }
    );
    assert!(profile.resolved_proxy_hops.is_empty());
}

#[test]
fn ssh_profile_private_key_path_saved_asset_preserves_saved_credential_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: "jump-host".into(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        },
    )
    .expect("build saved private key path profile");

    assert_eq!(profile.auth_method, SshAuthMethod::PrivateKeyPath);
    assert_eq!(
        profile.credential_ref.as_deref(),
        Some("ssh/saved-secrets/asset-prod")
    );
    assert_eq!(profile.private_key_path.as_deref(), Some("/tmp/id_ed25519"));
}

#[test]
fn ssh_profile_rejects_invalid_socks5_proxy_port_in_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();
    draft.proxy_type = "socks5".into();
    draft.proxy_socks5_host = "proxy.example.net".into();
    draft.proxy_socks5_port = "not-a-port".into();

    let err = ConnectionProfile::from_draft(&draft).expect_err("reject invalid socks5 port");

    assert!(err.to_string().contains("invalid socks5 proxy port"));
}

#[test]
fn ssh_profile_normalizes_saved_socks5_proxy_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: "".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
            }),
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/password/asset-prod".into()),
        },
    )
    .expect("build saved socks5 proxy profile");

    assert_eq!(
        profile.proxy,
        ConnectionProxyProfile::Socks5 {
            host: "proxy.example.net".into(),
            port: 1080,
            username: Some("ops-proxy".into()),
            password: None,
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }
    );
    assert!(profile.resolved_proxy_hops.is_empty());
}

#[test]
fn ssh_profile_normalizes_http_proxy_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();
    draft.proxy_type = "http".into();
    draft.proxy_socks5_host = "proxy.example.net".into();
    draft.proxy_socks5_port = "8080".into();
    draft.proxy_socks5_username = "ops-proxy".into();
    draft.proxy_socks5_password = "proxy-secret".into();

    let profile = ConnectionProfile::from_draft(&draft).expect("normalize http proxy draft");

    assert_eq!(
        profile.proxy,
        ConnectionProxyProfile::Http {
            host: "proxy.example.net".into(),
            port: 8080,
            username: Some("ops-proxy".into()),
            password: Some("proxy-secret".into()),
            credential_ref: None,
        }
    );
    assert!(profile.resolved_proxy_hops.is_empty());
}

#[test]
fn ssh_profile_normalizes_saved_http_proxy_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: "".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Http(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "8080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
            }),
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/password/asset-prod".into()),
        },
    )
    .expect("build saved http proxy profile");

    assert_eq!(
        profile.proxy,
        ConnectionProxyProfile::Http {
            host: "proxy.example.net".into(),
            port: 8080,
            username: Some("ops-proxy".into()),
            password: None,
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }
    );
    assert!(profile.resolved_proxy_hops.is_empty());
}

#[test]
fn ssh_proxy_chain_resolver_expands_recursive_upstream_chain() {
    let tree = saved_ssh_tree(vec![
        (
            "asset-b",
            "Upstream B",
            saved_password_spec(AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-b".into()),
            })),
        ),
        (
            "asset-a",
            "Upstream A",
            saved_password_spec(AssetSshProxySpec::SshAsset {
                asset_id: "asset-b".into(),
            }),
        ),
        (
            "asset-c",
            "Target C",
            saved_password_spec(AssetSshProxySpec::SshAsset {
                asset_id: "asset-a".into(),
            }),
        ),
    ]);
    let profile = ConnectionProfile::from_saved_asset(
        "asset-c",
        "Target C",
        tree.ssh_connection_spec("asset-c").expect("asset-c spec"),
    )
    .expect("normalize target profile");

    let hops = resolve_proxy_chain(&tree, &profile, 8).expect("resolve proxy chain");

    assert_eq!(hops.len(), 3);
    assert_eq!(
        hops[0],
        ResolvedProxyHop::Socks5 {
            host: "proxy.example.net".into(),
            port: 1080,
            username: Some("ops-proxy".into()),
            password: None,
        }
    );
    match &hops[1] {
        ResolvedProxyHop::Ssh(profile) => {
            assert_eq!(profile.asset_id.as_deref(), Some("asset-b"));
            assert_eq!(
                profile.proxy,
                ConnectionProxyProfile::Socks5 {
                    host: "proxy.example.net".into(),
                    port: 1080,
                    username: Some("ops-proxy".into()),
                    password: None,
                    credential_ref: Some("ssh/saved-secrets/asset-b".into()),
                }
            );
            assert!(profile.resolved_proxy_hops.is_empty());
        }
        other => panic!("unexpected hop: {other:?}"),
    }
    match &hops[2] {
        ResolvedProxyHop::Ssh(profile) => {
            assert_eq!(profile.asset_id.as_deref(), Some("asset-a"));
            assert_eq!(
                profile.proxy,
                ConnectionProxyProfile::SshAsset {
                    asset_id: "asset-b".into(),
                }
            );
            assert!(profile.resolved_proxy_hops.is_empty());
        }
        other => panic!("unexpected hop: {other:?}"),
    }
}

#[test]
fn ssh_proxy_chain_resolver_reports_cycle() {
    let tree = saved_ssh_tree(vec![
        (
            "asset-a",
            "Asset A",
            saved_password_spec(AssetSshProxySpec::SshAsset {
                asset_id: "asset-b".into(),
            }),
        ),
        (
            "asset-b",
            "Asset B",
            saved_password_spec(AssetSshProxySpec::SshAsset {
                asset_id: "asset-a".into(),
            }),
        ),
    ]);
    let profile = ConnectionProfile::from_saved_asset(
        "asset-a",
        "Asset A",
        tree.ssh_connection_spec("asset-a").expect("asset-a spec"),
    )
    .expect("normalize cycle profile");

    let err = resolve_proxy_chain(&tree, &profile, 8).expect_err("cycle should fail");

    assert!(err.to_string().contains("SSH proxy chain contains a cycle"));
}

#[test]
fn ssh_proxy_chain_resolver_reports_missing_upstream_asset() {
    let tree = saved_ssh_tree(vec![(
        "asset-a",
        "Asset A",
        saved_password_spec(AssetSshProxySpec::SshAsset {
            asset_id: "asset-missing".into(),
        }),
    )]);
    let profile = ConnectionProfile::from_saved_asset(
        "asset-a",
        "Asset A",
        tree.ssh_connection_spec("asset-a").expect("asset-a spec"),
    )
    .expect("normalize missing-upstream profile");

    let err = resolve_proxy_chain(&tree, &profile, 8).expect_err("missing upstream should fail");

    assert!(
        err.to_string()
            .contains("upstream SSH asset `asset-missing` was not found")
    );
}

#[test]
fn ssh_proxy_chain_resolver_reports_excessive_depth() {
    let mut nodes = Vec::new();
    for index in 1..=9 {
        let proxy = if index == 9 {
            AssetSshProxySpec::None
        } else {
            AssetSshProxySpec::SshAsset {
                asset_id: format!("asset-{}", index + 1),
            }
        };
        let asset_id = format!("asset-{index}");
        let title = format!("Asset {index}");
        nodes.push((asset_id, title, saved_password_spec(proxy)));
    }
    nodes.push((
        "asset-target".to_string(),
        "Target".to_string(),
        saved_password_spec(AssetSshProxySpec::SshAsset {
            asset_id: "asset-1".into(),
        }),
    ));
    let tree = saved_ssh_tree(
        nodes
            .iter()
            .map(|(id, title, spec)| (id.as_str(), title.as_str(), spec.clone()))
            .collect(),
    );
    let profile = ConnectionProfile::from_saved_asset(
        "asset-target",
        "Target",
        tree.ssh_connection_spec("asset-target")
            .expect("asset-target spec"),
    )
    .expect("normalize deep-chain profile");

    let err = resolve_proxy_chain(&tree, &profile, 8).expect_err("deep chain should fail");

    assert!(err.to_string().contains("SSH proxy chain is too deep"));
}
