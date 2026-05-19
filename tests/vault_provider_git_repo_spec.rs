use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use anyhow::{Result, anyhow};
use git2::Repository;
use mica_term::app::vault::auth::git::{
    GitTransportAuthPlan, build_https_auth_plan, build_ssh_auth_plan,
};
use mica_term::app::vault::crypto::{encrypt_snapshot, generate_vault_key};
use mica_term::app::vault::model::{
    BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind, GitHostKind,
    KdfConfig, PackLayout, PackRef, ProviderAuthKind, ProviderKind, RemoteRole, VaultAssetCatalog,
    VaultHead, VaultManifest, VaultSnapshot,
};
use mica_term::app::vault::provider::git_repo::{
    GitRemoteSafetyStatus, GitRepoProvider, GitRepoProviderConfig, GitRepositoryMetadata,
    GitRepositoryMetadataSource, GitRepositoryVisibility, GitRepositoryWritePermission,
    validate_first_release_git_host, validate_remote_for_push,
};
use mica_term::app::vault::provider::{
    ProviderWriteRequest, VaultProvider, attach_snapshot_recovery_metadata,
};
use uuid::Uuid;

static PATH_ENV_LOCK: Mutex<()> = Mutex::new(());

fn sample_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-git-repo-provider-{label}-{}",
        Uuid::new_v4()
    ))
}

fn sample_remote_bare_repo(label: &str) -> PathBuf {
    sample_root(label).join("remote.git")
}

fn sample_cache_dir(label: &str) -> PathBuf {
    sample_root(label).join("cache")
}

fn sample_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "git-repo-provider-salt".into(),
    }
}

fn sample_snapshot() -> VaultSnapshot {
    VaultSnapshot {
        schema_version: 1,
        asset_catalog: VaultAssetCatalog::default(),
        ..VaultSnapshot::default()
    }
}

fn lock_path_env() -> MutexGuard<'static, ()> {
    PATH_ENV_LOCK.lock().expect("lock PATH env")
}

fn init_bare_remote_repo(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("remote parent")).expect("create remote parent");
    Repository::init_bare(path).expect("init bare remote repo");
}

struct MissingGitPathGuard {
    original_path: Option<OsString>,
    sandbox_path: PathBuf,
}

impl MissingGitPathGuard {
    fn install() -> Self {
        let sandbox_path = sample_root("missing-git-path").join("bin");
        fs::create_dir_all(&sandbox_path).expect("create missing git path sandbox");
        let original_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", &sandbox_path);
        }
        Self {
            original_path,
            sandbox_path,
        }
    }
}

impl Drop for MissingGitPathGuard {
    fn drop(&mut self) {
        match self.original_path.as_ref() {
            Some(path) => unsafe {
                std::env::set_var("PATH", path);
            },
            None => unsafe {
                std::env::remove_var("PATH");
            },
        }
        let _ = fs::remove_dir_all(&self.sandbox_path);
    }
}

fn sample_remote(
    remote_url: String,
    auth_kind: ProviderAuthKind,
    credential_ref: Option<String>,
    host_kind: GitHostKind,
) -> BootstrapRemoteConfig {
    BootstrapRemoteConfig {
        remote_id: "remote-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch: "main".into(),
            base_url: None,
            api_base_url: None,
            namespace: None,
            repository: None,
            root_path: None,
            display_name: None,
        },
        credential_ref,
        auth_kind,
        last_health: None,
    }
}

fn inline_https_credentials() -> String {
    serde_json::json!({
        "https_username": "demo",
        "https_secret": "secret-token"
    })
    .to_string()
}

