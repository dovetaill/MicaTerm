//! Smoke coverage for the dedicated Sync modal contract.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    PrivateKeyImporter, VaultProviderFactory, VaultRuntimeOptions,
    bind_top_status_bar_with_injected_services_and_vault_runtime, bind_top_status_bar_with_store,
};
use mica_term::app::ssh::credentials::{CredentialStore, MemoryCredentialStore};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::vault::bootstrap::load_local_vault_bootstrap_state;
use mica_term::app::vault::conflict_inbox::{ConflictInboxEntry, persist_conflict_entries};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind,
    GitHostKind, KdfConfig, PackLayout, ProviderAuthKind, ProviderKind, RemoteRole, VaultHead,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, VaultProvider};
use mica_term::app::window_effects::default_platform_window_effects;
use tokio::sync::mpsc;
use uuid::Uuid;

struct FakeLauncher;

struct NoopRuntimeControl;

#[derive(Clone, Default)]
struct CancelledPrivateKeyImporter;

#[derive(Clone, Default)]
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

#[derive(Clone)]
struct DelayedVaultProvider {
    inner: Arc<MockVaultProvider>,
    read_delay: Duration,
}

impl DelayedVaultProvider {
    fn new(inner: Arc<MockVaultProvider>, read_delay: Duration) -> Self {
        Self { inner, read_delay }
    }
}

impl VaultProvider for DelayedVaultProvider {
    fn remote_id(&self) -> &str {
        self.inner.remote_id()
    }

    fn capabilities(&self) -> ProviderCapabilities {
        self.inner.capabilities()
    }

    fn read_head(&self) -> Result<mica_term::app::vault::provider::ProviderReadResult> {
        if !self.read_delay.is_zero() {
            std::thread::sleep(self.read_delay);
        }
        self.inner.read_head()
    }

    fn read_revision(
        &self,
        head: &VaultHead,
    ) -> Result<mica_term::app::vault::provider::ProviderRevision> {
        if !self.read_delay.is_zero() {
            std::thread::sleep(self.read_delay);
        }
        self.inner.read_revision(head)
    }

    fn write_revision(
        &self,
        request: &mica_term::app::vault::provider::ProviderWriteRequest,
    ) -> Result<()> {
        self.inner.write_revision(request)
    }

    fn prune_revisions(&self, keep_latest: usize, live_head: &VaultHead) -> Result<()> {
        self.inner.prune_revisions(keep_latest, live_head)
    }
}

#[derive(Clone, Default)]
struct AnyVaultProviderFactory {
    providers: Arc<Mutex<BTreeMap<String, Arc<dyn VaultProvider>>>>,
}

impl AnyVaultProviderFactory {
    fn insert(&self, provider: Arc<dyn VaultProvider>) {
        self.providers
            .lock()
            .expect("lock vault provider factory")
            .insert(provider.remote_id().to_string(), provider);
    }
}

impl VaultProviderFactory for AnyVaultProviderFactory {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>> {
        let provider = self
            .providers
            .lock()
            .expect("lock vault provider factory")
            .get(&remote.remote_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing mock vault provider `{}`", remote.remote_id))?;
        Ok(provider)
    }
}

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

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
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

impl PrivateKeyImporter for CancelledPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<mica_term::app::bootstrap::ImportedPrivateKey>> {
        Ok(None)
    }
}

fn sample_vault_runtime_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("mica-term-sync-modal-{label}-{}", Uuid::new_v4()))
}

fn wait_for_condition(timeout: Duration, mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + timeout;
    loop {
        if predicate() {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "condition not met within {:?}",
            timeout
        );
        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();
    }
}

fn sample_bootstrap_bundle_with_primary_and_mirror() -> BootstrapBundle {
    BootstrapBundle {
        vault_id: "vault-main".into(),
        remotes: vec![
            BootstrapRemoteConfig {
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
            },
            BootstrapRemoteConfig {
                remote_id: "remote-mirror".into(),
                role: RemoteRole::Mirror,
                provider: ProviderKind::GitHubGist,
                locator: BootstrapRemoteLocator::GitHubGist {
                    gist_id: "gist-mirror".into(),
                },
                credential_ref: Some("vault/bootstrap/remote-mirror".into()),
                auth_kind: ProviderAuthKind::Pat,
                last_health: None,
            },
        ],
        auto_sync_enabled: false,
        ..BootstrapBundle::default()
    }
}

