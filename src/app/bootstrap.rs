//! Wires the Slint window to runtime state, persisted preferences, and native window hooks during startup.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use directories::ProjectDirs;
use slint::{
    Color, ComponentHandle, Image, Model, ModelRc, SharedString, Timer, TimerMode, VecModel,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AppWindow;
use crate::AssetsContextMenuItem;
use crate::ConnectionProgressDiagnosticRow;
use crate::ConnectionProgressStepRow;
use crate::ConsoleAssetItem;
use crate::QuickLaunchCardRow;
use crate::QuickLaunchDetailRow;
use crate::QuickLaunchGroupRow;
use crate::SftpPanelItem;
use crate::WorkspaceTabItem;
use crate::app::app_paths::{AppRootPathInputs, AppRootPaths, resolve_app_root_paths};
use crate::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    RedbAssetCatalogStore, asset_tree_to_catalog, asset_trees_to_catalog, catalog_to_asset_tree,
    catalog_to_asset_trees,
};
use crate::app::async_runtime::AppAsyncRuntime;
use crate::app::keychain::{
    KeychainNodePayload, derive_public_key_material_from_private_key,
    derive_public_key_material_from_public_key, resolve_saved_ssh_profile,
};
use crate::app::quick_launch_preferences::{
    QuickLaunchPreferences, QuickLaunchPreferencesStore, retain_known_ssh_asset_ids,
};
use crate::app::runtime_profile::{AppRuntimeProfile, TerminalRenderMode};
use crate::app::sftp::{
    SftpBrowserController, SftpBrowserLoadRequest, SftpBrowserSessionState,
    SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode, SftpSessionBindingState,
};
use crate::app::ssh::connection_progress::{
    ConnectionAttemptState, ConnectionHeadlineState, ConnectionStepState, ConnectionStepStateItem,
};
use crate::app::ssh::credentials::{
    CachedCredentialStore, CredentialStore, EncryptedFileCredentialStore, FallbackCredentialStore,
    FileCredentialStore, MirroredCredentialStore, StoredKeychainKeySecretBundle,
    StoredSecretLookupError, StoredSshSecretBundle, SystemCredentialStore,
    load_secret_bundle_with_diagnostics, persist_keychain_key_secret_bundle, persist_secret_bundle,
    required_secret_bundle_field, restore_snapshot_secret_bundle,
};
use crate::app::ssh::known_hosts::{KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
use crate::app::ssh::proxy::resolve_proxy_chain;
use crate::app::ssh::runtime::{
    SessionRuntimeEvent, SshSessionRuntime, TerminalKeyEvent, TerminalMouseButton,
    TerminalMouseEventKind, TerminalMouseInput, TerminalSurfaceState, UnknownHostKeyError,
    load_optional_stored_secret_bundle, stored_secret_lookup_message,
};
use crate::app::ssh::session_manager::{
    EnhancedSessionState, OpenSessionMode, SessionHandle, SessionManager,
    SessionRuntimeControl, SessionRuntimeLauncher, SessionState,
};
use crate::app::terminal_atlas::TerminalAtlasSelection;
#[cfg(all(target_os = "windows", feature = "terminal-native-renderer"))]
use crate::app::terminal_presenter::WindowsNativePresenter;
use crate::app::terminal_presenter::{
    BitmapAtlasPresenter, NativeTerminalFrame, PresentedTerminalFrame, TerminalPresentationOptions,
    TerminalPresenter,
};
use crate::app::terminal_renderer::{NativeTerminalSurface, NativeTerminalSurfaceRect};
use crate::app::terminal_theme::{preset_for_theme_mode, selection_overlay_rgba};
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::vault::bootstrap::{
    LocalVaultBootstrapState, bootstrap_provider_credential_ref, load_local_vault_bootstrap_state,
    load_provider_credential, persist_provider_credential, save_local_vault_bootstrap_state,
};
use crate::app::vault::cache::{load_encrypted_cache, store_encrypted_cache};
use crate::app::vault::crypto::{
    WrappedVaultKey, decrypt_snapshot, encrypt_snapshot, generate_vault_key, unwrap_vault_key,
    wrap_vault_key,
};
use crate::app::vault::engine::{SyncEngine, SyncError, SyncRequest};
use crate::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, GiteeRemoteDraft, KdfConfig, ProviderAuthKind,
    ProviderKind, RemoteRole, SnapshotSyncPreferences, VaultAssetPayload, VaultSnapshot,
};
use crate::app::vault::provider::gitee_gist::{GiteeGistProvider, GiteeGistProviderConfig};
use crate::app::vault::provider::github_gist::{GitHubGistProvider, GitHubGistProviderConfig};
use crate::app::vault::provider::gitlab_snippet::{
    GitLabSnippetProvider, GitLabSnippetProviderConfig,
};
use crate::app::vault::provider::s3::{S3VaultProvider, S3VaultProviderConfig};
use crate::app::vault::provider::{
    VaultProvider, first_release_formal_auth_label, first_release_formal_provider_kind,
    first_release_formal_provider_label,
};
use crate::app::vault::snapshot::{apply_vault_snapshot, export_vault_snapshot};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{
    ModalDragState, ModalOffset, WindowController, apply_restored_window_size, begin_modal_drag,
    parse_resize_direction, update_modal_drag, window_appearance,
};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
};
use crate::shell::assets::{
    AssetDisclosureState, AssetSocks5ProxySpec, AssetSshConnectionSpec, AssetSshProxySpec,
    AssetTree, ConsoleAssetKind, SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY,
};
use crate::shell::context_menu::{
    CONTEXT_MENU_COLUMN_GAP, CONTEXT_MENU_COLUMN_WIDTH, ContextMenuActionNode,
    ContextMenuActionState, ContextTargetKind, MenuPlacementInput, Rect, SelectionContext,
    context_menu_column_height, context_menu_column_offset, resolve_action_tree,
    resolve_root_menu_origin, should_keep_corridor_open, visible_columns_for_path,
};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for, toolbar_descriptor_for};
use crate::shell::tabs::WorkspaceTab;
use crate::shell::view_model::{
    AssetModalState, AssetSshConnectionDraft, KeychainSshKeyDraft, RightPanelView, ShellViewModel,
    SnippetActivation, SnippetCreateAction, SshModalAction, SyncModalMode,
};
use crate::theme::ThemeMode;
use russh::keys::ssh_key::{LineEnding, rand_core::OsRng};
use russh::keys::{Algorithm, PrivateKey, PublicKey};

#[derive(Clone)]
struct ShellSessionBridge {
    manager: SessionManager,
}

const MAX_SSH_PROXY_CHAIN_DEPTH: usize = 8;
const WORKSPACE_PASTE_EDITOR_LINE_THRESHOLD: usize = 4;

thread_local! {
    static WORKSPACE_TERMINAL_PRESENTER: RefCell<Box<dyn TerminalPresenter>> = RefCell::new(
        Box::new(
            BitmapAtlasPresenter::new().expect("bundled Sarasa presenter should initialize")
        )
    );
    static WORKSPACE_NATIVE_TERMINAL_SURFACE: RefCell<Option<NativeTerminalSurface>> = const {
        RefCell::new(None)
    };
}

#[derive(Clone)]
struct PendingHostKeyApproval {
    profile: ConnectionProfile,
    public_key_openssh: String,
    intent: HostKeyApprovalIntent,
}

#[derive(Clone)]
struct PendingAssetClick {
    asset_id: String,
    clicked_at: Instant,
}

#[derive(Clone)]
struct PendingWorkspacePasteWarning {
    session_id: Uuid,
    text: String,
    logical_line_count: usize,
    prompt_mode: WorkspacePastePromptMode,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspacePastePromptMode {
    Confirm,
    Editor,
}

#[derive(Clone, Copy)]
enum HostKeyApprovalIntent {
    ModalTestConnection,
    OpenSession(OpenSessionMode),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VaultSyncTrigger {
    Manual,
    DebouncedAuto,
    Periodic,
}

#[derive(Default)]
struct VaultSyncSchedulerState {
    dirty: bool,
    running: bool,
}

const VAULT_AUTO_SYNC_DEBOUNCE_MS: u64 = 1_200;
const VAULT_PERIODIC_SYNC_INTERVAL_MS: u64 = 120_000;

struct VaultSessionState {
    root_dir: PathBuf,
    provider_factory: Arc<dyn VaultProviderFactory>,
    bootstrap_template: Option<BootstrapBundle>,
    local_state: Option<LocalVaultBootstrapState>,
    unlocked_vault_key: Option<[u8; 32]>,
    decrypted_snapshot: Option<VaultSnapshot>,
}

impl VaultSessionState {
    fn new(
        root_dir: PathBuf,
        provider_factory: Arc<dyn VaultProviderFactory>,
        bootstrap_template: Option<BootstrapBundle>,
        local_state: Option<LocalVaultBootstrapState>,
    ) -> Self {
        Self {
            root_dir,
            provider_factory,
            bootstrap_template,
            local_state,
            unlocked_vault_key: None,
            decrypted_snapshot: None,
        }
    }

    fn bootstrap_state_path(&self) -> PathBuf {
        self.root_dir.join("vault-bootstrap-state.json")
    }

    fn cache_root(&self) -> PathBuf {
        self.root_dir.join("cache")
    }

    fn known_hosts_path(&self) -> PathBuf {
        vault_known_hosts_path(&self.root_dir)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct NativeTerminalModifierState {
    ctrl: bool,
    shift: bool,
    alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeTerminalClipboardShortcut {
    Copy,
    Paste,
}

#[derive(Clone)]
struct LiveSessionRuntimeLauncher {
    credential_store: Arc<dyn CredentialStore>,
}

pub trait VaultProviderFactory: Send + Sync {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>>;
}

#[derive(Clone)]
pub struct VaultRuntimeOptions {
    pub root_dir: Option<PathBuf>,
    pub provider_factory: Arc<dyn VaultProviderFactory>,
    pub bootstrap_template: Option<BootstrapBundle>,
}

impl Default for VaultRuntimeOptions {
    fn default() -> Self {
        Self {
            root_dir: None,
            provider_factory: Arc::new(DefaultVaultProviderFactory),
            bootstrap_template: None,
        }
    }
}

#[derive(Default)]
struct DefaultVaultProviderFactory;

impl VaultProviderFactory for DefaultVaultProviderFactory {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>> {
        match remote.provider {
            ProviderKind::S3Compatible => Ok(Arc::new(S3VaultProvider::new(
                S3VaultProviderConfig::try_from(remote)?,
            )?)),
            ProviderKind::GitHubGist => Ok(Arc::new(GitHubGistProvider::new(
                GitHubGistProviderConfig::try_from(remote)?,
            )?)),
            ProviderKind::GitLabSnippet => Ok(Arc::new(GitLabSnippetProvider::new(
                GitLabSnippetProviderConfig::try_from(remote)?,
            )?)),
            ProviderKind::GiteeGist => Ok(Arc::new(GiteeGistProvider::new(
                GiteeGistProviderConfig::try_from(remote)?,
            )?)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPrivateKey {
    pub path: PathBuf,
    pub content: String,
}

pub trait PrivateKeyImporter: Send + Sync {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>>;
}

#[derive(Default)]
struct LivePrivateKeyImporter;

#[derive(Default)]
struct EditSshModalSecretHydration {
    password: Option<String>,
    private_key_content: Option<String>,
    passphrase: Option<String>,
    proxy_socks5_password: Option<String>,
    inline_error: Option<String>,
}

impl PrivateKeyImporter for LivePrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Import SSH Private Key")
            .pick_file()
        else {
            return Ok(None);
        };

        let content = fs::read_to_string(&path)
            .with_context(|| format!("failed to read private key file `{}`", path.display()))?;

        Ok(Some(ImportedPrivateKey { path, content }))
    }
}

impl SessionRuntimeLauncher for LiveSessionRuntimeLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let credential_store = Arc::clone(&self.credential_store);
        Box::pin(async move {
            let session = SshSessionRuntime::connect_with_credential_store(
                profile,
                session_id,
                attempt_id,
                event_tx,
                credential_store,
            )
            .await?;
            Ok(Box::new(session) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        let credential_store = Arc::clone(&self.credential_store);
        Box::pin(async move {
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let runtime = SshSessionRuntime::connect_with_credential_store(
                profile,
                Uuid::new_v4(),
                Uuid::new_v4(),
                event_tx,
                credential_store,
            )
            .await?;
            runtime.disconnect()?;
            Ok(())
        })
    }
}

fn update_native_terminal_modifier_state(
    modifiers: &mut NativeTerminalModifierState,
    event: &slint::winit_030::winit::event::KeyEvent,
) {
    use slint::winit_030::winit::event::ElementState;
    use slint::winit_030::winit::keyboard::{Key, NamedKey};

    let pressed = event.state == ElementState::Pressed;
    match &event.logical_key {
        Key::Named(NamedKey::Control) => modifiers.ctrl = pressed,
        Key::Named(NamedKey::Shift) => modifiers.shift = pressed,
        Key::Named(NamedKey::Alt | NamedKey::AltGraph) => modifiers.alt = pressed,
        _ => {}
    }
}

fn native_terminal_clipboard_shortcut(
    key: &slint::winit_030::winit::keyboard::Key,
    modifiers: NativeTerminalModifierState,
) -> Option<NativeTerminalClipboardShortcut> {
    use slint::winit_030::winit::keyboard::{Key, NamedKey};

    if !modifiers.ctrl || !modifiers.shift || modifiers.alt {
        return None;
    }

    match key {
        Key::Named(NamedKey::Copy) => Some(NativeTerminalClipboardShortcut::Copy),
        Key::Named(NamedKey::Paste) => Some(NativeTerminalClipboardShortcut::Paste),
        Key::Character(text) if text == "\u{3}" => Some(NativeTerminalClipboardShortcut::Copy),
        Key::Character(text) if text == "\u{16}" => Some(NativeTerminalClipboardShortcut::Paste),
        Key::Character(text) if text.eq_ignore_ascii_case("c") => {
            Some(NativeTerminalClipboardShortcut::Copy)
        }
        Key::Character(text) if text.eq_ignore_ascii_case("v") => {
            Some(NativeTerminalClipboardShortcut::Paste)
        }
        _ => None,
    }
}

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn runtime_window_title(_profile: AppRuntimeProfile) -> String {
    app_title().to_owned()
}

pub fn startup_failure_message(profile: AppRuntimeProfile, err: &str) -> Option<String> {
    Some(format!(
        "Mica Term failed to initialize {}: {err}",
        profile.selector_label()
    ))
}

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

#[cfg(target_os = "windows")]
fn sync_windows_true_window_placement(
    window: &AppWindow,
    state: &Rc<RefCell<ShellViewModel>>,
    effects: &dyn PlatformWindowEffects,
    winit_window: &slint::winit_030::winit::window::Window,
) {
    let Some(next) = query_true_window_placement(winit_window) else {
        return;
    };

    let mut state = state.borrow_mut();
    if state.window_placement() == next {
        return;
    }

    state.set_window_placement(next);
    sync_top_status_bar_state(window, &state, effects);
}

fn bind_windows_window_state_tracking(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    _effects: Rc<dyn PlatformWindowEffects>,
    session_bridge: Option<Rc<ShellSessionBridge>>,
    pending_workspace_paste_warning: Rc<RefCell<Option<PendingWorkspacePasteWarning>>>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    let modifiers = Rc::new(RefCell::new(NativeTerminalModifierState::default()));
    window
        .window()
        .on_winit_window_event(move |_slint_window, event| {
            if matches!(event, winit::event::WindowEvent::Focused(false)) {
                *modifiers.borrow_mut() = NativeTerminalModifierState::default();
            }

            if let winit::event::WindowEvent::KeyboardInput {
                event: key_event,
                is_synthetic,
                ..
            } = event
            {
                let mut modifier_state = modifiers.borrow_mut();
                update_native_terminal_modifier_state(&mut modifier_state, key_event);

                if key_event.state == winit::event::ElementState::Pressed
                    && !key_event.repeat
                    && !is_synthetic
                    && let Some(shortcut) =
                        native_terminal_clipboard_shortcut(&key_event.logical_key, *modifier_state)
                {
                    drop(modifier_state);
                    let window = handle.unwrap();
                    if window.get_workspace_session_host_mode() == "terminal"
                        && !window.get_active_workspace_session_id().is_empty()
                    {
                        match shortcut {
                            NativeTerminalClipboardShortcut::Copy
                                if window.get_workspace_session_selection_active() =>
                            {
                                let state = state.borrow();
                                forward_active_workspace_copy_selection(
                                    &state,
                                    window.get_workspace_session_selection_start_row(),
                                    window.get_workspace_session_selection_start_col(),
                                    window.get_workspace_session_selection_end_row(),
                                    window.get_workspace_session_selection_end_col(),
                                );
                                return EventResult::PreventDefault;
                            }
                            NativeTerminalClipboardShortcut::Paste => {
                                let state = state.borrow();
                                let _ = forward_active_workspace_paste(
                                    &state,
                                    session_bridge.as_deref(),
                                    pending_workspace_paste_warning.as_ref(),
                                );
                                return EventResult::PreventDefault;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Win32 snap/maximize state can drift from declarative UI state, so re-sample it when
            // the platform reports geometry-affecting events.
            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) {
                #[cfg(target_os = "windows")]
                {
                    use slint::winit_030::WinitWindowAccessor;

                    let window = handle.unwrap();
                    let _ = window.window().with_winit_window(|winit_window| {
                        sync_windows_true_window_placement(
                            &window,
                            &state,
                            _effects.as_ref(),
                            winit_window,
                        );
                    });
                }
            }

            EventResult::Propagate
        });
}

fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    window.window().request_redraw();

    let request = build_native_window_appearance_request(state.theme_mode, window_appearance());
    let report = effects.apply_to_app_window(window, &request);

    if matches!(
        report.backdrop_status,
        crate::app::window_effects::BackdropApplyStatus::Failed
    ) {
        tracing::error!(
            target: "app.window",
            theme = ?request.theme,
            backdrop = ?request.backdrop,
            backdrop_error = %report.backdrop_error.as_deref().unwrap_or("unknown"),
            "failed to apply native window appearance"
        );
    }
}

fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_transfer_center_open(state.transfer_center_open());
    window.set_transfer_queue_total(
        i32::try_from(state.sftp_queue_summary.total_count).unwrap_or(i32::MAX),
    );
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized());
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
    window.set_sync_feedback_text(state.sync_feedback_state().text.clone().into());
    window.set_sync_feedback_sequence(state.sync_feedback_state().sequence);
    window.set_sync_feedback_running(state.sync_feedback_state().running);
}

fn sync_sync_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let modal = state.sync_modal_state();

    window.set_sync_modal_open(modal.open);
    window.set_sync_modal_mode(modal.mode.id().into());
    window.set_sync_modal_title(modal.title.clone().into());
    window.set_sync_modal_headline(modal.headline.clone().into());
    window.set_sync_modal_status_text(modal.status_text.clone().into());
    window.set_sync_modal_error_text(modal.error_text.clone().into());
    window.set_sync_modal_provider_label(modal.provider_label.clone().into());
    window.set_sync_modal_target_label(modal.target_label.clone().into());
    window.set_sync_modal_primary_action_label(modal.primary_action_label.clone().into());
    window.set_sync_modal_secondary_action_label(modal.secondary_action_label.clone().into());
    window.set_sync_modal_auto_sync_enabled(modal.auto_sync_enabled);
    window.set_sync_modal_primary_gist_id(modal.primary_gist_id.clone().into());
    window.set_sync_modal_primary_pat(modal.primary_pat.clone().into());
    window.set_sync_modal_mirror_enabled(modal.mirror_enabled);
    window.set_sync_modal_mirror_gist_id(modal.mirror_gist_id.clone().into());
    window.set_sync_modal_mirror_pat(modal.mirror_pat.clone().into());
    window.set_sync_modal_master_password(modal.master_password.clone().into());
}

fn sync_sftp_panel_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_sftp_panel_mode(state.sftp_panel_mode_id().into());
    window.set_sftp_panel_host_label(state.sftp_panel_host_label().into());
    window.set_sftp_panel_path(state.sftp_panel_path().into());
    window.set_sftp_panel_follow_mode(state.sftp_panel_follow_mode_id().into());
    window.set_sftp_panel_can_go_back(state.sftp_panel_can_go_back());
    window.set_sftp_panel_can_go_forward(state.sftp_panel_can_go_forward());
    window.set_sftp_panel_can_go_up(state.sftp_panel_can_go_up());
    window.set_sftp_panel_actions_enabled(state.sftp_panel_actions_enabled());
    window.set_sftp_panel_sort_column(state.sftp_panel_sort_column_id().into());
    window.set_sftp_panel_sort_direction(state.sftp_panel_sort_direction_id().into());
    window.set_sftp_panel_name_column_width(state.sftp_panel_name_column_width_px());
    window.set_sftp_panel_type_column_width(state.sftp_panel_type_column_width_px());
    window.set_sftp_panel_modified_column_width(state.sftp_panel_modified_column_width_px());
    window.set_sftp_panel_size_column_width(state.sftp_panel_size_column_width_px());
    window.set_sftp_queue_drawer_open(state.sftp_queue_drawer_open());

    let items = state
        .project_sftp_panel_entries(state.sftp_panel_entries())
        .iter()
        .map(|entry| SftpPanelItem {
            id: entry.id.as_str().into(),
            name: entry.name.as_str().into(),
            type_label: sftp_panel_entry_type_label(entry.kind).into(),
            modified_label: sftp_panel_entry_modified_label(entry).into(),
            size_label: sftp_panel_entry_size_label(entry).into(),
            kind: sftp_panel_entry_kind(entry.kind).into(),
            selected: state
                .sftp_panel_selected_entry_ids()
                .iter()
                .any(|selected_id| selected_id == &entry.id),
        })
        .collect::<Vec<_>>();
    sync_vec_model(window.get_sftp_panel_items(), items, |model| {
        window.set_sftp_panel_items(model)
    });

    let selected_ids = state
        .sftp_panel_selected_entry_ids()
        .iter()
        .map(|entry_id| SharedString::from(entry_id.as_str()))
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_sftp_panel_selected_entry_ids(),
        selected_ids,
        |model| window.set_sftp_panel_selected_entry_ids(model),
    );

    let queue = &state.sftp_queue_summary;
    window.set_sftp_panel_queue_active(i32::try_from(queue.active_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_failed(i32::try_from(queue.failed_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_current_session(
        i32::try_from(queue.current_session_count).unwrap_or(i32::MAX),
    );
}

fn sync_right_panel_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_right_panel_view(state.right_panel_view_id().into());
    sync_sftp_panel_state(window, state);
}

fn sync_sftp_remote_file_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let editor = state.sftp_remote_file_editor_state();
    window.set_sftp_remote_file_modal_open(editor.open);
    window.set_sftp_remote_file_modal_title(editor.title.clone().into());
    window.set_sftp_remote_file_modal_path(editor.remote_path.clone().into());
    window.set_sftp_remote_file_modal_content(editor.content.clone().into());
    window.set_sftp_remote_file_modal_status_text(editor.status_text.clone().into());
    window.set_sftp_remote_file_modal_error_text(editor.error_text.clone().into());
    window.set_sftp_remote_file_modal_can_save(state.sftp_remote_file_editor_can_save());
}

fn sftp_panel_entry_type_label(kind: SftpDirectoryEntryKind) -> &'static str {
    match kind {
        SftpDirectoryEntryKind::Directory => "Folder",
        SftpDirectoryEntryKind::Symlink => "Link",
        SftpDirectoryEntryKind::Unknown => "Unknown",
        SftpDirectoryEntryKind::File => "File",
    }
}

fn sftp_panel_entry_modified_label(entry: &crate::app::sftp::SftpDirectoryEntry) -> String {
    let Some(unix_seconds) = entry.modified_unix_seconds else {
        return String::new();
    };
    let Some(timestamp) = DateTime::<Utc>::from_timestamp(unix_seconds as i64, 0) else {
        return String::new();
    };
    timestamp.format("%Y-%m-%d %H:%M").to_string()
}

fn sftp_panel_entry_size_label(entry: &crate::app::sftp::SftpDirectoryEntry) -> String {
    entry.size_bytes.map(format_binary_size).unwrap_or_default()
}

fn sftp_panel_entry_kind(kind: SftpDirectoryEntryKind) -> &'static str {
    match kind {
        SftpDirectoryEntryKind::Directory => "directory",
        SftpDirectoryEntryKind::File => "file",
        SftpDirectoryEntryKind::Symlink => "symlink",
        SftpDirectoryEntryKind::Unknown => "unknown",
    }
}

fn format_binary_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn sync_sidebar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_assets_sidebar(state.show_assets_sidebar);
    window.set_active_sidebar_destination(state.active_sidebar_destination.id().into());
    window.set_sidebar_items(ModelRc::new(VecModel::from(sidebar_items_for(state))));
    sync_assets_toolbar_state(window, state);
    sync_console_assets(window, state);
    sync_keychain_assets(window, state);
}

fn sync_assets_toolbar_state(window: &AppWindow, state: &ShellViewModel) {
    let descriptor = toolbar_descriptor_for(state.active_sidebar_destination, state);
    window.set_asset_view_mode(state.asset_view_mode.id().into());
    window.set_asset_search_expanded(state.asset_search_expanded);
    let active_query = if state.active_sidebar_destination == SidebarDestination::Keychain {
        state.keychain_search_query.clone()
    } else {
        state.asset_search_query.clone()
    };
    window.set_assets_search_query(active_query.into());
    window.set_asset_create_menu_open(state.asset_create_menu_open);
    window.set_asset_uses_create_popover(descriptor.uses_create_popover);
    window.set_asset_tree_fully_expanded(state.asset_tree_fully_expanded);
    window.set_asset_primary_create_action_id(
        descriptor.primary_create_action_id.unwrap_or("").into(),
    );
    window.set_asset_primary_create_tooltip(descriptor.primary_create_tooltip.into());
    window.set_asset_search_tooltip(descriptor.search_tooltip.into());
    window.set_asset_view_mode_tooltip(descriptor.view_mode_tooltip.into());
    window.set_asset_tree_expansion_tooltip(descriptor.tree_expansion_tooltip.into());
    window.set_asset_show_tree_controls(descriptor.show_tree_controls);
    window.set_asset_tree_controls_enabled(descriptor.tree_controls_enabled);
}

fn sync_assets_context_menu_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_assets_context_menu_open(state.context_menu_open);
    window.set_assets_context_menu_anchor_x(state.context_menu_anchor_x);
    window.set_assets_context_menu_anchor_y(state.context_menu_anchor_y);
    window.set_assets_context_menu_origin_x(state.context_menu_origin_x);
    window.set_assets_context_menu_origin_y(state.context_menu_origin_y);
    window.set_assets_context_menu_child_flows_left(state.context_menu_child_flows_left);
    window.set_assets_context_menu_primary_items(ModelRc::new(VecModel::from(
        context_menu_primary_items_for(state),
    )));
    window.set_assets_context_menu_secondary_items(ModelRc::new(VecModel::from(
        context_menu_secondary_items_for(state),
    )));
    window.set_assets_context_menu_tertiary_items(ModelRc::new(VecModel::from(
        context_menu_tertiary_items_for(state),
    )));
    window.set_context_menu_feedback_text(state.context_menu_feedback_text.clone().into());
}

fn clear_asset_snippet_modal_fields(window: &AppWindow) {
    window.set_asset_snippet_modal_name("".into());
    window.set_asset_snippet_modal_script("".into());
    window.set_asset_snippet_modal_package("".into());
    sync_snippet_package_options(window, Vec::new());
    window.set_asset_snippet_modal_package_selected_label("".into());
    window.set_asset_snippet_package_modal_name("".into());
}

fn clear_asset_ssh_modal_fields(window: &AppWindow) {
    window.set_asset_ssh_modal_name("".into());
    window.set_asset_ssh_modal_host("".into());
    window.set_asset_ssh_modal_user("".into());
    window.set_asset_ssh_modal_port("22".into());
    window.set_asset_ssh_modal_auth_source("manual".into());
    window.set_asset_ssh_modal_auth_method("password".into());
    sync_ssh_keychain_identity_options(window, Vec::new());
    window.set_asset_ssh_modal_keychain_identity_selected_label("".into());
    window.set_asset_ssh_modal_keychain_identity_username("".into());
    window.set_asset_ssh_modal_keychain_identity_auth_summary("".into());
    window.set_asset_ssh_modal_private_key_source("content".into());
    window.set_asset_ssh_modal_password("".into());
    window.set_asset_ssh_modal_private_key_content("".into());
    window.set_asset_ssh_modal_private_key_path("".into());
    window.set_asset_ssh_modal_passphrase("".into());
    window.set_asset_ssh_modal_password_visible(false);
    window.set_asset_ssh_modal_remark("".into());
    window.set_asset_ssh_modal_environment("".into());
    window.set_asset_ssh_modal_proxy_type("none".into());
    window.set_asset_ssh_modal_proxy_socks5_host("".into());
    window.set_asset_ssh_modal_proxy_socks5_port("".into());
    window.set_asset_ssh_modal_proxy_socks5_username("".into());
    window.set_asset_ssh_modal_proxy_socks5_password("".into());
    window.set_asset_ssh_modal_proxy_socks5_password_visible(false);
    window.set_asset_ssh_modal_proxy_ssh_asset_id("".into());
    sync_ssh_proxy_target_options(window, Vec::new());
    window.set_asset_ssh_modal_proxy_ssh_selected_label("".into());
    window.set_asset_ssh_modal_proxy_method("".into());
}

fn sync_asset_modal_state(window: &AppWindow, state: &ShellViewModel) {
    sync_keychain_modal_defaults(window);
    match &state.asset_modal_state {
        Some(AssetModalState::NewFolder { draft_name, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-folder".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpNewFolder { draft_name }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-folder".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewSnippet { draft, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-snippet".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_snippet_modal_name(draft.name.clone().into());
            window.set_asset_snippet_modal_script(draft.script.clone().into());
            window.set_asset_snippet_modal_package(draft.package.clone().into());
            let mut package_options = vec!["No Package".to_string()];
            package_options.extend(state.snippet_package_option_labels());
            sync_snippet_package_options(window, package_options);
            window.set_asset_snippet_modal_package_selected_label(
                if draft.package.trim().is_empty() {
                    "No Package"
                } else {
                    draft.package.as_str()
                }
                .into(),
            );
            window.set_asset_snippet_package_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewSnippetPackage { draft_name, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-snippet-package".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_snippet_modal_name("".into());
            window.set_asset_snippet_modal_script("".into());
            window.set_asset_snippet_modal_package("".into());
            sync_snippet_package_options(window, Vec::new());
            window.set_asset_snippet_modal_package_selected_label("".into());
            window.set_asset_snippet_package_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewKeychainSshKey { draft, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-keychain-ssh-key".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Key".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_folder_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_keychain_ssh_key_modal_name(draft.name.clone().into());
            window.set_keychain_ssh_key_modal_private_key(draft.private_key.clone().into());
            window.set_keychain_ssh_key_modal_public_key(draft.public_key.clone().into());
            window.set_keychain_ssh_key_modal_fingerprint(draft.fingerprint.clone().into());
        }
        Some(AssetModalState::NewSshConnection {
            draft,
            editing_asset_id,
            ..
        }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-ssh-connection".into());
            window.set_asset_ssh_modal_dialog_title(
                if editing_asset_id.is_some() {
                    "Edit SSH Connection"
                } else {
                    "New SSH Connection"
                }
                .into(),
            );
            window.set_asset_modal_can_confirm(state.ssh_modal_save_enabled());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_name(draft.name.clone().into());
            window.set_asset_ssh_modal_host(draft.host.clone().into());
            window.set_asset_ssh_modal_user(draft.user.clone().into());
            window.set_asset_ssh_modal_port(draft.port.clone().into());
            window.set_asset_ssh_modal_auth_source(draft.auth_source.clone().into());
            window.set_asset_ssh_modal_auth_method(draft.auth_method.clone().into());
            sync_ssh_keychain_identity_options(window, state.ssh_keychain_identity_option_labels());
            window.set_asset_ssh_modal_keychain_identity_selected_label(
                state.ssh_keychain_identity_selected_label().into(),
            );
            window.set_asset_ssh_modal_keychain_identity_username(
                state.ssh_keychain_identity_selected_username().into(),
            );
            window.set_asset_ssh_modal_keychain_identity_auth_summary(
                state.ssh_keychain_identity_selected_auth_summary().into(),
            );
            window.set_asset_ssh_modal_private_key_source(draft.private_key_source.clone().into());
            window.set_asset_ssh_modal_password(draft.password.clone().into());
            window
                .set_asset_ssh_modal_private_key_content(draft.private_key_content.clone().into());
            window.set_asset_ssh_modal_private_key_path(draft.private_key_path.clone().into());
            window.set_asset_ssh_modal_passphrase(draft.passphrase.clone().into());
            window.set_asset_ssh_modal_password_visible(draft.password_visible);
            window.set_asset_ssh_modal_remark(draft.remark.clone().into());
            window.set_asset_ssh_modal_environment(draft.environment.clone().into());
            window.set_asset_ssh_modal_proxy_type(draft.proxy_type.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_host(draft.proxy_socks5_host.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_port(draft.proxy_socks5_port.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_username(
                draft.proxy_socks5_username.clone().into(),
            );
            window.set_asset_ssh_modal_proxy_socks5_password(
                draft.proxy_socks5_password.clone().into(),
            );
            window.set_asset_ssh_modal_proxy_socks5_password_visible(
                draft.proxy_socks5_password_visible,
            );
            window.set_asset_ssh_modal_proxy_ssh_asset_id(draft.proxy_ssh_asset_id.clone().into());
            sync_ssh_proxy_target_options(window, state.ssh_proxy_target_option_labels());
            window.set_asset_ssh_modal_proxy_ssh_selected_label(
                state.ssh_proxy_target_selected_label().into(),
            );
            window.set_asset_ssh_modal_proxy_method(draft.proxy_method.clone().into());
            window.set_asset_ssh_modal_connect_family_enabled(
                state.ssh_modal_connect_family_enabled(),
            );
            window.set_asset_ssh_modal_feedback_state(state.ssh_modal_feedback_state_id().into());
            window.set_asset_ssh_modal_feedback_message(state.ssh_modal_feedback_message().into());
        }
        Some(AssetModalState::RenameAsset { draft_name, .. }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(true);
            window.set_asset_rename_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_validation_message(
                state.asset_rename_modal_validation_message().into(),
            );
            window.set_asset_rename_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpRenameEntry { draft_name, .. }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(true);
            window.set_asset_rename_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_validation_message(
                state.asset_rename_modal_validation_message().into(),
            );
            window.set_asset_rename_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::DeleteAssetConfirm {
            label,
            descendant_count,
            ..
        }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(true);
            window.set_asset_delete_confirm_target_label(label.clone().into());
            window.set_asset_delete_confirm_descendant_count(*descendant_count as i32);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpDeleteEntriesConfirm {
            label,
            descendant_count,
            ..
        }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(true);
            window.set_asset_delete_confirm_target_label(label.clone().into());
            window.set_asset_delete_confirm_descendant_count(*descendant_count as i32);
            clear_asset_ssh_modal_fields(window);
        }
        None => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
    }
}

fn sync_ssh_proxy_target_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_ssh_modal_proxy_ssh_options(),
        rows,
        |model| window.set_asset_ssh_modal_proxy_ssh_options(model),
    );
}

fn sync_ssh_keychain_identity_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_ssh_modal_keychain_identity_options(),
        rows,
        |model| window.set_asset_ssh_modal_keychain_identity_options(model),
    );
}

fn sync_snippet_package_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_snippet_modal_package_options(),
        rows,
        |model| window.set_asset_snippet_modal_package_options(model),
    );
}

fn sync_ssh_host_key_modal_state(window: &AppWindow, state: &ShellViewModel) {
    match &state.ssh_host_key_prompt_state {
        Some(prompt) => {
            window.set_ssh_host_key_modal_open(true);
            window.set_ssh_host_key_modal_host(prompt.host.clone().into());
            window.set_ssh_host_key_modal_fingerprint(prompt.fingerprint.clone().into());
        }
        None => {
            window.set_ssh_host_key_modal_open(false);
            window.set_ssh_host_key_modal_host("".into());
            window.set_ssh_host_key_modal_fingerprint("".into());
        }
    }
}

fn sync_workspace_paste_warning_modal_state(
    window: &AppWindow,
    pending: Option<&PendingWorkspacePasteWarning>,
) {
    match pending {
        Some(pending) => {
            window.set_workspace_paste_warning_line_count(
                i32::try_from(pending.logical_line_count).unwrap_or(i32::MAX),
            );
            window.set_workspace_paste_warning_editor_mode(matches!(
                pending.prompt_mode,
                WorkspacePastePromptMode::Editor
            ));
            window.set_workspace_paste_warning_text(pending.text.clone().into());
            window.set_workspace_paste_warning_modal_open(true);
        }
        None => {
            window.set_workspace_paste_warning_modal_open(false);
            window.set_workspace_paste_warning_line_count(0);
            window.set_workspace_paste_warning_editor_mode(false);
            window.set_workspace_paste_warning_text("".into());
        }
    }
}

fn schedule_asset_modal_focus(window: &AppWindow) {
    let handle = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        let window = handle.unwrap();
        if window.get_asset_modal_open()
            || window.get_asset_rename_modal_open()
            || window.get_asset_delete_confirm_modal_open()
            || window.get_workspace_paste_warning_modal_open()
        {
            window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);
        }
        if window.get_sftp_remote_file_modal_open() {
            window.set_sftp_remote_file_modal_focus_sequence(
                window.get_sftp_remote_file_modal_focus_sequence() + 1,
            );
        }
    });
}

fn open_pending_snippet_create_modal(state: &mut ShellViewModel) {
    match state.take_pending_snippet_create_action() {
        Some(SnippetCreateAction::NewSnippet) => state.open_new_snippet_modal(None),
        Some(SnippetCreateAction::NewPackage) => state.open_new_snippet_package_modal(),
        None => {}
    }
}

fn apply_pending_snippet_activation(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some((snippet_id, mode)) = state.take_pending_snippet_activation() else {
        return;
    };
    let Some(script) = state.snippet_script(&snippet_id).map(str::to_owned) else {
        return;
    };

    match mode {
        SnippetActivation::Paste => {
            let Some(session_id) = active_workspace_session_uuid(state) else {
                return;
            };
            forward_workspace_session_paste(state, bridge, session_id, &script);
        }
        SnippetActivation::Run => {
            let runnable_script = if script.ends_with('\n') {
                script
            } else {
                format!("{script}\n")
            };
            forward_active_workspace_text_input(state, bridge, &runnable_script);
        }
    }

    refresh_active_workspace_projection(window, state, bridge, follow_tracker);
}

fn parse_context_target_kind(
    value: &str,
    active_sidebar_destination: SidebarDestination,
) -> ContextTargetKind {
    match value {
        "sftp-blank" => ContextTargetKind::SftpBlankArea,
        "sftp-directory" => ContextTargetKind::SftpDirectory,
        "sftp-file" => ContextTargetKind::SftpFile,
        "sftp-selection" => ContextTargetKind::SftpMultiSelection,
        "ssh" => ContextTargetKind::SshConnection,
        "folder" => ContextTargetKind::Folder,
        "snippet-package" => ContextTargetKind::SnippetPackage,
        "snippet" => ContextTargetKind::Snippet,
        "blank" if active_sidebar_destination == SidebarDestination::Snippets => {
            ContextTargetKind::SnippetsBlankArea
        }
        _ => ContextTargetKind::BlankArea,
    }
}

fn shared_app_credential_store() -> Arc<dyn CredentialStore> {
    match app_root_paths_for_app() {
        Ok(app_paths) => build_shared_app_credential_store_for_paths(
            None,
            app_paths.data_dir.join("credentials-secure"),
            app_paths.data_dir.join("credentials"),
        ),
        Err(err) => {
            let encrypted_root = std::env::temp_dir().join("mica-term-fallback-credentials-secure");
            let recovery_root = std::env::temp_dir().join("mica-term-fallback-credentials");
            tracing::error!(
                target: "app.ssh",
                error = %err,
                encrypted_root = %encrypted_root.display(),
                recovery_root = %recovery_root.display(),
                "failed to resolve application data directory for ssh credentials; using fallback path"
            );
            build_shared_app_credential_store_for_paths(None, encrypted_root, recovery_root)
        }
    }
}

pub fn build_shared_app_credential_store_for_paths(
    preferred_system_store: Option<Arc<dyn CredentialStore>>,
    encrypted_root: PathBuf,
    recovery_root: PathBuf,
) -> Arc<dyn CredentialStore> {
    let primary_store = preferred_system_store
        .unwrap_or_else(|| Arc::new(SystemCredentialStore) as Arc<dyn CredentialStore>);
    let encrypted_store =
        Arc::new(EncryptedFileCredentialStore::new(encrypted_root)) as Arc<dyn CredentialStore>;
    let recovery_store =
        Arc::new(FileCredentialStore::new(recovery_root)) as Arc<dyn CredentialStore>;
    let encrypted_chain = Arc::new(FallbackCredentialStore::new(
        encrypted_store,
        recovery_store,
    )) as Arc<dyn CredentialStore>;
    let backing = Arc::new(MirroredCredentialStore::new(primary_store, encrypted_chain))
        as Arc<dyn CredentialStore>;

    Arc::new(CachedCredentialStore::new(backing))
}

fn build_session_bridge(
    runtime_handle: tokio::runtime::Handle,
    credential_store: Arc<dyn CredentialStore>,
) -> Rc<ShellSessionBridge> {
    Rc::new(ShellSessionBridge {
        manager: SessionManager::new_with_launcher(
            runtime_handle,
            Arc::new(LiveSessionRuntimeLauncher { credential_store }),
        ),
    })
}

fn selection_context_for(state: &ShellViewModel) -> SelectionContext {
    state.context_menu_selection()
}

fn profile_for_saved_asset(
    state: &ShellViewModel,
    asset_id: &str,
) -> anyhow::Result<ConnectionProfile> {
    let node = state
        .console_asset_tree()
        .node(asset_id)
        .with_context(|| format!("saved ssh asset `{asset_id}` is missing from the asset tree"))?;
    let spec = state
        .console_asset_tree()
        .ssh_connection_spec(asset_id)
        .with_context(|| {
            format!("saved ssh asset `{asset_id}` is missing its connection payload")
        })?;
    resolve_saved_ssh_profile(asset_id, &node.title, spec, state.keychain_catalog())
}

fn runtime_ready_profile(
    state: &ShellViewModel,
    mut profile: ConnectionProfile,
) -> anyhow::Result<ConnectionProfile> {
    profile.resolved_proxy_hops = resolve_proxy_chain(
        state.console_asset_tree(),
        &profile,
        MAX_SSH_PROXY_CHAIN_DEPTH,
    )?;
    Ok(profile)
}

fn runtime_profile_for_saved_asset(
    state: &ShellViewModel,
    asset_id: &str,
) -> anyhow::Result<ConnectionProfile> {
    runtime_ready_profile(state, profile_for_saved_asset(state, asset_id)?)
}

fn validate_saved_modal_profile(state: &ShellViewModel, asset_id: &str) -> anyhow::Result<()> {
    let _ = runtime_profile_for_saved_asset(state, asset_id)?;
    Ok(())
}

fn profile_for_modal_action(
    state: &ShellViewModel,
    draft: &AssetSshConnectionDraft,
) -> anyhow::Result<ConnectionProfile> {
    let (asset_id, existing_spec) = match &state.asset_modal_state {
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(asset_id),
            ..
        }) => {
            let spec = state
                .console_asset_tree()
                .ssh_connection_spec(asset_id)
                .with_context(|| {
                    format!("saved ssh asset `{asset_id}` is missing its connection payload")
                })?;
            (asset_id.clone(), Some(spec))
        }
        _ => ("__modal_draft__".into(), None),
    };
    let spec = saved_spec_for_modal_draft(asset_id.as_str(), draft, existing_spec);
    resolve_saved_ssh_profile(
        asset_id.as_str(),
        &draft.name,
        &spec,
        state.keychain_catalog(),
    )
}

