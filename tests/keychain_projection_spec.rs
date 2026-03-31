use std::collections::{BTreeMap, BTreeSet};

use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use mica_term::shell::keychain::{
    KeychainDeleteError, delete_keychain_node, project_keychain_rows,
};

fn sample_keychain_catalog() -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-team".into()],
        nodes: BTreeMap::from([
            (
                "folder-team".into(),
                KeychainNode {
                    id: "folder-team".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["identity-prod".into(), "key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-prod".into(),
                KeychainNode {
                    id: "identity-prod".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Prod Identity".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::SshKey,
                        ssh_key_id: Some("key-prod".into()),
                        credential_ref: Some("keychain/identity/identity-prod".into()),
                        remark: "shared login".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Prod Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:key-prod".into(),
                        public_key: "ssh-ed25519 AAAAC3Nzaprodpub".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some("keychain/key/key-prod".into()),
                        remark: "generated".into(),
                    }),
                },
            ),
        ]),
    }
}

fn sample_identity_backed_asset_tree() -> AssetTree {
    let mut tree = AssetTree::new();
    tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Gateway",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: String::new(),
            port: "22".into(),
            auth_method: String::new(),
            auth_source: "keychain-identity".into(),
            keychain_identity_id: Some("identity-prod".into()),
            private_key_source: String::new(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
        }),
    );
    tree
}

#[test]
fn tree_projection_preserves_folder_identity_and_ssh_key_order() {
    let rows = project_keychain_rows(
        &sample_keychain_catalog(),
        &BTreeSet::from(["folder-team".to_string()]),
        "",
    );

    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0].kind.id(), "folder");
    assert_eq!(rows[1].kind.id(), "identity");
    assert_eq!(rows[2].kind.id(), "ssh-key");
}

#[test]
fn search_matches_title_username_fingerprint_and_public_key_comment() {
    let catalog = sample_keychain_catalog();
    let expanded = BTreeSet::from(["folder-team".to_string()]);

    let title_rows = project_keychain_rows(&catalog, &expanded, "prod identity");
    assert!(title_rows.iter().any(|row| row.label == "Prod Identity"));

    let username_rows = project_keychain_rows(&catalog, &expanded, "ops");
    assert!(username_rows.iter().any(|row| row.label == "Prod Identity"));

    let fingerprint_rows = project_keychain_rows(&catalog, &expanded, "sha256:key-prod");
    assert!(fingerprint_rows.iter().any(|row| row.label == "Prod Key"));

    let comment_rows = project_keychain_rows(&catalog, &expanded, "prod@example");
    assert!(comment_rows.iter().any(|row| row.label == "Prod Key"));
}

#[test]
fn deletion_blocking_reports_when_identity_or_key_is_still_referenced() {
    let mut catalog = sample_keychain_catalog();
    let assets = sample_identity_backed_asset_tree();

    let delete_identity = delete_keychain_node(&mut catalog, "identity-prod", &assets);
    let delete_key = delete_keychain_node(&mut catalog, "key-prod", &assets);

    assert!(matches!(
        delete_identity,
        Err(KeychainDeleteError::ReferencedByHosts { reference_count: 1 })
    ));
    assert!(matches!(
        delete_key,
        Err(KeychainDeleteError::ReferencedByIdentities { reference_count: 1 })
    ));
}
