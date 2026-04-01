use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, anyhow, ensure};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepoProviderConfig {
    pub remote_id: String,
    pub host_kind: GitHostKind,
    pub remote_url: String,
    pub branch: String,
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
    pub fn from_bootstrap_remote(remote: &BootstrapRemoteConfig, cache_dir: PathBuf) -> Result<Self> {
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
            crate::app::vault::auth::git::GitAuthMode::HttpsCredentials => {
                build_https_auth_plan(
                    inline_credentials.https_username.as_str(),
                    inline_credentials.https_secret.as_str(),
                )?
            }
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
            cache_dir,
            auth,
        })
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

    fn ensure_repo_ready(&self) -> Result<()> {
        if !self.config.cache_dir.join(".git").exists() {
            fs::create_dir_all(&self.config.cache_dir).with_context(|| {
                format!(
                    "failed to create git repo cache `{}`",
                    self.config.cache_dir.display()
                )
            })?;
            self.run_git(["init"], None)?;
        }

        self.ensure_remote()?;
        self.run_git(["config", "user.name", "Mica Term Vault"], None)?;
        self.run_git(["config", "user.email", "vault@mica-term.local"], None)?;
        Ok(())
    }

    fn ensure_remote(&self) -> Result<()> {
        let output = self.run_git_capture(["remote", "get-url", REMOTE_NAME], None)?;
        if output.status.success() {
            let current = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if current != self.config.remote_url {
                self.run_git(["remote", "set-url", REMOTE_NAME, self.config.remote_url.as_str()], None)?;
            }
            return Ok(());
        }

        self.run_git(["remote", "add", REMOTE_NAME, self.config.remote_url.as_str()], None)
    }

    fn remote_branch_exists(&self) -> Result<bool> {
        let output =
            self.run_git_capture(["ls-remote", "--heads", REMOTE_NAME, self.config.branch.as_str()], None)?;
        if !output.status.success() {
            return Err(command_error("git ls-remote", &output));
        }

        Ok(!String::from_utf8_lossy(&output.stdout).trim().is_empty())
    }

    fn fetch_remote_branch(&self) -> Result<bool> {
        if !self.remote_branch_exists()? {
            return Ok(false);
        }

        let refspec = format!(
            "refs/heads/{}:refs/remotes/{}/{}",
            self.config.branch, REMOTE_NAME, self.config.branch
        );
        self.run_git(["fetch", REMOTE_NAME, refspec.as_str(), "--quiet"], None)?;
        Ok(true)
    }

    fn tracking_ref(&self) -> String {
        format!("refs/remotes/{}/{}", REMOTE_NAME, self.config.branch)
    }

    fn checkout_target_branch(&self, has_remote_branch: bool) -> Result<()> {
        if has_remote_branch {
            let tracking_ref = self.tracking_ref();
            self.run_git(
                ["checkout", "-B", self.config.branch.as_str(), tracking_ref.as_str()],
                None,
            )?;
            return Ok(());
        }

        self.run_git(["checkout", "--orphan", self.config.branch.as_str()], None)?;
        let _ = self.run_git_capture(["rm", "-rf", "--cached", "--ignore-unmatch", "."], None)?;
        clear_worktree(self.config.cache_dir.as_path())?;
        Ok(())
    }

    fn read_head_from_ref(&self, git_ref: &str) -> Result<VaultHead> {
        let bytes = self.read_file_from_ref(git_ref, HEAD_FILE_NAME)?;
        serde_json::from_slice(bytes.as_slice()).context("failed to decode git repo vault head")
    }

    fn read_revision_from_ref(&self, git_ref: &str) -> Result<ProviderRevision> {
        let head = self.read_head_from_ref(git_ref)?;
        let manifest: VaultManifest = bincode::deserialize(
            self.read_file_from_ref(git_ref, MANIFEST_FILE_NAME)?.as_slice(),
        )
        .context("failed to decode git repo vault manifest")?;
        let encrypted_snapshot = rebuild_snapshot_from_manifest(
            &head,
            &manifest,
            self.read_file_from_ref(git_ref, SNAPSHOT_FILE_NAME)?,
        )?;

        Ok(ProviderRevision {
            head,
            manifest,
            encrypted_snapshot,
        })
    }

    fn read_file_from_ref(&self, git_ref: &str, file_name: &str) -> Result<Vec<u8>> {
        let object = format!("{git_ref}:{file_name}");
        let output = self.run_git_capture(["show", object.as_str()], None)?;
        if !output.status.success() {
            return Err(command_error("git show", &output));
        }
        Ok(output.stdout)
    }

    fn find_commit_for_revision(&self, revision: &str) -> Result<String> {
        let tracking_ref = self.tracking_ref();
        let output = self.run_git_capture(["rev-list", tracking_ref.as_str()], None)?;
        if !output.status.success() {
            return Err(command_error("git rev-list", &output));
        }

        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let sha = line.trim();
            if sha.is_empty() {
                continue;
            }
            let candidate = match self.read_head_from_ref(sha) {
                Ok(candidate) => candidate,
                Err(_) => continue,
            };
            if candidate.vault_revision == revision {
                return Ok(sha.to_string());
            }
        }

        Err(anyhow!(
            "git repo remote `{}` is missing revision `{revision}`",
            self.config.remote_id
        ))
    }

    fn run_git<const N: usize>(&self, args: [&str; N], cwd: Option<&Path>) -> Result<()> {
        let output = self.run_git_capture(args, cwd)?;
        if !output.status.success() {
            return Err(command_error("git command", &output));
        }
        Ok(())
    }

    fn run_git_capture<const N: usize>(
        &self,
        args: [&str; N],
        cwd: Option<&Path>,
    ) -> Result<std::process::Output> {
        let auth = PreparedGitAuth::prepare(&self.config.auth)?;
        let mut command = Command::new("git");
        command.args(args);
        command.current_dir(cwd.unwrap_or(self.config.cache_dir.as_path()));
        auth.apply(&mut command);
        command
            .output()
            .with_context(|| format!("failed to run git command in `{}`", self.config.cache_dir.display()))
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
        self.ensure_repo_ready()?;
        if !self.fetch_remote_branch()? {
            return Ok(ProviderReadResult::default());
        }

        Ok(ProviderReadResult {
            head: Some(self.read_head_from_ref(self.tracking_ref().as_str())?),
        })
    }

    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision> {
        self.ensure_repo_ready()?;
        if !self.fetch_remote_branch()? {
            return Err(anyhow!(
                "git repo remote `{}` is empty",
                self.config.remote_id
            ));
        }
        let commit = self.find_commit_for_revision(head.vault_revision.as_str())?;
        self.read_revision_from_ref(commit.as_str())
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        self.ensure_repo_ready()?;
        let has_remote_branch = self.fetch_remote_branch()?;
        let remote_head = if has_remote_branch {
            Some(self.read_head_from_ref(self.tracking_ref().as_str())?)
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

        self.checkout_target_branch(has_remote_branch)?;
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

        self.run_git(["add", HEAD_FILE_NAME, MANIFEST_FILE_NAME, SNAPSHOT_FILE_NAME], None)?;
        let commit_message = format!("vault-revision: {}", request.head.vault_revision);
        self.run_git(["commit", "--quiet", "-m", commit_message.as_str()], None)?;

        let refspec = format!("HEAD:refs/heads/{}", self.config.branch);
        let output = self.run_git_capture(["push", REMOTE_NAME, refspec.as_str()], None)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("non-fast-forward") || stderr.contains("fetch first") {
                return Err(anyhow!(
                    "non-fast-forward push rejected for git remote `{}`",
                    self.config.remote_id
                ));
            }
            return Err(command_error("git push", &output));
        }

        Ok(())
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

fn command_error(prefix: &str, output: &std::process::Output) -> anyhow::Error {
    anyhow!(
        "{prefix} failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
}

struct PreparedGitAuth {
    envs: Vec<(String, String)>,
    cleanup_paths: Vec<PathBuf>,
}

impl PreparedGitAuth {
    fn prepare(plan: &GitTransportAuthPlan) -> Result<Self> {
        match plan {
            GitTransportAuthPlan::HttpsCredentials { username, secret } => {
                let askpass_path = write_askpass_script()?;
                Ok(Self {
                    envs: vec![
                        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
                        ("GIT_ASKPASS".into(), askpass_path.display().to_string()),
                        ("MICA_TERM_GIT_USERNAME".into(), username.clone()),
                        ("MICA_TERM_GIT_SECRET".into(), secret.clone()),
                    ],
                    cleanup_paths: vec![askpass_path],
                })
            }
            GitTransportAuthPlan::SshKey {
                private_key,
                passphrase: _,
            } => {
                let key_path = write_ssh_private_key(private_key.as_str())?;
                Ok(Self {
                    envs: vec![
                        (
                            "GIT_SSH_COMMAND".into(),
                            format!(
                                "ssh -i \"{}\" -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new -o BatchMode=yes",
                                key_path.display()
                            ),
                        ),
                        ("GIT_TERMINAL_PROMPT".into(), "0".into()),
                    ],
                    cleanup_paths: vec![key_path],
                })
            }
        }
    }

    fn apply(&self, command: &mut Command) {
        for (key, value) in &self.envs {
            command.env(key, value);
        }
    }
}

impl Drop for PreparedGitAuth {
    fn drop(&mut self) {
        for path in &self.cleanup_paths {
            let _ = fs::remove_file(path);
        }
    }
}

fn write_askpass_script() -> Result<PathBuf> {
    let suffix = if cfg!(windows) { "cmd" } else { "sh" };
    let path = std::env::temp_dir().join(format!(
        "mica-term-git-askpass-{}.{}",
        uuid::Uuid::new_v4(),
        suffix
    ));
    let body = if cfg!(windows) {
        "@echo off\r\nset prompt=%~1\r\nif not x%prompt:Username=%==x%prompt% (\r\n  echo %MICA_TERM_GIT_USERNAME%\r\n) else (\r\n  echo %MICA_TERM_GIT_SECRET%\r\n)\r\n"
            .to_string()
    } else {
        "#!/bin/sh\ncase \"$1\" in\n  *Username*) printf '%s' \"$MICA_TERM_GIT_USERNAME\" ;;\n  *) printf '%s' \"$MICA_TERM_GIT_SECRET\" ;;\nesac\n"
            .to_string()
    };
    fs::write(&path, body).with_context(|| format!("failed to write `{}`", path.display()))?;
    set_script_permissions(&path)?;
    Ok(path)
}

fn write_ssh_private_key(private_key: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!(
        "mica-term-git-ssh-key-{}",
        uuid::Uuid::new_v4()
    ));
    fs::write(&path, private_key)
        .with_context(|| format!("failed to write `{}`", path.display()))?;
    set_owner_only_permissions(&path)?;
    Ok(path)
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("failed to update permissions for `{}`", path.display()))
}

#[cfg(unix)]
fn set_script_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("failed to update permissions for `{}`", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(not(unix))]
fn set_script_permissions(_path: &Path) -> Result<()> {
    Ok(())
}
