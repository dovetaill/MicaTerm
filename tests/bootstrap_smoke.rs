//! Basic bootstrap helper coverage for the binary entrypoint.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mica_term::AppWindow;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetKind, PersistedAssetNode, PersistedAssetPayload, PersistedAssetSocks5ProxySpec,
    PersistedAssetSshProxySpec, PersistedSnippetSpec, PersistedSshConnectionSpec,
    catalog_to_asset_tree,
};
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter, VaultProviderFactory, VaultRuntimeOptions, app_title,
    bind_top_status_bar_with_injected_services_and_vault_runtime, bind_top_status_bar_with_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_terminal_defaults,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_transfer_store,
    build_shared_app_credential_store_for_paths, default_window_size,
    install_url_open_handler_for_test,
};
use mica_term::app::keychain::KeychainCatalog;
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::sftp::{
    BoxedSftpReader, BoxedSftpWriter, RedbTransferStore, SftpBackend, SftpDirectoryEntry,
    SftpDirectoryEntryKind, SftpRemoteMetadata, SftpRuntimeHandle, SftpWriteMode,
    TransferDirection, TransferResumeMode, TransferTask, TransferTaskAction, TransferTaskState,
};
use mica_term::app::ssh::connection_progress::{
    ConnectionProgressEvent, ConnectionStepState, ConnectionStepStateItem,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, SshCredentialKind, StoredSshSecretBundle,
    load_secret_bundle, persist_secret_bundle, ssh_credential_ref,
};
use mica_term::app::ssh::known_hosts::{
    KnownHostCheck, KnownHostsService, default_known_hosts_path,
};
use mica_term::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalKeyKind, TerminalMouseButton,
    TerminalMouseEventKind, TerminalMouseInput, TerminalRuntimeDefaults, TerminalSession,
    TerminalShellIntegrationState, TerminalSurfaceState, UnknownHostKeyError,
};
use mica_term::app::ssh::session_manager::{
    EnhancementPolicy, SessionManager, SessionRuntimeControl, SessionRuntimeLauncher,
};
use mica_term::app::terminal_theme::{preset_for_theme, preset_for_theme_mode};
use mica_term::app::vault::bootstrap::{
    LocalVaultBootstrapState, load_local_vault_bootstrap_state, load_runtime_vault_key,
    save_local_vault_bootstrap_state,
};
use mica_term::app::vault::cache::{load_encrypted_cache, store_encrypted_cache};
use mica_term::app::vault::crypto::{
    decrypt_snapshot, encrypt_snapshot, generate_vault_key, wrap_vault_key,
};
use mica_term::app::vault::device_identity::load_or_create_device_id;
use mica_term::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, CipherKind, CompressionKind,
    KdfConfig, PackLayout, PackRef, ProviderAuthKind, ProviderKind, RemoteRole,
    SnapshotSyncPreferences, VaultAssetPayload, VaultHead, VaultManifest,
};
use mica_term::app::vault::provider::mock::MockVaultProvider;
use mica_term::app::vault::provider::{ProviderCapabilities, ProviderRevision, VaultProvider};
use mica_term::app::vault::recovery::{RecoverySource, load_recovery_snapshots};
use mica_term::app::vault::snapshot::export_vault_snapshot;
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::assets::{
    AssetNodePayload, AssetSshConnectionSpec, AssetSshProxySpec, AssetTree, ConsoleAssetKind,
};
use mica_term::shell::metrics::ShellMetrics;
use mica_term::theme::ThemeMode;
use mica_term::theme::ThemeVariant;
use russh::keys::{HashAlg, PublicKey};
use secrecy::SecretString;
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, Model, ModelRc, SharedString, VecModel};
use tokio::io::{AsyncRead, AsyncSeek, AsyncWrite, ReadBuf};
use tokio::sync::mpsc;
use uuid::Uuid;

static KNOWN_HOSTS_ENV_LOCK: Mutex<()> = Mutex::new(());
static URL_OPEN_HOOK_LOCK: Mutex<()> = Mutex::new(());

fn run_on_large_stack(test_name: &str, test: fn()) {
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        // AppWindow-heavy smoke tests can exceed the default Rust test-thread stack once the
        // generated Slint tree grows; keep them on an explicit larger stack like build.rs does.
        .stack_size(32 * 1024 * 1024)
        .spawn(test)
        .expect("spawn large-stack test thread");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn rgb_tuple_to_hex((red, green, blue): (u8, u8, u8)) -> u32 {
    (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

#[test]
fn bootstrap_sftp_source_routes_browser_loads_through_async_dispatcher_contract() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");

    assert!(
        bootstrap_sftp.contains("SftpOperationKind::LoadDir"),
        "bootstrap SFTP wiring should route directory loads through the async operation dispatcher"
    );
    assert!(
        bootstrap_sftp.contains("dispatch_sftp_load_dir_operation("),
        "bootstrap SFTP wiring should dispatch browser loads through a shared async helper"
    );
    assert!(
        !bootstrap_sftp
            .contains("match manager.sftp_read_dir(request.session_id, request.path.as_str())"),
        "quick-browser directory loads should stop calling the synchronous session-manager wrapper directly"
    );
}

#[test]
fn open_remote_file_queues_background_download_instead_of_modal_editor() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let local_open_source = fs::read_to_string("src/app/sftp/local_open.rs").unwrap_or_default();

    let item_activated_block = bootstrap_sftp
        .split("window.on_sftp_panel_item_activated(move |entry_id, item_kind| {")
        .nth(1)
        .and_then(|rest| {
            rest.split("window.on_sftp_panel_open_queue_requested")
                .next()
        })
        .expect("sftp item activation block should exist");
    let open_action_block = bootstrap_sftp
        .split("PendingSftpContextAction::OpenRemote { entry_id } => {")
        .nth(1)
        .and_then(|rest| {
            rest.split("PendingSftpContextAction::UploadFiles =>")
                .next()
        })
        .expect("sftp open action block should exist");

    assert!(
        local_open_source.contains("DownloadAndOpen"),
        "the SFTP open flow should define a dedicated local-open action instead of reusing the remote modal"
    );
    assert!(
        !item_activated_block.contains("open_sftp_remote_file_editor_for_entry("),
        "activating a file row should stop routing the default Open path through the remote editor modal"
    );
    assert!(
        !open_action_block.contains("open_sftp_remote_file_editor_for_entry("),
        "the default SFTP file context action should stop depending on the remote editor modal"
    );
}

#[test]
fn edit_locally_tracks_working_copy_and_queues_async_upload_on_save() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let context_dispatcher = fs::read_to_string("src/shell/view_model/context_menu_dispatcher.rs")
        .expect("read context menu dispatcher");
    let working_copy_source =
        fs::read_to_string("src/app/sftp/working_copy.rs").unwrap_or_default();
    let local_open_source = fs::read_to_string("src/app/sftp/local_open.rs").unwrap_or_default();

    assert!(
        working_copy_source.contains("pub struct SftpWorkingCopy")
            && working_copy_source.contains("pub upload_on_save: bool"),
        "edit-locally should track a managed working copy that records whether local saves upload back"
    );
    assert!(
        local_open_source.contains("EditLocally"),
        "the local-open helper should distinguish edit-locally from plain download-and-open"
    );
    assert!(
        context_dispatcher.contains("\"edit-locally\""),
        "the context-menu dispatcher should expose a distinct edit-locally action"
    );
    assert!(
        bootstrap_sftp.contains("sftp_upload_file_async("),
        "edit-locally save-back should queue async uploads instead of calling the synchronous UI-thread wrapper"
    );
}

#[test]
fn bootstrap_source_uses_terminal_presenter_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("TerminalPresenter"),
        "bootstrap should depend on a terminal presenter seam instead of calling the atlas renderer directly"
    );
    assert!(
        bootstrap_source.contains("PresentedTerminalFrame"),
        "bootstrap should project presenter output through a PresentedTerminalFrame contract"
    );
    assert!(
        !bootstrap_source.contains("TerminalAtlasRenderer::new()"),
        "bootstrap should stop constructing TerminalAtlasRenderer directly once the presenter boundary exists"
    );
}

#[test]
fn bootstrap_source_threads_native_terminal_surface_contract() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("NativeTerminalSurface"),
        "bootstrap should depend on a native terminal surface hook once native terminal rendering is introduced"
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_render_mode"),
        "bootstrap should publish the active terminal render mode so the software wrapper can select the bitmap fallback"
    );
    assert!(
        bootstrap_source.contains("set_workspace_session_native_frame_token"),
        "bootstrap should publish native frame tokens for the renderer hook path"
    );
}

#[test]
fn bootstrap_source_keeps_workspace_native_terminal_rect_stable_during_context_menus() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !bootstrap_source.contains("|| window.get_workspace_session_context_menu_open()"),
        "workspace context menus should not collapse the native terminal rect now that host-owned overlays layer over a stable terminal body"
    );
    assert!(
        bootstrap_source.contains("workspace_blocks_native_terminal_surface(window)"),
        "bootstrap should still gate native terminal visibility behind explicit blocking-modal checks"
    );
    assert!(
        bootstrap_source.contains("sync_workspace_native_terminal_surface_geometry"),
        "bootstrap should keep synchronizing native terminal geometry when workspace overlay state changes"
    );
}

#[test]
fn theme_toggle_keeps_terminal_refreshes_on_a_surface_local_contract() {
    let shell_chrome_source =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome");

    assert!(
        shell_chrome_source.contains("refresh_active_terminal_surface_only("),
        "theme mode changes should refresh the active terminal through a dedicated surface-local helper so palette swaps stay off the heavier workspace projection rebuild path"
    );
    assert!(
        !shell_chrome_source.contains("sync_workspace_projection_from_manager("),
        "theme mode changes should not force a full workspace projection pass once the new terminal subsystem isolates terminal-local refresh work"
    );
}

#[test]
fn bootstrap_binds_workspace_terminal_hit_normalization_callbacks() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("window.on_workspace_session_normalize_hit_col("),
        "bootstrap should bind a host callback that normalizes wide-char mouse hit columns through the active terminal surface"
    );
    assert!(
        bootstrap_source.contains("window.on_workspace_session_normalize_selection_hit_col("),
        "bootstrap should bind a host callback that normalizes half-cell selection hit columns through the active terminal surface"
    );
    assert!(
        bootstrap_source.contains("workspace_terminal::normalize_active_workspace_hit_col("),
        "bootstrap should delegate wide-char hit normalization to the workspace terminal interaction helpers"
    );
}

#[test]
fn bootstrap_source_uses_windows_native_terminal_presenter_for_native_frames() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("WindowsNativePresenter"),
        "bootstrap should install a Windows native terminal presenter once the native text path exists"
    );
    assert!(
        bootstrap_source.contains("PresentedTerminalFrame::Native(frame)"),
        "bootstrap should consume native terminal frames from the presenter seam"
    );
    assert!(
        bootstrap_source.contains("ensure_workspace_terminal_presenter"),
        "bootstrap should lazily initialize the workspace terminal presenter through an on-demand helper"
    );
    assert!(
        !bootstrap_source.contains("install_workspace_terminal_presenter("),
        "bootstrap should stop eagerly installing the workspace terminal presenter during startup"
    );
    assert!(
        bootstrap_source.contains("PresentedTerminalFrame::Bitmap(frame)"),
        "bootstrap should keep consuming bitmap terminal frames for the software compatibility wrapper"
    );
    assert!(
        !bootstrap_source.contains("frame_token: u64::try_from(surface.seqno)"),
        "bootstrap should stop synthesizing native frame tokens directly from surface seqno once the native renderer owns frame preparation"
    );
}

#[test]
fn runtime_profile_source_defaults_windows_mainline_to_retained_native_terminal_subsystem() {
    let runtime_profile_source =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !runtime_profile_source.contains("pub enum TerminalSubsystemMode"),
        "runtime profile should stop exposing a separate terminal subsystem enum once retained-native is the only live Windows path"
    );
    assert!(
        !runtime_profile_source.contains("MICA_TERM_TERMINAL_SUBSYSTEM"),
        "runtime profile should stop parsing runtime subsystem overrides once the retired Windows path is removed"
    );
    assert!(
        runtime_profile_source.contains("WindowsSoftwareCompat"),
        "runtime profile should keep the software compatibility flavor even after Windows collapses to one terminal path"
    );
    assert!(
        runtime_profile_source.contains("prefers_native_terminal_renderer"),
        "runtime profile should expose a native-renderer preference helper after subsystem switching is removed"
    );
    assert!(
        !bootstrap_source.contains("profile.terminal_subsystem_mode()"),
        "bootstrap should stop threading a separate subsystem selector once retained-native is the only live Windows path"
    );
    assert!(
        bootstrap_source.contains("profile.prefers_native_terminal_renderer()"),
        "bootstrap should choose the Windows presenter path from the native-renderer preference helper"
    );
}

#[test]
fn session_manager_skips_auto_bootstrap_for_cached_fallback_host() {
    let runtime =
        mica_term::app::async_runtime::AppAsyncRuntime::new().expect("create app async runtime");
    let manager = SessionManager::new_with_launcher(runtime.handle(), Arc::new(FakeLauncher));
    let profile = ConnectionProfile {
        asset_id: Some("asset-prod".into()),
        name: "Prod Bastion".into(),
        host: "10.0.0.12".into(),
        user: "ops".into(),
        port: 22,
        auth_method: SshAuthMethod::Password,
        credential_ref: Some("draft://ssh-password/ops@10.0.0.12:22".into()),
        private_key_path: None,
        password: Some("secret".into()),
        private_key_content: None,
        passphrase: None,
        proxy: ConnectionProxyProfile::None,
        resolved_proxy_hops: Vec::new(),
        remark: String::new(),
    };

    manager.remember_enhancement_fallback(&profile, "bash");

    let policy = manager.enhancement_policy_for(&profile);

    assert_eq!(policy, EnhancementPolicy::SkipAutoBootstrap);
}

#[test]
fn path_errors_render_as_lightweight_status_rows_instead_of_full_height_empty_cards() {
    let panel_source = fs::read_to_string("ui/shell/right-panel.slint").expect("read right panel");

    assert!(
        panel_source.contains("status-row"),
        "right panel should expose a lightweight status row for loading, error, and disconnected feedback"
    );
    assert!(
        !panel_source.contains("root.sftp-panel-mode == \"empty\" || root.sftp-panel-mode == \"disconnected\" || root.sftp-panel-mode == \"error\" : empty-state"),
        "error and disconnected states should no longer be rendered through the full-height empty state shell"
    );
    assert!(
        !panel_source.contains("copy-card :="),
        "right panel should remove the legacy full-height copy card for path errors"
    );
}

#[test]
fn transfer_center_projection_contract_includes_completed_file_actions() {
    let shell_chrome_source =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome");

    assert!(
        shell_chrome_source.contains("can_open_file:"),
        "bootstrap transfer projection should publish a completed-row open-file capability"
    );
    assert!(
        shell_chrome_source.contains("can_open_folder:"),
        "bootstrap transfer projection should publish a completed-row open-folder capability"
    );
    assert!(
        shell_chrome_source.contains("can_remove:"),
        "bootstrap transfer projection should publish a row removal capability"
    );
}

#[test]
fn failed_transfer_rows_keep_retry_and_show_error_projection_contract() {
    let shell_chrome_source =
        fs::read_to_string("src/app/bootstrap/shell_chrome.rs").expect("read shell chrome");

    assert!(
        shell_chrome_source.contains("can_retry:"),
        "bootstrap transfer projection should keep retry capability for failed rows"
    );
    assert!(
        shell_chrome_source.contains("can_show_error:"),
        "bootstrap transfer projection should explicitly publish whether failed rows can surface show-error follow-up actions"
    );
}

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
struct DelayedTofuAwareLauncher {
    host_key: PublicKey,
    probe_delay: Duration,
}

#[derive(Clone, Default)]
struct PendingConnectionLauncher;

#[derive(Clone)]
struct AsyncProjectionLauncher;

#[derive(Clone, Default)]
struct InteractiveProjectionLauncher;

#[derive(Clone, Copy)]
struct WideProjectionLauncher {
    cols: u32,
}

#[derive(Clone, Default)]
struct PasteProjectionLauncher;

#[derive(Clone, Copy)]
struct PasteWarningProjectionLauncher {
    bracketed_paste_enabled: bool,
}

#[derive(Clone, Default)]
struct ScrollProjectionLauncher;

#[derive(Clone)]
struct CountingScrollProjectionLauncher {
    state: ScrollProjectionState,
}

#[derive(Clone)]
struct FollowProjectionLauncher {
    state: FollowProjectionState,
}

#[derive(Clone, Default)]
struct SelectionBoundaryState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
}

#[derive(Clone)]
struct SelectionBoundaryLauncher {
    state: SelectionBoundaryState,
}

#[derive(Clone, Default)]
struct ScrollbackCopyLauncher;

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

#[derive(Default)]
struct ObservingScrollbackLauncherState {
    launch_scrollback_lines: Vec<usize>,
}

#[derive(Clone)]
struct ObservingScrollbackLauncher {
    state: Arc<Mutex<ObservingScrollbackLauncherState>>,
    terminal_defaults: TerminalRuntimeDefaults,
}

#[derive(Default)]
struct ObservingViewportLauncherState {
    launch_viewports: Vec<(usize, usize, u32, u32)>,
}

#[derive(Clone)]
struct ObservingViewportLauncher {
    state: Arc<Mutex<ObservingViewportLauncherState>>,
    terminal_defaults: TerminalRuntimeDefaults,
    launch_delay: Duration,
}
#[derive(Clone, Default)]
struct LinkInteractionLauncherState {
    forwarded_mouse_inputs: Arc<Mutex<Vec<TerminalMouseInput>>>,
}

#[derive(Clone)]
struct LinkInteractionLauncher {
    state: LinkInteractionLauncherState,
    line: &'static str,
    alternate_screen_active: bool,
    mouse_grabbed: bool,
}
#[derive(Clone)]
struct SlowOpeningLauncher {
    state: Arc<Mutex<RecordingLauncherState>>,
    probe_delay: Duration,
    launch_delay: Duration,
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

struct LinkInteractionRuntimeControl {
    state: LinkInteractionLauncherState,
}

struct MemoryFileReader {
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFileReader {
    fn new(bytes: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(bytes),
        }
    }
}

impl AsyncRead for MemoryFileReader {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let mut chunk = vec![0; buf.remaining()];
        let read = Read::read(&mut self.cursor, &mut chunk)?;
        buf.put_slice(&chunk[..read]);
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for MemoryFileReader {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

struct MemoryFileWriter {
    path: String,
    files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    cursor: Cursor<Vec<u8>>,
}

impl MemoryFileWriter {
    fn new(path: String, files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>, bytes: Vec<u8>) -> Self {
        Self {
            path,
            files,
            cursor: Cursor::new(bytes),
        }
    }

    fn persist(&self) {
        self.files
            .lock()
            .expect("lock remote files")
            .insert(self.path.clone(), self.cursor.get_ref().clone());
    }
}

impl Drop for MemoryFileWriter {
    fn drop(&mut self) {
        self.persist();
    }
}

impl AsyncWrite for MemoryFileWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let written = Write::write(&mut self.cursor, buf)?;
        self.persist();
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.persist();
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.persist();
        Poll::Ready(Ok(()))
    }
}

impl AsyncSeek for MemoryFileWriter {
    fn start_seek(mut self: Pin<&mut Self>, position: SeekFrom) -> std::io::Result<()> {
        Seek::seek(&mut self.cursor, position)?;
        Ok(())
    }

    fn poll_complete(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<u64>> {
        Poll::Ready(Ok(self.cursor.position()))
    }
}

struct PendingConnectionRuntimeControl {
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct RecordingSftpState {
    read_dir_calls: Arc<Mutex<Vec<String>>>,
    download_file_calls: Arc<Mutex<Vec<String>>>,
    upload_file_calls: Arc<Mutex<Vec<(String, Vec<u8>)>>>,
    mkdir_calls: Arc<Mutex<Vec<String>>>,
    rename_calls: Arc<Mutex<Vec<(String, String)>>>,
    remove_file_calls: Arc<Mutex<Vec<String>>>,
    remove_dir_calls: Arc<Mutex<Vec<String>>>,
    upload_failures_remaining: Arc<Mutex<BTreeMap<String, usize>>>,
    remote_files: Arc<Mutex<BTreeMap<String, Vec<u8>>>>,
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
}

impl RecordingSftpState {
    fn take_read_dir_calls(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .read_dir_calls
                .lock()
                .expect("lock sftp read_dir calls"),
        )
    }

    fn take_download_file_calls(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .download_file_calls
                .lock()
                .expect("lock sftp download file calls"),
        )
    }

    fn take_upload_file_calls(&self) -> Vec<(String, Vec<u8>)> {
        std::mem::take(
            &mut *self
                .upload_file_calls
                .lock()
                .expect("lock sftp upload file calls"),
        )
    }

    fn take_mkdir_calls(&self) -> Vec<String> {
        std::mem::take(&mut *self.mkdir_calls.lock().expect("lock sftp mkdir calls"))
    }

    fn take_rename_calls(&self) -> Vec<(String, String)> {
        std::mem::take(&mut *self.rename_calls.lock().expect("lock sftp rename calls"))
    }

    fn take_remove_file_calls(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .remove_file_calls
                .lock()
                .expect("lock sftp remove-file calls"),
        )
    }

    fn take_remove_dir_calls(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .remove_dir_calls
                .lock()
                .expect("lock sftp remove-dir calls"),
        )
    }

    fn set_remote_file(&self, remote_path: &str, bytes: Vec<u8>) {
        self.remote_files
            .lock()
            .expect("lock remote files")
            .insert(remote_path.into(), bytes);
    }

    fn fail_upload_attempts(&self, remote_path: &str, attempts: usize) {
        self.upload_failures_remaining
            .lock()
            .expect("lock upload failure injection state")
            .insert(remote_path.into(), attempts);
    }

    fn set_event_tx(&self, event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>) {
        *self.event_tx.lock().expect("lock sftp event tx") = Some(event_tx);
    }

    fn emit_cwd(&self, cwd: &str) {
        if let Some(event_tx) = self.event_tx.lock().expect("lock sftp event tx").as_ref() {
            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.into()));
        }
    }
}

#[derive(Clone)]
struct RecordingSftpLauncher {
    state: RecordingSftpState,
}

struct RecordingSftpRuntimeControl {
    runtime: SftpRuntimeHandle,
}

#[derive(Clone)]
struct DelayedCwdRecordingSftpLauncher {
    state: RecordingSftpState,
}

#[derive(Clone)]
struct DelayedReadRecordingSftpLauncher {
    state: RecordingSftpState,
    read_delay_by_path: Arc<BTreeMap<String, Duration>>,
}

struct RecordingSftpBackend {
    responses: BTreeMap<String, Vec<SftpDirectoryEntry>>,
    state: RecordingSftpState,
}

struct DelayedRecordingSftpBackend {
    responses: BTreeMap<String, Vec<SftpDirectoryEntry>>,
    read_delay_by_path: Arc<BTreeMap<String, Duration>>,
    state: RecordingSftpState,
}

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
    write_delay: Duration,
}

impl DelayedVaultProvider {
    fn new(inner: Arc<MockVaultProvider>, read_delay: Duration, write_delay: Duration) -> Self {
        Self {
            inner,
            read_delay,
            write_delay,
        }
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

    fn read_revision(&self, head: &VaultHead) -> Result<ProviderRevision> {
        if !self.read_delay.is_zero() {
            std::thread::sleep(self.read_delay);
        }
        self.inner.read_revision(head)
    }

    fn write_revision(
        &self,
        request: &mica_term::app::vault::provider::ProviderWriteRequest,
    ) -> Result<()> {
        if !self.write_delay.is_zero() {
            std::thread::sleep(self.write_delay);
        }
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
        self.providers
            .lock()
            .expect("lock vault provider factory")
            .get(&remote.remote_id)
            .cloned()
            .ok_or_else(|| anyhow!("missing mock vault provider `{}`", remote.remote_id))
    }
}

struct InteractiveProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct KeyboardMatrixState {
    text_inputs: Arc<Mutex<Vec<String>>>,
    key_inputs: Arc<Mutex<Vec<TerminalKeyEvent>>>,
    paste_inputs: Arc<Mutex<Vec<String>>>,
}

impl KeyboardMatrixState {
    fn key_input_count(&self) -> usize {
        self.key_inputs
            .lock()
            .expect("lock keyboard matrix key inputs")
            .len()
    }

    fn take_text_inputs(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .text_inputs
                .lock()
                .expect("lock keyboard matrix text inputs"),
        )
    }

    fn take_key_inputs(&self) -> Vec<TerminalKeyEvent> {
        std::mem::take(
            &mut *self
                .key_inputs
                .lock()
                .expect("lock keyboard matrix key inputs"),
        )
    }

    fn take_paste_inputs(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .paste_inputs
                .lock()
                .expect("lock keyboard matrix paste inputs"),
        )
    }
}

#[derive(Clone)]
struct KeyboardMatrixLauncher {
    state: KeyboardMatrixState,
    bracketed_paste_enabled: bool,
}

impl KeyboardMatrixLauncher {
    fn new(state: KeyboardMatrixState) -> Self {
        Self {
            state,
            bracketed_paste_enabled: false,
        }
    }

    fn with_bracketed_paste_enabled(mut self, enabled: bool) -> Self {
        self.bracketed_paste_enabled = enabled;
        self
    }
}

struct PasteProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
}

#[derive(Clone, Default)]
struct ScrollProjectionState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
    scroll_call_count: Arc<Mutex<usize>>,
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
}

#[derive(Clone, Default)]
struct FollowProjectionState {
    surface: Arc<Mutex<Option<TerminalSurfaceState>>>,
    event_tx: Arc<Mutex<Option<mpsc::UnboundedSender<SessionRuntimeEvent>>>>,
}

impl FollowProjectionState {
    fn emit_remote_output(&self, appended_lines: u32) {
        let mut surface_guard = self.surface.lock().expect("lock follow projection surface");
        let current = surface_guard
            .clone()
            .expect("current follow projection surface");
        let next_offset = if current.viewport_at_bottom {
            0
        } else {
            current.viewport_offset_lines.saturating_add(appended_lines)
        };
        let next_surface = bootstrap_surface_with_viewport(
            current.session_id,
            current.seqno.saturating_add(1),
            next_offset,
            current
                .viewport_max_offset_lines
                .saturating_add(appended_lines),
        );
        *surface_guard = Some(next_surface.clone());
        drop(surface_guard);

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock follow projection event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(next_surface));
        }
    }

    fn emit_live_surface(&self, label: &str) {
        let mut surface_guard = self.surface.lock().expect("lock follow projection surface");
        let current = surface_guard
            .clone()
            .expect("current follow projection surface");
        let mut next_surface = bootstrap_surface_with_viewport(
            current.session_id,
            current.seqno.saturating_add(1),
            0,
            current.viewport_max_offset_lines,
        );
        next_surface.visible_lines = vec!["live".into(), label.into()];
        *surface_guard = Some(next_surface.clone());
        drop(surface_guard);

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock follow projection event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(next_surface));
        }
    }
}

impl SelectionBoundaryState {
    fn set_event_tx(&self, event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>) {
        *self
            .event_tx
            .lock()
            .expect("lock selection boundary event tx") = Some(event_tx);
    }

    fn set_surface(&self, surface: TerminalSurfaceState) {
        *self
            .surface
            .lock()
            .expect("lock selection boundary surface") = Some(surface.clone());

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock selection boundary event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
        }
    }

    fn emit_alt_screen_surface(&self) {
        let mut next_surface = self
            .surface
            .lock()
            .expect("lock selection boundary surface")
            .clone()
            .expect("current selection boundary surface");
        next_surface.seqno = next_surface.seqno.saturating_add(1);
        next_surface.alternate_screen_active = true;
        next_surface.viewport_offset_lines = 0;
        next_surface.viewport_max_offset_lines = 0;
        next_surface.viewport_at_bottom = true;
        self.set_surface(next_surface);
    }
}

