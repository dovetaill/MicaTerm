use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind, GitHostKind,
    KdfConfig, PackLayout, ProviderAuthKind, ProviderKind, RemoteRole, VaultHead,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::git_repo::{
    GitRepositoryMetadata, GitRepositoryMetadataSource, GitRepositoryVisibility,
    GitRepositoryWritePermission, validate_remote_for_sync,
};
use mica_term::app::vault::provider::github_gist::{
    GitHubGistApi, GitHubGistAuth, GitHubGistDocument, GitHubGistFile, GitHubGistProvider,
    GitHubGistProviderConfig, GitHubGistUpdateRequest,
};

fn sample_device_flow_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-github-device".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitHubGist,
        locator: BootstrapRemoteLocator::GitHubGist {
            gist_id: "gist-device-123".into(),
        },
        credential_ref: Some("vault/bootstrap/github-device".into()),
        auth_kind: ProviderAuthKind::DeviceFlow,
        last_health: None,
    }
}

fn sample_pat_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-github-pat".into(),
        role: RemoteRole::Mirror,
        provider: ProviderKind::GitHubGist,
        locator: BootstrapRemoteLocator::GitHubGist {
            gist_id: "gist-pat-456".into(),
        },
        credential_ref: Some("vault/bootstrap/github-pat".into()),
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    }
}

fn sample_github_git_repo_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-github-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::GitHub,
            remote_url: "https://github.com/octo-org/mica-vault.git".into(),
            branch: "main".into(),
            base_url: Some("https://github.com".into()),
            api_base_url: Some("https://api.github.com".into()),
            namespace: Some("octo-org".into()),
            repository: Some("mica-vault".into()),
            root_path: Some(".mica-term-sync".into()),
            display_name: Some("octo-org/mica-vault".into()),
        },
        credential_ref: Some("vault/bootstrap/remote-github-primary".into()),
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    }
}

struct FakeGitHubRepositoryMetadataSource {
    next_result: Mutex<Option<Result<GitRepositoryMetadata>>>,
    recorded_auth: Mutex<Vec<Option<String>>>,
}

impl FakeGitHubRepositoryMetadataSource {
    fn returning(result: Result<GitRepositoryMetadata>) -> Self {
        Self {
            next_result: Mutex::new(Some(result)),
            recorded_auth: Mutex::new(Vec::new()),
        }
    }
}

impl GitRepositoryMetadataSource for FakeGitHubRepositoryMetadataSource {
    fn fetch_repository_metadata(
        &self,
        _remote: &BootstrapRemoteConfig,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        self.recorded_auth
            .lock()
            .map_err(|_| anyhow!("recorded auth lock poisoned"))?
            .push(access_token.map(ToOwned::to_owned));
        self.next_result
            .lock()
            .map_err(|_| anyhow!("metadata result lock poisoned"))?
            .take()
            .ok_or_else(|| anyhow!("missing repository metadata result"))?
    }
}

fn sample_github_repository_metadata(
    visibility: GitRepositoryVisibility,
    write_permission: GitRepositoryWritePermission,
) -> GitRepositoryMetadata {
    GitRepositoryMetadata {
        canonical_id: "octo-org/mica-vault".into(),
        display_name: "octo-org/mica-vault".into(),
        visibility,
        write_permission,
        default_branch: Some("main".into()),
    }
}

#[derive(Default)]
struct RecordingGitHubGistApi {
    gist: Mutex<Option<GitHubGistDocument>>,
    raw_reads: Mutex<Vec<String>>,
    update_calls: Mutex<Vec<(String, GitHubGistUpdateRequest, Option<String>)>>,
}

impl RecordingGitHubGistApi {
    fn with_gist(gist: GitHubGistDocument) -> Self {
        Self {
            gist: Mutex::new(Some(gist)),
            raw_reads: Mutex::new(Vec::new()),
            update_calls: Mutex::new(Vec::new()),
        }
    }
}

