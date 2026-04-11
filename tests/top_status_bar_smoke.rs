//! Smoke coverage for titlebar bindings, theme sync, and auxiliary actions.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    PrivateKeyImporter, VaultProviderFactory, VaultRuntimeOptions,
    bind_top_status_bar_with_injected_services_and_vault_runtime, bind_top_status_bar_with_store,
    bind_top_status_bar_with_store_and_effects,
    bind_top_status_bar_with_store_and_profile_and_effects, runtime_window_title,
};
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::runtime_profile::AppRuntimeProfile;
use mica_term::app::ssh::credentials::{CredentialStore, MemoryCredentialStore};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::ui_preferences::UiPreferencesStore;
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, ProviderAuthKind, ProviderKind,
    RemoteRole,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, VaultProvider};
use mica_term::app::window_effects::{
    BackdropApplyStatus, BackdropPreference, NativeWindowAppearanceRequest,
    NativeWindowCornerPreference, NativeWindowTheme, PlatformWindowEffects,
    WindowAppearanceSyncReport, default_platform_window_effects,
};
use mica_term::shell::metrics::ShellMetrics;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};
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
        "mica-term-titlebar-sync-{label}-{}",
        Uuid::new_v4()
    ))
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

#[derive(Clone)]
struct RecordingWindowEffects {
    requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>,
}

impl RecordingWindowEffects {
    fn new(requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>) -> Self {
        Self { requests }
    }
}

impl PlatformWindowEffects for RecordingWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        self.requests.borrow_mut().push(*request);
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Applied,
            backdrop_error: None,
            redraw_requested: request.request_redraw,
        }
    }
}

#[derive(Clone)]
struct FailingBackdropWindowEffects {
    error_text: &'static str,
}

impl PlatformWindowEffects for FailingBackdropWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Failed,
            backdrop_error: Some(self.error_text.to_string()),
            redraw_requested: request.request_redraw,
        }
    }
}

#[test]
fn app_title_stays_stable_for_mainline_profile() {
    assert_eq!(
        runtime_window_title(AppRuntimeProfile::mainline()),
        "Mica Term"
    );
}

#[test]
fn app_window_title_is_runtime_bound() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(content.contains("in property <string> window-title"));
    assert!(content.contains("title: root.window-title;"));
}

#[test]
fn app_window_source_no_longer_exposes_recovery_mask_contract() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!content.contains("render-revision"));
    assert!(!content.contains("experimental-recovery-mask"));
}

#[test]
fn app_window_source_does_not_expose_flat_window_chrome_binding() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!content.contains("use-flat-window-chrome"));
}

#[test]
fn bootstrap_binds_top_status_bar_callbacks_to_window_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-ui-preferences.json");
    let _ = fs::remove_file(&temp_path);

    app.set_dark_mode(false);
    app.set_show_right_panel(true);
    app.set_show_global_menu(true);
    app.set_is_window_maximized(true);
    app.set_is_window_active(false);
    app.set_is_window_always_on_top(true);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_dark_mode());
    assert!(!app.get_show_right_panel());
    assert!(!app.get_show_global_menu());
    assert!(!app.get_is_window_maximized());
    assert!(app.get_is_window_active());
    assert!(!app.get_is_window_always_on_top());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_right_panel());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_toggle_global_menu_requested();
    assert!(app.get_show_global_menu());

    app.invoke_close_global_menu_requested();
    assert!(!app.get_show_global_menu());

    app.invoke_toggle_theme_mode_requested();
    assert!(!app.get_dark_mode());

    app.invoke_toggle_window_always_on_top_requested();
    assert!(app.get_is_window_always_on_top());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());

    let _ = fs::remove_file(temp_path);
}

#[test]
fn titlebar_exposes_sync_as_a_first_class_action() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());
    app.invoke_sync_now_requested();
    assert!(app.get_sync_modal_open());
}

#[test]
fn restart_then_manual_sync_runs_without_password_prompt() {
    i_slint_backend_testing::init_no_event_loop();

    let temp_root = sample_vault_runtime_root("immediate-action");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));

    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    let restarted_provider_factory = RecordingVaultProviderFactory::default();
    restarted_provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
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

    restarted.invoke_sync_now_requested();

    assert!(
        !restarted.get_sync_modal_open(),
        "configured sync should keep the titlebar action on immediate sync/check semantics"
    );
}