impl ScrollProjectionState {
    fn set_event_tx(&self, event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>) {
        *self
            .event_tx
            .lock()
            .expect("lock scroll projection event tx") = Some(event_tx);
    }

    fn emit_surface_with_viewport(&self, offset: u32, max_offset: u32) {
        let mut surface_guard = self.surface.lock().expect("lock scroll projection surface");
        let current = surface_guard
            .clone()
            .expect("current scroll projection surface");
        let next_surface = bootstrap_surface_with_viewport(
            current.session_id,
            current.seqno.saturating_add(1),
            offset,
            max_offset,
        );
        *surface_guard = Some(next_surface.clone());
        drop(surface_guard);

        if let Some(event_tx) = self
            .event_tx
            .lock()
            .expect("lock scroll projection event tx")
            .as_ref()
        {
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(next_surface));
        }
    }

    fn record_scroll_call(&self) {
        let mut count = self
            .scroll_call_count
            .lock()
            .expect("lock scroll projection call count");
        *count = count.saturating_add(1);
    }

    fn scroll_call_count(&self) -> usize {
        *self
            .scroll_call_count
            .lock()
            .expect("lock scroll projection call count")
    }
}

struct ScrollProjectionRuntimeControl {
    state: ScrollProjectionState,
}

struct FollowProjectionRuntimeControl {
    state: FollowProjectionState,
}

struct SelectionBoundaryRuntimeControl {
    state: SelectionBoundaryState,
}

struct ScrollbackCopyRuntimeControl {
    session_id: uuid::Uuid,
    terminal: Arc<Mutex<TerminalSession>>,
}

struct KeyboardMatrixRuntimeControl {
    state: KeyboardMatrixState,
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

impl SftpBackend for RecordingSftpBackend {
    fn read_dir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        let state = self.state.clone();
        let response = self.responses.get(path).cloned().unwrap_or_default();
        let path = path.to_string();
        Box::pin(async move {
            state
                .read_dir_calls
                .lock()
                .expect("lock sftp read_dir calls")
                .push(path);
            Ok(response)
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            state
                .mkdir_calls
                .lock()
                .expect("lock sftp mkdir calls")
                .push(path);
            Ok(())
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a str,
        to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let from = from.to_string();
        let to = to.to_string();
        Box::pin(async move {
            let staged_bytes = state
                .remote_files
                .lock()
                .expect("lock remote files")
                .get(&from)
                .cloned();
            if from.ends_with(".part")
                && let Some(bytes) = staged_bytes.clone()
            {
                state
                    .upload_file_calls
                    .lock()
                    .expect("lock sftp upload file calls")
                    .push((to.clone(), bytes));
                let mut failures = state
                    .upload_failures_remaining
                    .lock()
                    .expect("lock upload failure injection state");
                if let Some(remaining) = failures.get_mut(&to)
                    && *remaining > 0
                {
                    *remaining -= 1;
                    return Err(anyhow!("simulated upload failure"));
                }
            }
            state
                .rename_calls
                .lock()
                .expect("lock sftp rename calls")
                .push((from, to));
            Ok(())
        })
    }

    fn path_exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        let state = self.state.clone();
        let responses = self.responses.clone();
        let path = path.to_string();
        Box::pin(async move {
            let file_exists = state
                .remote_files
                .lock()
                .expect("lock remote files")
                .contains_key(&path);
            let directory_exists = responses.contains_key(&path);
            let listed_entry_exists = responses.values().flatten().any(|entry| entry.path == path);
            Ok(file_exists || directory_exists || listed_entry_exists)
        })
    }

    fn stat<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let size_bytes = state
                .remote_files
                .lock()
                .expect("lock remote files")
                .get(&path)
                .map(|bytes| bytes.len() as u64);
            if size_bytes.is_none() {
                return Err(anyhow!("missing remote file: {path}"));
            }

            Ok(SftpRemoteMetadata {
                size_bytes,
                modified_unix_seconds: Some(1_710_000_000),
            })
        })
    }

    fn open_file_reader<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let bytes = state
                .remote_files
                .lock()
                .expect("lock remote files")
                .get(&path)
                .cloned()
                .ok_or_else(|| anyhow!("missing remote file: {path}"))?;
            Ok(Box::pin(MemoryFileReader::new(bytes)) as BoxedSftpReader)
        })
    }

    fn open_file_writer<'a>(
        &'a self,
        path: &'a str,
        mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let initial = match mode {
                SftpWriteMode::CreateOrTruncate => Vec::new(),
                SftpWriteMode::CreateOrAppend => state
                    .remote_files
                    .lock()
                    .expect("lock remote files")
                    .get(&path)
                    .cloned()
                    .unwrap_or_default(),
            };
            Ok(Box::pin(MemoryFileWriter::new(
                path,
                Arc::clone(&state.remote_files),
                initial,
            )) as BoxedSftpWriter)
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            let mut failures = state
                .upload_failures_remaining
                .lock()
                .expect("lock upload failure injection state");
            if let Some(remaining) = failures.get_mut(&remote_path)
                && *remaining > 0
            {
                *remaining -= 1;
                drop(failures);
                state
                    .upload_file_calls
                    .lock()
                    .expect("lock sftp upload file calls")
                    .push((remote_path.clone(), data));
                return Err(anyhow!("simulated upload failure"));
            }
            drop(failures);
            state
                .remote_files
                .lock()
                .expect("lock remote files")
                .insert(remote_path.clone(), data.clone());
            state
                .upload_file_calls
                .lock()
                .expect("lock sftp upload file calls")
                .push((remote_path, data.clone()));
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .download_file_calls
                .lock()
                .expect("lock sftp download file calls")
                .push(remote_path.clone());
            Ok(state
                .remote_files
                .lock()
                .expect("lock remote files")
                .get(&remote_path)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn remove_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .remove_file_calls
                .lock()
                .expect("lock sftp remove-file calls")
                .push(remote_path);
            Ok(())
        })
    }

    fn remove_dir<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .remove_dir_calls
                .lock()
                .expect("lock sftp remove-dir calls")
                .push(remote_path);
            Ok(())
        })
    }
}

impl SftpBackend for DelayedRecordingSftpBackend {
    fn read_dir<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<SftpDirectoryEntry>>> + Send + 'a>> {
        let state = self.state.clone();
        let response = self.responses.get(path).cloned().unwrap_or_default();
        let delay = self
            .read_delay_by_path
            .get(path)
            .copied()
            .unwrap_or_default();
        let path = path.to_string();
        Box::pin(async move {
            if !delay.is_zero() {
                tokio::time::sleep(delay).await;
            }
            state
                .read_dir_calls
                .lock()
                .expect("lock delayed sftp read_dir calls")
                .push(path);
            Ok(response)
        })
    }

    fn mkdir<'a>(&'a self, path: &'a str) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            state
                .mkdir_calls
                .lock()
                .expect("lock sftp mkdir calls")
                .push(path);
            Ok(())
        })
    }

    fn rename<'a>(
        &'a self,
        from: &'a str,
        to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let from = from.to_string();
        let to = to.to_string();
        Box::pin(async move {
            let staged_bytes = state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .get(&from)
                .cloned();
            if from.ends_with(".part")
                && let Some(bytes) = staged_bytes.clone()
            {
                state
                    .upload_file_calls
                    .lock()
                    .expect("lock delayed sftp upload file calls")
                    .push((to.clone(), bytes));
                let mut failures = state
                    .upload_failures_remaining
                    .lock()
                    .expect("lock delayed upload failure injection state");
                if let Some(remaining) = failures.get_mut(&to)
                    && *remaining > 0
                {
                    *remaining -= 1;
                    return Err(anyhow!("simulated upload failure"));
                }
            }
            state
                .rename_calls
                .lock()
                .expect("lock sftp rename calls")
                .push((from, to));
            Ok(())
        })
    }

    fn path_exists<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        let state = self.state.clone();
        let responses = self.responses.clone();
        let path = path.to_string();
        Box::pin(async move {
            let file_exists = state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .contains_key(&path);
            let directory_exists = responses.contains_key(&path);
            let listed_entry_exists = responses.values().flatten().any(|entry| entry.path == path);
            Ok(file_exists || directory_exists || listed_entry_exists)
        })
    }

    fn stat<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<SftpRemoteMetadata>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let size_bytes = state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .get(&path)
                .map(|bytes| bytes.len() as u64);
            if size_bytes.is_none() {
                return Err(anyhow!("missing remote file: {path}"));
            }

            Ok(SftpRemoteMetadata {
                size_bytes,
                modified_unix_seconds: Some(1_710_000_000),
            })
        })
    }

    fn open_file_reader<'a>(
        &'a self,
        path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpReader>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let bytes = state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .get(&path)
                .cloned()
                .ok_or_else(|| anyhow!("missing remote file: {path}"))?;
            Ok(Box::pin(MemoryFileReader::new(bytes)) as BoxedSftpReader)
        })
    }

    fn open_file_writer<'a>(
        &'a self,
        path: &'a str,
        mode: SftpWriteMode,
    ) -> Pin<Box<dyn Future<Output = Result<BoxedSftpWriter>> + Send + 'a>> {
        let state = self.state.clone();
        let path = path.to_string();
        Box::pin(async move {
            let initial = match mode {
                SftpWriteMode::CreateOrTruncate => Vec::new(),
                SftpWriteMode::CreateOrAppend => state
                    .remote_files
                    .lock()
                    .expect("lock delayed remote files")
                    .get(&path)
                    .cloned()
                    .unwrap_or_default(),
            };
            Ok(Box::pin(MemoryFileWriter::new(
                path,
                Arc::clone(&state.remote_files),
                initial,
            )) as BoxedSftpWriter)
        })
    }

    fn upload_file<'a>(
        &'a self,
        remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            let mut failures = state
                .upload_failures_remaining
                .lock()
                .expect("lock delayed upload failure injection state");
            if let Some(remaining) = failures.get_mut(&remote_path)
                && *remaining > 0
            {
                *remaining -= 1;
                drop(failures);
                state
                    .upload_file_calls
                    .lock()
                    .expect("lock delayed sftp upload file calls")
                    .push((remote_path.clone(), data));
                return Err(anyhow!("simulated upload failure"));
            }
            drop(failures);
            state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .insert(remote_path.clone(), data.clone());
            state
                .upload_file_calls
                .lock()
                .expect("lock delayed sftp upload file calls")
                .push((remote_path, data.clone()));
            Ok(data.len() as u64)
        })
    }

    fn download_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .download_file_calls
                .lock()
                .expect("lock delayed sftp download file calls")
                .push(remote_path.clone());
            Ok(state
                .remote_files
                .lock()
                .expect("lock delayed remote files")
                .get(&remote_path)
                .cloned()
                .unwrap_or_default())
        })
    }

    fn remove_file<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .remove_file_calls
                .lock()
                .expect("lock sftp remove-file calls")
                .push(remote_path);
            Ok(())
        })
    }

    fn remove_dir<'a>(
        &'a self,
        remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        let state = self.state.clone();
        let remote_path = remote_path.to_string();
        Box::pin(async move {
            state
                .remove_dir_calls
                .lock()
                .expect("lock sftp remove-dir calls")
                .push(remote_path);
            Ok(())
        })
    }
}

impl SessionRuntimeControl for RecordingSftpRuntimeControl {
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

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.runtime.clone())
    }
}

impl SessionRuntimeLauncher for RecordingSftpLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let responses = match profile.host.as_str() {
                "10.0.0.12" => BTreeMap::from([
                    (
                        "/srv/app".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-logs".into(),
                            name: "logs".into(),
                            path: "/srv/app/logs".into(),
                            kind: SftpDirectoryEntryKind::Directory,
                            modified_unix_seconds: Some(1_775_012_700),
                            size_bytes: None,
                        }],
                    ),
                    (
                        "/srv/app/releases".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-release".into(),
                            name: "release.tar.gz".into(),
                            path: "/srv/app/releases/release.tar.gz".into(),
                            kind: SftpDirectoryEntryKind::File,
                            modified_unix_seconds: Some(1_775_013_060),
                            size_bytes: Some(14 * 1024),
                        }],
                    ),
                ]),
                _ => BTreeMap::from([(
                    "/srv/db".to_string(),
                    vec![SftpDirectoryEntry {
                        id: "entry-backup".into(),
                        name: "backup.sql".into(),
                        path: "/srv/db/backup.sql".into(),
                        kind: SftpDirectoryEntryKind::File,
                        modified_unix_seconds: Some(1_775_013_420),
                        size_bytes: Some(7 * 1024),
                    }],
                )]),
            };

            let cwd = if profile.host == "10.0.0.24" {
                "/srv/db"
            } else {
                "/srv/app"
            };
            state.set_event_tx(event_tx.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.into()));
            Ok(Box::new(RecordingSftpRuntimeControl {
                runtime: SftpRuntimeHandle::new(Arc::new(RecordingSftpBackend {
                    responses,
                    state,
                })),
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

impl SessionRuntimeLauncher for DelayedCwdRecordingSftpLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let responses = match profile.host.as_str() {
                "10.0.0.12" => BTreeMap::from([
                    (
                        "/".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-srv".into(),
                            name: "srv".into(),
                            path: "/srv".into(),
                            kind: SftpDirectoryEntryKind::Directory,
                            modified_unix_seconds: Some(1_775_012_340),
                            size_bytes: None,
                        }],
                    ),
                    (
                        "/srv/app".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-logs".into(),
                            name: "logs".into(),
                            path: "/srv/app/logs".into(),
                            kind: SftpDirectoryEntryKind::Directory,
                            modified_unix_seconds: Some(1_775_012_700),
                            size_bytes: None,
                        }],
                    ),
                ]),
                _ => BTreeMap::from([(
                    "/".to_string(),
                    vec![SftpDirectoryEntry {
                        id: "entry-home".into(),
                        name: "home".into(),
                        path: "/home".into(),
                        kind: SftpDirectoryEntryKind::Directory,
                        modified_unix_seconds: Some(1_775_011_980),
                        size_bytes: None,
                    }],
                )]),
            };

            state.set_event_tx(event_tx.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(RecordingSftpRuntimeControl {
                runtime: SftpRuntimeHandle::new(Arc::new(RecordingSftpBackend {
                    responses,
                    state,
                })),
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

impl SessionRuntimeLauncher for DelayedReadRecordingSftpLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        let read_delay_by_path = Arc::clone(&self.read_delay_by_path);
        Box::pin(async move {
            let responses = match profile.host.as_str() {
                "10.0.0.12" => BTreeMap::from([
                    (
                        "/srv/app".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-logs".into(),
                            name: "logs".into(),
                            path: "/srv/app/logs".into(),
                            kind: SftpDirectoryEntryKind::Directory,
                            modified_unix_seconds: Some(1_775_012_700),
                            size_bytes: None,
                        }],
                    ),
                    (
                        "/srv/app/logs".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-log-a".into(),
                            name: "app.log".into(),
                            path: "/srv/app/logs/app.log".into(),
                            kind: SftpDirectoryEntryKind::File,
                            modified_unix_seconds: Some(1_775_012_780),
                            size_bytes: Some(2048),
                        }],
                    ),
                    (
                        "/srv/app/releases".to_string(),
                        vec![SftpDirectoryEntry {
                            id: "entry-release".into(),
                            name: "release.tar.gz".into(),
                            path: "/srv/app/releases/release.tar.gz".into(),
                            kind: SftpDirectoryEntryKind::File,
                            modified_unix_seconds: Some(1_775_013_060),
                            size_bytes: Some(14 * 1024),
                        }],
                    ),
                ]),
                _ => BTreeMap::from([(
                    "/srv/db".to_string(),
                    vec![SftpDirectoryEntry {
                        id: "entry-backup".into(),
                        name: "backup.sql".into(),
                        path: "/srv/db/backup.sql".into(),
                        kind: SftpDirectoryEntryKind::File,
                        modified_unix_seconds: Some(1_775_013_420),
                        size_bytes: Some(7 * 1024),
                    }],
                )]),
            };

            let cwd = if profile.host == "10.0.0.24" {
                "/srv/db"
            } else {
                "/srv/app"
            };
            state.set_event_tx(event_tx.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd.into()));
            Ok(Box::new(RecordingSftpRuntimeControl {
                runtime: SftpRuntimeHandle::new(Arc::new(DelayedRecordingSftpBackend {
                    responses,
                    read_delay_by_path,
                    state,
                })),
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

impl SessionRuntimeControl for PendingConnectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::Disconnected);
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

impl DelayedTofuAwareLauncher {
    fn new(host_key: PublicKey, probe_delay: Duration) -> Self {
        Self {
            host_key,
            probe_delay,
        }
    }

    fn ensure_trusted(&self, profile: &ConnectionProfile) -> Result<()> {
        TofuAwareLauncher {
            host_key: self.host_key.clone(),
        }
        .ensure_trusted(profile)
    }
}

impl SessionRuntimeLauncher for TofuAwareLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
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

impl SessionRuntimeLauncher for DelayedTofuAwareLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
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
        Box::pin(async move {
            launcher.ensure_trusted(&profile)?;
            tokio::time::sleep(launcher.probe_delay).await;
            Ok(())
        })
    }
}

impl SessionRuntimeLauncher for PendingConnectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::ConnectionProgress(
                ConnectionProgressEvent::StepUpdated {
                    attempt_id,
                    step: ConnectionStepStateItem {
                        step_id: "00-connect-target".into(),
                        step_kind: "connect-target".into(),
                        title: "Connect Target".into(),
                        detail: "Opening SSH transport to 10.0.0.12".into(),
                        hop_label: "Target".into(),
                        state: ConnectionStepState::Running,
                    },
                },
            ));
            Ok(Box::new(PendingConnectionRuntimeControl { event_tx })
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

impl SessionRuntimeLauncher for AsyncProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
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
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::ShellIntegrationChanged(
                TerminalShellIntegrationState {
                    has_markers: true,
                    input_active: true,
                    command_running: false,
                    last_command_exit_code: Some(0),
                },
            ));
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

impl SessionRuntimeLauncher for WideProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let cols = self.cols;
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                terminal_surface_with_cells(
                    session_id,
                    1,
                    24,
                    cols,
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

impl SessionRuntimeLauncher for KeyboardMatrixLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        let bracketed_paste_enabled = self.bracketed_paste_enabled;
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let mut surface = terminal_surface_with_cells(
                session_id,
                1,
                24,
                80,
                vec!["welcome to mica-term".into()],
            );
            surface.bracketed_paste_enabled = bracketed_paste_enabled;
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(KeyboardMatrixRuntimeControl { state }) as Box<dyn SessionRuntimeControl>)
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
        _attempt_id: uuid::Uuid,
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

impl SessionRuntimeLauncher for PasteWarningProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let bracketed_paste_enabled = self.bracketed_paste_enabled;
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let mut surface = TerminalSurfaceState::from_visible_lines(
                session_id,
                1,
                24,
                80,
                vec!["welcome to mica-term".into()],
            );
            surface.bracketed_paste_enabled = bracketed_paste_enabled;
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
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
        _attempt_id: uuid::Uuid,
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
            state.set_event_tx(event_tx.clone());
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

impl CountingScrollProjectionLauncher {
    fn new(state: ScrollProjectionState) -> Self {
        Self { state }
    }
}

impl SessionRuntimeLauncher for CountingScrollProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let surface = bootstrap_surface_with_viewport(session_id, 1, 3, 8);
            *state
                .surface
                .lock()
                .expect("lock scroll projection surface") = Some(surface.clone());
            state.set_event_tx(event_tx.clone());
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

