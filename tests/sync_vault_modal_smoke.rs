//! Smoke coverage for the dedicated Sync modal contract.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
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
    GitHostKind, GitRemoteSafetyStatus, GitRepositoryVisibility, GitRepositoryWritePermission,
    KdfConfig, PackLayout, ProviderAuthKind, ProviderKind, RemoteRole, VaultHead,
};
use mica_term::app::vault::provider::git_repo::{
    GitRepositoryMetadata, GitRepositoryMetadataSource,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, VaultProvider};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::ComponentHandle;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use tokio::sync::mpsc;
use uuid::Uuid;

use i_slint_backend_testing::ElementHandle;

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

#[derive(Debug)]
struct FakeGitRepositoryMetadataSource {
    next_result: Mutex<Option<Result<GitRepositoryMetadata>>>,
    fetch_count: AtomicUsize,
}

impl FakeGitRepositoryMetadataSource {
    fn returning(result: Result<GitRepositoryMetadata>) -> Self {
        Self {
            next_result: Mutex::new(Some(result)),
            fetch_count: AtomicUsize::new(0),
        }
    }

    fn fetch_count(&self) -> usize {
        self.fetch_count.load(Ordering::SeqCst)
    }
}

impl GitRepositoryMetadataSource for FakeGitRepositoryMetadataSource {
    fn fetch_repository_metadata(
        &self,
        _remote: &BootstrapRemoteConfig,
        _access_token: Option<&str>,
    ) -> Result<GitRepositoryMetadata> {
        self.fetch_count.fetch_add(1, Ordering::SeqCst);
        self.next_result
            .lock()
            .expect("lock metadata source")
            .take()
            .unwrap_or_else(|| Err(anyhow!("missing fake metadata response")))
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

fn settle_modal_ui() {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn element_center(element: &ElementHandle) -> slint::LogicalPosition {
    slint::LogicalPosition::new(
        element.absolute_position().x + element.size().width / 2.0,
        element.absolute_position().y + element.size().height / 2.0,
    )
}

fn descendant_by_id(element: &ElementHandle, id: &str) -> ElementHandle {
    element
        .query_descendants()
        .match_id(id)
        .find_first()
        .unwrap_or_else(|| panic!("missing descendant `{id}`"))
}

fn dispatch_pointer_click(
    app: &AppWindow,
    position: slint::LogicalPosition,
    button: PointerEventButton,
) {
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window()
        .dispatch_event(WindowEvent::PointerPressed { position, button });
    settle_modal_ui();
    app.window()
        .dispatch_event(WindowEvent::PointerReleased { position, button });
    settle_modal_ui();
}

fn dispatch_modifier_pressed(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: modifier.into(),
    });
}

fn dispatch_modifier_released(app: &AppWindow, modifier: Key) {
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: modifier.into(),
    });
}

fn dispatch_text_key_chord(app: &AppWindow, key_text: &str, ctrl: bool, shift: bool, alt: bool) {
    if shift {
        dispatch_modifier_pressed(app, Key::Shift);
    }
    if ctrl {
        dispatch_modifier_pressed(app, Key::Control);
    }
    if alt {
        dispatch_modifier_pressed(app, Key::Alt);
    }

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: key_text.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: key_text.into(),
    });

    if alt {
        dispatch_modifier_released(app, Key::Alt);
    }
    if ctrl {
        dispatch_modifier_released(app, Key::Control);
    }
    if shift {
        dispatch_modifier_released(app, Key::Shift);
    }
    settle_modal_ui();
}

fn dispatch_text_sequence(app: &AppWindow, text: &str) {
    for ch in text.chars() {
        let key = ch.to_string();
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: key.clone().into(),
        });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: key.into() });
    }
    settle_modal_ui();
}

