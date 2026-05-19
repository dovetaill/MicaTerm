use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use git2::{
    Cred, CredentialType, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository,
    RepositoryInitOptions, Signature, build::CheckoutBuilder,
};
use serde::Deserialize;

use crate::app::vault::auth::git::{
    GitTransportAuthPlan, build_https_auth_plan, build_ssh_auth_plan,
    git_auth_mode_for_provider_auth,
};
use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, GitHostKind, ProviderAuthKind, ProviderKind,
    VaultHead, VaultManifest,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderRevision, ProviderWriteRequest,
    VaultProvider, rebuild_snapshot_from_manifest,
};

const REMOTE_NAME: &str = "origin";
const HEAD_FILE_NAME: &str = "vault-head.json";
const MANIFEST_FILE_NAME: &str = "vault-manifest.bin";
const SNAPSHOT_FILE_NAME: &str = "vault-snapshot.bin";
const DEFAULT_SYNC_ROOT_PATH: &str = ".mica-term-sync";
const REVISIONS_DIR_NAME: &str = "revisions";
const COMMITTER_NAME: &str = "Mica Term Vault";
const COMMITTER_EMAIL: &str = "vault@mica-term.local";
const GITHUB_BASE_URL: &str = "https://github.com";
const GITHUB_API_BASE_URL: &str = "https://api.github.com";
const GITLAB_BASE_URL: &str = "https://gitlab.com";
const GITLAB_API_PATH: &str = "api/v4";
const GITEE_BASE_URL: &str = "https://gitee.com";
const GITEE_API_PATH: &str = "api/v5";
const REQUEST_USER_AGENT: &str = "mica-term-vault-sync";

