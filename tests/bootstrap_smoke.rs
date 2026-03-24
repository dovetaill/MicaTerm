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
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedSshConnectionSpec,
    catalog_to_asset_tree,
};
use mica_term::app::bootstrap::{
    app_title, bind_top_status_bar_with_store_and_effects_and_asset_repo,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store,
    default_window_size,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, StoredSshSecretBundle,
    load_secret_bundle, persist_secret_bundle, ssh_credential_ref,
};
use mica_term::app::ssh::known_hosts::{
    KnownHostCheck, KnownHostsService, default_known_hosts_path,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalSurfaceState, UnknownHostKeyError};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::metrics::ShellMetrics;
use russh::keys::{HashAlg, PublicKey};
use slint::Model;
use tokio::sync::mpsc;

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

struct NoopRuntimeControl;

struct InteractiveProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_input(&self, _bytes: Vec<u8>) -> Result<()> {
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
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
                public_key_openssh: self
                    .host_key
                    .to_openssh()
                    .expect("encode tofu host key"),
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
        Box::pin(async move {
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(25)).await;
                let _ = event_tx.send(SessionRuntimeEvent::Connected);
                let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                    mica_term::app::ssh::runtime::TerminalSurfaceState {
                        session_id,
                        seqno: 1,
                        rows: 24,
                        cols: 80,
                        visible_lines: vec!["welcome to mica-term".into()],
                    },
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(TerminalSurfaceState {
                session_id,
                seqno: 1,
                rows: 24,
                cols: 80,
                visible_lines: vec!["welcome to mica-term".into()],
            }));
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

impl SessionRuntimeLauncher for FailingProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
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

    fn send_input(&self, bytes: Vec<u8>) -> Result<()> {
        let rendered = String::from_utf8(bytes).unwrap_or_default();
        let _ = self
            .event_tx
            .send(SessionRuntimeEvent::SurfaceChanged(TerminalSurfaceState {
                session_id: self.session_id,
                seqno: 2,
                rows: 24,
                cols: 80,
                visible_lines: vec!["welcome to mica-term".into(), format!("$ {}", rendered)],
            }));
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for StoredSecretProbeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let store = Arc::clone(&self.store);
        let message = self.message;
        Box::pin(async move {
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
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>> {
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

fn bind_with_fake_sessions(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
) {
    bind_with_launcher(app, asset_repo, Arc::new(FakeLauncher));
}

fn bind_with_launcher(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
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

fn sample_known_hosts_path(label: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-bootstrap-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti bootstrap-tofu@example.com",
    )
    .expect("parse public key")
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
                        proxy_method: "jump-host".into(),
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
                    proxy_method: String::new(),
                    remark: String::new(),
                    credential_ref: None,
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
    assert_eq!(app.get_asset_ssh_modal_dialog_title().as_str(), "Edit SSH Connection");
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "");

    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
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
    app.invoke_asset_ssh_modal_draft_changed("proxy_method".into(), "jump-host".into());
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

    let launcher_state = launcher_state.lock().expect("lock recording launcher state");
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

    let launcher_state = launcher_state.lock().expect("lock recording launcher state");
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

    let launcher_state = launcher_state.lock().expect("lock recording launcher state");
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
fn asset_activation_emits_layered_ssh_open_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ssh-open-logs-activation");
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
    assert!(log_content.contains("asset activated from explorer"));
    assert!(log_content.contains("activating asset"));
    assert!(log_content.contains("attempting to open ssh session after probe gate"));
    assert!(log_content.contains("session manager registered new session handle"));
    assert!(log_content.contains("synchronized workspace projection from session manager"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn context_menu_open_emits_explicit_ssh_open_logs() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("ssh-open-logs-context-menu");
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
    assert!(log_content.contains("opening ssh asset from context menu"));
    assert!(log_content.contains("session manager registered new session handle"));

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
fn close_connection_context_action_tracks_live_workspace_session_state() {
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
    assert!(!context_menu_item_enabled(&app, "close-connection"));

    app.invoke_close_assets_context_menu_requested();
    app.invoke_asset_activated(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert!(context_menu_item_enabled(&app, "close-connection"));

    app.invoke_assets_context_menu_action_invoked("close-connection".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_workspace_tab_items()
            .row_data(0)
            .expect("disconnected tab")
            .state
            .as_str(),
        "disconnected"
    );
    assert!(app.get_workspace_session_can_reconnect());
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