fn sample_remote_head(revision: &str) -> VaultHead {
    VaultHead {
        format_version: 1,
        vault_id: "vault-main".into(),
        vault_revision: revision.into(),
        parent_revision: Some("rev-0000".into()),
        device_id: "device-a".into(),
        committed_at: "2026-03-31T08:00:00Z".into(),
        committed_by_device: "device-a".into(),
        payload_hash: "sha256:payload-prev".into(),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: "wrapped-key-prev".into(),
        kdf: KdfConfig::Argon2id {
            memory_cost_kib: 19_456,
            time_cost: 2,
            parallelism: 1,
            salt_b64: "sync-modal-remote-salt".into(),
        },
        cipher: CipherKind::XChaCha20Poly1305,
        compression: CompressionKind::Zstd,
        pack_layout: PackLayout::BundledFiles,
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

#[test]
fn sync_modal_defaults_to_not_configured_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();

    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert_eq!(app.get_sync_modal_provider_label().as_str(), "Gitee");
    assert_eq!(app.get_sync_modal_git_remote_url().as_str(), "");
    assert_eq!(app.get_sync_modal_git_branch().as_str(), "main");
    assert_eq!(app.get_sync_modal_git_auth_mode().as_str(), "https");
    assert_eq!(
        app.get_sync_modal_primary_action_label().as_str(),
        "Save and enable"
    );
}

#[test]
fn sync_modal_projects_persisted_conflict_summary_from_local_inbox() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("conflict-summary");
    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);
    persist_conflict_entries(
        temp_root.join("conflicts").as_path(),
        &[ConflictInboxEntry {
            vault_id: bundle.vault_id.clone(),
            target_id: "asset-prod".into(),
            conflict_kind: "asset-delete-vs-modify".into(),
            local_device_id: "device-local".into(),
            remote_device_id: "device-remote".into(),
            captured_at: "00000000000000000042".into(),
        }],
    )
    .expect("persist conflict inbox entry");

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: Some(bundle),
        },
    );

    app.invoke_open_sync_modal_requested();

    assert_eq!(app.get_sync_modal_conflict_count(), 1);
    let summary = app.get_sync_modal_conflict_summary().to_string();
    assert!(
        summary.contains("asset-prod"),
        "unexpected summary: {summary}"
    );
    assert!(
        summary.contains("asset-delete-vs-modify"),
        "unexpected summary: {summary}"
    );
}

#[test]
fn sync_modal_close_request_resets_the_open_flag() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    assert!(app.get_sync_modal_open());

    app.invoke_sync_modal_close_requested();

    assert!(!app.get_sync_modal_open());
}

#[test]
fn first_enable_flow_requires_a_remote_before_local_vault_is_created() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("remote-first");
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("Configure a Gitee Git remote first")
    );
    assert!(!temp_root.join("vault-bootstrap-state.json").exists());
}

#[test]
fn sync_settings_primary_action_persists_primary_target_and_creates_local_vault() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("settings-primary");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: None,
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed(
        "git-remote-url".into(),
        "https://gitee.com/demo/mica-vault.git".into(),
    );
    app.invoke_sync_modal_draft_changed("git-branch".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "https".into());
    app.invoke_sync_modal_draft_changed("git-https-username".into(), "demo-user".into());
    app.invoke_sync_modal_draft_changed("git-https-secret".into(), "pat-primary-secret".into());
    app.invoke_sync_modal_draft_changed("master-password".into(), "vault-pass".into());

    app.invoke_sync_modal_primary_action_requested();

    let saved =
        load_local_vault_bootstrap_state(temp_root.join("vault-bootstrap-state.json").as_path())
            .expect("load local bootstrap state")
            .expect("expected persisted local bootstrap state");
    let primary = saved.bundle.primary_remote().expect("primary remote");
    assert_eq!(primary.provider, ProviderKind::GitRepo);
    assert_eq!(primary.auth_kind, ProviderAuthKind::HttpsCredentials);
    match &primary.locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
        } => {
            assert_eq!(*host_kind, GitHostKind::Gitee);
            assert_eq!(remote_url, "https://gitee.com/demo/mica-vault.git");
            assert_eq!(branch, "mica-vault");
        }
        other => panic!("unexpected primary locator: {other:?}"),
    }
}