pub use crate::app::vault::model::{
    GitRemoteSafetyStatus, GitRepositoryVisibility, GitRepositoryWritePermission,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepositoryMetadata {
    pub canonical_id: String,
    pub display_name: String,
    pub visibility: GitRepositoryVisibility,
    pub write_permission: GitRepositoryWritePermission,
    pub default_branch: Option<String>,
}

pub trait GitRepositoryMetadataSource: Send + Sync {
    fn fetch_repository_metadata(
        &self,
        remote: &BootstrapRemoteConfig,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata>;
}

#[derive(Debug, Default)]
pub struct ReqwestGitRepositoryMetadataSource;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepoProviderConfig {
    pub remote_id: String,
    pub host_kind: GitHostKind,
    pub remote_url: String,
    pub branch: String,
    pub base_url: Option<String>,
    pub api_base_url: Option<String>,
    pub namespace: Option<String>,
    pub repository: Option<String>,
    pub root_path: Option<String>,
    pub display_name: Option<String>,
    pub cache_dir: PathBuf,
    pub auth: GitTransportAuthPlan,
}

#[derive(Debug, Deserialize, Default)]
struct InlineGitCredentialMaterial {
    #[serde(default)]
    https_username: String,
    #[serde(default)]
    https_secret: String,
    #[serde(default)]
    ssh_private_key: String,
    #[serde(default)]
    ssh_passphrase: String,
}

impl GitRepoProviderConfig {
    pub fn from_bootstrap_remote(
        remote: &BootstrapRemoteConfig,
        cache_dir: PathBuf,
    ) -> Result<Self> {
        if remote.provider != ProviderKind::GitRepo {
            return Err(anyhow!(
                "bootstrap remote `{}` is not a Git repo provider",
                remote.remote_id
            ));
        }

        let BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            base_url,
            api_base_url,
            namespace,
            repository,
            root_path,
            display_name,
        } = &remote.locator
        else {
            return Err(anyhow!(
                "bootstrap remote `{}` is missing a Git repo locator",
                remote.remote_id
            ));
        };

        ensure!(
            !remote_url.trim().is_empty(),
            "bootstrap remote `{}` is missing a Git remote URL",
            remote.remote_id
        );
        ensure!(
            !branch.trim().is_empty(),
            "bootstrap remote `{}` is missing a Git branch",
            remote.remote_id
        );

        let inline_credentials =
            decode_inline_credentials(remote.credential_ref.as_deref(), remote.auth_kind);
        let auth = match git_auth_mode_for_provider_auth(remote.auth_kind)? {
            crate::app::vault::auth::git::GitAuthMode::HttpsCredentials => build_https_auth_plan(
                inline_credentials.https_username.as_str(),
                inline_credentials.https_secret.as_str(),
            )?,
            crate::app::vault::auth::git::GitAuthMode::SshKey => build_ssh_auth_plan(
                inline_credentials.ssh_private_key.as_str(),
                Some(inline_credentials.ssh_passphrase.as_str()),
            )?,
        };

        Ok(Self {
            remote_id: remote.remote_id.clone(),
            host_kind: *host_kind,
            remote_url: remote_url.clone(),
            branch: branch.clone(),
            base_url: base_url.clone(),
            api_base_url: api_base_url.clone(),
            namespace: namespace.clone(),
            repository: repository.clone(),
            root_path: root_path.clone(),
            display_name: display_name.clone(),
            cache_dir,
            auth,
        })
    }
}

#[derive(Debug, Deserialize)]
struct GitHubRepositoryDocument {
    full_name: Option<String>,
    default_branch: Option<String>,
    private: Option<bool>,
    visibility: Option<String>,
    permissions: Option<GitHubRepositoryPermissions>,
}

#[derive(Debug, Deserialize)]
struct GitHubRepositoryPermissions {
    #[serde(default)]
    push: bool,
    #[serde(default)]
    admin: bool,
    #[serde(default)]
    maintain: bool,
}

#[derive(Debug, Deserialize)]
struct GitLabProjectDocument {
    path_with_namespace: Option<String>,
    default_branch: Option<String>,
    visibility: Option<String>,
    permissions: Option<GitLabProjectPermissions>,
}

#[derive(Debug, Deserialize)]
struct GitLabProjectPermissions {
    project_access: Option<GitLabAccessLevel>,
    group_access: Option<GitLabAccessLevel>,
}

#[derive(Debug, Deserialize)]
struct GitLabAccessLevel {
    access_level: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GiteeRepositoryDocument {
    full_name: Option<String>,
    default_branch: Option<String>,
    private: Option<bool>,
    public: Option<bool>,
    internal: Option<bool>,
    can_push: Option<bool>,
}

impl GitRepositoryMetadataSource for ReqwestGitRepositoryMetadataSource {
    fn fetch_repository_metadata(
        &self,
        remote: &BootstrapRemoteConfig,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        let target = resolve_repository_target(remote)?;
        match target.host_kind {
            GitHostKind::GitHub => self.fetch_github_metadata(&target, access_token),
            GitHostKind::GitLab => self.fetch_gitlab_metadata(&target, access_token),
            GitHostKind::Gitee => self.fetch_gitee_metadata(&target, access_token),
            GitHostKind::Generic => Err(anyhow!(
                "generic Git hosts cannot be validated for private repository sync"
            )),
        }
    }
}

impl ReqwestGitRepositoryMetadataSource {
    fn fetch_github_metadata(
        &self,
        target: &ResolvedGitRepositoryTarget,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        let mut url =
            reqwest::Url::parse(&target.api_base_url).context("invalid GitHub API base URL")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow!("invalid GitHub API base URL path"))?;
            segments.push("repos");
            segments.push(target.namespace.as_str());
            segments.push(target.repository.as_str());
        }

        run_git_repository_request(async move {
            let client = reqwest::Client::new();
            let mut request = client
                .get(url)
                .header(reqwest::header::USER_AGENT, REQUEST_USER_AGENT)
                .header(reqwest::header::ACCEPT, "application/vnd.github+json");
            if let Some(token) = access_token.filter(|token| !token.trim().is_empty()) {
                request = request.bearer_auth(token.trim());
            }

            let response = request
                .send()
                .await
                .context("failed to call GitHub repository metadata API")?;
            let response = ensure_json_success(response, "GitHub repository metadata").await?;
            let document = response
                .json::<GitHubRepositoryDocument>()
                .await
                .context("failed to decode GitHub repository metadata response")?;
            Ok(GitRepositoryMetadata {
                canonical_id: document
                    .full_name
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                display_name: document
                    .full_name
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                visibility: github_visibility(&document),
                write_permission: github_write_permission(&document),
                default_branch: document.default_branch.clone(),
            })
        })
    }

    fn fetch_gitlab_metadata(
        &self,
        target: &ResolvedGitRepositoryTarget,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        let mut url =
            reqwest::Url::parse(&target.api_base_url).context("invalid GitLab API base URL")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow!("invalid GitLab API base URL path"))?;
            segments.push("projects");
            segments.push(format!("{}/{}", target.namespace, target.repository).as_str());
        }

        run_git_repository_request(async move {
            let client = reqwest::Client::new();
            let mut request = client
                .get(url)
                .header(reqwest::header::USER_AGENT, REQUEST_USER_AGENT);
            if let Some(token) = access_token.filter(|token| !token.trim().is_empty()) {
                request = request.header("PRIVATE-TOKEN", token.trim());
            }

            let response = request
                .send()
                .await
                .context("failed to call GitLab repository metadata API")?;
            let response = ensure_json_success(response, "GitLab repository metadata").await?;
            let document = response
                .json::<GitLabProjectDocument>()
                .await
                .context("failed to decode GitLab repository metadata response")?;
            Ok(GitRepositoryMetadata {
                canonical_id: document
                    .path_with_namespace
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                display_name: document
                    .path_with_namespace
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                visibility: gitlab_visibility(&document),
                write_permission: gitlab_write_permission(&document),
                default_branch: document.default_branch.clone(),
            })
        })
    }

    fn fetch_gitee_metadata(
        &self,
        target: &ResolvedGitRepositoryTarget,
        access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        let mut url =
            reqwest::Url::parse(&target.api_base_url).context("invalid Gitee API base URL")?;
        {
            let mut segments = url
                .path_segments_mut()
                .map_err(|_| anyhow!("invalid Gitee API base URL path"))?;
            segments.push("repos");
            segments.push(target.namespace.as_str());
            segments.push(target.repository.as_str());
        }

        run_git_repository_request(async move {
            let client = reqwest::Client::new();
            let mut request = client
                .get(url)
                .header(reqwest::header::USER_AGENT, REQUEST_USER_AGENT);
            if let Some(token) = access_token.filter(|token| !token.trim().is_empty()) {
                request = request.query(&[("access_token", token.trim())]);
            }

            let response = request
                .send()
                .await
                .context("failed to call Gitee repository metadata API")?;
            let response = ensure_json_success(response, "Gitee repository metadata").await?;
            let document = response
                .json::<GiteeRepositoryDocument>()
                .await
                .context("failed to decode Gitee repository metadata response")?;
            Ok(GitRepositoryMetadata {
                canonical_id: document
                    .full_name
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                display_name: document
                    .full_name
                    .clone()
                    .unwrap_or_else(|| target.display_name.clone()),
                visibility: gitee_visibility(&document),
                write_permission: gitee_write_permission(&document),
                default_branch: document.default_branch.clone(),
            })
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedGitRepositoryTarget {
    host_kind: GitHostKind,
    base_url: String,
    api_base_url: String,
    namespace: String,
    repository: String,
    display_name: String,
}

fn resolve_repository_target(
    remote: &BootstrapRemoteConfig,
) -> Result<ResolvedGitRepositoryTarget> {
    let BootstrapRemoteLocator::GitRepo {
        host_kind,
        remote_url,
        base_url,
        api_base_url,
        namespace,
        repository,
        display_name,
        ..
    } = &remote.locator
    else {
        return Err(anyhow!(
            "bootstrap remote `{}` is missing a Git repo locator",
            remote.remote_id
        ));
    };

    let (parsed_namespace, parsed_repository) =
        parse_namespace_and_repository(*host_kind, remote_url)?;
    let namespace = namespace
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or(parsed_namespace)
        .ok_or_else(|| anyhow!("missing Git repository namespace"))?;
    let repository = repository
        .clone()
        .filter(|value| !value.trim().is_empty())
        .or(parsed_repository)
        .ok_or_else(|| anyhow!("missing Git repository name"))?;
    let base_url = base_url
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_base_url(*host_kind).to_string());
    let api_base_url = api_base_url
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default_api_base_url(*host_kind, &base_url));
    let display_name = display_name
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{namespace}/{repository}"));

    Ok(ResolvedGitRepositoryTarget {
        host_kind: *host_kind,
        base_url,
        api_base_url,
        namespace,
        repository,
        display_name,
    })
}

fn parse_namespace_and_repository(
    host_kind: GitHostKind,
    remote_url: &str,
) -> Result<(Option<String>, Option<String>)> {
    let remote_url = remote_url.trim();
    if remote_url.is_empty() {
        return Ok((None, None));
    }

    let path = if let Some(url) = remote_url
        .strip_prefix("https://")
        .or_else(|| remote_url.strip_prefix("http://"))
    {
        url.split_once('/').map(|(_, path)| path.to_string())
    } else if let Some((_host, path)) = remote_url
        .strip_prefix("git@")
        .and_then(|value| value.split_once(':'))
    {
        Some(path.to_string())
    } else {
        None
    };

    let Some(path) = path else {
        return Ok((None, None));
    };
    let trimmed = path.trim_matches('/').trim_end_matches(".git");
    let mut segments = trimmed.split('/').collect::<Vec<_>>();
    if segments.len() < 2 {
        return Ok((None, None));
    }
    let repository = segments.pop().map(str::to_string);
    let namespace = Some(segments.join("/"));

    if matches!(host_kind, GitHostKind::Generic) {
        return Ok((namespace, repository));
    }

    Ok((namespace, repository))
}

fn default_base_url(host_kind: GitHostKind) -> &'static str {
    match host_kind {
        GitHostKind::GitHub => GITHUB_BASE_URL,
        GitHostKind::GitLab => GITLAB_BASE_URL,
        GitHostKind::Gitee | GitHostKind::Generic => GITEE_BASE_URL,
    }
}

