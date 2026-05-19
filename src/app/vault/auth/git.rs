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
    let secret = secret.trim();
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
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}
