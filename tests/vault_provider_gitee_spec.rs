use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
    RemoteRole,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::gitee_gist::{
    GiteeGistAuth, GiteeGistProvider, GiteeGistProviderConfig,
};

fn sample_gitee_remote(auth_kind: ProviderAuthKind) -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: format!("remote-gitee-{auth_kind:?}").to_lowercase(),
        role: RemoteRole::Mirror,
        provider: ProviderKind::GiteeGist,
        locator: BootstrapRemoteLocator::GiteeGist {
            gist_id: "gitee-gist-456".into(),
        },
        credential_ref: Some("vault/bootstrap/gitee".into()),
        auth_kind,
        last_health: None,
    }
}

#[test]
fn gitee_bootstrap_config_supports_pat_only_for_first_release() {
    let pat_config =
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
            .expect("parse gitee pat config");

    assert!(matches!(
        pat_config.auth,
        GiteeGistAuth::PersonalAccessToken { .. }
    ));
    assert!(matches!(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pkce)),
        Err(_)
    ));
}

#[test]
fn gitee_provider_defaults_to_bundled_files_layout() {
    let provider = GiteeGistProvider::new(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
            .expect("parse gitee provider config"),
    )
    .expect("build gitee provider");

    let capabilities = provider.capabilities();

    assert_eq!(capabilities.preferred_pack_strategy, PackLayout::BundledFiles);
    assert!(capabilities.max_pack_count <= 6);
}

#[test]
fn gitee_provider_capabilities_report_non_conditional_writes() {
    let provider = GiteeGistProvider::new(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
            .expect("parse gitee provider config"),
    )
    .expect("build gitee provider");

    assert!(!provider.capabilities().supports_conditional_head_write);
}
