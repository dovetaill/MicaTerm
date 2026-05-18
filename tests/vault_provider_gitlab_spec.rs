use std::sync::Mutex;

use anyhow::{Result, anyhow};
use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, GitHostKind, PackLayout, ProviderAuthKind,
    ProviderKind, RemoteRole,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::git_repo::{
    GitRepositoryMetadata, GitRepositoryMetadataSource, GitRepositoryVisibility,
    GitRepositoryWritePermission, validate_remote_for_sync,
};
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

fn sample_gitlab_git_repo_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-gitlab-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::GitLab,
            remote_url: "https://gitlab.example.internal/platform/mica-vault.git".into(),
            branch: "main".into(),
        },
        credential_ref: Some("vault/bootstrap/remote-gitlab-primary".into()),
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    }
}

struct FakeGitLabRepositoryMetadataSource {
    next_result: Mutex<Result<GitRepositoryMetadata>>,
}

impl FakeGitLabRepositoryMetadataSource {
    fn returning(result: Result<GitRepositoryMetadata>) -> Self {
        Self {
            next_result: Mutex::new(result),
        }
    }
}

impl GitRepositoryMetadataSource for FakeGitLabRepositoryMetadataSource {
    fn fetch_repository_metadata(
        &self,
        _remote: &BootstrapRemoteConfig,
        _access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        self.next_result
            .lock()
            .map_err(|_| anyhow!("metadata result lock poisoned"))?
            .clone()
    }
}

fn sample_gitlab_repository_metadata(
    visibility: GitRepositoryVisibility,
    write_permission: GitRepositoryWritePermission,
) -> GitRepositoryMetadata {
    GitRepositoryMetadata {
        canonical_id: "platform/mica-vault".into(),
        display_name: "platform/mica-vault".into(),
        visibility,
        write_permission,
        default_branch: Some("main".into()),
    }
}

#[test]
fn gitlab_bootstrap_auth_can_downgrade_from_device_flow_to_pkce_or_pat() {
    let device_config =
        GitLabSnippetProviderConfig::try_from(&sample_gitlab_remote(ProviderAuthKind::DeviceFlow))
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

    assert_eq!(
        capabilities.preferred_pack_strategy,
        PackLayout::BundledFiles
    );
    assert!(!capabilities.supports_conditional_head_write);
    assert_eq!(capabilities.max_pack_count, 8);
}

#[test]
fn gitlab_public_repo_is_rejected() {
    let remote = sample_gitlab_git_repo_remote();
    let source = FakeGitLabRepositoryMetadataSource::returning(Ok(
        sample_gitlab_repository_metadata(
            GitRepositoryVisibility::Public,
            GitRepositoryWritePermission::Writable,
        ),
    ));

    let err = validate_remote_for_sync(&remote, &source, Some("gitlab-pat"))
        .expect_err("public gitlab repository must be rejected");

    assert!(
        err.to_string().contains("private"),
        "unexpected error: {err}"
    );
}

#[test]
fn gitlab_internal_repo_is_rejected() {
    let remote = sample_gitlab_git_repo_remote();
    let source = FakeGitLabRepositoryMetadataSource::returning(Ok(
        sample_gitlab_repository_metadata(
            GitRepositoryVisibility::Internal,
            GitRepositoryWritePermission::Writable,
        ),
    ));

    let err = validate_remote_for_sync(&remote, &source, Some("gitlab-pat"))
        .expect_err("internal gitlab repository must be rejected");

    assert!(
        err.to_string().contains("internal"),
        "unexpected error: {err}"
    );
}

#[test]
fn gitlab_private_repo_is_accepted_when_writable() {
    let remote = sample_gitlab_git_repo_remote();
    let source = FakeGitLabRepositoryMetadataSource::returning(Ok(
        sample_gitlab_repository_metadata(
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::Writable,
        ),
    ));

    let metadata = validate_remote_for_sync(&remote, &source, Some("gitlab-pat"))
        .expect("private writable gitlab repository should be accepted");

    assert_eq!(metadata.visibility, GitRepositoryVisibility::Private);
    assert_eq!(
        metadata.write_permission,
        GitRepositoryWritePermission::Writable
    );
}
