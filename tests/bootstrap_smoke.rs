//! Basic bootstrap helper coverage for the binary entrypoint.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload,
    PersistedAssetSocks5ProxySpec, PersistedAssetSshProxySpec, PersistedSshConnectionSpec,
    catalog_to_asset_tree,
};
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter, app_title,
    build_shared_app_credential_store_for_paths,
    bind_top_status_bar_with_injected_services_and_vault_runtime,
    bind_top_status_bar_with_store_and_effects_and_asset_repo,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer,
    default_window_size,
    VaultProviderFactory, VaultRuntimeOptions,
};
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, StoredSshSecretBundle,
    load_secret_bundle, persist_secret_bundle, ssh_credential_ref,
};
use mica_term::app::ssh::known_hosts::{
    KnownHostCheck, KnownHostsService, default_known_hosts_path,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalKeyKind, TerminalMouseInput, TerminalSession,
    TerminalSurfaceState, UnknownHostKeyError,
};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::vault::bootstrap::{
    LocalVaultBootstrapState, load_local_vault_bootstrap_state, save_local_vault_bootstrap_state,
};
use mica_term::app::vault::cache::store_encrypted_cache;
use mica_term::app::vault::crypto::{encrypt_snapshot, generate_vault_key, wrap_vault_key};
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, KdfConfig,
    ProviderAuthKind, ProviderKind, RemoteRole, SnapshotSyncPreferences,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, VaultProvider};
use mica_term::app::vault::snapshot::export_vault_snapshot;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::metrics::ShellMetrics;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use russh::keys::{HashAlg, PublicKey};
use secrecy::SecretString;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model};
use tokio::sync::mpsc;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: Mutex<()> = Mutex::new(());

#[derive(Default)]
struct AssetRepoState {
    load_calls: usize,
    save_attempts: Vec<PersistedAssetCatalog>,
}

struct RecordingAssetRepo {
    loaded_catalog: PersistedAssetCatalog,
    state: Rc<RefCell<AssetRepoState>>,
    save_error: Option<&'static str>,
}

impl RecordingAssetRepo {
    fn new(
        loaded_catalog: PersistedAssetCatalog,
        state: Rc<RefCell<AssetRepoState>>,
        save_error: Option<&'static str>,
    ) -> Self {
        Self {
            loaded_catalog,
            state,
            save_error,
        }
    }
}

impl AssetCatalogRepository for RecordingAssetRepo {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        self.state.borrow_mut().load_calls += 1;
        Ok(self.loaded_catalog.clone())
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        self.state.borrow_mut().save_attempts.push(catalog.clone());
        if let Some(message) = self.save_error {
            return Err(anyhow!(message));
        }

        Ok(())
    }
}

#[derive(Clone, Default)]
struct FakeLauncher;

#[derive(Clone)]
struct TofuAwareLauncher {
    host_key: PublicKey,
}

#[derive(Clone)]
struct AsyncProjectionLauncher;

#[derive(Clone, Default)]
struct InteractiveProjectionLauncher;

#[derive(Clone, Default)]
struct PasteProjectionLauncher;

#[derive(Clone, Default)]
struct ScrollProjectionLauncher;

#[derive(Clone)]
struct FailingProbeLauncher {
    message: &'static str,
}

#[derive(Clone)]
struct StoredSecretProbeLauncher {
    store: Arc<dyn CredentialStore>,
    message: &'static str,
}

#[derive(Default)]
struct RecordingLauncherState {
    launch_profiles: Vec<ConnectionProfile>,
    probe_profiles: Vec<ConnectionProfile>,
}

#[derive(Clone)]
struct RecordingLauncher {
    state: Arc<Mutex<RecordingLauncherState>>,
}

#[derive(Clone)]
struct SuccessfulPrivateKeyImporter {
    path: std::path::PathBuf,
    content: &'static str,
}

#[derive(Clone, Default)]
struct CancelledPrivateKeyImporter;

#[derive(Clone)]
struct FailingPrivateKeyImporter {
    message: &'static str,
}

#[derive(Default)]
struct UnavailableCredentialStore;

struct NoopRuntimeControl;

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

struct InteractiveProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

struct PasteProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct ScrollProjectionState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
}

struct ScrollProjectionRuntimeControl {
    state: ScrollProjectionState,
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

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
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

impl TofuAwareLauncher {
    fn new(host_key: PublicKey) -> Self {
        Self { host_key }
    }

