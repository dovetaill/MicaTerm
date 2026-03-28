//! Stable SSH connection profile normalized from modal draft state.

use anyhow::{Context, bail};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::app::ssh::credentials::{SshCredentialKind, ssh_credential_ref};
use crate::shell::assets::{AssetSshConnectionSpec, AssetSshProxySpec};
use crate::shell::view_model::AssetSshConnectionDraft;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshAuthMethod {
    Password,
    PrivateKeyPath,
    PrivateKeyContent,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ConnectionProxyProfile {
    #[default]
    None,
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
        credential_ref: Option<String>,
    },
    SshAsset {
        asset_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedProxyHop {
    Socks5 {
        host: String,
        port: u16,
        username: Option<String>,
        password: Option<String>,
    },
    Ssh(Box<ConnectionProfile>),
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
    pub proxy: ConnectionProxyProfile,
    pub resolved_proxy_hops: Vec<ResolvedProxyHop>,
    pub remark: String,
}

impl ConnectionProfile {
    pub fn from_draft(draft: &AssetSshConnectionDraft) -> anyhow::Result<Self> {
        Self::from_draft_with_saved_secret_ref(draft, None)
    }

    pub fn from_modal_draft(
        asset_id: &str,
        _existing_spec: &AssetSshConnectionSpec,
        draft: &AssetSshConnectionDraft,
    ) -> anyhow::Result<Self> {
        let mut profile = Self::from_draft(draft)?;
        profile.asset_id = Some(asset_id.to_string());
        Ok(profile)
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
                    reusable_saved_secret_ref.clone().ok_or_else(|| {
                        anyhow::anyhow!("password authentication requires a password")
                    })?
                } else {
                    format!("draft://ssh-password/{user}@{host}:{port}")
                };
                (SshAuthMethod::Password, Some(credential_ref), None)
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
                            anyhow::anyhow!(
                                "inline private key authentication requires key content"
                            )
                        })?
                    } else {
                        format!("draft://ssh-inline-key/{user}@{host}:{port}")
                    };
                    (SshAuthMethod::PrivateKeyContent, Some(credential_ref), None)
                }
                other => bail!("unsupported private key source: {other}"),
            },
            other => bail!("unsupported ssh auth method: {other}"),
        };
        let proxy = normalize_draft_proxy(draft)?;

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
            proxy,
            resolved_proxy_hops: Vec::new(),
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
                        spec.credential_ref.clone(),
                        Some(private_key_path.to_string()),
                    )
                }
                "content" => (SshAuthMethod::PrivateKeyContent, saved_credential_ref, None),
                other => bail!("unsupported private key source: {other}"),
            },
            other => bail!("unsupported ssh auth method: {other}"),
        };
        let proxy = normalize_saved_proxy(&spec.proxy)?;

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
            proxy,
            resolved_proxy_hops: Vec::new(),
            remark: spec.remark.trim().to_string(),
        })
    }

    pub fn temporary_session_asset_id(&self) -> String {
        let auth_scope = match self.auth_method {
            SshAuthMethod::Password => "password".to_string(),
            SshAuthMethod::PrivateKeyPath => format!(
                "private-key-path:{}",
                self.private_key_path.as_deref().unwrap_or_default()
            ),
            SshAuthMethod::PrivateKeyContent => "private-key-content".to_string(),
        };
        let identity = format!(
            "{}@{}:{}:{}",
            self.user.trim(),
            self.host.trim(),
            self.port,
            auth_scope
        );
        let mut hasher = DefaultHasher::new();
        identity.hash(&mut hasher);

        format!("session:{:016x}", hasher.finish())
    }
}

fn saved_ssh_credential_ref(asset_id: &str, spec: &AssetSshConnectionSpec) -> String {
    spec.credential_ref
        .clone()
        .unwrap_or_else(|| ssh_credential_ref(asset_id, SshCredentialKind::SavedSecrets))
}

fn normalize_draft_proxy(draft: &AssetSshConnectionDraft) -> anyhow::Result<ConnectionProxyProfile> {
    match draft.proxy_type.trim() {
        "" | "none" => Ok(ConnectionProxyProfile::None),
        "socks5" => Ok(ConnectionProxyProfile::Socks5 {
            host: draft.proxy_socks5_host.trim().to_string(),
            port: parse_socks5_port(draft.proxy_socks5_port.trim())?,
            username: optional_string(&draft.proxy_socks5_username),
            password: optional_string(&draft.proxy_socks5_password),
            credential_ref: None,
        }),
        "ssh-asset" => Ok(ConnectionProxyProfile::SshAsset {
            asset_id: draft.proxy_ssh_asset_id.trim().to_string(),
        }),
        other => bail!("unsupported ssh proxy type: {other}"),
    }
}

fn normalize_saved_proxy(spec: &AssetSshProxySpec) -> anyhow::Result<ConnectionProxyProfile> {
    match spec {
        AssetSshProxySpec::None => Ok(ConnectionProxyProfile::None),
        AssetSshProxySpec::Socks5(proxy) => Ok(ConnectionProxyProfile::Socks5 {
            host: proxy.host.trim().to_string(),
            port: parse_socks5_port(proxy.port.trim())?,
            username: optional_string(&proxy.username),
            password: None,
            credential_ref: proxy.password_credential_ref.clone(),
        }),
        AssetSshProxySpec::SshAsset { asset_id } => Ok(ConnectionProxyProfile::SshAsset {
            asset_id: asset_id.trim().to_string(),
        }),
    }
}

fn parse_socks5_port(raw: &str) -> anyhow::Result<u16> {
    raw.parse::<u16>()
        .with_context(|| format!("invalid socks5 proxy port: {raw}"))
}

fn optional_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}