#[test]
fn sync_modal_can_switch_between_https_and_ssh_auth_modes() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "ssh".into());
    app.invoke_sync_modal_draft_changed(
        "git-ssh-private-key".into(),
        "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
    );

    assert_eq!(app.get_sync_modal_git_auth_mode().as_str(), "ssh");
    assert_eq!(
        app.get_sync_modal_git_ssh_private_key().as_str(),
        "-----BEGIN OPENSSH PRIVATE KEY-----"
    );
}

#[test]
fn sync_modal_never_enters_locked_mode_after_sync_is_enabled() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("no-locked-mode");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);
    provider_factory.insert(mirror);
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert!(temp_root.join("vault-bootstrap-state.json").exists());

    app.invoke_sync_modal_secondary_action_requested();
    app.invoke_open_sync_modal_requested();

    assert_ne!(app.get_sync_modal_mode().as_str(), "locked");
    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_ne!(app.get_sync_modal_secondary_action_label().as_str(), "Lock");
}

#[test]
fn sync_modal_refuses_to_reinitialize_an_empty_local_state_over_an_existing_remote_revision() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("existing-remote-guard");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    primary.set_remote_head(Some(sample_remote_head("rev-0004")));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);
    provider_factory.insert(mirror);

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("rev-0004"),
        "unexpected error: {}",
        app.get_sync_modal_error_text()
    );
    assert!(!temp_root.join("vault-bootstrap-state.json").exists());
}

#[test]
fn restart_with_saved_sync_configuration_does_not_require_unlock() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("restart-no-unlock");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);
    provider_factory.insert(mirror);

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    let restarted_provider_factory = RecordingVaultProviderFactory::default();
    restarted_provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    restarted_provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));

    let restarted = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &restarted,
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(restarted_provider_factory),
            bootstrap_template: None,
        },
    );

    restarted.invoke_open_sync_modal_requested();

    assert_ne!(restarted.get_sync_modal_mode().as_str(), "locked");
    assert!(
        !restarted
            .get_sync_modal_headline()
            .as_str()
            .contains("Unlock")
    );
    assert!(
        !restarted
            .get_sync_modal_primary_action_label()
            .as_str()
            .contains("Unlock")
    );
}

#[test]
fn sync_modal_primary_action_routes_to_sync_and_secondary_action_closes() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("actions");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    provider_factory.insert(mirror.clone());

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");

    app.invoke_sync_modal_draft_changed(
        "git-remote-url".into(),
        "https://gitee.com/demo/mica-vault.git".into(),
    );
    app.invoke_sync_modal_draft_changed("git-branch".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "https".into());
    app.invoke_sync_modal_draft_changed("git-https-username".into(), "demo-user".into());
    app.invoke_sync_modal_draft_changed("git-https-secret".into(), "pat-primary-secret".into());
    app.invoke_sync_modal_primary_action_requested();
    wait_for_condition(Duration::from_secs(2), || {
        primary.recorded_writes().len() == 1
    });

    app.invoke_sync_modal_secondary_action_requested();
    assert!(!app.get_sync_modal_open());
}

#[test]
fn titlebar_sync_failure_updates_error_state_without_reopening_modal() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("titlebar-sync-failure");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();
    assert!(!app.get_sync_modal_open());

    primary.set_read_error(Some("token expired"));

    app.invoke_sync_now_requested();

    wait_for_condition(Duration::from_secs(2), || {
        !app.get_sync_modal_open()
            && app
                .get_sync_modal_error_text()
                .to_string()
                .contains("token expired")
    });

    let error = app.get_sync_modal_error_text().to_string();
    assert!(!app.get_sync_modal_open());
    assert!(error.contains("token expired"), "unexpected error: {error}");
}

#[test]
fn sync_modal_does_not_reuse_right_panel_vault_copy() {
    let source = fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();

    assert!(!source.contains("Primary remote"));
    assert!(!source.contains("Mirror remote"));
    assert!(!source.contains("primary-action := Rectangle"));
}