fn default_api_base_url(host_kind: GitHostKind, base_url: &str) -> String {
    match host_kind {
        GitHostKind::GitHub => GITHUB_API_BASE_URL.into(),
        GitHostKind::GitLab => {
            format!("{}/{}", base_url.trim_end_matches('/'), GITLAB_API_PATH)
        }
        GitHostKind::Gitee | GitHostKind::Generic => {
            format!("{}/{}", base_url.trim_end_matches('/'), GITEE_API_PATH)
        }
    }
}

fn github_visibility(document: &GitHubRepositoryDocument) -> GitRepositoryVisibility {
    match document.visibility.as_deref() {
        Some("private") => GitRepositoryVisibility::Private,
        Some("public") => GitRepositoryVisibility::Public,
        Some("internal") => GitRepositoryVisibility::Internal,
        Some(_) => GitRepositoryVisibility::Unknown,
        None => match document.private {
            Some(true) => GitRepositoryVisibility::Private,
            Some(false) => GitRepositoryVisibility::Public,
            None => GitRepositoryVisibility::Unknown,
        },
    }
}

fn github_write_permission(document: &GitHubRepositoryDocument) -> GitRepositoryWritePermission {
    match &document.permissions {
        Some(permissions) if permissions.push || permissions.admin || permissions.maintain => {
            GitRepositoryWritePermission::Writable
        }
        Some(_) => GitRepositoryWritePermission::ReadOnly,
        None => GitRepositoryWritePermission::Unknown,
    }
}

fn gitlab_visibility(document: &GitLabProjectDocument) -> GitRepositoryVisibility {
    match document.visibility.as_deref() {
        Some("private") => GitRepositoryVisibility::Private,
        Some("public") => GitRepositoryVisibility::Public,
        Some("internal") => GitRepositoryVisibility::Internal,
        _ => GitRepositoryVisibility::Unknown,
    }
}

fn gitlab_write_permission(document: &GitLabProjectDocument) -> GitRepositoryWritePermission {
    let Some(permissions) = &document.permissions else {
        return GitRepositoryWritePermission::Unknown;
    };
    let project_level = permissions
        .project_access
        .as_ref()
        .and_then(|access| access.access_level);
    let group_level = permissions
        .group_access
        .as_ref()
        .and_then(|access| access.access_level);
    let max_level = project_level.into_iter().chain(group_level).max();
    match max_level {
        Some(level) if level >= 30 => GitRepositoryWritePermission::Writable,
        Some(_) => GitRepositoryWritePermission::ReadOnly,
        None => GitRepositoryWritePermission::Unknown,
    }
}

fn gitee_visibility(document: &GiteeRepositoryDocument) -> GitRepositoryVisibility {
    if document.internal.unwrap_or(false) {
        return GitRepositoryVisibility::Internal;
    }
    match (document.private, document.public) {
        (Some(true), _) => GitRepositoryVisibility::Private,
        (Some(false), _) | (_, Some(true)) => GitRepositoryVisibility::Public,
        (_, Some(false)) => GitRepositoryVisibility::Private,
        _ => GitRepositoryVisibility::Unknown,
    }
}

fn gitee_write_permission(document: &GiteeRepositoryDocument) -> GitRepositoryWritePermission {
    match document.can_push {
        Some(true) => GitRepositoryWritePermission::Writable,
        Some(false) => GitRepositoryWritePermission::ReadOnly,
        None => GitRepositoryWritePermission::Unknown,
    }
}

fn run_git_repository_request<T>(
    future: impl std::future::Future<Output = Result<T>>,
) -> Result<T> {
    let runtime = tokio::runtime::Runtime::new()
        .context("failed to create tokio runtime for git repository metadata request")?;
    runtime.block_on(future)
}

async fn ensure_json_success(
    response: reqwest::Response,
    operation: &str,
) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| String::new());
    if !body.trim().is_empty() {
        return Err(anyhow!("{operation} failed with {status}: {body}"));
    }

    Err(anyhow!("{operation} failed with {status}"))
}