fn set_clipboard_text(text: &str) {
    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(text, slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed clipboard text");
}

fn clipboard_text() -> String {
    i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard text")
    .unwrap_or_default()
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

fn sample_git_repository_metadata(
    display_name: &str,
    visibility: GitRepositoryVisibility,
    write_permission: GitRepositoryWritePermission,
) -> GitRepositoryMetadata {
    GitRepositoryMetadata {
        canonical_id: display_name.into(),
        display_name: display_name.into(),
        visibility,
        write_permission,
        default_branch: Some("main".into()),
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
fn public_repo_validation_error_is_visible() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("public-repo-validation");
    let credential_store = Arc::new(MemoryCredentialStore::default());
    let metadata_source = Arc::new(FakeGitRepositoryMetadataSource::returning(Ok(
        sample_git_repository_metadata(
            "demo/mica-vault",
            GitRepositoryVisibility::Public,
            GitRepositoryWritePermission::Writable,
        ),
    )));

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::clone(&credential_store) as Arc<dyn CredentialStore>,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
            git_repo_metadata_source: metadata_source.clone(),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("git-provider-kind".into(), "github".into());
    app.invoke_sync_modal_draft_changed("git-base-url".into(), "https://github.com".into());
    app.invoke_sync_modal_draft_changed("git-namespace".into(), "demo".into());
    app.invoke_sync_modal_draft_changed("git-repository".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-branch".into(), "main".into());
    app.invoke_sync_modal_draft_changed("git-https-username".into(), "demo-user".into());
    app.invoke_sync_modal_draft_changed("git-pat".into(), "pat-public".into());

    app.invoke_sync_modal_validate_requested();

    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_validation_state().as_str() == "blocking-error"
    });

    assert_eq!(metadata_source.fetch_count(), 1);
    assert!(
        app.get_sync_modal_error_text()
            .to_string()
            .contains("must stay private")
    );
    assert_eq!(app.get_sync_modal_mode().as_str(), "not-configured");
    assert_eq!(
        credential_store
            .get_secret("vault/bootstrap/remote-primary")
            .expect("read credential")
            .as_deref(),
        None
    );
    assert!(!temp_root.join("vault-bootstrap-state.json").exists());
}

#[test]
fn gitlab_internal_repo_validation_error_is_visible() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("gitlab-internal-validation");
    let metadata_source = Arc::new(FakeGitRepositoryMetadataSource::returning(Ok(
        sample_git_repository_metadata(
            "group/mica-vault",
            GitRepositoryVisibility::Internal,
            GitRepositoryWritePermission::Writable,
        ),
    )));

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
            git_repo_metadata_source: metadata_source,
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("git-provider-kind".into(), "gitlab".into());
    app.invoke_sync_modal_draft_changed("git-base-url".into(), "https://gitlab.example.com".into());
    app.invoke_sync_modal_draft_changed("git-namespace".into(), "group".into());
    app.invoke_sync_modal_draft_changed("git-repository".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-pat".into(), "pat-internal".into());

    app.invoke_sync_modal_validate_requested();

    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_validation_state().as_str() == "blocking-error"
    });

    assert!(
        app.get_sync_modal_error_text()
            .to_string()
            .contains("internal")
    );
}

#[test]
fn private_repo_validation_success_enables_setup() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("private-repo-validation");
    let credential_store = Arc::new(MemoryCredentialStore::default());
    let metadata_source = Arc::new(FakeGitRepositoryMetadataSource::returning(Ok(
        sample_git_repository_metadata(
            "demo/mica-vault",
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::Writable,
        ),
    )));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::clone(&credential_store) as Arc<dyn CredentialStore>,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: None,
            git_repo_metadata_source: metadata_source,
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("git-provider-kind".into(), "github".into());
    app.invoke_sync_modal_draft_changed("git-base-url".into(), "https://github.com".into());
    app.invoke_sync_modal_draft_changed("git-namespace".into(), "demo".into());
    app.invoke_sync_modal_draft_changed("git-repository".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-root-path".into(), ".mica-term-sync".into());
    app.invoke_sync_modal_draft_changed("git-branch".into(), "main".into());
    app.invoke_sync_modal_draft_changed("git-https-username".into(), "demo-user".into());
    app.invoke_sync_modal_draft_changed("git-pat".into(), "pat-private".into());

    app.invoke_sync_modal_validate_requested();

    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_validation_state().as_str() == "success"
    });

    app.invoke_sync_modal_draft_changed("master-password".into(), "vault-pass".into());
    app.invoke_sync_modal_primary_action_requested();

    let saved =
        load_local_vault_bootstrap_state(temp_root.join("vault-bootstrap-state.json").as_path())
            .expect("load local bootstrap state")
            .expect("expected persisted local bootstrap state");
    let primary = saved.bundle.primary_remote().expect("primary remote");
    assert_eq!(primary.provider, ProviderKind::GitRepo);
    assert_eq!(primary.auth_kind, ProviderAuthKind::Pat);
    match &primary.locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            base_url,
            namespace,
            repository,
            branch,
            root_path,
            ..
        } => {
            assert_eq!(*host_kind, GitHostKind::GitHub);
            assert_eq!(base_url.as_deref(), Some("https://github.com"));
            assert_eq!(namespace.as_deref(), Some("demo"));
            assert_eq!(repository.as_deref(), Some("mica-vault"));
            assert_eq!(branch, "main");
            assert_eq!(root_path.as_deref(), Some(".mica-term-sync"));
        }
        other => panic!("unexpected primary locator: {other:?}"),
    }
    assert_eq!(app.get_sync_modal_validation_state().as_str(), "success");
    assert_eq!(app.get_sync_modal_provider_label().as_str(), "GitHub");
}

