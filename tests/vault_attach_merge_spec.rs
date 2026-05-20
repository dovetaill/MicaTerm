use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter, VaultProviderFactory, VaultRuntimeOptions,
    bind_top_status_bar_with_injected_services_and_vault_runtime,
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
    LocalVaultBootstrapState, load_local_vault_bootstrap_state, load_runtime_vault_key,
};
use mica_term::app::vault::cache::load_encrypted_cache;
use mica_term::app::vault::crypto::{
    decrypt_snapshot, encrypt_snapshot, generate_vault_key, wrap_vault_key,
};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind,
    KdfConfig, PackLayout, PackRef, ProviderAuthKind, ProviderKind, RemoteRole,
    SnapshotSyncPreferences, VaultAssetPayload, VaultHead, VaultManifest,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, ProviderRevision, VaultProvider};
use mica_term::app::vault::snapshot::export_vault_snapshot;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use secrecy::SecretString;
use slint::Model;
use tokio::sync::mpsc;
use uuid::Uuid;

fn run_on_large_stack(test_name: &str, test: fn()) {
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        // These attach-merge tests instantiate the full AppWindow + vault projection path, which
        // can exceed the default Rust test-thread stack after Slint codegen grows.
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .expect("spawn large-stack test thread");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

struct NoopRuntimeControl;

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeLauncher;

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Clone, Default)]
struct CancelledPrivateKeyImporter;

impl PrivateKeyImporter for CancelledPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(None)
    }
}

#[derive(Default)]
struct RecordingVaultProviderFactory {
    providers: Arc<Mutex<BTreeMap<String, Arc<MockVaultProvider>>>>,
}

impl RecordingVaultProviderFactory {
    fn insert(&self, provider: Arc<MockVaultProvider>) {
        self.providers
            .lock()
            .expect("lock vault provider factory")
            .insert(provider.remote_id().to_string(), provider);
    }
}

impl VaultProviderFactory for RecordingVaultProviderFactory {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>> {
        let provider = self
            .providers
            .lock()
            .expect("lock vault provider factory")
            .get(&remote.remote_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing mock vault provider `{}`", remote.remote_id))?;
        Ok(provider as Arc<dyn VaultProvider>)
    }
}

fn bind_with_vault_runtime(
    app: &AppWindow,
    credential_store: Arc<dyn CredentialStore>,
    vault_runtime: VaultRuntimeOptions,
) {
    bind_top_status_bar_with_injected_services_and_vault_runtime(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        credential_store,
        Arc::new(CancelledPrivateKeyImporter),
        vault_runtime,
    );
}

fn sample_vault_runtime_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-attach-merge-{}-{}",
        label,
        Uuid::new_v4()
    ))
}

fn sample_known_hosts_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-attach-merge-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ))
}

fn sample_vault_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: "vault-attach-merge-salt".into(),
    }
}

fn sample_bootstrap_bundle_with_primary() -> BootstrapBundle {
    BootstrapBundle {
        vault_id: "vault-main".into(),
        remotes: vec![BootstrapRemoteConfig {
            remote_id: "remote-primary".into(),
            role: RemoteRole::Primary,
            provider: ProviderKind::S3Compatible,
            locator: BootstrapRemoteLocator::S3 {
                bucket: "vault-bucket".into(),
                prefix: "mica".into(),
                endpoint: None,
                region: Some("us-east-1".into()),
                force_path_style: false,
            },
            credential_ref: Some("vault/bootstrap/remote-primary".into()),
            auth_kind: ProviderAuthKind::AwsStandardChain,
            last_health: None,
        }],
        auto_sync_enabled: false,
        ..BootstrapBundle::default()
    }
}

fn sample_vault_asset_tree(host: &str) -> (AssetTree, String) {
    let mut tree = AssetTree::new();
    let credential_ref = format!("ssh/saved-secrets/imported-{host}");
    tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Imported Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: host.into(),
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
            credential_ref: Some(credential_ref.clone()),
        }),
    );
    (tree, credential_ref)
}

