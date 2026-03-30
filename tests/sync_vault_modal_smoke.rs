//! Smoke coverage for the dedicated Sync modal contract.

use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;

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
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, ProviderAuthKind,
    ProviderKind, RemoteRole,
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
    std::env::temp_dir().join(format!(
        "mica-term-sync-modal-{label}-{}",
        Uuid::new_v4()
    ))
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
    assert_eq!(
        app.get_sync_modal_primary_action_label().as_str(),
        "Set up sync"
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
            .contains("Configure a Gitee remote first")
    );
    assert!(!temp_root.join("vault-bootstrap-state.json").exists());
}

#[test]
fn sync_modal_submit_lock_unlock_and_close_actions_update_modal_state() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("lock-unlock");
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(MemoryCredentialStore::default()),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert!(temp_root.join("vault-bootstrap-state.json").exists());

    app.invoke_sync_modal_lock_requested();
    assert_eq!(app.get_sync_modal_mode().as_str(), "locked");

    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");

    app.invoke_sync_modal_close_requested();
    assert!(!app.get_sync_modal_open());
}

#[test]
fn sync_modal_primary_and_secondary_actions_route_to_sync_and_lock() {
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

    app.invoke_sync_modal_primary_action_requested();
    assert_eq!(primary.recorded_writes().len(), 1);

    app.invoke_sync_modal_secondary_action_requested();
    assert_eq!(app.get_sync_modal_mode().as_str(), "locked");
}

#[test]
fn sync_modal_does_not_reuse_right_panel_vault_copy() {
    let source = fs::read_to_string("ui/components/sync-vault-modal.slint").unwrap();

    assert!(!source.contains("Primary remote"));
    assert!(!source.contains("Mirror remote"));
    assert!(!source.contains("primary-action := Rectangle"));
}

#[test]
fn sync_modal_window_contract_exposes_task_three_callbacks() {
    let source = fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(source.contains("callback sync-modal-submit-master-password(string);"));
    assert!(source.contains("callback sync-modal-sync-now-requested();"));
    assert!(source.contains("callback sync-modal-lock-requested();"));
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
