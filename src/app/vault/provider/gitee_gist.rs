use anyhow::{Result, anyhow};

use crate::app::vault::auth::oauth::{OAuthCodeBootstrap, gitee_oauth_code_bootstrap};
use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderWriteRequest, VaultProvider,
};

const GITEE_MAX_PACK_COUNT: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiteeGistAuth {
    PersonalAccessToken {
        credential_ref: Option<String>,
    },
    OAuthCode {
        oauth: OAuthCodeBootstrap,
        credential_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiteeGistProviderConfig {
    pub remote_id: String,
    pub gist_id: String,
    pub auth: GiteeGistAuth,
}

impl TryFrom<&BootstrapRemoteConfig> for GiteeGistProviderConfig {
    type Error = anyhow::Error;

    fn try_from(remote: &BootstrapRemoteConfig) -> Result<Self> {
        if remote.provider != ProviderKind::GiteeGist {
            return Err(anyhow!(
                "bootstrap remote `{}` is not a Gitee Gist provider",
                remote.remote_id
            ));
        }

        let BootstrapRemoteLocator::GiteeGist { gist_id } = &remote.locator else {
            return Err(anyhow!(
                "bootstrap remote `{}` is missing a Gitee Gist locator",
                remote.remote_id
            ));
        };

        let auth = match remote.auth_kind {
            ProviderAuthKind::Pat => GiteeGistAuth::PersonalAccessToken {
                credential_ref: remote.credential_ref.clone(),
            },
            ProviderAuthKind::Pkce => GiteeGistAuth::OAuthCode {
                oauth: gitee_oauth_code_bootstrap(),
                credential_ref: remote.credential_ref.clone(),
            },
            other => {
                return Err(anyhow!(
                    "bootstrap remote `{}` uses unsupported Gitee auth kind `{other:?}`",
                    remote.remote_id
                ));
            }
        };

        Ok(Self {
            remote_id: remote.remote_id.clone(),
            gist_id: gist_id.clone(),
            auth,
        })
    }
}

pub struct GiteeGistProvider {
    config: GiteeGistProviderConfig,
}

impl GiteeGistProvider {
    pub fn new(config: GiteeGistProviderConfig) -> Result<Self> {
        if config.gist_id.trim().is_empty() {
            return Err(anyhow!("Gitee gist_id must not be empty"));
        }

        if let GiteeGistAuth::OAuthCode { oauth, .. } = &config.auth {
            oauth.validate(Some("mica-term-gitee-client"))?;
        }

        Ok(Self { config })
    }
}

impl VaultProvider for GiteeGistProvider {
    fn remote_id(&self) -> &str {
        self.config.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_conditional_head_write: false,
            max_pack_count: GITEE_MAX_PACK_COUNT,
            max_pack_bytes: 512 * 1024,
            preferred_pack_strategy: PackLayout::BundledFiles,
        }
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        Err(anyhow!(
            "Gitee gist provider read_head is not wired yet for `{}`",
            self.config.remote_id
        ))
    }

    fn write_revision(&self, _request: &ProviderWriteRequest) -> Result<()> {
        Err(anyhow!(
            "Gitee gist provider write_revision is not wired yet for `{}`",
            self.config.remote_id
        ))
    }
}