impl SessionRuntimeLauncher for FollowProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            let surface = bootstrap_surface_with_viewport(session_id, 1, 0, 8);
            *state
                .surface
                .lock()
                .expect("lock follow projection surface") = Some(surface.clone());
            *state
                .event_tx
                .lock()
                .expect("lock follow projection event tx") = Some(event_tx.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(FollowProjectionRuntimeControl { state })
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

impl SessionRuntimeLauncher for SelectionBoundaryLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        Box::pin(async move {
            state.set_event_tx(event_tx.clone());
            let surface = terminal_surface_with_cells(
                session_id,
                1,
                24,
                80,
                vec!["welcome to mica-term".into()],
            );
            *state
                .surface
                .lock()
                .expect("lock selection boundary surface") = Some(surface.clone());
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(SelectionBoundaryRuntimeControl { state })
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

impl SessionRuntimeLauncher for ScrollbackCopyLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let mut session = TerminalSession::new(4, 20);
            session.apply_remote_bytes(b"zero\r\none\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
            session.scroll_viewport_lines(2);
            let surface = session.surface_state(session_id);
            let terminal = Arc::new(Mutex::new(session));
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(ScrollbackCopyRuntimeControl {
                session_id,
                terminal,
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
        _attempt_id: uuid::Uuid,
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
        let _ = self
            .event_tx
            .send(SessionRuntimeEvent::ShellIntegrationChanged(
                TerminalShellIntegrationState {
                    has_markers: true,
                    input_active: true,
                    command_running: false,
                    last_command_exit_code: Some(0),
                },
            ));
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
        let _ = self
            .event_tx
            .send(SessionRuntimeEvent::ShellIntegrationChanged(
                TerminalShellIntegrationState {
                    has_markers: true,
                    input_active: true,
                    command_running: false,
                    last_command_exit_code: Some(0),
                },
            ));
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

impl SessionRuntimeControl for LinkInteractionRuntimeControl {
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

    fn send_mouse_input(&self, event: TerminalMouseInput) -> Result<()> {
        self.state
            .forwarded_mouse_inputs
            .lock()
            .expect("lock forwarded mouse inputs")
            .push(event);
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeControl for KeyboardMatrixRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        self.state
            .text_inputs
            .lock()
            .expect("lock keyboard matrix text inputs")
            .push(text);
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        self.state
            .key_inputs
            .lock()
            .expect("lock keyboard matrix key inputs")
            .push(event);
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.state
            .paste_inputs
            .lock()
            .expect("lock keyboard matrix paste inputs")
            .push(text);
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
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
        self.state.record_scroll_call();
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

impl SessionRuntimeControl for FollowProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        self.state.emit_live_surface(&format!("text {text}"));
        Ok(())
    }

    fn send_key_input(&self, event: TerminalKeyEvent) -> Result<()> {
        let rendered = match event.key {
            TerminalKeyKind::Named(name) => name.to_string(),
            TerminalKeyKind::Function(number) => format!("f{number}"),
            TerminalKeyKind::Char(ch) => ch.to_string(),
        };
        self.state.emit_live_surface(&format!("key {rendered}"));
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        self.state.emit_live_surface(&format!("paste {text}"));
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        self.state.emit_live_surface("mouse");
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
            .expect("lock follow projection surface")
            .clone()
            .expect("current follow projection surface");
        let next_offset = (surface.viewport_offset_lines as i32 + delta)
            .clamp(0, surface.viewport_max_offset_lines as i32) as u32;
        surface = bootstrap_surface_with_viewport(
            surface.session_id,
            surface.seqno,
            next_offset,
            surface.viewport_max_offset_lines,
        );
        *self
            .state
            .surface
            .lock()
            .expect("lock follow projection surface") = Some(surface.clone());
        Ok(surface)
    }
}

impl SessionRuntimeControl for SelectionBoundaryRuntimeControl {
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

    fn resize(&self, rows: u32, cols: u32) -> Result<()> {
        let mut next_surface = self
            .state
            .surface
            .lock()
            .expect("lock selection boundary surface")
            .clone()
            .expect("current selection boundary surface");
        next_surface.seqno = next_surface.seqno.saturating_add(1);
        next_surface.rows = rows.max(1);
        next_surface.cols = cols.max(1);
        self.state.set_surface(next_surface);
        Ok(())
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        self.state
            .surface
            .lock()
            .expect("lock selection boundary surface")
            .clone()
            .ok_or_else(|| anyhow!("selection boundary surface is unavailable"))
    }
}

impl SessionRuntimeControl for ScrollbackCopyRuntimeControl {
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

    fn selection_text_from_buffer_rows(
        &self,
        start_row: u32,
        start_col: u32,
        end_row: u32,
        end_col: u32,
    ) -> Result<Option<String>> {
        let terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("lock scrollback copy terminal"))?;
        Ok(Some(terminal.selection_text_from_buffer_rows(
            start_row, start_col, end_row, end_col,
        )))
    }

    fn terminal_surface(&self) -> Result<TerminalSurfaceState> {
        let terminal = self
            .terminal
            .lock()
            .map_err(|_| anyhow!("lock scrollback copy terminal"))?;
        Ok(terminal.surface_state(self.session_id))
    }
}

impl SessionRuntimeLauncher for StoredSecretProbeLauncher {
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
        _attempt_id: uuid::Uuid,
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

impl SessionRuntimeLauncher for ObservingScrollbackLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        let terminal_defaults = self.terminal_defaults.clone();
        Box::pin(async move {
            state
                .lock()
                .expect("lock observing scrollback launcher state")
                .launch_scrollback_lines
                .push(terminal_defaults.scrollback_lines());
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

impl SessionRuntimeLauncher for ObservingViewportLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        let terminal_defaults = self.terminal_defaults.clone();
        let launch_delay = self.launch_delay;
        Box::pin(async move {
            tokio::time::sleep(launch_delay).await;
            let viewport = (
                terminal_defaults.viewport_rows(),
                terminal_defaults.viewport_cols(),
                terminal_defaults.viewport_pixel_width(),
                terminal_defaults.viewport_pixel_height(),
            );
            state
                .lock()
                .expect("lock observing viewport launcher state")
                .launch_viewports
                .push(viewport);
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                TerminalSurfaceState::from_visible_lines(
                    session_id,
                    1,
                    viewport.0 as u32,
                    viewport.1 as u32,
                    vec!["viewport defaults captured".into()],
                ),
            ));
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
impl SessionRuntimeLauncher for LinkInteractionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = self.state.clone();
        let line = self.line;
        let alternate_screen_active = self.alternate_screen_active;
        let mouse_grabbed = self.mouse_grabbed;
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let mut surface = terminal_surface_with_cells(session_id, 1, 24, 80, vec![line.into()]);
            surface.alternate_screen_active = alternate_screen_active;
            surface.mouse_grabbed = mouse_grabbed;
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(surface));
            Ok(Box::new(LinkInteractionRuntimeControl { state }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}
impl SessionRuntimeLauncher for SlowOpeningLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let state = Arc::clone(&self.state);
        let launch_delay = self.launch_delay;
        Box::pin(async move {
            state
                .lock()
                .expect("lock slow opening launcher state")
                .launch_profiles
                .push(profile);
            tokio::time::sleep(launch_delay).await;
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let state = Arc::clone(&self.state);
        let probe_delay = self.probe_delay;
        Box::pin(async move {
            state
                .lock()
                .expect("lock slow opening launcher state")
                .probe_profiles
                .push(profile);
            tokio::time::sleep(probe_delay).await;
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

#[test]
fn snippet_create_modal_projects_runtime_rows_through_window_callbacks() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");

    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Deploy prod".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/api".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_snippet_asset_items();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().kind.as_str(), "snippet");
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Deploy prod");
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn snippet_edit_and_delete_actions_route_through_bootstrap_callbacks() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    let snippet_id = create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_context_menu_requested(
        snippet_id.clone().into(),
        "snippet".into(),
        96.0,
        160.0,
    );
    app.invoke_assets_context_menu_action_invoked("edit-snippet".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-snippet");
    assert_eq!(app.get_asset_snippet_modal_name().as_str(), "Deploy prod");
    assert_eq!(
        app.get_asset_snippet_modal_script().as_str(),
        "kubectl rollout restart deploy/api"
    );

    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Restart api".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/web".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_snippet_asset_items();
    assert_eq!(rows.row_count(), 1);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Restart api");

    app.invoke_asset_context_menu_requested(snippet_id.into(), "snippet".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-snippet".into());

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(
        app.get_asset_delete_confirm_target_label().as_str(),
        "Restart api"
    );

    app.invoke_confirm_delete_asset_requested();
    assert_eq!(app.get_snippet_asset_items().row_count(), 0);
}

#[test]
fn snippet_double_click_activation_defaults_to_paste() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PasteProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    let snippet_id = create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_selected(snippet_id.clone().into());
    app.invoke_asset_selected(snippet_id.into());
    flush_runtime_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "paste kubectl rollout restart deploy/api"
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

fn sample_remote_revision_for_tree(
    password: &SecretString,
    asset_tree: &AssetTree,
    credential_store: &dyn CredentialStore,
    revision: &str,
) -> ProviderRevision {
    let snapshot = export_vault_snapshot(
        asset_tree,
        &KeychainCatalog::default(),
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
        committed_at: "2026-03-31T10:00:00Z".into(),
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

fn sample_remote_revision_for_existing_vault_key(
    asset_tree: &AssetTree,
    credential_store: &dyn CredentialStore,
    revision: &str,
    vault_key: &[u8; 32],
    wrapped_vault_key: &str,
    kdf: &KdfConfig,
) -> ProviderRevision {
    let snapshot = export_vault_snapshot(
        asset_tree,
        &KeychainCatalog::default(),
        credential_store,
        &sample_known_hosts_path("remote-revision"),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .expect("export vault snapshot");
    let encrypted_snapshot = encrypt_snapshot(&snapshot, vault_key).expect("encrypt snapshot");
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
        committed_at: "2026-03-31T10:00:00Z".into(),
        committed_by_device: "device-remote".into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: wrapped_vault_key.into(),
        kdf: kdf.clone(),
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

fn sample_remote_revision_for_snapshot(
    snapshot: &mica_term::app::vault::model::VaultSnapshot,
    revision: &str,
    vault_key: &[u8; 32],
    wrapped_vault_key: &str,
    kdf: &KdfConfig,
) -> ProviderRevision {
    let encrypted_snapshot = encrypt_snapshot(snapshot, vault_key).expect("encrypt snapshot");
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
        committed_at: "2026-03-31T10:00:00Z".into(),
        committed_by_device: "device-remote".into(),
        payload_hash: format!("sha256:{}", encrypted_snapshot.payload_sha256),
        manifest_ref: format!("bundle/{revision}/manifest.bin"),
        wrapped_vault_key: wrapped_vault_key.into(),
        kdf: kdf.clone(),
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
    surface.cursor.fg_rgba = 0xfff7_f9fc;
    surface.cursor.bg_rgba = 0xff4b_5058;
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

fn settle_sync_scheduler(delay: Duration) {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(delay);
    slint::platform::update_timers_and_animations();
}

const VAULT_SYNC_WAIT_TIMEOUT: Duration = Duration::from_secs(20);

fn init_bootstrap_smoke_test() -> std::sync::MutexGuard<'static, ()> {
    static BOOTSTRAP_SMOKE_TEST_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();

    let lock = BOOTSTRAP_SMOKE_TEST_MUTEX
        .get_or_init(|| Mutex::new(()))
        .lock();
    let guard = match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    i_slint_backend_testing::init_no_event_loop();
    guard
}

fn wait_for_condition(timeout: Duration, mut predicate: impl FnMut() -> bool) {
    // Full `cargo test` runs execute multiple binaries in parallel, so real
    // background-thread work can need extra scheduler slack beyond the nominal
    // smoke-test timeout even when the underlying behavior is correct.
    let scheduling_grace = if timeout >= Duration::from_secs(2) {
        Duration::from_secs(6)
    } else {
        Duration::ZERO
    };
    let deadline = Instant::now() + timeout + scheduling_grace;
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

fn run_with_large_test_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .expect("spawn large-stack test thread")
        .join()
        .expect("join large-stack test thread");
}

fn terminal_interaction_position(app: &AppWindow) -> LogicalPosition {
    LogicalPosition::new(
        app.get_layout_main_workspace_x() + 96.0,
        app.get_layout_titlebar_height() + 96.0,
    )
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

fn dispatch_shared_key_chord(
    app: &AppWindow,
    key_text: slint::SharedString,
    ctrl: bool,
    shift: bool,
    alt: bool,
) {
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
        text: key_text.clone(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: key_text });

    if alt {
        dispatch_modifier_released(app, Key::Alt);
    }
    if ctrl {
        dispatch_modifier_released(app, Key::Control);
    }
    if shift {
        dispatch_modifier_released(app, Key::Shift);
    }
}

fn dispatch_text_key_chord(app: &AppWindow, key_text: &str, ctrl: bool, shift: bool, alt: bool) {
    dispatch_shared_key_chord(app, key_text.into(), ctrl, shift, alt);
}

fn dispatch_named_key_chord(app: &AppWindow, key_name: &str, ctrl: bool, shift: bool, alt: bool) {
    let key_text = match key_name {
        "left" => Key::LeftArrow.into(),
        "right" => Key::RightArrow.into(),
        "up" => Key::UpArrow.into(),
        "down" => Key::DownArrow.into(),
        "home" => Key::Home.into(),
        "end" => Key::End.into(),
        "insert" => Key::Insert.into(),
        "page-up" => Key::PageUp.into(),
        "page-down" => Key::PageDown.into(),
        other => panic!("unsupported named key `{other}`"),
    };
    dispatch_shared_key_chord(app, key_text, ctrl, shift, alt);
}

fn dispatch_function_key(app: &AppWindow, number: u8) {
    let key_text = match number {
        1 => Key::F1.into(),
        2 => Key::F2.into(),
        3 => Key::F3.into(),
        4 => Key::F4.into(),
        5 => Key::F5.into(),
        6 => Key::F6.into(),
        7 => Key::F7.into(),
        8 => Key::F8.into(),
        9 => Key::F9.into(),
        10 => Key::F10.into(),
        11 => Key::F11.into(),
        12 => Key::F12.into(),
        13 => Key::F13.into(),
        14 => Key::F14.into(),
        15 => Key::F15.into(),
        16 => Key::F16.into(),
        17 => Key::F17.into(),
        18 => Key::F18.into(),
        19 => Key::F19.into(),
        20 => Key::F20.into(),
        21 => Key::F21.into(),
        22 => Key::F22.into(),
        23 => Key::F23.into(),
        24 => Key::F24.into(),
        other => panic!("unsupported function key F{other}"),
    };
    dispatch_shared_key_chord(app, key_text, false, false, false);
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

fn select_terminal_welcome_span(app: &AppWindow) {
    let selection_start = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 0.25),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );
    let selection_end = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 10.5),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
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
}

fn drag_terminal_padding_into_grid(app: &AppWindow) {
    let drag_start = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x() - 4.0,
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );
    let drag_end = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 10.5),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );

    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: drag_start,
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: drag_start,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: drag_end });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: drag_end,
        button: PointerEventButton::Left,
    });
}

fn drag_within_first_terminal_cell(app: &AppWindow) {
    let drag_start = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 0.1),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );
    let drag_end = LogicalPosition::new(
        app.get_layout_workspace_session_native_surface_x()
            + (app.get_workspace_session_cell_width() * 0.9),
        app.get_layout_titlebar_height()
            + app.get_layout_workspace_session_native_surface_y()
            + (app.get_workspace_session_cell_height() * 0.5),
    );

    app.window().dispatch_event(WindowEvent::PointerMoved {
        position: drag_start,
    });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position: drag_start,
        button: PointerEventButton::Left,
    });
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position: drag_end });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position: drag_end,
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

fn find_console_asset_id(app: &AppWindow, label: &str) -> String {
    let items = app.get_console_asset_items();
    (0..items.row_count())
        .find_map(|index| {
            items
                .row_data(index)
                .and_then(|row| (row.label.as_str() == label).then(|| row.id.to_string()))
        })
        .unwrap_or_else(|| panic!("expected console asset `{label}`"))
}

fn create_root_snippet(app: &AppWindow, name: &str, script: &str) -> String {
    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());
    app.invoke_asset_snippet_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_snippet_modal_draft_changed("script".into(), script.into());
    app.invoke_confirm_asset_modal_requested();

    app.get_snippet_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn flush_runtime_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn lock_known_hosts_env() -> std::sync::MutexGuard<'static, ()> {
    KNOWN_HOSTS_ENV_LOCK
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn sample_persisted_interrupted_download_task(
    app_root: &std::path::Path,
    temp_target_exists: bool,
) -> TransferTask {
    let download_root = app_root.join("downloads");
    fs::create_dir_all(&download_root).expect("create persisted transfer download root");
    let local_path = download_root.join("release.env");
    let temp_target_path = download_root.join("release.env.part");
    if temp_target_exists {
        fs::write(&temp_target_path, vec![b'x'; 512]).expect("write persisted transfer .part");
    }

    TransferTask {
        id: "persisted-download-task".into(),
        session_id: "persisted-session".into(),
        source_path: "/srv/app/release.env".into(),
        target_path: local_path.to_string_lossy().to_string(),
        direction: TransferDirection::Download,
        action: TransferTaskAction::Download { local_path },
        state: TransferTaskState::Interrupted,
        bytes_total: 1024,
        bytes_transferred: 512,
        bytes_confirmed: 512,
        temp_target_path: Some(temp_target_path),
        resume_mode: TransferResumeMode::ResumeIfPossible,
        conflict_policy: None,
        error_message: Some("network interrupted".into()),
    }
}

#[test]
fn settings_panel_can_create_a_vault_and_persist_local_bootstrap_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("create");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("correct horse battery staple".into());

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("persisted local bootstrap state");

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert!(!local_state.device_id.trim().is_empty());
    assert_eq!(
        local_state.device_id,
        load_or_create_device_id(temp_root.as_path()).unwrap()
    );
}

#[test]
fn missing_local_vault_state_recovers_from_primary_remote_without_uploading_empty_data() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("recover-remote");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let remote_revision =
        sample_remote_revision_for_tree(&password, &asset_tree, source_store.as_ref(), "rev-0004");
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision.clone()));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
    assert_eq!(primary.recorded_writes().len(), 0);

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("recovered local bootstrap state");
    assert_eq!(local_state.current_revision.as_deref(), Some("rev-0004"));
    assert_eq!(local_state.bundle.vault_id, "vault-main");
    assert_eq!(
        local_state.wrapped_vault_key,
        remote_revision.head.wrapped_vault_key
    );
    assert!(!local_state.device_id.trim().is_empty());
    assert_eq!(
        local_state.device_id,
        load_or_create_device_id(temp_root.as_path()).unwrap()
    );

    let cached = load_encrypted_cache(&temp_root.join("cache"), "vault-main")
        .unwrap()
        .expect("recovered encrypted cache");
    assert_eq!(
        cached.payload_sha256,
        remote_revision.encrypted_snapshot.payload_sha256
    );
}

#[test]
fn missing_local_vault_state_with_preexisting_assets_merges_and_pushes_on_attach() {
    run_on_large_stack(
        "missing_local_vault_state_with_preexisting_assets_merges_and_pushes_on_attach",
        missing_local_vault_state_with_preexisting_assets_merges_and_pushes_on_attach_body,
    );
}

fn missing_local_vault_state_with_preexisting_assets_merges_and_pushes_on_attach_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("recover-remote-attach-merge");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.99");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let remote_revision =
        sample_remote_revision_for_tree(&password, &asset_tree, source_store.as_ref(), "rev-0004");
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    create_root_ssh(&app, "Local Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_console_asset_items().row_count(), 2);
    assert_eq!(primary.recorded_writes().len(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("recovered local bootstrap state");
    assert_eq!(local_state.current_revision.as_deref(), Some("rev-0005"));

    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after attach merge");
    let merged_snapshot = decrypt_snapshot(
        &load_encrypted_cache(&temp_root.join("cache"), "vault-main")
            .unwrap()
            .expect("cached merged snapshot"),
        &runtime_vault_key,
    )
    .unwrap();
    assert!(
        merged_snapshot
            .asset_catalog
            .nodes
            .values()
            .any(|node| matches!(
                &node.payload,
                VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.12"
            ))
    );
    assert!(
        merged_snapshot
            .asset_catalog
            .nodes
            .values()
            .any(|node| matches!(
                &node.payload,
                VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.99"
            ))
    );
}

#[test]
fn missing_local_vault_state_surfaces_legacy_remote_as_unrecoverable() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("recover-legacy-remote");
    let password = SecretString::new("vault-pass".into());
    let source_store = Arc::new(MemoryCredentialStore::default());
    let (asset_tree, credential_ref) = sample_vault_asset_tree("10.0.0.100");
    persist_secret_bundle(
        source_store.as_ref(),
        &credential_ref,
        &StoredSshSecretBundle {
            password: Some("hunter2".into()),
            ..StoredSshSecretBundle::default()
        },
    )
    .unwrap();

    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let mut remote_revision =
        sample_remote_revision_for_tree(&password, &asset_tree, source_store.as_ref(), "rev-0004");
    remote_revision
        .manifest
        .provider_capability_fallbacks
        .clear();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("legacy remote revision"),
        "unexpected error: {}",
        app.get_sync_modal_error_text()
    );
    assert!(
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .is_none()
    );
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn unlocking_existing_vault_restores_cached_snapshot_without_loading_while_locked() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
        &KeychainCatalog::default(),
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
            device_id: "device-bootstrap-smoke".into(),
            logical_revision: Some("rev-0001".into()),
            transport_revision_hint: None,
            current_revision: Some("rev-0001".into()),
            local_snapshot_hash: Some(format!("sha256:{}", encrypted.payload_sha256)),
            last_local_change_at: Some("2026-03-31T10:00:00Z".into()),
            last_successful_push_at: None,
            last_successful_pull_at: None,
            last_sync_error: None,
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
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_none()
    );

    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn enabling_sync_persists_runtime_vault_key_material() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("runtime-key-persist");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("persisted local bootstrap state");
    let runtime_key =
        load_runtime_vault_key(credential_store.as_ref(), &local_state.bundle.vault_id)
            .expect("load persisted runtime vault key");

    assert!(runtime_key.is_some());
    assert_ne!(runtime_key.expect("runtime key"), [0u8; 32]);
}

#[test]
fn restart_recovers_vault_session_without_prompting_for_unlock() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("restart-runtime-key");
    let initial_provider_factory = RecordingVaultProviderFactory::default();
    initial_provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));

    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let app = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(initial_provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );

    let asset_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let credential_ref = ssh_credential_ref(&asset_id, SshCredentialKind::SavedSecrets);
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    let persisted_before =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("persisted local bootstrap state before restart");

    let restarted = AppWindow::new().unwrap();
    bind_with_vault_runtime(
        &restarted,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(RecordingVaultProviderFactory::default()),
            bootstrap_template: None,
        },
    );

    assert_eq!(restarted.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );

    restarted.invoke_open_sync_modal_requested();
    assert_eq!(restarted.get_sync_modal_mode().as_str(), "ready");

    let persisted_after =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("persisted local bootstrap state after restart");
    assert_eq!(persisted_after.device_id, persisted_before.device_id);
    assert_eq!(
        persisted_after.device_id,
        load_or_create_device_id(temp_root.as_path()).unwrap()
    );
}

#[test]
fn manual_sync_merges_divergent_local_and_remote_additions_before_push() {
    run_on_large_stack(
        "manual_sync_merges_divergent_local_and_remote_additions_before_push",
        manual_sync_merges_divergent_local_and_remote_additions_before_push_body,
    );
}

fn manual_sync_merges_divergent_local_and_remote_additions_before_push_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("recovery-pull-before-replace");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());
    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() == 1
    });
    assert_eq!(primary.recorded_writes().len(), 1);

    let local_added_asset_id = create_root_ssh(&app, "DB Replica", "10.0.0.24");
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
    .unwrap();
    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("local bootstrap state after first sync");
    let first_write = primary
        .recorded_writes()
        .into_iter()
        .next()
        .expect("first synced write");
    assert_eq!(first_write.head.device_id, local_state.device_id);
    assert_eq!(first_write.head.committed_by_device, local_state.device_id);
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_snapshot = decrypt_snapshot(
        &load_encrypted_cache(&temp_root.join("cache"), "vault-main")
            .unwrap()
            .expect("cached base snapshot"),
        &runtime_vault_key,
    )
    .unwrap();
    let mut remote_seed_snapshot = export_vault_snapshot(
        &remote_tree,
        &KeychainCatalog::default(),
        remote_store.as_ref(),
        &sample_known_hosts_path("manual-sync-remote-seed"),
        SnapshotSyncPreferences::default(),
        &mica_term::app::ui_preferences::UiPreferences::default(),
    )
    .unwrap();
    let remote_seed_id = remote_seed_snapshot
        .asset_catalog
        .root_ids
        .first()
        .cloned()
        .expect("remote seeded asset id");
    let mut remote_seed_node = remote_seed_snapshot
        .asset_catalog
        .nodes
        .remove(&remote_seed_id)
        .expect("remote seeded asset");
    remote_seed_node.id = local_added_asset_id.clone();
    remote_seed_snapshot.asset_catalog.root_ids = vec![local_added_asset_id.clone()];
    remote_seed_snapshot
        .asset_catalog
        .nodes
        .insert(local_added_asset_id.clone(), remote_seed_node);
    if let Some(secret_bundle) = remote_seed_snapshot
        .ssh_secret_bundles
        .remove(&remote_seed_id)
    {
        remote_seed_snapshot
            .ssh_secret_bundles
            .insert(local_added_asset_id.clone(), secret_bundle);
    }
    remote_snapshot
        .asset_catalog
        .root_ids
        .extend(remote_seed_snapshot.asset_catalog.root_ids.clone());
    remote_snapshot
        .asset_catalog
        .nodes
        .extend(remote_seed_snapshot.asset_catalog.nodes.clone());
    remote_snapshot
        .ssh_secret_bundles
        .extend(remote_seed_snapshot.ssh_secret_bundles.clone());
    let mut remote_revision = sample_remote_revision_for_snapshot(
        &remote_snapshot,
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 2
            && app
                .get_sync_modal_status_text()
                .as_str()
                .contains("Merged local and remote changes")
            && load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
                .is_ok()
    });

    assert_eq!(primary.recorded_writes().len(), 2);
    let latest_write = primary
        .recorded_writes()
        .into_iter()
        .last()
        .expect("merged sync write");
    assert_eq!(
        latest_write.head.parent_revision.as_deref(),
        Some("rev-0002")
    );
    assert_eq!(latest_write.head.vault_revision, "rev-0003");
    assert!(
        credential_store
            .get_secret(&remote_credential_ref)
            .unwrap()
            .is_some()
    );

    let recovery_entries =
        load_recovery_snapshots(temp_root.join("recovery").as_path(), "vault-main")
            .expect("load persisted recovery snapshots");
    assert!(recovery_entries.is_empty());

    let cached_snapshot = decrypt_snapshot(
        &load_encrypted_cache(&temp_root.join("cache"), "vault-main")
            .unwrap()
            .expect("cached merged snapshot"),
        &runtime_vault_key,
    )
    .unwrap();
    let cached_hosts = cached_snapshot
        .asset_catalog
        .nodes
        .values()
        .filter_map(|node| match &node.payload {
            VaultAssetPayload::SshConnection(spec) => Some(spec.host.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for host in ["10.0.0.24", "10.0.0.99"] {
        assert!(
            cached_hosts.iter().any(|candidate| candidate == host),
            "expected merged snapshot to contain host `{host}`, got {:?}",
            cached_hosts
        );
    }
}

#[test]
fn manual_sync_modal_returns_before_slow_primary_write_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("manual-sync-background");
    let primary_inner = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let primary = Arc::new(DelayedVaultProvider::new(
        Arc::clone(&primary_inner),
        Duration::ZERO,
        Duration::from_millis(250),
    ));
    let provider_factory = AnyVaultProviderFactory::default();
    provider_factory.insert(primary as Arc<dyn VaultProvider>);

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    let started = Instant::now();
    app.invoke_sync_modal_sync_now_requested();

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "manual sync should return quickly while the provider runs in the background"
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if primary_inner.recorded_writes().len() == 1 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "background sync never completed within the deadline"
        );
        std::thread::sleep(Duration::from_millis(20));
        slint::platform::update_timers_and_animations();
    }
}

#[test]
fn debounced_auto_sync_returns_before_slow_primary_write_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("debounced-auto-sync-background");
    let primary_inner = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    let primary = Arc::new(DelayedVaultProvider::new(
        Arc::clone(&primary_inner),
        Duration::ZERO,
        Duration::from_millis(250),
    ));
    let provider_factory = AnyVaultProviderFactory::default();
    provider_factory.insert(primary as Arc<dyn VaultProvider>);

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");
    assert_eq!(primary_inner.recorded_writes().len(), 0);

    let started = Instant::now();
    settle_sync_scheduler(Duration::from_millis(1300));

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "debounced auto sync should return quickly while the provider writes in the background"
    );
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary_inner.recorded_writes().len() == 1
    });
}

#[test]
fn unlocking_existing_vault_waits_for_a_real_mutation_before_background_sync() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("unlock-auto-sync");
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
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    assert_eq!(primary.recorded_writes().len(), 0);

    app.invoke_sync_modal_close_requested();
    app.invoke_open_sync_modal_requested();

    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(primary.recorded_writes().len(), 0);
}

#[test]
fn asset_mutation_syncs_without_auto_sync_toggle() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("asset-auto-sync");
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
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();
    assert_eq!(primary.recorded_writes().len(), 0);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    assert_eq!(primary.recorded_writes().len(), 0);
    settle_sync_scheduler(Duration::from_millis(1300));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 1
            && app
                .get_sync_modal_status_text()
                .as_str()
                .contains("Primary is now at")
    });

    let folder_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved folder asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(folder_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();
    assert_eq!(primary.recorded_writes().len(), 1);
    settle_sync_scheduler(Duration::from_millis(1300));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 2
            && app
                .get_sync_modal_status_text()
                .as_str()
                .contains("Primary is now at")
    });

    app.invoke_asset_context_menu_requested(folder_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();
    assert_eq!(primary.recorded_writes().len(), 2);
    settle_sync_scheduler(Duration::from_millis(1300));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 3
            && app
                .get_sync_modal_status_text()
                .as_str()
                .contains("Primary is now at")
    });
    assert_eq!(primary.recorded_writes().len(), 3);
}

#[test]
fn periodic_sync_pulls_remote_changes_even_without_local_dirty_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("periodic-pull-clean-local");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() == 1
    });
    app.invoke_sync_modal_close_requested();

    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(app.get_console_asset_items().row_count(), 0);

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
    .unwrap();
    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("local bootstrap state after initial sync");
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_revision = sample_remote_revision_for_existing_vault_key(
        &remote_tree,
        remote_store.as_ref(),
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    settle_sync_scheduler(Duration::from_secs(121));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        app.get_console_asset_items().row_count() == 1
    });

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "periodic sync should pull clean remote changes instead of pushing a new head"
    );
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&remote_credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn periodic_sync_returns_before_slow_primary_refresh_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("periodic-sync-background");
    let primary_inner = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let primary = Arc::new(DelayedVaultProvider::new(
        Arc::clone(&primary_inner),
        Duration::from_millis(250),
        Duration::ZERO,
    ));
    let provider_factory = AnyVaultProviderFactory::default();
    provider_factory.insert(primary as Arc<dyn VaultProvider>);

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary_inner.recorded_writes().len() == 1
    });
    app.invoke_sync_modal_close_requested();

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
    .unwrap();
    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("local bootstrap state after initial sync");
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_revision = sample_remote_revision_for_existing_vault_key(
        &remote_tree,
        remote_store.as_ref(),
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary_inner.set_remote_head(Some(remote_revision.head.clone()));
    primary_inner.set_remote_revision(Some(remote_revision));

    let started = Instant::now();
    settle_sync_scheduler(Duration::from_secs(121));

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "periodic sync should return quickly while the provider refresh runs in the background"
    );
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        app.get_console_asset_items().row_count() == 1
    });
}

#[test]
fn periodic_sync_conflicts_use_merge_engine_and_persist_conflict_copies() {
    run_on_large_stack(
        "periodic_sync_conflicts_use_merge_engine_and_persist_conflict_copies",
        periodic_sync_conflicts_use_merge_engine_and_persist_conflict_copies_body,
    );
}

fn periodic_sync_conflicts_use_merge_engine_and_persist_conflict_copies_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("periodic-pull-recovery");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    ));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        VaultRuntimeOptions {
            root_dir: Some(temp_root.clone()),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() == 1
    });
    assert_eq!(primary.recorded_writes().len(), 1);

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("synced ssh asset")
        .id
        .to_string();

    primary.set_write_error(Some("temporary outage"));
    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.24".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());
    settle_sync_scheduler(Duration::from_millis(1300));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        app.get_sync_modal_error_text()
            .as_str()
            .contains("temporary outage")
    });
    assert_eq!(primary.recorded_writes().len(), 1);
    primary.set_write_error(None);

    let local_state =
        load_local_vault_bootstrap_state(&temp_root.join("vault-bootstrap-state.json"))
            .unwrap()
            .expect("local bootstrap state after dirty local mutation");
    let runtime_vault_key = load_runtime_vault_key(credential_store.as_ref(), "vault-main")
        .unwrap()
        .expect("runtime vault key after enabling sync");
    let mut remote_snapshot = decrypt_snapshot(
        &load_encrypted_cache(&temp_root.join("cache"), "vault-main")
            .unwrap()
            .expect("cached base snapshot"),
        &runtime_vault_key,
    )
    .unwrap();
    let remote_node = remote_snapshot
        .asset_catalog
        .nodes
        .get_mut(&ssh_id)
        .expect("remote asset");
    remote_node.title = "Remote Bastion".into();
    match &mut remote_node.payload {
        VaultAssetPayload::SshConnection(spec) => {
            spec.host = "10.0.0.99".into();
        }
        other => panic!("unexpected payload: {other:?}"),
    }
    let mut remote_revision = sample_remote_revision_for_snapshot(
        &remote_snapshot,
        "rev-0002",
        &runtime_vault_key,
        &local_state.wrapped_vault_key,
        &local_state.kdf,
    );
    remote_revision.head.parent_revision = Some("rev-0001".into());
    remote_revision.head.committed_at = "99999999999999999999".into();
    primary.set_remote_head(Some(remote_revision.head.clone()));
    primary.set_remote_revision(Some(remote_revision));

    settle_sync_scheduler(Duration::from_secs(121));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 2
    });

    assert_eq!(primary.recorded_writes().len(), 2);
    let latest_write = primary
        .recorded_writes()
        .into_iter()
        .last()
        .expect("merged periodic sync write");
    assert_eq!(
        latest_write.head.parent_revision.as_deref(),
        Some("rev-0002")
    );
    assert_eq!(latest_write.head.vault_revision, "rev-0003");
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    let recovery_entries =
        load_recovery_snapshots(temp_root.join("recovery").as_path(), "vault-main")
            .expect("load persisted periodic recovery snapshots");
    assert_eq!(recovery_entries.len(), 2);
    assert!(
        recovery_entries
            .iter()
            .any(|entry| entry.source == RecoverySource::LocalConflictCopy)
    );
    assert!(
        recovery_entries
            .iter()
            .any(|entry| entry.source == RecoverySource::RemoteConflictCopy)
    );

    let merged_snapshot = decrypt_snapshot(
        &load_encrypted_cache(&temp_root.join("cache"), "vault-main")
            .unwrap()
            .expect("cached merged snapshot"),
        &runtime_vault_key,
    )
    .unwrap();
    assert!(
        merged_snapshot
            .asset_catalog
            .nodes
            .values()
            .any(|node| matches!(
                &node.payload,
                VaultAssetPayload::SshConnection(spec) if spec.host == "10.0.0.24"
            ))
    );
}

#[test]
fn back_to_back_mutations_share_one_debounced_auto_sync_upload() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("debounced-auto-sync");
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
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");
    create_root_snippet(&app, "Restart api", "kubectl rollout restart deploy/web");

    assert_eq!(primary.recorded_writes().len(), 0);
    settle_sync_scheduler(Duration::from_millis(1300));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() == 1
    });

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "two quick local mutations should collapse into one debounced upload"
    );
}