pub fn fetch_repository_metadata(
    remote: &BootstrapRemoteConfig,
    source: &dyn GitRepositoryMetadataSource,
    access_token: Option<&str>,
) -> Result<GitRepositoryMetadata> {
    ensure!(
        remote.provider == ProviderKind::GitRepo,
        "bootstrap remote `{}` is not a Git repository sync remote",
        remote.remote_id
    );
    let BootstrapRemoteLocator::GitRepo { .. } = &remote.locator else {
        return Err(anyhow!(
            "bootstrap remote `{}` is missing a Git repo locator",
            remote.remote_id
        ));
    };

    source.fetch_repository_metadata(remote, access_token)
}

pub fn ensure_private_repository(metadata: &GitRepositoryMetadata) -> Result<()> {
    match metadata.visibility {
        GitRepositoryVisibility::Private => Ok(()),
        GitRepositoryVisibility::Internal => Err(anyhow!(
            "remote repository `{}` is internal; only private repositories may enable sync",
            metadata.display_name
        )),
        GitRepositoryVisibility::Public => Err(anyhow!(
            "remote repository `{}` must stay private before sync can be enabled",
            metadata.display_name
        )),
        GitRepositoryVisibility::Unknown => Err(anyhow!(
            "remote repository visibility could not be confirmed; refusing to enable sync without verified private visibility"
        )),
    }
}

pub fn ensure_writable(metadata: &GitRepositoryMetadata) -> Result<()> {
    match metadata.write_permission {
        GitRepositoryWritePermission::Writable => Ok(()),
        GitRepositoryWritePermission::ReadOnly => Err(anyhow!(
            "remote repository `{}` does not grant write permission with the supplied token",
            metadata.display_name
        )),
        GitRepositoryWritePermission::Unknown => Err(anyhow!(
            "remote repository write permission could not be confirmed; refusing to enable sync"
        )),
    }
}

pub fn validate_remote_for_sync(
    remote: &BootstrapRemoteConfig,
    source: &dyn GitRepositoryMetadataSource,
    access_token: Option<&str>,
) -> Result<GitRepositoryMetadata> {
    let metadata = fetch_repository_metadata(remote, source, access_token)?;
    ensure_private_repository(&metadata)?;
    ensure_writable(&metadata)?;
    Ok(metadata)
}

pub fn validate_remote_for_push(
    remote: &BootstrapRemoteConfig,
    safety_status: GitRemoteSafetyStatus,
    source: &dyn GitRepositoryMetadataSource,
    access_token: Option<&str>,
) -> Result<GitRepositoryMetadata> {
    match safety_status {
        GitRemoteSafetyStatus::Paused => Err(anyhow!(
            "remote sync is paused until the repository is revalidated as private and writable"
        )),
        GitRemoteSafetyStatus::Safe
        | GitRemoteSafetyStatus::Unknown
        | GitRemoteSafetyStatus::Stale => validate_remote_for_sync(remote, source, access_token),
    }
}

pub fn validate_first_release_git_host(host_kind: GitHostKind) -> Result<()> {
    if !matches!(
        host_kind,
        GitHostKind::Gitee | GitHostKind::GitHub | GitHostKind::GitLab
    ) {
        return Err(anyhow!(
            "only Gitee, GitHub, and GitLab Git primary remotes are supported"
        ));
    }

    Ok(())
}

pub struct GitRepoProvider {
    config: GitRepoProviderConfig,
}

impl GitRepoProvider {
    pub fn new(config: GitRepoProviderConfig) -> Result<Self> {
        ensure!(
            !config.remote_id.trim().is_empty(),
            "Git repo remote_id must not be empty"
        );
        ensure!(
            !config.branch.trim().is_empty(),
            "Git repo branch must not be empty"
        );
        ensure!(
            !config.remote_url.trim().is_empty(),
            "Git repo remote URL must not be empty"
        );

        Ok(Self { config })
    }

    pub fn config(&self) -> &GitRepoProviderConfig {
        &self.config
    }

    fn managed_root_relative(&self) -> PathBuf {
        PathBuf::from(
            self.config
                .root_path
                .as_deref()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or(DEFAULT_SYNC_ROOT_PATH),
        )
    }

    fn managed_root_absolute(&self) -> PathBuf {
        self.config.cache_dir.join(self.managed_root_relative())
    }

    fn managed_head_relative(&self) -> PathBuf {
        self.managed_root_relative().join(HEAD_FILE_NAME)
    }

    fn revisions_root_relative(&self) -> PathBuf {
        self.managed_root_relative().join(REVISIONS_DIR_NAME)
    }

    fn revisions_root_absolute(&self) -> PathBuf {
        self.config.cache_dir.join(self.revisions_root_relative())
    }

    fn revision_dir_relative(&self, revision: &str) -> PathBuf {
        self.revisions_root_relative().join(revision)
    }

    fn revision_dir_absolute(&self, revision: &str) -> PathBuf {
        self.config
            .cache_dir
            .join(self.revision_dir_relative(revision))
    }

    fn revision_head_relative(&self, revision: &str) -> PathBuf {
        self.revision_dir_relative(revision).join(HEAD_FILE_NAME)
    }

    fn revision_manifest_relative(&self, revision: &str) -> PathBuf {
        self.revision_dir_relative(revision)
            .join(MANIFEST_FILE_NAME)
    }

    fn revision_snapshot_relative(&self, revision: &str) -> PathBuf {
        self.revision_dir_relative(revision)
            .join(SNAPSHOT_FILE_NAME)
    }

    fn revision_file_paths_relative(&self, revision: &str) -> [PathBuf; 3] {
        [
            self.revision_head_relative(revision),
            self.revision_manifest_relative(revision),
            self.revision_snapshot_relative(revision),
        ]
    }