#[test]
fn sync_modal_security_pause_state_is_visible() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("paused-sync-modal");
    let bundle = BootstrapBundle {
        vault_id: "vault-main".into(),
        remotes: vec![BootstrapRemoteConfig {
            remote_id: "remote-primary".into(),
            role: RemoteRole::Primary,
            provider: ProviderKind::GitRepo,
            locator: BootstrapRemoteLocator::GitRepo {
                host_kind: GitHostKind::GitHub,
                remote_url: "https://github.com/demo/mica-vault.git".into(),
                branch: "main".into(),
                base_url: Some("https://github.com".into()),
                api_base_url: Some("https://api.github.com".into()),
                namespace: Some("demo".into()),
                repository: Some("mica-vault".into()),
                root_path: Some(".mica-term-sync".into()),
                display_name: Some("demo/mica-vault".into()),
            },
            credential_ref: Some("vault/bootstrap/remote-primary".into()),
            auth_kind: ProviderAuthKind::Pat,
            last_health: None,
        }],
        ..BootstrapBundle::default()
    };
    mica_term::app::vault::bootstrap::save_local_vault_bootstrap_state(
        temp_root.join("vault-bootstrap-state.json").as_path(),
        &mica_term::app::vault::bootstrap::LocalVaultBootstrapState {
            bundle,
            wrapped_vault_key: "wrapped-key-prev".into(),
            kdf: sample_remote_head("rev-0042").kdf,
            device_id: "device-a".into(),
            logical_revision: None,
            transport_revision_hint: None,
            base_revision: Some("rev-0042".into()),
            current_revision: Some("rev-0042".into()),
            local_snapshot_hash: Some("sha256:payload-prev".into()),
            last_local_change_at: Some("2026-05-19T08:00:00Z".into()),
            last_successful_push_at: Some("2026-05-19T08:00:00Z".into()),
            last_successful_pull_at: Some("2026-05-19T08:00:00Z".into()),
            last_sync_error: Some(
                "remote repository `demo/mica-vault` must stay private before sync can resume"
                    .into(),
            ),
            remote_safety_status: GitRemoteSafetyStatus::Paused,
        },
    )
    .expect("persist paused local state");

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
            ..VaultRuntimeOptions::default()
        },
    );

    app.invoke_open_sync_modal_requested();

    assert_eq!(app.get_sync_modal_mode().as_str(), "paused");
    assert!(
        app.get_sync_modal_status_text()
            .to_string()
            .to_ascii_lowercase()
            .contains("paused")
    );
    assert!(
        app.get_sync_modal_error_text()
            .to_string()
            .contains("must stay private")
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
            ..VaultRuntimeOptions::default()
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
            ..VaultRuntimeOptions::default()
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
    let metadata_source = Arc::new(FakeGitRepositoryMetadataSource::returning(Ok(
        sample_git_repository_metadata(
            "demo/mica-vault",
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::Writable,
        ),
    )));
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
            git_repo_metadata_source: metadata_source,
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("git-provider-kind".into(), "gitee".into());
    app.invoke_sync_modal_draft_changed("git-base-url".into(), "https://gitee.com".into());
    app.invoke_sync_modal_draft_changed("git-namespace".into(), "demo".into());
    app.invoke_sync_modal_draft_changed("git-repository".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-branch".into(), "mica-vault".into());
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "https".into());
    app.invoke_sync_modal_draft_changed("git-https-username".into(), "demo-user".into());
    app.invoke_sync_modal_draft_changed("git-pat".into(), "pat-primary-secret".into());
    app.invoke_sync_modal_validate_requested();
    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_validation_state().as_str() == "success"
    });
    app.invoke_sync_modal_draft_changed("master-password".into(), "vault-pass".into());

    app.invoke_sync_modal_primary_action_requested();

    let saved =
        load_local_vault_bootstrap_state(temp_root.join("vault-bootstrap-state.json").as_path())
            .expect("load local bootstrap state")
            .expect("expected persisted local bootstrap state");
    let primary = saved.bundle.primary_remote().expect("primary remote");
    assert_eq!(primary.provider, ProviderKind::GitRepo);
    assert_eq!(primary.auth_kind, ProviderAuthKind::Pat);
    match &primary.locator {
        BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            ..
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
fn sync_modal_round_trips_secret_visibility_flags() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.set_sync_modal_master_password_visible(true);
    app.set_sync_modal_git_https_secret_visible(true);
    app.set_sync_modal_git_ssh_passphrase_visible(true);

    assert!(app.get_sync_modal_master_password_visible());
    assert!(app.get_sync_modal_git_https_secret_visible());
    assert!(app.get_sync_modal_git_ssh_passphrase_visible());
}

