use anyhow::{Result, anyhow};

use crate::app::vault::auth::oauth::{
    OAuthDeviceFlowBootstrap, OAuthPkceBootstrap, gitlab_device_flow_bootstrap,
    gitlab_pkce_bootstrap,
};
use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderRevision, ProviderWriteRequest, VaultProvider,
};

const DEFAULT_GITLAB_BASE_URL: &str = "https://gitlab.com";
const GITLAB_MAX_TOTAL_FILES: usize = 10;
const GITLAB_RESERVED_FILES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitLabSnippetAuth {
    DeviceFlow {
        oauth: OAuthDeviceFlowBootstrap,
        credential_ref: Option<String>,
    },
    Pkce {
        oauth: OAuthPkceBootstrap,
        credential_ref: Option<String>,
    },
    PersonalAccessToken {
        credential_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLabSnippetProviderConfig {
    pub remote_id: String,
    pub base_url: String,
    pub project_id: Option<String>,
    pub snippet_id: String,
    pub auth: GitLabSnippetAuth,
}

impl TryFrom<&BootstrapRemoteConfig> for GitLabSnippetProviderConfig {
    type Error = anyhow::Error;

    fn try_from(remote: &BootstrapRemoteConfig) -> Result<Self> {
        if remote.provider != ProviderKind::GitLabSnippet {
            return Err(anyhow!(
                "bootstrap remote `{}` is not a GitLab Snippet provider",
                remote.remote_id
            ));
        }

        let BootstrapRemoteLocator::GitLabSnippet {
            base_url,
            project_id,
            snippet_id,
        } = &remote.locator
        else {
            return Err(anyhow!(
                "bootstrap remote `{}` is missing a GitLab Snippet locator",
                remote.remote_id
            ));
        };
        let base_url = base_url
            .clone()
            .unwrap_or_else(|| DEFAULT_GITLAB_BASE_URL.to_string());

        let auth = match remote.auth_kind {
            ProviderAuthKind::DeviceFlow => GitLabSnippetAuth::DeviceFlow {
                oauth: gitlab_device_flow_bootstrap(base_url.as_str()),
                credential_ref: remote.credential_ref.clone(),
            },
            ProviderAuthKind::Pkce => GitLabSnippetAuth::Pkce {
                oauth: gitlab_pkce_bootstrap(base_url.as_str()),
                credential_ref: remote.credential_ref.clone(),
            },
            ProviderAuthKind::Pat => GitLabSnippetAuth::PersonalAccessToken {
                credential_ref: remote.credential_ref.clone(),
            },
            other => {
                return Err(anyhow!(
                    "bootstrap remote `{}` uses unsupported GitLab auth kind `{other:?}`",
                    remote.remote_id
                ));
            }
        };

        Ok(Self {
            remote_id: remote.remote_id.clone(),
            base_url,
            project_id: project_id.clone(),
            snippet_id: snippet_id.clone(),
            auth,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLabSnippetFileLayout {
    pub file_names: Vec<String>,
}

impl GitLabSnippetFileLayout {
    pub fn for_revision(revision: &str, pack_count: usize) -> Result<Self> {
        if pack_count + GITLAB_RESERVED_FILES > GITLAB_MAX_TOTAL_FILES {
            return Err(anyhow!(
                "gitlab snippet revision `{revision}` exceeds the 10-file snippet limit"
            ));
        }

        let mut file_names = vec![
            "vault-head.json".into(),
            format!("vault-{revision}-manifest.bin"),
        ];
        file_names
            .extend((0..pack_count).map(|index| format!("vault-{revision}-pack-{index:04}.bin")));

        Ok(Self { file_names })
    }
}

pub struct GitLabSnippetProvider {
    config: GitLabSnippetProviderConfig,
}

impl GitLabSnippetProvider {
    pub fn new(config: GitLabSnippetProviderConfig) -> Result<Self> {
        if config.snippet_id.trim().is_empty() {
            return Err(anyhow!("GitLab snippet_id must not be empty"));
        }

        match &config.auth {
            GitLabSnippetAuth::DeviceFlow { oauth, .. } => {
                let _ = oauth.build_request_plan(Some("mica-term-gitlab-client"))?;
            }
            GitLabSnippetAuth::Pkce { oauth, .. } => {
                oauth.validate(Some("mica-term-gitlab-client"))?;
            }
            GitLabSnippetAuth::PersonalAccessToken { .. } => {}
        }

        Ok(Self { config })
    }
}

impl VaultProvider for GitLabSnippetProvider {
    fn remote_id(&self) -> &str {
        self.config.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_conditional_head_write: false,
            max_pack_count: GITLAB_MAX_TOTAL_FILES - GITLAB_RESERVED_FILES,
            max_pack_bytes: 768 * 1024,
            preferred_pack_strategy: PackLayout::BundledFiles,
        }
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        Err(anyhow!(
            "GitLab snippet provider read_head is not wired yet for `{}`",
            self.config.remote_id
        ))
    }

    fn read_revision(
        &self,
        _head: &crate::app::vault::model::VaultHead,
    ) -> Result<ProviderRevision> {
        Err(anyhow!(
            "GitLab snippet provider read_revision is not wired yet for `{}`",
            self.config.remote_id
        ))
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let _layout = GitLabSnippetFileLayout::for_revision(
            &request.head.vault_revision,
            request.manifest.packs.len().max(1),
        )?;
        Err(anyhow!(
            "GitLab snippet provider write_revision is not wired yet for `{}`",
            self.config.remote_id
        ))
    }
}