    fn ensure_repo_ready(&self) -> Result<Repository> {
        let repo = if self.config.cache_dir.join(".git").exists() {
            Repository::open(self.config.cache_dir.as_path()).with_context(|| {
                format!(
                    "failed to open git repo cache `{}`",
                    self.config.cache_dir.display()
                )
            })?
        } else {
            fs::create_dir_all(&self.config.cache_dir).with_context(|| {
                format!(
                    "failed to create git repo cache `{}`",
                    self.config.cache_dir.display()
                )
            })?;
            let mut options = RepositoryInitOptions::new();
            options.initial_head(self.config.branch.as_str());
            Repository::init_opts(self.config.cache_dir.as_path(), &options).with_context(|| {
                format!(
                    "failed to initialize git repo cache `{}`",
                    self.config.cache_dir.display()
                )
            })?
        };

        self.ensure_remote(&repo)?;
        self.configure_repo(&repo)?;
        Ok(repo)
    }

    fn configure_repo(&self, repo: &Repository) -> Result<()> {
        let mut config = repo.config().context("failed to open git repo config")?;
        config
            .set_str("user.name", COMMITTER_NAME)
            .context("failed to configure git repo committer name")?;
        config
            .set_str("user.email", COMMITTER_EMAIL)
            .context("failed to configure git repo committer email")?;
        Ok(())
    }

    fn ensure_remote(&self, repo: &Repository) -> Result<()> {
        match repo.find_remote(REMOTE_NAME) {
            Ok(remote) => {
                if remote.url() != Some(self.config.remote_url.as_str()) {
                    repo.remote_set_url(REMOTE_NAME, self.config.remote_url.as_str())
                        .with_context(|| {
                            format!(
                                "failed to update git remote URL for `{}`",
                                self.config.remote_id
                            )
                        })?;
                }
                self.configure_remote_fetch_spec(repo)
            }
            Err(_) => {
                repo.remote(REMOTE_NAME, self.config.remote_url.as_str())
                    .map(|_| ())
                    .with_context(|| {
                        format!(
                            "failed to create git remote `{}` for `{}`",
                            REMOTE_NAME, self.config.remote_id
                        )
                    })?;
                self.configure_remote_fetch_spec(repo)
            }
        }
    }

    fn configure_remote_fetch_spec(&self, repo: &Repository) -> Result<()> {
        let mut config = repo.config().context("failed to open git repo config")?;
        let fetch_key = format!("remote.{REMOTE_NAME}.fetch");
        config
            .set_str(
                fetch_key.as_str(),
                format!("+{}", self.fetch_refspec()).as_str(),
            )
            .with_context(|| format!("failed to configure `{fetch_key}`"))?;
        Ok(())
    }