    fn ensure_trusted(&self, profile: &ConnectionProfile) -> Result<()> {
        let known_hosts = KnownHostsService::new(default_known_hosts_path()?);
        match known_hosts.check(&profile.host, profile.port, &self.host_key)? {
            KnownHostCheck::Trusted => Ok(()),
            KnownHostCheck::Unknown { fingerprint } => Err(UnknownHostKeyError {
                host: profile.host.clone(),
                port: profile.port,
                fingerprint,
                public_key_openssh: self.host_key.to_openssh().expect("encode tofu host key"),
            }
            .into()),
            KnownHostCheck::Changed { expected, actual } => Err(anyhow!(
                "SSH host key changed for `{}`:{} (expected {}, got {})",
                profile.host,
                profile.port,
                expected,
                actual
            )),
        }
    }
}

impl SessionRuntimeLauncher for TofuAwareLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let launcher = self.clone();
        Box::pin(async move {
            launcher.ensure_trusted(&profile)?;
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let launcher = self.clone();
        Box::pin(async move { launcher.ensure_trusted(&profile) })
    }
}

impl SessionRuntimeLauncher for AsyncProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let _ = event_tx.send(SessionRuntimeEvent::Connected);
                let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                    mica_term::app::ssh::runtime::TerminalSurfaceState::from_visible_lines(
                        session_id,
                        1,
                        24,
                        80,
                        vec!["welcome to mica-term".into()],
                    ),
                ));
            });
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for InteractiveProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                terminal_surface_with_cells(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(InteractiveProjectionRuntimeControl {
                session_id,
                event_tx,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for PasteProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                TerminalSurfaceState::from_visible_lines(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(PasteProjectionRuntimeControl {
                session_id,
                event_tx,
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for ScrollProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let state = ScrollProjectionState::default();
            let surface = bootstrap_surface_with_viewport(session_id, 1, 3, 8);
            *state
                .surface
                .lock()
                .expect("lock scroll projection surface") = Some(surface.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(ScrollProjectionRuntimeControl { state })
                as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeLauncher for FailingProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let message = self.message;
        Box::pin(async move { Err(anyhow!(message)) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let message = self.message;
        Box::pin(async move { Err(anyhow!(message)) })
    }
}

impl SessionRuntimeControl for InteractiveProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("$ {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("$ {}", rendered)],
            ),
        ));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.send_text_input(text)
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            terminal_surface_with_cells(
                self.session_id,
                2,
                24,
                80,
                vec![
                    "welcome to mica-term".into(),
                    "mouse input forwarded".into(),
                ],
            ),
        ));
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for PasteProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("text {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("key {}", rendered)],
            ),
        ));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("paste {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for ScrollProjectionRuntimeControl {
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

    fn scroll_viewport_lines(&self, delta: i32) -> Result<TerminalSurfaceState> {
        let mut surface = self
            .state
            .surface
            .lock()
            .expect("lock scroll projection surface")
            .clone()
            .expect("current scroll projection surface");
        let next_offset = (surface.viewport_offset_lines as i32 + delta)
            .clamp(0, surface.viewport_max_offset_lines as i32) as u32;
        surface = bootstrap_surface_with_viewport(
            surface.session_id,
            surface.seqno.saturating_add(1),
            next_offset,
            surface.viewport_max_offset_lines,
        );
        *self
            .state
            .surface
            .lock()
            .expect("lock scroll projection surface") = Some(surface.clone());
        Ok(surface)
    }
}

impl SessionRuntimeLauncher for StoredSecretProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let store = Arc::clone(&self.store);
        let message = self.message;
        Box::pin(async move {
            if profile
                .password
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                return Ok(());
            }

            let credential_ref = profile
                .credential_ref
                .as_deref()
                .ok_or_else(|| anyhow!(message))?;
            let bundle = load_secret_bundle(store.as_ref(), credential_ref)?;
            if bundle
                .password
                .as_deref()
                .is_some_and(|value| !value.trim().is_empty())
            {
                Ok(())
            } else {
                Err(anyhow!(message))
            }
        })
    }
}

impl SessionRuntimeLauncher for RecordingLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .expect("lock recording launcher state")
                .launch_profiles
                .push(profile);
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let state = Arc::clone(&self.state);
        Box::pin(async move {
            state
                .lock()
                .expect("lock recording launcher state")
                .probe_profiles
                .push(profile);
            Ok(())
        })
    }
}

impl PrivateKeyImporter for SuccessfulPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(Some(ImportedPrivateKey {
            path: self.path.clone(),
            content: self.content.into(),
        }))
    }
}

impl PrivateKeyImporter for CancelledPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(None)
    }
}

impl PrivateKeyImporter for FailingPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Err(anyhow!(self.message))
    }
}

impl CredentialStore for UnavailableCredentialStore {
    fn put_secret(&self, _key: &str, _value: &str) -> Result<()> {
        Err(anyhow!("system credential store unavailable"))
    }

    fn get_secret(&self, _key: &str) -> Result<Option<String>> {
        Err(anyhow!("system credential store unavailable"))
    }

    fn delete_secret(&self, _key: &str) -> Result<()> {
        Err(anyhow!("system credential store unavailable"))
    }
}

fn bind_with_fake_sessions(app: &AppWindow, asset_repo: Option<Rc<dyn AssetCatalogRepository>>) {
    bind_with_launcher(app, asset_repo, Arc::new(FakeLauncher));
}

fn bind_with_launcher(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        Arc::new(MemoryCredentialStore::default()),
    );
}

fn bind_with_launcher_and_credential_store(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        credential_store,
    );
}

fn bind_with_launcher_and_credential_store_and_private_key_importer(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        credential_store,
        private_key_importer,
    );
}

fn bind_with_vault_runtime(
    app: &AppWindow,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    vault_runtime: VaultRuntimeOptions,
) {
    bind_top_status_bar_with_injected_services_and_vault_runtime(
        app,
        None,
        default_platform_window_effects(),
        None,
        launcher,
        credential_store,
        Arc::new(CancelledPrivateKeyImporter),
        vault_runtime,
    );
}

