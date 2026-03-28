use mica_term::app::ssh::credentials::{
    CachedCredentialStore, CredentialStore, FileCredentialStore, MemoryCredentialStore,
    SshCredentialKind, StoredSecretLookupError, StoredSshSecretBundle, load_secret_bundle,
    load_secret_bundle_with_diagnostics, merge_edit_bundle, persist_secret_bundle,
    required_secret_bundle_field, ssh_credential_ref,
};
use mica_term::shell::view_model::AssetSshConnectionDraft;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

fn temp_credentials_dir() -> PathBuf {
    std::env::temp_dir().join(format!("mica-term-credential-store-{}", Uuid::new_v4()))
}

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

#[test]
fn persist_secret_bundle_stores_password_as_json_payload() {
    let store = MemoryCredentialStore::default();
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    persist_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist password bundle");

    assert_eq!(
        load_secret_bundle(&store, credential_ref.as_str())
            .expect("load password bundle")
            .password
            .as_deref(),
        Some("super-secret")
    );
}

#[test]
fn persist_secret_bundle_replaces_previous_secret_material_for_same_ref() {
    let store = MemoryCredentialStore::default();
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    persist_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("first-password".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist initial bundle");

    persist_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: None,
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("replace bundle");

    let bundle = load_secret_bundle(&store, credential_ref.as_str()).expect("load latest bundle");
    assert_eq!(bundle.password, None);
    assert_eq!(
        bundle.private_key_content.as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert_eq!(bundle.passphrase.as_deref(), Some("hunter2"));
}

#[test]
fn persist_secret_bundle_round_trips_proxy_socks5_password() {
    let store = MemoryCredentialStore::default();
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    persist_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("ssh-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: Some("proxy-secret".into()),
        },
    )
    .expect("persist proxy password bundle");

    let bundle = load_secret_bundle(&store, credential_ref.as_str()).expect("load proxy bundle");
    assert_eq!(bundle.password.as_deref(), Some("ssh-secret"));
    assert_eq!(bundle.proxy_socks5_password.as_deref(), Some("proxy-secret"));
}

#[test]
fn editing_saved_proxy_password_blank_clears_proxy_secret_without_touching_ssh_auth_secret() {
    let existing = StoredSshSecretBundle {
        password: Some("ssh-secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy_socks5_password: Some("stale-proxy-secret".into()),
    };
    let draft = AssetSshConnectionDraft {
        auth_method: "password".into(),
        password: "ssh-secret".into(),
        proxy_type: "socks5".into(),
        proxy_socks5_host: "proxy.example.net".into(),
        proxy_socks5_port: "1080".into(),
        ..AssetSshConnectionDraft::default()
    };

    let merged = merge_edit_bundle(existing, &draft);

    assert_eq!(merged.password.as_deref(), Some("ssh-secret"));
    assert_eq!(merged.proxy_socks5_password, None);
}

#[test]
fn editing_saved_password_mode_persists_proxy_password_alongside_ssh_auth_secret() {
    let draft = AssetSshConnectionDraft {
        auth_method: "password".into(),
        password: "ssh-secret".into(),
        proxy_type: "socks5".into(),
        proxy_socks5_host: "proxy.example.net".into(),
        proxy_socks5_port: "1080".into(),
        proxy_socks5_username: "ops-proxy".into(),
        proxy_socks5_password: "proxy-secret".into(),
        ..AssetSshConnectionDraft::default()
    };

    let merged = merge_edit_bundle(StoredSshSecretBundle::default(), &draft);

    assert_eq!(merged.password.as_deref(), Some("ssh-secret"));
    assert_eq!(merged.proxy_socks5_password.as_deref(), Some("proxy-secret"));
    assert_eq!(merged.private_key_content, None);
    assert_eq!(merged.passphrase, None);
}

#[test]
fn editing_saved_secret_fields_blank_keeps_existing_bundle() {
    let existing = StoredSshSecretBundle {
        password: None,
        private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
        passphrase: Some("hunter2".into()),
        proxy_socks5_password: None,
    };
    let draft = AssetSshConnectionDraft {
        auth_method: "private-key".into(),
        private_key_source: "content".into(),
        private_key_content: String::new(),
        passphrase: String::new(),
        ..AssetSshConnectionDraft::default()
    };

    let merged = merge_edit_bundle(existing, &draft);

    assert!(merged.is_empty());
}

#[test]
fn editing_saved_password_blank_clears_saved_bundle() {
    let existing = StoredSshSecretBundle {
        password: Some("super-secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy_socks5_password: None,
    };
    let draft = AssetSshConnectionDraft {
        auth_method: "password".into(),
        ..AssetSshConnectionDraft::default()
    };

    let merged = merge_edit_bundle(existing, &draft);

    assert!(merged.is_empty());
}

#[test]
fn file_credential_store_persists_secret_across_store_instances() {
    let root = temp_credentials_dir();
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    let store = FileCredentialStore::new(root.clone());
    persist_secret_bundle(
        &store,
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist password bundle to file-backed store");

    let reloaded_store = FileCredentialStore::new(root.clone());
    let bundle =
        load_secret_bundle(&reloaded_store, credential_ref.as_str()).expect("reload bundle");

    assert_eq!(bundle.password.as_deref(), Some("super-secret"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn missing_saved_secret_binding_reports_missing_credential_ref() {
    let store = MemoryCredentialStore::default();

    let err = load_secret_bundle_with_diagnostics(&store, None).expect_err("missing binding");

    assert_eq!(err, StoredSecretLookupError::MissingCredentialRef);
}

#[test]
fn missing_saved_secret_reports_missing_keyring_entry() {
    let store = MemoryCredentialStore::default();

    let err = load_secret_bundle_with_diagnostics(&store, Some("ssh/saved-secrets/asset-prod"))
        .expect_err("missing keyring entry");

    assert_eq!(
        err,
        StoredSecretLookupError::MissingEntry {
            credential_ref: "ssh/saved-secrets/asset-prod".into(),
        }
    );
}

#[test]
fn empty_saved_secret_field_reports_empty_bundle_field() {
    let err = required_secret_bundle_field(
        &StoredSshSecretBundle::default(),
        "ssh/saved-secrets/asset-prod",
        "password",
    )
    .expect_err("missing password field");

    assert_eq!(
        err,
        StoredSecretLookupError::EmptyBundleField {
            credential_ref: "ssh/saved-secrets/asset-prod".into(),
            field: "password",
        }
    );
}

#[test]
fn cached_credential_store_survives_same_process_backend_read_miss() {
    let backing = Arc::new(MemoryCredentialStore::default());
    let store = CachedCredentialStore::new(backing.clone() as Arc<dyn CredentialStore>);
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    store
        .put_secret(credential_ref.as_str(), "super-secret")
        .expect("write cached secret");
    backing
        .delete_secret(credential_ref.as_str())
        .expect("simulate backend read miss");

    assert_eq!(
        store
            .get_secret(credential_ref.as_str())
            .expect("read cached secret after backend miss")
            .as_deref(),
        Some("super-secret")
    );
}

#[test]
fn cached_credential_store_delete_clears_cache_and_backing_store() {
    let backing = Arc::new(MemoryCredentialStore::default());
    let store = CachedCredentialStore::new(backing.clone() as Arc<dyn CredentialStore>);
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);

    store
        .put_secret(credential_ref.as_str(), "super-secret")
        .expect("write cached secret");
    store
        .delete_secret(credential_ref.as_str())
        .expect("delete cached secret");

    assert_eq!(
        store
            .get_secret(credential_ref.as_str())
            .expect("read cached store after delete"),
        None
    );
    assert_eq!(
        backing
            .get_secret(credential_ref.as_str())
            .expect("read backing store after delete"),
        None
    );
}