fn hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn sample_remote_revision(
    password: &SecretString,
    asset_tree: &AssetTree,
    keychain_catalog: &KeychainCatalog,
    credential_store: &dyn CredentialStore,
    revision: &str,
) -> ProviderRevision {
    let snapshot = export_vault_snapshot(
        asset_tree,
        keychain_catalog,
        credential_store,
        &sample_known_hosts_path("remote-revision"),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .expect("export vault snapshot");
    let vault_key = generate_vault_key();
    let encrypted_snapshot = encrypt_snapshot(&snapshot, &vault_key).expect("encrypt snapshot");
    let manifest = VaultManifest {
        packs: vec![PackRef {
            pack_id: format!("pack-{revision}"),
            object_name: format!("bundle/{revision}/snapshot.bin"),
            size_bytes: encrypted_snapshot.ciphertext.len() as u64,
            digest: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        }],
        provider_capability_fallbacks: BTreeMap::from([
            (
                "snapshot.nonce_hex".into(),
                hex(encrypted_snapshot.nonce.as_slice()),
            ),
            (
                "snapshot.plaintext_len".into(),
                encrypted_snapshot.plaintext_len.to_string(),
            ),
            (
                "snapshot.compressed_len".into(),
                encrypted_snapshot.compressed_len.to_string(),
            ),
            (
                "snapshot.payload_sha256".into(),
                encrypted_snapshot.payload_sha256.clone(),
            ),
        ]),
        ..VaultManifest::default()
    };
    let head = VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0003".into()),
        device_id: "device-remote".into(),
        committed_at: "2026-04-01T10:00:00Z".into(),
        committed_by_device: "device-remote".into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: serde_json::to_string(
            &wrap_vault_key(password, &sample_vault_kdf(), &vault_key)
                .expect("wrap remote vault key"),
        )
        .expect("encode wrapped vault key"),
        kdf: sample_vault_kdf(),
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
    };

    ProviderRevision {
        head,
        manifest,
        encrypted_snapshot,
    }
}

fn sample_remote_revision_for_tree(
    password: &SecretString,
    asset_tree: &AssetTree,
    credential_store: &dyn CredentialStore,
    revision: &str,
) -> ProviderRevision {
    sample_remote_revision(
        password,
        asset_tree,
        &KeychainCatalog::default(),
        credential_store,
        revision,
    )
}

fn sample_keychain_backed_remote_tree(host: &str, identity_id: &str) -> AssetTree {
    let mut tree = AssetTree::new();
    tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Remote Keychain Host",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: host.into(),
            user: "remote-ops".into(),
            port: "22".into(),
            auth_method: "private-key".into(),
            auth_source: "keychain-identity".into(),
            keychain_identity_id: Some(identity_id.into()),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "remote merge test".into(),
            credential_ref: None,
        }),
    );
    tree
}

fn sample_remote_keychain_catalog(identity_id: &str, key_id: &str) -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec![identity_id.into(), key_id.into()],
        nodes: BTreeMap::from([
            (
                identity_id.into(),
                KeychainNode {
                    id: identity_id.into(),
                    parent_id: None,
                    title: "Remote Identity".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "remote-ops".into(),
                        auth_kind: KeychainIdentityAuthKind::SshKey,
                        ssh_key_id: Some(key_id.into()),
                        credential_ref: Some(format!("keychain/identity/{identity_id}")),
                        remark: "remote".into(),
                    }),
                },
            ),
            (
                key_id.into(),
                KeychainNode {
                    id: key_id.into(),
                    parent_id: None,
                    title: "Remote Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:remote-key".into(),
                        public_key: "ssh-ed25519 AAAAREMOTE remote@example".into(),
                        comment: "remote@example".into(),
                        credential_ref: Some(format!("keychain/key/{key_id}")),
                        remark: "remote".into(),
                    }),
                },
            ),
        ]),
        merge_metadata: BTreeMap::new(),
    }
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());
}

fn find_keychain_row_id(app: &AppWindow, kind: &str, label: &str) -> String {
    let rows = app.get_keychain_asset_items();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .find(|row| row.kind.as_str() == kind && row.label.as_str() == label)
        .map(|row| row.id.to_string())
        .expect("matching keychain row")
}

fn create_local_keychain_ssh_key(app: &AppWindow, name: &str) -> String {
    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-ssh-key".into());
    app.invoke_keychain_ssh_key_modal_draft_changed("name".into(), name.into());
    app.invoke_keychain_ssh_key_modal_draft_changed(
        "private_key".into(),
        "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
    );
    app.invoke_keychain_ssh_key_modal_draft_changed(
        "public_key".into(),
        "ssh-ed25519 AAAALOCAL local@example".into(),
    );
    app.invoke_keychain_ssh_key_modal_draft_changed(
        "fingerprint".into(),
        "SHA256:local-key".into(),
    );
    app.invoke_confirm_asset_modal_requested();
    find_keychain_row_id(app, "ssh-key", name)
}