    fn remote_callbacks(&self) -> RemoteCallbacks<'static> {
        let auth = self.config.auth.clone();
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(move |_url, username_from_url, allowed| match &auth {
            GitTransportAuthPlan::HttpsCredentials { username, secret } => {
                if allowed.contains(CredentialType::USERNAME)
                    && !allowed.contains(CredentialType::USER_PASS_PLAINTEXT)
                {
                    Cred::username(username.as_str())
                } else {
                    Cred::userpass_plaintext(username.as_str(), secret.as_str())
                }
            }
            GitTransportAuthPlan::SshKey {
                private_key,
                passphrase,
            } => {
                let username = username_from_url.unwrap_or("git");
                if allowed.contains(CredentialType::USERNAME)
                    && !allowed.contains(CredentialType::SSH_KEY)
                {
                    Cred::username(username)
                } else {
                    Cred::ssh_key_from_memory(
                        username,
                        None,
                        private_key.as_str(),
                        passphrase.as_deref(),
                    )
                }
            }
        });
        callbacks.push_update_reference(|_reference, status| match status {
            Some(status) => Err(git2::Error::from_str(status)),
            None => Ok(()),
        });
        callbacks
    }

    fn fetch_refspec(&self) -> String {
        format!(
            "refs/heads/{}:refs/remotes/{}/{}",
            self.config.branch, REMOTE_NAME, self.config.branch
        )
    }

    fn local_branch_ref(&self) -> String {
        format!("refs/heads/{}", self.config.branch)
    }

    fn fetch_remote_branch(&self, repo: &Repository) -> Result<bool> {
        let mut remote = repo.find_remote(REMOTE_NAME).with_context(|| {
            format!(
                "failed to open git remote `{}` for `{}`",
                REMOTE_NAME, self.config.remote_id
            )
        })?;
        let mut options = FetchOptions::new();
        options.remote_callbacks(self.remote_callbacks());
        match remote.fetch::<&str>(&[], Some(&mut options), None) {
            Ok(()) => {
                if repo.refname_to_id(self.tracking_ref().as_str()).is_ok() {
                    return Ok(true);
                }
                match self.lookup_fetched_branch_oid(repo)? {
                    Some(remote_oid) => {
                        repo.reference(
                            self.tracking_ref().as_str(),
                            remote_oid,
                            true,
                            "mica-term update git primary tracking ref",
                        )
                        .context("failed to update Git tracking ref")?;
                        Ok(true)
                    }
                    None => Ok(false),
                }
            }
            Err(err) if is_missing_remote_branch_error(&err) => Ok(false),
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to fetch branch `{}` from `{}`",
                    self.config.branch, self.config.remote_url
                )
            }),
        }
    }

    fn lookup_fetched_branch_oid(&self, repo: &Repository) -> Result<Option<Oid>> {
        let target_ref = format!("refs/heads/{}", self.config.branch);
        let mut branch_oid = None;
        repo.fetchhead_foreach(|reference_name, _remote_url, oid, _is_merge| {
            if reference_name == target_ref {
                branch_oid = Some(*oid);
                false
            } else {
                true
            }
        })
        .context("failed to inspect Git FETCH_HEAD")?;
        Ok(branch_oid)
    }

    fn tracking_ref(&self) -> String {
        format!("refs/remotes/{}/{}", REMOTE_NAME, self.config.branch)
    }

    fn resolve_commit<'repo>(
        &self,
        repo: &'repo Repository,
        git_ref: &str,
    ) -> Result<git2::Commit<'repo>> {
        repo.revparse_single(git_ref)
            .with_context(|| format!("failed to resolve git revision `{git_ref}`"))?
            .peel_to_commit()
            .with_context(|| format!("failed to peel git revision `{git_ref}` to a commit"))
    }

    fn read_head_from_ref(&self, repo: &Repository, git_ref: &str) -> Result<VaultHead> {
        let commit = self.resolve_commit(repo, git_ref)?;
        self.read_head_from_commit(repo, &commit)
    }

    fn read_head_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
    ) -> Result<VaultHead> {
        let bytes = match self.maybe_read_file_from_commit(
            repo,
            commit,
            self.managed_head_relative().as_path(),
        )? {
            Some(bytes) => bytes,
            None => self.read_file_from_commit(repo, commit, Path::new(HEAD_FILE_NAME))?,
        };
        serde_json::from_slice(bytes.as_slice()).context("failed to decode git repo vault head")
    }

    fn read_managed_revision_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
        revision: &str,
    ) -> Result<ProviderRevision> {
        let head: VaultHead = serde_json::from_slice(
            self.read_file_from_commit(
                repo,
                commit,
                self.revision_head_relative(revision).as_path(),
            )?
            .as_slice(),
        )
        .context("failed to decode git repo retained vault head")?;
        let manifest: VaultManifest = bincode::deserialize(
            self.read_file_from_commit(
                repo,
                commit,
                self.revision_manifest_relative(revision).as_path(),
            )?
            .as_slice(),
        )
        .context("failed to decode git repo vault manifest")?;
        let encrypted_snapshot = rebuild_snapshot_from_manifest(
            &head,
            &manifest,
            self.read_file_from_commit(
                repo,
                commit,
                self.revision_snapshot_relative(revision).as_path(),
            )?,
        )?;

        Ok(ProviderRevision {
            head,
            manifest,
            encrypted_snapshot,
        })
    }

    fn read_legacy_revision_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
    ) -> Result<ProviderRevision> {
        let head = self.read_head_from_commit(repo, commit)?;
        let manifest: VaultManifest = bincode::deserialize(
            self.read_file_from_commit(repo, commit, Path::new(MANIFEST_FILE_NAME))?
                .as_slice(),
        )
        .context("failed to decode git repo legacy vault manifest")?;
        let encrypted_snapshot = rebuild_snapshot_from_manifest(
            &head,
            &manifest,
            self.read_file_from_commit(repo, commit, Path::new(SNAPSHOT_FILE_NAME))?,
        )?;

        Ok(ProviderRevision {
            head,
            manifest,
            encrypted_snapshot,
        })
    }

    fn maybe_read_file_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
        relative_path: &Path,
    ) -> Result<Option<Vec<u8>>> {
        let tree = commit.tree().context("failed to load git commit tree")?;
        let entry = match tree.get_path(relative_path) {
            Ok(entry) => entry,
            Err(_) => return Ok(None),
        };
        let blob = repo.find_blob(entry.id()).with_context(|| {
            format!(
                "failed to load `{}` blob from commit `{}`",
                relative_path.display(),
                commit.id()
            )
        })?;
        Ok(Some(blob.content().to_vec()))
    }

    fn read_file_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
        relative_path: &Path,
    ) -> Result<Vec<u8>> {
        self.maybe_read_file_from_commit(repo, commit, relative_path)?
            .ok_or_else(|| {
                anyhow!(
                    "git repo remote `{}` is missing `{}` in commit `{}`",
                    self.config.remote_id,
                    relative_path.display(),
                    commit.id(),
                )
            })
    }

    fn find_commit_for_revision(&self, repo: &Repository, revision: &str) -> Result<Oid> {
        let tracking_ref = self.tracking_ref();
        let mut walk = repo
            .revwalk()
            .context("failed to create Git revision walk")?;
        walk.push_ref(tracking_ref.as_str()).with_context(|| {
            format!(
                "failed to walk tracking ref `{}` for `{}`",
                tracking_ref, self.config.remote_id
            )
        })?;

        for oid in walk {
            let oid = oid.context("failed to iterate Git revision walk")?;
            let commit = match repo.find_commit(oid) {
                Ok(commit) => commit,
                Err(_) => continue,
            };
            let candidate = match self.read_head_from_commit(repo, &commit) {
                Ok(candidate) => candidate,
                Err(_) => continue,
            };
            if candidate.vault_revision == revision {
                return Ok(oid);
            }
        }

        Err(anyhow!(
            "git repo remote `{}` is missing revision `{revision}`",
            self.config.remote_id
        ))
    }

    fn checkout_tracking_branch(&self, repo: &Repository) -> Result<()> {
        let tracking_ref = self.tracking_ref();
        let tracking_oid = repo
            .refname_to_id(tracking_ref.as_str())
            .with_context(|| format!("failed to resolve tracking ref `{tracking_ref}`"))?;
        repo.reference(
            self.local_branch_ref().as_str(),
            tracking_oid,
            true,
            "mica-term align local git cache with remote tracking branch",
        )
        .context("failed to update local Git branch from tracking ref")?;
        repo.set_head(self.local_branch_ref().as_str())
            .context("failed to point HEAD at local Git branch")?;
        let mut checkout = CheckoutBuilder::new();
        checkout.force();
        repo.checkout_head(Some(&mut checkout))
            .context("failed to checkout tracked Git branch into cache worktree")?;
        Ok(())
    }

    fn prepare_worktree_for_write(&self, repo: &Repository, has_remote_branch: bool) -> Result<()> {
        if has_remote_branch {
            self.checkout_tracking_branch(repo)?;
        } else {
            clear_managed_root(self.managed_root_absolute().as_path())?;
        }
        Ok(())
    }

    fn write_revision_files(&self, request: &ProviderWriteRequest) -> Result<Vec<PathBuf>> {
        let managed_root = self.managed_root_absolute();
        let revision_dir = self.revision_dir_absolute(request.head.vault_revision.as_str());
        fs::create_dir_all(&revision_dir).with_context(|| {
            format!(
                "failed to create git repo revision directory `{}`",
                revision_dir.display()
            )
        })?;

        let managed_head_path = managed_root.join(HEAD_FILE_NAME);
        let retained_head_path = revision_dir.join(HEAD_FILE_NAME);
        let retained_manifest_path = revision_dir.join(MANIFEST_FILE_NAME);
        let retained_snapshot_path = revision_dir.join(SNAPSHOT_FILE_NAME);

        fs::write(
            &managed_head_path,
            serde_json::to_vec_pretty(&request.head).context("encode git repo vault head")?,
        )
        .with_context(|| {
            format!(
                "failed to write git repo head into `{}`",
                managed_head_path.display()
            )
        })?;
        fs::write(
            &retained_head_path,
            serde_json::to_vec_pretty(&request.head)
                .context("encode retained git repo vault head")?,
        )
        .with_context(|| {
            format!(
                "failed to write retained git repo head into `{}`",
                retained_head_path.display()
            )
        })?;
        fs::write(
            &retained_manifest_path,
            bincode::serialize(&request.manifest).context("encode git repo vault manifest")?,
        )
        .with_context(|| {
            format!(
                "failed to write git repo manifest into `{}`",
                retained_manifest_path.display()
            )
        })?;
        fs::write(
            &retained_snapshot_path,
            request.encrypted_snapshot.ciphertext.as_slice(),
        )
        .with_context(|| {
            format!(
                "failed to write git repo snapshot into `{}`",
                retained_snapshot_path.display()
            )
        })?;

        Ok(vec![
            self.managed_head_relative(),
            self.revision_head_relative(request.head.vault_revision.as_str()),
            self.revision_manifest_relative(request.head.vault_revision.as_str()),
            self.revision_snapshot_relative(request.head.vault_revision.as_str()),
        ])
    }

    fn remove_legacy_root_files(&self) -> Result<Vec<PathBuf>> {
        let legacy_paths = [
            PathBuf::from(HEAD_FILE_NAME),
            PathBuf::from(MANIFEST_FILE_NAME),
            PathBuf::from(SNAPSHOT_FILE_NAME),
        ];
        let mut removed_paths = Vec::new();
        for relative_path in legacy_paths {
            let absolute_path = self.config.cache_dir.join(&relative_path);
            if absolute_path.exists() {
                fs::remove_file(&absolute_path).with_context(|| {
                    format!(
                        "failed to remove legacy git sync file `{}`",
                        absolute_path.display()
                    )
                })?;
                removed_paths.push(relative_path);
            }
        }
        Ok(removed_paths)
    }

    fn commit_paths(
        &self,
        repo: &Repository,
        message: &str,
        staged_paths: &[PathBuf],
        removed_paths: &[PathBuf],
    ) -> Result<()> {
        let mut index = repo.index().context("failed to open Git index")?;
        for path in removed_paths {
            if let Err(err) = index.remove_path(path.as_path()) {
                if err.code() != git2::ErrorCode::NotFound {
                    return Err(err).with_context(|| {
                        format!("failed to stage removed Git path `{}`", path.display())
                    });
                }
            }
        }
        for path in staged_paths {
            index
                .add_path(path.as_path())
                .with_context(|| format!("failed to stage git repo path `{}`", path.display()))?;
        }
        index.write().context("failed to persist Git index")?;
        let tree_oid = index.write_tree().context("failed to write Git tree")?;
        let tree = repo
            .find_tree(tree_oid)
            .context("failed to load Git tree")?;
        let signature = Signature::now(COMMITTER_NAME, COMMITTER_EMAIL)
            .context("failed to build Git signature")?;
        let parent_commit = match repo.find_reference(self.local_branch_ref().as_str()) {
            Ok(reference) => Some(
                reference
                    .peel_to_commit()
                    .context("failed to open local Git parent commit")?,
            ),
            Err(err) if err.code() == git2::ErrorCode::NotFound => None,
            Err(err) => {
                return Err(err).context("failed to resolve local Git parent branch reference");
            }
        };
        let parents = parent_commit.iter().collect::<Vec<_>>();
        repo.commit(
            Some(self.local_branch_ref().as_str()),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )
        .context("failed to create Git commit")?;
        repo.set_head(self.local_branch_ref().as_str())
            .context("failed to point HEAD at local Git branch")?;
        Ok(())
    }

    fn commit_workspace(&self, repo: &Repository, request: &ProviderWriteRequest) -> Result<()> {
        let mut removed_paths = self.remove_legacy_root_files()?;
        let staged_paths = self.write_revision_files(request)?;
        let commit_message = format!("vault-revision: {}", request.head.vault_revision);
        self.commit_paths(repo, commit_message.as_str(), &staged_paths, &removed_paths)?;
        removed_paths.clear();
        Ok(())
    }

    fn list_retained_revisions(&self) -> Result<Vec<String>> {
        let revisions_root = self.revisions_root_absolute();
        if !revisions_root.exists() {
            return Ok(Vec::new());
        }

        let mut revisions = Vec::new();
        for entry in fs::read_dir(&revisions_root).with_context(|| {
            format!(
                "failed to enumerate git repo revisions root `{}`",
                revisions_root.display()
            )
        })? {
            let entry = entry?;
            if !entry.path().is_dir() {
                continue;
            }
            let file_name = entry.file_name();
            let Some(revision) = file_name.to_str() else {
                continue;
            };
            if revision.trim().is_empty() {
                continue;
            }
            revisions.push(revision.to_string());
        }
        Ok(revisions)
    }

    fn prune_revision_directories(
        &self,
        retained: &std::collections::BTreeSet<String>,
    ) -> Result<Vec<PathBuf>> {
        let mut removed_paths = Vec::new();
        for revision in self.list_retained_revisions()? {
            if retained.contains(revision.as_str()) {
                continue;
            }
            for path in self.revision_file_paths_relative(revision.as_str()) {
                removed_paths.push(path);
            }
            let revision_dir = self.revision_dir_absolute(revision.as_str());
            if revision_dir.exists() {
                fs::remove_dir_all(&revision_dir).with_context(|| {
                    format!(
                        "failed to remove retained git repo revision directory `{}`",
                        revision_dir.display()
                    )
                })?;
            }
        }
        Ok(removed_paths)
    }

    fn retained_revision_ids(
        &self,
        keep_latest: usize,
        live_revision: &str,
    ) -> Result<std::collections::BTreeSet<String>> {
        let mut revisions = self.list_retained_revisions()?;
        revisions.sort();
        revisions.reverse();
        let mut retained = revisions
            .into_iter()
            .take(keep_latest)
            .collect::<std::collections::BTreeSet<_>>();
        retained.insert(live_revision.to_string());
        Ok(retained)
    }

    fn push_local_branch(&self, repo: &Repository) -> Result<()> {
        let mut remote = repo.find_remote(REMOTE_NAME).with_context(|| {
            format!(
                "failed to open git remote `{}` for `{}`",
                REMOTE_NAME, self.config.remote_id
            )
        })?;
        let mut options = PushOptions::new();
        options.remote_callbacks(self.remote_callbacks());
        let refspec = self.local_branch_ref();
        match remote.push(&[refspec.as_str()], Some(&mut options)) {
            Ok(()) => Ok(()),
            Err(err) if is_non_fast_forward_error(&err) => Err(anyhow!(
                "non-fast-forward push rejected for git remote `{}`",
                self.config.remote_id
            )),
            Err(err) => Err(err).with_context(|| {
                format!(
                    "failed to push branch `{}` to `{}`",
                    self.config.branch, self.config.remote_url
                )
            }),
        }
    }
}