fn sample_write_request(
    revision: &str,
    parent_revision: Option<&str>,
    device_id: &str,
) -> ProviderWriteRequest {
    let snapshot = sample_snapshot();
    let encrypted_snapshot =
        encrypt_snapshot(&snapshot, &generate_vault_key()).expect("encrypt snapshot");
    let mut manifest = VaultManifest {
        packs: vec![PackRef {
            pack_id: format!("pack-{revision}"),
            object_name: "vault-snapshot.bin".into(),
            size_bytes: encrypted_snapshot.ciphertext.len() as u64,
            digest: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        }],
        ..VaultManifest::default()
    };
    attach_snapshot_recovery_metadata(&mut manifest, &encrypted_snapshot);
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: parent_revision.map(ToOwned::to_owned),
        device_id: device_id.into(),
        committed_at: format!("2026-04-01T00:00:{revision}Z"),
        committed_by_device: device_id.into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: "vault-manifest.bin".into(),
        wrapped_vault_key: "wrapped-vault-key".into(),
        kdf: sample_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    };

    ProviderWriteRequest {
        head,
        manifest,
        encrypted_snapshot,
        expected_parent_revision: parent_revision.map(ToOwned::to_owned),
        conditional_head_write: false,
    }
}

fn sample_provider(remote: &BootstrapRemoteConfig, cache_dir: PathBuf) -> GitRepoProvider {
    GitRepoProvider::new(
        GitRepoProviderConfig::from_bootstrap_remote(remote, cache_dir)
            .expect("build git repo config"),
    )
    .expect("build git repo provider")
}

struct CountingRepositoryMetadataSource {
    fetch_count: Mutex<usize>,
    next_result: Mutex<Option<Result<GitRepositoryMetadata>>>,
}

impl CountingRepositoryMetadataSource {
    fn returning(result: Result<GitRepositoryMetadata>) -> Self {
        Self {
            fetch_count: Mutex::new(0),
            next_result: Mutex::new(Some(result)),
        }
    }

    fn fetch_count(&self) -> usize {
        *self.fetch_count.lock().expect("lock fetch count")
    }
}

impl GitRepositoryMetadataSource for CountingRepositoryMetadataSource {
    fn fetch_repository_metadata(
        &self,
        _remote: &BootstrapRemoteConfig,
        _access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        *self.fetch_count.lock().expect("lock fetch count") += 1;
        self.next_result
            .lock()
            .map_err(|_| anyhow!("metadata result lock poisoned"))?
            .take()
            .ok_or_else(|| anyhow!("missing repository metadata result"))?
    }
}

fn sample_repository_metadata(
    visibility: GitRepositoryVisibility,
    write_permission: GitRepositoryWritePermission,
) -> GitRepositoryMetadata {
    GitRepositoryMetadata {
        canonical_id: "demo/mica-vault".into(),
        display_name: "demo/mica-vault".into(),
        visibility,
        write_permission,
        default_branch: Some("main".into()),
    }
}

fn bare_remote_tree_has_path(remote_repo: &Path, path: &str) -> bool {
    let repo = Repository::open_bare(remote_repo).expect("open bare remote");
    let oid = repo
        .refname_to_id("refs/heads/main")
        .expect("resolve bare remote main head");
    let commit = repo.find_commit(oid).expect("load bare remote commit");
    let tree = commit.tree().expect("load bare remote tree");
    tree.get_path(Path::new(path)).is_ok()
}

#[test]
fn git_repo_remote_round_trip_preserves_branch_auth_and_repository_coordinates() {
    let remote = BootstrapRemoteConfig {
        remote_id: "remote-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::Gitee,
            remote_url: "git@gitee.com:demo/mica-vault.git".into(),
            branch: "mica-vault".into(),
            base_url: Some("https://gitee.com".into()),
            api_base_url: Some("https://gitee.com/api/v5".into()),
            namespace: Some("demo".into()),
            repository: Some("mica-vault".into()),
            root_path: Some(".mica-term-sync".into()),
            display_name: Some("Demo Vault".into()),
        },
        credential_ref: Some("vault/bootstrap/remote-primary".into()),
        auth_kind: ProviderAuthKind::SshKey,
        last_health: None,
    };

    let encoded = serde_json::to_string_pretty(&remote).unwrap();
    let decoded: BootstrapRemoteConfig = serde_json::from_str(&encoded).unwrap();

    assert_eq!(decoded.provider, ProviderKind::GitRepo);
    assert_eq!(decoded.auth_kind, ProviderAuthKind::SshKey);
    match decoded.locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            base_url,
            api_base_url,
            namespace,
            repository,
            root_path,
            display_name,
        } => {
            assert_eq!(host_kind, GitHostKind::Gitee);
            assert_eq!(remote_url, "git@gitee.com:demo/mica-vault.git");
            assert_eq!(branch, "mica-vault");
            assert_eq!(base_url.as_deref(), Some("https://gitee.com"));
            assert_eq!(api_base_url.as_deref(), Some("https://gitee.com/api/v5"));
            assert_eq!(namespace.as_deref(), Some("demo"));
            assert_eq!(repository.as_deref(), Some("mica-vault"));
            assert_eq!(root_path.as_deref(), Some(".mica-term-sync"));
            assert_eq!(display_name.as_deref(), Some("Demo Vault"));
        }
        other => panic!("unexpected locator: {other:?}"),
    }
}

