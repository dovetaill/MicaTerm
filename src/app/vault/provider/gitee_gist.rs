use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
    VaultHead,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderWriteRequest, VaultProvider,
};

const GITEE_MAX_PACK_COUNT: usize = 4;
const GITEE_GIST_API_BASE_URL: &str = "https://gitee.com/api/v5/gists";
const GIST_HEAD_FILE_NAME: &str = "vault-head.json";
const GIST_DESCRIPTION: &str = "Mica Term Vault";
const CREDENTIAL_REF_PREFIX: &str = "vault/";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GiteeGistAuth {
    PersonalAccessToken {
        credential_ref: Option<String>,
        access_token: Option<String>,
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
                access_token: inline_access_token(remote.credential_ref.as_deref()),
            },
            other => {
                return Err(anyhow!(
                    "bootstrap remote `{}` uses unsupported Gitee auth kind `{other:?}`; first release supports PAT only",
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

impl GiteeGistProviderConfig {
    pub fn with_access_token(mut self, access_token: Option<String>) -> Self {
        let GiteeGistAuth::PersonalAccessToken {
            access_token: existing,
            ..
        } = &mut self.auth;
        *existing = access_token.filter(|value| !value.trim().is_empty());
        self
    }

    fn access_token(&self) -> Option<&str> {
        match &self.auth {
            GiteeGistAuth::PersonalAccessToken { access_token, .. } => access_token.as_deref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiteeGistFile {
    #[serde(default)]
    pub filename: String,
    pub raw_url: Option<String>,
    #[serde(default)]
    pub truncated: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GiteeGistDocument {
    #[serde(rename = "id")]
    pub gist_id: String,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub files: BTreeMap<String, GiteeGistFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiteeGistUpdateRequest {
    pub description: String,
    pub files: BTreeMap<String, String>,
}

pub trait GiteeGistApi: Send + Sync {
    fn get_gist(&self, gist_id: &str, access_token: Option<&str>) -> Result<GiteeGistDocument>;
    fn get_raw_text(&self, raw_url: &str, access_token: Option<&str>) -> Result<String>;
    fn update_gist(
        &self,
        gist_id: &str,
        request: &GiteeGistUpdateRequest,
        access_token: Option<&str>,
    ) -> Result<()>;
}

#[derive(Debug, Default)]
struct ReqwestGiteeGistApi;

#[derive(Debug, Serialize)]
struct GiteeGistUpdateFile<'a> {
    content: &'a str,
}

#[derive(Debug, Serialize)]
struct GiteeGistPatchPayload<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    access_token: Option<&'a str>,
    description: &'a str,
    files: BTreeMap<String, GiteeGistUpdateFile<'a>>,
}

#[derive(Debug, Deserialize)]
struct GiteeApiErrorDocument {
    message: Option<String>,
    error: Option<String>,
}

impl GiteeGistApi for ReqwestGiteeGistApi {
    fn get_gist(&self, gist_id: &str, access_token: Option<&str>) -> Result<GiteeGistDocument> {
        let url = format!("{GITEE_GIST_API_BASE_URL}/{gist_id}");
        run_gitee_request(async move {
            let client = reqwest::Client::new();
            let mut request = client.get(url);
            if let Some(token) = access_token {
                request = request.query(&[("access_token", token)]);
            }

            let response = request
                .send()
                .await
                .context("failed to call Gitee gist get API")?;
            ensure_success(response, "get gist")
                .await?
                .json::<GiteeGistDocument>()
                .await
                .context("failed to decode Gitee gist response")
        })
        .map(normalize_gist_document)
    }

    fn get_raw_text(&self, raw_url: &str, access_token: Option<&str>) -> Result<String> {
        let parsed_url = reqwest::Url::parse(raw_url).context("invalid Gitee gist raw_url")?;
        run_gitee_request(async move {
            let client = reqwest::Client::new();
            let mut request = client.get(parsed_url);
            if let Some(token) = access_token {
                request = request.query(&[("access_token", token)]);
            }

            let response = request
                .send()
                .await
                .context("failed to call Gitee gist raw file API")?;
            ensure_success(response, "load raw gist file")
                .await?
                .text()
                .await
                .context("failed to decode Gitee gist raw file")
        })
    }

    fn update_gist(
        &self,
        gist_id: &str,
        request: &GiteeGistUpdateRequest,
        access_token: Option<&str>,
    ) -> Result<()> {
        let url = format!("{GITEE_GIST_API_BASE_URL}/{gist_id}");
        let files = request
            .files
            .iter()
            .map(|(name, content)| {
                (
                    name.clone(),
                    GiteeGistUpdateFile {
                        content: content.as_str(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        let payload = GiteeGistPatchPayload {
            access_token,
            description: request.description.as_str(),
            files,
        };

        run_gitee_request(async move {
            let client = reqwest::Client::new();
            let response = client
                .patch(url)
                .json(&payload)
                .send()
                .await
                .context("failed to call Gitee gist update API")?;
            let _ = ensure_success(response, "update gist").await?;
            Ok(())
        })
    }
}

pub struct GiteeGistProvider {
    config: GiteeGistProviderConfig,
    api: Arc<dyn GiteeGistApi>,
}

impl GiteeGistProvider {
    pub fn new(config: GiteeGistProviderConfig) -> Result<Self> {
        Self::with_api(config, Arc::new(ReqwestGiteeGistApi))
    }

    pub fn with_api(config: GiteeGistProviderConfig, api: Arc<dyn GiteeGistApi>) -> Result<Self> {
        if config.gist_id.trim().is_empty() {
            return Err(anyhow!("Gitee gist_id must not be empty"));
        }

        Ok(Self { config, api })
    }

    pub fn load_gist_file_text(
        &self,
        file_name: &str,
        access_token: Option<&str>,
    ) -> Result<String> {
        let gist = self.api.get_gist(&self.config.gist_id, access_token)?;
        let file = gist.files.get(file_name).with_context(|| {
            format!(
                "Gitee gist `{}` is missing file `{file_name}`",
                self.config.gist_id
            )
        })?;

        self.resolve_gist_file_text(file, access_token)
    }

    fn resolve_gist_file_text(
        &self,
        file: &GiteeGistFile,
        access_token: Option<&str>,
    ) -> Result<String> {
        if file.truncated {
            let raw_url = file.raw_url.as_deref().with_context(|| {
                format!(
                    "Gitee gist file `{}` is truncated but missing raw_url",
                    file.filename
                )
            })?;
            let _validated_raw_url =
                reqwest::Url::parse(raw_url).context("invalid Gitee gist raw_url")?;
            return self.api.get_raw_text(raw_url, access_token);
        }

        file.content.clone().with_context(|| {
            format!(
                "Gitee gist file `{}` does not contain inline content",
                file.filename
            )
        })
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
        let raw = self.load_gist_file_text(GIST_HEAD_FILE_NAME, self.config.access_token())?;
        let head = serde_json::from_str::<VaultHead>(&raw).with_context(|| {
            format!(
                "failed to decode Gitee gist head for remote `{}`",
                self.config.remote_id
            )
        })?;
        Ok(ProviderReadResult { head: Some(head) })
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let pack_count = request.manifest.packs.len().max(1);
        let mut files = BTreeMap::new();
        files.insert(
            GIST_HEAD_FILE_NAME.to_string(),
            serde_json::to_string_pretty(&request.head)
                .context("failed to encode Gitee gist vault head")?,
        );
        files.insert(
            bundled_manifest_file_name(&request.head.vault_revision),
            encode_hex(
                bincode::serialize(&request.manifest)
                    .context("failed to encode Gitee gist vault manifest")?
                    .as_slice(),
            ),
        );

        for (index, chunk) in
            split_bytes(request.encrypted_snapshot.ciphertext.as_slice(), pack_count)
                .into_iter()
                .enumerate()
        {
            files.insert(
                bundled_pack_file_name(&request.head.vault_revision, index),
                encode_hex(chunk.as_slice()),
            );
        }

        self.api.update_gist(
            &self.config.gist_id,
            &GiteeGistUpdateRequest {
                description: GIST_DESCRIPTION.into(),
                files,
            },
            self.config.access_token(),
        )
    }
}

fn inline_access_token(value: Option<&str>) -> Option<String> {
    let candidate = value?.trim();
    if candidate.is_empty() || candidate.starts_with(CREDENTIAL_REF_PREFIX) {
        return None;
    }

    Some(candidate.to_string())
}

fn run_gitee_request<T>(future: impl std::future::Future<Output = Result<T>>) -> Result<T> {
    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    runtime.block_on(future)
}

async fn ensure_success(response: reqwest::Response, operation: &str) -> Result<reqwest::Response> {
    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_else(|_| String::new());

    if let Ok(error) = serde_json::from_str::<GiteeApiErrorDocument>(&body) {
        if let Some(message) = error.message.or(error.error) {
            return Err(anyhow!(
                "Gitee gist {operation} failed with {status}: {message}"
            ));
        }
    }

    if !body.trim().is_empty() {
        return Err(anyhow!(
            "Gitee gist {operation} failed with {status}: {body}"
        ));
    }

    Err(anyhow!("Gitee gist {operation} failed with {status}"))
}

fn normalize_gist_document(mut gist: GiteeGistDocument) -> GiteeGistDocument {
    for (name, file) in &mut gist.files {
        if file.filename.is_empty() {
            file.filename = name.clone();
        }
    }
    gist
}

fn bundled_manifest_file_name(revision: &str) -> String {
    format!("vault-{revision}-manifest.bin")
}

fn bundled_pack_file_name(revision: &str, index: usize) -> String {
    format!("vault-{revision}-pack-{index:04}.bin")
}

fn split_bytes(bytes: &[u8], chunk_count: usize) -> Vec<Vec<u8>> {
    let chunk_size = bytes.len().div_ceil(chunk_count);
    let mut chunks = Vec::with_capacity(chunk_count);
    for index in 0..chunk_count {
        let start = index.saturating_mul(chunk_size);
        let end = std::cmp::min(start.saturating_add(chunk_size), bytes.len());
        chunks.push(bytes.get(start..end).unwrap_or_default().to_vec());
    }
    chunks
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}
