use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, ssh_credential_ref,
};

#[test]
fn credential_store_round_trips_password_secret() {
    let store = MemoryCredentialStore::default();

    store
        .put_secret("ssh/password/prod-bastion", "super-secret")
        .expect("store password");

    assert_eq!(
        store
            .get_secret("ssh/password/prod-bastion")
            .expect("load password")
            .as_deref(),
        Some("super-secret")
    );
}

#[test]
fn credential_store_round_trips_inline_private_key_and_passphrase() {
    let store = MemoryCredentialStore::default();

    store
        .put_secret(
            "ssh/private-key/prod-bastion",
            "-----BEGIN OPENSSH PRIVATE KEY-----",
        )
        .expect("store private key");
    store
        .put_secret("ssh/passphrase/prod-bastion", "hunter2")
        .expect("store passphrase");

    assert_eq!(
        store
            .get_secret("ssh/private-key/prod-bastion")
            .expect("load private key")
            .as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert_eq!(
        store
            .get_secret("ssh/passphrase/prod-bastion")
            .expect("load passphrase")
            .as_deref(),
        Some("hunter2")
    );
}

#[test]
fn system_credential_store_can_replace_existing_secret_for_same_reference() {
    let store = MemoryCredentialStore::default();
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    store
        .put_secret(credential_ref.as_str(), "first-secret")
        .expect("store initial secret");
    store
        .put_secret(credential_ref.as_str(), "second-secret")
        .expect("replace secret");

    assert_eq!(
        store
            .get_secret(credential_ref.as_str())
            .expect("load latest secret")
            .as_deref(),
        Some("second-secret")
    );
}