fn create_local_keychain_identity(app: &AppWindow, name: &str) -> String {
    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-identity".into());
    app.invoke_keychain_identity_modal_draft_changed("name".into(), name.into());
    app.invoke_keychain_identity_modal_draft_changed("username".into(), "local-ops".into());
    app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "ssh-key".into());
    app.invoke_keychain_identity_modal_action_requested("use-existing-ssh-key".into());
    app.invoke_confirm_asset_modal_requested();
    find_keychain_row_id(app, "identity", name)
}

struct AttachMergeCase {
    app: AppWindow,
    primary: Arc<MockVaultProvider>,
    credential_store: Arc<dyn CredentialStore>,
    temp_root: PathBuf,
}

impl AttachMergeCase {
    fn complete_attach(&self) {
        self.app.invoke_open_sync_modal_requested();
        self.app
            .invoke_sync_modal_submit_master_password("vault-pass".into());
    }

    fn local_state(&self) -> LocalVaultBootstrapState {
        load_local_vault_bootstrap_state(&self.temp_root.join("vault-bootstrap-state.json"))
            .expect("load local bootstrap state")
            .expect("expected local bootstrap state")
    }

    fn cached_snapshot(&self) -> mica_term::app::vault::model::VaultSnapshot {
        let runtime_vault_key =
            load_runtime_vault_key(self.credential_store.as_ref(), "vault-main")
                .expect("load runtime vault key")
                .expect("runtime key should be present");
        let encrypted = load_encrypted_cache(&self.temp_root.join("cache"), "vault-main")
            .expect("load encrypted cache")
            .expect("cached snapshot should be present");
        decrypt_snapshot(&encrypted, &runtime_vault_key).expect("decrypt cached snapshot")
    }
}

fn setup_attach_merge_case() -> AttachMergeCase {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("case");
    let password = SecretString::new("vault-pass".into());
    let remote_store = Arc::new(MemoryCredentialStore::default());
    let (remote_tree, remote_credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        remote_store.as_ref(),
        &remote_credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist remote secret bundle");

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let remote_revision =
        sample_remote_revision_for_tree(&password, &remote_tree, remote_store.as_ref(), "rev-0004");
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary()),
            ..VaultRuntimeOptions::default()
        },
    );
    create_root_ssh(&app, "Local Bastion", "10.0.0.12");
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    AttachMergeCase {
        app,
        primary,
        credential_store,
        temp_root,
    }
}

#[test]
fn attach_time_merge_keeps_local_and_remote_assets_when_bootstrap_state_is_missing() {
    run_on_large_stack(
        "attach_time_merge_keeps_local_and_remote_assets_when_bootstrap_state_is_missing",
        attach_time_merge_keeps_local_and_remote_assets_when_bootstrap_state_is_missing_body,
    );
}

