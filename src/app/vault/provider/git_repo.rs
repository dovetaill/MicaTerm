use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, ensure};
use git2::{
    Cred, CredentialType, FetchOptions, Oid, PushOptions, RemoteCallbacks, Repository,
    RepositoryInitOptions, Signature,
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
const COMMITTER_NAME: &str = "Mica Term Vault";
const COMMITTER_EMAIL: &str = "vault@mica-term.local";

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
    if host_kind != GitHostKind::Gitee {
        return Err(anyhow!(
            "first release only exposes Gitee Git primary remotes"
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
        let bytes = self.read_file_from_commit(repo, commit, HEAD_FILE_NAME)?;
        serde_json::from_slice(bytes.as_slice()).context("failed to decode git repo vault head")
    }

    fn read_revision_from_ref(&self, repo: &Repository, git_ref: &str) -> Result<ProviderRevision> {
        let commit = self.resolve_commit(repo, git_ref)?;
        let head = self.read_head_from_commit(repo, &commit)?;
        let manifest: VaultManifest = bincode::deserialize(
            self.read_file_from_commit(repo, &commit, MANIFEST_FILE_NAME)?
                .as_slice(),
        )
        .context("failed to decode git repo vault manifest")?;
        let encrypted_snapshot = rebuild_snapshot_from_manifest(
            &head,
            &manifest,
            self.read_file_from_commit(repo, &commit, SNAPSHOT_FILE_NAME)?,
        )?;

        Ok(ProviderRevision {
            head,
            manifest,
            encrypted_snapshot,
        })
    }

    fn read_file_from_commit(
        &self,
        repo: &Repository,
        commit: &git2::Commit<'_>,
        file_name: &str,
    ) -> Result<Vec<u8>> {
        let tree = commit.tree().context("failed to load git commit tree")?;
        let entry = tree.get_name(file_name).ok_or_else(|| {
            anyhow!(
                "git repo remote `{}` is missing `{file_name}` in commit `{}`",
                self.config.remote_id,
                commit.id()
            )
        })?;
        let blob = repo.find_blob(entry.id()).with_context(|| {
            format!(
                "failed to load `{file_name}` blob from commit `{}`",
                commit.id()
            )
        })?;
        Ok(blob.content().to_vec())
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

    fn write_workspace_files(&self, request: &ProviderWriteRequest) -> Result<()> {
        clear_worktree(self.config.cache_dir.as_path())?;
        fs::write(
            self.config.cache_dir.join(HEAD_FILE_NAME),
            serde_json::to_vec_pretty(&request.head).context("encode git repo vault head")?,
        )
        .with_context(|| {
            format!(
                "failed to write git repo head into `{}`",
                self.config.cache_dir.display()
            )
        })?;
        fs::write(
            self.config.cache_dir.join(MANIFEST_FILE_NAME),
            bincode::serialize(&request.manifest).context("encode git repo vault manifest")?,
        )
        .with_context(|| {
            format!(
                "failed to write git repo manifest into `{}`",
                self.config.cache_dir.display()
            )
        })?;
        fs::write(
            self.config.cache_dir.join(SNAPSHOT_FILE_NAME),
            request.encrypted_snapshot.ciphertext.as_slice(),
        )
        .with_context(|| {
            format!(
                "failed to write git repo snapshot into `{}`",
                self.config.cache_dir.display()
            )
        })?;
        Ok(())
    }

    fn commit_workspace(
        &self,
        repo: &Repository,
        request: &ProviderWriteRequest,
        has_remote_branch: bool,
    ) -> Result<()> {
        let mut index = repo.index().context("failed to open Git index")?;
        index.clear().context("failed to clear Git index")?;
        index
            .add_path(Path::new(HEAD_FILE_NAME))
            .context("failed to stage vault head")?;
        index
            .add_path(Path::new(MANIFEST_FILE_NAME))
            .context("failed to stage vault manifest")?;
        index
            .add_path(Path::new(SNAPSHOT_FILE_NAME))
            .context("failed to stage vault snapshot")?;
        index.write().context("failed to persist Git index")?;
        let tree_oid = index.write_tree().context("failed to write Git tree")?;
        let tree = repo
            .find_tree(tree_oid)
            .context("failed to load Git tree")?;
        let signature = Signature::now(COMMITTER_NAME, COMMITTER_EMAIL)
            .context("failed to build Git signature")?;
        let commit_message = format!("vault-revision: {}", request.head.vault_revision);

        let parent_commit = if has_remote_branch {
            Some(
                repo.find_commit(
                    repo.refname_to_id(self.tracking_ref().as_str())
                        .context("failed to resolve tracking ref to commit")?,
                )
                .context("failed to open parent commit")?,
            )
        } else {
            None
        };
        let parents = parent_commit.iter().collect::<Vec<_>>();
        let commit_oid = repo
            .commit(
                None,
                &signature,
                &signature,
                commit_message.as_str(),
                &tree,
                &parents,
            )
            .context("failed to create Git commit")?;
        repo.reference(
            self.local_branch_ref().as_str(),
            commit_oid,
            true,
            "mica-term update git primary cache",
        )
        .context("failed to update local Git branch ref")?;
        Ok(())
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
        let commit = self.find_commit_for_revision(&repo, head.vault_revision.as_str())?;
        self.read_revision_from_ref(&repo, commit.to_string().as_str())
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

        self.write_workspace_files(request)?;
        self.commit_workspace(&repo, request, has_remote_branch)?;
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

fn clear_worktree(root: &Path) -> Result<()> {
    for entry in
        fs::read_dir(root).with_context(|| format!("failed to enumerate `{}`", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if entry.file_name() == ".git" {
            continue;
        }
        if path.is_dir() {
            fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove `{}`", path.display()))?;
        } else {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove `{}`", path.display()))?;
        }
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
