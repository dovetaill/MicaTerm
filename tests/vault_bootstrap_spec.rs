use std::fs;
use std::path::PathBuf;

use mica_term::app::ssh::credentials::MemoryCredentialStore;
use mica_term::app::vault::bootstrap::{
    bootstrap_provider_credential_ref, export_bootstrap_bundle, import_bootstrap_bundle,
    load_provider_credential, persist_provider_credential, restore_provider_credentials,
};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, KdfConfig,
    ProviderAuthKind, ProviderKind, RemoteRole,
};
use secrecy::SecretString;
use uuid::Uuid;

fn temp_bootstrap_export_path() -> PathBuf {
    std::env::temp_dir().join(format!("mica-term-bootstrap-export-{}.bin", Uuid::new_v4()))
}

fn sample_bootstrap_bundle() -> BootstrapBundle {
    BootstrapBundle {
        format_version: 1,
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
                remote_id: "remote-s3-primary".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::S3Compatible,
                locator: BootstrapRemoteLocator::S3 {
                    bucket: "vault-bucket".into(),
                    prefix: "users/demo".into(),
                    endpoint: Some("https://s3.example.com".into()),
                    region: Some("ap-southeast-1".into()),
                    force_path_style: true,
                },
                credential_ref: Some(bootstrap_provider_credential_ref("remote-s3-primary")),
                auth_kind: ProviderAuthKind::AwsStandardChain,
                last_health: None,
            },
            BootstrapRemoteConfig {
                remote_id: "remote-github-mirror".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitHubGist,
                locator: BootstrapRemoteLocator::GitHubGist {
                    gist_id: "gist-123".into(),
                },
                credential_ref: Some(bootstrap_provider_credential_ref("remote-github-mirror")),
                auth_kind: ProviderAuthKind::Pat,
                last_health: None,
            },
        ],
        auto_sync_enabled: true,
        bootstrap_cipher: CipherKind::XChaCha20Poly1305,
        bootstrap_kdf: Some(KdfConfig::Argon2id {
            memory_cost_kib: 19_456,
            time_cost: 2,
            parallelism: 1,
            salt_b64: "bootstrap-static-salt".into(),
        }),
    }
}

#[test]
fn bootstrap_provider_credentials_round_trip_through_credential_store_refs() {
    let store = MemoryCredentialStore::default();
    let credential_ref = bootstrap_provider_credential_ref("remote-s3-primary");

    persist_provider_credential(&store, credential_ref.as_str(), Some("aws-secret-token"))
        .expect("persist provider credential");

    assert_eq!(
        load_provider_credential(&store, Some(credential_ref.as_str()))
            .expect("load provider credential")
            .as_deref(),
        Some("aws-secret-token")
    );
}

#[test]
fn bootstrap_export_round_trips_bundle_and_provider_credentials() {
    let path = temp_bootstrap_export_path();
    let password = SecretString::new("bootstrap-passphrase".into());
    let source_store = MemoryCredentialStore::default();
    let bundle = sample_bootstrap_bundle();

    persist_provider_credential(
        &source_store,
        bundle.remotes[0].credential_ref.as_deref().expect("primary credential ref"),
        Some("aws-secret-token"),
    )
    .expect("persist primary provider credential");
    persist_provider_credential(
        &source_store,
        bundle.remotes[1].credential_ref.as_deref().expect("mirror credential ref"),
        Some("github-pat-token"),
    )
    .expect("persist mirror provider credential");

    export_bootstrap_bundle(&path, &bundle, &source_store, &password)
        .expect("export bootstrap bundle");

    let imported = import_bootstrap_bundle(&path, &password).expect("import bootstrap bundle");
    assert_eq!(imported.bundle, bundle);
    assert_eq!(
        imported
            .provider_credentials
            .get(bundle.remotes[0].credential_ref.as_ref().expect("primary ref"))
            .map(String::as_str),
        Some("aws-secret-token")
    );
    assert_eq!(
        imported
            .provider_credentials
            .get(bundle.remotes[1].credential_ref.as_ref().expect("mirror ref"))
            .map(String::as_str),
        Some("github-pat-token")
    );

    let restored_store = MemoryCredentialStore::default();
    restore_provider_credentials(&restored_store, &imported).expect("restore provider credentials");
    assert_eq!(
        load_provider_credential(
            &restored_store,
            bundle.remotes[1].credential_ref.as_deref()
        )
        .expect("reload restored provider credential")
        .as_deref(),
        Some("github-pat-token")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn bootstrap_export_file_is_not_plaintext_json() {
    let path = temp_bootstrap_export_path();
    let password = SecretString::new("bootstrap-passphrase".into());
    let store = MemoryCredentialStore::default();
    let bundle = sample_bootstrap_bundle();

    persist_provider_credential(
        &store,
        bundle.remotes[0].credential_ref.as_deref().expect("primary credential ref"),
        Some("aws-secret-token"),
    )
    .expect("persist provider credential");

    export_bootstrap_bundle(&path, &bundle, &store, &password).expect("export bootstrap bundle");

    let raw = fs::read(&path).expect("read encrypted bootstrap export");
    let printable = String::from_utf8_lossy(&raw);
    assert!(!printable.contains("vault-main"));
    assert!(!printable.contains("remote-s3-primary"));
    assert!(!printable.contains("\"remotes\""));
    assert!(!printable.contains("aws-secret-token"));

    let _ = fs::remove_file(path);
}