#[test]
fn legacy_git_repo_remote_locator_deserializes_without_repository_coordinates() {
    let encoded = serde_json::json!({
        "remote_id": "remote-primary",
        "role": "primary",
        "provider": "git-repo",
        "locator": {
            "git-repo": {
                "host_kind": "git-hub",
                "remote_url": "https://github.com/demo/mica-vault.git",
                "branch": "main"
            }
        },
        "credential_ref": "vault/bootstrap/remote-primary",
        "auth_kind": "pat",
        "last_health": null
    });

    let decoded: BootstrapRemoteConfig =
        serde_json::from_value(encoded).expect("deserialize legacy git repo remote");

    match decoded.locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            base_url,
            api_base_url,
            namespace,
            repository,
            root_path,
            display_name,
        } => {
            assert_eq!(host_kind, GitHostKind::GitHub);
            assert_eq!(remote_url, "https://github.com/demo/mica-vault.git");
            assert_eq!(branch, "main");
            assert_eq!(base_url, None);
            assert_eq!(api_base_url, None);
            assert_eq!(namespace, None);
            assert_eq!(repository, None);
            assert_eq!(root_path, None);
            assert_eq!(display_name, None);
        }
        other => panic!("unexpected locator: {other:?}"),
    }
}

#[test]
fn git_repo_remote_supports_https_credentials_contract() {
    let remote = BootstrapRemoteConfig {
        remote_id: "remote-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::Gitee,
            remote_url: "https://gitee.com/demo/mica-vault.git".into(),
            branch: "main".into(),
            base_url: Some("https://gitee.com".into()),
            api_base_url: Some("https://gitee.com/api/v5".into()),
            namespace: Some("demo".into()),
            repository: Some("mica-vault".into()),
            root_path: Some(".mica-term-sync".into()),
            display_name: Some("demo/mica-vault".into()),
        },
        credential_ref: Some("vault/bootstrap/remote-primary".into()),
        auth_kind: ProviderAuthKind::HttpsCredentials,
        last_health: None,
    };

    assert_eq!(remote.provider, ProviderKind::GitRepo);
    assert_eq!(remote.auth_kind, ProviderAuthKind::HttpsCredentials);
}

#[test]
fn git_repo_provider_reads_remote_head_from_repo_branch() {
    let remote_repo = sample_remote_bare_repo("read-head");
    init_bare_remote_repo(remote_repo.as_path());

    let writer_remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::Gitee,
    );
    let reader_remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::Gitee,
    );
    let writer = sample_provider(&writer_remote, sample_cache_dir("writer-provider"));
    let reader = sample_provider(&reader_remote, sample_cache_dir("reader-provider"));
    let request = sample_write_request("rev-0001", None, "device-a");

    writer
        .write_revision(&request)
        .expect("seed git repo remote");

    let head = reader
        .read_head()
        .expect("read remote head")
        .head
        .expect("remote head should exist");
    let revision = reader.read_revision(&head).expect("read remote revision");

    assert_eq!(head.vault_revision, "rev-0001");
    assert_eq!(revision.head.vault_revision, "rev-0001");
    assert_eq!(
        revision.encrypted_snapshot.payload_sha256,
        request.encrypted_snapshot.payload_sha256
    );
}

