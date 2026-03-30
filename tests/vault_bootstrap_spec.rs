use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use i_slint_backend_testing::init_no_event_loop;
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter, VaultRuntimeOptions,
    bind_top_status_bar_with_injected_services_and_vault_runtime, bind_top_status_bar_with_store,
};
use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, StoredSshSecretBundle, persist_secret_bundle,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::vault::bootstrap::{
    LocalVaultBootstrapState, bootstrap_provider_credential_ref, export_bootstrap_bundle,
    import_bootstrap_bundle, load_provider_credential, persist_provider_credential,
    restore_provider_credentials, save_local_vault_bootstrap_state, validate_bootstrap_bundle,
};
use mica_term::app::vault::cache::store_encrypted_cache;
use mica_term::app::vault::crypto::{encrypt_snapshot, generate_vault_key, wrap_vault_key};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, GiteeRemoteDraft,
    KdfConfig, ProviderAuthKind, ProviderKind, RemoteRole, SnapshotSyncPreferences,
};
use mica_term::app::vault::snapshot::export_vault_snapshot;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSnippetSpec, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree,
    ConsoleAssetKind,
};
use secrecy::SecretString;
use slint::Model;
use tokio::sync::mpsc;
use uuid::Uuid;

struct FakeLauncher;

struct NoopRuntimeControl;

#[derive(Clone, Default)]
struct CancelledPrivateKeyImporter;

fn temp_bootstrap_export_path() -> PathBuf {
    std::env::temp_dir().join(format!("mica-term-bootstrap-export-{}.bin", Uuid::new_v4()))
}

fn sample_vault_runtime_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-bootstrap-{label}-{}",
        Uuid::new_v4()
    ))
}

fn sample_vault_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "vault-bootstrap-smoke-salt".into(),
    }
}

fn sample_asset_tree(credential_ref: &str) -> AssetTree {
    let mut tree = AssetTree::new();
    tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Imported Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.42".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            auth_source: "manual".into(),
            keychain_identity_id: None,
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: Some(credential_ref.into()),
        }),
    );
    let package_id = tree.insert_root(ConsoleAssetKind::SnippetPackage, "Deploy");
    tree.insert_child_with_payload(
        &package_id,
        ConsoleAssetKind::Snippet,
        "Deploy prod",
        AssetNodePayload::Snippet(AssetSnippetSpec {
            script: "kubectl apply -f prod.yaml".into(),
            package_id: Some(package_id.clone()),
        }),
    );
    tree.insert_root_with_payload(
        ConsoleAssetKind::Snippet,
        "Restart API",
        AssetNodePayload::Snippet(AssetSnippetSpec {
            script: "kubectl rollout restart deploy/api".into(),
            package_id: None,
        }),
    );
    tree
}

fn sample_keychain_catalog(
    identity_credential_ref: &str,
    key_credential_ref: &str,
) -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-team".into(), "identity-ops".into()],
        nodes: std::collections::BTreeMap::from([
            (
                "folder-team".into(),
                KeychainNode {
                    id: "folder-team".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: None,
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::Password,
                        ssh_key_id: None,
                        credential_ref: Some(identity_credential_ref.into()),
                        remark: "shared ops login".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Prod Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:key-prod".into(),
                        public_key: "ssh-ed25519 AAAAC3NzaKeyProd".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some(key_credential_ref.into()),
                        remark: "generated".into(),
                    }),
                },
            ),
        ]),
    }
}

fn sample_bootstrap_bundle() -> BootstrapBundle {
    BootstrapBundle {
        format_version: 1,
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
                remote_id: "remote-s3-primary".into(),
                role: RemoteRole::Primary,
                provider: ProviderKind::S3Compatible,
                locator: BootstrapRemoteLocator::S3 {
                    bucket: "vault-bucket".into(),
                    prefix: "users/demo".into(),
                    endpoint: Some("https://s3.example.com".into()),
                    region: Some("ap-southeast-1".into()),
                    force_path_style: true,
                },
                credential_ref: Some(bootstrap_provider_credential_ref("remote-s3-primary")),
                auth_kind: ProviderAuthKind::AwsStandardChain,
                last_health: None,
            },
            BootstrapRemoteConfig {
                remote_id: "remote-github-mirror".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitHubGist,
                locator: BootstrapRemoteLocator::GitHubGist {
                    gist_id: "gist-123".into(),
                },
                credential_ref: Some(bootstrap_provider_credential_ref("remote-github-mirror")),
                auth_kind: ProviderAuthKind::Pat,
                last_health: None,
            },
        ],
        auto_sync_enabled: true,
        bootstrap_cipher: CipherKind::XChaCha20Poly1305,
        bootstrap_kdf: Some(KdfConfig::Argon2id {
            memory_cost_kib: 19_456,
            time_cost: 2,
            parallelism: 1,
            salt_b64: "bootstrap-static-salt".into(),
        }),
    }
}

