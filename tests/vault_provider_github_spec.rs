use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};

use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, PackLayout, ProviderAuthKind, ProviderKind,
    RemoteRole,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::github_gist::{
    GitHubGistApi, GitHubGistAuth, GitHubGistDocument, GitHubGistFile, GitHubGistProvider,
    GitHubGistProviderConfig, GitHubGistUpdateRequest,
};

fn sample_device_flow_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-github-device".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitHubGist,
        locator: BootstrapRemoteLocator::GitHubGist {
            gist_id: "gist-device-123".into(),
        },
        credential_ref: Some("vault/bootstrap/github-device".into()),
        auth_kind: ProviderAuthKind::DeviceFlow,
        last_health: None,
    }
}

fn sample_pat_remote() -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-github-pat".into(),
        role: RemoteRole::Mirror,
        provider: ProviderKind::GitHubGist,
        locator: BootstrapRemoteLocator::GitHubGist {
            gist_id: "gist-pat-456".into(),
        },
        credential_ref: Some("vault/bootstrap/github-pat".into()),
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    }
}

#[derive(Default)]
struct RecordingGitHubGistApi {
    gist: Mutex<Option<GitHubGistDocument>>,
    raw_reads: Mutex<Vec<String>>,
}

impl RecordingGitHubGistApi {
    fn with_gist(gist: GitHubGistDocument) -> Self {
        Self {
            gist: Mutex::new(Some(gist)),
            raw_reads: Mutex::new(Vec::new()),
        }
    }
}

impl GitHubGistApi for RecordingGitHubGistApi {
    fn get_gist(&self, _gist_id: &str, _access_token: Option<&str>) -> Result<GitHubGistDocument> {
        self.gist
            .lock()
            .map_err(|_| anyhow!("gist lock poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("missing gist"))
    }

    fn get_raw_text(&self, raw_url: &str, _access_token: Option<&str>) -> Result<String> {
        self.raw_reads
            .lock()
            .map_err(|_| anyhow!("raw reads lock poisoned"))?
            .push(raw_url.to_string());
        Ok("full-file-from-raw-url".into())
    }

    fn update_gist(
        &self,
        _gist_id: &str,
        _request: &GitHubGistUpdateRequest,
        _access_token: Option<&str>,
    ) -> Result<()> {
        Ok(())
    }
}

#[test]
fn github_gist_bootstrap_config_can_represent_device_flow_or_pat() {
    let device_config = GitHubGistProviderConfig::try_from(&sample_device_flow_remote())
        .expect("parse device flow");
    let pat_config = GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat");

    assert!(matches!(
        device_config.auth,
        GitHubGistAuth::DeviceFlow { .. }
    ));
    assert!(matches!(
        pat_config.auth,
        GitHubGistAuth::PersonalAccessToken { .. }
    ));
}

#[test]
fn github_gist_provider_uses_bundled_files_layout_without_conditional_head_write() {
    let provider = GitHubGistProvider::new(
        GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat config"),
    )
    .expect("build github gist provider");

    let capabilities = provider.capabilities();

    assert_eq!(
        capabilities.preferred_pack_strategy,
        PackLayout::BundledFiles
    );
    assert!(!capabilities.supports_conditional_head_write);
    assert!(capabilities.max_pack_count <= 8);
}

#[test]
fn github_gist_reads_use_raw_url_when_file_is_truncated() {
    let provider = GitHubGistProvider::with_api(
        GitHubGistProviderConfig::try_from(&sample_pat_remote()).expect("parse pat config"),
        Arc::new(RecordingGitHubGistApi::with_gist(GitHubGistDocument {
            gist_id: "gist-pat-456".into(),
            truncated: false,
            files: BTreeMap::from([(
                "vault-head.json".into(),
                GitHubGistFile {
                    filename: "vault-head.json".into(),
                    raw_url: Some(
                        "https://gist.githubusercontent.com/demo/raw/vault-head.json".into(),
                    ),
                    truncated: true,
                    content: Some("{\"partial\":true}".into()),
                },
            )]),
        })),
    )
    .expect("build provider with fake api");

    let content = provider
        .load_gist_file_text("vault-head.json", Some("token"))
        .expect("load truncated gist file");

    assert_eq!(content, "full-file-from-raw-url");
}