#[test]
fn periodic_auto_sync_retries_failed_dirty_changes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("periodic-auto-sync-retry");
    let primary = Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::s3_like(),
    ));
    primary.set_write_error(Some("temporary outage"));
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(primary.clone());

    let mut bundle = sample_bootstrap_bundle_with_primary_and_mirror();
    bundle
        .remotes
        .retain(|remote| remote.role == RemoteRole::Primary);

    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store,
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(bundle),
        },
    );
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    app.invoke_sync_modal_close_requested();

    create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");
    settle_sync_scheduler(Duration::from_millis(1300));
    assert_eq!(primary.recorded_writes().len(), 0);

    primary.set_write_error(None);
    settle_sync_scheduler(Duration::from_secs(121));
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() == 1
    });

    assert_eq!(
        primary.recorded_writes().len(),
        1,
        "periodic sync should retry a dirty local change after the provider becomes writable again"
    );
}

#[test]
fn manual_vault_sync_reports_mirror_degradation_after_primary_commit() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(VAULT_SYNC_WAIT_TIMEOUT, || {
        primary.recorded_writes().len() >= 1
            && app
                .get_sync_modal_status_text()
                .as_str()
                .contains("mirror unavailable")
    });

    assert_eq!(primary.recorded_writes().len(), 1);
    assert_eq!(mirror.recorded_writes().len(), 0);
    assert!(
        app.get_sync_modal_status_text()
            .as_str()
            .contains("mirror unavailable")
    );
}

#[test]
fn manual_vault_sync_surfaces_provider_auth_errors_in_panel_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("provider-auth");
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
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());
    primary.set_read_error(Some("token expired"));

    app.invoke_sync_modal_sync_now_requested();
    wait_for_condition(Duration::from_secs(2), || {
        app.get_sync_modal_error_text()
            .as_str()
            .contains("token expired")
    });

    assert!(
        app.get_sync_modal_error_text()
            .as_str()
            .contains("token expired")
    );
}

#[test]
fn locking_vault_clears_decrypted_assets_and_secrets_from_memory() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("lock");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
    let app = AppWindow::new().unwrap();
    let credential_store = Arc::new(MemoryCredentialStore::default());
    bind_with_vault_runtime(
        &app,
        Arc::new(FakeLauncher),
        credential_store.clone(),
        VaultRuntimeOptions {
            root_dir: Some(temp_root),
            provider_factory: Arc::new(provider_factory),
            bootstrap_template: Some(sample_bootstrap_bundle_with_primary_and_mirror()),
        },
    );
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let credential_ref = ssh_credential_ref(&ssh_id, SshCredentialKind::SavedSecrets);
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );

    app.invoke_sync_modal_close_requested();

    assert!(!app.get_sync_modal_open());
    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert!(
        credential_store
            .get_secret(&credential_ref)
            .unwrap()
            .is_some()
    );
}

#[test]
fn locking_and_unlocking_vault_round_trips_snippet_assets() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_root = sample_vault_runtime_root("snippet-lock");
    let provider_factory = RecordingVaultProviderFactory::default();
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-primary",
        ProviderCapabilities::bundled_files_like(),
    )));
    provider_factory.insert(Arc::new(MockVaultProvider::new(
        "remote-mirror",
        ProviderCapabilities::bundled_files_like(),
    )));
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

    create_root_snippet(&app, "Restart API", "kubectl rollout restart deploy/api");
    app.invoke_open_sync_modal_requested();
    app.invoke_sync_modal_submit_master_password("vault-pass".into());

    assert_eq!(app.get_snippet_asset_items().row_count(), 1);

    app.invoke_sync_modal_close_requested();
    assert!(!app.get_sync_modal_open());
    assert_eq!(app.get_snippet_asset_items().row_count(), 1);

    app.invoke_open_sync_modal_requested();
    assert_eq!(app.get_sync_modal_mode().as_str(), "ready");
    assert_eq!(app.get_snippet_asset_items().row_count(), 1);
    assert_eq!(
        app.get_snippet_asset_items()
            .row_data(0)
            .expect("snippet row after unlock")
            .kind
            .as_str(),
        "snippet"
    );
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
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
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

fn loaded_catalog_with_snippets_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec![
            "folder-root".into(),
            "ssh-root".into(),
            "snippet-package-root".into(),
            "snippet-root".into(),
        ],
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
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
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
                "snippet-package-root".into(),
                PersistedAssetNode {
                    id: "snippet-package-root".into(),
                    parent_id: None,
                    title: "Deploy".into(),
                    kind: PersistedAssetKind::SnippetPackage,
                    child_ids: vec!["snippet-child".into()],
                    payload: PersistedAssetPayload::SnippetPackage,
                },
            ),
            (
                "snippet-child".into(),
                PersistedAssetNode {
                    id: "snippet-child".into(),
                    parent_id: Some("snippet-package-root".into()),
                    title: "Deploy prod".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl apply -f prod.yaml".into(),
                        package_id: Some("snippet-package-root".into()),
                    }),
                },
            ),
            (
                "snippet-root".into(),
                PersistedAssetNode {
                    id: "snippet-root".into(),
                    parent_id: None,
                    title: "Restart API".into(),
                    kind: PersistedAssetKind::Snippet,
                    child_ids: Vec::new(),
                    payload: PersistedAssetPayload::Snippet(PersistedSnippetSpec {
                        script: "kubectl rollout restart deploy/api".into(),
                        package_id: None,
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
                    auth_source: String::new(),
                    keychain_identity_id: None,
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
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
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

fn loaded_keychain_identity_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-identity".into()],
        nodes: BTreeMap::from([(
            "ssh-identity".into(),
            PersistedAssetNode {
                id: "ssh-identity".into(),
                parent_id: None,
                title: "Identity Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.99".into(),
                    user: String::new(),
                    port: "22".into(),
                    auth_method: String::new(),
                    auth_source: "keychain-identity".into(),
                    keychain_identity_id: Some("identity-prod".into()),
                    private_key_source: String::new(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::None,
                    remark: "Identity-backed".into(),
                    credential_ref: None,
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
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
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
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
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
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
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
                        auth_source: "manual".into(),
                        keychain_identity_id: None,
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

fn loaded_saved_http_ssh_catalog_for_bootstrap() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: vec!["ssh-http".into()],
        nodes: BTreeMap::from([(
            "ssh-http".into(),
            PersistedAssetNode {
                id: "ssh-http".into(),
                parent_id: None,
                title: "HTTP Bastion".into(),
                kind: PersistedAssetKind::SshConnection,
                child_ids: Vec::new(),
                payload: PersistedAssetPayload::SshConnection(PersistedSshConnectionSpec {
                    host: "10.0.0.35".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
                    private_key_source: "content".into(),
                    private_key_path: String::new(),
                    environment: "prod".into(),
                    proxy: PersistedAssetSshProxySpec::Http(PersistedAssetSocks5ProxySpec {
                        host: "proxy.example.net".into(),
                        port: "8080".into(),
                        username: "ops-proxy".into(),
                        password_credential_ref: Some("ssh/saved-secrets/ssh-http".into()),
                    }),
                    remark: "Saved HTTP proxy credential".into(),
                    credential_ref: Some("ssh/saved-secrets/ssh-http".into()),
                }),
            },
        )]),
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
                    auth_source: "manual".into(),
                    keychain_identity_id: None,
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
fn bootstrap_shared_credential_store_reloads_saved_secret_when_system_store_is_empty_after_restart()
{
    let encrypted_root = sample_credential_root("secure-restart");
    let recovery_root = sample_credential_root("recovery-restart");
    let credential_ref = ssh_credential_ref("asset-prod", SshCredentialKind::SavedSecrets);
    let first_primary = Arc::new(MemoryCredentialStore::default()) as Arc<dyn CredentialStore>;
    let first_store = build_shared_app_credential_store_for_paths(
        Some(first_primary),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    persist_secret_bundle(
        first_store.as_ref(),
        credential_ref.as_str(),
        &StoredSshSecretBundle {
            password: Some("super-secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: None,
        },
    )
    .expect("persist bundle through initial shared bootstrap store");

    let second_store = build_shared_app_credential_store_for_paths(
        Some(Arc::new(MemoryCredentialStore::default()) as Arc<dyn CredentialStore>),
        encrypted_root.clone(),
        recovery_root.clone(),
    );

    assert_eq!(
        load_secret_bundle(second_store.as_ref(), credential_ref.as_str())
            .expect("reload shared credential bundle after restart")
            .password
            .as_deref(),
        Some("super-secret")
    );

    let _ = fs::remove_dir_all(encrypted_root);
    let _ = fs::remove_dir_all(recovery_root);
}

#[test]
fn bootstrap_loads_catalog_before_first_asset_projection_sync() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn bootstrap_loads_snippets_from_repository_without_leaking_them_into_console_projection() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_with_snippets_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    assert_eq!(repo_state.borrow().load_calls, 1);
    assert_eq!(app.get_console_asset_items().row_count(), 2);
    assert_eq!(app.get_snippet_asset_items().row_count(), 2);
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("console folder row")
            .kind
            .as_str(),
        "folder"
    );
    assert_eq!(
        app.get_snippet_asset_items()
            .row_data(0)
            .expect("snippet package row")
            .kind
            .as_str(),
        "snippet-package"
    );
}

#[test]
fn unrelated_catalog_saves_preserve_keychain_identity_host_fields() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_keychain_identity_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Scratch".into());
    app.invoke_confirm_asset_modal_requested();

    let persisted_catalog = repo_state
        .borrow()
        .save_attempts
        .last()
        .expect("persisted catalog after unrelated save")
        .clone();
    let PersistedAssetPayload::SshConnection(spec) = &persisted_catalog
        .nodes
        .get("ssh-identity")
        .expect("persisted keychain-backed ssh node")
        .payload
    else {
        panic!("expected persisted ssh connection payload");
    };
    assert_eq!(spec.auth_source, "keychain-identity");
    assert_eq!(spec.keychain_identity_id.as_deref(), Some("identity-prod"));
}

#[test]
fn activating_legacy_saved_ssh_asset_defaults_missing_auth_fields_and_opens_session() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
}

#[test]
fn opening_slow_saved_ssh_asset_returns_before_probe_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(SlowOpeningLauncher {
            state: Arc::clone(&launcher_state),
            probe_delay: Duration::from_millis(250),
            launch_delay: Duration::from_millis(250),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
    assert!(
        launcher_state
            .lock()
            .expect("lock slow opening launcher state")
            .probe_profiles
            .is_empty(),
        "opening a workspace SSH asset should create the tab before any synchronous probe runs"
    );
}

#[test]
fn opening_saved_ssh_asset_twice_creates_two_tabs() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .tab_id
        .to_string();

    app.invoke_asset_activated(ssh_id.into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .tab_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn editing_legacy_saved_ssh_asset_reuses_fallback_saved_secret_for_test_connection() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    flush_runtime_projection();

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Connection test succeeded."
    );
}

#[test]
fn editing_saved_password_modal_hydrates_real_secret_masked() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn editing_saved_upstream_ssh_modal_excludes_current_asset_from_dropdown_options() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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

    let options = app.get_asset_ssh_modal_proxy_ssh_options();
    assert_eq!(options.row_count(), 1);
    assert_eq!(options.row_data(0).unwrap().as_str(), "Upstream Bastion");
}

#[test]
fn editing_saved_http_modal_hydrates_proxy_fields_and_proxy_password() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_saved_http_ssh_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    persist_secret_bundle(
        credential_store.as_ref(),
        "ssh/saved-secrets/ssh-http",
        &StoredSshSecretBundle {
            password: Some("secret".into()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password: Some("proxy-secret".into()),
        },
    )
    .expect("persist saved http secret bundle");
    bind_with_launcher_and_credential_store(
        &app,
        Some(asset_repo),
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
    );

    let ssh_id = app
        .get_console_asset_items()
        .row_data(0)
        .expect("saved http ssh asset")
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_proxy_type().as_str(), "http");
    assert_eq!(
        app.get_asset_ssh_modal_proxy_socks5_host().as_str(),
        "proxy.example.net"
    );
    assert_eq!(app.get_asset_ssh_modal_proxy_socks5_port().as_str(), "8080");
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
fn test_connection_with_missing_upstream_reports_inline_feedback_without_probe() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn manual_ssh_modal_private_key_import_still_populates_inline_content() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_launcher_and_credential_store_and_private_key_importer(
        &app,
        None,
        Arc::new(FakeLauncher),
        Arc::clone(&credential_store),
        Arc::new(SuccessfulPrivateKeyImporter {
            path: std::path::PathBuf::from("/tmp/id_ed25519"),
            content: "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n",
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_action_requested("import-private-key".into());

    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(
        app.get_asset_ssh_modal_private_key_content().as_str(),
        "-----BEGIN OPENSSH PRIVATE KEY-----\nimported\n-----END OPENSSH PRIVATE KEY-----\n"
    );
    assert_eq!(
        app.get_asset_ssh_modal_auth_method().as_str(),
        "private-key"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_source().as_str(),
        "content"
    );
}

#[test]
fn importing_private_key_reports_feedback_when_file_selection_fails() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn snippet_create_persists_into_repository_catalog_alongside_console_assets() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));

    bind_with_fake_sessions(&app, Some(asset_repo));

    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());
    app.invoke_asset_snippet_modal_draft_changed("name".into(), "Restart API".into());
    app.invoke_asset_snippet_modal_draft_changed(
        "script".into(),
        "kubectl rollout restart deploy/api".into(),
    );
    app.invoke_confirm_asset_modal_requested();

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    assert!(
        save_attempts[0]
            .nodes
            .values()
            .any(|node| node.kind == PersistedAssetKind::Snippet)
    );
    assert!(
        save_attempts[0]
            .nodes
            .values()
            .any(|node| node.kind == PersistedAssetKind::Folder)
    );
}

#[test]
fn saving_self_referential_upstream_proxy_is_blocked_before_runtime_launch() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Upstream SSH connection cannot reference itself."
    );
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "idle");
}

#[test]
fn save_failure_logs_error_without_persisting_ui_session_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
        data_dir: temp_root.join("data"),
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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    assert!(launcher_state.probe_profiles.is_empty());
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
fn connect_action_returns_before_any_probe_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(SlowOpeningLauncher {
            state: Arc::clone(&launcher_state),
            probe_delay: Duration::from_millis(250),
            launch_delay: Duration::from_millis(250),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());

    let started = Instant::now();
    app.invoke_asset_ssh_modal_action_requested("connect".into());

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "opening an ephemeral SSH session from the modal should not block on probe_connection()"
    );
    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
    assert!(
        launcher_state
            .lock()
            .expect("lock slow opening launcher state")
            .probe_profiles
            .is_empty(),
        "modal connect should open the workspace tab without waiting for a synchronous probe"
    );
}

#[test]
fn connect_action_reuses_existing_ephemeral_session_for_same_draft() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn quick_launch_connect_opens_saved_asset_session_and_records_recent_items() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
        app.show().expect("show app window");

        create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        create_root_ssh(&app, "DB Replica", "10.0.0.24");
        let find_asset_id = |label: &str| {
            let rows = app.get_console_asset_items();
            (0..rows.row_count())
                .filter_map(|index| rows.row_data(index))
                .find(|row| row.label.as_str() == label)
                .map(|row| row.id.to_string())
                .expect("asset id by label")
        };
        let prod_id = find_asset_id("Prod Bastion");
        let db_id = find_asset_id("DB Replica");

        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_quick_launch_connect_requested(prod_id.clone().into());
        settle_terminal_projection();
        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_quick_launch_connect_requested(db_id.clone().into());
        settle_terminal_projection();

        let recent = app.get_welcome_quick_launch_recent_items();
        assert_eq!(app.get_workspace_tab_items().row_count(), 2);
        assert_eq!(recent.row_count(), 2);
        let recent_ids = (0..recent.row_count())
            .filter_map(|index| recent.row_data(index))
            .map(|row| row.asset_id.to_string())
            .collect::<Vec<_>>();
        assert!(recent_ids.iter().any(|asset_id| asset_id == &prod_id));
        assert!(recent_ids.iter().any(|asset_id| asset_id == &db_id));
        assert!(
            !recent
                .row_data(0)
                .expect("recent row 0")
                .state_label
                .as_str()
                .is_empty(),
            "recent row should expose connected state when a saved SSH session is already open"
        );
    });
}

#[test]
fn asset_activation_records_saved_ssh_into_new_tab_recent_items() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.clone().into());
        settle_terminal_projection();
        app.invoke_workspace_new_tab_requested();

        let recent = app.get_welcome_quick_launch_recent_items();
        assert_eq!(recent.row_count(), 1);
        let row = recent.row_data(0).expect("connected recent row");
        assert_eq!(row.asset_id.as_str(), ssh_id.as_str());
        assert!(
            !row.time_label.as_str().is_empty(),
            "asset activation should record the saved SSH in recent items even before the runtime reaches connected state"
        );
    });
}

#[test]
fn active_recent_connection_row_returns_to_existing_tab_without_duplicate_session() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.clone().into());
        settle_terminal_projection();
        let existing_session_id = app.get_active_workspace_session_id().to_string();

        app.invoke_workspace_new_tab_requested();
        assert_eq!(app.get_workspace_tab_items().row_count(), 2);

        app.invoke_welcome_quick_launch_connect_requested(ssh_id.into());

        assert_eq!(app.get_workspace_tab_items().row_count(), 1);
        assert_eq!(
            app.get_active_workspace_session_id().as_str(),
            existing_session_id.as_str()
        );
        let active_tab = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("existing tab after active recent row click");
        assert_eq!(active_tab.tab_id.as_str(), existing_session_id.as_str());
        assert_eq!(active_tab.title.as_str(), "Prod Bastion");
    });
}

#[test]
fn workspace_new_tab_request_opens_single_launcher_tab() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        app.invoke_workspace_new_tab_requested();
        app.invoke_workspace_new_tab_requested();

        assert_eq!(app.get_workspace_tab_items().row_count(), 1);
        assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
        assert_eq!(
            app.get_active_workspace_session_id().as_str(),
            "workspace-launcher"
        );
    });
}

#[test]
fn workspace_new_tab_request_collapses_native_terminal_surface_rect_immediately() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "terminal mode should expose a concrete native surface rect before switching back to the launcher"
    );

    app.invoke_workspace_new_tab_requested();

    assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    assert_eq!(
        app.get_layout_workspace_session_native_surface_width(),
        0.0,
        "opening the launcher tab should collapse the retained native terminal width immediately so the old child surface cannot keep covering the welcome host until a later layout invalidation"
    );
    assert_eq!(
        app.get_layout_workspace_session_native_surface_height(),
        0.0,
        "opening the launcher tab should collapse the retained native terminal height immediately so the old child surface cannot keep intercepting paint and hit-testing outside terminal mode"
    );
}

#[test]
fn workspace_tab_selection_restores_native_terminal_surface_rect_immediately() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let session_id = app.get_active_workspace_session_id().to_string();
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "terminal mode should expose a concrete native surface rect before switching away from the active session"
    );

    app.invoke_workspace_new_tab_requested();

    assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    assert_eq!(app.get_layout_workspace_session_native_surface_width(), 0.0);
    assert_eq!(
        app.get_layout_workspace_session_native_surface_height(),
        0.0
    );

    app.invoke_workspace_tab_selected(session_id.clone().into());

    assert_eq!(app.get_active_workspace_session_id().as_str(), session_id);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "reselecting the terminal tab should restore the native surface rect immediately so the retained surface can realign with the terminal host in the same callback turn"
    );
    assert!(
        app.get_workspace_session_surface_seqno() > 0,
        "reselecting the terminal tab should restore the staged terminal payload together with the host mode so the geometry rebind is not backed by an empty frame"
    );
}

#[test]
fn launcher_recent_connection_replaces_launcher_tab_with_real_session_tab() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_quick_launch_connect_requested(ssh_id.into());

        assert_eq!(app.get_workspace_tab_items().row_count(), 1);
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after launcher connect");
        assert_ne!(item.tab_id.as_str(), "workspace-launcher");
        assert_eq!(item.title.as_str(), "Prod Bastion");
    });
}

#[test]
fn workspace_tab_reorder_callback_preserves_active_session_and_ui_order() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        let prod_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        let stage_id = create_root_ssh(&app, "Stage Bastion", "10.0.0.22");

        app.invoke_asset_activated(prod_id.into());
        settle_terminal_projection();
        let first_tab_id = app.get_active_workspace_session_id().to_string();

        app.invoke_asset_activated(stage_id.into());
        settle_terminal_projection();
        let second_tab_id = app.get_active_workspace_session_id().to_string();

        let tab_ids = |app: &AppWindow| {
            let tabs = app.get_workspace_tab_items();
            (0..tabs.row_count())
                .map(|index| {
                    tabs.row_data(index)
                        .expect("workspace tab row")
                        .tab_id
                        .to_string()
                })
                .collect::<Vec<_>>()
        };

        assert_eq!(
            tab_ids(&app),
            vec![first_tab_id.clone(), second_tab_id.clone()]
        );
        assert_eq!(
            app.get_active_workspace_session_id().as_str(),
            second_tab_id
        );

        app.invoke_workspace_tab_reorder_requested(second_tab_id.clone().into(), 0);

        assert_eq!(tab_ids(&app), vec![second_tab_id.clone(), first_tab_id]);
        assert_eq!(
            app.get_active_workspace_session_id().as_str(),
            second_tab_id,
            "drag reorder should only change UI order and must not switch the active session"
        );
    });
}

#[test]
fn workspace_tab_menu_tooltip_and_close_all_follow_session_first_contract() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
            Ok(())
        })
        .expect("clear clipboard");

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        let prod_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        let stage_id = create_root_ssh(&app, "Stage Bastion", "10.0.0.22");

        app.invoke_asset_activated(prod_id.into());
        settle_terminal_projection();
        let prod_tab_id = app.get_active_workspace_session_id().to_string();

        app.invoke_asset_activated(stage_id.into());
        settle_terminal_projection();
        let stage_tab_id = app.get_active_workspace_session_id().to_string();

        app.invoke_workspace_tab_hovered(prod_tab_id.clone().into(), 144.0, 84.0);
        assert!(app.get_workspace_tab_tooltip_visible());
        let tooltip_text = app.get_workspace_tab_tooltip_text().to_string();
        assert!(
            !tooltip_text.is_empty(),
            "tab hover should publish tooltip copy through the shared app-window overlay state"
        );
        app.invoke_workspace_tab_hover_ended(prod_tab_id.clone().into());
        assert!(!app.get_workspace_tab_tooltip_visible());

        app.invoke_workspace_tab_context_menu_requested(prod_tab_id.clone().into(), 168.0, 96.0);
        assert!(app.get_workspace_tab_context_menu_open());
        assert_eq!(
            app.get_active_workspace_session_id().as_str(),
            stage_tab_id,
            "opening the context menu on an inactive tab must not switch the active session"
        );

        app.invoke_workspace_tab_context_menu_action_invoked("copy-name".into());
        let copied_name = i_slint_backend_selector::with_platform(|platform| {
            Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
        })
        .expect("read copied name");
        assert_eq!(copied_name.as_deref(), Some("Prod Bastion"));

        app.invoke_workspace_tab_context_menu_requested(prod_tab_id.clone().into(), 168.0, 96.0);
        app.invoke_workspace_tab_context_menu_action_invoked("copy-host".into());
        let copied_host = i_slint_backend_selector::with_platform(|platform| {
            Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
        })
        .expect("read copied host");
        assert_eq!(copied_host.as_deref(), Some("10.0.0.12"));

        app.invoke_workspace_tab_context_menu_requested(prod_tab_id.into(), 168.0, 96.0);
        app.invoke_workspace_tab_context_menu_action_invoked("close-all".into());

        assert_eq!(app.get_workspace_tab_items().row_count(), 0);
        assert_eq!(app.get_active_workspace_session_id().as_str(), "");
        assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    });
}

#[test]
fn launcher_quick_launch_connect_restores_native_terminal_surface_rect() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
        app.show().expect("show app window");

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

        app.invoke_workspace_new_tab_requested();

        assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
        assert_eq!(app.get_layout_workspace_session_native_surface_width(), 0.0);
        assert_eq!(
            app.get_layout_workspace_session_native_surface_height(),
            0.0
        );

        app.invoke_welcome_quick_launch_connect_requested(ssh_id.into());
        settle_terminal_projection();

        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after launcher quick launch connect");
        assert_ne!(item.tab_id.as_str(), "workspace-launcher");
        assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
        assert!(
            app.get_layout_workspace_session_native_surface_width() > 0.0
                && app.get_layout_workspace_session_native_surface_height() > 0.0,
            "launcher quick launch connect should restore the native terminal rect as soon as the launcher tab is replaced so the retained surface does not stay collapsed under a live terminal host"
        );
        assert!(
            app.get_workspace_session_surface_seqno() > 0,
            "launcher quick launch connect should stage a terminal payload together with the restored host mode so the revived geometry is backed by a real frame"
        );
    });
}

#[test]
fn duplicate_ssh_tabs_keep_resolved_titles_and_reuse_suffix_gaps() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.clone().into());
    app.invoke_asset_activated(ssh_id.clone().into());
    app.invoke_asset_activated(ssh_id.clone().into());

    let rows = app.get_workspace_tab_items();
    assert_eq!(rows.row_count(), 3);
    assert_eq!(
        rows.row_data(0).expect("first prod tab").title.as_str(),
        "Prod Bastion"
    );
    assert_eq!(
        rows.row_data(1).expect("second prod tab").title.as_str(),
        "Prod Bastion(2)"
    );
    assert_eq!(
        rows.row_data(2).expect("third prod tab").title.as_str(),
        "Prod Bastion(3)"
    );

    let second_tab_session_id = rows
        .row_data(1)
        .expect("second prod tab session id")
        .tab_id
        .to_string();
    app.invoke_workspace_tab_close_requested(second_tab_session_id.into());

    app.invoke_asset_activated(ssh_id.into());

    let reopened_rows = app.get_workspace_tab_items();
    assert_eq!(reopened_rows.row_count(), 3);
    assert_eq!(
        reopened_rows
            .row_data(0)
            .expect("first reopened tab")
            .title
            .as_str(),
        "Prod Bastion"
    );
    assert_eq!(
        reopened_rows
            .row_data(1)
            .expect("reused suffix tab")
            .title
            .as_str(),
        "Prod Bastion(3)"
    );
    assert_eq!(
        reopened_rows
            .row_data(2)
            .expect("reopened duplicate tab")
            .title
            .as_str(),
        "Prod Bastion(2)"
    );
}

#[test]
fn launcher_picker_activation_replaces_launcher_tab_and_closes_modal() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        let ssh_id = create_root_ssh(&app, "DB Admin", "10.0.0.24");

        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        assert!(app.get_open_saved_ssh_modal_open());

        app.invoke_open_saved_ssh_modal_asset_activated(ssh_id.into());

        assert!(!app.get_open_saved_ssh_modal_open());
        assert_eq!(app.get_workspace_tab_items().row_count(), 1);
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after picker activation");
        assert_ne!(item.tab_id.as_str(), "workspace-launcher");
        assert_eq!(item.title.as_str(), "DB Admin");
    });
}

#[test]
fn launcher_picker_primary_open_request_activates_the_selected_saved_ssh() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        create_root_ssh(&app, "Prod Bastion", "10.0.0.10");
        create_root_ssh(&app, "DB Admin", "10.0.0.24");
        let ssh_id = find_console_asset_id(&app, "DB Admin");

        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        assert!(app.get_open_saved_ssh_modal_open());
        assert!(app.get_open_saved_ssh_modal_can_open_selection());

        app.invoke_open_saved_ssh_modal_asset_selected(ssh_id.clone().into());
        assert!(app.get_open_saved_ssh_modal_can_open_selection());

        app.invoke_open_saved_ssh_modal_activate_selection_requested();

        assert!(!app.get_open_saved_ssh_modal_open());
        assert_eq!(app.get_workspace_tab_items().row_count(), 1);
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after picker primary open");
        assert_eq!(item.title.as_str(), "DB Admin");
    });
}

#[test]
fn launcher_picker_move_selection_request_advances_keyboard_target() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);

        create_root_ssh(&app, "Prod Bastion", "10.0.0.10");
        create_root_ssh(&app, "DB Admin", "10.0.0.24");
        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        assert!(app.get_open_saved_ssh_modal_can_open_selection());

        app.invoke_open_saved_ssh_modal_move_selection_requested(1);
        app.invoke_open_saved_ssh_modal_activate_selection_requested();

        assert!(!app.get_open_saved_ssh_modal_open());
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after keyboard picker activation");
        assert_eq!(item.title.as_str(), "DB Admin");
    });
}