fn sample_known_hosts_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-bootstrap-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_vault_runtime_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-vault-runtime-{}-{}",
        label,
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

fn sample_credential_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "mica-term-bootstrap-credentials-{}-{}",
        label,
        Uuid::new_v4()
    ))
}

fn bootstrap_surface_with_viewport(
    session_id: uuid::Uuid,
    seqno: usize,
    offset: u32,
    max_offset: u32,
) -> TerminalSurfaceState {
    let mut surface = TerminalSurfaceState::from_visible_lines(
        session_id,
        seqno,
        24,
        80,
        vec![format!("offset {offset}")],
    );
    surface.viewport_offset_lines = offset;
    surface.viewport_max_offset_lines = max_offset;
    surface.viewport_at_bottom = offset == 0;
    surface.default_fg_rgba = 0xff1f_2328;
    surface.default_bg_rgba = 0xfff7_f9fc;
    surface.cursor.fg_rgba = 0xffff_ffff;
    surface.cursor.bg_rgba = 0xff25_63eb;
    surface
}

fn terminal_surface_with_cells(
    session_id: uuid::Uuid,
    seqno: usize,
    rows: u32,
    cols: u32,
    visible_lines: Vec<String>,
) -> TerminalSurfaceState {
    let mut session = TerminalSession::new(rows as usize, cols as usize);
    let transcript = visible_lines
        .iter()
        .map(|line| format!("{line}\r\n"))
        .collect::<String>();
    session.apply_remote_bytes(transcript.as_bytes());

    let mut surface = session.surface_state(session_id);
    surface.seqno = seqno;
    surface
}

fn settle_terminal_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn terminal_interaction_position(app: &AppWindow) -> LogicalPosition {
    LogicalPosition::new(
        app.get_layout_main_workspace_x() + 96.0,
        app.get_layout_titlebar_height() + 96.0,
    )
}

fn focus_workspace_terminal(app: &AppWindow) {
    let position = terminal_interaction_position(app);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti bootstrap-tofu@example.com",
    )
    .expect("parse public key")
}

fn sample_logging_root(label: &str) -> std::path::PathBuf {
    std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join(format!("{label}-{}", uuid::Uuid::new_v4()))
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) -> String {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    app.get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string()
}

#[test]
fn settings_panel_can_create_a_vault_and_persist_local_bootstrap_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("create");
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_settings_panel_requested();
    app.invoke_vault_create_requested("correct horse battery staple".into());

    assert_eq!(app.get_vault_lock_state_label().as_str(), "Unlocked");
    assert!(
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .is_some()
    );
}

#[test]
fn unlocking_existing_vault_restores_cached_snapshot_without_loading_while_locked() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("unlock");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.42");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();
    let known_hosts_path = sample_known_hosts_path("vault-unlock");
    let snapshot = export_vault_snapshot(
        &asset_tree,
        source_store.as_ref(),
        &known_hosts_path,
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .unwrap();
    let vault_key = generate_vault_key();
    let encrypted = encrypt_snapshot(&snapshot, &vault_key).unwrap();
    let wrapped_vault_key =
        serde_json::to_string(&wrap_vault_key(&password, &sample_vault_kdf(), &vault_key).unwrap())
            .unwrap();
    save_local_vault_bootstrap_state(
        &temp_root.join("vault-bootstrap-state.json"),
        &LocalVaultBootstrapState {
            bundle: sample_bootstrap_bundle_with_primary_and_mirror(),
            wrapped_vault_key,
            kdf: sample_vault_kdf(),
            current_revision: Some("rev-0001".into()),
        },
    )
    .unwrap();
    store_encrypted_cache(&temp_root.join("cache"), "vault-main", &encrypted).unwrap();

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
        },
    );

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert!(credential_store.get_secret(&credential_ref).unwrap().is_none());

    app.invoke_vault_unlock_requested("vault-pass".into());

    assert_eq!(app.get_vault_lock_state_label().as_str(), "Unlocked");
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(credential_store.get_secret(&credential_ref).unwrap().is_some());
}

#[test]
fn manual_vault_sync_reports_mirror_degradation_after_primary_commit() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("mirror-degraded");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    mirror.set_write_error(Some("mirror unavailable"));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    provider_factory.insert(mirror.clone());

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_settings_panel_requested();
    app.invoke_vault_create_requested("vault-pass".into());

    app.invoke_vault_sync_now_requested();

    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(mirror.recorded_writes().len(), 0);
    assert!(
        app.get_vault_primary_status_label()
            .as_str()
            .contains("Mirror degraded")
    );
}

#[test]
fn manual_vault_sync_surfaces_provider_auth_errors_in_panel_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("provider-auth");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_read_error(Some("token expired"));
    let mirror = Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);
    provider_factory.insert(mirror);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_settings_panel_requested();
    app.invoke_vault_create_requested("vault-pass".into());

    app.invoke_vault_sync_now_requested();

    assert!(
        app.get_vault_primary_status_label()
            .as_str()
            .contains("Provider auth error")
    );
}

#[test]
fn locking_vault_clears_decrypted_assets_and_secrets_from_memory() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("lock");
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let credential_ref = ssh_credential_ref(&ssh_id, SshCredentialKind::SavedSecrets);
    app.invoke_open_settings_panel_requested();
    app.invoke_vault_create_requested("vault-pass".into());

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(credential_store.get_secret(&credential_ref).unwrap().is_some());

    app.invoke_vault_lock_requested();

    assert_eq!(app.get_vault_lock_state_label().as_str(), "Locked");
    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert!(credential_store.get_secret(&credential_ref).unwrap().is_none());
}