#[test]
fn sync_modal_window_contract_removes_lock_and_auto_sync_callbacks() {
    let source = fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!source.contains("callback sync-modal-lock-requested();"));
    assert!(!source.contains("in-out property <bool> sync-modal-auto-sync-enabled: false;"));
}

#[test]
fn sync_modal_window_contract_drops_legacy_vault_panel_callbacks() {
    let source = fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!source.contains("callback vault-create-requested(string);"));
    assert!(!source.contains("callback vault-unlock-requested(string);"));
    assert!(!source.contains("callback vault-sync-now-requested();"));
    assert!(!source.contains("callback vault-lock-requested();"));
    assert!(!source.contains("in-out property <string> vault-panel-title: \"Sync & Vault\";"));
}

#[test]
fn sync_modal_window_contract_exposes_conflict_summary_fields() {
    let app_window = fs::read_to_string("ui/app-window.slint").unwrap();
    let component = fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();

    assert!(app_window.contains("in-out property <int> sync-modal-conflict-count: 0;"));
    assert!(app_window.contains("in-out property <string> sync-modal-conflict-summary: \"\";"));
    assert!(component.contains("in property <int> conflict-count: 0;"));
    assert!(component.contains("in property <string> conflict-summary: \"\";"));
}

#[test]
fn sync_modal_shows_local_and_remote_sync_timestamps() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.set_sync_modal_open(true);
    app.set_sync_modal_local_last_sync_text("2026-04-02 10:30".into());
    app.set_sync_modal_remote_last_update_text("2026-04-02 10:31".into());
    app.set_sync_modal_primary_revision_text("rev-0042".into());
    app.set_sync_modal_remote_status_text("Primary remote is currently at rev-0042.".into());

    assert_eq!(
        app.get_sync_modal_local_last_sync_text().as_str(),
        "2026-04-02 10:30"
    );
    assert_eq!(
        app.get_sync_modal_remote_last_update_text().as_str(),
        "2026-04-02 10:31"
    );
    assert_eq!(
        app.get_sync_modal_primary_revision_text().as_str(),
        "rev-0042"
    );
    assert_eq!(
        app.get_sync_modal_remote_status_text().as_str(),
        "Primary remote is currently at rev-0042."
    );
}

#[test]
fn opening_sync_settings_refreshes_primary_head_in_background() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("remote-head-refresh-success");
    let primary_inner = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    primary_inner.set_remote_head(Some(sample_remote_head("rev-0042")));
    let primary = Arc::new(DelayedVaultProvider::new(
        Arc::clone(&primary_inner),
        Duration::from_millis(250),
    ));
    let provider_factory = AnyVaultProviderFactory::default();
    provider_factory.insert(primary as Arc<dyn VaultProvider>);

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    let started = Instant::now();
    app.invoke_open_sync_modal_requested();

    assert!(app.get_sync_modal_open());
    assert!(
        started.elapsed() < Duration::from_millis(120),
        "sync settings should open immediately while remote head refresh runs in the background"
    );

    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_primary_revision_text().as_str() == "rev-0042"
    });

    assert_eq!(
        app.get_sync_modal_remote_last_update_text().as_str(),
        "2026-03-31 08:00"
    );
    assert_eq!(
        app.get_sync_modal_primary_revision_text().as_str(),
        "rev-0042"
    );
    assert!(
        app.get_sync_modal_remote_status_text()
            .to_string()
            .contains("rev-0042")
    );
}

#[test]
fn remote_head_refresh_failure_keeps_sync_settings_non_blocking() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("remote-head-refresh-failure");
    let primary_inner = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    primary_inner.set_read_error(Some("token expired"));
    let primary = Arc::new(DelayedVaultProvider::new(
        Arc::clone(&primary_inner),
        Duration::from_millis(250),
    ));
    let provider_factory = AnyVaultProviderFactory::default();
    provider_factory.insert(primary as Arc<dyn VaultProvider>);

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    let started = Instant::now();
    app.invoke_open_sync_modal_requested();

    assert!(app.get_sync_modal_open());
    assert!(
        started.elapsed() < Duration::from_millis(120),
        "sync settings should stay non-blocking even when remote head refresh fails"
    );

    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_remote_status_text()
            .to_string()
            .contains("Failed to refresh remote status")
    });

    assert!(
        app.get_sync_modal_error_text()
            .to_string()
            .contains("token expired")
    );
}