#[test]
fn launcher_picker_down_arrow_key_advances_selection_and_enter_opens() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);
        app.show().expect("show app window");

        create_root_ssh(&app, "Prod Bastion", "10.0.0.10");
        create_root_ssh(&app, "DB Admin", "10.0.0.24");
        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(8));
        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));

        let window_size = app.window().size();
        let modal_x = ((window_size.width as f32) - 720.0) / 2.0;
        let modal_y = app.get_layout_titlebar_height()
            + (((window_size.height as f32) - app.get_layout_titlebar_height() - 620.0) / 2.0);
        let search_position = LogicalPosition::new(modal_x + 120.0, modal_y + 68.0 + 49.0);
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: search_position,
        });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: search_position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(30));
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: search_position,
            button: PointerEventButton::Left,
        });

        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::DownArrow.into(),
        });
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::DownArrow.into(),
        });
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "\n".into() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "\n".into() });

        assert!(!app.get_open_saved_ssh_modal_open());
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after keyboard down-arrow activation");
        assert_eq!(item.title.as_str(), "DB Admin");
    });
}

#[test]
fn launcher_picker_second_click_on_the_same_row_opens_the_saved_ssh() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);
        app.show().expect("show app window");

        create_root_ssh(&app, "Prod Bastion", "10.0.0.10");
        create_root_ssh(&app, "DB Admin", "10.0.0.24");
        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(8));
        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));

        let window_size = app.window().size();
        let modal_width = 720.0f32;
        let modal_height = 620.0f32;
        let header_height = 68.0f32;
        let row_height = 54.0f32;
        let modal_x = ((window_size.width as f32) - modal_width) / 2.0;
        let modal_y = app.get_layout_titlebar_height()
            + (((window_size.height as f32) - app.get_layout_titlebar_height() - modal_height)
                / 2.0);
        let row_position = LogicalPosition::new(
            modal_x + 120.0,
            modal_y + header_height + 108.0 + 10.0 + row_height + (row_height / 2.0),
        );

        for _ in 0..2 {
            app.window().dispatch_event(WindowEvent::PointerMoved {
                position: row_position,
            });
            app.window().dispatch_event(WindowEvent::PointerPressed {
                position: row_position,
                button: PointerEventButton::Left,
            });
            i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
            app.window().dispatch_event(WindowEvent::PointerReleased {
                position: row_position,
                button: PointerEventButton::Left,
            });
            i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(90));
        }

        assert!(!app.get_open_saved_ssh_modal_open());
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after repeated row click activation");
        assert_eq!(item.title.as_str(), "DB Admin");
    });
}

#[test]
fn launcher_picker_escape_clears_query_before_closing_modal() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_fake_sessions(&app, None);
        app.show().expect("show app window");

        create_root_ssh(&app, "Prod Bastion", "10.0.0.10");
        create_root_ssh(&app, "DB Admin", "10.0.0.24");
        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();
        app.invoke_open_saved_ssh_modal_query_changed("db".into());
        assert_eq!(app.get_open_saved_ssh_modal_query().as_str(), "db");

        let window_size = app.window().size();
        let modal_x = ((window_size.width as f32) - 720.0) / 2.0;
        let modal_y = app.get_layout_titlebar_height()
            + (((window_size.height as f32) - app.get_layout_titlebar_height() - 620.0) / 2.0);
        let search_position = LogicalPosition::new(modal_x + 120.0, modal_y + 68.0 + 49.0);
        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: search_position,
        });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: search_position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(30));
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: search_position,
            button: PointerEventButton::Left,
        });
        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Escape.into(),
        });

        assert!(app.get_open_saved_ssh_modal_open());
        assert_eq!(app.get_open_saved_ssh_modal_query().as_str(), "");

        app.window().dispatch_event(WindowEvent::KeyPressed {
            text: Key::Escape.into(),
        });
        app.window().dispatch_event(WindowEvent::KeyReleased {
            text: Key::Escape.into(),
        });

        assert!(!app.get_open_saved_ssh_modal_open());
    });
}

#[test]
fn launcher_picker_activation_restores_native_terminal_surface_rect() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
        app.show().expect("show app window");

        let ssh_id = create_root_ssh(&app, "DB Admin", "10.0.0.24");

        app.invoke_workspace_new_tab_requested();
        app.invoke_welcome_open_saved_ssh_requested();

        assert!(app.get_open_saved_ssh_modal_open());
        assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
        assert_eq!(app.get_layout_workspace_session_native_surface_width(), 0.0);
        assert_eq!(
            app.get_layout_workspace_session_native_surface_height(),
            0.0
        );

        app.invoke_open_saved_ssh_modal_asset_activated(ssh_id.into());
        settle_terminal_projection();

        assert!(!app.get_open_saved_ssh_modal_open());
        let item = app
            .get_workspace_tab_items()
            .row_data(0)
            .expect("workspace tab after picker activation");
        assert_ne!(item.tab_id.as_str(), "workspace-launcher");
        assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
        assert!(
            app.get_layout_workspace_session_native_surface_width() > 0.0
                && app.get_layout_workspace_session_native_surface_height() > 0.0,
            "saved ssh picker activation should restore the native terminal rect as soon as the launcher tab is replaced so the retained surface does not remain collapsed under a live terminal host"
        );
        assert!(
            app.get_workspace_session_surface_seqno() > 0,
            "saved ssh picker activation should stage a terminal payload together with the restored host mode so the revived geometry is backed by a real frame"
        );
    });
}

#[test]
fn settings_modal_scrollback_flows_into_newly_launched_saved_ssh_session() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("settings-modal-new-session-scrollback.json");
    let _ = std::fs::remove_file(&temp_path);

    let terminal_defaults = TerminalRuntimeDefaults::default();
    let launcher_state = Arc::new(Mutex::new(ObservingScrollbackLauncherState::default()));
    let launcher = Arc::new(ObservingScrollbackLauncher {
        state: Arc::clone(&launcher_state),
        terminal_defaults: terminal_defaults.clone(),
    });
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_terminal_defaults(
        &app,
        Some(mica_term::app::ui_preferences::UiPreferencesStore::new(
            temp_path.clone(),
        )),
        default_platform_window_effects(),
        None,
        launcher,
        Arc::new(MemoryCredentialStore::default()),
        terminal_defaults,
    );

    let ssh_id = create_root_ssh(&app, "DB Admin", "10.0.0.24");

    app.invoke_open_settings_panel_requested();
    app.invoke_settings_modal_terminal_scrollback_limit_changed(3000);
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    assert_eq!(
        launcher_state
            .lock()
            .expect("lock observing scrollback launcher state")
            .launch_scrollback_lines,
        vec![3000],
        "after changing the settings modal scrollback limit, the next saved SSH session launched through the real asset activation flow should observe that updated runtime default"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");

    let content = fs::read_to_string(&temp_path).expect("read persisted ui preferences");
    assert!(content.contains("\"terminal_scrollback_limit\": 3000"));

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn workspace_terminal_launch_captures_live_viewport_defaults_before_runtime_connects() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let terminal_defaults = TerminalRuntimeDefaults::default();
    let launcher_state = Arc::new(Mutex::new(ObservingViewportLauncherState::default()));
    let launcher = Arc::new(ObservingViewportLauncher {
        state: Arc::clone(&launcher_state),
        terminal_defaults: terminal_defaults.clone(),
        launch_delay: Duration::from_millis(80),
    });
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_terminal_defaults(
        &app,
        None,
        default_platform_window_effects(),
        None,
        launcher,
        Arc::new(MemoryCredentialStore::default()),
        terminal_defaults,
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Viewport Probe", "10.0.0.44");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let expected_rows = (app.get_layout_workspace_session_preferred_surface_height()
        / app.get_workspace_session_cell_height())
    .floor() as usize;
    let expected_cols = (app.get_layout_workspace_session_preferred_surface_width()
        / app.get_workspace_session_cell_width())
    .floor() as usize;

    wait_for_condition(Duration::from_millis(250), || {
        launcher_state
            .lock()
            .expect("lock observing viewport launcher state")
            .launch_viewports
            .len()
            == 1
    });

    let viewport = launcher_state
        .lock()
        .expect("lock observing viewport launcher state")
        .launch_viewports
        .first()
        .copied()
        .expect("captured launch viewport");

    assert_eq!(
        viewport.0,
        expected_rows.max(1),
        "new SSH launches should snapshot the host-computed viewport rows instead of booting with a stale 24-row default"
    );
    assert_eq!(
        viewport.1,
        expected_cols.max(1),
        "new SSH launches should snapshot the host-computed viewport cols instead of booting with a stale 80-col default"
    );
    assert!(
        viewport.2 > 0 && viewport.3 > 0,
        "launch-time viewport defaults should carry non-zero pixel dimensions for the initial PTY request"
    );
}

#[test]
fn launcher_picker_folder_activation_does_not_attempt_to_open_session() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(AssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> = Rc::new(RecordingAssetRepo::new(
        loaded_catalog_for_bootstrap(),
        Rc::clone(&repo_state),
        None,
    ));
    bind_with_fake_sessions(&app, Some(asset_repo));

    app.invoke_workspace_new_tab_requested();
    app.invoke_welcome_open_saved_ssh_requested();
    assert!(app.get_open_saved_ssh_modal_open());

    app.invoke_open_saved_ssh_modal_asset_activated("folder-root".into());

    assert!(app.get_open_saved_ssh_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let item = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("launcher tab after folder activation");
    assert_eq!(item.tab_id.as_str(), "workspace-launcher");
    assert_eq!(item.title.as_str(), "New Tab");
}

#[test]
fn asset_activation_restores_native_terminal_surface_rect_from_welcome() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    assert_eq!(app.get_layout_workspace_session_native_surface_width(), 0.0);
    assert_eq!(
        app.get_layout_workspace_session_native_surface_height(),
        0.0
    );

    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "activating an SSH asset from the welcome shell should restore the native terminal rect immediately so the first live terminal frame is not presented into a still-collapsed retained surface"
    );
    assert!(
        app.get_workspace_session_surface_seqno() > 0,
        "activating an SSH asset from the welcome shell should stage a terminal payload together with the host mode transition so the revived geometry is backed by a real frame"
    );
}

#[test]
fn context_menu_open_connection_restores_native_terminal_surface_rect_from_welcome() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    assert_eq!(app.get_workspace_session_host_mode().as_str(), "welcome");
    assert_eq!(app.get_layout_workspace_session_native_surface_width(), 0.0);
    assert_eq!(
        app.get_layout_workspace_session_native_surface_height(),
        0.0
    );

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    assert!(app.get_assets_context_menu_open());

    app.invoke_assets_context_menu_action_invoked("open-connection".into());
    settle_terminal_projection();

    assert!(!app.get_assets_context_menu_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "opening a connection from the assets context menu should restore the native terminal rect immediately so the first live terminal frame is not presented into a still-collapsed retained surface"
    );
    assert!(
        app.get_workspace_session_surface_seqno() > 0,
        "opening a connection from the assets context menu should stage a terminal payload together with the host mode transition so the revived geometry is backed by a real frame"
    );
}

#[test]
fn save_and_connect_persists_saved_secret_before_probe() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    flush_runtime_projection();

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
fn test_connection_action_returns_before_slow_probe_completes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = Arc::new(Mutex::new(RecordingLauncherState::default()));
    bind_with_launcher(
        &app,
        None,
        Arc::new(SlowOpeningLauncher {
            state: Arc::clone(&launcher_state),
            probe_delay: Duration::from_millis(250),
            launch_delay: Duration::from_millis(250),
        }),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());

    let started = Instant::now();
    app.invoke_asset_ssh_modal_action_requested("test".into());

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "testing an SSH connection from the modal should not block the UI while probe_connection() runs"
    );
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "busy");

    std::thread::sleep(Duration::from_millis(280));
    flush_runtime_projection();

    let launcher_state = launcher_state
        .lock()
        .expect("lock slow opening launcher state");
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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-activation");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let temp_root = sample_logging_root("ssh-open-logs-context-menu");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        data_dir: temp_root.join("data"),
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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
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
    flush_runtime_projection();

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "10.0.0.12:22");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        expected_fingerprint
    );

    app.invoke_ssh_host_key_modal_accept_requested();
    flush_runtime_projection();

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
fn accepting_unknown_host_key_retries_modal_test_without_blocking_ui() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("accept-test-nonblocking");
    let host_key = sample_public_key();
    let _ = fs::remove_file(&known_hosts_path);
    unsafe {
        std::env::set_var("MICA_TERM_KNOWN_HOSTS_PATH", &known_hosts_path);
    }

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedTofuAwareLauncher::new(
            host_key.clone(),
            Duration::from_millis(250),
        )),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());
    flush_runtime_projection();

    assert!(app.get_ssh_host_key_modal_open());

    let started = Instant::now();
    app.invoke_ssh_host_key_modal_accept_requested();

    assert!(
        started.elapsed() < Duration::from_millis(120),
        "accepting the SSH host key from the modal should queue the follow-up test probe instead of blocking the UI thread"
    );
    assert!(!app.get_ssh_host_key_modal_open());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "busy");

    std::thread::sleep(Duration::from_millis(280));
    flush_runtime_projection();

    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "success");
    assert_eq!(
        KnownHostsService::new(&known_hosts_path)
            .check("10.0.0.12", 22, &host_key)
            .expect("check trusted host after modal confirmation"),
        KnownHostCheck::Trusted
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn unknown_host_key_blocks_connection_in_workspace_timeline() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-block");
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
    flush_runtime_projection();

    assert!(
        !app.get_ssh_host_key_modal_open(),
        "workspace session host-key confirmation should stay inline instead of reusing the modal flow"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn workspace_host_key_inline_prompt_projects_decision_page_semantics() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-page-mode-decision");
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
    flush_runtime_projection();

    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    assert_eq!(
        app.get_workspace_session_connection_page_mode().as_str(),
        "decision"
    );
    assert_eq!(
        app.get_workspace_session_connection_task_title().as_str(),
        "Verify host key"
    );
    assert_eq!(
        app.get_workspace_session_connection_task_detail().as_str(),
        "The authenticity of the target host cannot be established. Please verify the host key fingerprint below before continuing.",
        "decision-mode task detail should use the more explicit reference-style host key guidance while still keeping host and fingerprint details out of the body copy"
    );
    assert!(
        !app.get_workspace_session_connection_task_detail()
            .as_str()
            .contains("SHA256:"),
        "decision-mode task detail should not duplicate the host-key fingerprint because the unified task panel renders fingerprint separately"
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn trusting_unknown_host_key_retries_connection_in_same_workspace_tab() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-trust");
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
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    assert_eq!(
        app.get_workspace_session_connection_page_mode().as_str(),
        "decision"
    );
    app.invoke_workspace_session_local_action_requested("trust-host-key".into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert_eq!(app.get_workspace_session_state().as_str(), "connected");
    assert_eq!(
        KnownHostsService::new(&known_hosts_path)
            .check("10.0.0.12", 22, &host_key)
            .expect("check trusted host after inline confirmation"),
        KnownHostCheck::Trusted
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn workspace_host_key_rejection_projects_troubleshooting_page_semantics() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-page-mode-reject");
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
    flush_runtime_projection();
    app.invoke_workspace_session_local_action_requested("reject-host-key".into());
    flush_runtime_projection();

    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_page_mode().as_str(),
        "troubleshooting"
    );
    assert_eq!(
        app.get_workspace_session_connection_task_title().as_str(),
        "Connection failed"
    );
    assert!(
        app.get_workspace_session_connection_task_detail()
            .as_str()
            .contains("Rejected unknown SSH host key"),
        "troubleshooting-mode task detail should preserve the rejection summary"
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn host_key_inline_flow_keeps_native_rect_collapsed_until_terminal_resumes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-geometry");
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
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    assert_eq!(
        app.get_layout_workspace_session_native_surface_width(),
        0.0,
        "while the host-key prompt is rendered inline inside the connection timeline, the retained native terminal rect should stay collapsed so no stale terminal surface can cover the waiting-user UI"
    );
    assert_eq!(
        app.get_layout_workspace_session_native_surface_height(),
        0.0,
        "while the host-key prompt is rendered inline inside the connection timeline, the retained native terminal rect should stay collapsed so hit-testing remains attached to the timeline host"
    );

    app.invoke_workspace_session_local_action_requested("trust-host-key".into());
    flush_runtime_projection();

    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "terminal");
    assert!(
        app.get_layout_workspace_session_native_surface_width() > 0.0
            && app.get_layout_workspace_session_native_surface_height() > 0.0,
        "after trusting the inline host-key prompt, the terminal should restore its native rect immediately when the same workspace tab resumes terminal mode"
    );
    assert_eq!(
        app.get_workspace_session_state().as_str(),
        "connected",
        "after trusting the inline host-key prompt, the same workspace tab should resume a connected terminal state even before the runtime emits the first terminal surface payload"
    );

    let _ = fs::remove_file(known_hosts_path);
    unsafe {
        std::env::remove_var("MICA_TERM_KNOWN_HOSTS_PATH");
    }
}

#[test]
fn rejecting_unknown_host_key_keeps_connection_timeline_in_same_tab() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let _env_lock = lock_known_hosts_env();
    let known_hosts_path = sample_known_hosts_path("workspace-host-key-reject");
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
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "waiting-user"
    );
    app.invoke_workspace_session_local_action_requested("reject-host-key".into());
    flush_runtime_projection();
    assert_eq!(
        app.get_workspace_session_connection_page_mode().as_str(),
        "troubleshooting"
    );

    let headline = app.get_workspace_session_connection_headline().to_string();
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert!(
        matches!(headline.as_str(), "cancelled" | "error"),
        "rejecting the host key should keep the timeline surface active with a terminal-free final state"
    );
    assert!(
        app.get_workspace_session_connection_current_detail()
            .as_str()
            .contains("Rejected unknown SSH host key"),
        "rejecting the host key should preserve a useful rejection detail in the timeline"
    );
    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
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
fn cancelling_running_connection_attempt_marks_timeline_cancelled() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(PendingConnectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    let session_id = app.get_active_workspace_session_id().to_string();

    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "connecting"
    );

    app.invoke_workspace_session_local_action_requested("cancel-connection-attempt".into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        session_id.as_str()
    );
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );
    assert_eq!(
        app.get_workspace_session_connection_headline().as_str(),
        "cancelled"
    );
}

#[test]
fn runtime_events_refresh_workspace_terminal_projection_after_opening_saved_asset() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn workspace_terminal_command_decorations_toggle_reprojects_existing_surface() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_text_input("pwd".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(
        app.get_workspace_session_command_blocks().row_count() > 0,
        "an interactive prompt line should project at least one running command block while decorations stay enabled"
    );
    assert_eq!(
        app.get_workspace_session_overview_markers().row_count(),
        0,
        "overview markers should now default off until the user explicitly opts into extra transcript chrome"
    );

    app.invoke_settings_modal_terminal_command_decorations_enabled_changed(false);

    assert_eq!(app.get_workspace_session_command_blocks().row_count(), 0);
    assert_eq!(app.get_workspace_session_overview_markers().row_count(), 0);

    app.invoke_settings_modal_terminal_command_decorations_enabled_changed(true);

    assert!(
        app.get_workspace_session_command_blocks().row_count() > 0,
        "re-enabling decorations should re-project the existing workspace surface without waiting for new terminal output"
    );
    assert_eq!(
        app.get_workspace_session_overview_markers().row_count(),
        0,
        "re-enabling decorations alone should not force overview markers back on"
    );
}

#[test]
fn workspace_terminal_overview_markers_toggle_reprojects_existing_surface() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_text_input("pwd".into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(
        app.get_workspace_session_command_blocks().row_count() > 0,
        "command gutter decorations should be present before toggling overview markers"
    );
    assert_eq!(
        app.get_workspace_session_overview_markers().row_count(),
        0,
        "overview markers should start off by default until the user explicitly enables them"
    );

    app.invoke_settings_modal_terminal_overview_markers_enabled_changed(true);

    assert!(
        app.get_workspace_session_overview_markers().row_count() > 0,
        "enabling overview markers should project them from the current workspace surface immediately"
    );

    app.invoke_settings_modal_terminal_overview_markers_enabled_changed(false);

    assert!(
        app.get_workspace_session_command_blocks().row_count() > 0,
        "disabling overview markers should keep command gutter decorations visible"
    );
    assert_eq!(app.get_workspace_session_overview_markers().row_count(), 0);

    app.invoke_settings_modal_terminal_overview_markers_enabled_changed(true);

    assert!(
        app.get_workspace_session_command_blocks().row_count() > 0,
        "re-enabling overview markers should leave existing command blocks intact"
    );
    assert!(
        app.get_workspace_session_overview_markers().row_count() > 0,
        "re-enabling overview markers should re-project them from the current workspace surface"
    );
}

#[test]
fn workspace_terminal_search_query_reprojects_visible_match_count() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_text_input("https://example.com/docs".into());
    wait_for_condition(Duration::from_millis(250), || {
        app.get_workspace_session_visible_lines()
            .iter()
            .any(|line| line.contains("https://example.com/docs"))
    });

    app.invoke_workspace_session_search_open_requested();
    app.invoke_workspace_session_search_query_changed("example".into());
    wait_for_condition(Duration::from_millis(250), || {
        app.get_workspace_session_search_match_count() > 0
    });

    assert!(app.get_workspace_session_search_open());
    assert_eq!(app.get_workspace_session_search_query().as_str(), "example");
    assert!(
        app.get_workspace_session_search_match_count() > 0,
        "entering a visible search query should project live match counts from the current terminal viewport"
    );

    app.invoke_workspace_session_search_query_changed("no-match-token".into());
    wait_for_condition(Duration::from_millis(250), || {
        app.get_workspace_session_search_match_count() == 0
    });

    assert_eq!(app.get_workspace_session_search_match_count(), 0);

    app.invoke_workspace_session_search_close_requested();
    assert!(!app.get_workspace_session_search_open());
}

#[test]
fn workspace_terminal_paste_callback_updates_active_session_surface() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn workspace_terminal_multiline_paste_warning_defers_unprotected_paste_until_confirmed() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(
            &app,
            None,
            Arc::new(PasteWarningProjectionLauncher {
                bracketed_paste_enabled: false,
            }),
        );

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        i_slint_backend_selector::with_platform(|platform| {
            platform
                .set_clipboard_text("pwd\necho hi", slint::platform::Clipboard::DefaultClipboard);
            Ok(())
        })
        .expect("seed multiline clipboard");

        app.invoke_workspace_session_paste_requested();

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        assert!(app.get_workspace_paste_warning_modal_open());
        assert_eq!(app.get_workspace_paste_warning_line_count(), 2);
        assert_eq!(app.get_workspace_session_surface_seqno(), 1);
        assert_eq!(app.get_workspace_session_visible_lines().row_count(), 1);

        app.invoke_workspace_paste_warning_confirm_requested();

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        assert!(!app.get_workspace_paste_warning_modal_open());
        assert_eq!(app.get_workspace_session_surface_seqno(), 2);
        assert_eq!(app.get_workspace_session_visible_lines().row_count(), 2);
        assert_eq!(
            app.get_workspace_session_visible_lines()
                .row_data(1)
                .unwrap()
                .as_str(),
            "paste pwd\necho hi"
        );
    });
}

#[test]
fn workspace_terminal_multiline_paste_warning_skips_bracketed_paste_sessions() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(
            &app,
            None,
            Arc::new(PasteWarningProjectionLauncher {
                bracketed_paste_enabled: true,
            }),
        );

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        i_slint_backend_selector::with_platform(|platform| {
            platform
                .set_clipboard_text("pwd\necho hi", slint::platform::Clipboard::DefaultClipboard);
            Ok(())
        })
        .expect("seed multiline clipboard");

        app.invoke_workspace_session_paste_requested();

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        assert!(!app.get_workspace_paste_warning_modal_open());
        assert_eq!(app.get_workspace_session_surface_seqno(), 2);
        assert_eq!(app.get_workspace_session_visible_lines().row_count(), 2);
    });
}

#[test]
fn workspace_terminal_long_multiline_paste_opens_editor_and_sends_edited_text() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let app = AppWindow::new().unwrap();
        bind_with_launcher(
            &app,
            None,
            Arc::new(PasteWarningProjectionLauncher {
                bracketed_paste_enabled: true,
            }),
        );

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(
                "one\ntwo\nthree\nfour",
                slint::platform::Clipboard::DefaultClipboard,
            );
            Ok(())
        })
        .expect("seed long multiline clipboard");

        app.invoke_workspace_session_paste_requested();

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        assert!(app.get_workspace_paste_warning_modal_open());
        assert!(app.get_workspace_paste_warning_editor_mode());
        assert_eq!(
            app.get_workspace_paste_warning_text(),
            "one\ntwo\nthree\nfour"
        );
        assert_eq!(app.get_workspace_session_surface_seqno(), 1);

        app.set_workspace_paste_warning_text("one\ntwo\nfour".into());
        app.invoke_workspace_paste_warning_confirm_requested();

        std::thread::sleep(Duration::from_millis(20));
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        slint::platform::update_timers_and_animations();

        assert!(!app.get_workspace_paste_warning_modal_open());
        assert_eq!(app.get_workspace_session_surface_seqno(), 2);
        assert_eq!(
            app.get_workspace_session_visible_lines()
                .row_data(1)
                .unwrap()
                .as_str(),
            "paste one\ntwo\nfour"
        );
    });
}

#[test]
fn workspace_terminal_right_click_paste_warning_restores_immediate_text_input_after_confirm() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let state = KeyboardMatrixState::default();
        let app = AppWindow::new().unwrap();
        bind_with_launcher(
            &app,
            None,
            Arc::new(
                KeyboardMatrixLauncher::new(state.clone()).with_bracketed_paste_enabled(false),
            ),
        );

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());
        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));

        settle_terminal_projection();
        focus_workspace_terminal(&app);
        settle_terminal_projection();

        i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(
                "line 1\nline 2\nline 3",
                slint::platform::Clipboard::DefaultClipboard,
            );
            Ok(())
        })
        .expect("seed multiline clipboard");

        let menu_origin = terminal_interaction_position(&app);
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: menu_origin,
        });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: menu_origin,
            button: PointerEventButton::Right,
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: menu_origin,
            button: PointerEventButton::Right,
        });
        settle_terminal_projection();

        let paste_row_position = LogicalPosition::new(menu_origin.x + 20.0, menu_origin.y + 60.0);
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: paste_row_position,
        });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: paste_row_position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: paste_row_position,
            button: PointerEventButton::Left,
        });
        settle_terminal_projection();

        assert!(
            app.get_workspace_paste_warning_modal_open(),
            "right-click Paste should route multiline clipboard content through the shared review modal"
        );

        let window_size = app.window().size();
        let titlebar_height = app.get_layout_titlebar_height();
        let modal_width = 460.0;
        let modal_height = 296.0;
        let modal_origin = LogicalPosition::new(
            (window_size.width as f32 - modal_width) / 2.0,
            titlebar_height + ((window_size.height as f32 - titlebar_height - modal_height) / 2.0),
        );
        let confirm_position = LogicalPosition::new(modal_origin.x + 380.0, modal_origin.y + 259.0);
        app.window().dispatch_event(WindowEvent::PointerMoved {
            position: confirm_position,
        });
        app.window().dispatch_event(WindowEvent::PointerPressed {
            position: confirm_position,
            button: PointerEventButton::Left,
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
        app.window().dispatch_event(WindowEvent::PointerReleased {
            position: confirm_position,
            button: PointerEventButton::Left,
        });
        settle_terminal_projection();

        assert!(
            !app.get_workspace_paste_warning_modal_open(),
            "clicking Paste in review mode should confirm the paste and close the modal"
        );

        dispatch_text_key_chord(&app, "z", false, false, false);
        settle_terminal_projection();

        assert_eq!(
            state.take_text_inputs(),
            vec!["z".to_string()],
            "the terminal should accept immediate typing after a right-click long-paste confirm instead of dropping input until focus recovers later"
        );
    });
}