fn loaded_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["folder-root".into(), "ssh-root".into()],
        nodes: BTreeMap::from([
            (
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            ),
            (
                "ssh-root".into(),
                PersistedAssetNode {
                    id: "ssh-root".into(),
                    parent_id: None,
                    title: "Gateway".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "gateway.example.com".into(),
                        user: "ops".into(),
                        port: "2022".into(),
                        auth_method: "password".into(),
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
        ]),
    }
}

fn loaded_legacy_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-legacy".into()],
        nodes: BTreeMap::from([(
            "ssh-legacy".into(),
            PersistedAssetNode {
                id: "ssh-legacy".into(),
                parent_id: None,
                title: "Legacy Gateway".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "legacy.example.com".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: String::new(),
                    private_key_source: String::new(),
                    private_key_path: String::new(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: String::new(),
                    credential_ref: None,
                }),
            },
        )]),
    }
}

fn loaded_saved_password_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-prod".into()],
        nodes: BTreeMap::from([(
            "ssh-prod".into(),
            PersistedAssetNode {
                id: "ssh-prod".into(),
                parent_id: None,
                title: "Prod Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.12".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Saved credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-prod".into()),
                }),
            },
        )]),
    }
}

fn loaded_saved_private_key_path_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-path".into()],
        nodes: BTreeMap::from([(
            "ssh-path".into(),
            PersistedAssetNode {
                id: "ssh-path".into(),
                parent_id: None,
                title: "Path Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.20".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "private-key".into(),
                    private_key_source: "path".into(),
                    private_key_path: "/tmp/id_ed25519".into(),
                    environment: String::new(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Saved passphrase".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-path".into()),
                }),
            },
        )]),
    }
}

fn loaded_saved_socks5_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-socks5".into()],
        nodes: BTreeMap::from([(
            "ssh-socks5".into(),
            PersistedAssetNode {
                id: "ssh-socks5".into(),
                parent_id: None,
                title: "SOCKS5 Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.30".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::Socks5(PersistedAssetSocks5ProxySpec {
                        host: "proxy.example.net".into(),
                        port: "1080".into(),
                        username: "ops-proxy".into(),
                        password_credential_ref: Some("ssh/saved-secrets/ssh-socks5".into()),
                    }),
                    remark: "Saved proxy credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-socks5".into()),
                }),
            },
        )]),
    }
}

fn loaded_saved_upstream_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-upstream".into(), "ssh-target".into()],
        nodes: BTreeMap::from([
            (
                "ssh-upstream".into(),
                PersistedAssetNode {
                    id: "ssh-upstream".into(),
                    parent_id: None,
                    title: "Upstream Bastion".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "10.0.0.40".into(),
                        user: "ops".into(),
                        port: "22".into(),
                        auth_method: "password".into(),
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::None,
                        remark: String::new(),
                        credential_ref: None,
                    }),
                },
            ),
            (
                "ssh-target".into(),
                PersistedAssetNode {
                    id: "ssh-target".into(),
                    parent_id: None,
                    title: "Target Bastion".into(),
                    kind: PersistedAssetKind::SshConnection,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                        host: "10.0.0.41".into(),
                        user: "ops".into(),
                        port: "22".into(),
                        auth_method: "password".into(),
                        private_key_source: "content".into(),
                        private_key_path: String::new(),
                        environment: "prod".into(),
                        proxy: PersistedAssetSshProxySpec::SshAsset {
                            asset_id: "ssh-upstream".into(),
                        },
                        remark: "Saved upstream reference".into(),
                        credential_ref: None,
                    }),
                },
            ),
        ]),
    }
}

fn loaded_missing_upstream_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-missing-upstream".into()],
        nodes: BTreeMap::from([(
            "ssh-missing-upstream".into(),
            PersistedAssetNode {
                id: "ssh-missing-upstream".into(),
                parent_id: None,
                title: "Broken Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.50".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::SshAsset {
                        asset_id: "ssh-upstream-missing".into(),
                    },
                    remark: "Missing upstream reference".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-missing-upstream".into()),
                }),
            },
        )]),
    }
}

fn context_menu_item_enabled(app: &AppWindow, action_id: &str) -> bool {
    let items = app.get_assets_context_menu_primary_items();
    (0..items.row_count())
        .filter_map(|index| items.row_data(index))
        .find(|item| item.id.as_str() == action_id)
        .map(|item| item.enabled)
        .unwrap_or(false)
}

#[test]
fn bootstrap_exposes_shell_default_window_budget() {
    assert_eq!(app_title(), "Mica Term");
    assert_eq!(
        default_window_size(),
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
    );
}

