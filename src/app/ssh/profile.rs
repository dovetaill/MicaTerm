//! Stable SSH connection profile normalized from modal draft state.

use anyhow::{Context, bail};

use crate::app::ssh::credentials::{SshCredentialKind, ssh_credential_ref};
use crate::shell::assets::AssetSshConnectionSpec;
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
    pub password: Option<String>,
    pub private_key_content: Option<String>,
    pub passphrase: Option<String>,
    pub remark: String,
}

impl ConnectionProfile {
    pub fn from_draft(draft: &AssetSshConnectionDraft) -> anyhow::Result<Self> {
        Self::from_draft_with_saved_secret_ref(draft, None)
    }

    pub fn from_modal_draft(
        asset_id: &str,
        existing_spec: &AssetSshConnectionSpec,
        draft: &AssetSshConnectionDraft,
    ) -> anyhow::Result<Self> {
        Self::from_draft_with_saved_secret_ref(
            draft,
            reusable_saved_secret_ref(asset_id, existing_spec, draft),
        )
    }

    fn from_draft_with_saved_secret_ref(
        draft: &AssetSshConnectionDraft,
        reusable_saved_secret_ref: Option<String>,
    ) -> anyhow::Result<Self> {
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
                let credential_ref = if draft.password.trim().is_empty() {
                    reusable_saved_secret_ref
                        .clone()
                        .ok_or_else(|| anyhow::anyhow!("password authentication requires a password"))?
                } else {
                    format!("draft://ssh-password/{user}@{host}:{port}")
                };
                (
                    SshAuthMethod::Password,
                    Some(credential_ref),
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
                        reusable_saved_secret_ref,
                        Some(private_key_path.to_string()),
                    )
                }
                "content" => {
                    let credential_ref = if draft.private_key_content.trim().is_empty() {
                        reusable_saved_secret_ref.clone().ok_or_else(|| {
                            anyhow::anyhow!("inline private key authentication requires key content")
                        })?
                    } else {
                        format!("draft://ssh-inline-key/{user}@{host}:{port}")
                    };
                    (
                        SshAuthMethod::PrivateKeyContent,
                        Some(credential_ref),
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
            password: (!draft.password.trim().is_empty()).then(|| draft.password.clone()),
            private_key_content: (!draft.private_key_content.trim().is_empty())
                .then(|| draft.private_key_content.clone()),
            passphrase: (!draft.passphrase.trim().is_empty()).then(|| draft.passphrase.clone()),
            remark: draft.remark.trim().to_string(),
        })
    }

    pub fn from_saved_asset(
        asset_id: &str,
        title: &str,
        spec: &AssetSshConnectionSpec,
    ) -> anyhow::Result<Self> {
        let host = spec.host.trim().to_string();
        let user = spec.user.trim().to_string();
        let name = title.trim().to_string();
        let port = if spec.port.trim().is_empty() {
            22
        } else {
            spec.port
                .trim()
                .parse::<u16>()
                .with_context(|| format!("invalid ssh port: {}", spec.port.trim()))?
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

        let saved_credential_ref = Some(saved_ssh_credential_ref(asset_id, spec));

        let auth_method_id = spec.auth_method.trim();
        let private_key_source_id = if spec.private_key_source.trim().is_empty() {
            "content"
        } else {
            spec.private_key_source.trim()
        };

        let (auth_method, credential_ref, private_key_path) = match auth_method_id {
            "" | "password" => (SshAuthMethod::Password, saved_credential_ref, None),
            "private-key" => match private_key_source_id {
                "path" => {
                    let private_key_path = spec.private_key_path.trim();
                    if private_key_path.is_empty() {
                        bail!("private key path authentication requires a file path");
                    }
                    (
                        SshAuthMethod::PrivateKeyPath,
                        None,
                        Some(private_key_path.to_string()),
                    )
                }
                "content" => (SshAuthMethod::PrivateKeyContent, saved_credential_ref, None),
                other => bail!("unsupported private key source: {other}"),
            },
            other => bail!("unsupported ssh auth method: {other}"),
        };

        Ok(Self {
            asset_id: Some(asset_id.to_string()),
            name,
            host,
            user,
            port,
            auth_method,
            credential_ref,
            private_key_path,
            password: None,
            private_key_content: None,
            passphrase: None,
            remark: spec.remark.trim().to_string(),
        })
    }
}

fn reusable_saved_secret_ref(
    asset_id: &str,
    existing_spec: &AssetSshConnectionSpec,
    draft: &AssetSshConnectionDraft,
) -> Option<String> {
    let existing_auth_method = normalized_ssh_auth_method(&existing_spec.auth_method);
    let existing_private_key_source = normalized_ssh_private_key_source(&existing_spec.private_key_source);

    if existing_auth_method != draft.auth_method {
        return None;
    }

    if draft.auth_method == "private-key" && existing_private_key_source != draft.private_key_source {
        return None;
    }

    match draft.auth_method.as_str() {
        "password" => Some(saved_ssh_credential_ref(asset_id, existing_spec)),
        "private-key" if draft.private_key_source == "content" => {
            Some(saved_ssh_credential_ref(asset_id, existing_spec))
        }
        "private-key" if draft.private_key_source == "path" && existing_spec.credential_ref.is_some() => {
            Some(saved_ssh_credential_ref(asset_id, existing_spec))
        }
        _ => None,
    }
}

fn saved_ssh_credential_ref(asset_id: &str, spec: &AssetSshConnectionSpec) -> String {
    spec.credential_ref
        .clone()
        .unwrap_or_else(|| ssh_credential_ref(asset_id, SshCredentialKind::SavedSecrets))
}

fn normalized_ssh_auth_method(value: &str) -> &str {
    if value.trim().is_empty() {
        "password"
    } else {
        value
    }
}

fn normalized_ssh_private_key_source(value: &str) -> &str {
    if value.trim().is_empty() {
        "content"
    } else {
        value
    }
}