impl GitHubGistApi for RecordingGitHubGistApi {
    fn get_gist(&self, _gist_id: &str, _access_token: Option<&str>) -> Result<GitHubGistDocument> {
        self.gist
            .lock()
            .map_err(|_| anyhow!("gist lock poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("missing gist"))
    }

    fn get_raw_text(&self, raw_url: &str, _access_token: Option<&str>) -> Result<String> {
        self.raw_reads
            .lock()
            .map_err(|_| anyhow!("raw reads lock poisoned"))?
            .push(raw_url.to_string());
        Ok("full-file-from-raw-url".into())
    }

    fn update_gist(
        &self,
        gist_id: &str,
        request: &GitHubGistUpdateRequest,
        access_token: Option<&str>,
    ) -> Result<()> {
        self.update_calls
            .lock()
            .map_err(|_| anyhow!("update calls lock poisoned"))?
            .push((
                gist_id.to_string(),
                request.clone(),
                access_token.map(ToOwned::to_owned),
            ));
        Ok(())
    }
}

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "github-provider-salt".into(),
    }
}

fn sample_head(revision: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-a".into(),
        committed_at: "2026-03-31T08:00:00Z".into(),
        committed_by_device: "device-a".into(),
        payload_hash: "sha256:payload".into(),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: "wrapped-key".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    }
}

fn gist_revision_payload_file_set(revision: &str) -> [(String, GitHubGistFile); 2] {
    [
        (
            format!("vault-{revision}-manifest.bin"),
            GitHubGistFile {
                filename: format!("vault-{revision}-manifest.bin"),
                raw_url: None,
                truncated: false,
                content: Some("manifest".into()),
            },
        ),
        (
            format!("vault-{revision}-pack-0000.bin"),
            GitHubGistFile {
                filename: format!("vault-{revision}-pack-0000.bin"),
                raw_url: None,
                truncated: false,
                content: Some("pack".into()),
            },
        ),
    ]
}

#[test]
fn github_gist_bootstrap_config_can_represent_device_flow_or_pat() {
    let device_config = GitHubGistProviderConfig::try_from(&sample_device_flow_remote())
        .expect("parse device flow");
    let pat_config = GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat");

    assert!(matches!(
        device_config.auth,
        GitHubGistAuth::DeviceFlow { .. }
    ));
    assert!(matches!(
        pat_config.auth,
        GitHubGistAuth::PersonalAccessToken { .. }
    ));
}

#[test]
fn github_gist_provider_uses_bundled_files_layout_without_conditional_head_write() {
    let provider = GitHubGistProvider::new(
        GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat config"),
    )
    .expect("build github gist provider");

    let capabilities = provider.capabilities();

    assert_eq!(
        capabilities.preferred_pack_strategy,
        PackLayout::BundledFiles
    );
    assert!(!capabilities.supports_conditional_head_write);
    assert!(capabilities.max_pack_count <= 8);
}

#[test]
fn github_gist_reads_use_raw_url_when_file_is_truncated() {
    let provider = GitHubGistProvider::with_api(
        GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat config"),
        Arc::new(RecordingGitHubGistApi::with_gist(GitHubGistDocument {
            gist_id: "gist-pat-456".into(),
            truncated: false,
            files: BTreeMap::from([(
                "vault-head.json".into(),
                GitHubGistFile {
                    filename: "vault-head.json".into(),
                    raw_url: Some(
                        "https://gist.githubusercontent.com/demo/raw/vault-head.json".into(),
                    ),
                    truncated: true,
                    content: Some("{\"partial\":true}".into()),
                },
            )]),
        })),
    )
    .expect("build provider with fake api");

    let content = provider
        .load_gist_file_text("vault-head.json", Some("token"))
        .expect("load truncated gist file");

    assert_eq!(content, "full-file-from-raw-url");
}