fn runtime_profile_for_modal_action(
    state: &ShellViewModel,
    draft: &AssetSshConnectionDraft,
) -> anyhow::Result<ConnectionProfile> {
    runtime_ready_profile(state, profile_for_modal_action(state, draft)?)
}

fn non_empty_saved_secret(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
}

fn saved_spec_for_modal_draft(
    asset_id: &str,
    draft: &AssetSshConnectionDraft,
    existing_spec: Option<&AssetSshConnectionSpec>,
) -> AssetSshConnectionSpec {
    let uses_saved_auth_secret = if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
        false
    } else {
        match draft.auth_method.as_str() {
            "password" => !draft.password.trim().is_empty(),
            "private-key" if draft.private_key_source == "content" => {
                !draft.private_key_content.trim().is_empty()
            }
            "private-key" if draft.private_key_source == "path" => {
                !draft.passphrase.trim().is_empty()
            }
            _ => false,
        }
    };
    let uses_saved_proxy_secret = matches!(draft.proxy_type.as_str(), "socks5" | "http")
        && !draft.proxy_socks5_password.trim().is_empty();
    let saved_secret_ref = (uses_saved_auth_secret || uses_saved_proxy_secret)
        .then(|| saved_ssh_credential_ref_for_modal(asset_id, existing_spec));
    let credential_ref = if uses_saved_auth_secret || uses_saved_proxy_secret {
        saved_secret_ref.clone()
    } else {
        None
    };
    let mut proxy = saved_proxy_spec_for_modal_draft(draft);
    match &mut proxy {
        AssetSshProxySpec::Socks5(spec) | AssetSshProxySpec::Http(spec) => {
            spec.password_credential_ref = if uses_saved_proxy_secret {
                saved_secret_ref.clone()
            } else {
                None
            };
        }
        AssetSshProxySpec::None | AssetSshProxySpec::SshAsset { .. } => {}
    }

    AssetSshConnectionSpec {
        host: draft.host.clone(),
        user: if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
            String::new()
        } else {
            draft.user.clone()
        },
        port: draft.port.clone(),
        auth_method: draft.auth_method.clone(),
        auth_source: draft.auth_source.clone(),
        keychain_identity_id: (draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY)
            .then(|| draft.keychain_identity_id.trim().to_string())
            .filter(|value| !value.is_empty()),
        private_key_source: draft.private_key_source.clone(),
        private_key_path: if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY
            || draft.private_key_source == "content"
        {
            String::new()
        } else {
            draft.private_key_path.clone()
        },
        environment: draft.environment.clone(),
        proxy,
        proxy_method: String::new(),
        remark: draft.remark.clone(),
        credential_ref,
    }
}

fn saved_proxy_spec_for_modal_draft(draft: &AssetSshConnectionDraft) -> AssetSshProxySpec {
    match draft.proxy_type.as_str() {
        "socks5" => AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
            host: draft.proxy_socks5_host.clone(),
            port: draft.proxy_socks5_port.clone(),
            username: draft.proxy_socks5_username.clone(),
            password_credential_ref: None,
        }),
        "http" => AssetSshProxySpec::Http(AssetSocks5ProxySpec {
            host: draft.proxy_socks5_host.clone(),
            port: draft.proxy_socks5_port.clone(),
            username: draft.proxy_socks5_username.clone(),
            password_credential_ref: None,
        }),
        "ssh-asset" => AssetSshProxySpec::SshAsset {
            asset_id: draft.proxy_ssh_asset_id.clone(),
        },
        _ => AssetSshProxySpec::None,
    }
}

fn saved_ssh_credential_ref_for_modal(
    asset_id: &str,
    existing_spec: Option<&AssetSshConnectionSpec>,
) -> String {
    existing_spec
        .and_then(|spec| spec.credential_ref.clone())
        .unwrap_or_else(|| {
            crate::app::ssh::credentials::ssh_credential_ref(
                asset_id,
                crate::app::ssh::credentials::SshCredentialKind::SavedSecrets,
            )
        })
}

fn active_edit_ssh_asset_id(state: &ShellViewModel) -> Option<String> {
    let Some(AssetModalState::NewSshConnection {
        editing_asset_id: Some(asset_id),
        ..
    }) = &state.asset_modal_state
    else {
        return None;
    };

    Some(asset_id.clone())
}

fn resolve_edit_ssh_modal_secret_hydration(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> EditSshModalSecretHydration {
    let proxy_socks5_password =
        match resolve_proxy_socks5_secret_hydration(profile, credential_store) {
            Ok(password) => password,
            Err(message) => {
                return EditSshModalSecretHydration {
                    inline_error: Some(message),
                    ..EditSshModalSecretHydration::default()
                };
            }
        };
    let secret_label = match profile.auth_method {
        SshAuthMethod::Password => "SSH password secret",
        SshAuthMethod::PrivateKeyContent => "SSH inline private key secret",
        SshAuthMethod::PrivateKeyPath => "SSH passphrase secret",
    };

    let stored_bundle = match load_optional_stored_secret_bundle(profile, credential_store) {
        Ok(stored_bundle) => stored_bundle,
        Err(err) => {
            return EditSshModalSecretHydration {
                inline_error: Some(stored_secret_lookup_message(profile, secret_label, &err)),
                ..EditSshModalSecretHydration::default()
            };
        }
    };

    match profile.auth_method {
        SshAuthMethod::Password => {
            let Some((credential_ref, bundle)) = stored_bundle else {
                return EditSshModalSecretHydration {
                    inline_error: Some(stored_secret_lookup_message(
                        profile,
                        secret_label,
                        &StoredSecretLookupError::MissingCredentialRef,
                    )),
                    ..EditSshModalSecretHydration::default()
                };
            };

            match required_secret_bundle_field(&bundle, &credential_ref, "password") {
                Ok(password) => EditSshModalSecretHydration {
                    password: Some(password),
                    proxy_socks5_password,
                    ..EditSshModalSecretHydration::default()
                },
                Err(err) => EditSshModalSecretHydration {
                    proxy_socks5_password,
                    inline_error: Some(stored_secret_lookup_message(profile, secret_label, &err)),
                    ..EditSshModalSecretHydration::default()
                },
            }
        }
        SshAuthMethod::PrivateKeyContent => {
            let Some((credential_ref, bundle)) = stored_bundle else {
                return EditSshModalSecretHydration {
                    inline_error: Some(stored_secret_lookup_message(
                        profile,
                        secret_label,
                        &StoredSecretLookupError::MissingCredentialRef,
                    )),
                    ..EditSshModalSecretHydration::default()
                };
            };
            let passphrase = non_empty_saved_secret(bundle.passphrase.as_deref());

            match required_secret_bundle_field(&bundle, &credential_ref, "private_key_content") {
                Ok(private_key_content) => EditSshModalSecretHydration {
                    private_key_content: Some(private_key_content),
                    passphrase,
                    proxy_socks5_password,
                    ..EditSshModalSecretHydration::default()
                },
                Err(err) => EditSshModalSecretHydration {
                    passphrase,
                    proxy_socks5_password,
                    inline_error: Some(stored_secret_lookup_message(profile, secret_label, &err)),
                    ..EditSshModalSecretHydration::default()
                },
            }
        }
        SshAuthMethod::PrivateKeyPath => EditSshModalSecretHydration {
            passphrase: stored_bundle
                .and_then(|(_, bundle)| non_empty_saved_secret(bundle.passphrase.as_deref())),
            proxy_socks5_password,
            ..EditSshModalSecretHydration::default()
        },
    }
}

fn hydrate_edit_ssh_modal_secret_from_store(
    state: &mut ShellViewModel,
    credential_store: &dyn CredentialStore,
) {
    let Some(asset_id) = active_edit_ssh_asset_id(state) else {
        return;
    };

    let hydration = match profile_for_saved_asset(state, &asset_id) {
        Ok(profile) => resolve_edit_ssh_modal_secret_hydration(&profile, credential_store),
        Err(err) => EditSshModalSecretHydration {
            inline_error: Some(err.to_string()),
            ..EditSshModalSecretHydration::default()
        },
    };

    state.update_ssh_modal_field(
        "proxy_socks5_password",
        hydration.proxy_socks5_password.clone().unwrap_or_default(),
    );
    state.hydrate_edit_ssh_modal_secret(
        hydration.password,
        hydration.private_key_content,
        hydration.passphrase,
        hydration.inline_error,
    );
}

fn temporary_session_asset_id_for_profile(profile: &ConnectionProfile) -> String {
    profile.temporary_session_asset_id()
}

fn saved_secret_bundle_for_draft(draft: &AssetSshConnectionDraft) -> StoredSshSecretBundle {
    let proxy_socks5_password = if matches!(draft.proxy_type.as_str(), "socks5" | "http") {
        (!draft.proxy_socks5_password.trim().is_empty())
            .then(|| draft.proxy_socks5_password.clone())
    } else {
        None
    };
    if draft.auth_source == crate::shell::assets::SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
        return StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password,
        };
    }
    match draft.auth_method.as_str() {
        "password" => StoredSshSecretBundle {
            password: (!draft.password.trim().is_empty()).then(|| draft.password.clone()),
            private_key_content: None,
            passphrase: None,
            proxy_socks5_password,
        },
        "private-key" if draft.private_key_source == "content" => StoredSshSecretBundle {
            password: None,
            private_key_content: (!draft.private_key_content.trim().is_empty())
                .then(|| draft.private_key_content.clone()),
            passphrase: (!draft.passphrase.trim().is_empty()).then(|| draft.passphrase.clone()),
            proxy_socks5_password,
        },
        "private-key" if draft.private_key_source == "path" => StoredSshSecretBundle {
            password: None,
            private_key_content: None,
            passphrase: (!draft.passphrase.trim().is_empty()).then(|| draft.passphrase.clone()),
            proxy_socks5_password,
        },
        _ => StoredSshSecretBundle::default(),
    }
}

fn resolve_proxy_socks5_secret_hydration(
    profile: &ConnectionProfile,
    credential_store: &dyn CredentialStore,
) -> std::result::Result<Option<String>, String> {
    let Some((credential_ref, secret_label)) = (match &profile.proxy {
        ConnectionProxyProfile::Socks5 {
            credential_ref: Some(credential_ref),
            ..
        } => Some((credential_ref.as_str(), "SOCKS5 proxy password secret")),
        ConnectionProxyProfile::Http {
            credential_ref: Some(credential_ref),
            ..
        } => Some((credential_ref.as_str(), "HTTP proxy password secret")),
        _ => None,
    }) else {
        return Ok(None);
    };

    let bundle = load_secret_bundle_with_diagnostics(credential_store, Some(credential_ref))
        .map_err(|err| stored_secret_lookup_message(profile, secret_label, &err))?;

    required_secret_bundle_field(&bundle, credential_ref, "proxy_socks5_password")
        .map(Some)
        .map_err(|err| stored_secret_lookup_message(profile, secret_label, &err))
}

fn sync_saved_ssh_secrets(
    store: &dyn CredentialStore,
    draft: &AssetSshConnectionDraft,
    existing_spec: Option<&AssetSshConnectionSpec>,
    saved_spec: &AssetSshConnectionSpec,
) -> Result<()> {
    let previous_ref = existing_spec.and_then(|spec| spec.credential_ref.as_deref());
    let next_ref = saved_spec.credential_ref.as_deref();

    if next_ref.is_none() {
        if let Some(previous_ref) = previous_ref {
            store.delete_secret(previous_ref)?;
        }
        return Ok(());
    }

    let next_ref = next_ref.expect("checked credential ref");
    let next_bundle = saved_secret_bundle_for_draft(draft);
    persist_secret_bundle(store, next_ref, &next_bundle)?;

    if let Some(previous_ref) = previous_ref
        && previous_ref != next_ref
    {
        store.delete_secret(previous_ref)?;
    }

    Ok(())
}