#[test]
fn bootstrap_shared_credential_store_prefers_encrypted_cache_when_preferred_store_is_unavailable() {
    let encrypted_root = sample_credential_root("secure");
    let recovery_root = sample_credential_root("recovery");
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);
    let store = build_shared_app_credential_store_for_paths(
        Some(Arc::new(UnavailableCredentialStore) as Arc<dyn CredentialStore>),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    persist_secret_bundle(
        store.as_ref(),
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist bundle through shared bootstrap store");

    assert_eq!(
        load_secret_bundle(store.as_ref(), credential_ref.as_str())
            .expect("reload shared credential bundle")
            .password
            .as_deref(),
        Some("super-secret")
    );

    let encrypted_bytes = fs::read(
        encrypted_root
            .join("ssh")
            .join("saved-secrets")
            .join("asset-prod.bin"),
    )
    .expect("read encrypted fallback file");
    assert!(!String::from_utf8_lossy(&encrypted_bytes).contains("super-secret"));
    assert!(
        !recovery_root
            .join("ssh")
            .join("saved-secrets")
            .join("asset-prod.json")
            .exists(),
        "plain recovery store should not be used when encrypted fallback succeeds"
    );

    let _ = fs::remove_dir_all(encrypted_root);
    let _ = fs::remove_dir_all(recovery_root);
}

#[test]
fn bootstrap_loads_catalog_before_first_asset_projection_sync() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Team");
    assert_eq!(rows.row_data(1).unwrap().label.as_str(), "Gateway");

    let state = repo_state.borrow();
    assert_eq!(state.load_calls, 1);
    assert!(state.save_attempts.is_empty());
}

#[test]
fn activating_legacy_saved_ssh_asset_defaults_missing_auth_fields_and_opens_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_legacy_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("legacy ssh asset")
        .id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
}

#[test]
fn opening_saved_ssh_asset_twice_creates_two_tabs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .session_id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .session_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn editing_legacy_saved_ssh_asset_reuses_fallback_saved_secret_for_test_connection() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_legacy_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        &ssh_credential_ref("ssh-legacy", SshCredentialKind::SavedSecrets),
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist legacy saved ssh secret");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(StoredSecretProbeLauncher {
            store: Arc::clone(&credential_store),
            message: "missing SSH password secret for `Legacy Gateway`",
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("legacy ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());

    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );
}

#[test]
fn editing_saved_password_modal_hydrates_real_secret_masked() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_password_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-prod",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh secret");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!app.get_asset_ssh_modal_password_visible());
}

#[test]
fn editing_saved_socks5_modal_hydrates_proxy_fields_and_proxy_password() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_socks5_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-socks5",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: Some("proxy-secret".into()),
        },
    )
    .expect("persist saved socks5 secret bundle");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved socks5 ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "socks5");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_host().as_str(),
        "proxy.example.net"
    );
    assert_eq!(app.get_asset_ssh_modal_proxy_socks5_port().as_str(), "1080");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_username().as_str(),
        "ops-proxy"
    );
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_password().as_str(),
        "proxy-secret"
    );
    assert!(!app.get_asset_ssh_modal_proxy_socks5_password_visible());
}

#[test]
fn editing_saved_upstream_ssh_modal_projects_selected_upstream_asset_id() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    bind_with_fake_sessions(&app, Some(asset_repo));

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .expect("target ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "ssh-asset");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_ssh_asset_id().as_str(),
        "ssh-upstream"
    );
}

#[test]
fn test_connection_with_missing_upstream_reports_inline_feedback_without_probe() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_missing_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-missing-upstream",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist missing-upstream ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("broken ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "upstream SSH asset `ssh-upstream-missing` was not found"
    );
}

#[test]
fn connect_with_missing_upstream_reports_inline_feedback_without_launch() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_missing_upstream_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-missing-upstream",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist missing-upstream ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("broken ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "upstream SSH asset `ssh-upstream-missing` was not found"
    );
}

#[test]
fn saved_password_asset_rehydrates_after_rebinding_with_same_store() {
    i_slint_backend_testing::init_no_event_loop();

    let shared_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let first_app = AppWindow::new().unwrap();
    let first_repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let first_asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&first_repo_state),
        None,
    ));
    bind_with_launcher_and_credential_store(
        &first_app,
        Some(first_asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&shared_store),
    );

    first_app.invoke_assets_create_action_selected("new-ssh-connection".into());
    first_app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    first_app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    first_app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    first_app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    first_app.invoke_asset_ssh_modal_action_requested("save".into());

    let rebound_catalog = first_repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("saved catalog snapshot")
        .clone();

    let second_app = AppWindow::new().unwrap();
    let second_repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let second_asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        rebound_catalog,
        Rc::clone(&second_repo_state),
        None,
    ));
    bind_with_launcher_and_credential_store(
        &second_app,
        Some(second_asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&shared_store),
    );

    let ssh_id = second_app
        .get_console_asset_items()
        .row_data(0)
        .expect("rebound ssh asset")
        .id
        .to_string();

    second_app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    second_app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(second_app.get_asset_modal_open());
    assert_eq!(second_app.get_asset_ssh_modal_password().as_str(), "secret");
    assert!(!second_app.get_asset_ssh_modal_password_visible());
}

#[test]
fn editing_saved_private_key_path_modal_hydrates_saved_passphrase() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-path",
        &StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh passphrase");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_dialog_title().as_str(),
        "Edit SSH Connection"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_path().as_str(),
        "/tmp/id_ed25519"
    );
    assert_eq!(app.get_asset_ssh_modal_passphrase().as_str(), "hunter2");
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_private_key_content().as_str(), "");
}

