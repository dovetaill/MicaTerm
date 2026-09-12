use anyhow::{Result, anyhow, ensure};

use crate::app::vault::model::ProviderAuthKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitAuthMode {
    HttpsCredentials,
    SshKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitTransportAuthPlan {
    HttpsCredentials {
        username: String,
        secret: String,
    },
    SshKey {
        private_key: String,
        passphrase: Option<String>,
    },
}

pub fn git_auth_mode_for_provider_auth(auth_kind: ProviderAuthKind) -> Result<GitAuthMode> {
    match auth_kind {
        ProviderAuthKind::HttpsCredentials | ProviderAuthKind::Pat => {
            Ok(GitAuthMode::HttpsCredentials)
        }
        ProviderAuthKind::SshKey => Ok(GitAuthMode::SshKey),
        other => Err(anyhow!("unsupported Git auth kind `{other:?}`")),
    }
}

pub fn build_https_auth_plan(username: &str, secret: &str) -> Result<GitTransportAuthPlan> {
    let username = username.trim();
    ensure!(!username.is_empty(), "Git HTTPS username must not be empty");
    ensure!(!secret.is_empty(), "Git HTTPS secret must not be empty");

    Ok(GitTransportAuthPlan::HttpsCredentials {
        username: username.to_string(),
        secret: secret.to_string(),
    })
}

pub fn build_ssh_auth_plan(
    private_key: &str,
    passphrase: Option<&str>,
) -> Result<GitTransportAuthPlan> {
    ensure!(
        !private_key.trim().is_empty(),
        "Git SSH private key must not be empty"
    );

    Ok(GitTransportAuthPlan::SshKey {
        private_key: private_key.to_string(),
        passphrase: passphrase
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

pub fn build_git_remote_url(
    base_url: &str,
    namespace: &str,
    repository: &str,
    ssh: bool,
) -> Result<String> {
    let mut base_url = reqwest::Url::parse(base_url.trim())?;
    ensure!(
        base_url.scheme() == "https" && base_url.host_str().is_some(),
        "Git repository base URL must use HTTPS"
    );
    ensure!(
        base_url.username().is_empty()
            && base_url.password().is_none()
            && base_url.query().is_none()
            && base_url.fragment().is_none(),
        "Git repository base URL must not contain credentials, a query, or a fragment"
    );
    let namespace = namespace.trim().trim_matches('/');
    let repository = repository.trim().trim_matches('/').trim_end_matches(".git");
    ensure!(
        !namespace.is_empty() && !repository.is_empty(),
        "Enter an owner/namespace and repository before enabling sync"
    );
    let path = format!(
        "{}/{namespace}/{repository}.git",
        base_url.path().trim_end_matches('/')
    );
    if ssh {
        Ok(format!(
            "git@{}:{}",
            base_url.host_str().unwrap(),
            path.trim_start_matches('/')
        ))
    } else {
        base_url.set_path(&path);
        Ok(base_url.to_string())
    }
}
