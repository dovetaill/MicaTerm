use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mica_term::app::vault::crypto::EncryptedSnapshot;
use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind, KdfConfig,
    PackLayout, PackRef, ProviderAuthKind, ProviderKind, RemoteRole, VaultHead, VaultManifest,
};
use mica_term::app::vault::provider::VaultProvider;
use mica_term::app::vault::provider::gitee_gist::{
    GiteeGistApi, GiteeGistAuth, GiteeGistDocument, GiteeGistFile, GiteeGistProvider,
    GiteeGistProviderConfig, GiteeGistUpdateRequest,
};

fn sample_gitee_remote(auth_kind: ProviderAuthKind) -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: format!("remote-gitee-{auth_kind:?}").to_lowercase(),
        role: RemoteRole::Mirror,
        provider: ProviderKind::GiteeGist,
        locator: BootstrapRemoteLocator::GiteeGist {
            gist_id: "gitee-gist-456".into(),
        },
        credential_ref: Some("vault/bootstrap/gitee".into()),
        auth_kind,
        last_health: None,
    }
}

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "gitee-provider-salt".into(),
    }
}

fn sample_head(revision: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-a".into(),
        created_at: "2026-03-31T08:00:00Z".into(),
        payload_hash: "sha256:payload".into(),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: "wrapped-key".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    }
}

fn sample_write_request(revision: &str) -> mica_term::app::vault::provider::ProviderWriteRequest {
    let manifest = VaultManifest {
        packs: vec![PackRef {
            pack_id: format!("pack-{revision}"),
            object_name: format!("bundle/{revision}/snapshot.bin"),
            size_bytes: 4,
            digest: "sha256:deadbeef".into(),
        }],
        ..VaultManifest::default()
    };

    mica_term::app::vault::provider::ProviderWriteRequest {
        head: sample_head(revision),
        manifest,
        encrypted_snapshot: EncryptedSnapshot {
            cipher: CipherKind::XChaCha20Poly1305,
            compression: CompressionKind::Zstd,
            nonce: vec![0, 1, 2, 3],
            ciphertext: vec![0xde, 0xad, 0xbe, 0xef],
            plaintext_len: 4,
            compressed_len: 4,
            payload_sha256: "deadbeef".into(),
        },
        expected_parent_revision: Some("rev-0000".into()),
        conditional_head_write: false,
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

#[derive(Default)]
struct RecordingGiteeGistApi {
    gist: Mutex<Option<GiteeGistDocument>>,
    gist_reads: Mutex<Vec<(String, Option<String>)>>,
    raw_reads: Mutex<Vec<(String, Option<String>)>>,
    update_calls: Mutex<Vec<(String, GiteeGistUpdateRequest, Option<String>)>>,
}

impl RecordingGiteeGistApi {
    fn with_gist(gist: GiteeGistDocument) -> Self {
        Self {
            gist: Mutex::new(Some(gist)),
            gist_reads: Mutex::new(Vec::new()),
            raw_reads: Mutex::new(Vec::new()),
            update_calls: Mutex::new(Vec::new()),
        }
    }
}

impl GiteeGistApi for RecordingGiteeGistApi {
    fn get_gist(&self, gist_id: &str, access_token: Option<&str>) -> Result<GiteeGistDocument> {
        self.gist_reads
            .lock()
            .map_err(|_| anyhow!("gist reads lock poisoned"))?
            .push((gist_id.to_string(), access_token.map(ToOwned::to_owned)));
        self.gist
            .lock()
            .map_err(|_| anyhow!("gist lock poisoned"))?
            .clone()
            .ok_or_else(|| anyhow!("missing gist"))
    }

    fn get_raw_text(&self, raw_url: &str, access_token: Option<&str>) -> Result<String> {
        self.raw_reads
            .lock()
            .map_err(|_| anyhow!("raw reads lock poisoned"))?
            .push((raw_url.to_string(), access_token.map(ToOwned::to_owned)));
        Ok("full-file-from-raw-url".into())
    }

    fn update_gist(
        &self,
        gist_id: &str,
        request: &GiteeGistUpdateRequest,
        access_token: Option<&str>,
    ) -> Result<()> {
        self.update_calls
            .lock()
            .map_err(|_| anyhow!("update calls lock poisoned"))?
            .push((
                gist_id.to_string(),
                request.clone(),
                access_token.map(ToOwned::to_owned),
            ));
        Ok(())
    }
}

#[test]
fn gitee_bootstrap_config_supports_pat_only_for_first_release() {
    let pat_config = GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
        .expect("parse gitee pat config");

    assert!(matches!(
        pat_config.auth,
        GiteeGistAuth::PersonalAccessToken { .. }
    ));
    assert!(matches!(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pkce)),
        Err(_)
    ));
}

