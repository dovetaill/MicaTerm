use std::collections::BTreeMap;

use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec, resolve_saved_ssh_profile,
};
use mica_term::app::ssh::credentials::{
    keychain_identity_credential_ref, keychain_key_credential_ref,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::shell::assets::{AssetSshConnectionSpec, AssetSshProxySpec};

fn identity_backed_host_spec(identity_id: &str) -> AssetSshConnectionSpec {
    AssetSshConnectionSpec {
        host: "10.0.0.12".into(),
        user: String::new(),
        port: "22".into(),
        auth_method: String::new(),
        auth_source: "keychain-identity".into(),
        keychain_identity_id: Some(identity_id.into()),
        private_key_source: String::new(),
        private_key_path: String::new(),
        environment: "prod".into(),
        proxy: AssetSshProxySpec::None,
        proxy_method: String::new(),
        remark: "Primary entry point".into(),
        credential_ref: None,
    }
}

fn manual_password_spec(credential_ref: &str) -> AssetSshConnectionSpec {
    AssetSshConnectionSpec {
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
        remark: "Primary entry point".into(),
        credential_ref: Some(credential_ref.into()),
    }
}

fn manual_inline_key_spec(credential_ref: &str) -> AssetSshConnectionSpec {
    AssetSshConnectionSpec {
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: "22".into(),
        auth_method: "private-key".into(),
        auth_source: "manual".into(),
        keychain_identity_id: None,
        private_key_source: "content".into(),
        private_key_path: String::new(),
        environment: "prod".into(),
        proxy: AssetSshProxySpec::None,
        proxy_method: String::new(),
        remark: "Primary entry point".into(),
        credential_ref: Some(credential_ref.into()),
    }
}

fn password_identity_catalog() -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["identity-prod".into()],
        nodes: BTreeMap::from([(
            "identity-prod".into(),
            KeychainNode {
                id: "identity-prod".into(),
                parent_id: None,
                title: "Ops Password".into(),
                kind: KeychainNodeKind::Identity,
                child_ids: Vec::new(),
                payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                    username: "ops".into(),
                    auth_kind: KeychainIdentityAuthKind::Password,
                    ssh_key_id: None,
                    credential_ref: Some(keychain_identity_credential_ref("identity-prod")),
                    remark: "shared password".into(),
                }),
            },
        )]),
        merge_metadata: BTreeMap::new(),
    }
}

fn ssh_key_identity_catalog(ssh_key_id: Option<&str>) -> KeychainCatalog {
    let mut nodes = BTreeMap::from([(
        "identity-prod".into(),
        KeychainNode {
            id: "identity-prod".into(),
            parent_id: None,
            title: "Ops Key".into(),
            kind: KeychainNodeKind::Identity,
            child_ids: Vec::new(),
            payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                username: "ops".into(),
                auth_kind: KeychainIdentityAuthKind::SshKey,
                ssh_key_id: ssh_key_id.map(ToString::to_string),
                credential_ref: Some(keychain_identity_credential_ref("identity-prod")),
                remark: "shared key".into(),
            }),
        },
    )]);
    if let Some(ssh_key_id) = ssh_key_id.filter(|id| *id == "key-prod") {
        nodes.insert(
            ssh_key_id.into(),
            KeychainNode {
                id: ssh_key_id.into(),
                parent_id: None,
                title: "Prod Key".into(),
                kind: KeychainNodeKind::SshKey,
                child_ids: Vec::new(),
                payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                    algorithm: "ed25519".into(),
                    fingerprint: "SHA256:key-prod".into(),
                    public_key: "ssh-ed25519 AAAAC3NzaKeyProd".into(),
                    comment: "prod@example".into(),
                    credential_ref: Some(keychain_key_credential_ref("key-prod")),
                    remark: "generated".into(),
                }),
            },
        );
    }

    KeychainCatalog {
        root_ids: vec!["identity-prod".into()],
        nodes,
        merge_metadata: BTreeMap::new(),
    }
}

#[test]
fn password_identity_resolves_to_same_runtime_shape_as_manual_password_auth() {
    let resolved = resolve_saved_ssh_profile(
        "asset-prod",
        "Prod Bastion",
        &identity_backed_host_spec("identity-prod"),
        &password_identity_catalog(),
    )
    .expect("resolve password identity");
    let expected = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &manual_password_spec("keychain/identity/identity-prod"),
    )
    .expect("manual password profile");

    assert_eq!(resolved, expected);
}

#[test]
fn ssh_key_identity_resolves_to_same_runtime_shape_as_manual_inline_key_auth() {
    let resolved = resolve_saved_ssh_profile(
        "asset-prod",
        "Prod Bastion",
        &identity_backed_host_spec("identity-prod"),
        &ssh_key_identity_catalog(Some("key-prod")),
    )
    .expect("resolve ssh key identity");
    let expected = ConnectionProfile::from_saved_asset(
        "asset-prod",
        "Prod Bastion",
        &manual_inline_key_spec("keychain/key/key-prod"),
    )
    .expect("manual inline key profile");

    assert_eq!(resolved, expected);
}

#[test]
fn missing_identity_reference_reports_explicit_diagnostics() {
    let err = resolve_saved_ssh_profile(
        "asset-prod",
        "Prod Bastion",
        &identity_backed_host_spec("identity-missing"),
        &KeychainCatalog::default(),
    )
    .expect_err("missing identity should fail");

    assert!(
        err.to_string().contains(
            "keychain identity `identity-missing` referenced by SSH asset `Prod Bastion`"
        )
    );
}

#[test]
fn missing_ssh_key_reference_reports_explicit_diagnostics() {
    let err = resolve_saved_ssh_profile(
        "asset-prod",
        "Prod Bastion",
        &identity_backed_host_spec("identity-prod"),
        &ssh_key_identity_catalog(None),
    )
    .expect_err("missing ssh key reference should fail");

    assert!(
        err.to_string()
            .contains("keychain identity `identity-prod` is missing SSH key reference")
    );
}