fn seed_locked_vault_runtime(
    temp_root: &PathBuf,
    password: &SecretString,
) -> (String, String, String) {
    let source_store = Arc::new(MemoryCredentialStore::default());
    let credential_ref = "ssh/saved-secrets/imported-bastion".to_string();
    let identity_credential_ref = "keychain/identity/identity-ops".to_string();
    let key_credential_ref = "keychain/key/key-prod".to_string();

    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist ssh secret");
    persist_secret_bundle(
        source_store.as_ref(),
        &identity_credential_ref,
        &StoredSshSecretBundle {
            password: Some("ops-password".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist keychain identity secret");
    persist_secret_bundle(
        source_store.as_ref(),
        &key_credential_ref,
        &StoredSshSecretBundle {
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("key-passphrase".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist keychain key secret");

    let snapshot = export_vault_snapshot(
        &sample_asset_tree(&credential_ref),
        &sample_keychain_catalog(&identity_credential_ref, &key_credential_ref),
        source_store.as_ref(),
        temp_root.join("known-hosts").as_path(),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences {
            theme_mode: mica_term::theme::ThemeMode::Light,
            always_on_top: true,
            right_panel_view: "appearance".into(),
        },
    )
    .expect("export seeded snapshot");
    let vault_key = generate_vault_key();
    let encrypted = encrypt_snapshot(&snapshot, &vault_key).expect("encrypt snapshot");
    let wrapped_vault_key = serde_json::to_string(
        &wrap_vault_key(password, &sample_vault_kdf(), &vault_key).expect("wrap vault key"),
    )
    .expect("encode wrapped vault key");

    save_local_vault_bootstrap_state(
        &temp_root.join("vault-bootstrap-state.json"),
        &LocalVaultBootstrapState {
            bundle: sample_bootstrap_bundle(),
            wrapped_vault_key,
            kdf: sample_vault_kdf(),
            current_revision: Some("rev-0001".into()),
        },
    )
    .expect("save local bootstrap state");
    store_encrypted_cache(&temp_root.join("cache"), "vault-main", &encrypted)
        .expect("store encrypted cache");

    (credential_ref, identity_credential_ref, key_credential_ref)
}

fn bind_with_vault_runtime(app: &AppWindow, credential_store: Arc<dyn CredentialStore>, root: PathBuf) {
    bind_top_status_bar_with_injected_services_and_vault_runtime(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        credential_store,
        Arc::new(CancelledPrivateKeyImporter),
        VaultRuntimeOptions {
            root_dir: Some(root),
            ..VaultRuntimeOptions::default()
        },
    );
}

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> anyhow::Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> anyhow::Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> anyhow::Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> anyhow::Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> anyhow::Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl PrivateKeyImporter for CancelledPrivateKeyImporter {
    fn import_private_key(&self) -> anyhow::Result<Option<ImportedPrivateKey>> {
        Ok(None)
    }
}

#[test]
fn bootstrap_provider_credentials_round_trip_through_credential_store_refs() {
    let store = MemoryCredentialStore::default();
    let credential_ref = bootstrap_provider_credential_ref("remote-s3-primary");

    persist_provider_credential(&store, credential_ref.as_str(), Some("aws-secret-token"))
        .expect("persist provider credential");

    assert_eq!(
        load_provider_credential(&store, Some(credential_ref.as_str()))
            .expect("load provider credential")
            .as_deref(),
        Some("aws-secret-token")
    );
}

#[test]
fn bootstrap_export_round_trips_bundle_and_provider_credentials() {
    let path = temp_bootstrap_export_path();
    let password = SecretString::new("bootstrap-passphrase".into());
    let source_store = MemoryCredentialStore::default();
    let bundle = sample_bootstrap_bundle();

    persist_provider_credential(
        &source_store,
        bundle.remotes[0].credential_ref.as_deref().expect("primary credential ref"),
        Some("aws-secret-token"),
    )
    .expect("persist primary provider credential");
    persist_provider_credential(
        &source_store,
        bundle.remotes[1].credential_ref.as_deref().expect("mirror credential ref"),
        Some("github-pat-token"),
    )
    .expect("persist mirror provider credential");

    export_bootstrap_bundle(&path, &bundle, &source_store, &password)
        .expect("export bootstrap bundle");

    let imported = import_bootstrap_bundle(&path, &password).expect("import bootstrap bundle");
    assert_eq!(imported.bundle, bundle);
    assert_eq!(
        imported
            .provider_credentials
            .get(bundle.remotes[0].credential_ref.as_ref().expect("primary ref"))
            .map(String::as_str),
        Some("aws-secret-token")
    );
    assert_eq!(
        imported
            .provider_credentials
            .get(bundle.remotes[1].credential_ref.as_ref().expect("mirror ref"))
            .map(String::as_str),
        Some("github-pat-token")
    );

    let restored_store = MemoryCredentialStore::default();
    restore_provider_credentials(&restored_store, &imported).expect("restore provider credentials");
    assert_eq!(
        load_provider_credential(
            &restored_store,
            bundle.remotes[1].credential_ref.as_deref()
        )
        .expect("reload restored provider credential")
        .as_deref(),
        Some("github-pat-token")
    );

    let _ = fs::remove_file(path);
}

#[test]
fn bootstrap_export_file_is_not_plaintext_json() {
    let path = temp_bootstrap_export_path();
    let password = SecretString::new("bootstrap-passphrase".into());
    let store = MemoryCredentialStore::default();
    let bundle = sample_bootstrap_bundle();

    persist_provider_credential(
        &store,
        bundle.remotes[0].credential_ref.as_deref().expect("primary credential ref"),
        Some("aws-secret-token"),
    )
    .expect("persist provider credential");

    export_bootstrap_bundle(&path, &bundle, &store, &password).expect("export bootstrap bundle");

    let raw = fs::read(&path).expect("read encrypted bootstrap export");
    let printable = String::from_utf8_lossy(&raw);
    assert!(!printable.contains("vault-main"));
    assert!(!printable.contains("remote-s3-primary"));
    assert!(!printable.contains("\"remotes\""));
    assert!(!printable.contains("aws-secret-token"));

    let _ = fs::remove_file(path);
}

#[test]
fn bootstrap_bundle_validation_rejects_missing_primary_remote() {
    let mut bundle = sample_bootstrap_bundle();
    bundle.remotes.retain(|remote| remote.role != RemoteRole::Primary);

    let err = validate_bootstrap_bundle(&bundle).expect_err("bundle without primary should fail");

    assert!(
        err.to_string().contains("at least one primary remote"),
        "unexpected validation error: {err}"
    );
}

#[test]
fn gitee_remote_draft_tracks_pat_and_gist_target_for_first_release() {
    let draft = GiteeRemoteDraft::default();

    assert_eq!(draft.personal_access_token, "");
    assert_eq!(draft.gist_id, "");
    assert!(draft.create_new_gist);
    assert!(draft.setup_summary().contains("Personal Access Token"));
    assert!(draft.setup_summary().contains("Gist ID"));
}

#[test]
fn sync_modal_first_release_copy_mentions_gitee_pat_targeting_only() {
    init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();

    let status = app.get_sync_modal_status_text();
    assert!(status.contains("Gitee"));
    assert!(status.contains("Personal Access Token"));
    assert!(status.contains("Gist ID"));
    assert!(!status.contains("GitHub"));
    assert!(!status.contains("GitLab"));
    assert!(!status.contains("S3"));
    assert!(!status.contains("OAuth"));
}

#[test]
fn unlock_restores_console_snippet_and_keychain_projection() {
    init_no_event_loop();

    let temp_root = sample_vault_runtime_root("unlock-restore");
    let password = SecretString::new("vault-pass".into());
    let (credential_ref, identity_credential_ref, key_credential_ref) =
        seed_locked_vault_runtime(&temp_root, &password);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(&app, credential_store.clone(), temp_root);

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_snippet_asset_items().row_count(), 0);
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_snippet_asset_items().row_count(), 2);
    assert_eq!(app.get_keychain_asset_items().row_count(), 3);
    assert_eq!(
        app.get_snippet_asset_items().row_data(0).unwrap().label.as_str(),
        "Deploy"
    );
    assert_eq!(
        app.get_keychain_asset_items().row_data(1).unwrap().label.as_str(),
        "Prod Key"
    );
    assert!(credential_store.get_secret(&credential_ref).unwrap().is_some());
    assert!(
        credential_store
            .get_secret(&identity_credential_ref)
            .unwrap()
            .is_some()
    );
    assert!(credential_store.get_secret(&key_credential_ref).unwrap().is_some());
}

#[test]
fn lock_clears_decrypted_keychain_and_asset_state() {
    init_no_event_loop();

    let temp_root = sample_vault_runtime_root("lock-clear");
    let password = SecretString::new("vault-pass".into());
    let (credential_ref, identity_credential_ref, key_credential_ref) =
        seed_locked_vault_runtime(&temp_root, &password);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(&app, credential_store.clone(), temp_root);

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_snippet_asset_items().row_count(), 2);
    assert_eq!(app.get_keychain_asset_items().row_count(), 3);

    app.invoke_sync_modal_lock_requested();

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_snippet_asset_items().row_count(), 0);
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);
    assert_eq!(credential_store.get_secret(&credential_ref).unwrap(), None);
    assert_eq!(
        credential_store.get_secret(&identity_credential_ref).unwrap(),
        None
    );
    assert_eq!(credential_store.get_secret(&key_credential_ref).unwrap(), None);
}