#[test]
fn sync_modal_secret_visibility_resets_on_close_and_auth_mode_change() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("master-password-visibility".into(), "visible".into());
    assert!(app.get_sync_modal_master_password_visible());

    app.invoke_sync_modal_close_requested();
    app.invoke_open_sync_modal_requested();
    assert!(!app.get_sync_modal_master_password_visible());

    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "https".into());
    app.invoke_sync_modal_draft_changed("git-https-secret-visibility".into(), "visible".into());
    assert!(app.get_sync_modal_git_https_secret_visible());
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "ssh".into());
    assert!(!app.get_sync_modal_git_https_secret_visible());

    app.invoke_sync_modal_draft_changed("git-ssh-passphrase-visibility".into(), "visible".into());
    assert!(app.get_sync_modal_git_ssh_passphrase_visible());
    app.invoke_sync_modal_draft_changed("git-auth-mode".into(), "https".into());
    assert!(!app.get_sync_modal_git_ssh_passphrase_visible());
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
            ..VaultRuntimeOptions::default()
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
fn sync_modal_master_password_visibility_resets_after_successful_submit() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("visibility-reset-after-submit");
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
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
            ..VaultRuntimeOptions::default()
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_draft_changed("master-password-visibility".into(), "visible".into());
    assert!(app.get_sync_modal_master_password_visible());

    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert!(!app.get_sync_modal_master_password_visible());
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
            ..VaultRuntimeOptions::default()
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
            ..VaultRuntimeOptions::default()
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
            ..VaultRuntimeOptions::default()
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
    let metadata_source = Arc::new(FakeGitRepositoryMetadataSource::returning(Ok(
        sample_git_repository_metadata(
            "demo/mica-vault",
            GitRepositoryVisibility::Private,
            GitRepositoryWritePermission::Writable,
        ),
    )));
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
            git_repo_metadata_source: metadata_source.clone(),
            ..VaultRuntimeOptions::default()
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
    assert_eq!(metadata_source.fetch_count(), 1);

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
            ..VaultRuntimeOptions::default()
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
            ..VaultRuntimeOptions::default()
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
            ..VaultRuntimeOptions::default()
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

#[test]
fn sync_modal_repository_field_right_click_keeps_selection_and_typing_owner() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_sync_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "git-base-url" {
            app.set_sync_modal_git_base_url(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_sync_modal_open(true);
    app.set_sync_modal_mode("configured".into());
    app.set_sync_modal_git_auth_mode("https".into());
    settle_modal_ui();

    let sync_modal = ElementHandle::find_by_element_type_name(&app, "SyncVaultModal")
        .next()
        .expect("find sync vault modal");
    let base_url_field = sync_modal
        .query_descendants()
        .match_inherits("DialogTextField")
        .find_all()
        .into_iter()
        .find(|field| field.size().height > 0.0)
        .expect("find first visible sync modal text field");
    let base_url_input = descendant_by_id(&base_url_field, "DialogTextField::field-input");
    let base_url_position = element_center(&base_url_input);

    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    dispatch_text_sequence(&app, "https://vault.example.com");

    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("sentinel-before-right-click");
    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Right);
    dispatch_text_key_chord(&app, "c", true, false, false);

    assert_eq!(
        clipboard_text(),
        "https://vault.example.com",
        "sync modal text fields should preserve the active selection across a right-click before copy runs"
    );

    dispatch_text_sequence(&app, "Z");

    assert_eq!(
        app.get_sync_modal_git_base_url().as_str(),
        "Z",
        "after right-clicking a selected sync modal field, the next typed character should still replace the same field selection"
    );
}