#[test]
fn git_repo_provider_round_trip_does_not_require_system_git() {
    let _env_lock = lock_path_env();
    let remote_repo = sample_remote_bare_repo("without-system-git");
    init_bare_remote_repo(remote_repo.as_path());

    let writer_remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::Gitee,
    );
    let reader_remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::Gitee,
    );
    let writer = sample_provider(
        &writer_remote,
        sample_cache_dir("without-system-git-writer"),
    );
    let reader = sample_provider(
        &reader_remote,
        sample_cache_dir("without-system-git-reader"),
    );
    let request = sample_write_request("rev-0001", None, "device-a");
    let _missing_git = MissingGitPathGuard::install();

    writer
        .write_revision(&request)
        .expect("write revision without system git");

    let head = reader
        .read_head()
        .expect("read remote head without system git")
        .head
        .expect("remote head should exist");
    let revision = reader
        .read_revision(&head)
        .expect("read remote revision without system git");

    assert_eq!(head.vault_revision, "rev-0001");
    assert_eq!(revision.head.vault_revision, "rev-0001");
    assert_eq!(
        revision.encrypted_snapshot.payload_sha256,
        request.encrypted_snapshot.payload_sha256
    );
}

#[test]
fn git_repo_provider_scopes_sync_payloads_under_managed_root() {
    let remote_repo = sample_remote_bare_repo("managed-root");
    init_bare_remote_repo(remote_repo.as_path());

    let remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::GitHub,
    );
    let provider = sample_provider(&remote, sample_cache_dir("managed-root-provider"));

    provider
        .write_revision(&sample_write_request("rev-0001", None, "device-a"))
        .expect("seed git repo remote");

    assert!(
        bare_remote_tree_has_path(remote_repo.as_path(), ".mica-term-sync/vault-head.json"),
        "managed root should contain the live head pointer"
    );
    assert!(
        bare_remote_tree_has_path(
            remote_repo.as_path(),
            ".mica-term-sync/revisions/rev-0001/vault-head.json",
        ),
        "managed root should keep the committed head alongside the retained revision payloads"
    );
    assert!(
        !bare_remote_tree_has_path(remote_repo.as_path(), "vault-head.json"),
        "git repo sync should not spill payload files into the repository root"
    );
}

#[test]
fn git_https_credentials_build_transport_auth_plan() {
    let plan = build_https_auth_plan("demo", "secret-token").expect("build https auth plan");

    assert_eq!(
        plan,
        GitTransportAuthPlan::HttpsCredentials {
            username: "demo".into(),
            secret: "secret-token".into(),
        }
    );
}

#[test]
fn git_ssh_key_builds_transport_auth_plan() {
    let plan = build_ssh_auth_plan(
        "-----BEGIN OPENSSH PRIVATE KEY-----\nkey-material\n-----END OPENSSH PRIVATE KEY-----\n",
        Some("vault-passphrase"),
    )
    .expect("build ssh auth plan");

    assert_eq!(
        plan,
        GitTransportAuthPlan::SshKey {
            private_key:
                "-----BEGIN OPENSSH PRIVATE KEY-----\nkey-material\n-----END OPENSSH PRIVATE KEY-----\n"
                    .into(),
            passphrase: Some("vault-passphrase".into()),
        }
    );
}

#[test]
fn git_repo_provider_rejects_non_fast_forward_push() {
    let remote_repo = sample_remote_bare_repo("non-fast-forward");
    init_bare_remote_repo(remote_repo.as_path());

    let remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::Gitee,
    );
    let provider_a = sample_provider(&remote, sample_cache_dir("provider-a"));
    let provider_b = sample_provider(&remote, sample_cache_dir("provider-b"));

    provider_a
        .write_revision(&sample_write_request("rev-0001", None, "device-a"))
        .expect("seed initial revision");
    provider_b
        .write_revision(&sample_write_request(
            "rev-0002",
            Some("rev-0001"),
            "device-b",
        ))
        .expect("advance remote head");

    let err = provider_a
        .write_revision(&sample_write_request(
            "rev-0003",
            Some("rev-0001"),
            "device-a",
        ))
        .expect_err("stale push should conflict");

    assert!(err.to_string().contains("non-fast-forward"));
}