fn import_private_key_into_ssh_modal(
    state: &mut ShellViewModel,
    private_key_importer: &dyn PrivateKeyImporter,
) -> Result<()> {
    let Some(AssetModalState::NewSshConnection { .. }) = state.asset_modal_state.as_ref() else {
        return Ok(());
    };

    let Some(imported) = private_key_importer.import_private_key()? else {
        return Ok(());
    };

    state.update_ssh_modal_field("auth_method", "private-key".into());
    state.update_ssh_modal_field("private_key_source", "content".into());
    state.update_ssh_modal_field("private_key_path", String::new());
    state.update_ssh_modal_field("private_key_content", imported.content);
    Ok(())
}

fn apply_keychain_private_key_material(state: &mut ShellViewModel, private_key: String) {
    state.update_keychain_ssh_key_modal_field("private_key", private_key.clone());
    match derive_public_key_material_from_private_key(&private_key) {
        Ok(derived) => {
            state.update_keychain_ssh_key_modal_field("public_key", derived.public_key);
            state.update_keychain_ssh_key_modal_field("fingerprint", derived.fingerprint);
        }
        Err(_) => {
            state.update_keychain_ssh_key_modal_field("public_key", String::new());
            state.update_keychain_ssh_key_modal_field("fingerprint", String::new());
        }
    }
}

fn apply_keychain_public_key_material(state: &mut ShellViewModel, public_key: String) {
    let trimmed = public_key.trim().to_string();
    state.update_keychain_ssh_key_modal_field("public_key", trimmed.clone());
    match derive_public_key_material_from_public_key(&trimmed) {
        Ok(derived) => {
            state.update_keychain_ssh_key_modal_field("public_key", derived.public_key);
            state.update_keychain_ssh_key_modal_field("fingerprint", derived.fingerprint);
        }
        Err(_) => {
            state.update_keychain_ssh_key_modal_field("fingerprint", String::new());
        }
    }
}

fn import_private_key_into_keychain_modal(
    state: &mut ShellViewModel,
    private_key_importer: &dyn PrivateKeyImporter,
) -> Result<()> {
    let Some(AssetModalState::NewKeychainSshKey { .. }) = state.asset_modal_state.as_ref() else {
        return Ok(());
    };
    let Some(imported) = private_key_importer.import_private_key()? else {
        return Ok(());
    };
    apply_keychain_private_key_material(state, imported.content);
    Ok(())
}

fn import_public_key_into_keychain_modal(
    state: &mut ShellViewModel,
    private_key_importer: &dyn PrivateKeyImporter,
) -> Result<()> {
    let Some(AssetModalState::NewKeychainSshKey { .. }) = state.asset_modal_state.as_ref() else {
        return Ok(());
    };
    let Some(imported) = private_key_importer.import_private_key()? else {
        return Ok(());
    };
    apply_keychain_public_key_material(state, imported.content);
    Ok(())
}

fn paste_private_key_into_keychain_modal(state: &mut ShellViewModel) {
    let Some(text) = system_clipboard_text() else {
        return;
    };
    apply_keychain_private_key_material(state, text);
}

fn paste_public_key_into_keychain_modal(state: &mut ShellViewModel) {
    let Some(text) = system_clipboard_text() else {
        return;
    };
    apply_keychain_public_key_material(state, text);
}

fn generate_key_pair_into_keychain_modal(state: &mut ShellViewModel) -> Result<()> {
    let Some(AssetModalState::NewKeychainSshKey { .. }) = state.asset_modal_state.as_ref() else {
        return Ok(());
    };
    let private_key =
        PrivateKey::random(&mut OsRng, Algorithm::Ed25519).context("failed to generate SSH key")?;
    let private_key_text = private_key
        .to_openssh(LineEnding::LF)
        .context("failed to encode generated SSH private key")?
        .to_string();
    apply_keychain_private_key_material(state, private_key_text);
    Ok(())
}

fn copy_public_key_from_keychain_modal(state: &ShellViewModel) -> Result<()> {
    let Some(AssetModalState::NewKeychainSshKey { draft, .. }) = state.asset_modal_state.as_ref()
    else {
        return Ok(());
    };
    if draft.public_key.trim().is_empty() {
        return Ok(());
    }
    set_system_clipboard_text(draft.public_key.trim())
}

fn persist_keychain_ssh_key_secret(
    credential_store: &dyn CredentialStore,
    key_id: &str,
    draft: &KeychainSshKeyDraft,
) -> Result<()> {
    let bundle = StoredKeychainKeySecretBundle {
        private_key_content: (!draft.private_key.trim().is_empty())
            .then(|| draft.private_key.clone()),
        passphrase: None,
    };
    let credential_ref = format!("keychain/key/{key_id}");
    persist_keychain_key_secret_bundle(credential_store, credential_ref.as_str(), &bundle)
}

fn merge_session_handle_into_tabs(state: &mut ShellViewModel, handle: &SessionHandle) {
    let mut tabs = state.workspace_tabs().to_vec();
    let next_tab = WorkspaceTab::from_session(handle);

    if let Some(existing) = tabs
        .iter_mut()
        .find(|tab| tab.session_id == next_tab.session_id)
    {
        *existing = next_tab;
    } else {
        tabs.push(next_tab);
    }

    state.set_workspace_tabs(tabs);
    let _ = state.activate_workspace_session(handle.session_id.to_string().as_str());
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WorkspaceProjectionDelta {
    tabs_changed: bool,
    surface_changed: bool,
    sftp_changed: bool,
}

impl WorkspaceProjectionDelta {
    fn any_changed(self) -> bool {
        self.tabs_changed || self.surface_changed || self.sftp_changed
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WorkspaceFollowIndicator {
    paused: bool,
    pending_output_lines: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct WorkspaceFollowSessionState {
    last_surface_seqno: usize,
    last_viewport_offset_lines: u32,
    pending_output_lines: u32,
}

#[derive(Debug, Default)]
struct WorkspaceFollowTracker {
    by_session: HashMap<Uuid, WorkspaceFollowSessionState>,
}

impl WorkspaceFollowTracker {
    fn indicator_for_surface(
        &mut self,
        surface: Option<&TerminalSurfaceState>,
    ) -> WorkspaceFollowIndicator {
        let Some(surface) = surface else {
            return WorkspaceFollowIndicator::default();
        };

        let session = self.by_session.entry(surface.session_id).or_default();
        if surface.viewport_at_bottom {
            session.pending_output_lines = 0;
        } else if session.last_surface_seqno != 0 && surface.seqno != session.last_surface_seqno {
            let appended_lines = surface
                .viewport_offset_lines
                .saturating_sub(session.last_viewport_offset_lines);
            session.pending_output_lines =
                session.pending_output_lines.saturating_add(appended_lines);
        }

        session.last_surface_seqno = surface.seqno;
        session.last_viewport_offset_lines = surface.viewport_offset_lines;

        WorkspaceFollowIndicator {
            paused: !surface.viewport_at_bottom,
            pending_output_lines: session.pending_output_lines,
        }
    }
}

fn projected_active_workspace_session_id(
    state: &ShellViewModel,
    next_tabs: &[WorkspaceTab],
) -> Option<String> {
    state
        .active_workspace_session_id()
        .filter(|candidate| next_tabs.iter().any(|tab| tab.session_id == *candidate))
        .map(str::to_string)
        .or_else(|| {
            state
                .workspace_tabs()
                .iter()
                .find(|tab| {
                    tab.active
                        && next_tabs
                            .iter()
                            .any(|candidate| candidate.session_id == tab.session_id)
                })
                .map(|tab| tab.session_id.clone())
        })
        .or_else(|| next_tabs.first().map(|tab| tab.session_id.clone()))
}

fn sync_workspace_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> WorkspaceProjectionDelta {
    let mut next_tabs = manager
        .ordered_sessions()
        .into_iter()
        .map(|handle| WorkspaceTab::from_session(&handle))
        .collect::<Vec<_>>();
    let manager_session_ids = next_tabs
        .iter()
        .map(|tab| tab.session_id.clone())
        .collect::<HashSet<_>>();
    let manager_asset_ids = next_tabs
        .iter()
        .map(|tab| tab.asset_id.clone())
        .collect::<HashSet<_>>();
    let preserved_error_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| {
            tab.state == "error"
                && !manager_session_ids.contains(&tab.session_id)
                && !manager_asset_ids.contains(&tab.asset_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let preserved_launcher_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| tab.is_launcher())
        .cloned()
        .collect::<Vec<_>>();
    next_tabs.extend(preserved_error_tabs);
    next_tabs.extend(preserved_launcher_tabs);
    let active_id = projected_active_workspace_session_id(state, &next_tabs);
    for tab in &mut next_tabs {
        tab.active = active_id.as_deref() == Some(tab.session_id.as_str());
    }
    let next_session_id = state
        .active_workspace_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok());
    let current_surface_signature = state
        .active_workspace_terminal_surface()
        .map(TerminalSurfaceState::signature);
    let next_surface_signature =
        next_session_id.and_then(|session_id| manager.terminal_surface_signature(session_id));

    let tabs_changed = state.workspace_tabs() != next_tabs.as_slice();
    if tabs_changed {
        state.set_workspace_tabs(next_tabs);
    }

    let surface_changed = current_surface_signature != next_surface_signature;
    if surface_changed {
        let next_surface =
            next_session_id.and_then(|session_id| manager.terminal_surface(session_id));
        state.set_active_workspace_terminal_surface(next_surface);
    }

    let sftp_changed = sync_active_sftp_projection_from_manager(state, manager);

    WorkspaceProjectionDelta {
        tabs_changed,
        surface_changed,
        sftp_changed,
    }
}

fn sync_active_sftp_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> bool {
    let Some(session_id_text) = state.active_workspace_session_id().map(str::to_string) else {
        return false;
    };
    let Some(session_id) = Uuid::parse_str(&session_id_text).ok() else {
        return false;
    };

    let binding = manager.sftp_binding(session_id);
    let cwd = manager.current_working_directory(session_id);
    let Some(binding) = binding else {
        return false;
    };

    let session_state = state.sftp_sessions.entry(session_id_text).or_default();
    let before = session_state.clone();

    match binding.mode() {
        SftpPanelMode::Disconnected => session_state.mark_disconnected(),
        _ if matches!(
            session_state.mode,
            SftpPanelMode::Empty | SftpPanelMode::Disconnected
        ) =>
        {
            session_state.mark_connecting()
        }
        _ => {}
    }

    if let Some(cwd) = cwd {
        if session_state.current_path.is_empty() {
            session_state.reenable_follow(cwd);
        } else if session_state.follow_mode == SftpFollowMode::FollowCwd {
            session_state.follow_terminal_path(cwd);
        }
    }

    before != *session_state
}

fn project_sftp_browser_state_into_view_model(
    state: &mut ShellViewModel,
    session_id: Uuid,
    browser_state: &SftpBrowserSessionState,
) -> bool {
    let next = SftpSessionBindingState {
        mode: browser_state.mode,
        follow_mode: browser_state.follow_mode,
        current_path: browser_state.current_path.clone(),
        history: browser_state.history.clone(),
        entries: browser_state.entries.clone(),
        selected_entry_ids: browser_state.selected_entry_ids.clone(),
        last_error: browser_state.last_error.clone(),
    };
    let session_id_text = session_id.to_string();
    if state.sftp_sessions.get(&session_id_text) == Some(&next) {
        return false;
    }
    state.set_sftp_session_state(session_id_text, next);
    true
}

fn execute_sftp_browser_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    request: SftpBrowserLoadRequest,
) -> bool {
    match manager.sftp_read_dir(request.session_id, request.path.as_str()) {
        Ok(entries) => controller.apply_loaded_directory(
            request.session_id,
            request.request_id,
            request.path.as_str(),
            entries,
        ),
        Err(err) => {
            if manager
                .sftp_binding(request.session_id)
                .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
            {
                controller.mark_disconnected(request.session_id);
            } else {
                controller.apply_load_error(
                    request.session_id,
                    request.request_id,
                    request.path.as_str(),
                    err.to_string(),
                );
            }
        }
    }

    controller
        .session_state(request.session_id)
        .is_some_and(|browser_state| {
            project_sftp_browser_state_into_view_model(state, request.session_id, browser_state)
        })
}

fn sftp_remote_file_title(remote_path: &str) -> String {
    remote_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Remote File")
        .to_string()
}

fn open_sftp_remote_file_editor_for_entry(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    session_id: Uuid,
    remote_path: &str,
) {
    match manager.sftp_download_file(session_id, remote_path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                text,
                "Editing remote text file".to_string(),
                String::new(),
            ),
            Err(err) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                String::from_utf8_lossy(err.as_bytes()).into_owned(),
                "View only".to_string(),
                "Only UTF-8 text files can be edited online right now.".to_string(),
            ),
        },
        Err(err) => state.open_sftp_remote_file_editor(
            session_id.to_string(),
            remote_path.to_string(),
            sftp_remote_file_title(remote_path),
            String::new(),
            "Open failed".to_string(),
            format!("Failed to open remote file: {err}"),
        ),
    }
}

fn initial_sftp_browser_path(manager: &SessionManager, session_id: Uuid) -> Option<String> {
    if let Some(cwd) = manager.current_working_directory(session_id) {
        return Some(cwd);
    }

    manager
        .sftp_binding(session_id)
        .filter(|binding| binding.mode() != SftpPanelMode::Disconnected)
        .map(|_| "/".to_string())
}

fn ensure_active_sftp_browser_started(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_some() {
        return false;
    }

    initial_sftp_browser_path(manager, session_id).is_some_and(|path| {
        let request = controller.open(session_id, path.as_str());
        execute_sftp_browser_request(state, controller, manager, request)
    })
}

fn open_active_sftp_browser_for_current_session(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_none() {
        return ensure_active_sftp_browser_started(state, controller, manager);
    }

    let request = if controller.session_state(session_id).is_some() {
        controller.session_activated(session_id)
    } else {
        None
    };
    request.is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

fn sync_active_sftp_browser_follow_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return false;
    };

    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        controller.mark_disconnected(session_id);
        return controller
            .session_state(session_id)
            .is_some_and(|browser_state| {
                project_sftp_browser_state_into_view_model(state, session_id, browser_state)
            });
    }

    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.follow_mode != SftpFollowMode::FollowCwd {
        return false;
    }

    let Some(cwd) = manager.current_working_directory(session_id) else {
        return false;
    };
    if browser_state.current_path == cwd {
        return false;
    }

    controller
        .follow_cwd(session_id, cwd.as_str())
        .is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

fn sync_active_sftp_browser_pending_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return false;
    };
    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.mode != SftpPanelMode::Connecting {
        return false;
    }
    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    controller
        .pending_request(session_id)
        .is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

fn active_workspace_session_uuid(state: &ShellViewModel) -> Option<Uuid> {
    state
        .active_workspace_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

fn snap_active_workspace_viewport_to_bottom_if_needed(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };
    let needs_snap = state
        .active_workspace_terminal_surface()
        .is_some_and(|surface| !surface.viewport_at_bottom);
    if !needs_snap {
        return;
    }

    if let Err(err) = bridge.manager.scroll_session_to_bottom(session_id) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to snap workspace terminal viewport to bottom"
        );
    }
}

fn refresh_active_workspace_projection(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some(bridge) = bridge else {
        return;
    };

    let projection = sync_workspace_projection_from_manager(state, &bridge.manager);
    if projection.any_changed() {
        sync_workspace_tabs_with_manager(window, state, follow_tracker, Some(&bridge.manager));
        if projection.sftp_changed {
            sync_right_panel_state(window, state);
        }
    }
}

fn forward_active_workspace_text_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    text: &str,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };
    if text.is_empty() {
        return;
    }

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));

    if let Err(err) = bridge
        .manager
        .send_session_text_input(session_id, text.to_string())
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to forward workspace terminal text input"
        );
    }
}

fn forward_active_workspace_key_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let Some(event) = terminal_key_event(key_name, alt, ctrl, shift) else {
        return;
    };

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));

    if let Err(err) = bridge.manager.send_session_key_input(session_id, event) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            key = key_name,
            error = %err,
            "failed to forward workspace terminal key input"
        );
    }
}

fn terminal_key_event(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Option<TerminalKeyEvent> {
    if let Some(number) = key_name
        .strip_prefix('f')
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .filter(|number| (1..=24).contains(number))
    {
        return Some(TerminalKeyEvent::function(number, alt, ctrl, shift));
    }

    if key_name.chars().count() == 1 {
        return key_name
            .chars()
            .next()
            .map(|ch| TerminalKeyEvent::character(ch, alt, ctrl, shift));
    }

    match key_name {
        "enter" => Some(TerminalKeyEvent::named("enter", alt, ctrl, shift)),
        "tab" => Some(TerminalKeyEvent::named("tab", alt, ctrl, shift)),
        "escape" => Some(TerminalKeyEvent::named("escape", alt, ctrl, shift)),
        "backspace" => Some(TerminalKeyEvent::named("backspace", alt, ctrl, shift)),
        "insert" => Some(TerminalKeyEvent::named("insert", alt, ctrl, shift)),
        "delete" => Some(TerminalKeyEvent::named("delete", alt, ctrl, shift)),
        "up" => Some(TerminalKeyEvent::named("up", alt, ctrl, shift)),
        "down" => Some(TerminalKeyEvent::named("down", alt, ctrl, shift)),
        "left" => Some(TerminalKeyEvent::named("left", alt, ctrl, shift)),
        "right" => Some(TerminalKeyEvent::named("right", alt, ctrl, shift)),
        "home" => Some(TerminalKeyEvent::named("home", alt, ctrl, shift)),
        "end" => Some(TerminalKeyEvent::named("end", alt, ctrl, shift)),
        "page-up" => Some(TerminalKeyEvent::named("page-up", alt, ctrl, shift)),
        "page-down" => Some(TerminalKeyEvent::named("page-down", alt, ctrl, shift)),
        _ => None,
    }
}

fn forward_active_workspace_resize(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    rows: i32,
    cols: i32,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let rows = rows.max(1) as u32;
    let cols = cols.max(1) as u32;
    if let Err(err) = bridge.manager.resize_session(session_id, rows, cols) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            rows,
            cols,
            error = %err,
            "failed to forward workspace terminal resize"
        );
    }
}

fn set_system_clipboard_text(text: &str) -> Result<()> {
    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(text, slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .map_err(anyhow::Error::from)
}

fn system_clipboard_text() -> Option<String> {
    i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .ok()
    .flatten()
}

fn forward_active_workspace_copy_selection(
    state: &ShellViewModel,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
) {
    let Some(surface) = state.active_workspace_terminal_surface() else {
        return;
    };

    let text = surface.selection_text(
        start_row.max(0) as u32,
        start_col.max(0) as u32,
        end_row.max(0) as u32,
        end_col.max(0) as u32,
    );
    if text.is_empty() {
        return;
    }

    if let Err(err) = set_system_clipboard_text(&text) {
        tracing::error!(
            target: "app.ssh",
            error = %err,
            "failed to copy workspace terminal selection to clipboard"
        );
    }
}

fn normalized_paste_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn workspace_paste_logical_line_count(text: &str) -> usize {
    let normalized = normalized_paste_newlines(text);
    let trimmed = normalized.trim_end_matches('\n');
    if trimmed.is_empty() {
        return usize::from(!text.is_empty());
    }

    trimmed.split('\n').count()
}

fn workspace_paste_prompt_mode(
    state: &ShellViewModel,
    text: &str,
) -> Option<WorkspacePastePromptMode> {
    let logical_line_count = workspace_paste_logical_line_count(text);
    if logical_line_count < 2 {
        return None;
    }

    if logical_line_count >= WORKSPACE_PASTE_EDITOR_LINE_THRESHOLD {
        return Some(WorkspacePastePromptMode::Editor);
    }

    if state
        .active_workspace_terminal_surface()
        .is_some_and(|surface| surface.bracketed_paste_enabled)
    {
        None
    } else {
        Some(WorkspacePastePromptMode::Confirm)
    }
}

fn forward_workspace_session_paste(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    session_id: Uuid,
    text: &str,
) {
    let Some(bridge) = bridge else {
        return;
    };
    if text.is_empty() {
        return;
    }

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));

    if let Err(err) = bridge
        .manager
        .send_session_paste(session_id, text.to_string())
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to forward workspace terminal paste"
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspacePasteRequestOutcome {
    Ignored,
    Prompted,
    Sent,
}

fn forward_active_workspace_paste(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    pending_warning: &RefCell<Option<PendingWorkspacePasteWarning>>,
) -> WorkspacePasteRequestOutcome {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return WorkspacePasteRequestOutcome::Ignored;
    };
    let Some(text) = system_clipboard_text() else {
        return WorkspacePasteRequestOutcome::Ignored;
    };

    if let Some(prompt_mode) = workspace_paste_prompt_mode(state, &text) {
        *pending_warning.borrow_mut() = Some(PendingWorkspacePasteWarning {
            session_id,
            logical_line_count: workspace_paste_logical_line_count(&text),
            text,
            prompt_mode,
        });
        return WorkspacePasteRequestOutcome::Prompted;
    }

    pending_warning.borrow_mut().take();
    forward_workspace_session_paste(state, bridge, session_id, &text);
    WorkspacePasteRequestOutcome::Sent
}

fn forward_active_workspace_scroll_ratio(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    ratio: f32,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    if let Err(err) = bridge.manager.scroll_session_to_ratio(session_id, ratio) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            ratio,
            error = %err,
            "failed to update workspace terminal scrollback ratio"
        );
    }
}

fn forward_active_workspace_mouse_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    event: TerminalMouseInput,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));

    if let Err(err) = bridge.manager.send_session_mouse_input(session_id, event) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            row = event.row,
            col = event.col,
            error = %err,
            "failed to forward workspace terminal mouse input"
        );
    }
}

struct WorkspaceScrollInput {
    delta_lines: i32,
    row: i32,
    col: i32,
    shift: bool,
    ctrl: bool,
    alt: bool,
}

fn forward_active_workspace_scroll(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    input: WorkspaceScrollInput,
) {
    if input.delta_lines == 0 {
        return;
    }

    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let mouse_grabbed = state
        .active_workspace_terminal_surface()
        .map(|surface| surface.mouse_grabbed)
        .unwrap_or(false);

    if mouse_grabbed {
        let button = if input.delta_lines > 0 {
            TerminalMouseButton::WheelUp
        } else {
            TerminalMouseButton::WheelDown
        };
        let event = TerminalMouseInput {
            kind: TerminalMouseEventKind::Scroll,
            button,
            row: input.row.max(0) as u32,
            col: input.col.max(0) as u32,
            shift: input.shift,
            ctrl: input.ctrl,
            alt: input.alt,
        };
        if let Err(err) = bridge.manager.send_session_mouse_input(session_id, event) {
            tracing::error!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                delta_lines = input.delta_lines,
                row = input.row,
                col = input.col,
                error = %err,
                "failed to forward workspace terminal wheel input"
            );
        }
        return;
    }

    if let Err(err) = bridge
        .manager
        .scroll_session_viewport(session_id, input.delta_lines)
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            delta_lines = input.delta_lines,
            error = %err,
            "failed to update workspace terminal local scrollback"
        );
    }
}

fn parse_terminal_mouse_kind(value: &str) -> Option<TerminalMouseEventKind> {
    match value {
        "down" => Some(TerminalMouseEventKind::Down),
        "up" => Some(TerminalMouseEventKind::Up),
        "move" => Some(TerminalMouseEventKind::Move),
        _ => None,
    }
}

fn parse_terminal_mouse_button(value: &str) -> Option<TerminalMouseButton> {
    match value {
        "left" => Some(TerminalMouseButton::Left),
        "middle" => Some(TerminalMouseButton::Middle),
        "right" => Some(TerminalMouseButton::Right),
        "none" => Some(TerminalMouseButton::None),
        _ => None,
    }
}

fn open_session_with_profile(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    profile: ConnectionProfile,
    mode: OpenSessionMode,
) -> anyhow::Result<()> {
    let handle = bridge.manager.open_session(profile, mode)?;
    let resolved = bridge.manager.session(handle.session_id).unwrap_or(handle);
    merge_session_handle_into_tabs(state, &resolved);
    let _ = sync_workspace_projection_from_manager(state, &bridge.manager);
    Ok(())
}

fn show_failed_session_tab(
    state: &mut ShellViewModel,
    profile: &ConnectionProfile,
    message: impl Into<String>,
) {
    let asset_id = profile
        .asset_id
        .clone()
        .unwrap_or_else(|| format!("session-error:{}", Uuid::new_v4()));
    let handle = SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id,
        title: profile.name.clone(),
        subtitle: format!("{}@{}:{}", profile.user, profile.host, profile.port),
        state: SessionState::Error(message.into()),
        can_reconnect: true,
        enhanced_session_state: EnhancedSessionState::Plain,
    };
    merge_session_handle_into_tabs(state, &handle);
}

fn show_failed_saved_asset_tab(
    state: &mut ShellViewModel,
    asset_id: &str,
    message: impl Into<String>,
) {
    let (title, subtitle) = match (
        state.console_asset_tree().node(asset_id),
        state.console_asset_tree().ssh_connection_spec(asset_id),
    ) {
        (Some(node), Some(spec)) => {
            let port = if spec.port.trim().is_empty() {
                "22"
            } else {
                spec.port.trim()
            };
            (
                node.title.clone(),
                format!("{}@{}:{}", spec.user.trim(), spec.host.trim(), port),
            )
        }
        (Some(node), None) => (node.title.clone(), String::new()),
        _ => ("SSH Connection".into(), String::new()),
    };

    let handle = SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: asset_id.to_string(),
        title,
        subtitle,
        state: SessionState::Error(message.into()),
        can_reconnect: true,
        enhanced_session_state: EnhancedSessionState::Plain,
    };
    merge_session_handle_into_tabs(state, &handle);
}

fn prompt_unknown_host_key(
    state: &mut ShellViewModel,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    profile: ConnectionProfile,
    error: &UnknownHostKeyError,
    intent: HostKeyApprovalIntent,
) {
    pending_host_key_approval
        .borrow_mut()
        .replace(PendingHostKeyApproval {
            profile,
            public_key_openssh: error.public_key_openssh.clone(),
            intent,
        });
    state.open_ssh_host_key_prompt(
        format!("{}:{}", error.host, error.port),
        error.fingerprint.clone(),
    );
}

fn attempt_test_connection(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    profile: ConnectionProfile,
) {
    match bridge.manager.probe_connection(profile.clone()) {
        Ok(()) => state.finish_ssh_modal_action_success("Connection test succeeded."),
        Err(err) => {
            if let Some(unknown) = err.downcast_ref::<UnknownHostKeyError>() {
                prompt_unknown_host_key(
                    state,
                    pending_host_key_approval,
                    profile,
                    unknown,
                    HostKeyApprovalIntent::ModalTestConnection,
                );
            } else {
                tracing::error!(
                    target: "app.ssh",
                    error = %err,
                    "ssh probe failed"
                );
                state.finish_ssh_modal_action_error(err.to_string());
            }
        }
    }
}

fn attempt_open_session_with_profile(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    profile: ConnectionProfile,
    mode: OpenSessionMode,
) -> anyhow::Result<()> {
    if matches!(mode, OpenSessionMode::ActivateExisting)
        && let Some(asset_id) = profile.asset_id.as_deref()
        && target_session_id_for_asset(state, asset_id).is_some()
    {
        return open_session_with_profile(state, bridge, profile, mode);
    }

    match bridge.manager.probe_connection(profile.clone()) {
        Ok(()) => {
            open_session_with_profile(state, bridge, profile.clone(), mode).inspect_err(|err| {
                show_failed_session_tab(state, &profile, err.to_string());
            })
        }
        Err(err) => {
            if let Some(unknown) = err.downcast_ref::<UnknownHostKeyError>() {
                prompt_unknown_host_key(
                    state,
                    pending_host_key_approval,
                    profile,
                    unknown,
                    HostKeyApprovalIntent::OpenSession(mode),
                );
                Ok(())
            } else {
                tracing::error!(
                    target: "app.ssh",
                    error = %err,
                    "ssh probe failed before opening workspace session"
                );
                show_failed_session_tab(state, &profile, err.to_string());
                Err(err)
            }
        }
    }
}

