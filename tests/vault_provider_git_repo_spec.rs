use std::fs;
use std::path::PathBuf;
use std::process::Command;

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
    GitRepoProvider, GitRepoProviderConfig, validate_first_release_git_host,
};
use mica_term::app::vault::provider::{
    ProviderWriteRequest, VaultProvider, attach_snapshot_recovery_metadata,
};
use uuid::Uuid;

fn sample_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mica-term-git-repo-provider-{label}-{}", Uuid::new_v4()))
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

fn run_git(args: &[&str], cwd: Option<&std::path::Path>) {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command.output().expect("run git command");
    assert!(
        output.status.success(),
        "git {:?} failed: stdout=`{}` stderr=`{}`",
        args,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn init_bare_remote_repo(path: &std::path::Path) {
    fs::create_dir_all(path.parent().expect("remote parent")).expect("create remote parent");
    run_git(&["init", "--bare", path.to_str().expect("remote path")], None);
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
        GitRepoProviderConfig::from_bootstrap_remote(remote, cache_dir).expect("build git repo config"),
    )
    .expect("build git repo provider")
}

#[test]
fn git_repo_remote_round_trip_preserves_branch_and_auth_mode() {
    let remote = BootstrapRemoteConfig {
        remote_id: "remote-primary".into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind: GitHostKind::Gitee,
            remote_url: "git@gitee.com:demo/mica-vault.git".into(),
            branch: "mica-vault".into(),
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
        } => {
            assert_eq!(host_kind, GitHostKind::Gitee);
            assert_eq!(remote_url, "git@gitee.com:demo/mica-vault.git");
            assert_eq!(branch, "mica-vault");
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

    writer.write_revision(&request).expect("seed git repo remote");

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
        .write_revision(&sample_write_request("rev-0002", Some("rev-0001"), "device-b"))
        .expect("advance remote head");

    let err = provider_a
        .write_revision(&sample_write_request("rev-0003", Some("rev-0001"), "device-a"))
        .expect_err("stale push should conflict");

    assert!(err.to_string().contains("non-fast-forward"));
}

#[test]
fn first_release_git_host_validation_rejects_non_gitee_hosts() {
    let err = validate_first_release_git_host(GitHostKind::GitHub)
        .expect_err("non-gitee hosts should stay hidden in first release");

    assert!(err.to_string().contains("first release"));
}