#[test]
fn gitee_provider_defaults_to_bundled_files_layout() {
    let provider = GiteeGistProvider::new(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
            .expect("parse gitee provider config"),
    )
    .expect("build gitee provider");

    let capabilities = provider.capabilities();

    assert_eq!(
        capabilities.preferred_pack_strategy,
        PackLayout::BundledFiles
    );
    assert!(capabilities.max_pack_count <= 6);
}

#[test]
fn gitee_provider_capabilities_report_non_conditional_writes() {
    let provider = GiteeGistProvider::new(
        GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
            .expect("parse gitee provider config"),
    )
    .expect("build gitee provider");

    assert!(!provider.capabilities().supports_conditional_head_write);
}

#[test]
fn gitee_provider_reads_head_from_gist_document_and_threads_pat_auth() {
    let expected_head = sample_head("rev-0009");
    let api = Arc::new(RecordingGiteeGistApi::with_gist(GiteeGistDocument {
        gist_id: "gitee-gist-456".into(),
        truncated: false,
        files: BTreeMap::from([(
            "vault-head.json".into(),
            GiteeGistFile {
                filename: "vault-head.json".into(),
                raw_url: None,
                truncated: false,
                content: Some(
                    serde_json::to_string(&expected_head).expect("encode expected vault head"),
                ),
            },
        )]),
    }));
    let config = GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
        .expect("parse gitee provider config")
        .with_access_token(Some("gitee-pat".into()));
    let provider = GiteeGistProvider::with_api(config, api.clone()).expect("build provider");

    let read_result = provider.read_head().expect("read gitee gist head");

    assert_eq!(read_result.head, Some(expected_head));
    assert_eq!(
        api.gist_reads.lock().expect("lock gist reads").as_slice(),
        &[("gitee-gist-456".into(), Some("gitee-pat".into()))]
    );
}

#[test]
fn gitee_provider_reads_use_raw_url_when_file_is_truncated() {
    let api = Arc::new(RecordingGiteeGistApi::with_gist(GiteeGistDocument {
        gist_id: "gitee-gist-456".into(),
        truncated: false,
        files: BTreeMap::from([(
            "vault-head.json".into(),
            GiteeGistFile {
                filename: "vault-head.json".into(),
                raw_url: Some("https://gitee.com/demo/raw/vault-head.json".into()),
                truncated: true,
                content: Some("{\"partial\":true}".into()),
            },
        )]),
    }));
    let config = GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
        .expect("parse gitee provider config")
        .with_access_token(Some("gitee-pat".into()));
    let provider = GiteeGistProvider::with_api(config, api.clone()).expect("build provider");

    let content = provider
        .load_gist_file_text("vault-head.json", Some("gitee-pat"))
        .expect("load truncated gitee gist file");

    assert_eq!(content, "full-file-from-raw-url");
    assert_eq!(
        api.raw_reads.lock().expect("lock raw reads").as_slice(),
        &[(
            "https://gitee.com/demo/raw/vault-head.json".into(),
            Some("gitee-pat".into()),
        )]
    );
}

#[test]
fn gitee_provider_writes_bundled_revision_files_back_to_the_gist() {
    let api = Arc::new(RecordingGiteeGistApi::default());
    let config = GiteeGistProviderConfig::try_from(&sample_gitee_remote(ProviderAuthKind::Pat))
        .expect("parse gitee provider config")
        .with_access_token(Some("gitee-pat".into()));
    let provider = GiteeGistProvider::with_api(config, api.clone()).expect("build provider");
    let request = sample_write_request("rev-0002");

    provider
        .write_revision(&request)
        .expect("write bundled gitee gist revision");

    let update_calls = api.update_calls.lock().expect("lock update calls");
    assert_eq!(update_calls.len(), 1);
    let (gist_id, update, access_token) = &update_calls[0];

    assert_eq!(gist_id, "gitee-gist-456");
    assert_eq!(access_token.as_deref(), Some("gitee-pat"));
    assert_eq!(update.description, "Mica Term Vault");
    assert!(
        update.files["vault-head.json"].contains("\"vault_revision\": \"rev-0002\""),
        "head file should contain the committed vault revision"
    );
    assert_eq!(
        update.files["vault-rev-0002-manifest.bin"],
        hex(bincode::serialize(&request.manifest)
            .expect("encode manifest")
            .as_slice())
    );
    assert_eq!(
        update.files["vault-rev-0002-pack-0000.bin"],
        hex(request.encrypted_snapshot.ciphertext.as_slice())
    );
}