fn register_asset_click(
    tracker: &Rc<RefCell<Option<PendingAssetClick>>>,
    asset_id: &str,
    now: Instant,
) -> bool {
    const DOUBLE_CLICK_WINDOW: Duration = Duration::from_millis(350);

    let should_activate = tracker
        .borrow()
        .as_ref()
        .map(|pending| {
            pending.asset_id == asset_id
                && now.duration_since(pending.clicked_at) <= DOUBLE_CLICK_WINDOW
        })
        .unwrap_or(false);

    if should_activate {
        tracker.borrow_mut().take();
    } else {
        tracker.borrow_mut().replace(PendingAssetClick {
            asset_id: asset_id.to_string(),
            clicked_at: now,
        });
    }

    should_activate
}

fn activate_asset(
    state: &mut ShellViewModel,
    session_bridge: Option<&ShellSessionBridge>,
    _pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    asset_id: &str,
) {
    match state.asset_kind(asset_id) {
        Some(crate::shell::assets::ConsoleAssetKind::Folder) => {
            state.toggle_folder_expanded(asset_id);
        }
        Some(crate::shell::assets::ConsoleAssetKind::SshConnection) => {
            match runtime_profile_for_saved_asset(state, asset_id) {
                Ok(profile) => {
                    if let Some(session_bridge) = session_bridge {
                        if let Err(err) = open_session_with_profile(
                            state,
                            session_bridge,
                            profile,
                            OpenSessionMode::ForceNewTab,
                        ) {
                            tracing::error!(
                                target: "app.ssh",
                                asset_id = asset_id,
                                error = %err,
                                "failed to open ssh session for activated asset"
                            );
                        }
                    } else {
                        let message = "SSH session bridge is unavailable.";
                        show_failed_saved_asset_tab(state, asset_id, message);
                        tracing::error!(
                            target: "app.ssh",
                            asset_id = asset_id,
                            error = message,
                            "failed to open ssh session for activated asset"
                        );
                    }
                }
                Err(err) => {
                    show_failed_saved_asset_tab(state, asset_id, err.to_string());
                    tracing::error!(
                        target: "app.ssh",
                        asset_id = asset_id,
                        error = %err,
                        "failed to resolve saved ssh profile for activated asset"
                    );
                }
            }
        }
        Some(crate::shell::assets::ConsoleAssetKind::SnippetPackage) => {
            state.toggle_folder_expanded(asset_id);
        }
        Some(crate::shell::assets::ConsoleAssetKind::Snippet) => {
            state.begin_snippet_activation(asset_id, SnippetActivation::Paste);
        }
        None => {}
    }
}

fn open_saved_ssh_asset_from_quick_launch(
    state: &mut ShellViewModel,
    session_bridge: Option<&ShellSessionBridge>,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    asset_id: &str,
    mode: OpenSessionMode,
) {
    let had_existing_session = matches!(mode, OpenSessionMode::ActivateExisting)
        && target_session_id_for_asset(state, asset_id).is_some();

    match runtime_profile_for_saved_asset(state, asset_id) {
        Ok(profile) => {
            if let Some(session_bridge) = session_bridge {
                if let Err(err) = attempt_open_session_with_profile(
                    state,
                    session_bridge,
                    pending_host_key_approval,
                    profile,
                    mode,
                ) {
                    tracing::error!(
                        target: "app.quick_launch",
                        asset_id,
                        error = %err,
                        "failed to open saved ssh asset from quick launch"
                    );
                } else if had_existing_session || state.ssh_host_key_prompt_state.is_none() {
                    state.record_recent_saved_ssh_asset(asset_id);
                }
            } else {
                let message = "SSH session bridge is unavailable.";
                show_failed_saved_asset_tab(state, asset_id, message);
                tracing::error!(
                    target: "app.quick_launch",
                    asset_id,
                    error = message,
                    "failed to open saved ssh asset from quick launch"
                );
            }
        }
        Err(err) => {
            show_failed_saved_asset_tab(state, asset_id, err.to_string());
            tracing::error!(
                target: "app.quick_launch",
                asset_id,
                error = %err,
                "failed to resolve saved ssh profile for quick launch"
            );
        }
    }
}

fn resolve_pending_host_key(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    accept: bool,
) {
    let Some(pending) = pending_host_key_approval.borrow_mut().take() else {
        return;
    };

    let host = pending.profile.host.clone();
    let port = pending.profile.port;

    if accept {
        let result = (|| -> Result<()> {
            let public_key = PublicKey::from_openssh(pending.public_key_openssh.as_str())
                .context("failed to parse accepted SSH host key")?;
            let known_hosts = KnownHostsService::new(default_known_hosts_path()?);
            known_hosts.accept_unknown(host.as_str(), port, &public_key)
        })();

        if let Err(err) = result {
            match pending.intent {
                HostKeyApprovalIntent::ModalTestConnection => {
                    state.finish_ssh_modal_action_error(err.to_string());
                }
                HostKeyApprovalIntent::OpenSession(_) => {
                    show_failed_session_tab(state, &pending.profile, err.to_string());
                }
            }
            return;
        }

        let Some(bridge) = bridge else {
            let message = "SSH session bridge is unavailable.".to_string();
            match pending.intent {
                HostKeyApprovalIntent::ModalTestConnection => {
                    state.finish_ssh_modal_action_error(message);
                }
                HostKeyApprovalIntent::OpenSession(_) => {
                    show_failed_session_tab(state, &pending.profile, message);
                }
            }
            return;
        };

        match pending.intent {
            HostKeyApprovalIntent::ModalTestConnection => {
                attempt_test_connection(state, bridge, pending_host_key_approval, pending.profile);
            }
            HostKeyApprovalIntent::OpenSession(mode) => {
                if let Err(err) = attempt_open_session_with_profile(
                    state,
                    bridge,
                    pending_host_key_approval,
                    pending.profile,
                    mode,
                ) {
                    tracing::error!(
                        target: "app.ssh",
                        error = %err,
                        "failed to open ssh session after host key acceptance"
                    );
                }
            }
        }
        return;
    }

    let message = format!("Rejected unknown SSH host key for `{}`:{}.", host, port);
    match pending.intent {
        HostKeyApprovalIntent::ModalTestConnection => {
            state.finish_ssh_modal_action_error(message);
        }
        HostKeyApprovalIntent::OpenSession(_) => {
            show_failed_session_tab(state, &pending.profile, message);
        }
    }
}

fn target_session_id_for_asset(state: &ShellViewModel, asset_id: &str) -> Option<String> {
    state
        .active_workspace_tab()
        .filter(|tab| tab.asset_id == asset_id)
        .map(|tab| tab.session_id.clone())
        .or_else(|| {
            state
                .workspace_tabs()
                .iter()
                .find(|tab| tab.asset_id == asset_id)
                .map(|tab| tab.session_id.clone())
        })
}

fn close_session_by_id(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    session_id: &str,
) -> bool {
    if let Some(bridge) = bridge
        && let Ok(session_uuid) = Uuid::parse_str(session_id)
    {
        let _ = bridge.manager.close_session(session_uuid);
    }
    state.close_workspace_session_with_fallback(session_id)
}

fn context_menu_roots_for(state: &ShellViewModel) -> Vec<ContextMenuActionNode> {
    let Some(target_kind) = state.context_menu_target_kind else {
        return Vec::new();
    };

    if !state.context_menu_open {
        return Vec::new();
    }

    resolve_action_tree(target_kind, &selection_context_for(state))
}

fn context_menu_columns_for(state: &ShellViewModel) -> [Vec<ContextMenuActionNode>; 3] {
    let roots = context_menu_roots_for(state);
    visible_columns_for_path(&roots, &state.context_menu_open_path)
}

fn context_menu_items_to_model(
    items: Vec<ContextMenuActionNode>,
    open_index: Option<usize>,
) -> Vec<AssetsContextMenuItem> {
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| AssetsContextMenuItem {
            id: item.id.into(),
            label: item.label.into(),
            icon_id: item.icon_id.into(),
            enabled: item.state != ContextMenuActionState::Disabled,
            planned: item.state == ContextMenuActionState::Planned,
            has_children: !item.children.is_empty(),
            open: open_index == Some(index),
            divider_before: item.divider_before,
        })
        .collect()
}

fn context_menu_primary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(
        columns[0].clone(),
        state.context_menu_open_path.first().copied(),
    )
}

fn context_menu_secondary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(
        columns[1].clone(),
        state.context_menu_open_path.get(1).copied(),
    )
}

fn context_menu_tertiary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(columns[2].clone(), None)
}

fn context_menu_hover_path_for(
    state: &ShellViewModel,
    column_index: usize,
    row_index: usize,
) -> Vec<usize> {
    let columns = context_menu_columns_for(state);

    match column_index {
        0 => columns[0]
            .get(row_index)
            .map(|node| {
                if node.children.is_empty() {
                    Vec::new()
                } else {
                    vec![row_index]
                }
            })
            .unwrap_or_default(),
        1 => {
            let Some(first_index) = state.context_menu_open_path.first().copied() else {
                return Vec::new();
            };

            columns[1]
                .get(row_index)
                .map(|node| {
                    if node.children.is_empty() {
                        vec![first_index]
                    } else {
                        vec![first_index, row_index]
                    }
                })
                .unwrap_or_else(|| vec![first_index])
        }
        _ => state.context_menu_open_path.clone(),
    }
}

fn context_menu_action_entry_for(
    state: &ShellViewModel,
    action_id: &str,
) -> Option<(Vec<usize>, ContextMenuActionNode)> {
    find_context_menu_action_entry(&context_menu_roots_for(state), action_id, Vec::new())
}

fn find_context_menu_action_entry(
    nodes: &[ContextMenuActionNode],
    action_id: &str,
    prefix: Vec<usize>,
) -> Option<(Vec<usize>, ContextMenuActionNode)> {
    for (index, node) in nodes.iter().enumerate() {
        let mut path = prefix.clone();
        path.push(index);

        if node.id == action_id {
            return Some((path, node.clone()));
        }

        if let Some(found) = find_context_menu_action_entry(&node.children, action_id, path) {
            return Some(found);
        }
    }

    None
}

fn context_menu_visible_column_count(state: &ShellViewModel) -> usize {
    context_menu_columns_for(state)
        .into_iter()
        .take_while(|column| !column.is_empty())
        .count()
}

fn context_menu_overlay_height_for(state: &ShellViewModel) -> f32 {
    context_menu_columns_for(state)
        .into_iter()
        .filter(|column| !column.is_empty())
        .map(|column| context_menu_column_height(column.as_slice()))
        .fold(0.0, f32::max)
}

fn context_menu_child_width_for(state: &ShellViewModel) -> f32 {
    let child_count = context_menu_visible_column_count(state).saturating_sub(1) as f32;
    if child_count <= 0.0 {
        0.0
    } else {
        child_count * (CONTEXT_MENU_COLUMN_WIDTH + CONTEXT_MENU_COLUMN_GAP)
    }
}

fn context_menu_column_rects_for(state: &ShellViewModel) -> [Option<Rect>; 3] {
    let columns = context_menu_columns_for(state);
    let visible_column_count = columns
        .iter()
        .take_while(|column| !column.is_empty())
        .count();
    let mut rects = [None, None, None];

    for column_index in 0..visible_column_count {
        let height = context_menu_column_height(columns[column_index].as_slice());
        rects[column_index] = Some(Rect {
            x: state.context_menu_origin_x
                + context_menu_column_offset(
                    column_index,
                    visible_column_count,
                    state.context_menu_child_flows_left,
                ),
            y: state.context_menu_origin_y,
            width: CONTEXT_MENU_COLUMN_WIDTH,
            height,
        });
    }

    rects
}

fn update_context_menu_placement(window: &AppWindow, state: &mut ShellViewModel) {
    if !state.context_menu_open {
        state.set_context_menu_placement(0.0, 0.0, false);
        return;
    }

    let (host_width, host_height) = current_window_size(window);
    let (origin_x, origin_y, child_flows_left) = resolve_root_menu_origin(MenuPlacementInput {
        host_width: host_width as f32,
        host_height: host_height as f32,
        anchor_x: state.context_menu_anchor_x,
        anchor_y: state.context_menu_anchor_y,
        root_width: CONTEXT_MENU_COLUMN_WIDTH,
        root_height: context_menu_overlay_height_for(state),
        child_width: context_menu_child_width_for(state),
    });

    state.set_context_menu_placement(origin_x, origin_y, child_flows_left);
}

fn sync_console_assets(window: &AppWindow, state: &ShellViewModel) {
    let project_rows = |rows: Vec<crate::shell::assets::VisibleAssetRow>| {
        rows.into_iter()
            .map(|row| ConsoleAssetItem {
                id: row.id.clone().into(),
                kind: row.kind.id().into(),
                label: row.label.clone().into(),
                depth: row.depth as i32,
                has_children: row.has_children,
                expanded: row.expanded,
                selected: state.selected_asset_ids.iter().any(|id| id == &row.id),
                focused: state.focused_asset_id.as_deref() == Some(row.id.as_str()),
                disclosure_state: match row.disclosure_state {
                    AssetDisclosureState::None => "none",
                    AssetDisclosureState::Collapsed => "collapsed",
                    AssetDisclosureState::Expanded => "expanded",
                }
                .into(),
                path_hint: row.path_hint.clone().unwrap_or_default().into(),
                compact_flat_mode: state.asset_view_mode.id() == "flat",
            })
            .collect::<Vec<_>>()
    };

    window.set_console_asset_items(ModelRc::new(VecModel::from(project_rows(
        state.visible_console_asset_rows(),
    ))));
    window.set_snippet_asset_items(ModelRc::new(VecModel::from(project_rows(
        state.visible_snippet_rows(),
    ))));
    sync_welcome_quick_launch_state(window, state);
    sync_saved_ssh_picker_state(window, state);
}

fn sync_keychain_assets(window: &AppWindow, state: &ShellViewModel) {
    let rows = state
        .visible_keychain_rows()
        .into_iter()
        .map(|row| ConsoleAssetItem {
            id: row.id.clone().into(),
            kind: row.kind.id().into(),
            label: row.label.clone().into(),
            depth: row.depth as i32,
            has_children: row.has_children,
            expanded: row.expanded,
            selected: state
                .selected_keychain_ids
                .iter()
                .any(|selected_id| selected_id == &row.id),
            focused: state.focused_keychain_id.as_deref() == Some(row.id.as_str()),
            disclosure_state: match row.disclosure_state {
                AssetDisclosureState::None => "none",
                AssetDisclosureState::Collapsed => "collapsed",
                AssetDisclosureState::Expanded => "expanded",
            }
            .into(),
            path_hint: row.path_hint.unwrap_or_default().into(),
            compact_flat_mode: false,
        })
        .collect::<Vec<_>>();

    window.set_keychain_asset_items(ModelRc::new(VecModel::from(rows)));
}

fn sync_keychain_modal_defaults(window: &AppWindow) {
    window.set_keychain_identity_modal_name("".into());
    window.set_keychain_identity_modal_username("".into());
    window.set_keychain_identity_modal_auth_kind("password".into());
    window.set_keychain_identity_modal_password("".into());
    window.set_keychain_identity_modal_ssh_key_label("".into());
    window.set_keychain_identity_modal_remark("".into());
    window.set_keychain_ssh_key_modal_name("".into());
    window.set_keychain_ssh_key_modal_private_key("".into());
    window.set_keychain_ssh_key_modal_public_key("".into());
    window.set_keychain_ssh_key_modal_fingerprint("".into());
}

fn sync_workspace_tab_items(window: &AppWindow, state: &ShellViewModel) {
    let tabs = state
        .workspace_tabs()
        .iter()
        .map(|tab| WorkspaceTabItem {
            session_id: tab.session_id.clone().into(),
            title: tab.title.clone().into(),
            subtitle: tab.subtitle.clone().into(),
            state: tab.state.clone().into(),
            enhanced_session_state: tab.enhanced_session_state.clone().into(),
            active: tab.active,
        })
        .collect::<Vec<_>>();

    window.set_workspace_tab_items(ModelRc::new(VecModel::from(tabs)));
}

fn sync_welcome_quick_launch_state(window: &AppWindow, state: &ShellViewModel) {
    let selected_asset_id = state.quick_launch_selected_asset_id();
    let active_group_id = state.quick_launch_active_group_id();
    let project_card = |item: crate::shell::quick_launch::QuickLaunchCardItem| QuickLaunchCardRow {
        asset_id: item.asset_id.clone().into(),
        title: item.title.into(),
        subtitle: item.subtitle.into(),
        badge: item.badge.into(),
        meta: item.meta.into(),
        icon_kind: item.icon_kind.into(),
        accent_kind: item.accent_kind.into(),
        favorite: item.favorite,
        selected: selected_asset_id == Some(item.asset_id.as_str()),
    };
    let project_group =
        |item: crate::shell::quick_launch::QuickLaunchGroupItem| QuickLaunchGroupRow {
            group_id: item.group_id.clone().into(),
            label: item.label.into(),
            count: i32::try_from(item.count).unwrap_or(i32::MAX),
            selected: active_group_id == Some(item.group_id.as_str()),
        };
    let project_detail =
        |item: crate::shell::quick_launch::QuickLaunchDetailItem| QuickLaunchDetailRow {
            asset_id: item.asset_id.into(),
            title: item.title.into(),
            subtitle: item.subtitle.into(),
            environment: item.environment.into(),
            auth_summary: item.auth_summary.into(),
            proxy_summary: item.proxy_summary.into(),
            remark: item.remark.into(),
            recent_label: item.recent_label.into(),
        };
    let empty_detail = || QuickLaunchDetailRow {
        asset_id: "".into(),
        title: "".into(),
        subtitle: "".into(),
        environment: "".into(),
        auth_summary: "".into(),
        proxy_summary: "".into(),
        remark: "".into(),
        recent_label: "".into(),
    };

    sync_vec_model(
        window.get_welcome_quick_launch_recent_items(),
        state
            .quick_launch_recent_items()
            .into_iter()
            .map(project_card)
            .collect::<Vec<_>>(),
        |model| window.set_welcome_quick_launch_recent_items(model),
    );
    sync_vec_model(
        window.get_welcome_quick_launch_favorite_items(),
        state
            .quick_launch_favorite_items()
            .into_iter()
            .map(project_card)
            .collect::<Vec<_>>(),
        |model| window.set_welcome_quick_launch_favorite_items(model),
    );
    sync_vec_model(
        window.get_welcome_quick_launch_group_items(),
        state
            .quick_launch_group_items()
            .into_iter()
            .map(project_group)
            .collect::<Vec<_>>(),
        |model| window.set_welcome_quick_launch_group_items(model),
    );
    sync_vec_model(
        window.get_welcome_quick_launch_visible_group_items(),
        state
            .quick_launch_visible_group_items()
            .into_iter()
            .map(project_card)
            .collect::<Vec<_>>(),
        |model| window.set_welcome_quick_launch_visible_group_items(model),
    );
    window.set_welcome_quick_launch_selected_detail(
        state
            .quick_launch_selected_detail()
            .map(project_detail)
            .unwrap_or_else(empty_detail),
    );
    window.set_welcome_quick_launch_search_query(state.quick_launch_search_query().into());
}

fn sync_saved_ssh_picker_state(window: &AppWindow, state: &ShellViewModel) {
    let items = state
        .saved_ssh_picker_items()
        .into_iter()
        .map(|item| ConsoleAssetItem {
            id: item.id.into(),
            kind: item.kind.into(),
            label: item.label.into(),
            depth: item.depth as i32,
            has_children: item.has_children,
            expanded: item.expanded,
            selected: item.selected,
            focused: item.focused,
            disclosure_state: item.disclosure_state.into(),
            path_hint: item.path_hint.into(),
            compact_flat_mode: item.compact_flat_mode,
        })
        .collect::<Vec<_>>();

    window.set_open_saved_ssh_modal_open(state.saved_ssh_picker_open());
    window.set_open_saved_ssh_modal_query(state.saved_ssh_picker_query().into());
    window.set_open_saved_ssh_modal_items(ModelRc::new(VecModel::from(items)));
}

fn slint_color_from_rgba(rgba: u32) -> Color {
    let a = ((rgba >> 24) & 0xff) as u8;
    let r = ((rgba >> 16) & 0xff) as u8;
    let g = ((rgba >> 8) & 0xff) as u8;
    let b = (rgba & 0xff) as u8;
    Color::from_argb_u8(a, r, g, b)
}

fn terminal_selection_overlay_rgba(theme_mode: ThemeMode) -> u32 {
    selection_overlay_rgba(theme_mode)
}

fn active_workspace_terminal_selection(window: &AppWindow) -> Option<TerminalAtlasSelection> {
    if !window.get_workspace_session_selection_active() {
        return None;
    }

    let start_row = window.get_workspace_session_selection_start_row();
    let start_col = window.get_workspace_session_selection_start_col();
    let end_row = window.get_workspace_session_selection_end_row();
    let end_col = window.get_workspace_session_selection_end_col();
    if start_row < 0 || start_col < 0 || end_row < 0 || end_col < 0 {
        return None;
    }

    Some(TerminalAtlasSelection::new(
        start_row as u32,
        start_col as u32,
        end_row as u32,
        end_col as u32,
    ))
}

fn sync_vec_model<T>(current: ModelRc<T>, next_rows: Vec<T>, replace: impl FnOnce(ModelRc<T>))
where
    T: Clone + PartialEq + 'static,
{
    if let Some(model) = current.as_any().downcast_ref::<VecModel<T>>() {
        reconcile_vec_model_rows(model, &next_rows);
    } else {
        replace(ModelRc::from(Rc::new(VecModel::from(next_rows))));
    }
}

fn reconcile_vec_model_rows<T>(model: &VecModel<T>, next_rows: &[T])
where
    T: Clone + PartialEq + 'static,
{
    let current_len = model.row_count();
    let shared_len = current_len.min(next_rows.len());

    for (index, next_row) in next_rows.iter().take(shared_len).enumerate() {
        if model.row_data(index).as_ref() != Some(next_row) {
            model.set_row_data(index, next_row.clone());
        }
    }

    while model.row_count() > next_rows.len() {
        let _ = model.remove(next_rows.len());
    }

    for next_row in next_rows.iter().skip(shared_len) {
        model.push(next_row.clone());
    }
}

fn build_workspace_terminal_presenter(
    profile: AppRuntimeProfile,
) -> Result<(Box<dyn TerminalPresenter>, TerminalRenderMode)> {
    if cfg!(target_os = "windows")
        && matches!(profile.terminal_render_mode, TerminalRenderMode::Native)
    {
        return Ok((
            build_native_terminal_presenter()?,
            TerminalRenderMode::Native,
        ));
    }

    Ok((
        Box::new(BitmapAtlasPresenter::new()?),
        TerminalRenderMode::Bitmap,
    ))
}

#[cfg(all(target_os = "windows", feature = "terminal-native-renderer"))]
fn build_native_terminal_presenter() -> Result<Box<dyn TerminalPresenter>> {
    Ok(Box::new(WindowsNativePresenter::new()?))
}

#[cfg(not(all(target_os = "windows", feature = "terminal-native-renderer")))]
fn build_native_terminal_presenter() -> Result<Box<dyn TerminalPresenter>> {
    Err(anyhow!(
        "native terminal renderer is unavailable in this build"
    ))
}

fn install_workspace_terminal_presenter(window: &AppWindow, profile: AppRuntimeProfile) {
    let (presenter, active_render_mode) = match build_workspace_terminal_presenter(profile) {
        Ok(presenter) => presenter,
        Err(err) => {
            tracing::error!(
                target: "app.terminal",
                error = %err,
                "failed to build requested terminal presenter; falling back to bitmap presenter"
            );
            (
                Box::new(
                    BitmapAtlasPresenter::new()
                        .expect("bundled Sarasa presenter should initialize after fallback"),
                ) as Box<dyn TerminalPresenter>,
                TerminalRenderMode::Bitmap,
            )
        }
    };

    WORKSPACE_TERMINAL_PRESENTER.with(|cell| {
        *cell.borrow_mut() = presenter;
    });
    window.set_workspace_session_render_mode(active_render_mode.as_str().into());
    if matches!(active_render_mode, TerminalRenderMode::Bitmap) {
        window.set_workspace_session_native_frame_token(0);
    }
}

fn workspace_native_terminal_rect(window: &AppWindow) -> NativeTerminalSurfaceRect {
    NativeTerminalSurfaceRect {
        x: window.get_layout_workspace_session_native_surface_x() as i32,
        y: window.get_layout_workspace_session_native_surface_y() as i32,
        width: window.get_layout_workspace_session_native_surface_width() as i32,
        height: window.get_layout_workspace_session_native_surface_height() as i32,
    }
}

fn sync_workspace_native_terminal_surface_geometry(window: &AppWindow) {
    let rect = workspace_native_terminal_rect(window);
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        if let Some(surface) = surface.borrow().as_ref() {
            surface.update_terminal_rect(rect);
        }
    });
}

fn present_workspace_native_terminal_frame(window: &AppWindow, frame: NativeTerminalFrame) {
    window.set_workspace_session_surface_image(Image::default());
    window.set_workspace_session_native_frame_token(
        i32::try_from(frame.frame_token).unwrap_or(i32::MAX),
    );
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        if let Some(surface) = surface.borrow().as_ref() {
            surface.present(frame);
        }
    });
}

fn clear_workspace_native_terminal_frame(window: &AppWindow) {
    window.set_workspace_session_surface_image(Image::default());
    window.set_workspace_session_native_frame_token(0);
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        if let Some(surface) = surface.borrow().as_ref() {
            surface.clear_frame();
        }
    });
}

#[cfg(test)]
fn sync_workspace_session_state(
    window: &AppWindow,
    state: &ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    sync_workspace_session_state_with_manager(window, state, follow_tracker, None);
}

fn connection_progress_headline_token(headline: ConnectionHeadlineState) -> &'static str {
    match headline {
        ConnectionHeadlineState::Connecting => "connecting",
        ConnectionHeadlineState::WaitingUser => "waiting-user",
        ConnectionHeadlineState::Connected => "connected",
        ConnectionHeadlineState::Cancelled => "cancelled",
        ConnectionHeadlineState::Error => "error",
    }
}

fn connection_progress_step_state_token(state: ConnectionStepState) -> &'static str {
    match state {
        ConnectionStepState::Pending => "pending",
        ConnectionStepState::Running => "running",
        ConnectionStepState::Done => "done",
        ConnectionStepState::Blocked => "blocked",
        ConnectionStepState::Failed => "failed",
        ConnectionStepState::Cancelled => "cancelled",
    }
}

fn active_connection_progress_step(
    attempt: &ConnectionAttemptState,
) -> Option<&ConnectionStepStateItem> {
    attempt
        .steps
        .iter()
        .rfind(|step| {
            matches!(
                step.state,
                ConnectionStepState::Running
                    | ConnectionStepState::Blocked
                    | ConnectionStepState::Failed
            )
        })
        .or_else(|| attempt.steps.last())
}

fn default_connection_progress_detail(headline: ConnectionHeadlineState) -> &'static str {
    match headline {
        ConnectionHeadlineState::Connecting => "Preparing SSH connection timeline...",
        ConnectionHeadlineState::WaitingUser => "Waiting for SSH connection input.",
        ConnectionHeadlineState::Connected => "SSH session is ready.",
        ConnectionHeadlineState::Cancelled => "SSH connection attempt was cancelled.",
        ConnectionHeadlineState::Error => "SSH connection attempt failed.",
    }
}

