use anyhow::{Context, Result};
use oauth2::{AuthUrl, ClientId, DeviceAuthorizationUrl, Scope, TokenUrl};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthDeviceFlowBootstrap {
    pub provider_name: String,
    pub device_authorization_url: String,
    pub token_url: String,
    pub verification_url: String,
    pub default_scopes: Vec<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthPkceBootstrap {
    pub provider_name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub default_scopes: Vec<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthCodeBootstrap {
    pub provider_name: String,
    pub authorize_url: String,
    pub token_url: String,
    pub default_scopes: Vec<String>,
    pub client_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuthDeviceFlowRequestPlan {
    pub device_authorization_url: String,
    pub token_url: String,
    pub verification_url: String,
    pub parameters: Vec<(String, String)>,
}

pub fn github_device_flow_bootstrap() -> OAuthDeviceFlowBootstrap {
    OAuthDeviceFlowBootstrap {
        provider_name: "github".into(),
        device_authorization_url: "https://github.com/login/device/code".into(),
        token_url: "https://github.com/login/oauth/access_token".into(),
        verification_url: "https://github.com/login/device".into(),
        default_scopes: vec!["gist".into()],
        client_id: None,
    }
}

pub fn github_pkce_bootstrap() -> OAuthPkceBootstrap {
    OAuthPkceBootstrap {
        provider_name: "github".into(),
        authorize_url: "https://github.com/login/oauth/authorize".into(),
        token_url: "https://github.com/login/oauth/access_token".into(),
        default_scopes: vec!["gist".into()],
        client_id: None,
    }
}

pub fn gitlab_device_flow_bootstrap(base_url: &str) -> OAuthDeviceFlowBootstrap {
    let base_url = normalize_base_url(base_url);
    OAuthDeviceFlowBootstrap {
        provider_name: "gitlab".into(),
        device_authorization_url: format!("{base_url}/oauth/authorize_device"),
        token_url: format!("{base_url}/oauth/token"),
        verification_url: format!("{base_url}/oauth/device"),
        default_scopes: vec!["api".into()],
        client_id: None,
    }
}

pub fn gitlab_pkce_bootstrap(base_url: &str) -> OAuthPkceBootstrap {
    let base_url = normalize_base_url(base_url);
    OAuthPkceBootstrap {
        provider_name: "gitlab".into(),
        authorize_url: format!("{base_url}/oauth/authorize"),
        token_url: format!("{base_url}/oauth/token"),
        default_scopes: vec!["api".into()],
        client_id: None,
    }
}

pub fn gitee_oauth_code_bootstrap() -> OAuthCodeBootstrap {
    OAuthCodeBootstrap {
        provider_name: "gitee".into(),
        authorize_url: "https://gitee.com/oauth/authorize".into(),
        token_url: "https://gitee.com/oauth/token".into(),
        default_scopes: vec!["gists".into()],
        client_id: None,
    }
}

impl OAuthDeviceFlowBootstrap {
    pub fn build_request_plan(
        &self,
        fallback_client_id: Option<&str>,
    ) -> Result<OAuthDeviceFlowRequestPlan> {
        let client_id = self
            .client_id
            .clone()
            .or_else(|| fallback_client_id.map(ToOwned::to_owned))
            .context("OAuth device flow requires a client_id")?;
        let _client_id = ClientId::new(client_id.clone());
        let _device_authorization_url =
            DeviceAuthorizationUrl::new(self.device_authorization_url.clone())
                .context("invalid OAuth device authorization URL")?;
        let _token_url =
            TokenUrl::new(self.token_url.clone()).context("invalid OAuth token URL")?;
        let _verification_url = reqwest::Url::parse(&self.verification_url)
            .context("invalid OAuth verification URL")?;
        let scope = self
            .default_scopes
            .iter()
            .map(|value| Scope::new(value.clone()).to_string())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(OAuthDeviceFlowRequestPlan {
            device_authorization_url: self.device_authorization_url.clone(),
            token_url: self.token_url.clone(),
            verification_url: self.verification_url.clone(),
            parameters: vec![("client_id".into(), client_id), ("scope".into(), scope)],
        })
    }
}

impl OAuthPkceBootstrap {
    pub fn validate(&self, fallback_client_id: Option<&str>) -> Result<()> {
        let client_id = self
            .client_id
            .clone()
            .or_else(|| fallback_client_id.map(ToOwned::to_owned))
            .context("OAuth PKCE flow requires a client_id")?;
        let _client_id = ClientId::new(client_id);
        let _authorize_url =
            AuthUrl::new(self.authorize_url.clone()).context("invalid OAuth authorize URL")?;
        let _token_url =
            TokenUrl::new(self.token_url.clone()).context("invalid OAuth token URL")?;
        Ok(())
    }
}

impl OAuthCodeBootstrap {
    pub fn validate(&self, fallback_client_id: Option<&str>) -> Result<()> {
        let client_id = self
            .client_id
            .clone()
            .or_else(|| fallback_client_id.map(ToOwned::to_owned))
            .context("OAuth code flow requires a client_id")?;
        let _client_id = ClientId::new(client_id);
        let _authorize_url =
            AuthUrl::new(self.authorize_url.clone()).context("invalid OAuth authorize URL")?;
        let _token_url =
            TokenUrl::new(self.token_url.clone()).context("invalid OAuth token URL")?;
        Ok(())
    }
}

fn normalize_base_url(base_url: &str) -> String {
    base_url.trim_end_matches('/').to_string()
}