#[test]
fn github_provider_prune_revisions_older_than_keep_latest_limit() {
    let mut files = BTreeMap::from([(
        "vault-head.json".into(),
        GitHubGistFile {
            filename: "vault-head.json".into(),
            raw_url: None,
            truncated: false,
            content: Some(
                serde_json::to_string(&sample_head("rev-0012")).expect("encode live head"),
            ),
        },
    )]);
    for revision in 1..=12 {
        files.extend(gist_revision_payload_file_set(
            format!("rev-{revision:04}").as_str(),
        ));
    }

    let api = Arc::new(RecordingGitHubGistApi::with_gist(GitHubGistDocument {
        gist_id: "gist-pat-456".into(),
        truncated: false,
        files,
    }));
    let provider = GitHubGistProvider::with_api(
        GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat config"),
        api.clone(),
    )
    .expect("build provider with fake api");

    provider
        .prune_revisions(10, &sample_head("rev-0012"))
        .expect("prune old github revisions");

    let update_calls = api.update_calls.lock().expect("lock update calls");
    assert_eq!(update_calls.len(), 1);
    let (gist_id, update, access_token) = &update_calls[0];
    assert_eq!(gist_id, "gist-pat-456");
    assert_eq!(access_token, &None);
    assert_eq!(
        update.deleted_files,
        vec![
            "vault-rev-0001-manifest.bin".to_string(),
            "vault-rev-0001-pack-0000.bin".to_string(),
            "vault-rev-0002-manifest.bin".to_string(),
            "vault-rev-0002-pack-0000.bin".to_string(),
        ]
    );
}

#[test]
fn github_public_repo_is_rejected() {
    let remote = sample_github_git_repo_remote();
    let source =
        FakeGitHubRepositoryMetadataSource::returning(Ok(sample_github_repository_metadata(
            GitRepositoryVisibility::Public,
            GitRepositoryWritePermission::Writable,
        )));

    let err = validate_remote_for_sync(&remote, &source, Some("github-pat"))
        .expect_err("public github repository must be rejected");

    assert!(
        err.to_string().contains("private"),
        "unexpected error: {err}"
    );
}

#[test]
fn github_private_repo_is_accepted_when_writable() {
    let remote = sample_github_git_repo_remote();
    let source =
        FakeGitHubRepositoryMetadataSource::returning(Ok(sample_github_repository_metadata(
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::Writable,
        )));

    let metadata = validate_remote_for_sync(&remote, &source, Some("github-pat"))
        .expect("private writable github repository should be accepted");

    assert_eq!(metadata.visibility, GitRepositoryVisibility::Private);
    assert_eq!(
        metadata.write_permission,
        GitRepositoryWritePermission::Writable
    );
    assert_eq!(
        source
            .recorded_auth
            .lock()
            .expect("lock recorded auth")
            .as_slice(),
        &[Some("github-pat".into())]
    );
}

#[test]
fn github_private_repo_without_push_permission_is_rejected() {
    let remote = sample_github_git_repo_remote();
    let source =
        FakeGitHubRepositoryMetadataSource::returning(Ok(sample_github_repository_metadata(
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::ReadOnly,
        )));

    let err = validate_remote_for_sync(&remote, &source, Some("github-pat"))
        .expect_err("github repository without push permission must be rejected");

    assert!(err.to_string().contains("write"), "unexpected error: {err}");
}

#[test]
fn github_unknown_visibility_fails_closed() {
    let remote = sample_github_git_repo_remote();
    let source =
        FakeGitHubRepositoryMetadataSource::returning(Ok(sample_github_repository_metadata(
            GitRepositoryVisibility::Unknown,
            GitRepositoryWritePermission::Writable,
        )));

    let err = validate_remote_for_sync(&remote, &source, Some("github-pat"))
        .expect_err("unknown github visibility must fail closed");

    assert!(
        err.to_string().contains("visibility"),
        "unexpected error: {err}"
    );
}