fn attach_time_merge_keeps_local_and_remote_assets_when_bootstrap_state_is_missing_body() {
    let case = setup_attach_merge_case();

    case.complete_attach();

    let snapshot = case.cached_snapshot();
    let cached_hosts = snapshot
        .asset_catalog
        .nodes
        .values()
        .filter_map(|node| match &node.payload {
            VaultAssetPayload::SshConnection(spec) => Some(spec.host.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        case.app.get_console_asset_items().row_count(),
        2,
        "attach-time merge should project both local and remote assets into the current view (mode={}, error={}, cached_hosts={:?})",
        case.app.get_sync_modal_mode(),
        case.app.get_sync_modal_error_text(),
        cached_hosts
    );
    assert!(snapshot.asset_catalog.nodes.values().any(|node| matches!(
        &node.payload,
        VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.12"
    )));
    assert!(snapshot.asset_catalog.nodes.values().any(|node| matches!(
        &node.payload,
        VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.99"
    )));
}

#[test]
fn attach_time_merge_pushes_a_new_merged_revision_back_to_primary() {
    run_on_large_stack(
        "attach_time_merge_pushes_a_new_merged_revision_back_to_primary",
        attach_time_merge_pushes_a_new_merged_revision_back_to_primary_body,
    );
}

fn attach_time_merge_pushes_a_new_merged_revision_back_to_primary_body() {
    let case = setup_attach_merge_case();

    case.complete_attach();

    let writes = case.primary.recorded_writes();
    assert_eq!(
        writes.len(),
        1,
        "expected attach-time merge to push once (mode={}, error={})",
        case.app.get_sync_modal_mode(),
        case.app.get_sync_modal_error_text()
    );
    assert_eq!(writes[0].head.parent_revision.as_deref(), Some("rev-0004"));
    assert_eq!(writes[0].head.vault_revision, "rev-0005");
    let local_state = case.local_state();
    assert_eq!(local_state.current_revision.as_deref(), Some("rev-0005"));
}

#[test]
fn attach_time_merge_remaps_remote_keychain_ids_and_keeps_secret_ownership() {
    run_on_large_stack(
        "attach_time_merge_remaps_remote_keychain_ids_and_keeps_secret_ownership",
        attach_time_merge_remaps_remote_keychain_ids_and_keeps_secret_ownership_body,
    );
}

fn attach_time_merge_remaps_remote_keychain_ids_and_keeps_secret_ownership_body() {
    let case = setup_attach_merge_case();
    let password = SecretString::new("vault-pass".into());
    let local_key_id = create_local_keychain_ssh_key(&case.app, "Local Key");
    let local_identity_id = create_local_keychain_identity(&case.app, "Local Identity");

    assert_eq!(local_key_id, "key-1");
    assert_eq!(local_identity_id, "identity-1");

    let remote_store = Arc::new(MemoryCredentialStore::default());
    let remote_tree = sample_keychain_backed_remote_tree("10.0.0.99", "identity-1");
    let remote_catalog = sample_remote_keychain_catalog("identity-1", "key-1");

    persist_secret_bundle(
        remote_store.as_ref(),
        "keychain/identity/identity-1",
        &StoredSshSecretBundle {
            password: Some("remote-password".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist remote identity secret");
    persist_secret_bundle(
        remote_store.as_ref(),
        "keychain/key/key-1",
        &StoredSshSecretBundle {
            private_key_content: Some("-----BEGIN OPENSSH PRIVATE KEY-----".into()),
            passphrase: Some("remote-passphrase".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .expect("persist remote key secret");

    let remote_revision = sample_remote_revision(
        &password,
        &remote_tree,
        &remote_catalog,
        remote_store.as_ref(),
        "rev-0004",
    );
    case.primary
        .set_remote_head(Some(remote_revision.head.clone()));
    case.primary.set_remote_revision(Some(remote_revision));

    case.complete_attach();

    let snapshot = case.cached_snapshot();
    let remote_identity_id = "identity-1-remote-merge-1";
    let remote_key_id = "key-1-remote-merge-1";
    let remote_identity_ref = format!("keychain/identity/{remote_identity_id}");
    let remote_key_ref = format!("keychain/key/{remote_key_id}");

    let remote_host = snapshot
        .asset_catalog
        .nodes
        .values()
        .find(|node| {
            matches!(
                &node.payload,
                VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.99"
            )
        })
        .expect("remote host should be present after merge");
    match &remote_host.payload {
        VaultAssetPayload::SshConnection(spec) => {
            assert_eq!(
                spec.keychain_identity_id.as_deref(),
                Some(remote_identity_id)
            );
        }
        other => panic!("unexpected remote host payload: {other:?}"),
    }

    let remote_identity = snapshot
        .keychain_catalog
        .nodes
        .get(remote_identity_id)
        .expect("remapped remote identity");
    match &remote_identity.payload {
        KeychainNodePayload::Identity(spec) => {
            assert_eq!(spec.ssh_key_id.as_deref(), Some(remote_key_id));
            assert_eq!(
                spec.credential_ref.as_deref(),
                Some(remote_identity_ref.as_str())
            );
        }
        other => panic!("unexpected remote identity payload: {other:?}"),
    }

    let remote_key = snapshot
        .keychain_catalog
        .nodes
        .get(remote_key_id)
        .expect("remapped remote key");
    match &remote_key.payload {
        KeychainNodePayload::SshKey(spec) => {
            assert_eq!(
                spec.credential_ref.as_deref(),
                Some(remote_key_ref.as_str())
            );
        }
        other => panic!("unexpected remote key payload: {other:?}"),
    }

    assert!(snapshot.keychain_catalog.nodes.contains_key("identity-1"));
    assert!(snapshot.keychain_catalog.nodes.contains_key("key-1"));
    assert!(
        snapshot
            .keychain_identity_secret_bundles
            .contains_key(remote_identity_id)
    );
    assert!(
        snapshot
            .keychain_key_secret_bundles
            .contains_key(remote_key_id)
    );
}