#[test]
fn titlebar_sync_feedback_contract_supports_a_persistent_inflight_state() {
    let titlebar = std::fs::read_to_string("ui/shell/titlebar.slint").unwrap();
    let app_window = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(titlebar.contains("in property <bool> sync-feedback-running: false;"));
    assert!(titlebar.contains("changed sync-feedback-running => {"));
    assert!(titlebar.contains("active: root.sync-feedback-visible || root.sync-feedback-running;"));
    assert!(app_window.contains("in-out property <bool> sync-feedback-running: false;"));
    assert!(app_window.contains("sync-feedback-running: root.sync-feedback-running;"));
}

#[test]
fn titlebar_exposes_transfer_icon_with_queue_badge() {
    let content = std::fs::read_to_string("ui/shell/titlebar.slint").unwrap();

    assert!(
        content.contains("transfer-button"),
        "titlebar should expose a dedicated transfer action button"
    );
    assert!(
        content.contains("transfer-badge"),
        "titlebar should surface a queue badge on the transfer action"
    );
}

#[test]
fn clicking_transfer_icon_opens_transfer_center_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_transfer_center_open());

    app.invoke_open_transfer_center_requested();
    assert!(app.get_transfer_center_open());

    app.invoke_open_transfer_center_requested();
    assert!(!app.get_transfer_center_open());
}

#[test]
fn settings_no_longer_routes_into_vault_flow() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_settings_panel_requested();

    assert_ne!(app.get_right_panel_view().as_str(), "vault");
}

#[test]
fn retained_native_terminal_bind_disables_backdrop_composition() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let requests = Rc::new(RefCell::new(Vec::new()));
    let effects = Rc::new(RecordingWindowEffects::new(Rc::clone(&requests)));

    bind_top_status_bar_with_store_and_profile_and_effects(
        &app,
        None,
        AppRuntimeProfile::mainline(),
        effects,
        None,
    );

    let requests = requests.borrow();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].backdrop, BackdropPreference::None);
}

#[test]
fn bootstrap_syncs_native_window_effects_on_bind_and_theme_toggle() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-window-effects.json");
    let _ = fs::remove_file(&temp_path);

    let requests = Rc::new(RefCell::new(Vec::new()));
    let effects = Rc::new(RecordingWindowEffects::new(Rc::clone(&requests)));

    bind_top_status_bar_with_store_and_effects(
        &app,
        Some(UiPreferencesStore::new(temp_path.clone())),
        effects,
    );

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].theme, NativeWindowTheme::Dark);
        assert_eq!(
            requests[0].corner_preference,
            NativeWindowCornerPreference::DoNotRound
        );
        assert!(requests[0].request_redraw);
    }

    app.invoke_toggle_theme_mode_requested();

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].theme, NativeWindowTheme::Light);
        assert_eq!(
            requests[1].corner_preference,
            NativeWindowCornerPreference::DoNotRound
        );
        assert!(requests[1].request_redraw);
    }

    let _ = fs::remove_file(temp_path);
}

#[test]
fn bootstrap_applies_default_restored_size_before_run() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.window().set_size(PhysicalSize::new(800, 500));

    bind_top_status_bar_with_store(&app, None);

    let size = app.window().size();
    assert_eq!((size.width, size.height), (1440, 900));
}

#[test]
fn maximize_toggle_updates_window_maximized_binding() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_is_window_maximized());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}

#[test]
fn maximize_toggle_keeps_drag_related_window_state_bindings_consistent() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}

#[test]
fn maximize_toggle_does_not_change_shell_geometry_exports_yet() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.invoke_maximize_toggle_requested();
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn bootstrap_logs_backdrop_error_details_when_native_sync_fails() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("theme-sync-backdrop-error-log");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let temp_prefs = temp_root.join("ui-preferences.json");
    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_top_status_bar_with_store_and_effects(
            &app,
            Some(UiPreferencesStore::new(temp_prefs.clone())),
            Rc::new(FailingBackdropWindowEffects {
                error_text: "mock backdrop failure",
            }),
        );
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("backdrop_error=mock backdrop failure"));
    assert!(content.contains("failed to apply native window appearance"));
    assert!(!content.contains("native window appearance sync finished"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn pointer_click_on_panel_toggle_flips_right_panel_request_state_after_sync_button_is_promoted() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    let content_width = app.get_layout_titlebar_content_width();
    let window_controls_width = app.get_layout_titlebar_window_controls_width();
    let utility_zone_x =
        6.0 + content_width - window_controls_width - ShellMetrics::TITLEBAR_UTILITY_WIDTH as f32;
    let position = LogicalPosition::new(
        utility_zone_x + 80.0 + (ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE as f32 / 2.0),
        6.0 + (ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE as f32 / 2.0),
    );

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

    assert!(app.get_show_right_panel());
    assert!(app.get_effective_show_right_panel());
    assert!(app.get_layout_right_panel_width() > 0.0);
}