fn clear_workspace_connection_progress_state(window: &AppWindow) {
    window.set_workspace_session_connection_headline("".into());
    window.set_workspace_session_connection_current_hop("".into());
    window.set_workspace_session_connection_current_detail("".into());
    window.set_workspace_session_host_key_prompt_host("".into());
    window.set_workspace_session_host_key_prompt_fingerprint("".into());
    sync_vec_model(
        window.get_workspace_session_connection_steps(),
        Vec::<ConnectionProgressStepRow>::new(),
        |model| window.set_workspace_session_connection_steps(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_diagnostics(),
        Vec::<ConnectionProgressDiagnosticRow>::new(),
        |model| window.set_workspace_session_connection_diagnostics(model),
    );
}

fn sync_workspace_connection_progress_state(
    window: &AppWindow,
    state: &ShellViewModel,
    manager: Option<&SessionManager>,
) {
    if state.workspace_session_host_mode() != "connection-progress" {
        clear_workspace_connection_progress_state(window);
        return;
    }

    let Some(manager) = manager else {
        clear_workspace_connection_progress_state(window);
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        clear_workspace_connection_progress_state(window);
        return;
    };
    let Some(attempt) = manager.connection_attempt(session_id) else {
        clear_workspace_connection_progress_state(window);
        return;
    };

    let current_step = active_connection_progress_step(&attempt);
    let steps = attempt
        .steps
        .iter()
        .map(|step| ConnectionProgressStepRow {
            state: connection_progress_step_state_token(step.state).into(),
            hop_label: step.hop_label.clone().into(),
            title: step.title.clone().into(),
            detail: step.detail.clone().into(),
        })
        .collect::<Vec<_>>();
    let diagnostics = attempt
        .diagnostics
        .iter()
        .map(|line| ConnectionProgressDiagnosticRow {
            text: line.message.clone().into(),
        })
        .collect::<Vec<_>>();

    window.set_workspace_session_connection_headline(
        connection_progress_headline_token(attempt.headline).into(),
    );
    window.set_workspace_session_connection_current_hop(
        current_step
            .map(|step| step.hop_label.clone())
            .unwrap_or_default()
            .into(),
    );
    window.set_workspace_session_connection_current_detail(
        current_step
            .map(|step| step.detail.clone())
            .or_else(|| attempt.diagnostics.last().map(|line| line.message.clone()))
            .unwrap_or_else(|| default_connection_progress_detail(attempt.headline).into())
            .into(),
    );
    window.set_workspace_session_host_key_prompt_host(
        attempt
            .prompt
            .as_ref()
            .map(|prompt| format!("{}:{}", prompt.host, prompt.port))
            .unwrap_or_default()
            .into(),
    );
    window.set_workspace_session_host_key_prompt_fingerprint(
        attempt
            .prompt
            .as_ref()
            .map(|prompt| prompt.fingerprint.clone())
            .unwrap_or_default()
            .into(),
    );
    sync_vec_model(
        window.get_workspace_session_connection_steps(),
        steps,
        |model| window.set_workspace_session_connection_steps(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_diagnostics(),
        diagnostics,
        |model| window.set_workspace_session_connection_diagnostics(model),
    );
}

fn sync_workspace_session_state_with_manager(
    window: &AppWindow,
    state: &ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
    manager: Option<&SessionManager>,
) {
    window
        .set_active_workspace_session_id(state.active_workspace_session_id().unwrap_or("").into());
    window.set_workspace_session_host_mode(state.workspace_session_host_mode().into());
    let visible_lines = state
        .workspace_terminal_visible_lines()
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_workspace_session_visible_lines(),
        visible_lines,
        |model| window.set_workspace_session_visible_lines(model),
    );

    let (default_cell_width_px, default_cell_height_px) =
        WORKSPACE_TERMINAL_PRESENTER.with(|presenter| presenter.borrow().default_cell_size());
    window.set_workspace_session_cell_width(default_cell_width_px as f32);
    window.set_workspace_session_cell_height(default_cell_height_px as f32);
    sync_workspace_native_terminal_surface_geometry(window);

    let follow_indicator =
        follow_tracker.indicator_for_surface(state.active_workspace_terminal_surface());

    if let Some(surface) = state.active_workspace_terminal_surface() {
        let selection = active_workspace_terminal_selection(window);
        let selection_overlay_rgba = terminal_selection_overlay_rgba(state.theme_mode);
        WORKSPACE_TERMINAL_PRESENTER.with(|presenter| {
            let mut presenter = presenter.borrow_mut();
            presenter.set_raster_scale(window.window().scale_factor());
            match presenter.present(
                surface,
                TerminalPresentationOptions {
                    selection,
                    selection_overlay_rgba,
                },
            ) {
                Ok(PresentedTerminalFrame::Bitmap(frame)) => {
                    window.set_workspace_session_render_mode(
                        TerminalRenderMode::Bitmap.as_str().into(),
                    );
                    window.set_workspace_session_surface_image(frame.image);
                    window.set_workspace_session_native_frame_token(0);
                    window.set_workspace_session_cell_width(frame.cell_width_px as f32);
                    window.set_workspace_session_cell_height(frame.cell_height_px as f32);
                }
                Ok(PresentedTerminalFrame::Native(frame)) => {
                    window.set_workspace_session_render_mode(
                        TerminalRenderMode::Native.as_str().into(),
                    );
                    window.set_workspace_session_cell_width(frame.cell_width_px as f32);
                    window.set_workspace_session_cell_height(frame.cell_height_px as f32);
                    present_workspace_native_terminal_frame(window, frame);
                }
                Err(err) => {
                    tracing::error!(
                        target: "app.terminal",
                        session_id = surface.session_id.to_string(),
                        error = %err,
                        "failed to render workspace terminal atlas surface"
                    );
                    window.set_workspace_session_render_mode(
                        TerminalRenderMode::Bitmap.as_str().into(),
                    );
                    window.set_workspace_session_cell_width(default_cell_width_px as f32);
                    window.set_workspace_session_cell_height(default_cell_height_px as f32);
                    clear_workspace_native_terminal_frame(window);
                }
            }
        });
        window.set_workspace_session_rows(i32::try_from(surface.rows).unwrap_or(i32::MAX));
        window.set_workspace_session_cols(i32::try_from(surface.cols).unwrap_or(i32::MAX));
        window.set_workspace_session_cursor_row(
            i32::try_from(surface.cursor.row).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_cursor_col(
            i32::try_from(surface.cursor.col).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_cursor_visible(surface.cursor.visible);
        window.set_workspace_session_cursor_blinking(surface.cursor.blinking);
        window.set_workspace_session_cursor_shape(
            match surface.cursor.shape {
                crate::app::ssh::runtime::TerminalCursorShape::Block => "block",
                crate::app::ssh::runtime::TerminalCursorShape::Underline => "underline",
                crate::app::ssh::runtime::TerminalCursorShape::Bar => "bar",
            }
            .into(),
        );
        window.set_workspace_session_cursor_fg(slint_color_from_rgba(surface.cursor.fg_rgba));
        window.set_workspace_session_cursor_bg(slint_color_from_rgba(surface.cursor.bg_rgba));
        window.set_workspace_session_default_fg(slint_color_from_rgba(surface.default_fg_rgba));
        window.set_workspace_session_default_bg(slint_color_from_rgba(surface.default_bg_rgba));
        window.set_workspace_session_mouse_grabbed(surface.mouse_grabbed);
        window.set_workspace_session_viewport_offset_lines(
            i32::try_from(surface.viewport_offset_lines).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_viewport_max_offset_lines(
            i32::try_from(surface.viewport_max_offset_lines).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_viewport_at_bottom(surface.viewport_at_bottom);
        window.set_workspace_session_follow_paused(follow_indicator.paused);
        window.set_workspace_session_pending_output_lines(
            i32::try_from(follow_indicator.pending_output_lines).unwrap_or(i32::MAX),
        );
    } else {
        let preset = preset_for_theme_mode(state.theme_mode);
        window.set_workspace_session_rows(24);
        window.set_workspace_session_cols(80);
        window.set_workspace_session_cursor_row(0);
        window.set_workspace_session_cursor_col(0);
        window.set_workspace_session_cursor_visible(false);
        window.set_workspace_session_cursor_blinking(false);
        window.set_workspace_session_cursor_shape("block".into());
        window
            .set_workspace_session_cursor_fg(slint_color_from_rgba(0xff00_0000 | preset.cursor_fg));
        window
            .set_workspace_session_cursor_bg(slint_color_from_rgba(0xff00_0000 | preset.cursor_bg));
        window.set_workspace_session_default_fg(slint_color_from_rgba(
            0xff00_0000 | preset.foreground,
        ));
        window.set_workspace_session_default_bg(slint_color_from_rgba(
            0xff00_0000 | preset.background,
        ));
        clear_workspace_native_terminal_frame(window);
        window.set_workspace_session_mouse_grabbed(false);
        window.set_workspace_session_viewport_offset_lines(0);
        window.set_workspace_session_viewport_max_offset_lines(0);
        window.set_workspace_session_viewport_at_bottom(true);
        window.set_workspace_session_follow_paused(false);
        window.set_workspace_session_pending_output_lines(0);
    }

    if let Some(active_tab) = state.active_workspace_tab() {
        window.set_workspace_session_title(active_tab.title.clone().into());
        window.set_workspace_session_subtitle(active_tab.subtitle.clone().into());
        window.set_workspace_session_state(active_tab.state.clone().into());
        window.set_workspace_session_error_detail(active_tab.error_detail.clone().into());
        window.set_workspace_session_can_reconnect(active_tab.can_reconnect());
        window.set_workspace_session_surface_seqno(
            i32::try_from(state.workspace_terminal_surface_seqno()).unwrap_or(i32::MAX),
        );
    } else {
        window.set_workspace_session_title("".into());
        window.set_workspace_session_subtitle("".into());
        window.set_workspace_session_state("".into());
        window.set_workspace_session_error_detail("".into());
        window.set_workspace_session_can_reconnect(false);
        window.set_workspace_session_surface_seqno(0);
    }

    sync_workspace_connection_progress_state(window, state, manager);
}

fn sync_workspace_tabs(
    window: &AppWindow,
    state: &ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    sync_workspace_tabs_with_manager(window, state, follow_tracker, None);
}

fn sync_workspace_tabs_with_manager(
    window: &AppWindow,
    state: &ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
    manager: Option<&SessionManager>,
) {
    sync_workspace_tab_items(window, state);
    sync_workspace_session_state_with_manager(window, state, follow_tracker, manager);
}

fn sync_shell_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    sync_top_status_bar_state(window, state, effects);
    sync_sync_modal_state(window, state);
    sync_right_panel_state(window, state);
    sync_sidebar_state(window, state);
    sync_workspace_tabs(window, state, follow_tracker);
    sync_assets_context_menu_state(window, state);
    sync_asset_modal_state(window, state);
    sync_sftp_remote_file_modal_state(window, state);
    sync_ssh_host_key_modal_state(window, state);
}

fn sync_shell_layout(
    window: &AppWindow,
    state: &mut ShellViewModel,
    logical_width: u32,
    logical_height: u32,
) {
    // Rust owns the responsive policy so Slint can consume stable booleans instead of repeating
    // width-threshold logic in multiple components.
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: logical_width.max(ShellMetrics::WINDOW_MIN_WIDTH),
        request_assets_sidebar: state.requested_assets_sidebar(),
        request_right_panel: state.requested_right_panel(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
    window.set_shell_body_height_cache(
        logical_height.saturating_sub(ShellMetrics::TITLEBAR_HEIGHT) as f32,
    );
    sync_workspace_native_terminal_surface_geometry(window);
    update_context_menu_placement(window, state);
    sync_assets_context_menu_state(window, state);
}

fn current_window_size(window: &AppWindow) -> (u32, u32) {
    let size = window.window().size();
    (size.width, size.height)
}

fn default_vault_runtime_root() -> PathBuf {
    app_root_paths_for_app()
        .map(|paths| paths.data_dir.join("vault"))
        .unwrap_or_else(|_| std::env::temp_dir().join("mica-term").join("vault"))
}

fn vault_known_hosts_path(root_dir: &std::path::Path) -> PathBuf {
    root_dir.join("known_hosts")
}

fn default_vault_kdf() -> KdfConfig {
    KdfConfig::Argon2id {
        memory_cost_kib: 19_456,
        time_cost: 2,
        parallelism: 1,
        salt_b64: Uuid::new_v4().simple().to_string(),
    }
}

fn next_vault_revision(current_revision: Option<&str>) -> String {
    let next_number = current_revision
        .and_then(|revision| revision.strip_prefix("rev-"))
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value + 1)
        .unwrap_or(1);
    format!("rev-{next_number:04}")
}

fn update_vault_panel_for_local_state(state: &mut ShellViewModel, vault: &VaultSessionState) {
    let panel = state.vault_panel_state_mut();
    match (&vault.local_state, vault.unlocked_vault_key.is_some()) {
        (None, false) => {
            panel.lock_state_label = "Locked".into();
            panel.primary_status_label = "Primary not configured".into();
            panel.primary_action_label = "Set".into();
            panel.secondary_action_label = "Change".into();
            panel.tertiary_action_label = "Lock now".into();
        }
        (Some(local_state), false) => {
            panel.lock_state_label = "Locked".into();
            panel.primary_status_label = if local_state
                .bundle
                .remotes
                .iter()
                .any(|remote| remote.role == RemoteRole::Primary)
            {
                "Primary configured".into()
            } else {
                "Primary not configured".into()
            };
            panel.primary_action_label = "Unlock".into();
            panel.secondary_action_label = "Change".into();
            panel.tertiary_action_label = "Lock now".into();
        }
        (Some(local_state), true) => {
            panel.lock_state_label = "Unlocked".into();
            panel.primary_status_label = if local_state
                .bundle
                .remotes
                .iter()
                .any(|remote| remote.role == RemoteRole::Primary)
            {
                "Primary configured".into()
            } else {
                "Primary not configured".into()
            };
            panel.primary_action_label = "Change".into();
            panel.secondary_action_label = "Sync now".into();
            panel.tertiary_action_label = "Lock now".into();
        }
        (None, true) => {}
    }
}

fn sync_settings_remote_id(role: RemoteRole) -> &'static str {
    match role {
        RemoteRole::Primary => "remote-primary",
        RemoteRole::Mirror => "remote-mirror",
    }
}

fn configured_sync_bundle(vault: &VaultSessionState) -> Option<&BootstrapBundle> {
    vault
        .local_state
        .as_ref()
        .map(|local_state| &local_state.bundle)
        .or(vault.bootstrap_template.as_ref())
}

fn hydrate_sync_modal_draft(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    credential_store: &dyn CredentialStore,
) {
    let modal = state.sync_modal_state_mut();
    let bundle = configured_sync_bundle(vault);
    let primary = bundle.and_then(BootstrapBundle::primary_remote);
    let mirror = bundle.and_then(|bundle| {
        bundle
            .remotes
            .iter()
            .find(|remote| remote.role == RemoteRole::Mirror)
    });

    modal.auto_sync_enabled = bundle.is_some_and(|bundle| bundle.auto_sync_enabled);
    modal.primary_gist_id = match primary.map(|remote| &remote.locator) {
        Some(crate::app::vault::model::BootstrapRemoteLocator::GiteeGist { gist_id }) => {
            gist_id.clone()
        }
        _ => String::new(),
    };
    modal.primary_pat = primary
        .and_then(|remote| {
            load_provider_credential(credential_store, remote.credential_ref.as_deref()).ok()
        })
        .flatten()
        .unwrap_or_default();
    modal.mirror_enabled = mirror.is_some();
    modal.mirror_gist_id = match mirror.map(|remote| &remote.locator) {
        Some(crate::app::vault::model::BootstrapRemoteLocator::GiteeGist { gist_id }) => {
            gist_id.clone()
        }
        _ => String::new(),
    };
    modal.mirror_pat = mirror
        .and_then(|remote| {
            load_provider_credential(credential_store, remote.credential_ref.as_deref()).ok()
        })
        .flatten()
        .unwrap_or_default();
    modal.master_password.clear();
}

fn build_sync_bundle_from_modal(
    state: &ShellViewModel,
    existing_bundle: Option<&BootstrapBundle>,
) -> Result<BootstrapBundle> {
    let modal = state.sync_modal_state();
    let primary_gist_id = modal.primary_gist_id.trim();
    let primary_pat = modal.primary_pat.trim();

    if primary_gist_id.is_empty() {
        return Err(anyhow!("Enter a primary Gist ID before enabling sync"));
    }
    if primary_pat.is_empty() {
        return Err(anyhow!(
            "Enter a primary Personal Access Token before enabling sync"
        ));
    }
    if modal.mirror_enabled {
        if modal.mirror_gist_id.trim().is_empty() {
            return Err(anyhow!(
                "Enter a mirror Gist ID or disable the mirror target"
            ));
        }
        if modal.mirror_pat.trim().is_empty() {
            return Err(anyhow!(
                "Enter a mirror Personal Access Token or disable the mirror target"
            ));
        }
    }

    let mut bundle = existing_bundle.cloned().unwrap_or_default();
    bundle.auto_sync_enabled = modal.auto_sync_enabled;
    bundle.remotes.clear();
    bundle.remotes.push(BootstrapRemoteConfig {
        remote_id: sync_settings_remote_id(RemoteRole::Primary).into(),
        role: RemoteRole::Primary,
        provider: first_release_formal_provider_kind(),
        locator: crate::app::vault::model::BootstrapRemoteLocator::GiteeGist {
            gist_id: primary_gist_id.into(),
        },
        credential_ref: Some(bootstrap_provider_credential_ref(sync_settings_remote_id(
            RemoteRole::Primary,
        ))),
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    });

    if modal.mirror_enabled {
        bundle.remotes.push(BootstrapRemoteConfig {
            remote_id: sync_settings_remote_id(RemoteRole::Mirror).into(),
            role: RemoteRole::Mirror,
            provider: first_release_formal_provider_kind(),
            locator: crate::app::vault::model::BootstrapRemoteLocator::GiteeGist {
                gist_id: modal.mirror_gist_id.trim().into(),
            },
            credential_ref: Some(bootstrap_provider_credential_ref(sync_settings_remote_id(
                RemoteRole::Mirror,
            ))),
            auth_kind: ProviderAuthKind::Pat,
            last_health: None,
        });
    }

    Ok(bundle)
}

fn persist_sync_modal_settings(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let existing_bundle = configured_sync_bundle(vault);
    let bundle = build_sync_bundle_from_modal(state, existing_bundle)?;
    let modal = state.sync_modal_state();

    persist_provider_credential(
        credential_store,
        bootstrap_provider_credential_ref(sync_settings_remote_id(RemoteRole::Primary)).as_str(),
        Some(modal.primary_pat.as_str()),
    )?;
    persist_provider_credential(
        credential_store,
        bootstrap_provider_credential_ref(sync_settings_remote_id(RemoteRole::Mirror)).as_str(),
        if modal.mirror_enabled {
            Some(modal.mirror_pat.as_str())
        } else {
            None
        },
    )?;

    let bootstrap_state_path = vault.bootstrap_state_path();
    if let Some(local_state) = vault.local_state.as_mut() {
        local_state.bundle = bundle;
        save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
    } else {
        vault.bootstrap_template = Some(bundle);
    }

    update_sync_modal_for_local_state(state, vault);
    Ok(())
}

fn update_sync_modal_for_local_state(state: &mut ShellViewModel, vault: &VaultSessionState) {
    let has_primary_remote = vault_primary_remote(vault).is_some();
    let gitee_setup = GiteeRemoteDraft::default();
    let mirror_count = configured_sync_bundle(vault)
        .map(|bundle| {
            bundle
                .remotes
                .iter()
                .filter(|remote| remote.role == RemoteRole::Mirror)
                .count()
        })
        .unwrap_or(0);
    let modal = state.sync_modal_state_mut();

    modal.title = "Sync Settings".into();
    debug_assert_eq!(
        first_release_formal_provider_kind(),
        ProviderKind::GiteeGist
    );
    modal.provider_label = first_release_formal_provider_label().into();
    modal.target_label = if has_primary_remote {
        if mirror_count == 0 {
            "1 target configured".into()
        } else {
            format!("1 primary + {mirror_count} mirror")
        }
    } else {
        String::new()
    };
    modal.error_text.clear();

    match (
        &vault.local_state,
        vault.unlocked_vault_key.is_some(),
        has_primary_remote,
    ) {
        (None, false, false) => {
            modal.mode = SyncModalMode::NotConfigured;
            modal.headline = "Configure sync".into();
            modal.status_text = gitee_setup.setup_summary();
            modal.primary_action_label = "Save and enable".into();
            modal.secondary_action_label = "Close".into();
        }
        (None, false, true) => {
            modal.mode = SyncModalMode::NotConfigured;
            modal.headline = "Enable or recover sync".into();
            modal.status_text = "The target is configured. Enter a master password to recover from the remote if it already has data, or create a new local vault if it is still empty.".into();
            modal.primary_action_label = "Save and enable".into();
            modal.secondary_action_label = "Close".into();
        }
        (Some(_), false, false) | (Some(_), false, true) => {
            modal.mode = SyncModalMode::Locked;
            modal.headline = "Unlock sync".into();
            modal.status_text = "Sync is configured. Enter your master password to unlock local secrets and resume sync.".into();
            modal.primary_action_label = "Unlock".into();
            modal.secondary_action_label = "Close".into();
        }
        (Some(_), true, false) => {
            modal.mode = SyncModalMode::UnlockedButRemoteIncomplete;
            modal.headline = "Finish sync settings".into();
            modal.status_text = format!(
                "Add a {} target authenticated with {} before sync can run.",
                first_release_formal_provider_label(),
                first_release_formal_auth_label()
            );
            modal.primary_action_label = "Save settings".into();
            modal.secondary_action_label = "Lock".into();
        }
        (Some(_), true, true) => {
            modal.mode = SyncModalMode::Ready;
            modal.headline = "Sync is ready".into();
            modal.status_text =
                if configured_sync_bundle(vault).is_some_and(|bundle| bundle.auto_sync_enabled) {
                    "Auto sync is enabled. Titlebar Sync runs an immediate foreground sync.".into()
                } else {
                    "Auto sync is disabled. Use the titlebar Sync button when you want to sync now."
                        .into()
                };
            modal.primary_action_label = "Sync now".into();
            modal.secondary_action_label = "Lock".into();
        }
        (None, true, _) => {
            modal.mode = SyncModalMode::SyncError;
            modal.headline = "Sync state is inconsistent".into();
            modal.status_text = "The local vault state could not be resolved.".into();
            modal.error_text = "Missing local bootstrap state".into();
            modal.primary_action_label = "Close".into();
            modal.secondary_action_label.clear();
        }
    }
}

fn vault_primary_remote(vault: &VaultSessionState) -> Option<&BootstrapRemoteConfig> {
    vault
        .local_state
        .as_ref()
        .and_then(|local_state| local_state.bundle.primary_remote())
        .or_else(|| {
            vault
                .bootstrap_template
                .as_ref()
                .and_then(BootstrapBundle::primary_remote)
        })
}

fn set_sync_modal_error(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    error: impl Into<String>,
) {
    update_sync_modal_for_local_state(state, vault);
    state.set_sync_modal_error(error);
}

fn set_sync_modal_error_without_opening(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    error: impl Into<String>,
) {
    update_sync_modal_for_local_state(state, vault);
    state.sync_modal_state_mut().error_text = error.into();
}

fn submit_sync_modal_master_password(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    if vault.local_state.is_some() {
        unlock_local_vault_into_shell(state, vault, credential_store, password)
    } else {
        if recover_local_vault_from_primary_remote(state, vault, credential_store, password)? {
            return Ok(());
        }
        create_local_vault_from_shell_state(state, vault, credential_store, password)
    }
}

fn sync_preferences_for_bundle(
    bundle: &BootstrapBundle,
    last_sync_result: Option<String>,
) -> SnapshotSyncPreferences {
    SnapshotSyncPreferences {
        auto_sync_enabled: bundle.auto_sync_enabled,
        selected_primary_remote_id: bundle
            .primary_remote()
            .map(|remote| remote.remote_id.clone()),
        selected_mirror_remote_ids: bundle
            .remotes
            .iter()
            .filter(|remote| remote.role == RemoteRole::Mirror)
            .map(|remote| remote.remote_id.clone())
            .collect(),
        last_sync_result,
    }
}

fn apply_vault_snapshot_to_shell(
    state: &mut ShellViewModel,
    snapshot: &VaultSnapshot,
    credential_store: &dyn CredentialStore,
    known_hosts_path: &std::path::Path,
) -> Result<()> {
    let applied = apply_vault_snapshot(snapshot, credential_store, known_hosts_path)?;
    let (console_tree, snippet_tree) =
        catalog_to_asset_trees(&asset_tree_to_catalog(&applied.asset_tree));
    state.replace_vault_projection(console_tree, snippet_tree, applied.keychain_catalog);
    Ok(())
}

fn clear_vault_decrypted_state(
    state: &mut ShellViewModel,
    snapshot: Option<&VaultSnapshot>,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    if let Some(snapshot) = snapshot {
        for node in snapshot.asset_catalog.nodes.values() {
            let VaultAssetPayload::SshConnection(spec) = &node.payload else {
                continue;
            };
            restore_snapshot_secret_bundle(credential_store, spec.credential_ref.as_deref(), None)?;
        }

        for node in snapshot.keychain_catalog.nodes.values() {
            match &node.payload {
                KeychainNodePayload::Folder => {}
                KeychainNodePayload::Identity(spec) => restore_snapshot_secret_bundle(
                    credential_store,
                    spec.credential_ref.as_deref(),
                    None,
                )?,
                KeychainNodePayload::SshKey(spec) => restore_snapshot_secret_bundle(
                    credential_store,
                    spec.credential_ref.as_deref(),
                    None,
                )?,
            }
        }
    }
    state.clear_vault_projection();
    Ok(())
}

fn create_local_vault_from_shell_state(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    let mut bundle = vault
        .bootstrap_template
        .clone()
        .ok_or_else(|| anyhow!("Configure a Gitee remote first"))?;
    if bundle.primary_remote().is_none() {
        return Err(anyhow!("Configure a Gitee remote first"));
    }
    if bundle.vault_id.trim().is_empty() {
        bundle.vault_id = format!("vault-{}", Uuid::new_v4().simple());
    }
    ensure_primary_remote_is_empty_before_first_local_bootstrap(vault, &bundle, credential_store)?;
    let kdf = default_vault_kdf();
    let vault_key = generate_vault_key();
    let wrapped_vault_key = serde_json::to_string(&wrap_vault_key(password, &kdf, &vault_key)?)
        .context("failed to encode wrapped vault key")?;
    let snapshot = export_vault_snapshot(
        &combined_asset_tree(state),
        state.keychain_catalog(),
        credential_store,
        vault.known_hosts_path().as_path(),
        sync_preferences_for_bundle(&bundle, None),
        &UiPreferences::from(&*state),
    )?;
    let encrypted_snapshot = encrypt_snapshot(&snapshot, &vault_key)?;
    store_encrypted_cache(
        vault.cache_root().as_path(),
        &bundle.vault_id,
        &encrypted_snapshot,
    )?;
    let local_state = LocalVaultBootstrapState {
        bundle,
        wrapped_vault_key,
        kdf: kdf.clone(),
        current_revision: None,
    };
    save_local_vault_bootstrap_state(vault.bootstrap_state_path().as_path(), &local_state)?;
    vault.local_state = Some(local_state);
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);

    Ok(())
}

fn recover_local_vault_from_primary_remote(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<bool> {
    let Some(mut bundle) = vault.bootstrap_template.clone() else {
        return Ok(false);
    };
    let Some(primary_remote) = bundle.primary_remote().cloned() else {
        return Ok(false);
    };
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault.provider_factory.build_provider(&primary_remote)?;
    let Some(remote_head) = provider
        .read_head()
        .with_context(|| {
            format!(
                "failed to inspect primary remote `{}` before enabling sync",
                primary_remote.remote_id
            )
        })?
        .head
    else {
        return Ok(false);
    };
    let remote_revision = provider.read_revision(&remote_head).map_err(|err| {
        anyhow!(
            "failed to read recoverable revision `{}` from primary remote `{}`: {err}",
            remote_head.vault_revision,
            primary_remote.remote_id
        )
    })?;
    let wrapped: WrappedVaultKey = serde_json::from_str(&remote_head.wrapped_vault_key)
        .context("failed to decode wrapped vault key from remote head")?;
    let vault_key = unwrap_vault_key(password, &wrapped)?;
    let snapshot = decrypt_snapshot(&remote_revision.encrypted_snapshot, &vault_key)?;
    bundle.vault_id = remote_head.vault_id.clone();
    store_encrypted_cache(
        vault.cache_root().as_path(),
        &bundle.vault_id,
        &remote_revision.encrypted_snapshot,
    )?;
    let local_state = LocalVaultBootstrapState {
        bundle,
        wrapped_vault_key: remote_head.wrapped_vault_key.clone(),
        kdf: remote_head.kdf.clone(),
        current_revision: Some(remote_head.vault_revision.clone()),
    };
    save_local_vault_bootstrap_state(vault.bootstrap_state_path().as_path(), &local_state)?;
    apply_vault_snapshot_to_shell(
        state,
        &snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;
    vault.local_state = Some(local_state);
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    state.vault_panel_state_mut().primary_status_label =
        format!("Recovered from primary {}", remote_head.vault_revision);
    state.sync_modal_state_mut().status_text = format!(
        "Recovered local vault from primary remote at {}.",
        remote_head.vault_revision
    );

    Ok(true)
}

fn ensure_primary_remote_is_empty_before_first_local_bootstrap(
    vault: &VaultSessionState,
    bundle: &BootstrapBundle,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let primary_remote = bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let resolved = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault.provider_factory.build_provider(&resolved)?;
    let remote_head = provider
        .read_head()
        .with_context(|| {
            format!(
                "failed to inspect primary remote `{}` before enabling sync",
                primary_remote.remote_id
            )
        })?
        .head;

    if let Some(head) = remote_head {
        return Err(anyhow!(
            "primary remote `{}` already contains revision `{}`. Local recovery from remote is not implemented yet, so refusing to initialize a new empty local vault over existing remote data.",
            primary_remote.remote_id,
            head.vault_revision
        ));
    }

    Ok(())
}

fn unlock_local_vault_into_shell(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
    password: &secrecy::SecretString,
) -> Result<()> {
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let wrapped: WrappedVaultKey = serde_json::from_str(&local_state.wrapped_vault_key)
        .context("failed to decode wrapped vault key")?;
    let vault_key = unwrap_vault_key(password, &wrapped)?;
    let encrypted_snapshot =
        load_encrypted_cache(vault.cache_root().as_path(), &local_state.bundle.vault_id)?
            .ok_or_else(|| anyhow!("encrypted cache is unavailable"))?;
    let snapshot = decrypt_snapshot(&encrypted_snapshot, &vault_key)?;
    apply_vault_snapshot_to_shell(
        state,
        &snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;
    vault.unlocked_vault_key = Some(vault_key);
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    Ok(())
}

fn lock_local_vault(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    clear_vault_decrypted_state(state, vault.decrypted_snapshot.as_ref(), credential_store)?;
    vault.unlocked_vault_key = None;
    vault.decrypted_snapshot = None;
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    Ok(())
}

fn resolve_remote_for_sync(
    remote: &BootstrapRemoteConfig,
    credential_store: &dyn CredentialStore,
) -> Result<BootstrapRemoteConfig> {
    let mut resolved = remote.clone();

    if remote.provider == ProviderKind::GiteeGist && remote.auth_kind == ProviderAuthKind::Pat {
        let inline_secret =
            load_provider_credential(credential_store, remote.credential_ref.as_deref())?;
        let inline_secret = inline_secret.ok_or_else(|| {
            anyhow!(
                "missing saved provider credential for remote `{}`",
                remote.remote_id
            )
        })?;
        resolved.credential_ref = Some(inline_secret);
    }

    Ok(resolved)
}

fn sync_local_vault(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<()> {
    let known_hosts_path = vault.known_hosts_path();
    let cache_root = vault.cache_root();
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let local_bundle = local_state.bundle.clone();
    let current_revision = local_state.current_revision.clone();
    let wrapped_vault_key = local_state.wrapped_vault_key.clone();
    let kdf = local_state.kdf.clone();
    let vault_key = vault
        .unlocked_vault_key
        .ok_or_else(|| anyhow!("vault is locked"))?;
    let snapshot = export_vault_snapshot(
        &combined_asset_tree(state),
        state.keychain_catalog(),
        credential_store,
        known_hosts_path.as_path(),
        sync_preferences_for_bundle(&local_bundle, None),
        &UiPreferences::from(&*state),
    )?;
    if let Some(stable_revision) = current_revision.as_ref().filter(|_| {
        vault
            .decrypted_snapshot
            .as_ref()
            .is_some_and(|existing| existing == &snapshot)
    }) {
        update_vault_panel_for_local_state(state, vault);
        update_sync_modal_for_local_state(state, vault);
        state.vault_panel_state_mut().primary_status_label =
            format!("Already synced {stable_revision}");
        state.sync_modal_state_mut().status_text =
            format!("No local changes to upload. Primary stays at {stable_revision}.");
        return Ok(());
    }

    let primary_remote = local_bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let primary_provider = vault.provider_factory.build_provider(&primary_remote)?;
    let mirror_providers = local_bundle
        .remotes
        .iter()
        .filter(|remote| remote.role == RemoteRole::Mirror)
        .map(|remote| {
            let resolved = resolve_remote_for_sync(remote, credential_store)?;
            vault.provider_factory.build_provider(&resolved)
        })
        .collect::<Result<Vec<_>>>()?;
    let request = SyncRequest {
        vault_id: local_bundle.vault_id.clone(),
        snapshot: snapshot.clone(),
        next_revision: next_vault_revision(current_revision.as_deref()),
        parent_revision: current_revision,
        device_id: "local-device".into(),
        created_at: "2026-03-28T00:00:00Z".into(),
        wrapped_vault_key,
        kdf,
        provider_kind: primary_remote.provider,
        vault_key,
    };
    let engine = SyncEngine::new(primary_provider, mirror_providers);

    match engine.sync(request) {
        Ok(report) => {
            let local_state = vault
                .local_state
                .as_mut()
                .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
            store_encrypted_cache(
                cache_root.as_path(),
                &local_bundle.vault_id,
                &report.encrypted_snapshot,
            )?;
            local_state.current_revision = Some(report.primary_revision.clone());
            vault.decrypted_snapshot = Some(snapshot);
            update_vault_panel_for_local_state(state, vault);
            update_sync_modal_for_local_state(state, vault);
            if report.is_mirror_degraded() {
                let mirror_degraded_message = format!(
                    "Mirror degraded: {}",
                    report
                        .mirror_failures
                        .first()
                        .map(|failure| failure.message.as_str())
                        .unwrap_or("unknown mirror failure")
                );
                state.vault_panel_state_mut().primary_status_label =
                    mirror_degraded_message.clone();
                state.sync_modal_state_mut().status_text = format!(
                    "Primary synced {}. {}",
                    report.primary_revision, mirror_degraded_message
                );
            } else {
                state.vault_panel_state_mut().primary_status_label =
                    format!("Primary synced {}", report.primary_revision);
                state.sync_modal_state_mut().status_text = format!(
                    "Sync completed. Primary is now at {}.",
                    report.primary_revision
                );
            }
            Ok(())
        }
        Err(err) => {
            update_vault_panel_for_local_state(state, vault);
            update_sync_modal_for_local_state(state, vault);
            state.vault_panel_state_mut().primary_status_label = match &err {
                SyncError::PrimaryReadFailed { message, .. }
                | SyncError::PrimaryWriteFailed { message, .. } => {
                    format!("Provider auth error: {message}")
                }
                SyncError::Conflict { .. } => "Remote conflict".into(),
                SyncError::PayloadAssemblyFailed { message } => {
                    format!("Vault decrypt error: {message}")
                }
            };
            state.sync_modal_state_mut().error_text = err.to_string();
            Err(anyhow!(err.to_string()))
        }
    }
}

fn refresh_local_vault_from_primary_remote_if_changed(
    state: &mut ShellViewModel,
    vault: &mut VaultSessionState,
    credential_store: &dyn CredentialStore,
) -> Result<bool> {
    let local_state = vault
        .local_state
        .as_ref()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    let current_revision = local_state.current_revision.clone();
    let primary_remote = local_state
        .bundle
        .primary_remote()
        .cloned()
        .ok_or_else(|| anyhow!("primary remote is not configured"))?;
    let primary_remote = resolve_remote_for_sync(&primary_remote, credential_store)?;
    let provider = vault.provider_factory.build_provider(&primary_remote)?;
    let Some(remote_head) = provider
        .read_head()
        .map_err(|err| {
            anyhow!(
                "failed to inspect primary remote `{}`: {err}",
                primary_remote.remote_id
            )
        })?
        .head
    else {
        return Ok(false);
    };
    if current_revision.as_deref() == Some(remote_head.vault_revision.as_str()) {
        update_vault_panel_for_local_state(state, vault);
        update_sync_modal_for_local_state(state, vault);
        state.vault_panel_state_mut().primary_status_label =
            format!("Already synced {}", remote_head.vault_revision);
        state.sync_modal_state_mut().status_text =
            format!("No remote changes found. Primary stays at {}.", remote_head.vault_revision);
        return Ok(false);
    }

    let remote_revision = provider.read_revision(&remote_head).map_err(|err| {
        anyhow!(
            "failed to read primary revision `{}` from remote `{}`: {err}",
            remote_head.vault_revision,
            primary_remote.remote_id
        )
    })?;
    let vault_key = vault
        .unlocked_vault_key
        .ok_or_else(|| anyhow!("vault is locked"))?;
    let snapshot = decrypt_snapshot(&remote_revision.encrypted_snapshot, &vault_key)?;
    clear_vault_decrypted_state(state, vault.decrypted_snapshot.as_ref(), credential_store)?;
    apply_vault_snapshot_to_shell(
        state,
        &snapshot,
        credential_store,
        vault.known_hosts_path().as_path(),
    )?;

    let bootstrap_state_path = vault.bootstrap_state_path();
    let cache_root = vault.cache_root();
    let local_state = vault
        .local_state
        .as_mut()
        .ok_or_else(|| anyhow!("vault bootstrap is not initialized"))?;
    local_state.wrapped_vault_key = remote_head.wrapped_vault_key.clone();
    local_state.kdf = remote_head.kdf.clone();
    local_state.current_revision = Some(remote_head.vault_revision.clone());
    save_local_vault_bootstrap_state(bootstrap_state_path.as_path(), local_state)?;
    store_encrypted_cache(
        cache_root.as_path(),
        &local_state.bundle.vault_id,
        &remote_revision.encrypted_snapshot,
    )?;
    vault.decrypted_snapshot = Some(snapshot);
    update_vault_panel_for_local_state(state, vault);
    update_sync_modal_for_local_state(state, vault);
    state.vault_panel_state_mut().primary_status_label =
        format!("Pulled primary {}", remote_head.vault_revision);
    state.sync_modal_state_mut().status_text = format!(
        "Pulled remote changes from primary {}.",
        remote_head.vault_revision
    );

    Ok(true)
}

fn vault_auto_sync_ready(vault: &VaultSessionState) -> bool {
    vault.local_state.is_some()
        && vault.unlocked_vault_key.is_some()
        && configured_sync_bundle(vault).is_some_and(|bundle| bundle.auto_sync_enabled)
}

fn mark_local_vault_dirty_and_arm_auto_sync(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    scheduler: &Rc<RefCell<VaultSyncSchedulerState>>,
    auto_sync_timer: &Rc<Timer>,
    run_sync: Rc<dyn Fn(VaultSyncTrigger)>,
) {
    scheduler.borrow_mut().dirty = true;
    state.sync_modal_state_mut().status_text =
        "Local changes queued for sync when the vault is ready.".into();

    if vault_auto_sync_ready(vault) {
        auto_sync_timer.start(
            TimerMode::SingleShot,
            Duration::from_millis(VAULT_AUTO_SYNC_DEBOUNCE_MS),
            move || {
                run_sync(VaultSyncTrigger::DebouncedAuto);
            },
        );
    }
}

#[cfg(target_os = "windows")]
const WINDOW_FRAME_RESERVED_RESIZE_BAND: i32 = 10;

#[cfg(target_os = "windows")]
fn install_windows_frame_adapter(window: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;

    // The native subclass needs the live maximize-button geometry from Slint so Windows snap
    // layouts still target the custom titlebar button.
    let placement = query_true_window_placement_from_app(window);
    let maximize_button = CaptionButtonGeometry {
        x: window.get_layout_titlebar_maximize_button_x() as i32,
        y: window.get_layout_titlebar_maximize_button_y() as i32,
        width: window.get_layout_titlebar_maximize_button_width() as i32,
        height: window.get_layout_titlebar_maximize_button_height() as i32,
    };

    let _ = window.window().with_winit_window(|winit_window| {
        install_window_frame_adapter(
            winit_window,
            maximize_button,
            placement,
            WINDOW_FRAME_RESERVED_RESIZE_BAND,
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn install_windows_frame_adapter(_window: &AppWindow) {}

#[cfg(target_os = "windows")]
fn query_true_window_placement_from_app(window: &AppWindow) -> WindowPlacementKind {
    use slint::winit_030::WinitWindowAccessor;

    window
        .window()
        .with_winit_window(query_true_window_placement)
        .flatten()
        .unwrap_or(WindowPlacementKind::Unknown)
}

fn load_ui_preferences(store: &Option<Rc<UiPreferencesStore>>) -> UiPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                tracing::error!(
                    target: "config.preferences",
                    error = %err,
                    "failed to load ui preferences"
                );
                UiPreferences::default()
            }
        },
        None => UiPreferences::default(),
    }
}

fn load_quick_launch_preferences(
    store: &Option<Rc<QuickLaunchPreferencesStore>>,
) -> QuickLaunchPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                tracing::error!(
                    target: "config.quick_launch_preferences",
                    error = %err,
                    "failed to load quick launch preferences"
                );
                QuickLaunchPreferences::default()
            }
        },
        None => QuickLaunchPreferences::default(),
    }
}

fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store
        && let Err(err) = store.save(&UiPreferences::from(state))
    {
        tracing::error!(
            target: "config.preferences",
            error = %err,
            "failed to save ui preferences"
        );
    }
}

fn save_quick_launch_preferences(
    store: &Option<Rc<QuickLaunchPreferencesStore>>,
    prefs: &QuickLaunchPreferences,
) {
    if let Some(store) = store
        && let Err(err) = store.save(prefs)
    {
        tracing::error!(
            target: "config.quick_launch_preferences",
            error = %err,
            "failed to save quick launch preferences"
        );
    }
}

fn save_quick_launch_preferences_from_state(
    store: &Option<Rc<QuickLaunchPreferencesStore>>,
    state: &ShellViewModel,
) {
    save_quick_launch_preferences(store, state.quick_launch_preferences());
}

fn empty_asset_catalog() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: Vec::new(),
        nodes: BTreeMap::new(),
    }
}