#[test]
fn editing_saved_private_key_path_modal_saving_blank_passphrase_deletes_saved_passphrase() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-path",
        &StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: Some("hunter2".into()),
            proxy_socks5_password: None,
        },
    )
    .expect("persist saved ssh passphrase");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("passphrase".into(), "".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert!(
        load_secret_bundle(credential_store.as_ref(), "ssh/saved-secrets/ssh-path")
            .expect("load cleared saved secret")
            .is_empty()
    );
    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after clear")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-path")
        .expect("saved ssh path node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.credential_ref, None);
}

#[test]
fn importing_private_key_into_saved_path_asset_migrates_it_to_content_mode_on_save() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_private_key_path_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(SuccessfulPrivateKeyImporter {
            path: std::path::PathBuf::from("/tmp/id_ed25519"),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n",
        }),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh path asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(
        app.get_asset_ssh_modal_private_key_content().as_str(),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n"
    );

    app.invoke_asset_ssh_modal_action_requested("save".into());

    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after import")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-path")
        .expect("saved ssh path node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.auth_method, "private-key");
    assert_eq!(spec.private_key_source, "content");
    assert_eq!(spec.private_key_path, "");
    assert_eq!(
        spec.credential_ref.as_deref(),
        Some("ssh/saved-secrets/ssh-path")
    );

    let bundle = load_secret_bundle(credential_store.as_ref(), "ssh/saved-secrets/ssh-path")
        .expect("load imported secret bundle");
    assert_eq!(
        bundle.private_key_content.as_deref(),
        Some("-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n")
    );
}

#[test]
fn importing_private_key_can_be_cancelled_without_mutating_modal_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(CancelledPrivateKeyImporter),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_ssh_modal_private_key_content().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "idle");
    assert_eq!(app.get_asset_ssh_modal_feedback_message().as_str(), "");
}

#[test]
fn importing_private_key_reports_feedback_when_file_selection_fails() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(FailingPrivateKeyImporter {
            message: "failed to read private key file",
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "failed to read private key file"
    );
}

#[test]
fn create_rename_delete_and_ssh_edit_trigger_repository_save() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_toggle_assets_search_requested();
    app.invoke_assets_search_query_changed("prod".into());
    app.invoke_toggle_assets_view_mode_requested();
    assert!(repo_state.borrow().save_attempts.is_empty());
    app.invoke_toggle_assets_view_mode_requested();
    app.invoke_assets_search_query_changed("".into());
    app.invoke_close_assets_search_requested();

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 1);

    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "socks5".into());
    app.invoke_asset_ssh_modal_draft_changed(
        "proxy_socks5_host".into(),
        "proxy.example.net".into(),
    );
    app.invoke_asset_ssh_modal_draft_changed("proxy_socks5_port".into(), "1080".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 2);

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 3);

    let ssh_id = app
        .get_console_asset_items()
        .row_data(1)
        .unwrap()
        .id
        .to_string();
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();
    assert_eq!(repo_state.borrow().save_attempts.len(), 4);
}

#[test]
fn saving_self_referential_upstream_proxy_is_blocked_before_runtime_launch() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_password_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-prod",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist existing ssh auth secret");
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_type".into(), "ssh-asset".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_ssh_asset_id".into(), ssh_id.into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert!(repo_state.borrow().save_attempts.is_empty());
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "SSH proxy chain contains a cycle"
    );
}

#[test]
fn save_failure_logs_error_without_persisting_ui_session_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("assets-persistence-save-error");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: vec!["folder-root".into()],
            nodes: BTreeMap::from([(
                "folder-root".into(),
                PersistedAssetNode {
                    id: "folder-root".into(),
                    parent_id: None,
                    title: "Prod".into(),
                    kind: PersistedAssetKind::Folder,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Folder,
                },
            )]),
        },
        Rc::clone(&repo_state),
        Some("disk full"),
    ));

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_top_status_bar_with_store_and_effects_and_asset_repo(
            &app,
            None,
            default_platform_window_effects(),
            Some(asset_repo),
        );

        let folder_id = app
            .get_console_asset_items()
            .row_data(0)
            .unwrap()
            .id
            .to_string();
        app.invoke_toggle_assets_tree_expansion_requested();
        app.invoke_toggle_assets_search_requested();
        app.invoke_assets_search_query_changed("Prod".into());
        app.invoke_asset_selected(folder_id.clone().into());
        app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
        app.invoke_assets_context_menu_action_invoked("rename-asset".into());
        app.invoke_asset_rename_modal_name_changed("Infra".into());
        app.invoke_confirm_asset_rename_requested();
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(log_content.contains("failed to save asset catalog"));
    assert!(log_content.contains("error=disk full"));

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    let persisted_tree = catalog_to_asset_tree(&save_attempts[0]);
    assert_eq!(persisted_tree.is_expanded("folder-root"), Some(false));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn save_action_persists_asset_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(app.get_active_workspace_session_id().is_empty());
}

#[test]
fn connect_action_opens_temporary_session_without_persisting_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert!(!app.get_active_workspace_session_id().is_empty());
}

#[test]
fn connect_action_keeps_session_ephemeral_and_does_not_persist_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        None,
    ));
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert!(repo_state.borrow().save_attempts.is_empty());
    assert_eq!(launcher_state.probe_profiles.len(), 1);
    assert_eq!(launcher_state.launch_profiles.len(), 1);
    assert!(
        launcher_state.launch_profiles[0]
            .asset_id
            .as_deref()
            .expect("ephemeral asset id")
            .starts_with("session:")
    );
}

