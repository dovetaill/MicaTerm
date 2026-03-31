use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, ProviderAuthKind, ProviderKind, RemoteRole,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::s3::{
    S3CredentialMode, S3ObjectKeySet, S3VaultProvider, S3VaultProviderConfig,
};

fn sample_s3_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-s3-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::S3Compatible,
        locator: BootstrapRemoteLocator::S3 {
            bucket: "vault-bucket".into(),
            prefix: "users/demo".into(),
            endpoint: Some("https://minio.example.internal:9000".into()),
            region: Some("cn-hangzhou".into()),
            force_path_style: true,
        },
        credential_ref: Some("vault/bootstrap/remote-s3-primary".into()),
        auth_kind: ProviderAuthKind::AwsStandardChain,
        last_health: None,
    }
}

#[test]
fn s3_provider_config_parses_bucket_prefix_region_endpoint_and_path_style() {
    let remote = sample_s3_remote();

    let config = S3VaultProviderConfig::try_from(&remote).expect("parse s3 remote");

    assert_eq!(config.remote_id, "remote-s3-primary");
    assert_eq!(config.bucket, "vault-bucket");
    assert_eq!(config.prefix, "users/demo");
    assert_eq!(
        config.endpoint.as_deref(),
        Some("https://minio.example.internal:9000")
    );
    assert_eq!(config.region.as_deref(), Some("cn-hangzhou"));
    assert!(config.force_path_style);
}

#[test]
fn s3_provider_uses_standard_credential_chain_by_default() {
    let remote = sample_s3_remote();

    let config = S3VaultProviderConfig::try_from(&remote).expect("parse s3 remote");

    assert_eq!(config.credential_mode, S3CredentialMode::StandardChain);
}

#[test]
fn s3_provider_exposes_conditional_head_write_capability() {
    let provider = S3VaultProvider::new(
        S3VaultProviderConfig::try_from(&sample_s3_remote()).expect("parse s3 remote"),
    )
    .expect("build s3 provider");

    let capabilities = provider.capabilities();

    assert!(capabilities.supports_conditional_head_write);
}

#[test]
fn s3_provider_generates_stable_deterministic_object_names_for_head_manifest_and_packs() {
    let keys = S3ObjectKeySet::for_revision("users/demo", "rev-0002", 3);

    assert_eq!(keys.head_key, "users/demo/head.json");
    assert_eq!(
        keys.manifest_key,
        "users/demo/revisions/rev-0002/manifest.bin"
    );
    assert_eq!(
        keys.pack_keys,
        vec![
            "users/demo/revisions/rev-0002/packs/pack-0000.bin".to_string(),
            "users/demo/revisions/rev-0002/packs/pack-0001.bin".to_string(),
            "users/demo/revisions/rev-0002/packs/pack-0002.bin".to_string(),
        ]
    );
}