fn load_asset_catalog(repo: &dyn AssetCatalogRepository) -> PersistedAssetCatalog {
    match repo.load() {
        Ok(catalog) => catalog,
        Err(err) => {
            tracing::error!(
                target: "config.assets_catalog",
                error = %err,
                "failed to load asset catalog"
            );
            empty_asset_catalog()
        }
    }
}

fn combined_asset_tree(state: &ShellViewModel) -> AssetTree {
    catalog_to_asset_tree(&asset_trees_to_catalog(
        state.console_asset_tree(),
        state.snippet_asset_tree(),
    ))
}

fn save_asset_catalog(repo: &dyn AssetCatalogRepository, state: &ShellViewModel) -> Result<()> {
    let catalog = asset_trees_to_catalog(state.console_asset_tree(), state.snippet_asset_tree());
    repo.save(&catalog)
}

fn save_asset_catalog_if_available(
    repo: &Option<Rc<dyn AssetCatalogRepository>>,
    state: &ShellViewModel,
) {
    if let Some(repo) = repo
        && let Err(err) = save_asset_catalog(repo.as_ref(), state)
    {
        tracing::error!(
            target: "config.assets_catalog",
            error = %err,
            "failed to save asset catalog"
        );
    }
}

fn collect_saved_ssh_asset_ids(
    tree: &AssetTree,
    node_ids: &[String],
    output: &mut BTreeSet<String>,
) {
    for node_id in node_ids {
        let Some(node) = tree.node(node_id) else {
            continue;
        };

        if node.kind == ConsoleAssetKind::SshConnection {
            output.insert(node.id.clone());
        }

        collect_saved_ssh_asset_ids(tree, &node.children, output);
    }
}

fn saved_ssh_asset_ids_for_tree(tree: &AssetTree) -> BTreeSet<String> {
    let mut output = BTreeSet::new();
    collect_saved_ssh_asset_ids(tree, tree.root_ids(), &mut output);
    output
}

fn app_root_paths_for_app() -> Result<AppRootPaths> {
    let project_dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
        .context("project directories are unavailable")?;
    let executable_dir = std::env::current_exe()?
        .parent()
        .context("executable directory is unavailable")?
        .to_path_buf();
    resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: std::env::var_os("MICA_TERM_APP_DIR").map(PathBuf::from),
        executable_dir,
        standard_local_data_dir: project_dirs.data_local_dir().join("MicaTerm"),
        portable_marker_name: ".mica-term-portable",
    })
}

fn asset_catalog_repository_for_app() -> Result<Rc<dyn AssetCatalogRepository>> {
    let app_paths = app_root_paths_for_app()?;
    Ok(Rc::new(RedbAssetCatalogStore::new(app_paths.data_dir)))
}

fn quick_launch_preferences_store_for_app() -> Result<QuickLaunchPreferencesStore> {
    let app_paths = app_root_paths_for_app()?;
    Ok(QuickLaunchPreferencesStore::new(
        app_paths.data_dir.join("quick-launch-preferences.json"),
    ))
}

pub fn bind_top_status_bar_with_store_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo(window, store, effects, None);
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::mainline(),
        effects,
        asset_repo,
    );
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
) {
    let credential_store = shared_app_credential_store();
    let (session_runtime_guard, session_bridge) = match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let session_bridge = Rc::new(ShellSessionBridge {
                manager: SessionManager::new_with_launcher(runtime.handle(), launcher),
            });
            (Some(runtime), Some(session_bridge))
        }
        Err(err) => {
            tracing::error!(
                target: "app.ssh",
                error = %err,
                "failed to create app async runtime for injected shell session launcher"
            );
            (None, None)
        }
    };

    bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
        window,
        store,
        AppRuntimeProfile::mainline(),
        effects,
        asset_repo,
        session_bridge,
        session_runtime_guard,
        credential_store,
        Arc::new(LivePrivateKeyImporter),
        VaultRuntimeOptions::default(),
    );
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer(
        window,
        store,
        effects,
        asset_repo,
        launcher,
        credential_store,
        Arc::new(LivePrivateKeyImporter),
    );
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
) {
    bind_top_status_bar_with_injected_services_and_vault_runtime(
        window,
        store,
        effects,
        asset_repo,
        launcher,
        credential_store,
        private_key_importer,
        VaultRuntimeOptions::default(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn bind_top_status_bar_with_injected_services_and_vault_runtime(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
    vault_runtime: VaultRuntimeOptions,
) {
    let (session_runtime_guard, session_bridge) = match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let session_bridge = Rc::new(ShellSessionBridge {
                manager: SessionManager::new_with_launcher(runtime.handle(), launcher),
            });
            (Some(runtime), Some(session_bridge))
        }
        Err(err) => {
            tracing::error!(
                target: "app.ssh",
                error = %err,
                "failed to create app async runtime for injected shell services"
            );
            (None, None)
        }
    };

    bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
        window,
        store,
        AppRuntimeProfile::mainline(),
        effects,
        asset_repo,
        session_bridge,
        session_runtime_guard,
        credential_store,
        private_key_importer,
        vault_runtime,
    );
}