#[test]
fn connect_action_reuses_existing_ephemeral_session_for_same_draft() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    for _ in 0..2 {
        app.invoke_assets_create_action_selected("new-ssh-connection".into());
        app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
        app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
        app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
        app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
        app.invoke_asset_ssh_modal_action_requested("connect".into());
    }

    assert_eq!(app.get_console_asset_items().row_count(), 0);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
}

#[test]
fn save_and_connect_persists_saved_secret_before_probe() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store(
        &app,
        None,
        Arc::new(StoredSecretProbeLauncher {
            store: Arc::clone(&credential_store),
            message: "missing SSH password secret for `Prod Bastion`",
        }),
        Arc::clone(&credential_store),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
}

#[test]
fn save_and_connect_persists_asset_then_opens_session_with_saved_identity() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        },
        Rc::clone(&repo_state),
        Some("persist failed before launch"),
    ));
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        Some(asset_repo),
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert_eq!(repo_state.borrow().save_attempts.len(), 1);
    assert!(launcher_state.probe_profiles.is_empty());
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "error");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "persist failed before launch"
    );
}

#[test]
fn test_connection_updates_feedback_without_creating_workspace_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingLauncher {
            state: Arc::clone(&launcher_state),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    let launcher_state = launcher_state
        .lock()
        .expect("lock recording launcher state");
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(launcher_state.probe_profiles.len(), 1);
    assert!(launcher_state.launch_profiles.is_empty());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );
}

#[test]
fn asset_activation_omits_internal_ssh_runtime_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-activation");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!log_content.contains("asset activated from explorer"));
    assert!(!log_content.contains("activating asset"));
    assert!(!log_content.contains("attempting to open ssh session after probe gate"));
    assert!(!log_content.contains("reusing existing workspace tab for activated ssh asset"));
    assert!(!log_content.contains("ssh probe succeeded, opening workspace session"));
    assert!(!log_content.contains("session manager registered new session handle"));
    assert!(!log_content.contains("session manager reused existing session handle"));
    assert!(!log_content.contains("resolved saved ssh asset profile inputs"));
    assert!(!log_content.contains("session manager probing ssh connection"));
    assert!(!log_content.contains("starting ssh runtime connection"));
    assert!(!log_content.contains("ssh runtime established transport connection"));
    assert!(!log_content.contains("authenticating ssh client"));
    assert!(!log_content.contains("loading stored ssh secret bundle"));
    assert!(!log_content.contains("ssh runtime completed authentication"));
    assert!(!log_content.contains("ssh runtime opened session channel"));
    assert!(!log_content.contains("ssh runtime negotiated pty"));
    assert!(!log_content.contains("ssh runtime requested remote shell"));
    assert!(!log_content.contains("session manager probe completed"));
    assert!(!log_content.contains("session manager received connected event"));
    assert!(!log_content.contains("session manager received disconnected event"));
    assert!(!log_content.contains("session manager received terminal surface update"));
    assert!(!log_content.contains("synchronized workspace projection from session manager"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn context_menu_open_omits_ssh_action_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-context-menu");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let runtime =
        build_test_logging_runtime(&paths, &AppLoggingConfig::new(AppLogMode::Debug)).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
        app.invoke_assets_context_menu_action_invoked("open-connection".into());
    });

    drop(runtime.guard);

    let log_content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(!log_content.contains("opening ssh asset from context menu"));
    assert!(!log_content.contains("opening ssh asset in a new tab from context menu"));
    assert!(!log_content.contains("activating asset"));
    assert!(!log_content.contains("attempting to open ssh session after probe gate"));
    assert!(!log_content.contains("ssh probe succeeded, opening workspace session"));
    assert!(!log_content.contains("session manager registered new session handle"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn save_action_persists_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(app.get_active_workspace_session_id().is_empty());
}

#[test]
fn ssh_context_menu_keeps_open_as_the_only_connection_action() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "open-connection"));
    assert!(!context_menu_item_enabled(&app, "open-in-new-tab"));
    assert!(!context_menu_item_enabled(&app, "close-connection"));

    app.invoke_close_assets_context_menu_requested();
    app.invoke_asset_activated(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "open-connection"));
    assert!(!context_menu_item_enabled(&app, "open-in-new-tab"));
    assert!(!context_menu_item_enabled(&app, "close-connection"));
}