#[test]
fn workspace_terminal_ctrl_shift_v_editor_warning_accepts_enter_to_confirm() {
    run_with_large_test_stack(|| {
        let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

        let state = KeyboardMatrixState::default();
        let app = AppWindow::new().unwrap();
        bind_with_launcher(
            &app,
            None,
            Arc::new(
                KeyboardMatrixLauncher::new(state.clone()).with_bracketed_paste_enabled(false),
            ),
        );

        let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
        app.invoke_asset_activated(ssh_id.into());

        settle_terminal_projection();
        focus_workspace_terminal(&app);
        settle_terminal_projection();

        i_slint_backend_selector::with_platform(|platform| {
            platform.set_clipboard_text(
                "line 1\nline 2\nline 3\nline 4",
                slint::platform::Clipboard::DefaultClipboard,
            );
            Ok(())
        })
        .expect("seed multiline clipboard for editor warning");

        dispatch_text_key_chord(&app, "V", true, true, false);
        settle_terminal_projection();

        assert!(
            app.get_workspace_paste_warning_modal_open(),
            "Ctrl+Shift+V should open the paste review modal for editor-mode multiline content"
        );
        assert!(
            app.get_workspace_paste_warning_editor_mode(),
            "four-line paste should open the editable long-paste review mode"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "the modal should defer the terminal paste until the user confirms"
        );

        app.window()
            .dispatch_event(WindowEvent::WindowActiveChanged(true));
        app.window()
            .dispatch_event(WindowEvent::KeyPressed { text: "\n".into() });
        app.window()
            .dispatch_event(WindowEvent::KeyReleased { text: "\n".into() });
        settle_terminal_projection();

        assert!(
            !app.get_workspace_paste_warning_modal_open(),
            "Enter in review mode should confirm the paste and close the modal (terminal key leaks observed: {})",
            state.key_input_count()
        );
        assert_eq!(
            state.take_paste_inputs(),
            vec!["line 1\nline 2\nline 3\nline 4".to_string()],
            "Enter should trigger the same terminal paste send path as clicking the Paste button"
        );
    });
}

#[test]
fn workspace_terminal_single_line_trailing_newline_pastes_without_warning() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(PasteWarningProjectionLauncher {
            bracketed_paste_enabled: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("pwd\n", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("seed single-line clipboard");

    app.invoke_workspace_session_paste_requested();

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    assert!(!app.get_workspace_paste_warning_modal_open());
    assert_eq!(app.get_workspace_session_surface_seqno(), 2);
    assert_eq!(
        app.get_workspace_session_visible_lines()
            .row_data(1)
            .unwrap()
            .as_str(),
        "paste pwd\n"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_c_copies_selected_text_to_clipboard() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    select_terminal_welcome_span(&app);

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
fn workspace_terminal_selection_keeps_native_frame_contract_active() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let before = app.get_workspace_session_native_frame_token();
    let before_surface_seqno = app.get_workspace_session_surface_seqno();

    select_terminal_welcome_span(&app);
    settle_terminal_projection();

    let after = app.get_workspace_session_native_frame_token();
    let after_surface_seqno = app.get_workspace_session_surface_seqno();
    let render_mode = app.get_workspace_session_render_mode();

    assert!(
        app.get_workspace_session_selection_active(),
        "pointer drag should activate terminal selection state"
    );
    assert_eq!(
        render_mode.as_str(),
        "native",
        "mainline bootstrap should keep the retained native surface active while selection changes once the new terminal subsystem becomes the default path"
    );
    assert!(
        before > 0,
        "native composition should keep a retained native frame token active before selection"
    );
    assert!(
        after > 0,
        "native composition should keep a retained native frame token active after selection"
    );
    assert!(
        before_surface_seqno > 0 && after_surface_seqno > 0,
        "selection should keep the staged terminal surface alive while native composition handles the visible frame"
    );
}

#[test]
fn workspace_terminal_padding_drag_does_not_start_selection_inside_the_grid() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    drag_terminal_padding_into_grid(&app);
    settle_terminal_projection();

    assert!(
        !app.get_workspace_session_selection_active(),
        "pointer drags that start in the terminal padding should not be clamped into column 0 because that makes the selection model feel offset from the rendered grid"
    );
}

#[test]
fn workspace_terminal_half_cell_drag_selects_a_single_ascii_cell() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    drag_within_first_terminal_cell(&app);
    settle_terminal_projection();

    assert!(
        app.get_workspace_session_selection_active(),
        "dragging from the left half to the right half of the same visible cell should still select that cell instead of collapsing to an empty range"
    );
    assert_eq!(app.get_workspace_session_selection_start_row(), 0);
    assert_eq!(app.get_workspace_session_selection_end_row(), 0);
    assert_eq!(app.get_workspace_session_selection_start_col(), 0);
    assert_eq!(app.get_workspace_session_selection_end_col(), 1);
}

#[test]
fn workspace_terminal_selection_rows_stay_bound_to_buffer_when_scrolling() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    select_terminal_welcome_span(&app);
    settle_terminal_projection();

    assert!(app.get_workspace_session_selection_active());
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        3,
        "test fixture should start scrolled away from the bottom so selection rows must account for the current scrollback origin"
    );
    assert_eq!(
        app.get_workspace_session_selection_start_row(),
        5,
        "selection rows should be stored against the scrollback buffer instead of raw viewport-local rows"
    );
    assert_eq!(app.get_workspace_session_selection_end_row(), 5);

    let position = terminal_interaction_position(&app);
    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 60.0,
    });
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 6);
    assert!(app.get_workspace_session_selection_active());
    assert_eq!(
        app.get_workspace_session_selection_start_row(),
        5,
        "scrolling the viewport should not rewrite the selected buffer row to a new viewport-local coordinate"
    );
    assert_eq!(app.get_workspace_session_selection_end_row(), 5);
}

#[test]
fn workspace_terminal_copy_selection_reads_full_scrollback_buffer_text() {
    run_on_large_stack(
        "workspace_terminal_copy_selection_reads_full_scrollback_buffer_text",
        workspace_terminal_copy_selection_reads_full_scrollback_buffer_text_body,
    );
}

fn workspace_terminal_copy_selection_reads_full_scrollback_buffer_text_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollbackCopyLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_copy_selection_requested(0, 0, 3, 20);

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard");

    assert_eq!(
        copied.as_deref(),
        Some("zero\none\ntwo\nthree"),
        "copy should read the selected scrollback buffer rows even when the selection starts above the visible viewport"
    );
}

#[test]
fn workspace_terminal_entering_alt_screen_clears_existing_selection() {
    run_on_large_stack(
        "workspace_terminal_entering_alt_screen_clears_existing_selection",
        workspace_terminal_entering_alt_screen_clears_existing_selection_body,
    );
}

fn workspace_terminal_entering_alt_screen_clears_existing_selection_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let state = SelectionBoundaryState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(SelectionBoundaryLauncher {
            state: state.clone(),
        }),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);
    select_terminal_welcome_span(&app);
    settle_terminal_projection();

    assert!(app.get_workspace_session_selection_active());

    state.emit_alt_screen_surface();
    settle_terminal_projection();

    assert!(
        !app.get_workspace_session_selection_active(),
        "switching into alternate-screen content should invalidate any host-side shell selection so stale primary-buffer ranges do not paint over TUI frames"
    );
    assert_eq!(app.get_workspace_session_selection_start_row(), -1);
    assert_eq!(app.get_workspace_session_selection_start_col(), -1);
    assert_eq!(app.get_workspace_session_selection_end_row(), -1);
    assert_eq!(app.get_workspace_session_selection_end_col(), -1);
}

#[test]
fn workspace_terminal_surface_resize_clears_existing_selection() {
    run_on_large_stack(
        "workspace_terminal_surface_resize_clears_existing_selection",
        workspace_terminal_surface_resize_clears_existing_selection_body,
    );
}

fn workspace_terminal_surface_resize_clears_existing_selection_body() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let state = SelectionBoundaryState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(SelectionBoundaryLauncher {
            state: state.clone(),
        }),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);
    select_terminal_welcome_span(&app);
    settle_terminal_projection();

    assert!(app.get_workspace_session_selection_active());

    app.invoke_workspace_session_resize_requested(12, 8);
    settle_terminal_projection();

    assert!(
        !app.get_workspace_session_selection_active(),
        "changing the terminal grid geometry should clear the existing host-side selection because the old buffer row/column span is no longer guaranteed to map safely after reflow"
    );
    assert_eq!(app.get_workspace_session_selection_start_row(), -1);
    assert_eq!(app.get_workspace_session_selection_start_col(), -1);
    assert_eq!(app.get_workspace_session_selection_end_row(), -1);
    assert_eq!(app.get_workspace_session_selection_end_col(), -1);
}

#[test]
fn opening_right_panel_clamps_terminal_surface_width_before_pty_resize_roundtrip() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(WideProjectionLauncher { cols: 140 }));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let before_workspace_width = app.get_layout_main_workspace_width();
    let before = app.get_layout_workspace_session_native_surface_width();

    app.invoke_open_sftp_panel_requested();

    assert!(
        app.get_effective_show_right_panel(),
        "opening the SFTP panel should immediately toggle the right-panel layout state"
    );
    assert!(
        app.get_layout_main_workspace_width() < before_workspace_width,
        "opening the SFTP panel should immediately shrink the main workspace width budget"
    );
    assert!(
        app.get_layout_workspace_session_native_surface_width() < before,
        "terminal surface width should clamp to the shrunken content viewport immediately instead of waiting for a later PTY resize roundtrip, otherwise the software path flashes and hit-testing drifts under the right panel"
    );
}

#[test]
fn opening_right_panel_updates_live_terminal_viewport_defaults_immediately() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let terminal_defaults = TerminalRuntimeDefaults::default();
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_terminal_defaults(
        &app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(WideProjectionLauncher { cols: 140 }),
        Arc::new(MemoryCredentialStore::default()),
        terminal_defaults.clone(),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let before = (
        terminal_defaults.viewport_rows(),
        terminal_defaults.viewport_cols(),
        terminal_defaults.viewport_pixel_width(),
    );

    app.invoke_open_sftp_panel_requested();
    wait_for_condition(Duration::from_millis(250), || {
        terminal_defaults.viewport_cols() < before.1
    });

    let after = (
        terminal_defaults.viewport_rows(),
        terminal_defaults.viewport_cols(),
        terminal_defaults.viewport_pixel_width(),
    );

    let expected_after = (
        (app.get_layout_workspace_session_preferred_surface_height()
            / app.get_workspace_session_cell_height())
        .floor() as usize,
        (app.get_layout_workspace_session_preferred_surface_width()
            / app.get_workspace_session_cell_width())
        .floor() as usize,
        app.get_layout_workspace_session_preferred_surface_width()
            .round() as u32,
    );
    assert_eq!(
        after.0, expected_after.0,
        "opening the right panel should update the live viewport row contract to the latest preferred host rows"
    );
    assert!(
        after.1 < before.1,
        "opening the right panel should immediately shrink the live terminal cols contract so later PTY resizes do not lag one layout behind"
    );
    assert_eq!(
        after.1, expected_after.1,
        "opening the right panel should align the live viewport cols contract with the latest preferred host cols"
    );
    assert!(
        after.2 < before.2,
        "opening the right panel should immediately shrink the pixel viewport width stored for future PTY requests"
    );
    assert_eq!(
        after.2, expected_after.2,
        "opening the right panel should update the stored pixel viewport width to the preferred host width"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_c_copies_selected_text_when_backend_emits_etx() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    select_terminal_welcome_span(&app);

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
    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: "\u{3}".into(),
    });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: "\u{3}".into(),
    });
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
        "Ctrl+Shift+C should still copy when the backend emits ETX instead of a literal C"
    );
}

#[test]
fn workspace_terminal_plain_ctrl_a_forwards_prefix_key_without_selecting_all() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_workspace_session_selection_active(),
        "terminal selection should start inactive before testing Ctrl+A forwarding"
    );

    app.window().dispatch_event(WindowEvent::KeyPressed {
        text: Key::Control.into(),
    });
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: "a".into() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text: "a".into() });
    app.window().dispatch_event(WindowEvent::KeyReleased {
        text: Key::Control.into(),
    });
    settle_terminal_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(
        visible_lines
            .row_data(1)
            .expect("forwarded Ctrl+A line")
            .as_str(),
        "$ a",
        "plain Ctrl+A should stay in the terminal input stream so screen/tmux prefix chords still work"
    );
    assert!(
        !app.get_workspace_session_selection_active(),
        "plain Ctrl+A should not trigger a local select-all gesture inside the terminal host"
    );
}

#[test]
fn workspace_terminal_ctrl_key_matrix_forwards_common_shell_shortcuts() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = ['a', 'c', 'v', 'z', 'd', 'l'];
    for key in cases {
        dispatch_text_key_chord(&app, &key.to_string(), true, false, false);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::character(key, false, true, false)],
            "Ctrl+{key} should be forwarded to the remote terminal as a control chord"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "Ctrl+{key} should not be converted into a local paste action"
        );
    }
}

#[test]
fn workspace_terminal_ctrl_shift_shortcut_matrix_keeps_local_contract() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);
    select_terminal_welcome_span(&app);

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .expect("clear clipboard");

    dispatch_text_key_chord(&app, "C", true, true, false);
    settle_terminal_projection();

    let copied = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard")
    .expect("clipboard text after Ctrl+Shift+C");
    assert!(
        copied.contains("welcome"),
        "Ctrl+Shift+C should still copy the active terminal selection"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+C should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+C should not touch the remote paste channel"
    );

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(
            "printf 'matrix paste'",
            slint::platform::Clipboard::DefaultClipboard,
        );
        Ok(())
    })
    .expect("seed clipboard");

    dispatch_text_key_chord(&app, "V", true, true, false);
    settle_terminal_projection();

    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+V should not forward a remote key chord"
    );
    assert_eq!(
        state.take_paste_inputs(),
        vec!["printf 'matrix paste'".to_string()],
        "Ctrl+Shift+V should use the terminal paste channel"
    );

    for key in ["T", "W", "P", "F"] {
        dispatch_text_key_chord(&app, key, true, true, false);
        settle_terminal_projection();
        assert!(
            state.take_key_inputs().is_empty(),
            "reserved Ctrl+Shift+{key} should stay local and never forward to the remote terminal"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "reserved Ctrl+Shift+{key} should not hit the terminal paste channel"
        );
    }
}

#[test]
fn workspace_terminal_ctrl_shift_t_opens_new_tab_from_active_terminal_asset() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    let first_session_id = app.get_active_workspace_session_id().to_string();

    dispatch_text_key_chord(&app, "T", true, true, false);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_ne!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id,
        "Ctrl+Shift+T should create and activate a fresh workspace tab"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+T should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+T should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_w_closes_active_terminal_tab_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    settle_terminal_projection();
    let first_session_id = app.get_active_workspace_session_id().to_string();

    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_ne!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id
    );

    dispatch_text_key_chord(&app, "W", true, true, false);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        first_session_id,
        "Ctrl+Shift+W should close the active workspace tab and fall back to the previous one"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+W should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+W should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_f_expands_asset_search_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_asset_search_expanded(),
        "asset search should start collapsed"
    );

    dispatch_text_key_chord(&app, "F", true, true, false);
    settle_terminal_projection();

    assert!(
        app.get_asset_search_expanded(),
        "Ctrl+Shift+F should expand the local asset search"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+F should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+F should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_m_toggles_focus_mode_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_assets_sidebar());
    assert!(app.get_show_right_panel());

    dispatch_text_key_chord(&app, "M", true, true, false);
    settle_terminal_projection();

    assert!(
        app.get_workspace_focus_mode(),
        "Ctrl+Shift+M should enter the local workspace focus mode"
    );
    assert!(
        !app.get_show_assets_sidebar() && !app.get_show_right_panel(),
        "Ctrl+Shift+M should hide both side regions together"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+M should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+M should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_ctrl_shift_p_opens_global_menu_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert!(
        !app.get_show_global_menu(),
        "global menu should start closed"
    );

    dispatch_text_key_chord(&app, "P", true, true, false);
    settle_terminal_projection();
    assert!(
        app.get_show_global_menu(),
        "Ctrl+Shift+P should open the local global menu"
    );
    assert!(
        state.take_key_inputs().is_empty(),
        "Ctrl+Shift+P should stay local and must not forward a remote key chord"
    );
    assert!(
        state.take_paste_inputs().is_empty(),
        "Ctrl+Shift+P should not hit the terminal paste channel"
    );
}

#[test]
fn workspace_terminal_alt_arrow_matrix_forwards_modifier_aware_named_keys() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = [
        ("left", "left"),
        ("right", "right"),
        ("up", "up"),
        ("down", "down"),
    ];
    for (named_key, expected_name) in cases {
        dispatch_named_key_chord(&app, named_key, false, false, true);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::named(expected_name, true, false, false)],
            "Alt+{named_key} should preserve the alt modifier in the remote terminal event"
        );
    }
}

#[test]
fn workspace_terminal_named_key_matrix_forwards_navigation_keys() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    let cases = [("home", "home"), ("end", "end"), ("insert", "insert")];
    for (named_key, expected_name) in cases {
        dispatch_named_key_chord(&app, named_key, false, false, false);
        wait_for_condition(Duration::from_secs(1), || {
            let inputs = state
                .key_inputs
                .lock()
                .expect("lock keyboard matrix key inputs");
            inputs.len() == 1
                && inputs[0] == TerminalKeyEvent::named(expected_name, false, false, false)
        });
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::named(expected_name, false, false, false)],
            "{named_key} should forward as a named terminal key event"
        );
    }
}

#[test]
fn workspace_terminal_function_key_matrix_forwards_f1_through_f24() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    for number in 1u8..=24 {
        dispatch_function_key(&app, number);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::function(number, false, false, false)],
            "F{number} should forward to the remote terminal function-key path"
        );
    }
}

#[test]
fn workspace_terminal_shift_page_shortcuts_scroll_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);

    dispatch_named_key_chord(&app, "page-up", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "Shift+PageUp should move local scrollback toward the top"
    );

    dispatch_named_key_chord(&app, "page-down", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        0,
        "Shift+PageDown should move local scrollback back toward the bottom"
    );
}

#[test]
fn workspace_terminal_shift_home_end_shortcuts_jump_scrollback_locally() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 3);

    dispatch_named_key_chord(&app, "home", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        8,
        "Shift+Home should jump local scrollback to the top"
    );
    assert!(!app.get_workspace_session_viewport_at_bottom());

    dispatch_named_key_chord(&app, "end", false, true, false);
    settle_terminal_projection();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        0,
        "Shift+End should jump local scrollback back to the bottom"
    );
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_mouse_input_callback_forwards_events_when_runtime_owns_the_pointer() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = LinkInteractionLauncherState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(LinkInteractionLauncher {
            state: launcher_state.clone(),
            line: "welcome to mica-term",
            alternate_screen_active: false,
            mouse_grabbed: true,
        }),
    );

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

    let forwarded = launcher_state
        .forwarded_mouse_inputs
        .lock()
        .expect("lock forwarded mouse inputs");
    assert_eq!(forwarded.len(), 1);
    assert_eq!(
        forwarded[0].kind,
        TerminalMouseEventKind::Down,
        "mouse-grabbed sessions should still forward workspace mouse events through the runtime callback"
    );
    assert_eq!(
        forwarded[0].button,
        TerminalMouseButton::Left,
        "mouse-grabbed sessions should preserve the original button when forwarding workspace mouse input"
    );
}

#[test]
fn workspace_terminal_ctrl_click_opens_trimmed_url_without_forwarding_mouse_input() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = LinkInteractionLauncherState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(LinkInteractionLauncher {
            state: launcher_state.clone(),
            line: "see https://example.com).",
            alternate_screen_active: false,
            mouse_grabbed: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let _hook_lock = URL_OPEN_HOOK_LOCK.lock().expect("lock URL open hook");
    let opened = Arc::new(Mutex::new(Vec::<String>::new()));
    let opened_ref = Arc::clone(&opened);
    let _guard = install_url_open_handler_for_test(move |url| {
        opened_ref
            .lock()
            .expect("lock opened urls")
            .push(url.to_string());
        Ok(())
    });

    app.invoke_workspace_session_mouse_input(
        "down".into(),
        "left".into(),
        0,
        8,
        false,
        true,
        false,
    );
    app.invoke_workspace_session_mouse_input("up".into(), "left".into(), 0, 8, false, true, false);

    assert_eq!(
        opened.lock().expect("lock opened urls").as_slice(),
        ["https://example.com"],
        "Ctrl+click should open the trimmed HTTP(S) URL instead of including trailing punctuation from terminal output"
    );
    assert!(
        launcher_state
            .forwarded_mouse_inputs
            .lock()
            .expect("lock forwarded mouse inputs")
            .is_empty(),
        "host-owned Ctrl+click link opens should short-circuit before forwarding remote mouse input"
    );
}

#[test]
fn workspace_terminal_ctrl_drag_does_not_open_link_or_forward_mouse_input() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = LinkInteractionLauncherState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(LinkInteractionLauncher {
            state: launcher_state.clone(),
            line: "see https://example.com",
            alternate_screen_active: false,
            mouse_grabbed: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let _hook_lock = URL_OPEN_HOOK_LOCK.lock().expect("lock URL open hook");
    let opened = Arc::new(Mutex::new(Vec::<String>::new()));
    let opened_ref = Arc::clone(&opened);
    let _guard = install_url_open_handler_for_test(move |url| {
        opened_ref
            .lock()
            .expect("lock opened urls")
            .push(url.to_string());
        Ok(())
    });

    app.invoke_workspace_session_mouse_input(
        "down".into(),
        "left".into(),
        0,
        8,
        false,
        true,
        false,
    );
    app.invoke_workspace_session_mouse_input(
        "move".into(),
        "none".into(),
        0,
        12,
        false,
        true,
        false,
    );
    app.invoke_workspace_session_mouse_input("up".into(), "left".into(), 0, 12, false, true, false);

    assert!(
        opened.lock().expect("lock opened urls").is_empty(),
        "selection-like drags must cancel any pending Ctrl+click link candidate instead of opening a URL on release"
    );
    assert!(
        launcher_state
            .forwarded_mouse_inputs
            .lock()
            .expect("lock forwarded mouse inputs")
            .is_empty(),
        "local link drag suppression should stay on the host path instead of leaking drag events into the remote PTY"
    );
}

#[test]
fn workspace_terminal_alt_screen_ctrl_click_does_not_open_link_and_still_forwards_mouse_input() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let launcher_state = LinkInteractionLauncherState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(LinkInteractionLauncher {
            state: launcher_state.clone(),
            line: "see https://example.com",
            alternate_screen_active: true,
            mouse_grabbed: false,
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    let _hook_lock = URL_OPEN_HOOK_LOCK.lock().expect("lock URL open hook");
    let opened = Arc::new(Mutex::new(Vec::<String>::new()));
    let opened_ref = Arc::clone(&opened);
    let _guard = install_url_open_handler_for_test(move |url| {
        opened_ref
            .lock()
            .expect("lock opened urls")
            .push(url.to_string());
        Ok(())
    });

    app.invoke_workspace_session_mouse_input(
        "down".into(),
        "left".into(),
        0,
        8,
        false,
        true,
        false,
    );
    app.invoke_workspace_session_mouse_input("up".into(), "left".into(), 0, 8, false, true, false);

    assert!(
        opened.lock().expect("lock opened urls").is_empty(),
        "alternate-screen content should suppress host link opening even when the hovered token looks like a URL"
    );
    assert_eq!(
        launcher_state
            .forwarded_mouse_inputs
            .lock()
            .expect("lock forwarded mouse inputs")
            .len(),
        2,
        "alternate-screen sessions should keep mouse ownership so Ctrl+click still forwards to the remote terminal"
    );
}

#[test]
fn bootstrap_projects_terminal_scrollback_state_into_window_properties() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn single_line_scrollback_does_not_shrink_workspace_terminal_width() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = ScrollProjectionState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(CountingScrollProjectionLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    state.emit_surface_with_viewport(0, 0);
    settle_terminal_projection();

    let width_without_scrollback = app.get_layout_workspace_session_native_surface_width();
    let cols_without_scrollback = app.get_layout_workspace_session_preferred_cols();

    state.emit_surface_with_viewport(0, 1);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_max_offset_lines(), 1);
    assert_eq!(
        app.get_layout_workspace_session_native_surface_width(),
        width_without_scrollback,
        "a transient one-line scrollback should not reserve scrollbar gutter width because that feeds back into PTY cols and makes bottom-anchored TUIs reflow"
    );
    assert_eq!(
        app.get_layout_workspace_session_preferred_cols(),
        cols_without_scrollback,
        "a transient one-line scrollback should keep the preferred PTY cols stable"
    );

    state.emit_surface_with_viewport(0, 2);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_max_offset_lines(), 2);
    assert!(
        app.get_layout_workspace_session_native_surface_width() < width_without_scrollback,
        "once scrollback grows beyond one line the scrollbar gutter may reserve width again"
    );
    assert!(
        app.get_layout_workspace_session_preferred_cols() < cols_without_scrollback,
        "once scrollback grows beyond one line the host may reduce preferred PTY cols to make room for the real scrollbar gutter"
    );
}

#[test]
fn bootstrap_projects_terminal_canvas_palette_into_window_properties() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
        0xfff7_f9fc
    );
    assert_eq!(
        app.get_workspace_session_cursor_bg().as_argb_encoded(),
        0xff4b_5058
    );
}

#[test]
fn bootstrap_projects_dark_terminal_cursor_as_light_grey() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_cursor_bg().as_argb_encoded(),
        0xff00_0000 | preset_for_theme_mode(ThemeMode::Dark).cursor_bg
    );
}

#[test]
fn toggling_theme_without_active_terminal_surface_refreshes_fallback_palette() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("bootstrap-no-surface-theme-toggle.json");
    let store = mica_term::app::ui_preferences::UiPreferencesStore::new(temp_path.clone());
    store
        .save(&mica_term::app::ui_preferences::UiPreferences {
            theme_mode: ThemeMode::Light,
            ..mica_term::app::ui_preferences::UiPreferences::default()
        })
        .expect("save light theme prefs");

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        &app,
        Some(store),
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
    );

    let light = preset_for_theme_mode(ThemeMode::Light);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | light.foreground,
        "without an active terminal surface bootstrap should project the light fallback terminal foreground from the Catppuccin preset"
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xff00_0000 | light.background,
        "without an active terminal surface bootstrap should project the light fallback terminal background from the Catppuccin preset"
    );

    app.invoke_toggle_theme_mode_requested();

    let dark = preset_for_theme_mode(ThemeMode::Dark);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | dark.foreground,
        "toggling theme without an active terminal surface should refresh the fallback terminal foreground instead of leaving the previous preset latched"
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xff00_0000 | dark.background,
        "toggling theme without an active terminal surface should refresh the fallback terminal background instead of leaving the previous preset latched"
    );

    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn no_surface_terminal_projection_uses_catppuccin_defaults_and_tracks_theme_toggle() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let dark_preset = preset_for_theme_mode(ThemeMode::Dark);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | dark_preset.foreground
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xff00_0000 | dark_preset.background
    );

    app.invoke_toggle_theme_mode_requested();

    let light_preset = preset_for_theme_mode(ThemeMode::Light);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | light_preset.foreground,
        "when no terminal surface is active the fallback terminal projection should still use the Catppuccin light foreground after a theme toggle"
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xff00_0000 | light_preset.background,
        "when no terminal surface is active the fallback terminal projection should still use the Catppuccin light background after a theme toggle"
    );
}