#[test]
fn sync_modal_base_url_context_menu_copy_and_paste_actions_work() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_sync_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "git-base-url" {
            app.set_sync_modal_git_base_url(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_sync_modal_open(true);
    app.set_sync_modal_mode("configured".into());
    app.set_sync_modal_git_auth_mode("https".into());
    settle_modal_ui();

    let base_url_field =
        ElementHandle::find_by_element_id(&app, "SyncVaultModal::git-base-url-field")
            .next()
            .expect("find sync base url field");
    let base_url_input = descendant_by_id(&base_url_field, "DialogTextField::field-input");
    let base_url_position = element_center(&base_url_input);

    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    dispatch_text_sequence(&app, "https://vault.example.com");
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-base-url-copy");
    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Right);

    assert!(
        app.get_text_context_menu_copy_enabled(),
        "public sync repository fields should expose Copy through the shared text context menu"
    );

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let copy_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::copy-row");
    dispatch_pointer_click(&app, element_center(&copy_row), PointerEventButton::Left);

    assert_eq!(
        clipboard_text(),
        "https://vault.example.com",
        "the shared text context menu Copy row should keep working for public sync repository fields"
    );

    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    set_clipboard_text("https://mirror.example.com");
    dispatch_pointer_click(&app, base_url_position, PointerEventButton::Right);

    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_sync_modal_git_base_url().as_str(),
        "https://mirror.example.com",
        "the shared text context menu Paste row should keep working for public sync repository fields"
    );
}

#[test]
fn sync_modal_master_password_pastes_without_exposing_copy_even_when_revealed() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();
    app.on_sync_modal_draft_changed(move |field, value| {
        let app = app_weak.upgrade().expect("upgrade app");
        if field.as_str() == "master-password" {
            app.set_sync_modal_master_password(value);
        }
    });

    app.show().expect("show app window");
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    app.set_sync_modal_open(true);
    app.set_sync_modal_mode("not-configured".into());
    settle_modal_ui();

    let password_input =
        ElementHandle::find_by_element_id(&app, "SyncVaultModal::master-password-field")
            .next()
            .map(|password_field| descendant_by_id(&password_field, "DialogTextField::field-input"))
            .expect("find sync master password input");
    let password_position = element_center(&password_input);

    dispatch_pointer_click(&app, password_position, PointerEventButton::Left);
    dispatch_text_sequence(&app, "vault-master-secret");
    dispatch_text_key_chord(&app, "a", true, false, false);

    set_clipboard_text("sentinel-before-master-password-menu");
    dispatch_pointer_click(&app, password_position, PointerEventButton::Right);

    assert!(
        !app.get_text_context_menu_copy_enabled(),
        "secret sync credentials should not expose Copy through the shared text context menu"
    );
    assert!(
        app.get_text_context_menu_paste_enabled(),
        "secret sync credentials should still expose Paste through the shared text context menu"
    );

    set_clipboard_text("vault-master-replaced");
    let text_menu_overlay =
        ElementHandle::find_by_element_id(&app, "AppWindow::text-context-menu-overlay")
            .next()
            .expect("find text context menu overlay");
    let paste_row = descendant_by_id(&text_menu_overlay, "TextContextMenuOverlay::paste-row");
    dispatch_pointer_click(&app, element_center(&paste_row), PointerEventButton::Left);

    assert_eq!(
        app.get_sync_modal_master_password().as_str(),
        "vault-master-replaced",
        "the shared text context menu Paste row should still work for secret sync credential fields"
    );

    app.set_sync_modal_master_password_visible(true);
    settle_modal_ui();
    dispatch_pointer_click(&app, password_position, PointerEventButton::Left);
    dispatch_text_key_chord(&app, "a", true, false, false);
    dispatch_pointer_click(&app, password_position, PointerEventButton::Right);

    assert!(
        !app.get_text_context_menu_copy_enabled(),
        "revealing a secret sync credential should not automatically grant Copy in the shared text context menu"
    );
}