impl VaultProvider for GitRepoProvider {
    fn remote_id(&self) -> &str {
        self.config.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities::git_repo_primary()
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        let repo = self.ensure_repo_ready()?;
        if !self.fetch_remote_branch(&repo)? {
            return Ok(ProviderReadResult::default());
        }

        Ok(ProviderReadResult {
            head: Some(self.read_head_from_ref(&repo, self.tracking_ref().as_str())?),
        })
    }

    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision> {
        let repo = self.ensure_repo_ready()?;
        if !self.fetch_remote_branch(&repo)? {
            return Err(anyhow!(
                "git repo remote `{}` is empty",
                self.config.remote_id
            ));
        }
        let tracking_commit = self.resolve_commit(&repo, self.tracking_ref().as_str())?;
        if self
            .maybe_read_file_from_commit(
                &repo,
                &tracking_commit,
                self.managed_head_relative().as_path(),
            )?
            .is_some()
        {
            return self.read_managed_revision_from_commit(
                &repo,
                &tracking_commit,
                head.vault_revision.as_str(),
            );
        }

        let commit = self.find_commit_for_revision(&repo, head.vault_revision.as_str())?;
        let commit = repo
            .find_commit(commit)
            .context("failed to open legacy Git revision commit")?;
        self.read_legacy_revision_from_commit(&repo, &commit)
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let repo = self.ensure_repo_ready()?;
        let has_remote_branch = self.fetch_remote_branch(&repo)?;
        let remote_head = if has_remote_branch {
            Some(self.read_head_from_ref(&repo, self.tracking_ref().as_str())?)
        } else {
            None
        };
        let actual_parent = remote_head.as_ref().map(|head| head.vault_revision.clone());
        if request.expected_parent_revision != actual_parent {
            return Err(anyhow!(
                "non-fast-forward push rejected for git remote `{}`: expected {:?}, found {:?}",
                self.config.remote_id,
                request.expected_parent_revision,
                actual_parent
            ));
        }

        self.prepare_worktree_for_write(&repo, has_remote_branch)?;
        self.commit_workspace(&repo, request)?;
        self.push_local_branch(&repo)
    }