#[test]
fn changing_theme_variant_from_settings_reprojects_fallback_terminal_palette() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let premium = preset_for_theme(ThemeMode::Dark, ThemeVariant::PremiumDefault);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | premium.foreground
    );

    app.invoke_open_settings_panel_requested();
    app.invoke_settings_modal_theme_variant_changed("legacy_hacker_green".into());

    let legacy = preset_for_theme(ThemeMode::Dark, ThemeVariant::LegacyHackerGreen);
    assert_eq!(
        app.get_workspace_session_default_fg().as_argb_encoded(),
        0xff00_0000 | legacy.foreground,
        "changing theme variant from settings should refresh the fallback terminal foreground without needing an active surface"
    );
    assert_eq!(
        app.get_workspace_session_default_bg().as_argb_encoded(),
        0xff00_0000 | legacy.background,
        "changing theme variant from settings should refresh the fallback terminal background too"
    );
}

#[test]
fn bootstrap_projects_terminal_shell_chrome_contract_from_theme_preset() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let app_window_source = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane_source =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        bootstrap_source.contains("set_workspace_session_scrollbar_thumb(")
            && bootstrap_source.contains("set_workspace_session_scrollbar_thumb_active("),
        "bootstrap should publish terminal scrollbar thumb colors through the workspace session contract so fallback and live shell chrome stay on the same Catppuccin preset source"
    );
    assert!(
        !bootstrap_source.contains("set_workspace_session_jump_to_latest"),
        "bootstrap should stop publishing removed jump-to-latest pill colors"
    );
    assert!(
        app_window_source.contains("workspace-session-scrollbar-thumb")
            && app_window_source.contains("workspace-session-scrollbar-thumb-active")
            && !app_window_source.contains("workspace-session-jump-to-latest"),
        "AppWindow should surface terminal shell chrome colors as first-class workspace session properties so Rust can project the Catppuccin preset into the shell host"
    );
    assert!(
        workspace_pane_source.contains("workspace-session-scrollbar-thumb")
            && workspace_pane_source.contains("workspace-session-scrollbar-thumb-active")
            && !workspace_pane_source.contains("workspace-session-jump-to-latest"),
        "WorkspacePane should thread the terminal shell chrome properties through to TerminalSessionHost instead of letting that chrome drift back to generic shell tokens"
    );
}

#[test]
fn bootstrap_clears_terminal_renderer_caches_when_no_surface_remains() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let host_source =
        fs::read_to_string("src/app/terminal_renderer/host.rs").expect("read renderer host");

    assert!(
        host_source.contains("pub fn clear_transient_caches(&mut self)"),
        "terminal renderer host should expose a clear_transient_caches hook so bootstrap can drop retained presenter caches when no terminal surface remains"
    );
    assert!(
        bootstrap_source.contains("clear_workspace_terminal_transient_caches("),
        "bootstrap should route terminal cache shrink through a lifecycle helper so close-driven and idle-driven shrink paths can share cache diagnostics"
    );
    assert!(
        bootstrap_source.contains("rearm_workspace_terminal_no_surface_idle_shrink("),
        "bootstrap should re-arm the no-surface idle timer when the active workspace surface disappears so the delayed cleanup path can still run later"
    );
}

#[test]
fn bootstrap_tracks_no_surface_idle_before_terminal_cache_shrink() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("const WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS: u64 ="),
        "bootstrap should define a dedicated idle threshold before shrinking terminal caches so active session transitions do not immediately throw away caches needed for fast reconnect or tab switching"
    );
    assert!(
        bootstrap_source.contains("workspace_terminal_no_surface_since"),
        "bootstrap should track how long the workspace has been without an active terminal surface before firing the idle cache shrink path"
    );
    assert!(
        bootstrap_source.contains("release_workspace_terminal_renderer_resources();"),
        "bootstrap should release retained renderer resources from the delayed no-surface shrink path after the idle threshold elapses"
    );
    assert!(
        bootstrap_source.contains("purge_workspace_backend_memory(window);"),
        "bootstrap should request a Slint backend purge from the delayed no-surface shrink path so renderer-global caches can be reclaimed before falling back to a working-set trim"
    );
}

#[test]
fn bootstrap_tracks_active_surface_idle_before_terminal_cache_shrink() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        bootstrap_source.contains("update_workspace_terminal_active_idle_cache_shrink("),
        "bootstrap should route visible-surface idle shrink through a dedicated helper so the active-window path can reset independently from the no-surface shrink lifecycle"
    );
    assert!(
        bootstrap_source.contains("workspace_terminal_active_surface_since"),
        "bootstrap should track when the visible terminal surface last changed so stable seqno and viewport windows can trigger an active idle cache shrink"
    );
    assert!(
        bootstrap_source.contains("settings_modal_terminal_active_idle_shrink_enabled()"),
        "bootstrap should gate the active idle shrink path behind the persisted settings toggle from the titlebar settings modal"
    );
    assert!(
        bootstrap_source
            .contains("WorkspaceTerminalActiveSurfaceFingerprint::from_surface(active_surface)"),
        "bootstrap should fingerprint the visible surface before the idle shrink fires so only unchanged frames trigger the delayed cache clear"
    );
}

#[test]
fn no_surface_terminal_shell_chrome_tracks_catppuccin_theme_toggle() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(InteractiveProjectionLauncher));

    let dark_preset = preset_for_theme_mode(ThemeMode::Dark);
    assert_eq!(
        app.get_workspace_session_scrollbar_thumb()
            .as_argb_encoded(),
        0xff00_0000 | rgb_tuple_to_hex(dark_preset.scrollbar_thumb)
    );
    assert_eq!(
        app.get_workspace_session_scrollbar_thumb_active()
            .as_argb_encoded(),
        0xff00_0000 | rgb_tuple_to_hex(dark_preset.scrollbar_thumb_active)
    );

    app.invoke_toggle_theme_mode_requested();

    let light_preset = preset_for_theme_mode(ThemeMode::Light);
    assert_eq!(
        app.get_workspace_session_scrollbar_thumb()
            .as_argb_encoded(),
        0xff00_0000 | rgb_tuple_to_hex(light_preset.scrollbar_thumb),
        "without an active terminal surface the terminal scrollbar thumb should still refresh to the light Catppuccin shell chrome palette after a theme toggle"
    );
    assert_eq!(
        app.get_workspace_session_scrollbar_thumb_active()
            .as_argb_encoded(),
        0xff00_0000 | rgb_tuple_to_hex(light_preset.scrollbar_thumb_active),
        "without an active terminal surface the terminal scrollbar hover thumb should still refresh to the light Catppuccin shell chrome palette after a theme toggle"
    );
}

#[test]
fn terminal_input_callback_snaps_scrolled_session_back_to_latest_surface() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
fn ctrl_shift_non_reserved_letter_shortcuts_forward_remote_terminal_input() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let state = KeyboardMatrixState::default();
    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(KeyboardMatrixLauncher::new(state.clone())),
    );
    app.show().expect("show app window");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();
    focus_workspace_terminal(&app);

    for key in ['A', 'B', 'K', 'L', 'N', 'O', 'R'] {
        dispatch_text_key_chord(&app, &key.to_string(), true, true, false);
        settle_terminal_projection();
        assert_eq!(
            state.take_key_inputs(),
            vec![TerminalKeyEvent::character(key, false, true, true)],
            "Ctrl+Shift+{key} should forward to the remote terminal once it is no longer reserved locally"
        );
        assert!(
            state.take_paste_inputs().is_empty(),
            "Ctrl+Shift+{key} should not hit the terminal paste channel"
        );
    }
}

#[test]
fn workspace_terminal_scroll_callbacks_update_active_session_surface() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    wait_for_condition(Duration::from_secs(1), || {
        app.get_workspace_session_viewport_offset_lines() == 8
    });
    assert!(!app.get_workspace_session_viewport_at_bottom());

    app.invoke_workspace_session_scroll_thumb_drag_requested(0.0);
    wait_for_condition(Duration::from_secs(1), || {
        app.get_workspace_session_viewport_offset_lines() == 0
    });
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_scroll_thumb_drag_coalesces_runtime_scroll_updates() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let state = ScrollProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(CountingScrollProjectionLauncher::new(state.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_thumb_drag_requested(1.0);
    app.invoke_workspace_session_scroll_thumb_drag_requested(0.0);

    assert_eq!(
        state.scroll_call_count(),
        0,
        "continuous thumb drag should defer runtime scroll projection until the debounce window elapses"
    );
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        3,
        "the visible viewport should remain stable until the coalesced thumb-drag refresh executes"
    );

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(8));
    slint::platform::update_timers_and_animations();

    assert_eq!(
        state.scroll_call_count(),
        1,
        "continuous thumb drag should collapse to one runtime scroll update that applies the latest ratio"
    );
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_scroll_thumb_drag_refreshes_within_single_digit_milliseconds() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let state = ScrollProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(CountingScrollProjectionLauncher::new(state.clone())),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_thumb_drag_requested(0.0);

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(7));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        state.scroll_call_count(),
        0,
        "thumb-drag projection refresh should still be deferred before the tighter low-latency budget expires"
    );

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        state.scroll_call_count(),
        1,
        "thumb-drag projection refresh should land once roughly 8ms have elapsed so local scrollback feels less delayed"
    );
}

#[test]
fn workspace_terminal_scroll_jump_refreshes_faster_than_thumb_drag() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(&app, None, Arc::new(ScrollProjectionLauncher));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());

    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();

    app.invoke_workspace_session_scroll_jump_requested(0.0);

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(3));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        3,
        "scroll-jump projection should still be deferred before the tighter wheel/jump budget expires"
    );

    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(1));
    slint::platform::update_timers_and_animations();
    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        0,
        "wheel and scroll-jump updates should project within roughly 4ms so keyboard-like navigation feels more immediate than thumb drag coalescing"
    );
}

#[test]
fn workspace_terminal_pointer_wheel_scrolls_proportionally_for_partial_notches() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
        6,
        "half-wheel motion should move half a notch worth of terminal lines immediately"
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
        "a second half-notch should continue the proportional scrollback, capped by the current viewport max offset"
    );
}

#[test]
fn workspace_terminal_small_pointer_wheel_delta_scrolls_gradually() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
        delta_y: 20.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        4,
        "small wheel deltas should start moving the viewport immediately instead of waiting for a full notch"
    );

    app.window().dispatch_event(WindowEvent::PointerScrolled {
        position,
        delta_x: 0.0,
        delta_y: 20.0,
    });
    settle_terminal_projection();

    assert_eq!(
        app.get_workspace_session_viewport_offset_lines(),
        5,
        "successive small wheel deltas should continue accumulating into smooth line-by-line scrollback"
    );
}

#[test]
fn workspace_terminal_scroll_jump_returns_viewport_to_latest() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let follow_state = FollowProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FollowProjectionLauncher {
            state: follow_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    settle_terminal_projection();
    assert!(!app.get_workspace_session_viewport_at_bottom());

    follow_state.emit_remote_output(3);
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 11);
    assert!(!app.get_workspace_session_viewport_at_bottom());

    app.invoke_workspace_session_scroll_jump_requested(0.0);
    settle_terminal_projection();
    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn workspace_terminal_live_input_resumes_follow_from_scrollback() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let follow_state = FollowProjectionState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(FollowProjectionLauncher {
            state: follow_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    settle_terminal_projection();

    app.invoke_workspace_session_scroll_jump_requested(1.0);
    settle_terminal_projection();
    follow_state.emit_remote_output(2);
    settle_terminal_projection();

    assert!(!app.get_workspace_session_viewport_at_bottom());

    app.invoke_workspace_session_text_input("a".into());
    settle_terminal_projection();

    assert_eq!(app.get_workspace_session_viewport_offset_lines(), 0);
    assert!(app.get_workspace_session_viewport_at_bottom());
}

#[test]
fn async_launch_failure_projects_error_tab_after_projection_timer_ticks() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

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
    assert_eq!(app.get_workspace_session_state().as_str(), "connecting");
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress"
    );

    std::thread::sleep(Duration::from_millis(80));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(100));
    slint::platform::update_timers_and_animations();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_state().as_str(), "error");
    assert_eq!(
        app.get_workspace_session_host_mode().as_str(),
        "connection-progress",
        "retry-capable launch failures should stay on the redesigned connection sheet instead of dropping into the generic session-error surface"
    );
    assert_eq!(
        app.get_workspace_session_connection_page_mode().as_str(),
        "troubleshooting",
        "retry-capable launch failures should project troubleshooting mode on the connection sheet"
    );
    assert_eq!(
        app.get_workspace_session_connection_task_title().as_str(),
        "Connection failed",
        "launch failures without a deeper active step should still expose a stable troubleshooting title"
    );
    assert!(
        app.get_workspace_session_connection_task_detail()
            .as_str()
            .contains("missing SSH password secret"),
        "retry-capable launch failures should keep the underlying failure summary inside the connection sheet"
    );
    assert_eq!(
        app.get_workspace_session_error_detail().as_str(),
        "missing SSH password secret for `SSH Connection 1`"
    );
}

#[test]
fn sftp_navigation_callbacks_update_projected_path_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app, None);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_sftp_panel_path_submitted("/srv/app".into());
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(app.get_sftp_panel_follow_mode().as_str(), "manual-browse");

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");

    app.invoke_sftp_panel_back_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");

    app.invoke_sftp_panel_forward_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");

    app.invoke_sftp_panel_up_requested();
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
}

#[test]
fn sftp_navigation_toolbar_triggers_real_directory_reads() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();
    app.invoke_sftp_panel_back_requested();
    flush_runtime_projection();
    app.invoke_sftp_panel_forward_requested();
    flush_runtime_projection();
    app.invoke_sftp_panel_up_requested();
    flush_runtime_projection();

    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app".to_string(),
        ]
    );
}

#[test]
fn opening_sftp_reads_the_active_session_directory_instead_of_staying_connecting() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(app.get_sftp_panel_items().row_count(), 2);
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .name
            .as_str(),
        ".."
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .type_label
            .as_str(),
        "Up"
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("sftp row")
            .name
            .as_str(),
        "logs"
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("sftp row")
            .type_label
            .as_str(),
        "Folder"
    );
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string()]
    );
}

#[test]
fn opening_sftp_without_initial_cwd_falls_back_to_root_until_follow_cwd_arrives() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedCwdRecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/");
    if app.get_sftp_panel_mode().as_str() != "ready" {
        assert_eq!(app.get_sftp_panel_mode().as_str(), "loading");
        flush_runtime_projection();
    }
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(sftp_state.take_read_dir_calls(), vec!["/".to_string()]);

    sftp_state.emit_cwd("/srv/app");
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    if app.get_sftp_panel_mode().as_str() != "ready" {
        assert_eq!(app.get_sftp_panel_mode().as_str(), "loading");
        flush_runtime_projection();
    }
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string()]
    );
}

#[test]
fn refresh_and_path_submit_trigger_real_directory_reads() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/releases");
    assert_eq!(app.get_sftp_panel_items().row_count(), 2);
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .name
            .as_str(),
        ".."
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("sftp row")
            .type_label
            .as_str(),
        "Up"
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("sftp row")
            .name
            .as_str(),
        "release.tar.gz"
    );
    let release_row = app.get_sftp_panel_items().row_data(1).expect("release row");
    assert_eq!(release_row.kind.as_str(), "archive");
    assert_eq!(release_row.type_label.as_str(), "Archive");
    assert_eq!(release_row.size_label.as_str(), "14 KB");
    assert!(
        release_row
            .meta_label
            .as_str()
            .starts_with("Archive · 14 KB"),
        "quick browser rows should project a compact prebuilt meta label instead of rebuilding metadata fragments inside Slint"
    );
    assert!(
        !release_row.modified_label.is_empty(),
        "release row should expose a real modified timestamp"
    );

    app.invoke_sftp_panel_refresh_requested();
    flush_runtime_projection();

    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app/releases".to_string(),
        ]
    );
}

#[test]
fn pointer_clicking_an_sftp_row_selects_it() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let (window_width, window_height) = default_window_size();
    app.window()
        .set_size(slint::PhysicalSize::new(window_width, window_height));

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    let position = LogicalPosition::new(
        (window_width - 392) as f32 + 64.0,
        48.0 + 90.0 + 44.0 + 22.0,
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
    flush_runtime_projection();

    assert!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("logs row")
            .selected,
        "left-clicking a visible quick-browser row should select it again"
    );
}

#[test]
fn sftp_context_menu_refresh_dispatches_a_real_directory_reload() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string()]
    );

    app.invoke_sftp_panel_context_menu_requested("".into(), "sftp-blank".into(), 64.0, 96.0);
    app.invoke_assets_context_menu_action_invoked("refresh-sftp".into());
    flush_runtime_projection();

    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string()],
        "refresh from the SFTP blank-area context menu should trigger the same remote read path as the toolbar button"
    );
}

#[test]
fn revisiting_a_previous_remote_path_keeps_its_cached_snapshot_visible_while_refreshing() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedReadRecordingSftpLauncher {
            state: sftp_state.clone(),
            read_delay_by_path: Arc::new(BTreeMap::from([(
                "/srv/app".to_string(),
                Duration::from_millis(180),
            )])),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    std::thread::sleep(Duration::from_millis(220));
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("ready /srv/app row")
            .name
            .as_str(),
        "logs"
    );

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("ready /srv/app/releases row")
            .name
            .as_str(),
        "release.tar.gz"
    );

    app.invoke_sftp_panel_path_submitted("/srv/app".into());

    assert_eq!(app.get_sftp_panel_mode().as_str(), "loading");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("cached /srv/app row while loading")
            .name
            .as_str(),
        "logs",
        "revisiting a previously loaded path should immediately show its cached snapshot while the background refresh is still in flight"
    );

    std::thread::sleep(Duration::from_millis(220));
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("ready /srv/app row after refresh")
            .name
            .as_str(),
        "logs"
    );
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app".to_string(),
        ]
    );
}

#[test]
fn switching_tabs_keeps_terminal_selection_fast_and_leaves_the_previous_quick_browser_snapshot_visible_until_refresh_completes()
 {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedReadRecordingSftpLauncher {
            state: sftp_state.clone(),
            read_delay_by_path: Arc::new(BTreeMap::from([(
                "/srv/db".to_string(),
                Duration::from_millis(180),
            )])),
        }),
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let prod_id = find_console_asset_id(&app, "Prod Bastion");
    app.invoke_asset_activated(prod_id.into());
    flush_runtime_projection();
    let prod_session_id = app.get_active_workspace_session_id().to_string();

    create_root_ssh(&app, "DB Replica", "10.0.0.24");
    let db_id = find_console_asset_id(&app, "DB Replica");
    app.invoke_asset_activated(db_id.into());
    flush_runtime_projection();
    let db_session_id = app.get_active_workspace_session_id().to_string();

    app.invoke_workspace_tab_selected(prod_session_id.clone().into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("prod quick browser row")
            .name
            .as_str(),
        "logs"
    );

    let started = Instant::now();
    app.invoke_workspace_tab_selected(db_session_id.clone().into());

    assert!(
        started.elapsed() < Duration::from_millis(80),
        "switching the active SSH tab should not wait for the right-side SFTP browser refresh"
    );
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        db_session_id.as_str(),
        "terminal focus should switch immediately even while the quick browser is still refreshing in the background"
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("previous quick browser row while db refresh is pending")
            .name
            .as_str(),
        "logs",
        "until the target tab refresh completes, the quick browser should keep showing the previous snapshot instead of blanking or blocking"
    );

    flush_runtime_projection();
    std::thread::sleep(Duration::from_millis(220));
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/db");
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("db quick browser row after refresh")
            .name
            .as_str(),
        "backup.sql"
    );
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string(), "/srv/db".to_string()]
    );
}

#[test]
fn parent_directory_row_navigates_up_and_stays_first_in_the_sftp_table() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();

    let parent_row = app.get_sftp_panel_items().row_data(0).expect("parent row");
    assert_eq!(parent_row.id.as_str(), "__sftp_parent__");
    assert_eq!(parent_row.name.as_str(), "..");
    assert_eq!(parent_row.kind.as_str(), "parent-directory");
    assert_eq!(parent_row.type_label.as_str(), "Up");

    app.invoke_sftp_panel_item_activated("__sftp_parent__".into(), "parent-directory".into());
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(0)
            .expect("parent row after navigate up")
            .name
            .as_str(),
        ".."
    );
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/releases".to_string(),
            "/srv/app".to_string(),
        ]
    );
}

#[test]
fn opening_sftp_with_a_slow_backend_returns_before_directory_loading_finishes() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedReadRecordingSftpLauncher {
            state: sftp_state.clone(),
            read_delay_by_path: Arc::new(BTreeMap::from([(
                "/srv/app".to_string(),
                Duration::from_millis(180),
            )])),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    let started = Instant::now();
    app.invoke_open_sftp_panel_requested();

    assert!(
        started.elapsed() < Duration::from_millis(80),
        "opening the quick browser should no longer block on a slow remote directory read"
    );
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app");
    assert_ne!(
        app.get_sftp_panel_mode().as_str(),
        "ready",
        "slow background reads should leave the quick browser in a pending state until the async result arrives"
    );

    std::thread::sleep(Duration::from_millis(220));
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_items().row_count(), 2);
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string()]
    );
}

#[test]
fn latest_sftp_directory_request_wins_when_slower_results_finish_last() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(DelayedReadRecordingSftpLauncher {
            state: sftp_state.clone(),
            read_delay_by_path: Arc::new(BTreeMap::from([
                ("/srv/app/releases".to_string(), Duration::from_millis(180)),
                ("/srv/app/logs".to_string(), Duration::from_millis(10)),
            ])),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");

    let started = Instant::now();
    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    app.invoke_sftp_panel_path_submitted("/srv/app/logs".into());

    assert!(
        started.elapsed() < Duration::from_millis(80),
        "submitting two quick browser navigations should not block on the first slow read before queuing the second request"
    );
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/logs");

    std::thread::sleep(Duration::from_millis(40));
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_mode().as_str(), "ready");
    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/logs");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("latest log row")
            .name
            .as_str(),
        "app.log"
    );

    std::thread::sleep(Duration::from_millis(220));
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/logs");
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("latest log row after stale completion")
            .name
            .as_str(),
        "app.log",
        "a stale slower directory response should not overwrite the newest quick browser request"
    );
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec![
            "/srv/app".to_string(),
            "/srv/app/logs".to_string(),
            "/srv/app/releases".to_string(),
        ]
    );
}

#[test]
fn activating_sftp_rows_navigates_directories_and_downloads_files_for_local_open() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    sftp_state.set_remote_file(
        "/srv/app/releases/release.tar.gz",
        b"port=22
"
        .to_vec(),
    );
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_item_activated("entry-logs".into(), "directory".into());
    flush_runtime_projection();

    assert_eq!(app.get_sftp_panel_path().as_str(), "/srv/app/logs");
    assert_eq!(
        sftp_state.take_read_dir_calls(),
        vec!["/srv/app".to_string(), "/srv/app/logs".to_string()]
    );

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    flush_runtime_projection();

    app.invoke_sftp_panel_item_activated("entry-release".into(), "file".into());
    flush_runtime_projection();

    assert!(
        !app.get_sftp_remote_file_modal_open(),
        "default Open should download and hand off locally instead of surfacing the legacy remote editor modal"
    );
    assert_eq!(
        sftp_state.take_download_file_calls(),
        vec!["/srv/app/releases/release.tar.gz".to_string()]
    );
    assert!(
        sftp_state.take_upload_file_calls().is_empty(),
        "default Open should not synchronously upload anything back"
    );
}

#[test]
fn sftp_new_folder_dispatches_backend_mkdir_instead_of_local_push() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_context_menu_requested("".into(), "sftp-blank".into(), 64.0, 96.0);
    app.invoke_assets_context_menu_action_invoked("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("shared".into());
    app.invoke_confirm_asset_modal_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        sftp_state
            .mkdir_calls
            .lock()
            .expect("lock sftp mkdir calls")
            .len()
            == 1
    });

    assert_eq!(
        sftp_state.take_mkdir_calls(),
        vec!["/srv/app/shared".to_string()]
    );
    let item_names = (0..app.get_sftp_panel_items().row_count())
        .filter_map(|index| app.get_sftp_panel_items().row_data(index))
        .map(|row| row.name.to_string())
        .collect::<Vec<_>>();
    assert!(
        !item_names.iter().any(|name| name == "shared"),
        "quick-browser new-folder should stop inserting a local-only row before the backend refresh completes"
    );
}

#[test]
fn sftp_new_file_dispatches_backend_empty_upload_instead_of_local_push() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_context_menu_requested("".into(), "sftp-blank".into(), 64.0, 96.0);
    app.invoke_assets_context_menu_action_invoked("new-file".into());
    assert_eq!(
        app.get_asset_modal_kind().as_str(),
        "new-file",
        "new-file should open a dedicated creation modal instead of falling through to a placeholder action"
    );

    app.invoke_asset_folder_modal_name_changed("notes.txt".into());
    app.invoke_confirm_asset_modal_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        sftp_state
            .upload_file_calls
            .lock()
            .expect("lock sftp upload file calls")
            .len()
            == 1
    });

    assert_eq!(
        sftp_state.take_upload_file_calls(),
        vec![("/srv/app/notes.txt".to_string(), Vec::new())]
    );
    let item_names = (0..app.get_sftp_panel_items().row_count())
        .filter_map(|index| app.get_sftp_panel_items().row_data(index))
        .map(|row| row.name.to_string())
        .collect::<Vec<_>>();
    assert!(
        !item_names.iter().any(|name| name == "notes.txt"),
        "quick-browser new-file should wait for the backend refresh instead of pushing a local-only row"
    );
}

#[test]
fn sftp_rename_dispatches_backend_rename_instead_of_local_relabel() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.invoke_sftp_panel_context_menu_requested(
        "entry-release".into(),
        "sftp-file".into(),
        80.0,
        120.0,
    );
    app.invoke_assets_context_menu_action_invoked("rename-sftp-entry".into());
    app.invoke_asset_rename_modal_name_changed("release-v2.tar.gz".into());
    app.invoke_confirm_asset_rename_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        sftp_state
            .rename_calls
            .lock()
            .expect("lock sftp rename calls")
            .len()
            == 1
    });

    assert_eq!(
        sftp_state.take_rename_calls(),
        vec![(
            "/srv/app/releases/release.tar.gz".to_string(),
            "/srv/app/releases/release-v2.tar.gz".to_string(),
        )]
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("release row")
            .name
            .as_str(),
        "release.tar.gz",
        "quick-browser rename should wait for the remote refresh instead of relabeling the visible row immediately"
    );
}

#[test]
fn sftp_delete_dispatches_backend_remove_and_requires_confirmation() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.invoke_sftp_panel_context_menu_requested(
        "entry-release".into(),
        "sftp-file".into(),
        80.0,
        120.0,
    );
    app.invoke_assets_context_menu_action_invoked("delete-sftp-entry".into());
    assert!(
        sftp_state
            .remove_file_calls
            .lock()
            .expect("lock sftp remove-file calls")
            .is_empty(),
        "delete should still wait for explicit confirmation"
    );

    app.invoke_confirm_delete_asset_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        sftp_state
            .remove_file_calls
            .lock()
            .expect("lock sftp remove-file calls")
            .len()
            == 1
    });

    assert_eq!(
        sftp_state.take_remove_file_calls(),
        vec!["/srv/app/releases/release.tar.gz".to_string()]
    );
    assert!(
        sftp_state.take_remove_dir_calls().is_empty(),
        "deleting a file row should not call the directory removal backend"
    );
    assert_eq!(
        app.get_sftp_panel_items()
            .row_data(1)
            .expect("release row")
            .name
            .as_str(),
        "release.tar.gz",
        "quick-browser delete should stop pruning the projected row locally before the remote refresh lands"
    );
}

#[test]
fn external_sftp_drop_callbacks_toggle_overlay_and_queue_background_uploads() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("sftp-drop-upload");
    let upload_path = temp_root.join("release.env");
    fs::create_dir_all(temp_root.as_path()).expect("create drop upload temp root");
    fs::write(&upload_path, b"PORT=22\n").expect("write local drop source");

    app.invoke_sftp_panel_external_drop_hover_changed(true);
    flush_runtime_projection();
    assert!(
        !app.get_sftp_panel_drop_target_active(),
        "drag hover should stay inactive until the quick browser has a ready SFTP target"
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_external_drop_hover_changed(true);
    flush_runtime_projection();
    assert!(
        app.get_sftp_panel_drop_target_active(),
        "drag hover should expose the quick-browser drop overlay once the active SFTP path is ready"
    );

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        !sftp_state
            .upload_file_calls
            .lock()
            .expect("lock sftp upload file calls")
            .is_empty()
    });

    let upload_calls = sftp_state.take_upload_file_calls();
    assert_eq!(
        upload_calls,
        vec![("/srv/app/release.env".to_string(), b"PORT=22\n".to_vec())]
    );
    assert!(
        !app.get_sftp_panel_drop_target_active(),
        "drop completion should clear the hover overlay so the quick browser returns to normal browsing chrome"
    );
}

