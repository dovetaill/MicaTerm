use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, StoredKeychainIdentitySecretBundle,
    StoredKeychainKeySecretBundle,
    keychain_identity_credential_ref, keychain_key_credential_ref,
    load_keychain_identity_secret_bundle, load_keychain_key_secret_bundle,
    persist_keychain_identity_secret_bundle, persist_keychain_key_secret_bundle,
    restore_keychain_identity_secret_bundle, restore_keychain_key_secret_bundle,
    snapshot_keychain_identity_secret_bundle, snapshot_keychain_key_secret_bundle,
};

#[test]
fn keychain_identity_and_key_refs_use_stable_namespaces() {
    assert_eq!(
        keychain_identity_credential_ref("id-1"),
        "keychain/identity/id-1"
    );
    assert_eq!(keychain_key_credential_ref("key-1"), "keychain/key/key-1");
}

#[test]
fn identity_secret_bundle_round_trips_password_and_empties_cleanly() {
    let store = MemoryCredentialStore::default();
    let credential_ref = keychain_identity_credential_ref("identity-prod");

    persist_keychain_identity_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredKeychainIdentitySecretBundle {
            password: Some("ops-password".into()),
        },
    )
    .expect("persist identity secret bundle");

    let loaded =
        load_keychain_identity_secret_bundle(&store, credential_ref.as_str()).expect("load bundle");
    assert_eq!(loaded.password.as_deref(), Some("ops-password"));

    let snapshotted =
        snapshot_keychain_identity_secret_bundle(&store, Some(credential_ref.as_str()))
            .expect("snapshot bundle");
    assert_eq!(
        snapshotted.as_ref().and_then(|bundle| bundle.password.as_deref()),
        Some("ops-password")
    );

    restore_keychain_identity_secret_bundle(
        &store,
        Some(credential_ref.as_str()),
        Some(&StoredKeychainIdentitySecretBundle { password: None }),
    )
    .expect("delete empty identity bundle");

    assert_eq!(
        store
            .get_secret(credential_ref.as_str())
            .expect("load deleted identity secret"),
        None
    );
}

#[test]
fn key_secret_bundle_round_trips_private_key_and_passphrase_and_empties_cleanly() {
    let store = MemoryCredentialStore::default();
    let credential_ref = keychain_key_credential_ref("key-prod");

    persist_keychain_key_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredKeychainKeySecretBundle {
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("key-passphrase".into()),
        },
    )
    .expect("persist key secret bundle");

    let loaded =
        load_keychain_key_secret_bundle(&store, credential_ref.as_str()).expect("load bundle");
    assert_eq!(
        loaded.private_key_content.as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert_eq!(loaded.passphrase.as_deref(), Some("key-passphrase"));

    let snapshotted = snapshot_keychain_key_secret_bundle(&store, Some(credential_ref.as_str()))
        .expect("snapshot key bundle");
    assert_eq!(
        snapshotted
            .as_ref()
            .and_then(|bundle| bundle.private_key_content.as_deref()),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );

    restore_keychain_key_secret_bundle(
        &store,
        Some(credential_ref.as_str()),
        Some(&StoredKeychainKeySecretBundle {
            private_key_content: None,
            passphrase: None,
        }),
    )
    .expect("delete empty key bundle");

    assert_eq!(
        store
            .get_secret(credential_ref.as_str())
            .expect("load deleted key secret"),
        None
    );
}