    fn prune_revisions(&self, keep_latest: usize, live_head: &VaultHead) -> Result<()> {
        let repo = self.ensure_repo_ready()?;
        if !self.fetch_remote_branch(&repo)? {
            return Ok(());
        }
        self.checkout_tracking_branch(&repo)?;
        if self
            .maybe_read_file_from_commit(
                &repo,
                &self.resolve_commit(&repo, self.tracking_ref().as_str())?,
                self.managed_head_relative().as_path(),
            )?
            .is_none()
        {
            return Ok(());
        }

        let retained =
            self.retained_revision_ids(keep_latest, live_head.vault_revision.as_str())?;
        let removed_paths = self.prune_revision_directories(&retained)?;
        if removed_paths.is_empty() {
            return Ok(());
        }
        self.commit_paths(
            &repo,
            format!("vault-retention: {}", live_head.vault_revision).as_str(),
            &[],
            &removed_paths,
        )?;
        self.push_local_branch(&repo)
    }
}

fn decode_inline_credentials(
    raw: Option<&str>,
    auth_kind: ProviderAuthKind,
) -> InlineGitCredentialMaterial {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return InlineGitCredentialMaterial::default();
    };

    serde_json::from_str::<InlineGitCredentialMaterial>(raw).unwrap_or_else(|_| match auth_kind {
        ProviderAuthKind::SshKey => InlineGitCredentialMaterial {
            ssh_private_key: raw.to_string(),
            ..InlineGitCredentialMaterial::default()
        },
        _ => InlineGitCredentialMaterial {
            https_secret: raw.to_string(),
            ..InlineGitCredentialMaterial::default()
        },
    })
}

fn clear_managed_root(root: &Path) -> Result<()> {
    if root.exists() {
        fs::remove_dir_all(root)
            .with_context(|| format!("failed to remove `{}`", root.display()))?;
    }
    Ok(())
}

fn is_non_fast_forward_error(err: &git2::Error) -> bool {
    let message = err.message().to_ascii_lowercase();
    message.contains("non-fast-forward")
        || message.contains("non-fastforward")
        || message.contains("fetch first")
        || message.contains("reference update failed")
        || message.contains("failed to push some refs")
}

fn is_missing_remote_branch_error(err: &git2::Error) -> bool {
    let message = err.message().to_ascii_lowercase();
    message.contains("couldn't find remote ref")
        || message.contains("remote ref does not exist")
        || message.contains("remote branch not found")
        || message.contains("reference not found")
        || message.contains("not our ref")
}
