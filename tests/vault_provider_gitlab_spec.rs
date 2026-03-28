use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
    RemoteRole,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::gitlab_snippet::{
    GitLabSnippetAuth, GitLabSnippetFileLayout, GitLabSnippetProvider, GitLabSnippetProviderConfig,
};

fn sample_gitlab_remote(auth_kind: ProviderAuthKind) -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: format!("remote-gitlab-{auth_kind:?}").to_lowercase(),
        role: RemoteRole::Mirror,
        provider: ProviderKind::GitLabSnippet,
        locator: BootstrapRemoteLocator::GitLabSnippet {
            base_url: Some("https://gitlab.example.internal".into()),
            project_id: Some("group/project".into()),
            snippet_id: "snippet-123".into(),
        },
        credential_ref: Some("vault/bootstrap/gitlab".into()),
        auth_kind,
        last_health: None,
    }
}

#[test]
fn gitlab_bootstrap_auth_can_downgrade_from_device_flow_to_pkce_or_pat() {
    let device_config = GitLabSnippetProviderConfig::try_from(
        &sample_gitlab_remote(ProviderAuthKind::DeviceFlow),
    )
    .expect("parse gitlab device-flow config");
    let pkce_config =
        GitLabSnippetProviderConfig::try_from(&sample_gitlab_remote(ProviderAuthKind::Pkce))
            .expect("parse gitlab pkce config");
    let pat_config =
        GitLabSnippetProviderConfig::try_from(&sample_gitlab_remote(ProviderAuthKind::Pat))
            .expect("parse gitlab pat config");

    assert!(matches!(
        device_config.auth,
        GitLabSnippetAuth::DeviceFlow { .. }
    ));
    assert!(matches!(pkce_config.auth, GitLabSnippetAuth::Pkce { .. }));
    assert!(matches!(
        pat_config.auth,
        GitLabSnippetAuth::PersonalAccessToken { .. }
    ));
}

#[test]
fn gitlab_pack_layout_respects_the_ten_file_constraint() {
    let layout = GitLabSnippetFileLayout::for_revision("rev-0002", 8)
        .expect("8 packs plus head and manifest should fit within 10 files");

    assert_eq!(layout.file_names.len(), 10);
    assert!(
        GitLabSnippetFileLayout::for_revision("rev-0002", 9).is_err(),
        "9 packs would exceed the 10-file gitlab snippet limit"
    );
}

#[test]
fn gitlab_provider_capabilities_report_bundled_transport_without_strict_cas() {
    let provider = GitLabSnippetProvider::new(
        GitLabSnippetProviderConfig::try_from(&sample_gitlab_remote(ProviderAuthKind::Pat))
            .expect("parse gitlab provider config"),
    )
    .expect("build gitlab provider");

    let capabilities = provider.capabilities();

    assert_eq!(capabilities.preferred_pack_strategy, PackLayout::BundledFiles);
    assert!(!capabilities.supports_conditional_head_write);
    assert_eq!(capabilities.max_pack_count, 8);
}