#[test]
fn transfer_center_receives_live_rows_from_background_sftp_transfers() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-upload");
    let upload_path = temp_root.join("release.env");
    fs::create_dir_all(temp_root.as_path()).expect("create transfer-center temp root");
    fs::write(&upload_path, b"PORT=22\n").expect("write transfer-center upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_transfer_center_items().row_count() > 0
    });

    let rows = app.get_transfer_center_items();
    let first_row = rows.row_data(0).expect("first transfer center row");
    assert!(
        first_row.title.contains("release.env"),
        "transfer center should surface the queued/uploaded file name"
    );
    assert!(
        first_row.detail.contains("Upload"),
        "transfer center rows should carry the transfer direction summary"
    );
}

#[test]
fn transfer_center_conflict_rows_expose_inline_error_summary_and_tooltip() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-conflict-row");
    let upload_path = temp_root.join("logs");
    fs::create_dir_all(temp_root.as_path()).expect("create transfer-center conflict temp root");
    fs::write(&upload_path, b"pretend archive bytes").expect("write conflict upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_transfer_center_items().row_count() > 0
    });

    let row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("transfer-center conflict row");
    assert_eq!(row.status_label.as_str(), "Conflict");
    assert!(
        row.show_error,
        "failed/conflict transfer rows should opt into a dedicated inline error line"
    );
    assert!(
        row.error_summary.as_str().contains("already exists"),
        "the inline transfer-center error summary should explain the actionable conflict reason"
    );
    assert!(
        row.error_tooltip.as_str().contains("already exists"),
        "the transfer-center tooltip payload should preserve the full conflict text for hover display"
    );
}

#[test]
fn transfer_center_filters_toggle_failed_completed_and_all_views() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-filter-tabs");
    let upload_path = temp_root.join("release.env");
    let conflict_path = temp_root.join("logs");
    fs::create_dir_all(temp_root.as_path()).expect("create transfer-center filter temp root");
    fs::write(&upload_path, b"PORT=22\n").expect("write transfer-center completed upload source");
    fs::write(&conflict_path, b"pretend archive bytes")
        .expect("write transfer-center conflict upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        conflict_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let statuses = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| row.status_label.to_string())
            .collect::<Vec<_>>();
        statuses.iter().any(|status| status == "Completed")
            && statuses.iter().any(|status| status == "Conflict")
    });

    app.invoke_transfer_center_filter_toggle_requested("failed".into());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() == 1
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Conflict")
                .unwrap_or(false)
    });

    app.invoke_transfer_center_filter_toggle_requested("completed".into());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() == 1
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Completed")
                .unwrap_or(false)
    });

    app.invoke_transfer_center_filter_toggle_requested("completed".into());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_transfer_center_items().row_count() >= 2
    });
}

#[test]
fn failed_filter_includes_failed_and_conflict_rows() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    sftp_state.fail_upload_attempts("/srv/app/release.env", 1);
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-failed-filter");
    let failed_path = temp_root.join("release.env");
    let conflict_path = temp_root.join("logs");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center failed filter temp root");
    fs::write(&failed_path, b"PORT=22\n").expect("write transfer-center failed upload source");
    fs::write(&conflict_path, b"pretend archive bytes")
        .expect("write transfer-center conflict upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        failed_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .any(|row| row.title.as_str() == "release.env" && row.status_label.as_str() == "Failed")
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        conflict_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let statuses = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| (row.title.to_string(), row.status_label.to_string()))
            .collect::<Vec<_>>();
        statuses
            .iter()
            .any(|(title, status)| title == "release.env" && status == "Failed")
            && statuses
                .iter()
                .any(|(title, status)| title == "logs" && status == "Conflict")
    });

    app.invoke_transfer_center_filter_toggle_requested("failed".into());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let statuses = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| row.status_label.to_string())
            .collect::<Vec<_>>();
        statuses.len() == 2
            && statuses.iter().any(|status| status == "Failed")
            && statuses.iter().any(|status| status == "Conflict")
    });
}

#[test]
fn clear_completed_only_removes_completed_rows() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-clear-completed");
    let completed_path = temp_root.join("release.env");
    let conflict_path = temp_root.join("logs");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center clear-completed temp root");
    fs::write(&completed_path, b"PORT=22\n")
        .expect("write transfer-center completed upload source");
    fs::write(&conflict_path, b"pretend archive bytes")
        .expect("write transfer-center conflict upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        completed_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        conflict_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let statuses = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| (row.title.to_string(), row.status_label.to_string()))
            .collect::<Vec<_>>();
        statuses
            .iter()
            .any(|(title, status)| title == "release.env" && status == "Completed")
            && statuses
                .iter()
                .any(|(title, status)| title == "logs" && status == "Conflict")
    });

    app.invoke_transfer_center_clear_completed_requested();
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let entries = (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .map(|row| (row.title.to_string(), row.status_label.to_string()))
            .collect::<Vec<_>>();
        !entries
            .iter()
            .any(|(title, status)| title == "release.env" && status == "Completed")
            && entries
                .iter()
                .any(|(title, status)| title == "logs" && status == "Conflict")
    });
}

#[test]
fn transfer_center_failed_rows_expose_retry_and_retry_real_transfer() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    sftp_state.fail_upload_attempts("/srv/app/release.env", 1);
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-retry");
    let upload_path = temp_root.join("release.env");
    fs::create_dir_all(temp_root.as_path()).expect("create transfer-center retry temp root");
    fs::write(&upload_path, b"PORT=22\n").expect("write transfer-center retry upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Failed")
                .unwrap_or(false)
    });

    let failed_row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("failed transfer-center row");
    assert!(
        failed_row.can_retry,
        "failed transfer rows should expose a real retry affordance"
    );

    app.invoke_transfer_center_retry_requested(failed_row.id.clone());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let upload_calls = sftp_state
            .upload_file_calls
            .lock()
            .expect("lock sftp retry upload calls")
            .len();
        let rows = app.get_transfer_center_items();
        upload_calls >= 2
            && rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Completed")
                .unwrap_or(false)
    });
}

#[test]
fn bootstrap_loads_interrupted_transfer_tasks_from_store() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app_root = sample_vault_runtime_root("transfer-bootstrap-load");
    let _ = fs::remove_dir_all(&app_root);
    let store = Arc::new(RedbTransferStore::new(app_root.join("data")));
    store
        .save_tasks(&[sample_persisted_interrupted_download_task(
            app_root.as_path(),
            true,
        )])
        .expect("persist interrupted transfer task");

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_transfer_store(
        &app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        Arc::new(MemoryCredentialStore::default()),
        Arc::clone(&store),
    );
    flush_runtime_projection();

    assert_eq!(app.get_transfer_queue_total(), 1);
    let row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("restored transfer-center row");
    assert_eq!(row.status_label.as_str(), "Interrupted");
    assert!(row.can_retry);
}

#[test]
fn bootstrap_marks_invalid_resume_tasks_as_restart_required() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app_root = sample_vault_runtime_root("transfer-bootstrap-restart-required");
    let _ = fs::remove_dir_all(&app_root);
    let store = Arc::new(RedbTransferStore::new(app_root.join("data")));
    store
        .save_tasks(&[sample_persisted_interrupted_download_task(
            app_root.as_path(),
            false,
        )])
        .expect("persist invalid interrupted transfer task");

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_transfer_store(
        &app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        Arc::new(MemoryCredentialStore::default()),
        Arc::clone(&store),
    );
    flush_runtime_projection();

    assert_eq!(app.get_transfer_queue_total(), 1);
    let row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("restored transfer-center row");
    assert!(
        row.status_label.as_str().contains("Restart"),
        "invalid persisted checkpoints should degrade to an explicit restart-required state"
    );
    assert!(row.can_retry);
}

#[test]
fn transfer_center_open_actions_use_native_shell_and_reveal_helpers() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let local_open_source =
        fs::read_to_string("src/app/sftp/local_open.rs").expect("read local open helper");

    assert!(
        bootstrap_sftp.contains("reveal_path_locally(local_path.as_path())"),
        "transfer-center folder actions should route through a reveal helper so file downloads can select the finished artifact inside the native file manager"
    );
    assert!(
        local_open_source.contains("pub fn reveal_path_locally(path: &Path) -> Result<()>")
            && local_open_source.contains("org.freedesktop.FileManager1")
            && local_open_source.contains("ShowItems")
            && local_open_source.contains("shell-open"),
        "the local-open helper should distinguish native shell-open from native reveal handling, including a Linux FileManager1 reveal path before directory fallback"
    );
}

#[test]
fn transfer_center_windows_open_helpers_use_shell_api_contracts() {
    let local_open_source =
        fs::read_to_string("src/app/sftp/local_open.rs").expect("read local open helper");

    assert!(
        local_open_source.contains("ShellExecuteW")
            && local_open_source.contains("SHOpenFolderAndSelectItems")
            && local_open_source.contains("ILCreateFromPathW")
            && local_open_source.contains("CoInitializeEx")
            && local_open_source.contains("CoUninitialize"),
        "Windows local-open helpers should route through Shell APIs instead of command-shell fallbacks so downloaded files and reveal actions use native Explorer semantics"
    );
    assert!(
        !local_open_source.contains("Command::new(\"cmd\")")
            && !local_open_source.contains("Command::new(\"explorer\")"),
        "Windows local-open helpers should stop spawning cmd/explorer directly once the Shell API path exists"
    );
}

#[test]
fn transfer_center_open_actions_route_through_platform_helpers_and_visible_feedback() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let local_open_source =
        fs::read_to_string("src/app/sftp/local_open.rs").expect("read local open helper");

    assert!(
        bootstrap_sftp.contains("window.on_transfer_center_open_file_requested")
            && bootstrap_sftp.contains("window.on_transfer_center_open_folder_requested")
            && bootstrap_sftp.contains("show_transfer_center_feedback("),
        "transfer-center row actions should route open failures through a visible feedback path instead of failing silently"
    );
    assert!(
        bootstrap_sftp.contains("open_path_locally(local_path.as_path())")
            && bootstrap_sftp.contains("reveal_path_locally(local_path.as_path())"),
        "transfer-center Open File and Open Folder should call dedicated platform helpers instead of sharing a single file-open path"
    );
    assert!(
        local_open_source.contains("pub fn reveal_path_locally(path: &Path) -> Result<()>")
            && local_open_source.contains("SHOpenFolderAndSelectItems")
            && local_open_source.contains("xdg-open")
            && local_open_source.contains("arg(\"-R\")"),
        "the local-open helper should expose cross-platform file-open plus containing-folder reveal logic with a native Shell reveal path on Windows"
    );
}

#[test]
fn transfer_center_local_shell_actions_leave_the_ui_thread_free() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");

    assert!(
        bootstrap_sftp.contains("queue_transfer_center_open_file_action(")
            && bootstrap_sftp.contains("queue_transfer_center_open_folder_action(")
            && bootstrap_sftp.contains("queue_transfer_center_remove_action(")
            && bootstrap_sftp.contains("drain_sftp_local_action_background_messages(")
            && bootstrap_sftp.contains("std::thread::spawn(move || {"),
        "transfer-center local shell actions should be queued onto background threads and drained back onto the UI model so Explorer or trash handoff cannot freeze the terminal surface"
    );
}

#[test]
fn download_conflicts_apply_preferred_default_and_auto_open_when_asking() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");

    assert!(
        bootstrap_sftp.contains("state.settings_modal_download_conflict_default()")
            && bootstrap_sftp.contains("crate::app::ui_preferences::DownloadConflictDefault::Ask")
            && bootstrap_sftp.contains("state.open_transfer_conflict_modal("),
        "download scheduling should honor the persisted default conflict policy, and Ask should auto-open the conflict modal when a local download collision is reported"
    );
}

#[test]
fn transfer_center_attention_rows_can_open_linked_sftp_workspace() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-open-workspace");
    let conflict_path = temp_root.join("logs");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center open workspace temp root");
    fs::write(&conflict_path, b"pretend archive bytes")
        .expect("write transfer-center open workspace upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        conflict_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Conflict")
                .unwrap_or(false)
    });

    let attention_row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("transfer-center attention row");
    assert!(
        attention_row.can_open_workspace,
        "attention rows should expose a path into the full SFTP workspace for heavier follow-up work"
    );

    app.invoke_transfer_center_open_workspace_requested(attention_row.id.clone());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_workspace_session_host_mode().as_str() == "sftp"
    });

    assert_eq!(
        app.get_workspace_session_title().as_str(),
        "Files: Prod Bastion"
    );
}

#[test]
fn transfer_center_remove_missing_download_only_clears_record() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp");
    let local_open_source =
        fs::read_to_string("src/app/sftp/local_open.rs").expect("read local open helper");

    assert!(
        bootstrap_sftp.contains("trash_path_locally(local_path.as_path())")
            && bootstrap_sftp.contains("local file is already missing"),
        "remove should trash finished downloads when the artifact exists, but fall back to record-only cleanup with explicit feedback once the local file is gone"
    );
    assert!(
        local_open_source.contains("pub fn trash_path_locally(path: &Path) -> Result<()>"),
        "local-open helpers should expose an explicit trash helper so transfer-center removal does not silently bypass the desktop recycle bin or trash"
    );
}

#[test]
fn transfer_center_conflict_rows_can_open_resolve_modal_and_replace() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-conflict-replace");
    let upload_path = temp_root.join("release.tar.gz");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center conflict replace temp root");
    fs::write(&upload_path, b"archive bytes")
        .expect("write transfer-center conflict replace upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Conflict" && row.can_resolve_conflict)
                .unwrap_or(false)
    });

    let row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("transfer-center conflict row");
    assert_eq!(
        row.error_summary.as_str(),
        "An item with the same name already exists at the destination."
    );
    app.invoke_transfer_center_resolve_conflict_requested(row.id.clone());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_open()
    });

    assert_eq!(
        app.get_sftp_conflict_modal_source_path().as_str(),
        upload_path.to_string_lossy().as_ref()
    );
    assert_eq!(
        app.get_sftp_conflict_modal_target_path().as_str(),
        "/srv/app/releases/release.tar.gz"
    );

    app.invoke_sftp_conflict_modal_replace_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        !app.get_sftp_conflict_modal_open()
            && rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|updated| updated.status_label.as_str() == "Completed")
                .unwrap_or(false)
    });
}

#[test]
fn transfer_center_conflict_modal_can_apply_replace_to_matching_destination_batch() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-conflict-batch-replace");
    let release_a_path = temp_root.join("release-a.tar.gz");
    let release_b_path = temp_root.join("release-b.tar.gz");
    let other_path = temp_root.join("other.env");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center conflict batch temp root");
    fs::write(&release_a_path, b"archive a")
        .expect("write transfer-center conflict batch source a");
    fs::write(&release_b_path, b"archive b")
        .expect("write transfer-center conflict batch source b");
    fs::write(&other_path, b"PORT=22\n").expect("write transfer-center conflict batch source c");

    sftp_state.set_remote_file("/srv/app/releases/release-a.tar.gz", b"existing a".to_vec());
    sftp_state.set_remote_file("/srv/app/releases/release-b.tar.gz", b"existing b".to_vec());
    sftp_state.set_remote_file("/srv/app/config/other.env", b"existing c".to_vec());

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![
        SharedString::from(release_a_path.to_string_lossy().to_string()),
        SharedString::from(release_b_path.to_string_lossy().to_string()),
    ])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut release_conflicts = 0;
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.status_label.as_str() == "Conflict"
                && (row.title.as_str() == "release-a.tar.gz"
                    || row.title.as_str() == "release-b.tar.gz")
            {
                release_conflicts += 1;
            }
        }
        release_conflicts == 2
    });

    app.invoke_sftp_panel_path_submitted("/srv/app/config".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/config"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        other_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut conflict_titles = Vec::new();
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.status_label.as_str() == "Conflict" {
                conflict_titles.push(row.title.to_string());
            }
        }
        conflict_titles
            .iter()
            .any(|title| title == "release-a.tar.gz")
            && conflict_titles
                .iter()
                .any(|title| title == "release-b.tar.gz")
            && conflict_titles.iter().any(|title| title == "other.env")
    });

    let release_a_row = {
        let rows = app.get_transfer_center_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .find(|row| row.title.as_str() == "release-a.tar.gz")
            .expect("release-a conflict row")
    };
    app.invoke_transfer_center_resolve_conflict_requested(release_a_row.id.clone());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_open()
    });

    assert_eq!(app.get_sftp_conflict_modal_batch_conflict_count(), 1);
    app.invoke_sftp_conflict_modal_apply_to_batch_toggled(true);
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_apply_to_batch()
    });

    app.invoke_sftp_conflict_modal_replace_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut release_a_done = false;
        let mut release_b_done = false;
        let mut other_still_conflict = false;
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.title.as_str() == "release-a.tar.gz" && row.status_label.as_str() == "Completed"
            {
                release_a_done = true;
            }
            if row.title.as_str() == "release-b.tar.gz" && row.status_label.as_str() == "Completed"
            {
                release_b_done = true;
            }
            if row.title.as_str() == "other.env" && row.status_label.as_str() == "Conflict" {
                other_still_conflict = true;
            }
        }
        !app.get_sftp_conflict_modal_open()
            && release_a_done
            && release_b_done
            && other_still_conflict
    });
}

#[test]
fn transfer_center_conflict_modal_requests_focus_when_opened_for_keyboard_access() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );
    let temp_root = sample_vault_runtime_root("transfer-center-conflict-keyboard-batch");
    let release_a_path = temp_root.join("release-a.tar.gz");
    let release_b_path = temp_root.join("release-b.tar.gz");
    fs::create_dir_all(temp_root.as_path()).expect("create transfer-center keyboard temp root");
    fs::write(&release_a_path, b"archive a").expect("write transfer-center keyboard source a");
    fs::write(&release_b_path, b"archive b").expect("write transfer-center keyboard source b");

    sftp_state.set_remote_file("/srv/app/releases/release-a.tar.gz", b"existing a".to_vec());
    sftp_state.set_remote_file("/srv/app/releases/release-b.tar.gz", b"existing b".to_vec());

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![
        SharedString::from(release_a_path.to_string_lossy().to_string()),
        SharedString::from(release_b_path.to_string_lossy().to_string()),
    ])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut release_conflicts = 0;
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.status_label.as_str() == "Conflict" {
                release_conflicts += 1;
            }
        }
        release_conflicts == 2
    });

    let release_a_row = {
        let rows = app.get_transfer_center_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .find(|row| row.title.as_str() == "release-a.tar.gz")
            .expect("release-a conflict row")
    };
    app.invoke_transfer_center_resolve_conflict_requested(release_a_row.id.clone());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_open()
    });

    assert_eq!(app.get_sftp_conflict_modal_batch_conflict_count(), 1);
    assert!(
        !app.get_sftp_conflict_modal_apply_to_batch(),
        "batch scope should start disabled until the user explicitly opts in"
    );
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_focus_sequence() > 0
    });
}

#[test]
fn transfer_center_conflict_rows_can_skip_conflicted_transfer() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-conflict-skip");
    let upload_path = temp_root.join("release.tar.gz");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center conflict skip temp root");
    fs::write(&upload_path, b"archive bytes")
        .expect("write transfer-center conflict skip upload source");

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        upload_path.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|row| row.status_label.as_str() == "Conflict")
                .unwrap_or(false)
    });

    let row = app
        .get_transfer_center_items()
        .row_data(0)
        .expect("transfer-center conflict row");
    app.invoke_transfer_center_resolve_conflict_requested(row.id.clone());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_open()
    });

    app.invoke_sftp_conflict_modal_skip_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        !app.get_sftp_conflict_modal_open()
            && rows.row_count() > 0
            && rows
                .row_data(0)
                .map(|updated| updated.status_label.as_str() == "Skipped")
                .unwrap_or(false)
    });
}

#[test]
fn transfer_center_conflict_modal_close_skips_only_the_current_download() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-conflict-close-skip-current");
    let release_a_path = temp_root.join("release-a.tar.gz");
    let release_b_path = temp_root.join("release-b.tar.gz");
    fs::create_dir_all(temp_root.as_path())
        .expect("create transfer-center close-skip-current temp root");
    fs::write(&release_a_path, b"archive a")
        .expect("write transfer-center close-skip-current source a");
    fs::write(&release_b_path, b"archive b")
        .expect("write transfer-center close-skip-current source b");
    sftp_state.set_remote_file("/srv/app/releases/release-a.tar.gz", b"existing a".to_vec());
    sftp_state.set_remote_file("/srv/app/releases/release-b.tar.gz", b"existing b".to_vec());

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();

    app.invoke_sftp_panel_path_submitted("/srv/app/releases".into());
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_sftp_panel_path().as_str() == "/srv/app/releases"
    });

    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![
        SharedString::from(release_a_path.to_string_lossy().to_string()),
        SharedString::from(release_b_path.to_string_lossy().to_string()),
    ])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut conflict_titles = Vec::new();
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.status_label.as_str() == "Conflict" {
                conflict_titles.push(row.title.to_string());
            }
        }
        conflict_titles
            .iter()
            .any(|title| title == "release-a.tar.gz")
            && conflict_titles
                .iter()
                .any(|title| title == "release-b.tar.gz")
    });

    let release_a_row = {
        let rows = app.get_transfer_center_items();
        (0..rows.row_count())
            .filter_map(|index| rows.row_data(index))
            .find(|row| row.title.as_str() == "release-a.tar.gz")
            .expect("release-a conflict row")
    };
    app.invoke_transfer_center_resolve_conflict_requested(release_a_row.id.clone());
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_open()
    });

    assert_eq!(app.get_sftp_conflict_modal_batch_conflict_count(), 1);
    app.invoke_sftp_conflict_modal_apply_to_batch_toggled(true);
    wait_for_condition(Duration::from_millis(300), || {
        flush_runtime_projection();
        app.get_sftp_conflict_modal_apply_to_batch()
    });

    app.invoke_sftp_conflict_modal_close_requested();
    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        let rows = app.get_transfer_center_items();
        let mut release_a_skipped = false;
        let mut release_b_conflict = false;
        for index in 0..rows.row_count() {
            let Some(row) = rows.row_data(index) else {
                continue;
            };
            if row.title.as_str() == "release-a.tar.gz" && row.status_label.as_str() == "Skipped" {
                release_a_skipped = true;
            }
            if row.title.as_str() == "release-b.tar.gz" && row.status_label.as_str() == "Conflict" {
                release_b_conflict = true;
            }
        }
        !app.get_sftp_conflict_modal_open() && release_a_skipped && release_b_conflict
    });
}

#[test]
fn transfer_summary_recomputes_current_session_counts_when_switching_tabs() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    let sftp_state = RecordingSftpState::default();
    bind_with_launcher(
        &app,
        None,
        Arc::new(RecordingSftpLauncher {
            state: sftp_state.clone(),
        }),
    );

    let temp_root = sample_vault_runtime_root("transfer-center-session-counts");
    fs::create_dir_all(temp_root.as_path()).expect("create session-count temp root");
    let prod_file_a = temp_root.join("prod-a.env");
    let prod_file_b = temp_root.join("prod-b.env");
    let db_file = temp_root.join("db.env");
    fs::write(&prod_file_a, b"PORT=22\n").expect("write prod-a upload source");
    fs::write(&prod_file_b, b"PORT=23\n").expect("write prod-b upload source");
    fs::write(&db_file, b"PORT=24\n").expect("write db upload source");

    let prod_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(prod_id.into());
    flush_runtime_projection();
    let prod_session_id = app.get_active_workspace_session_id().to_string();

    let db_id = create_root_ssh(&app, "DB Replica", "10.0.0.24");
    app.invoke_asset_activated(db_id.into());
    flush_runtime_projection();
    let db_session_id = app.get_active_workspace_session_id().to_string();

    app.invoke_workspace_tab_selected(prod_session_id.clone().into());
    flush_runtime_projection();
    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![
        SharedString::from(prod_file_a.to_string_lossy().to_string()),
        SharedString::from(prod_file_b.to_string_lossy().to_string()),
    ])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_transfer_queue_total() >= 2 && app.get_transfer_queue_current_session() == 2
    });

    app.invoke_workspace_tab_selected(db_session_id.clone().into());
    flush_runtime_projection();
    app.set_sftp_panel_external_drop_paths(ModelRc::new(VecModel::from(vec![SharedString::from(
        db_file.to_string_lossy().to_string(),
    )])));
    app.invoke_sftp_panel_external_drop_requested();

    wait_for_condition(Duration::from_secs(2), || {
        flush_runtime_projection();
        app.get_transfer_queue_total() >= 3 && app.get_transfer_queue_current_session() == 1
    });

    app.invoke_workspace_tab_selected(prod_session_id.into());
    flush_runtime_projection();

    wait_for_condition(Duration::from_millis(500), || {
        flush_runtime_projection();
        app.get_transfer_queue_current_session() == 2
    });
}

#[test]
fn native_windowing_bridge_wires_os_file_drop_events_into_sftp_callbacks() {
    let windowing_source =
        fs::read_to_string("src/app/bootstrap/windowing.rs").expect("read windowing source");

    assert!(
        windowing_source.contains("WindowEvent::HoveredFile")
            && windowing_source.contains("invoke_sftp_panel_external_drop_hover_changed"),
        "native windowing should forward hovered-file events into the quick-browser drop hover callback"
    );
    assert!(
        windowing_source.contains("WindowEvent::DroppedFile")
            && windowing_source.contains("invoke_sftp_panel_external_drop_requested"),
        "native windowing should forward dropped files into the quick-browser upload callback"
    );
}

#[test]
fn sftp_sort_and_column_width_callbacks_round_trip_runtime_window_state() {
    let _bootstrap_smoke_test_guard = init_bootstrap_smoke_test();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_sftp_panel_sort_column().as_str(), "default");
    assert_eq!(app.get_sftp_panel_sort_direction().as_str(), "none");
    assert_eq!(app.get_sftp_panel_name_column_width(), 226.0);
    assert_eq!(app.get_sftp_panel_type_column_width(), 78.0);
    assert_eq!(app.get_sftp_panel_modified_column_width(), 150.0);
    assert_eq!(app.get_sftp_panel_size_column_width(), 72.0);

    app.invoke_sftp_panel_sort_requested("modified".into());
    assert_eq!(app.get_sftp_panel_sort_column().as_str(), "modified");
    assert_eq!(app.get_sftp_panel_sort_direction().as_str(), "asc");

    app.invoke_sftp_panel_sort_requested("modified".into());
    assert_eq!(app.get_sftp_panel_sort_column().as_str(), "modified");
    assert_eq!(app.get_sftp_panel_sort_direction().as_str(), "desc");

    app.invoke_sftp_panel_sort_requested("modified".into());
    assert_eq!(app.get_sftp_panel_sort_column().as_str(), "default");
    assert_eq!(app.get_sftp_panel_sort_direction().as_str(), "none");

    app.invoke_sftp_panel_column_width_change_requested("name".into(), 320.0);
    app.invoke_sftp_panel_column_width_change_requested("type".into(), 12.0);
    app.invoke_sftp_panel_column_width_change_requested("modified".into(), 64.0);
    app.invoke_sftp_panel_column_width_change_requested("size".into(), 20.0);

    assert_eq!(app.get_sftp_panel_name_column_width(), 320.0);
    assert_eq!(app.get_sftp_panel_type_column_width(), 72.0);
    assert_eq!(app.get_sftp_panel_modified_column_width(), 132.0);
    assert_eq!(app.get_sftp_panel_size_column_width(), 72.0);
}
