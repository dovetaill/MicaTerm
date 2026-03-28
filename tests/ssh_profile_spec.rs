use mica_term::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
use mica_term::shell::assets::{AssetSocks5ProxySpec, AssetSshConnectionSpec, AssetSshProxySpec};
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

#[test]
fn ssh_profile_normalizes_password_mode_from_modal_draft() {
    let mut draft = base_draft();
    draft.auth_method = "password".into();
    draft.password = "super-secret".into();

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
fn ssh_profile_preserves_saved_upstream_ssh_proxy_reference() {
    let profile = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
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
