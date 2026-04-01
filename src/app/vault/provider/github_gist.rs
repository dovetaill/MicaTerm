use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::app::vault::auth::oauth::{
    OAuthDeviceFlowBootstrap, github_device_flow_bootstrap, github_pkce_bootstrap,
};
use crate::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
    VaultHead,
};
use crate::app::vault::provider::{
    ProviderCapabilities, ProviderReadResult, ProviderRevision, ProviderWriteRequest,
    VaultProvider, rebuild_snapshot_from_manifest,
};

const GIST_HEAD_FILE_NAME: &str = "vault-head.json";
const GIST_DESCRIPTION: &str = "Mica Term Vault";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubGistAuth {
    DeviceFlow {
        oauth: OAuthDeviceFlowBootstrap,
        credential_ref: Option<String>,
    },
    PersonalAccessToken {
        credential_ref: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubGistProviderConfig {
    pub remote_id: String,
    pub gist_id: String,
    pub auth: GitHubGistAuth,
    pub api_base_url: String,
    pub authorize_base_url: String,
}

impl TryFrom<&BootstrapRemoteConfig> for GitHubGistProviderConfig {
    type Error = anyhow::Error;

    fn try_from(remote: &BootstrapRemoteConfig) -> Result<Self> {
        if remote.provider != ProviderKind::GitHubGist {
            return Err(anyhow!(
                "bootstrap remote `{}` is not a GitHub Gist provider",
                remote.remote_id
            ));
        }

        let BootstrapRemoteLocator::GitHubGist { gist_id } = &remote.locator else {
            return Err(anyhow!(
                "bootstrap remote `{}` is missing a GitHub Gist locator",
                remote.remote_id
            ));
        };

        let auth = match remote.auth_kind {
            ProviderAuthKind::DeviceFlow => GitHubGistAuth::DeviceFlow {
                oauth: github_device_flow_bootstrap(),
                credential_ref: remote.credential_ref.clone(),
            },
            ProviderAuthKind::Pat => GitHubGistAuth::PersonalAccessToken {
                credential_ref: remote.credential_ref.clone(),
            },
            other => {
                return Err(anyhow!(
                    "bootstrap remote `{}` uses unsupported GitHub auth kind `{other:?}`",
                    remote.remote_id
                ));
            }
        };

        Ok(Self {
            remote_id: remote.remote_id.clone(),
            gist_id: gist_id.clone(),
            auth,
            api_base_url: "https://api.github.com".into(),
            authorize_base_url: github_pkce_bootstrap().authorize_url,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubGistFile {
    pub filename: String,
    pub raw_url: Option<String>,
    pub truncated: bool,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitHubGistDocument {
    pub gist_id: String,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub files: BTreeMap<String, GitHubGistFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubGistUpdateRequest {
    pub description: String,
    pub files: BTreeMap<String, String>,
    pub deleted_files: Vec<String>,
}

pub trait GitHubGistApi: Send + Sync {
    fn get_gist(&self, gist_id: &str, access_token: Option<&str>) -> Result<GitHubGistDocument>;
    fn get_raw_text(&self, raw_url: &str, access_token: Option<&str>) -> Result<String>;
    fn update_gist(
        &self,
        gist_id: &str,
        request: &GitHubGistUpdateRequest,
        access_token: Option<&str>,
    ) -> Result<()>;
}

#[derive(Debug, Default)]
struct UnconfiguredGitHubGistApi;

impl GitHubGistApi for UnconfiguredGitHubGistApi {
    fn get_gist(&self, _gist_id: &str, _access_token: Option<&str>) -> Result<GitHubGistDocument> {
        Err(anyhow!("GitHub Gist API client is not configured"))
    }

    fn get_raw_text(&self, _raw_url: &str, _access_token: Option<&str>) -> Result<String> {
        Err(anyhow!("GitHub Gist API client is not configured"))
    }

    fn update_gist(
        &self,
        _gist_id: &str,
        _request: &GitHubGistUpdateRequest,
        _access_token: Option<&str>,
    ) -> Result<()> {
        Err(anyhow!("GitHub Gist API client is not configured"))
    }
}

pub struct GitHubGistProvider {
    config: GitHubGistProviderConfig,
    api: Arc<dyn GitHubGistApi>,
}

impl GitHubGistProvider {
    pub fn new(config: GitHubGistProviderConfig) -> Result<Self> {
        Self::with_api(config, Arc::new(UnconfiguredGitHubGistApi))
    }

    pub fn with_api(config: GitHubGistProviderConfig, api: Arc<dyn GitHubGistApi>) -> Result<Self> {
        if config.gist_id.trim().is_empty() {
            return Err(anyhow!("GitHub gist_id must not be empty"));
        }

        if let GitHubGistAuth::DeviceFlow { oauth, .. } = &config.auth {
            let _ = oauth.build_request_plan(Some("mica-term-public-client"))?;
        }

        Ok(Self { config, api })
    }

    pub fn config(&self) -> &GitHubGistProviderConfig {
        &self.config
    }

    pub fn load_gist_file_text(
        &self,
        file_name: &str,
        access_token: Option<&str>,
    ) -> Result<String> {
        let gist = self.api.get_gist(&self.config.gist_id, access_token)?;
        let file = gist.files.get(file_name).with_context(|| {
            format!(
                "GitHub gist `{}` is missing file `{file_name}`",
                self.config.gist_id
            )
        })?;

        self.resolve_gist_file_text(file, access_token)
    }

    fn resolve_gist_file_text(
        &self,
        file: &GitHubGistFile,
        access_token: Option<&str>,
    ) -> Result<String> {
        if file.truncated {
            let raw_url = file.raw_url.as_deref().with_context(|| {
                format!(
                    "GitHub gist file `{}` is truncated but missing raw_url",
                    file.filename
                )
            })?;
            let _validated_raw_url =
                reqwest::Url::parse(raw_url).context("invalid GitHub gist raw_url")?;
            return self.api.get_raw_text(raw_url, access_token);
        }

        file.content.clone().with_context(|| {
            format!(
                "GitHub gist file `{}` does not contain inline content",
                file.filename
            )
        })
    }
}

impl VaultProvider for GitHubGistProvider {
    fn remote_id(&self) -> &str {
        self.config.remote_id.as_str()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_conditional_head_write: false,
            max_pack_count: 8,
            max_pack_bytes: 1024 * 1024,
            preferred_pack_strategy: PackLayout::BundledFiles,
        }
    }

    fn read_head(&self) -> Result<ProviderReadResult> {
        let raw = self.load_gist_file_text(GIST_HEAD_FILE_NAME, None)?;
        let head = serde_json::from_str::<VaultHead>(&raw).with_context(|| {
            format!(
                "failed to decode GitHub gist head for remote `{}`",
                self.config.remote_id
            )
        })?;
        Ok(ProviderReadResult { head: Some(head) })
    }

    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision> {
        let gist = self.api.get_gist(&self.config.gist_id, None)?;
        let manifest = load_bundled_manifest_from_gist(&gist, head, self)?;
        let ciphertext =
            load_bundled_ciphertext_from_gist(&gist, head, manifest.packs.len().max(1), self)?;
        let encrypted_snapshot = rebuild_snapshot_from_manifest(head, &manifest, ciphertext)?;

        Ok(ProviderRevision {
            head: head.clone(),
            manifest,
            encrypted_snapshot,
        })
    }

    fn write_revision(&self, request: &ProviderWriteRequest) -> Result<()> {
        let pack_count = request.manifest.packs.len().max(1);
        let mut files = BTreeMap::new();
        files.insert(
            GIST_HEAD_FILE_NAME.to_string(),
            serde_json::to_string_pretty(&request.head)
                .context("failed to encode GitHub gist vault head")?,
        );
        files.insert(
            bundled_manifest_file_name(&request.head.vault_revision),
            encode_hex(
                bincode::serialize(&request.manifest)
                    .context("failed to encode GitHub gist vault manifest")?
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
            &GitHubGistUpdateRequest {
                description: GIST_DESCRIPTION.into(),
                files,
                deleted_files: Vec::new(),
            },
            None,
        )
    }

    fn prune_revisions(&self, keep_latest: usize, live_head: &VaultHead) -> Result<()> {
        let gist = self.api.get_gist(&self.config.gist_id, None)?;
        let retained = retained_revision_ids(
            gist.files.keys().filter_map(|name| revision_id_from_payload_file(name.as_str())),
            keep_latest,
            live_head.vault_revision.as_str(),
        );
        let deleted_files = gist
            .files
            .keys()
            .filter(|name| {
                revision_id_from_payload_file(name.as_str())
                    .is_some_and(|revision| !retained.contains(revision))
            })
            .cloned()
            .collect::<Vec<_>>();

        if deleted_files.is_empty() {
            return Ok(());
        }

        self.api.update_gist(
            &self.config.gist_id,
            &GitHubGistUpdateRequest {
                description: GIST_DESCRIPTION.into(),
                files: BTreeMap::new(),
                deleted_files,
            },
            None,
        )
    }
}

fn bundled_manifest_file_name(revision: &str) -> String {
    format!("vault-{revision}-manifest.bin")
}

fn bundled_pack_file_name(revision: &str, index: usize) -> String {
    format!("vault-{revision}-pack-{index:04}.bin")
}

fn revision_id_from_payload_file(name: &str) -> Option<&str> {
    let name = name.strip_prefix("vault-")?;
    if let Some(revision) = name.strip_suffix("-manifest.bin") {
        return Some(revision);
    }

    let revision = name.strip_suffix(".bin")?;
    revision.split_once("-pack-").map(|(revision, _)| revision)
}

fn retained_revision_ids<'a>(
    revisions: impl Iterator<Item = &'a str>,
    keep_latest: usize,
    live_revision: &str,
) -> std::collections::BTreeSet<String> {
    let mut revisions = revisions
        .map(ToOwned::to_owned)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    revisions.sort();
    revisions.reverse();

    let mut retained = revisions
        .into_iter()
        .take(keep_latest)
        .collect::<std::collections::BTreeSet<_>>();
    retained.insert(live_revision.to_string());
    retained
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

fn decode_hex(value: &str) -> Result<Vec<u8>> {
    if !value.len().is_multiple_of(2) {
        return Err(anyhow!(
            "hex payload must contain an even number of characters"
        ));
    }

    let mut output = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let hex = std::str::from_utf8(pair).context("hex payload is not valid UTF-8")?;
        let byte = u8::from_str_radix(hex, 16)
            .with_context(|| format!("invalid hex payload byte `{hex}`"))?;
        output.push(byte);
    }

    Ok(output)
}

fn load_bundled_manifest_from_gist(
    gist: &GitHubGistDocument,
    head: &VaultHead,
    provider: &GitHubGistProvider,
) -> Result<crate::app::vault::model::VaultManifest> {
    let manifest_raw = load_gist_file_from_document(
        gist,
        bundled_manifest_file_name(&head.vault_revision).as_str(),
        provider,
    )?;
    let manifest_bytes = decode_hex(manifest_raw.trim()).with_context(|| {
        format!(
            "failed to decode GitHub gist manifest payload for remote `{}` revision `{}`",
            provider.config.remote_id, head.vault_revision
        )
    })?;

    bincode::deserialize::<crate::app::vault::model::VaultManifest>(manifest_bytes.as_slice())
        .with_context(|| {
            format!(
                "failed to decode GitHub gist manifest for remote `{}` revision `{}`",
                provider.config.remote_id, head.vault_revision
            )
        })
}

fn load_bundled_ciphertext_from_gist(
    gist: &GitHubGistDocument,
    head: &VaultHead,
    pack_count: usize,
    provider: &GitHubGistProvider,
) -> Result<Vec<u8>> {
    let mut ciphertext = Vec::new();
    for index in 0..pack_count {
        let pack_raw = load_gist_file_from_document(
            gist,
            bundled_pack_file_name(&head.vault_revision, index).as_str(),
            provider,
        )?;
        let mut pack_bytes = decode_hex(pack_raw.trim()).with_context(|| {
            format!(
                "failed to decode GitHub gist pack payload for remote `{}` revision `{}` index {}",
                provider.config.remote_id, head.vault_revision, index
            )
        })?;
        ciphertext.append(&mut pack_bytes);
    }

    Ok(ciphertext)
}

fn load_gist_file_from_document(
    gist: &GitHubGistDocument,
    file_name: &str,
    provider: &GitHubGistProvider,
) -> Result<String> {
    let file = gist.files.get(file_name).with_context(|| {
        format!(
            "GitHub gist `{}` is missing file `{file_name}`",
            provider.config.gist_id
        )
    })?;
    provider.resolve_gist_file_text(file, None)
}