#[test]
fn provider_keeps_latest_10_revisions() {
    let remote_repo = sample_remote_bare_repo("retention-latest-10");
    init_bare_remote_repo(remote_repo.as_path());

    let remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::GitHub,
    );
    let writer = sample_provider(&remote, sample_cache_dir("retention-writer"));

    let mut parent_revision = None;
    for revision in 1..=12 {
        let revision_id = format!("rev-{revision:04}");
        writer
            .write_revision(&sample_write_request(
                revision_id.as_str(),
                parent_revision.as_deref(),
                "device-a",
            ))
            .expect("write retained git revision");
        writer
            .prune_revisions(
                10,
                &sample_write_request(revision_id.as_str(), None, "device-a").head,
            )
            .expect("prune retained git revisions");
        parent_revision = Some(revision_id);
    }

    let reader = sample_provider(&remote, sample_cache_dir("retention-reader"));
    let live_head = reader
        .read_head()
        .expect("read retained live head")
        .head
        .expect("retained live head");
    assert_eq!(live_head.vault_revision, "rev-0012");

    reader
        .read_revision(&sample_write_request("rev-0003", None, "device-a").head)
        .expect("latest 10 revisions should stay readable");
    let err = reader
        .read_revision(&sample_write_request("rev-0002", None, "device-a").head)
        .expect_err("older revisions should be pruned once the retention limit is exceeded");
    assert!(
        err.to_string().contains("rev-0002"),
        "unexpected error: {err}"
    );
}

#[test]
fn cleanup_never_deletes_current_head_revision() {
    let remote_repo = sample_remote_bare_repo("retention-live-head");
    init_bare_remote_repo(remote_repo.as_path());

    let remote = sample_remote(
        remote_repo.display().to_string(),
        ProviderAuthKind::HttpsCredentials,
        Some(inline_https_credentials()),
        GitHostKind::GitLab,
    );
    let writer = sample_provider(&remote, sample_cache_dir("retention-live-head-writer"));

    writer
        .write_revision(&sample_write_request("rev-0001", None, "device-a"))
        .expect("write first revision");
    writer
        .write_revision(&sample_write_request(
            "rev-0002",
            Some("rev-0001"),
            "device-a",
        ))
        .expect("write second revision");
    writer
        .prune_revisions(0, &sample_write_request("rev-0002", None, "device-a").head)
        .expect("prune all but the live head");

    let reader = sample_provider(&remote, sample_cache_dir("retention-live-head-reader"));
    let live_head = reader
        .read_head()
        .expect("read live head after cleanup")
        .head
        .expect("live head after cleanup");
    assert_eq!(live_head.vault_revision, "rev-0002");
    reader
        .read_revision(&live_head)
        .expect("cleanup must keep the committed live head readable");
    reader
        .read_revision(&sample_write_request("rev-0001", None, "device-a").head)
        .expect_err("cleanup should still remove older revisions when keeping the live head");
}

#[test]
fn first_release_git_host_validation_rejects_non_gitee_hosts() {
    let err = validate_first_release_git_host(GitHostKind::GitHub)
        .expect_err("non-gitee hosts should stay hidden in first release");

    assert!(err.to_string().contains("first release"));
}

#[test]
fn configured_remote_revalidated_before_push_if_safety_status_stale() {
    let remote = sample_remote(
        "https://github.com/demo/mica-vault.git".into(),
        ProviderAuthKind::Pat,
        Some("vault/bootstrap/remote-primary".into()),
        GitHostKind::GitHub,
    );
    let source = CountingRepositoryMetadataSource::returning(Ok(sample_repository_metadata(
        GitRepositoryVisibility::Public,
        GitRepositoryWritePermission::Writable,
    )));

    let err = validate_remote_for_push(
        &remote,
        GitRemoteSafetyStatus::Stale,
        &source,
        Some("github-pat"),
    )
    .expect_err("stale safety status should force push-time revalidation");

    assert_eq!(source.fetch_count(), 1);
    assert!(
        err.to_string().contains("private"),
        "unexpected error: {err}"
    );
}