#[test]
fn accepting_unknown_host_key_retries_test_connection_and_persists_known_host() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let known_hosts_path = sample_known_hosts_path("accept-test");
    let host_key = sample_public_key();
    let expected_fingerprint = host_key.fingerprint(HashAlg::Sha256).to_string();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "10.0.0.12:22");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        expected_fingerprint
    );

    app.invoke_ssh_host_key_modal_accept_requested();

    assert!(!app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );

    let known_hosts = KnownHostsService::new(&known_hosts_path);
    assert_eq!(
        known_hosts
            .check("10.0.0.12", 22, &host_key)
            .expect("check trusted host"),
        KnownHostCheck::Trusted
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn rejecting_unknown_host_key_for_open_session_surfaces_error_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = KNOWN_HOSTS_ENV_LOCK.lock().expect("lock known_hosts env");
    let known_hosts_path = sample_known_hosts_path("reject-open");
    let host_key = sample_public_key();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(TofuAwareLauncher::new(host_key.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());

    assert!(app.get_ssh_host_key_modal_open());
    app.invoke_ssh_host_key_modal_reject_requested();

    assert!(!app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "error");
    assert_eq!(
        app.get_workspace_session_error_detail().as_str(),
        "Rejected unknown SSH host key for `10.0.0.12`:22."
    );
    assert_eq!(
        KnownHostsService::new(&known_hosts_path)
            .check("10.0.0.12", 22, &host_key)
            .expect("recheck rejected host"),
        KnownHostCheck::Unknown {
            fingerprint: host_key.fingerprint(HashAlg::Sha256).to_string()
        }
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn runtime_events_refresh_workspace_terminal_projection_after_opening_saved_asset() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(AsyncProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(80));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(100));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connected");
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert_eq!(app.get_workspace_session_surface_seqno(), 1);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("first projected terminal row")
            .as_str(),
        "welcome to mica-term"
    );
}

#[test]
fn workspace_terminal_input_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_surface_seqno(), 1);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("initial visible row")
            .as_str(),
        "welcome to mica-term"
    );

    app.invoke_workspace_session_text_input("pwd".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(visible_lines.row_data(1).unwrap().as_str(), "$ pwd");
}

#[test]
fn workspace_terminal_paste_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PasteProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(visible_lines.row_data(1).unwrap().as_str(), "paste pwd");
}

#[test]
fn workspace_terminal_ctrl_shift_c_copies_selected_text_to_clipboard() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let selection_start = LogicalPosition::new(
        app.get_layout_main_workspace_x() + 18.0,
        app.get_layout_titlebar_height() + 56.0,
    );
    let selection_end = LogicalPosition::new(
        app.get_layout_main_workspace_x() + 92.0,
        app.get_layout_titlebar_height() + 56.0,
    );

    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: selection_start,
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: selection_start,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: selection_end,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: selection_end,
        button: PointerEventButton::Left,
    });

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "C".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "C".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard");

    assert!(
        copied
            .as_deref()
            .is_some_and(|text| text.contains("welcome")),
        "Ctrl+Shift+C should copy the current terminal selection into the system clipboard"
    );
}

#[test]
fn workspace_terminal_mouse_input_callback_updates_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_mouse_input(
        "down".into(),
        "left".into(),
        2,
        4,
        false,
        false,
        false,
    );

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "mouse input forwarded"
    );
}

#[test]
fn bootstrap_projects_terminal_scrollback_state_into_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);
    assert_eq!(app.get_workspace_session_viewport_max_offset_lines(), 8);
    assert!(!app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn bootstrap_projects_terminal_canvas_palette_into_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff1f_2328
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xfff7_f9fc
    );
    assert_eq!(
        app.get_workspace_session_cursor_fg().as_argb_encoded(),
        0xffff_ffff
    );
    assert_eq!(
        app.get_workspace_session_cursor_bg().as_argb_encoded(),
        0xff25_63eb
    );
}

#[test]
fn terminal_input_callback_snaps_scrolled_session_back_to_latest_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("scrolled visible line")
            .as_str(),
        "offset 3"
    );

    app.invoke_workspace_session_text_input("pwd".into());

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(0)
            .expect("bottom visible line")
            .as_str(),
        "offset 0"
    );
}

#[test]
fn ctrl_shift_letter_shortcuts_do_not_forward_remote_terminal_input() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Shift.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "f".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "f".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Shift.into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    settle_terminal_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(
        app.get_workspace_session_surface_seqno(),
        1,
        "reserved Ctrl+Shift shortcuts should stay local and must not trigger a remote surface update"
    );
    assert_eq!(visible_lines.row_count(), 1);
    assert_eq!(
        visible_lines
            .row_data(0)
            .expect("initial terminal line")
            .as_str(),
        "welcome to mica-term"
    );
}

#[test]
fn workspace_terminal_scroll_callbacks_update_active_session_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 8);
    assert!(!app.get_workspace_session_viewport_at_bottom());

    app.invoke_workspace_session_scroll_thumb_drag_requested(0.0);
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_pointer_wheel_accumulates_before_multi_line_scrollback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let position = terminal_interaction_position(&app);
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 60.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        3,
        "half-wheel motion should be retained locally until the accumulation threshold is crossed"
    );

    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 60.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "one accumulated wheel notch should request six local lines, capped by the current viewport max offset"
    );
}

#[test]
fn probe_failure_keeps_visible_error_tab_after_projection_timer_ticks() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FailingProbeLauncher {
            message: "missing SSH password secret for `SSH Connection 1`",
        }),
    );

    let ssh_id = create_root_ssh(&app, "SSH Connection 1", "157.254.53.77");
    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "error");
    assert_eq!(
        app.get_workspace_session_error_detail().as_str(),
        "missing SSH password secret for `SSH Connection 1`"
    );

    std::thread::sleep(Duration::from_millis(80));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(100));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "error");
    assert_eq!(
        app.get_workspace_session_error_detail().as_str(),
        "missing SSH password secret for `SSH Connection 1`"
    );
}
