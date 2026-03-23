//! Stable SSH connection profile normalized from modal draft state.

use anyhow::{Context, bail};

use crate::shell::view_model::AssetSshConnectionDraft;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuthMethod {
    Password,
    PrivateKeyPath,
    PrivateKeyContent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionProfile {
    pub asset_id: Option<String>,
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: u16,
    pub auth_method: SshAuthMethod,
    pub credential_ref: Option<String>,
    pub private_key_path: Option<String>,
    pub remark: String,
}

impl ConnectionProfile {
    pub fn from_draft(draft: &AssetSshConnectionDraft) -> anyhow::Result<Self> {
        let name = draft.name.trim().to_string();
        let host = draft.host.trim().to_string();
        let user = draft.user.trim().to_string();
        let port = if draft.port.trim().is_empty() {
            22
        } else {
            draft
                .port
                .trim()
                .parse::<u16>()
                .with_context(|| format!("invalid ssh port: {}", draft.port.trim()))?
        };

        if name.is_empty() {
            bail!("ssh profile name is required");
        }
        if host.is_empty() {
            bail!("ssh profile host is required");
        }
        if user.is_empty() {
            bail!("ssh profile user is required");
        }

        let (auth_method, credential_ref, private_key_path) = match draft.auth_method.as_str() {
            "password" => {
                if draft.password.trim().is_empty() {
                    bail!("password authentication requires a password");
                }
                (
                    SshAuthMethod::Password,
                    Some(format!("draft://ssh-password/{user}@{host}:{port}")),
                    None,
                )
            }
            "private-key" => match draft.private_key_source.as_str() {
                "path" => {
                    let private_key_path = draft.private_key_path.trim();
                    if private_key_path.is_empty() {
                        bail!("private key path authentication requires a file path");
                    }
                    (
                        SshAuthMethod::PrivateKeyPath,
                        None,
                        Some(private_key_path.to_string()),
                    )
                }
                "content" => {
                    if draft.private_key_content.trim().is_empty() {
                        bail!("inline private key authentication requires key content");
                    }
                    (
                        SshAuthMethod::PrivateKeyContent,
                        Some(format!("draft://ssh-inline-key/{user}@{host}:{port}")),
                        None,
                    )
                }
                other => bail!("unsupported private key source: {other}"),
            },
            other => bail!("unsupported ssh auth method: {other}"),
        };

        Ok(Self {
            asset_id: None,
            name,
            host,
            user,
            port,
            auth_method,
            credential_ref,
            private_key_path,
            remark: draft.remark.trim().to_string(),
        })
    }
}