pub fn bind_top_status_bar_with_store_and_profile_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    _profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
) {
    match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let credential_store = shared_app_credential_store();
            let session_bridge =
                build_session_bridge(runtime.handle(), Arc::clone(&credential_store));
            bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
                window,
                store,
                _profile,
                effects,
                asset_repo,
                Some(session_bridge),
                Some(runtime),
                credential_store,
                Arc::new(LivePrivateKeyImporter),
                VaultRuntimeOptions::default(),
            );
        }
        Err(err) => {
            tracing::error!(
                target: "app.ssh",
                error = %err,
                "failed to create default app async runtime for shell services"
            );
            bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
                window,
                store,
                _profile,
                effects,
                asset_repo,
                None,
                None,
                shared_app_credential_store(),
                Arc::new(LivePrivateKeyImporter),
                VaultRuntimeOptions::default(),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    session_bridge: Option<Rc<ShellSessionBridge>>,
    session_runtime_guard: Option<AppAsyncRuntime>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
    vault_runtime: VaultRuntimeOptions,
) {
    let store = store.map(Rc::new);
    let quick_launch_store = match quick_launch_preferences_store_for_app() {
        Ok(store) => Some(Rc::new(store)),
        Err(err) => {
            tracing::error!(
                target: "config.quick_launch_preferences",
                error = %err,
                "failed to resolve quick launch preferences store"
            );
            None
        }
    };
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    if let Some(repo) = asset_repo.as_ref() {
        let (console_tree, snippet_tree) =
            catalog_to_asset_trees(&load_asset_catalog(repo.as_ref()));
        initial_view_model.replace_console_asset_tree(console_tree);
        initial_view_model.replace_snippet_asset_tree(snippet_tree);
    }
    let quick_launch_preferences = {
        let loaded = load_quick_launch_preferences(&quick_launch_store);
        let filtered = retain_known_ssh_asset_ids(
            &loaded,
            &saved_ssh_asset_ids_for_tree(initial_view_model.console_asset_tree()),
        );
        if filtered != loaded {
            save_quick_launch_preferences(&quick_launch_store, &filtered);
        }
        filtered
    };
    initial_view_model.apply_quick_launch_preferences(quick_launch_preferences);
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    initial_view_model.set_right_panel_view(RightPanelView::from_id(&prefs.right_panel_view));
    let vault_root_dir = vault_runtime
        .root_dir
        .clone()
        .unwrap_or_else(default_vault_runtime_root);
    let initial_local_vault_state = load_local_vault_bootstrap_state(
        vault_root_dir.join("vault-bootstrap-state.json").as_path(),
    )
    .unwrap_or_else(|err| {
        tracing::error!(
            target: "app.vault",
            error = %err,
            "failed to load local vault bootstrap state"
        );
        None
    });
    let initial_vault_session = VaultSessionState::new(
        vault_root_dir,
        Arc::clone(&vault_runtime.provider_factory),
        vault_runtime.bootstrap_template.clone(),
        initial_local_vault_state,
    );
    update_vault_panel_for_local_state(&mut initial_view_model, &initial_vault_session);
    update_sync_modal_for_local_state(&mut initial_view_model, &initial_vault_session);
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let workspace_follow_tracker = Rc::new(RefCell::new(WorkspaceFollowTracker::default()));
    let sftp_browser_controller = Rc::new(RefCell::new(SftpBrowserController::default()));
    let vault_session = Rc::new(RefCell::new(initial_vault_session));
    if let Some(session_bridge_ref) = session_bridge.as_ref()
        && let Err(err) = session_bridge_ref
            .manager
            .set_theme_mode(view_model.borrow().theme_mode)
    {
        tracing::error!(
            target: "app.ssh",
            error = %err,
            "failed to apply initial theme mode to SSH session manager"
        );
    }
    let controller = Rc::new(WindowController::new(window));
    let modal_drag_state = Rc::new(RefCell::new(None::<ModalDragState>));
    let pending_host_key_approval = Rc::new(RefCell::new(None::<PendingHostKeyApproval>));
    let pending_workspace_paste_warning =
        Rc::new(RefCell::new(None::<PendingWorkspacePasteWarning>));
    let asset_click_tracker = Rc::new(RefCell::new(None::<PendingAssetClick>));
    let pending_double_click_activation = Rc::new(RefCell::new(None::<String>));
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        *surface.borrow_mut() = Some(NativeTerminalSurface::attach_or_detach(window));
    });
    install_workspace_terminal_presenter(window, profile);

    apply_restored_window_size(window, default_window_size());
    bind_windows_window_state_tracking(
        window,
        Rc::clone(&view_model),
        Rc::clone(&effects),
        session_bridge.clone(),
        Rc::clone(&pending_workspace_paste_warning),
    );
    sync_shell_state(
        window,
        &view_model.borrow(),
        effects.as_ref(),
        &mut workspace_follow_tracker.borrow_mut(),
    );
    sync_workspace_paste_warning_modal_state(window, None);
    {
        let mut state = view_model.borrow_mut();
        sync_shell_layout(
            window,
            &mut state,
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        );
    }
    install_windows_frame_adapter(window);
    let session_projection_timer = Rc::new(Timer::default());
    if let Some(session_bridge_ref) = session_bridge.as_ref() {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let manager = session_bridge_ref.manager.clone();
        let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
        let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
        let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
        session_projection_timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
            let Some(window) = handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            let projection_delta = sync_workspace_projection_from_manager(&mut state, &manager);
            let should_clear_pending_paste = pending_workspace_paste_warning_ref
                .borrow()
                .as_ref()
                .is_some_and(|pending| {
                    Some(pending.session_id) != active_workspace_session_uuid(&state)
                });
            if should_clear_pending_paste {
                pending_workspace_paste_warning_ref.borrow_mut().take();
                sync_workspace_paste_warning_modal_state(&window, None);
            }
            if projection_delta.tabs_changed {
                sync_workspace_tab_items(&window, &state);
                sync_assets_context_menu_state(&window, &state);
            }
            if projection_delta.any_changed() {
                sync_workspace_session_state_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&manager),
                );
                let (sftp_open_changed, sftp_retry_changed, sftp_follow_changed) = if state.show_right_panel {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    let open_changed =
                        ensure_active_sftp_browser_started(&mut state, &mut controller, &manager);
                    let retry_changed =
                        sync_active_sftp_browser_pending_request(&mut state, &mut controller, &manager);
                    let follow_changed =
                        sync_active_sftp_browser_follow_request(&mut state, &mut controller, &manager);
                    (open_changed, retry_changed, follow_changed)
                } else {
                    (false, false, false)
                };
                if projection_delta.sftp_changed
                    || sftp_open_changed
                    || sftp_retry_changed
                    || sftp_follow_changed
                {
                    sync_right_panel_state(&window, &state);
                }
            }
        });
    }

    let vault_sync_scheduler = Rc::new(RefCell::new(VaultSyncSchedulerState::default()));
    let vault_auto_sync_timer = Rc::new(Timer::default());
    let vault_periodic_sync_timer = Rc::new(Timer::default());
    let run_vault_sync: Rc<dyn Fn(VaultSyncTrigger)> = {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let store_ref = store.clone();
        let effects_ref = Rc::clone(&effects);
        let vault_session_ref = Rc::clone(&vault_session);
        let credential_store_ref = Arc::clone(&credential_store);
        let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
        let scheduler_ref = Rc::clone(&vault_sync_scheduler);
        let auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
        let periodic_timer_keepalive = Rc::clone(&vault_periodic_sync_timer);
        Rc::new(move |trigger| {
            let _keep_periodic_timer_alive = &periodic_timer_keepalive;
            let Some(window) = handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            let mut vault = vault_session_ref.borrow_mut();
            let (width, height) = current_window_size(&window);
            let (should_attempt_push, should_attempt_refresh) = {
                let mut scheduler = scheduler_ref.borrow_mut();
                if scheduler.running {
                    return;
                }
                let should_attempt_push = scheduler.dirty;
                let should_attempt_refresh = !should_attempt_push
                    && matches!(trigger, VaultSyncTrigger::Manual | VaultSyncTrigger::Periodic);
                if matches!(trigger, VaultSyncTrigger::DebouncedAuto | VaultSyncTrigger::Periodic)
                    && !vault_auto_sync_ready(&vault)
                {
                    return;
                }
                if matches!(trigger, VaultSyncTrigger::DebouncedAuto) && !should_attempt_push {
                    return;
                }
                if !should_attempt_push && !should_attempt_refresh {
                    return;
                }
                scheduler.running = true;
                (should_attempt_push, should_attempt_refresh)
            };

            if matches!(trigger, VaultSyncTrigger::Manual) {
                auto_sync_timer_ref.stop();
                state.start_sync_feedback(if should_attempt_push {
                    "Syncing pending changes..."
                } else {
                    "Checking remote sync..."
                });
                sync_shell_state(
                    &window,
                    &state,
                    effects_ref.as_ref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
                sync_shell_layout(&window, &mut state, width, height);
                save_ui_preferences(&store_ref, &state);
            }

            let result = if should_attempt_push {
                sync_local_vault(&mut state, &mut vault, credential_store_ref.as_ref()).map(|_| true)
            } else if should_attempt_refresh {
                refresh_local_vault_from_primary_remote_if_changed(
                    &mut state,
                    &mut vault,
                    credential_store_ref.as_ref(),
                )
            } else {
                Ok(false)
            };

            {
                let mut scheduler = scheduler_ref.borrow_mut();
                scheduler.running = false;
                if result.is_ok() && should_attempt_push {
                    scheduler.dirty = false;
                }
            }

            match result {
                Ok(changed) => {
                    if matches!(trigger, VaultSyncTrigger::Manual) {
                        let feedback = if !state.vault_panel_state().primary_status_label.trim().is_empty()
                        {
                            state.vault_panel_state().primary_status_label.clone()
                        } else if changed {
                            "Sync completed".into()
                        } else {
                            "Sync already up to date".into()
                        };
                        state.show_sync_feedback(feedback);
                    } else {
                        state.clear_sync_feedback();
                    }
                }
                Err(err) => {
                    tracing::error!(
                        target: "app.vault",
                        error = %err,
                        vault_sync_trigger = ?trigger,
                        "failed to run scheduled vault sync"
                    );
                    set_sync_modal_error_without_opening(&mut state, &vault, err.to_string());
                    if matches!(trigger, VaultSyncTrigger::Manual) {
                        state.show_sync_feedback("Sync failed");
                    } else {
                        state.clear_sync_feedback();
                    }
                }
            }

            sync_shell_state(
                &window,
                &state,
                effects_ref.as_ref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
            sync_shell_layout(&window, &mut state, width, height);
            save_ui_preferences(&store_ref, &state);
        })
    };
    {
        let run_vault_sync_ref = Rc::clone(&run_vault_sync);
        vault_periodic_sync_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(VAULT_PERIODIC_SYNC_INTERVAL_MS),
            move || {
                run_vault_sync_ref(VaultSyncTrigger::Periodic);
            },
        );
    }

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_projection_timer_ref = Rc::clone(&session_projection_timer);
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_toggle_right_panel_requested(move || {
        let _keep_session_projection_timer_alive = &session_projection_timer_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.toggle_right_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_right_panel_state(&window, &state);
        sync_shell_layout(&window, &mut state, width, height);
        sync_workspace_session_state_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_open_transfer_center_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_transfer_center();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    let vault_auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
    let _vault_periodic_sync_timer_ref = Rc::clone(&vault_periodic_sync_timer);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_now_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let vault = vault_session_ref.borrow();
        let (width, height) = current_window_size(&window);

        update_sync_modal_for_local_state(&mut state, &vault);
        if matches!(state.sync_modal_state().mode, SyncModalMode::Ready) {
            drop(vault);
            drop(state);
            vault_auto_sync_timer_ref.stop();
            run_vault_sync_ref(VaultSyncTrigger::Manual);
            return;
        } else {
            hydrate_sync_modal_draft(&mut state, &vault, credential_store_ref.as_ref());
            state.open_sync_modal();
        }

        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    window.on_open_sync_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let vault = vault_session_ref.borrow();
        hydrate_sync_modal_draft(&mut state, &vault, credential_store_ref.as_ref());
        update_sync_modal_for_local_state(&mut state, &vault);
        state.open_sync_modal();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_sync_modal_state(&window, &state);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sync_modal_field(field.as_str(), value.to_string());
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_sync_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_toggle_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sync_modal_toggle(field.as_str(), value);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_sync_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_sync_modal();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_sync_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_submit_master_password(move |password| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        let secret = secrecy::SecretString::new(password.to_string().into());
        if let Err(err) = submit_sync_modal_master_password(
            &mut state,
            &mut vault,
            credential_store_ref.as_ref(),
            &secret,
        ) {
            tracing::error!(target: "app.vault", error = %err, "failed to submit sync modal password");
            set_sync_modal_error(&mut state, &vault, err.to_string());
        }
        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_sync_now_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        if let Err(err) = sync_local_vault(&mut state, &mut vault, credential_store_ref.as_ref()) {
            tracing::error!(target: "app.vault", error = %err, "failed to sync local vault from sync modal");
            set_sync_modal_error(&mut state, &vault, err.to_string());
            state.show_sync_feedback("Sync failed");
        } else {
            let feedback = state.vault_panel_state().primary_status_label.clone();
            state.show_sync_feedback(feedback);
        }
        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_lock_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        if let Err(err) = lock_local_vault(&mut state, &mut vault, credential_store_ref.as_ref()) {
            tracing::error!(target: "app.vault", error = %err, "failed to lock local vault from sync modal");
            set_sync_modal_error(&mut state, &vault, format!("Vault lock failed: {err}"));
        }
        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_primary_action_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        let master_password = state.sync_modal_state().master_password.clone();
        match state.sync_modal_state().mode {
            SyncModalMode::NotConfigured => {
                if let Err(err) =
                    persist_sync_modal_settings(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    set_sync_modal_error(&mut state, &vault, err.to_string());
                } else if master_password.trim().is_empty() {
                    state.set_sync_modal_error("Enter a master password to enable sync.");
                } else {
                    let secret = secrecy::SecretString::new(master_password.into());
                    if let Err(err) = submit_sync_modal_master_password(
                        &mut state,
                        &mut vault,
                        credential_store_ref.as_ref(),
                        &secret,
                    ) {
                        tracing::error!(target: "app.vault", error = %err, "failed to enable sync from sync settings");
                        set_sync_modal_error(&mut state, &vault, err.to_string());
                    }
                }
            }
            SyncModalMode::Locked => {
                if master_password.trim().is_empty() {
                    state.set_sync_modal_error("Enter a master password to unlock sync.");
                } else {
                    let secret = secrecy::SecretString::new(master_password.into());
                    if let Err(err) = submit_sync_modal_master_password(
                        &mut state,
                        &mut vault,
                        credential_store_ref.as_ref(),
                        &secret,
                    ) {
                        tracing::error!(target: "app.vault", error = %err, "failed to unlock sync from sync settings");
                        set_sync_modal_error(&mut state, &vault, err.to_string());
                    }
                }
            }
            SyncModalMode::UnlockedButRemoteIncomplete => {
                if let Err(err) =
                    persist_sync_modal_settings(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    set_sync_modal_error(&mut state, &vault, err.to_string());
                }
            }
            SyncModalMode::Ready => {
                if let Err(err) =
                    persist_sync_modal_settings(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    set_sync_modal_error(&mut state, &vault, err.to_string());
                } else if let Err(err) =
                    sync_local_vault(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    tracing::error!(target: "app.vault", error = %err, "failed to sync local vault from primary sync modal action");
                    set_sync_modal_error(&mut state, &vault, err.to_string());
                    state.show_sync_feedback("Sync failed");
                } else {
                    let feedback = state.vault_panel_state().primary_status_label.clone();
                    state.show_sync_feedback(feedback);
                }
            }
            SyncModalMode::SyncError => state.close_sync_modal(),
        }
        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_secondary_action_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        match state.sync_modal_state().mode {
            SyncModalMode::UnlockedButRemoteIncomplete | SyncModalMode::Ready => {
                if let Err(err) =
                    lock_local_vault(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    tracing::error!(target: "app.vault", error = %err, "failed to lock local vault from secondary sync modal action");
                    set_sync_modal_error(&mut state, &vault, format!("Vault lock failed: {err}"));
                }
            }
            SyncModalMode::NotConfigured | SyncModalMode::Locked | SyncModalMode::SyncError => {
                state.close_sync_modal();
            }
        }
        sync_shell_state(
            &window,
            &state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    window.on_open_settings_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_settings_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_right_panel_state(&window, &state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    window.on_open_appearance_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_appearance_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_right_panel_state(&window, &state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_open_sftp_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_sftp_panel();
        if let Some(session_bridge) = session_bridge_ref.as_ref() {
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            let _ = open_active_sftp_browser_for_current_session(
                &mut state,
                &mut controller,
                &session_bridge.manager,
            );
        }
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_right_panel_state(&window, &state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_context_menu_requested(
        move |target_id, target_kind, anchor_x, anchor_y| {
            let window = handle.unwrap();
            let mut state = state.borrow_mut();
            state.open_context_menu_for_target(
                parse_context_target_kind(target_kind.as_str(), SidebarDestination::Console),
                if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                },
                anchor_x,
                anchor_y,
            );
            update_context_menu_placement(&window, &mut state);
            sync_assets_context_menu_state(&window, &state);
        },
    );

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_item_selected(move |entry_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.select_sftp_panel_entry(entry_id.as_str()) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_item_activated(move |entry_id, item_kind| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let selection_changed = state.select_sftp_panel_entry(entry_id.as_str());
        let entry = state.active_sftp_entry(entry_id.as_str()).cloned();
        let mut panel_changed = selection_changed;
        let was_modal_open = state.sftp_remote_file_editor_state().open;

        if let Some(entry) = entry {
            if item_kind.as_str() == "directory" || entry.kind == SftpDirectoryEntryKind::Directory {
                if let Some(session_bridge) = session_bridge_ref.as_ref()
                    && let Some(session_id) = active_workspace_session_uuid(&state)
                {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, entry.path.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    panel_changed |= execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    );
                }
            } else if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some(session_id) = active_workspace_session_uuid(&state)
            {
                open_sftp_remote_file_editor_for_entry(
                    &mut state,
                    &session_bridge.manager,
                    session_id,
                    entry.path.as_str(),
                );
            }
        }

        if panel_changed {
            sync_right_panel_state(&window, &state);
        }
        sync_sftp_remote_file_modal_state(&window, &state);
        if !was_modal_open && state.sftp_remote_file_editor_state().open {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_open_queue_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_sftp_queue_drawer();
        sync_right_panel_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_path_submitted(move |path| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    false
                } else {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, trimmed)
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                }
            } else {
                false
            }
        } else {
            state.submit_sftp_panel_path(path.to_string())
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_back_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_back(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_back()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_forward_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_forward(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_forward()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_up_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_up()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_refresh_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.refresh(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.refresh_sftp_panel()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_retry_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let retried = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                if let Err(err) = session_bridge.manager.retry_session(session_id) {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to retry active SSH session from SFTP panel"
                    );
                    false
                } else {
                    let projection =
                        sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                    let browser_changed = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        if let Some(request) = controller.retry(session_id) {
                            if session_bridge
                                .manager
                                .sftp_binding(session_id)
                                .is_some_and(|binding| binding.mode() != SftpPanelMode::Disconnected)
                            {
                                execute_sftp_browser_request(
                                    &mut state,
                                    &mut controller,
                                    &session_bridge.manager,
                                    request,
                                )
                            } else {
                                controller.session_state(session_id).is_some_and(|browser_state| {
                                    project_sftp_browser_state_into_view_model(
                                        &mut state,
                                        session_id,
                                        browser_state,
                                    )
                                })
                            }
                        } else {
                            false
                        }
                    };
                    browser_changed
                        || projection.sftp_changed
                        || projection.tabs_changed
                        || projection.surface_changed
                }
            } else {
                false
            }
        } else {
            state.retry_sftp_panel()
        };
        if retried {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_sftp_panel_reenable_follow_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = active_workspace_session_uuid(&state) {
                if let Some(cwd) = session_bridge.manager.current_working_directory(session_id) {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.open(session_id, cwd.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.reenable_sftp_follow()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_sort_requested(move |column_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.cycle_sftp_panel_sort(column_id.as_str()) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_column_width_change_requested(move |column_id, width| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.set_sftp_panel_column_width(column_id.as_str(), width) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_sftp_remote_file_editor();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_content_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sftp_remote_file_editor_content(value.to_string());
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    window.on_sftp_remote_file_modal_save_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if let Some((session_id, remote_path, content)) = state.sftp_remote_file_editor_save_payload()
            && let Some(session_bridge) = session_bridge_ref.as_ref()
        {
            match Uuid::parse_str(session_id.as_str())
                .map_err(anyhow::Error::from)
                .and_then(|session_id| {
                    session_bridge.manager.sftp_upload_file(
                        session_id,
                        remote_path.as_str(),
                        content.into_bytes(),
                    )
                }) {
                Ok(_) => state.mark_sftp_remote_file_editor_saved(),
                Err(err) => state
                    .set_sftp_remote_file_editor_error(format!("Failed to save remote file: {err}")),
            }
        }
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_theme_mode();
        if let Some(session_bridge) = session_bridge_ref.as_deref() {
            if let Err(err) = session_bridge.manager.set_theme_mode(state.theme_mode) {
                tracing::error!(
                    target: "app.ssh",
                    error = %err,
                    theme_mode = ?state.theme_mode,
                    "failed to synchronize theme mode into SSH sessions"
                );
            }
            let projection =
                sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
            if projection.tabs_changed || projection.surface_changed {
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
        }
        sync_theme_and_window_effects(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    window.on_toggle_window_always_on_top_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_always_on_top();
        window.set_is_window_always_on_top(state.is_always_on_top);
        save_ui_preferences(&store_ref, &state);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_minimize_requested(move || {
        controller_ref.minimize();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    let effects_ref = Rc::clone(&effects);
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_sidebar_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_assets_sidebar();
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sidebar_destination_selected(move |destination_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        let destination = SidebarDestination::from_id(destination_id.as_str())
            .unwrap_or(SidebarDestination::Console);
        state.select_sidebar_destination(destination);
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_new_tab_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.open_workspace_launcher_tab();
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_asset_selected(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_quick_launch_asset(asset_id.to_string());
        sync_welcome_quick_launch_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_search_changed(move |query| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_quick_launch_search_query(query.to_string());
        sync_welcome_quick_launch_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_toggle_favorite_requested(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_quick_launch_favorite(asset_id.as_str());
        sync_welcome_quick_launch_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_connect_requested(move |asset_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state
            .active_workspace_tab()
            .is_some_and(|tab| tab.is_launcher())
        {
            state.close_workspace_launcher_tab();
        }
        open_saved_ssh_asset_from_quick_launch(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            asset_id.as_str(),
            OpenSessionMode::ForceNewTab,
        );
        sync_welcome_quick_launch_state(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_saved_ssh_picker_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_connect_in_new_tab_requested(move |asset_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state
            .active_workspace_tab()
            .is_some_and(|tab| tab.is_launcher())
        {
            state.close_workspace_launcher_tab();
        }
        open_saved_ssh_asset_from_quick_launch(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            asset_id.as_str(),
            OpenSessionMode::ForceNewTab,
        );
        sync_welcome_quick_launch_state(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_saved_ssh_picker_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_welcome_open_saved_ssh_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.open_saved_ssh_picker();
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_open_saved_ssh_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_saved_ssh_picker();
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_open_saved_ssh_modal_query_changed(move |query| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_saved_ssh_picker_query(query.to_string());
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_open_saved_ssh_modal_asset_selected(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_saved_ssh_picker_asset(asset_id.to_string());
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_open_saved_ssh_modal_toggle_expanded_requested(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_saved_ssh_picker_expanded(asset_id.as_str());
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_open_saved_ssh_modal_asset_activated(move |asset_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        match state.console_asset_tree().kind(asset_id.as_str()) {
            Some(ConsoleAssetKind::Folder) => {
                state.toggle_saved_ssh_picker_expanded(asset_id.as_str());
                sync_saved_ssh_picker_state(&window, &state);
                return;
            }
            Some(ConsoleAssetKind::SshConnection) => {}
            _ => return,
        }
        state.close_saved_ssh_picker();
        if state
            .active_workspace_tab()
            .is_some_and(|tab| tab.is_launcher())
        {
            state.close_workspace_launcher_tab();
        }
        open_saved_ssh_asset_from_quick_launch(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            asset_id.as_str(),
            OpenSessionMode::ForceNewTab,
        );
        sync_welcome_quick_launch_state(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_saved_ssh_picker_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_welcome_quick_launch_reveal_in_assets_requested(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_quick_launch_asset(asset_id.to_string());
        state.select_sidebar_destination(SidebarDestination::Console);
        state.select_asset(asset_id.as_str());
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.activate_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_search_query_changed(move |query| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.set_keychain_search_query(query.to_string());
        } else {
            state.set_asset_search_query(query.to_string());
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_collapse_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.collapse_asset_search_if_empty();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_view_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_view_mode();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_tree_expansion_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_tree_expansion();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_create_action_selected(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();
        state.dismiss_empty_asset_search_on_shell_interaction();
        if state.active_sidebar_destination == SidebarDestination::Snippets {
            state.handle_snippet_create_action(action_id.as_str());
            open_pending_snippet_create_modal(&mut state);
        } else {
            state.handle_assets_create_action(action_id.as_str());
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_close_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.cancel_asset_modal();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let credential_store_ref = Arc::clone(&credential_store);
    let vault_session_ref = Rc::clone(&vault_session);
    let vault_sync_scheduler_ref = Rc::clone(&vault_sync_scheduler);
    let vault_auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    window.on_confirm_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let pending_keychain_draft = match state.asset_modal_state.as_ref() {
            Some(AssetModalState::NewKeychainSshKey { draft, .. }) => Some(draft.clone()),
            _ => None,
        };
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            if let Some(draft) = pending_keychain_draft.as_ref()
                && let Some(key_id) = state.focused_keychain_id.clone()
                && let Err(err) = persist_keychain_ssh_key_secret(
                    credential_store_ref.as_ref(),
                    key_id.as_str(),
                    draft,
                )
            {
                tracing::error!(
                    target: "app.keychain",
                    key_id,
                    error = %err,
                    "failed to persist keychain SSH key secret bundle"
                );
            }
            save_asset_catalog_if_available(&asset_repo_ref, &state);
            let vault = vault_session_ref.borrow();
            mark_local_vault_dirty_and_arm_auto_sync(
                &mut state,
                &vault,
                &vault_sync_scheduler_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_rename_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_rename_asset_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let vault_session_ref = Rc::clone(&vault_session);
    let vault_sync_scheduler_ref = Rc::clone(&vault_sync_scheduler);
    let vault_auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    window.on_confirm_asset_rename_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
            let vault = vault_session_ref.borrow();
            mark_local_vault_dirty_and_arm_auto_sync(
                &mut state,
                &vault,
                &vault_sync_scheduler_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let vault_session_ref = Rc::clone(&vault_session);
    let vault_sync_scheduler_ref = Rc::clone(&vault_sync_scheduler);
    let vault_auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    window.on_confirm_delete_asset_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let did_mutate = state.confirm_delete_asset();
        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
            let vault = vault_session_ref.borrow();
            mark_local_vault_dirty_and_arm_auto_sync(
                &mut state,
                &vault,
                &vault_sync_scheduler_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_folder_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_new_folder_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_snippet_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_snippet_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_snippet_package_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_snippet_package_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_ssh_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_ssh_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_keychain_ssh_key_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_keychain_ssh_key_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let credential_store_ref = Arc::clone(&credential_store);
    let private_key_importer_ref = Arc::clone(&private_key_importer);
    let vault_session_ref = Rc::clone(&vault_session);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let vault_sync_scheduler_ref = Rc::clone(&vault_sync_scheduler);
    let vault_auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    window.on_asset_ssh_modal_action_requested(move |action| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if action.as_str() == "import-private-key" {
            if let Err(err) =
                import_private_key_into_ssh_modal(&mut state, private_key_importer_ref.as_ref())
            {
                state.finish_ssh_modal_action_error(err.to_string());
            }
            sync_asset_modal_state(&window, &state);
            return;
        }
        let accepted = state.begin_ssh_modal_action(action.as_str());
        let pending_action = state.take_pending_ssh_modal_action();
        let mut did_mutate = false;
        let mut catalog_persisted_in_action = false;

        if let Some(request) = pending_action {
            match request.action {
                SshModalAction::Save => {
                    let previous_state = (*state).clone();
                    let existing_saved_spec = match &state.asset_modal_state {
                        Some(AssetModalState::NewSshConnection {
                            editing_asset_id: Some(asset_id),
                            ..
                        }) => state
                            .console_asset_tree()
                            .ssh_connection_spec(asset_id)
                            .cloned(),
                        _ => None,
                    };
                    did_mutate = state.confirm_asset_modal();
                    if !did_mutate {
                        state.finish_ssh_modal_action_error("Failed to save connection.");
                    } else if let Some(asset_id) = state.focused_asset_id.clone() {
                        if let Err(err) = validate_saved_modal_profile(&state, &asset_id) {
                            *state = previous_state;
                            did_mutate = false;
                            state.finish_ssh_modal_action_error(err.to_string());
                        } else if let Some(saved_spec) =
                            state
                                .console_asset_tree()
                                .ssh_connection_spec(&asset_id)
                                .cloned()
                            && let Err(err) = sync_saved_ssh_secrets(
                                credential_store_ref.as_ref(),
                                &request.draft,
                                existing_saved_spec.as_ref(),
                                &saved_spec,
                            )
                        {
                            *state = previous_state;
                            did_mutate = false;
                            state.finish_ssh_modal_action_error(err.to_string());
                        }
                    }
                }
                SshModalAction::TestConnection => {
                    match runtime_profile_for_modal_action(&state, &request.draft) {
                        Ok(profile) => {
                            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                                attempt_test_connection(
                                    &mut state,
                                    session_bridge.as_ref(),
                                    &pending_host_key_approval_ref,
                                    profile,
                                );
                            } else {
                                state.finish_ssh_modal_action_error(
                                    "SSH session bridge is unavailable.",
                                );
                            }
                        }
                        Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                    }
                }
                SshModalAction::Connect => match runtime_profile_for_modal_action(&state, &request.draft) {
                    Ok(mut profile) => {
                        profile.asset_id = Some(temporary_session_asset_id_for_profile(&profile));
                        if let Some(session_bridge) = session_bridge_ref.as_ref() {
                            if let Err(err) = attempt_open_session_with_profile(
                                &mut state,
                                session_bridge.as_ref(),
                                &pending_host_key_approval_ref,
                                profile,
                                OpenSessionMode::ActivateExisting,
                            ) {
                                tracing::error!(
                                    target: "app.ssh",
                                    error = %err,
                                    "failed to open temporary ssh session from modal action"
                                );
                                state.finish_ssh_modal_action_error(err.to_string());
                            } else {
                                state.cancel_asset_modal();
                            }
                        } else {
                            state.finish_ssh_modal_action_error(
                                "SSH session bridge is unavailable.",
                            );
                        }
                    }
                    Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                },
                SshModalAction::SaveAndConnect => {
                    let previous_state = (*state).clone();
                    let existing_saved_spec = match &state.asset_modal_state {
                        Some(AssetModalState::NewSshConnection {
                            editing_asset_id: Some(asset_id),
                            ..
                        }) => state
                            .console_asset_tree()
                            .ssh_connection_spec(asset_id)
                            .cloned(),
                        _ => None,
                    };
                    did_mutate = state.confirm_asset_modal();
                    if did_mutate {
                        if let Some(asset_id) = state.focused_asset_id.clone() {
                            if let Err(err) = validate_saved_modal_profile(&state, &asset_id) {
                                *state = previous_state;
                                did_mutate = false;
                                state.finish_ssh_modal_action_error(err.to_string());
                            } else if let Some(saved_spec) = state
                                .console_asset_tree()
                                .ssh_connection_spec(&asset_id)
                                .cloned()
                            {
                                if let Err(err) = sync_saved_ssh_secrets(
                                    credential_store_ref.as_ref(),
                                    &request.draft,
                                    existing_saved_spec.as_ref(),
                                    &saved_spec,
                                ) {
                                    *state = previous_state;
                                    did_mutate = false;
                                    state.finish_ssh_modal_action_error(err.to_string());
                                } else {
                                    if let Some(repo) = asset_repo_ref.as_ref()
                                        && let Err(err) = save_asset_catalog(repo.as_ref(), &state)
                                    {
                                        *state = previous_state;
                                        did_mutate = false;
                                        state.finish_ssh_modal_action_error(err.to_string());
                                    } else {
                                        catalog_persisted_in_action = asset_repo_ref.is_some();
                                        match runtime_profile_for_saved_asset(&state, &asset_id) {
                                            Ok(profile) => {
                                                if let Some(session_bridge) = session_bridge_ref.as_ref()
                                                    && let Err(err) = attempt_open_session_with_profile(
                                                        &mut state,
                                                        session_bridge.as_ref(),
                                                        &pending_host_key_approval_ref,
                                                        profile,
                                                        OpenSessionMode::ActivateExisting,
                                                    )
                                                {
                                                    tracing::error!(
                                                        target: "app.ssh",
                                                        error = %err,
                                                        "failed to open ssh session from modal action"
                                                    );
                                                    state.finish_ssh_modal_action_error(err.to_string());
                                                } else {
                                                    state.cancel_asset_modal();
                                                }
                                            }
                                            Err(err) => {
                                                state.finish_ssh_modal_action_error(err.to_string());
                                            }
                                        }
                                    }
                                }
                            } else {
                                *state = previous_state;
                                did_mutate = false;
                                state.finish_ssh_modal_action_error(
                                    "Failed to resolve saved secret target after saving connection.",
                                );
                            }
                        } else {
                            state.finish_ssh_modal_action_error(
                                "Failed to resolve saved connection profile.",
                            );
                        }
                    } else {
                        state.finish_ssh_modal_action_error(
                            "Failed to save connection before opening session.",
                        );
                    }
                }
            }
        } else if accepted {
            state.finish_ssh_modal_action_error("SSH modal action did not produce a request.");
        }

        if did_mutate && !catalog_persisted_in_action {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        if did_mutate {
            let vault = vault_session_ref.borrow();
            mark_local_vault_dirty_and_arm_auto_sync(
                &mut state,
                &vault,
                &vault_sync_scheduler_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_assets_context_menu_state(&window, &state);
        sync_asset_modal_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_ssh_host_key_modal_accept_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.accept_ssh_host_key_prompt();
        resolve_pending_host_key(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            true,
        );
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_ssh_host_key_modal_state(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let private_key_importer_ref = Arc::clone(&private_key_importer);
    window.on_keychain_ssh_key_modal_action_requested(move |action| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let result = match action.as_str() {
            "import-private-key" => import_private_key_into_keychain_modal(
                &mut state,
                private_key_importer_ref.as_ref(),
            ),
            "import-public-key" => {
                import_public_key_into_keychain_modal(&mut state, private_key_importer_ref.as_ref())
            }
            "paste-private-key" => {
                paste_private_key_into_keychain_modal(&mut state);
                Ok(())
            }
            "paste-public-key" => {
                paste_public_key_into_keychain_modal(&mut state);
                Ok(())
            }
            "generate-key-pair" => generate_key_pair_into_keychain_modal(&mut state),
            "copy-public-key" => copy_public_key_from_keychain_modal(&state),
            _ => Ok(()),
        };
        if let Err(err) = result {
            tracing::error!(
                target: "app.keychain",
                action = action.as_str(),
                error = %err,
                "failed to handle keychain SSH key modal action"
            );
        }
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_ssh_host_key_modal_reject_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.reject_ssh_host_key_prompt();
        resolve_pending_host_key(&mut state, None, &pending_host_key_approval_ref, false);
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            None,
        );
        sync_ssh_host_key_modal_state(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_drag_requested(move |pointer_x, pointer_y| {
        let window = handle.unwrap();
        let current_offset = ModalOffset {
            x: window.get_blocking_modal_offset_x(),
            y: window.get_blocking_modal_offset_y(),
        };
        *modal_drag_state_ref.borrow_mut() =
            Some(begin_modal_drag(pointer_x, pointer_y, current_offset));
    });

    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_drag_moved(move |pointer_x, pointer_y| {
        let Some(drag_state) = *modal_drag_state_ref.borrow() else {
            return;
        };
        let window = handle.unwrap();
        let next_offset = update_modal_drag(drag_state, pointer_x, pointer_y);
        window.set_blocking_modal_offset_x(next_offset.x);
        window.set_blocking_modal_offset_y(next_offset.y);
    });

    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_drag_ended(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_focus_restore_requested(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let handle = window.as_weak();
    let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
    window.on_workspace_paste_warning_cancel_requested(move || {
        let window = handle.unwrap();
        pending_workspace_paste_warning_ref.borrow_mut().take();
        sync_workspace_paste_warning_modal_state(&window, None);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_paste_warning_confirm_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let pending = pending_workspace_paste_warning_ref.borrow_mut().take();
        let draft_text = window.get_workspace_paste_warning_text().to_string();
        sync_workspace_paste_warning_modal_state(&window, None);
        let Some(pending) = pending else {
            return;
        };
        if active_workspace_session_uuid(&state) != Some(pending.session_id) {
            return;
        }
        let text = if matches!(pending.prompt_mode, WorkspacePastePromptMode::Editor) {
            draft_text
        } else {
            pending.text.clone()
        };

        forward_workspace_session_paste(
            &state,
            session_bridge_ref.as_deref(),
            pending.session_id,
            &text,
        );
        refresh_active_workspace_projection(
            &window,
            &mut state,
            session_bridge_ref.as_deref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
    window.on_workspace_tab_selected(move |session_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.activate_workspace_session(session_id.as_str()) {
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                let (rows, cols) = state
                    .active_workspace_terminal_surface()
                    .map(|surface| (surface.rows as i32, surface.cols as i32))
                    .unwrap_or((24, 80));
                forward_active_workspace_resize(&state, Some(session_bridge), rows, cols);
            }
            sync_workspace_tabs_with_manager(
                &window,
                &state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            if state.show_right_panel
                && let Some(session_bridge) = session_bridge_ref.as_ref()
            {
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                let _ = open_active_sftp_browser_for_current_session(
                    &mut state,
                    &mut controller,
                    &session_bridge.manager,
                );
            }
            sync_right_panel_state(&window, &state);
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_tab_close_requested(move |session_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if close_session_by_id(
            &mut state,
            session_bridge_ref.as_deref(),
            session_id.as_str(),
        ) {
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                let (rows, cols) = state
                    .active_workspace_terminal_surface()
                    .map(|surface| (surface.rows as i32, surface.cols as i32))
                    .unwrap_or((24, 80));
                forward_active_workspace_resize(&state, Some(session_bridge), rows, cols);
            }
            sync_workspace_tabs_with_manager(
                &window,
                &state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_local_action_requested(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        match action_id.as_str() {
            "new-tab" => {
                let Some(asset_id) = state
                    .active_workspace_tab()
                    .map(|tab| tab.asset_id.clone())
                    .filter(|asset_id| !asset_id.is_empty())
                else {
                    return;
                };
                activate_asset(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &pending_host_key_approval_ref,
                    asset_id.as_str(),
                );
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                );
                sync_assets_context_menu_state(&window, &state);
                sync_ssh_host_key_modal_state(&window, &state);
            }
            "close-tab" => {
                let Some(session_id) = state.active_workspace_session_id().map(str::to_owned)
                else {
                    return;
                };
                if close_session_by_id(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    session_id.as_str(),
                ) {
                    if let Some(session_bridge) = session_bridge_ref.as_ref() {
                        let _ = sync_workspace_projection_from_manager(
                            &mut state,
                            &session_bridge.manager,
                        );
                        let (rows, cols) = state
                            .active_workspace_terminal_surface()
                            .map(|surface| (surface.rows as i32, surface.cols as i32))
                            .unwrap_or((24, 80));
                        forward_active_workspace_resize(&state, Some(session_bridge), rows, cols);
                    }
                    sync_workspace_tabs_with_manager(
                        &window,
                        &state,
                        &mut workspace_follow_tracker_ref.borrow_mut(),
                        session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                    );
                    sync_assets_context_menu_state(&window, &state);
                }
            }
            "toggle-asset-search" => {
                state.activate_asset_search();
                sync_assets_toolbar_state(&window, &state);
                sync_console_assets(&window, &state);
                sync_keychain_assets(&window, &state);
            }
            "toggle-global-menu" => {
                state.toggle_global_menu();
                window.set_show_global_menu(state.show_global_menu);
            }
            "cancel-connection-attempt" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                let Some(session_id) = active_workspace_session_uuid(&state) else {
                    return;
                };
                let _ = session_bridge.manager.cancel_connection_attempt(session_id);
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            "retry-connection-attempt" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                let Some(session_id) = active_workspace_session_uuid(&state) else {
                    return;
                };
                if let Err(err) = session_bridge.manager.retry_session(session_id) {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to retry workspace ssh connection attempt"
                    );
                    return;
                }
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            "trust-host-key" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                let Some(session_id) = active_workspace_session_uuid(&state) else {
                    return;
                };
                let Some(prompt) = session_bridge
                    .manager
                    .connection_attempt(session_id)
                    .and_then(|attempt| attempt.prompt)
                else {
                    return;
                };
                let accept_result = (|| -> anyhow::Result<()> {
                    let public_key = PublicKey::from_openssh(prompt.public_key_openssh.as_str())
                        .context("failed to parse accepted SSH host key")?;
                    let known_hosts = KnownHostsService::new(default_known_hosts_path()?);
                    known_hosts.accept_unknown(prompt.host.as_str(), prompt.port, &public_key)?;
                    let _ = session_bridge.manager.retry_session(session_id)?;
                    Ok(())
                })();
                if let Err(err) = accept_result {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to trust workspace ssh host key"
                    );
                    return;
                }
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            "reject-host-key" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                let Some(session_id) = active_workspace_session_uuid(&state) else {
                    return;
                };
                let _ = session_bridge.manager.reject_host_key_prompt(session_id);
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
                sync_workspace_tabs_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            _ => {}
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_text_input(move |text| {
        let mut state = state.borrow_mut();
        forward_active_workspace_text_input(&state, session_bridge_ref.as_deref(), text.as_str());
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_key_input(move |key, alt, ctrl, shift| {
        let mut state = state.borrow_mut();
        forward_active_workspace_key_input(
            &state,
            session_bridge_ref.as_deref(),
            key.as_str(),
            alt,
            ctrl,
            shift,
        );
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    window.on_workspace_session_resize_requested(move |rows, cols| {
        let state = state.borrow();
        forward_active_workspace_resize(&state, session_bridge_ref.as_deref(), rows, cols);
    });

    let state = Rc::clone(&view_model);
    let window_handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_selection_changed(move || {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let state = state.borrow();
        sync_workspace_session_state_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    window.on_workspace_session_copy_selection_requested(
        move |start_row, start_col, end_row, end_col| {
            let state = state.borrow();
            forward_active_workspace_copy_selection(&state, start_row, start_col, end_row, end_col);
        },
    );

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_paste_requested(move || {
        let mut state = state.borrow_mut();
        let outcome = forward_active_workspace_paste(
            &state,
            session_bridge_ref.as_deref(),
            pending_workspace_paste_warning_ref.as_ref(),
        );
        if let Some(window) = window_handle.upgrade() {
            let pending = pending_workspace_paste_warning_ref.borrow();
            sync_workspace_paste_warning_modal_state(&window, pending.as_ref());
            if matches!(outcome, WorkspacePasteRequestOutcome::Sent) {
                refresh_active_workspace_projection(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
            }
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_mouse_input(move |kind, button, row, col, shift, ctrl, alt| {
        let mut state = state.borrow_mut();
        let Some(kind) = parse_terminal_mouse_kind(kind.as_str()) else {
            tracing::warn!(
                target: "app.ssh",
                kind = %kind,
                "ignored unknown workspace terminal mouse kind"
            );
            return;
        };
        let Some(button) = parse_terminal_mouse_button(button.as_str()) else {
            tracing::warn!(
                target: "app.ssh",
                button = %button,
                "ignored unknown workspace terminal mouse button"
            );
            return;
        };
        forward_active_workspace_mouse_input(
            &state,
            session_bridge_ref.as_deref(),
            TerminalMouseInput {
                kind,
                button,
                row: row.max(0) as u32,
                col: col.max(0) as u32,
                shift,
                ctrl,
                alt,
            },
        );
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_scroll_requested(move |delta_lines, row, col, shift, ctrl, alt| {
        let mut state = state.borrow_mut();
        forward_active_workspace_scroll(
            &state,
            session_bridge_ref.as_deref(),
            WorkspaceScrollInput {
                delta_lines,
                row,
                col,
                shift,
                ctrl,
                alt,
            },
        );

        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_scroll_thumb_drag_requested(move |ratio| {
        let mut state = state.borrow_mut();
        forward_active_workspace_scroll_ratio(&state, session_bridge_ref.as_deref(), ratio);
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_scroll_jump_requested(move |ratio| {
        let mut state = state.borrow_mut();
        forward_active_workspace_scroll_ratio(&state, session_bridge_ref.as_deref(), ratio);
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_jump_to_latest_requested(move || {
        let mut state = state.borrow_mut();
        forward_active_workspace_scroll_ratio(&state, session_bridge_ref.as_deref(), 0.0);
        if let Some(window) = window_handle.upgrade() {
            refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let asset_click_tracker_ref = Rc::clone(&asset_click_tracker);
    let pending_double_click_activation_ref = Rc::clone(&pending_double_click_activation);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_asset_selected(move |item_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.select_keychain_item(item_id.as_str());
            asset_click_tracker_ref.borrow_mut().take();
            pending_double_click_activation_ref.borrow_mut().take();
        } else {
            state.select_asset(item_id.as_str());
            let should_activate =
                register_asset_click(&asset_click_tracker_ref, item_id.as_str(), Instant::now());
            if should_activate {
                pending_double_click_activation_ref
                    .borrow_mut()
                    .replace(item_id.to_string());
                activate_asset(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &pending_host_key_approval_ref,
                    item_id.as_str(),
                );
                apply_pending_snippet_activation(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_assets_context_menu_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let asset_click_tracker_ref = Rc::clone(&asset_click_tracker);
    let pending_double_click_activation_ref = Rc::clone(&pending_double_click_activation);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_asset_activated(move |item_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            asset_click_tracker_ref.borrow_mut().take();
            pending_double_click_activation_ref.borrow_mut().take();
            state.select_keychain_item(item_id.as_str());
        } else {
            asset_click_tracker_ref.borrow_mut().take();
            state.select_asset(item_id.as_str());
            let skip_duplicate = pending_double_click_activation_ref
                .borrow()
                .as_ref()
                .map(|asset_id| asset_id == item_id.as_str())
                .unwrap_or(false);
            pending_double_click_activation_ref.borrow_mut().take();
            if !skip_duplicate {
                activate_asset(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &pending_host_key_approval_ref,
                    item_id.as_str(),
                );
                apply_pending_snippet_activation(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_assets_context_menu_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_expanded_requested(move |item_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.toggle_keychain_folder_expanded(item_id.as_str());
        } else {
            state.toggle_folder_expanded(item_id.as_str());
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_context_menu_requested(move |target_id, target_kind, anchor_x, anchor_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let active_sidebar_destination = state.active_sidebar_destination;
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.open_context_menu_for_target(
            parse_context_target_kind(target_kind.as_str(), active_sidebar_destination),
            if target_id.is_empty() {
                None
            } else {
                Some(target_id.to_string())
            },
            anchor_x,
            anchor_y,
        );
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_interaction_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.dismiss_empty_asset_search_on_shell_interaction() {
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_assets_context_menu_action_invoked(move |action_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();

        if let Some((path, action)) = context_menu_action_entry_for(&state, action_id.as_str()) {
            if !action.children.is_empty() {
                state.set_context_menu_open_path(path);
            } else if action.state == ContextMenuActionState::Enabled {
                match action_id.as_str() {
                    "open-connection" => {
                        let target_asset_id = state.context_target_asset_id.clone();
                        state.close_context_menu();
                        if let Some(asset_id) = target_asset_id {
                            activate_asset(
                                &mut state,
                                session_bridge_ref.as_deref(),
                                &pending_host_key_approval_ref,
                                &asset_id,
                            );
                        }
                    }
                    _ => state.handle_context_menu_leaf_action(action_id.as_str()),
                }
            } else {
                state.handle_context_menu_leaf_action(action_id.as_str());
            }
        }

        apply_pending_snippet_activation(
            &window,
            &mut state,
            session_bridge_ref.as_deref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        hydrate_edit_ssh_modal_secret_from_store(&mut state, credential_store_ref.as_ref());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_asset_modal_state(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
        sync_ssh_host_key_modal_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_key_pressed(move |command| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();

        match command.as_str() {
            "escape" => state.handle_context_menu_escape(),
            "left" => state.navigate_context_menu_left(),
            "right" => state.navigate_context_menu_right(),
            "enter" => state.invoke_current_context_menu_item(),
            _ => {}
        }

        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_row_hovered(move |column_index, row_index| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next_path =
            context_menu_hover_path_for(&state, column_index as usize, row_index as usize);
        state.hover_context_menu_path(next_path);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_pointer_moved(move |pointer_x, pointer_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.context_menu_open {
            return;
        }

        let pointer = (pointer_x, pointer_y);
        let rects = context_menu_column_rects_for(&state);
        let original_path = state.context_menu_open_path.clone();

        if state.context_menu_open_path.len() >= 2
            && let (Some(parent_rect), Some(child_rect)) = (rects[1], rects[2])
            && !should_keep_corridor_open(pointer, parent_rect, child_rect)
        {
            state.truncate_context_menu_open_path(1);
        }

        if !state.context_menu_open_path.is_empty()
            && let (Some(parent_rect), Some(child_rect)) = (rects[0], rects[1])
        {
            let keep_open = should_keep_corridor_open(pointer, parent_rect, child_rect);
            if !keep_open {
                state.truncate_context_menu_open_path(0);
            }
        }

        if state.context_menu_open_path != original_path {
            update_context_menu_placement(&window, &mut state);
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_context_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_context_menu();
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, height| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        sync_shell_layout(&window, &mut state, width as u32, height as u32);
        install_windows_frame_adapter(&window);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_close_requested(move || {
        let _ = controller_ref.close();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_requested(move || {
        let _ = controller_ref.drag();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_resize_requested(move |direction| {
        if let Some(direction) = parse_resize_direction(direction.as_str()) {
            let _ = controller_ref.drag_resize(direction);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    let effects_ref = Rc::clone(&effects);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });
}

pub fn bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::mainline(),
        default_platform_window_effects(),
        None,
    );
}

pub fn bind_top_status_bar_with_profile(window: &AppWindow, profile: AppRuntimeProfile) {
    bind_top_status_bar_with_profile_and_async_handle(window, profile, None);
}

fn bind_top_status_bar_with_profile_and_async_handle(
    window: &AppWindow,
    profile: AppRuntimeProfile,
    async_runtime_handle: Option<tokio::runtime::Handle>,
) {
    let store = match UiPreferencesStore::for_app() {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to resolve ui preferences store"
            );
            None
        }
    };
    let asset_repo = match asset_catalog_repository_for_app() {
        Ok(repo) => Some(repo),
        Err(err) => {
            tracing::error!(
                target: "config.assets_catalog",
                error = %err,
                "failed to resolve asset catalog repository"
            );
            None
        }
    };

    let effects = default_platform_window_effects();
    match async_runtime_handle {
        Some(async_runtime_handle) => {
            let credential_store = shared_app_credential_store();
            let session_bridge =
                build_session_bridge(async_runtime_handle, Arc::clone(&credential_store));
            bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
                window,
                store,
                profile,
                effects,
                asset_repo,
                Some(session_bridge),
                None,
                credential_store,
                Arc::new(LivePrivateKeyImporter),
                VaultRuntimeOptions::default(),
            );
        }
        None => {
            bind_top_status_bar_with_store_and_profile_and_effects(
                window, store, profile, effects, asset_repo,
            );
        }
    }
}

pub fn bind_top_status_bar(window: &AppWindow) {
    bind_top_status_bar_with_profile(window, AppRuntimeProfile::mainline());
}

pub fn run() -> Result<()> {
    let async_runtime = AppAsyncRuntime::new()?;
    run_with_profile(AppRuntimeProfile::mainline(), async_runtime.handle())
}

pub fn run_with_profile(
    profile: AppRuntimeProfile,
    async_runtime_handle: tokio::runtime::Handle,
) -> Result<()> {
    let window = AppWindow::new()?;
    window.set_window_title(runtime_window_title(profile).into());
    bind_top_status_bar_with_profile_and_async_handle(&window, profile, Some(async_runtime_handle));
    window.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ssh::credentials::MemoryCredentialStore;
    use crate::app::ssh::profile::SshAuthMethod;
    use crate::app::ssh::runtime::{TerminalCellState, TerminalKeyEvent, TerminalSurfaceState};
    use std::future::Future;
    use std::pin::Pin;

    #[derive(Clone, Default)]
    struct NoopLauncher;

    #[derive(Clone, Default)]
    struct SequencedSurfaceLauncher;

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

    impl SessionRuntimeLauncher for NoopLauncher {
        fn launch(
            &self,
            _profile: ConnectionProfile,
            _session_id: Uuid,
            _attempt_id: Uuid,
            _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
        ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
        {
            Box::pin(
                async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) },
            )
        }

        fn probe(
            &self,
            _profile: ConnectionProfile,
        ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
            Box::pin(async move { Ok(()) })
        }
    }

    impl SessionRuntimeLauncher for SequencedSurfaceLauncher {
        fn launch(
            &self,
            _profile: ConnectionProfile,
            session_id: Uuid,
            _attempt_id: Uuid,
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
                        vec!["welcome".into()],
                    ),
                ));
                let delayed_tx = event_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                    let _ = delayed_tx.send(SessionRuntimeEvent::SurfaceChanged(
                        TerminalSurfaceState::from_visible_lines(
                            session_id,
                            2,
                            24,
                            80,
                            vec!["welcome".into(), "$ pwd".into()],
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

    fn sample_profile(asset_id: &str) -> ConnectionProfile {
        ConnectionProfile {
            asset_id: Some(asset_id.into()),
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
        }
    }

    fn sample_gitee_sync_remote() -> BootstrapRemoteConfig {
        BootstrapRemoteConfig {
            remote_id: "remote-gitee-primary".into(),
            role: RemoteRole::Primary,
            provider: ProviderKind::GiteeGist,
            locator: crate::app::vault::model::BootstrapRemoteLocator::GiteeGist {
                gist_id: "gitee-gist-456".into(),
            },
            credential_ref: Some("vault/bootstrap/remote-gitee-primary".into()),
            auth_kind: ProviderAuthKind::Pat,
            last_health: None,
        }
    }

    #[test]
    fn sync_remote_resolution_inlines_saved_gitee_pat_before_provider_build() {
        let remote = sample_gitee_sync_remote();
        let store = MemoryCredentialStore::default();
        persist_provider_credential(
            &store,
            remote.credential_ref.as_deref().expect("credential ref"),
            Some("gitee-pat"),
        )
        .expect("persist provider credential");

        let resolved = resolve_remote_for_sync(&remote, &store).expect("resolve sync remote");

        assert_eq!(resolved.credential_ref.as_deref(), Some("gitee-pat"));
        assert_eq!(
            remote.credential_ref.as_deref(),
            Some("vault/bootstrap/remote-gitee-primary"),
            "runtime resolution should not mutate the persisted bootstrap remote"
        );
    }

    #[test]
    fn sync_remote_resolution_rejects_missing_saved_gitee_pat() {
        let remote = sample_gitee_sync_remote();
        let store = MemoryCredentialStore::default();

        let err = resolve_remote_for_sync(&remote, &store).expect_err("missing PAT should fail");

        assert!(
            err.to_string()
                .contains("missing saved provider credential for remote `remote-gitee-primary`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn workspace_projection_ignores_locally_derived_active_flag_when_tabs_are_unchanged() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager =
            SessionManager::new_with_launcher(runtime.handle().clone(), Arc::new(NoopLauncher));
        let handle = manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open session");
        let mut state = ShellViewModel::default();

        let delta = sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert!(!delta.surface_changed);
        assert_eq!(
            state.active_workspace_session_id(),
            Some(handle.session_id.to_string().as_str())
        );

        let delta = sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(
            !delta.tabs_changed && !delta.surface_changed,
            "re-running projection without manager changes should not churn tab chrome"
        );
    }

    #[test]
    fn workspace_projection_surface_refresh_does_not_report_a_tab_change() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager = SessionManager::new_with_launcher(
            runtime.handle().clone(),
            Arc::new(SequencedSurfaceLauncher),
        );
        manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open session");
        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(1)).await;
        });

        let mut state = ShellViewModel::default();
        let delta = sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert!(
            !delta.surface_changed,
            "initial projection should establish the active session id before surface hydration"
        );

        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });

        let delta = sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(
            delta.surface_changed,
            "terminal surface refresh should still update the active workspace surface"
        );
        assert!(
            !delta.tabs_changed,
            "terminal surface refresh should not rebuild the workspace tab strip"
        );
        assert_eq!(state.workspace_terminal_surface_seqno(), 2);
        assert_eq!(
            state.workspace_tabs().len(),
            1,
            "surface refresh should not manufacture a second workspace tab"
        );
    }

    #[test]
    fn workspace_session_state_refreshes_terminal_image_across_surface_updates() {
        i_slint_backend_testing::init_no_event_loop();

        let window = AppWindow::new().expect("create app window");
        let session_id = Uuid::new_v4();
        let mut state = ShellViewModel::default();
        let mut tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Connected,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        });
        tab.active = true;
        state.set_workspace_tabs(vec![tab]);
        let mut initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["welcome".into()]);
        initial_surface.cells = vec![TerminalCellState {
            row: 0,
            col: 0,
            width: 1,
            text: "w".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff0d_1117,
        }];
        state.set_active_workspace_terminal_surface(Some(initial_surface));
        let mut follow_tracker = WorkspaceFollowTracker::default();

        sync_workspace_session_state(&window, &state, &mut follow_tracker);
        let initial_lines_model = window.get_workspace_session_visible_lines();
        let initial_image = window.get_workspace_session_surface_image();

        let mut updated_surface = TerminalSurfaceState::from_visible_lines(
            session_id,
            2,
            24,
            80,
            vec!["welcome".into(), "$ pwd".into()],
        );
        updated_surface.cells = vec![TerminalCellState {
            row: 1,
            col: 0,
            width: 1,
            text: "$".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff0d_1117,
        }];
        state.set_active_workspace_terminal_surface(Some(updated_surface));

        sync_workspace_session_state(&window, &state, &mut follow_tracker);

        assert_eq!(
            window.get_workspace_session_visible_lines(),
            initial_lines_model,
            "terminal visible line projection should reuse the same VecModel instance"
        );
        assert_ne!(
            window
                .get_workspace_session_surface_image()
                .to_rgba8()
                .expect("updated terminal atlas image")
                .as_bytes(),
            initial_image
                .to_rgba8()
                .expect("initial terminal atlas image")
                .as_bytes(),
            "terminal atlas image should refresh when the active surface changes"
        );
    }

    #[test]
    fn workspace_session_state_clears_terminal_image_when_surface_clears() {
        i_slint_backend_testing::init_no_event_loop();

        let window = AppWindow::new().expect("create app window");
        let session_id = Uuid::new_v4();
        let mut state = ShellViewModel::default();
        let mut tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Connected,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        });
        tab.active = true;
        state.set_workspace_tabs(vec![tab]);
        let mut initial_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["welcome".into()]);
        initial_surface.cells = vec![TerminalCellState {
            row: 0,
            col: 0,
            width: 1,
            text: "w".into(),
            bold: false,
            underline: false,
            fg_rgba: 0xffff_ffff,
            bg_rgba: 0xff0d_1117,
        }];
        state.set_active_workspace_terminal_surface(Some(initial_surface));
        let mut follow_tracker = WorkspaceFollowTracker::default();

        sync_workspace_session_state(&window, &state, &mut follow_tracker);
        let initial_lines_model = window.get_workspace_session_visible_lines();
        assert_ne!(
            window.get_workspace_session_surface_image(),
            Image::default(),
            "rendered terminal surfaces should publish a non-empty atlas image"
        );

        state.set_active_workspace_terminal_surface(None);
        sync_workspace_session_state(&window, &state, &mut follow_tracker);

        assert_eq!(
            window.get_workspace_session_visible_lines(),
            initial_lines_model,
            "clearing the surface should keep reusing the visible line model"
        );
        assert!(
            window
                .get_workspace_session_surface_image()
                .to_rgba8()
                .is_none(),
            "clearing the surface should reset the atlas image to an empty handle"
        );
        assert_eq!(window.get_workspace_session_visible_lines().row_count(), 0);
    }

    #[test]
    fn native_terminal_clipboard_shortcut_matches_ctrl_shift_copy_and_paste_keys() {
        use slint::winit_030::winit::keyboard::{Key, NamedKey};

        let modifiers = NativeTerminalModifierState {
            ctrl: true,
            shift: true,
            alt: false,
        };

        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Character("C".into()), modifiers),
            Some(NativeTerminalClipboardShortcut::Copy)
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Character("v".into()), modifiers),
            Some(NativeTerminalClipboardShortcut::Paste)
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Named(NamedKey::Copy), modifiers),
            Some(NativeTerminalClipboardShortcut::Copy)
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Named(NamedKey::Paste), modifiers),
            Some(NativeTerminalClipboardShortcut::Paste)
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Character("\u{3}".into()), modifiers),
            Some(NativeTerminalClipboardShortcut::Copy)
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(&Key::Character("\u{16}".into()), modifiers),
            Some(NativeTerminalClipboardShortcut::Paste)
        );
    }

    #[test]
    fn native_terminal_clipboard_shortcut_requires_ctrl_shift_without_alt() {
        use slint::winit_030::winit::keyboard::Key;

        assert_eq!(
            native_terminal_clipboard_shortcut(
                &Key::Character("C".into()),
                NativeTerminalModifierState {
                    ctrl: true,
                    shift: false,
                    alt: false,
                }
            ),
            None
        );
        assert_eq!(
            native_terminal_clipboard_shortcut(
                &Key::Character("V".into()),
                NativeTerminalModifierState {
                    ctrl: true,
                    shift: true,
                    alt: true,
                }
            ),
            None
        );
    }

    #[test]
    fn workspace_multiline_paste_detection_normalizes_platform_line_endings() {
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), ""),
            None
        );
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), "echo hello\n"),
            None
        );
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), "echo hello\r\n"),
            None
        );
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), "echo hello\nwhoami"),
            Some(WorkspacePastePromptMode::Confirm)
        );
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), "echo hello\r\nwhoami"),
            Some(WorkspacePastePromptMode::Confirm)
        );
        assert_eq!(
            workspace_paste_prompt_mode(&ShellViewModel::default(), "echo hello\rwhoami"),
            Some(WorkspacePastePromptMode::Confirm)
        );
    }

    #[test]
    fn workspace_multiline_paste_warning_auto_skips_bracketed_paste_sessions() {
        let session_id = Uuid::new_v4();
        let mut state = ShellViewModel::default();
        let mut tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Connected,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        });
        tab.active = true;
        state.set_workspace_tabs(vec![tab]);
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["$ ".into()]);
        surface.bracketed_paste_enabled = true;
        state.set_active_workspace_terminal_surface(Some(surface));

        assert_eq!(
            workspace_paste_prompt_mode(&state, "echo hello\nwhoami"),
            None
        );
    }

    #[test]
    fn workspace_long_multiline_paste_uses_editor_even_with_bracketed_paste() {
        let session_id = Uuid::new_v4();
        let mut state = ShellViewModel::default();
        let mut tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Connected,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        });
        tab.active = true;
        state.set_workspace_tabs(vec![tab]);
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 24, 80, vec!["$ ".into()]);
        surface.bracketed_paste_enabled = true;
        state.set_active_workspace_terminal_surface(Some(surface));

        assert_eq!(
            workspace_paste_prompt_mode(&state, "one\ntwo\nthree\nfour"),
            Some(WorkspacePastePromptMode::Editor)
        );
    }

    #[test]
    fn terminal_key_event_parses_function_key_names() {
        assert_eq!(
            terminal_key_event("f1", false, false, false),
            Some(TerminalKeyEvent::function(1, false, false, false))
        );
        assert_eq!(
            terminal_key_event("f12", true, false, true),
            Some(TerminalKeyEvent::function(12, true, false, true))
        );
        assert_eq!(
            terminal_key_event("f24", false, true, false),
            Some(TerminalKeyEvent::function(24, false, true, false))
        );
    }

    #[test]
    fn terminal_key_event_preserves_plain_insert_key() {
        assert_eq!(
            terminal_key_event("insert", false, false, false),
            Some(TerminalKeyEvent::named("insert", false, false, false))
        );
    }
}
