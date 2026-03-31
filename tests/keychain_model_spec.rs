use std::collections::BTreeMap;

use mica_term::app::keychain::model::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use serde_json::json;

#[test]
fn keychain_catalog_roundtrip_preserves_folder_identity_and_key_nodes() {
    let catalog = KeychainCatalog {
        root_ids: vec!["folder-prod".into(), "identity-ops".into()],
        nodes: BTreeMap::from([
            (
                "folder-prod".into(),
                KeychainNode {
                    id: "folder-prod".into(),
                    parent_id: None,
                    title: "Production".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: None,
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::Password,
                        ssh_key_id: None,
                        credential_ref: Some("keychain/identity/identity-ops".into()),
                        remark: "shared ops login".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-prod".into()),
                    title: "Prod SSH Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:prod".into(),
                        public_key: "ssh-ed25519 AAAAC3NzaProd".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some("keychain/key/key-prod".into()),
                        remark: "generated on prod laptop".into(),
                    }),
                },
            ),
        ]),
    };

    let encoded = serde_json::to_string_pretty(&catalog).unwrap();
    let decoded: KeychainCatalog = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.root_ids.len(), 2);
    assert_eq!(
        decoded.nodes["identity-ops"].kind,
        KeychainNodeKind::Identity
    );
    match &decoded.nodes["identity-ops"].payload {
        KeychainNodePayload::Identity(identity) => {
            assert_eq!(identity.username, "ops");
            assert_eq!(identity.auth_kind, KeychainIdentityAuthKind::Password);
        }
        other => panic!("expected identity payload, got {other:?}"),
    }
    match &decoded.nodes["key-prod"].payload {
        KeychainNodePayload::SshKey(ssh_key) => {
            assert_eq!(ssh_key.algorithm, "ed25519");
            assert_eq!(ssh_key.comment, "prod@example");
        }
        other => panic!("expected ssh key payload, got {other:?}"),
    }
}

#[test]
fn keychain_catalog_defaults_to_empty_tree() {
    let decoded: KeychainCatalog = serde_json::from_value(json!({})).unwrap();

    assert!(decoded.root_ids.is_empty());
    assert!(decoded.nodes.is_empty());
}

#[test]
fn keychain_node_kind_and_auth_kind_use_kebab_case_ids() {
    let ssh_key_kind = serde_json::to_value(KeychainNodeKind::SshKey).unwrap();
    let ssh_key_auth = serde_json::to_value(KeychainIdentityAuthKind::SshKey).unwrap();

    assert_eq!(ssh_key_kind, json!("ssh-key"));
    assert_eq!(ssh_key_auth, json!("ssh-key"));
    assert_eq!(
        serde_json::from_value::<KeychainNodeKind>(json!("identity")).unwrap(),
        KeychainNodeKind::Identity
    );
    assert_eq!(
        serde_json::from_value::<KeychainIdentityAuthKind>(json!("password")).unwrap(),
        KeychainIdentityAuthKind::Password
    );
}

#[test]
fn keychain_identity_spec_defaults_to_password_auth_without_key_reference() {
    let identity: KeychainIdentitySpec =
        serde_json::from_value(json!({ "username": "ops" })).unwrap();

    assert_eq!(identity.username, "ops");
    assert_eq!(identity.auth_kind, KeychainIdentityAuthKind::Password);
    assert!(identity.ssh_key_id.is_none());
    assert!(identity.credential_ref.is_none());
    assert!(identity.remark.is_empty());
}

#[test]
fn keychain_ssh_key_spec_defaults_to_empty_public_metadata() {
    let ssh_key: KeychainSshKeySpec = serde_json::from_value(json!({
        "algorithm": "ed25519"
    }))
    .unwrap();

    assert_eq!(ssh_key.algorithm, "ed25519");
    assert!(ssh_key.fingerprint.is_empty());
    assert!(ssh_key.public_key.is_empty());
    assert!(ssh_key.comment.is_empty());
    assert!(ssh_key.credential_ref.is_none());
}
