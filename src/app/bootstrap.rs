//! Wires the Slint window to runtime state, persisted preferences, and native window hooks during startup.

mod assets_keychain;
mod sftp;
mod shell_chrome;
mod vault_sync;
mod windowing;
mod workspace_terminal;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use slint::{
    Color, ComponentHandle, Image, Model, ModelRc, SharedString, Timer, TimerMode, VecModel,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AppWindow;
use crate::AssetsContextMenuItem;
use crate::ConnectionProgressDiagnosticRow;
use crate::ConnectionProgressFieldRow;
use crate::ConnectionProgressHopRow;
use crate::ConnectionProgressStepRow;
use crate::ConsoleAssetItem;
use crate::QuickLaunchCardRow;
use crate::SftpPanelItem;
use crate::TerminalCommandBlockRow;
use crate::TerminalOverviewMarkerRow;
use crate::TransferCenterItem;
use crate::WorkspaceTabItem;
use crate::app::app_paths::app_root_paths_for_app;
use crate::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    RedbAssetCatalogStore, asset_tree_to_catalog, asset_trees_to_catalog, catalog_to_asset_tree,
    catalog_to_asset_trees,
};
use crate::app::async_runtime::AppAsyncRuntime;
use crate::app::font_diagnostics::{configure_ui_font_fallbacks, log_ui_shell_font_diagnostics};
use crate::app::keychain::{
    KeychainCatalog, KeychainCatalogRepository, KeychainNodePayload, RedbKeychainCatalogStore,
    derive_public_key_material_from_private_key, derive_public_key_material_from_public_key,
    resolve_saved_ssh_profile,
};
use crate::app::quick_launch_preferences::{
    QuickLaunchPreferences, QuickLaunchPreferencesStore, retain_known_ssh_asset_ids,
};
use crate::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, TerminalRenderMode};
use crate::app::sftp::{
    RedbTransferStore, SftpBrowserController, SftpBrowserLoadRequest, SftpBrowserSessionState,
    SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode, restore_tasks_for_bootstrap,
};
use crate::app::ssh::connection_progress::{
    ConnectionAttemptState, ConnectionHeadlineState, ConnectionHopKind, ConnectionHopStateItem,
    ConnectionHopVisualState, ConnectionInfoField, ConnectionPreviewFixture,
    ConnectionPreviewState, ConnectionStepState, ConnectionStepStateItem, ConnectionVisualState,
};
use crate::app::ssh::credentials::{
    CachedCredentialStore, CredentialStore, EncryptedFileCredentialStore, FallbackCredentialStore,
    FileCredentialStore, MirroredCredentialStore, StoredKeychainIdentitySecretBundle,
    StoredKeychainKeySecretBundle, StoredSecretLookupError, StoredSshSecretBundle,
    SystemCredentialStore, keychain_identity_credential_ref, keychain_key_credential_ref,
    load_keychain_identity_secret_bundle, load_keychain_key_secret_bundle,
    load_secret_bundle_with_diagnostics, persist_keychain_identity_secret_bundle,
    persist_keychain_key_secret_bundle, persist_secret_bundle, required_secret_bundle_field,
    restore_snapshot_secret_bundle,
};
use crate::app::ssh::known_hosts::{KnownHostsService, default_known_hosts_path};
use crate::app::ssh::profile::{
    ConnectionProfile, ConnectionProxyProfile, ResolvedProxyHop, SshAuthMethod,
};
use crate::app::ssh::proxy::resolve_proxy_chain;
use crate::app::ssh::runtime::{
    SessionRuntimeEvent, SshSessionRuntime, TerminalKeyEvent, TerminalMouseButton,
    TerminalMouseEventKind, TerminalMouseInput, TerminalRuntimeDefaults, TerminalSurfaceState,
    UnknownHostKeyError, load_optional_stored_secret_bundle, stored_secret_lookup_message,
};
#[cfg(test)]
use crate::app::ssh::session_manager::EnhancedSessionState;
use crate::app::ssh::session_manager::{
    OpenSessionMode, SessionHandle, SessionManager, SessionRuntimeControl, SessionRuntimeLauncher,
    SessionState,
};
use crate::app::terminal_atlas::TerminalAtlasSelection;
use crate::app::terminal_model::TerminalModelFrame;
#[cfg(feature = "terminal-native-renderer")]
use crate::app::terminal_presenter::WindowsNativePresenter;
use crate::app::terminal_presenter::{
    BitmapAtlasPresenter, NativeTerminalFrame, PresentedTerminalFrame, TerminalPresenter,
};
use crate::app::terminal_renderer::{
    NativeTerminalSurface, NativeTerminalSurfaceRect, TerminalRendererHost,
    TerminalRendererHostOptions,
};
use crate::app::terminal_semantic::{
    CommandBlock, CommandBlockStatus, OverviewMarker, OverviewMarkerKind, TerminalSemanticSettings,
    analyze_semantic_annotations_with_settings,
};
use crate::app::terminal_theme::{
    ProjectedThemePreset, projected_theme_for, projected_theme_for_mode, selection_overlay_rgba,
    selection_overlay_rgba_for,
};
#[cfg(any(target_os = "windows", test))]
use crate::app::ui_preferences::PersistedWindowBounds;
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::vault::bootstrap::{
    LocalVaultBootstrapState, bootstrap_provider_credential_ref, load_local_vault_bootstrap_state,
    load_provider_credential, load_runtime_vault_key, persist_provider_credential,
    persist_runtime_vault_key, save_local_vault_bootstrap_state, vault_runtime_key_credential_ref,
};
use crate::app::vault::cache::{load_encrypted_cache, store_encrypted_cache};
use crate::app::vault::conflict_inbox::{
    ConflictInboxEntry, load_conflict_entries, persist_conflict_entries,
};
use crate::app::vault::crypto::{
    WrappedVaultKey, decrypt_snapshot, encrypt_snapshot, generate_vault_key, unwrap_vault_key,
    wrap_vault_key,
};
use crate::app::vault::device_identity::{git_remote_cache_dir, load_or_create_device_id};
use crate::app::vault::engine::{SyncEngine, SyncError, SyncRequest};
use crate::app::vault::merge::{MergeInput, merge_snapshots};
use crate::app::vault::model::{
    BootstrapBundle, BootstrapRemoteConfig, BootstrapRemoteLocator, GitHostKind,
    GitRepoRemoteDraft, KdfConfig, ProviderAuthKind, ProviderKind, RemoteRole,
    SnapshotSyncPreferences, VaultAssetPayload, VaultHead, VaultSnapshot, VaultSshProxySpec,
};
use crate::app::vault::provider::git_repo::{
    GitRepoProvider, GitRepoProviderConfig, GitRepositoryMetadata, GitRepositoryMetadataSource,
    ReqwestGitRepositoryMetadataSource, validate_first_release_git_host, validate_remote_for_sync,
};
use crate::app::vault::provider::gitee_gist::{GiteeGistProvider, GiteeGistProviderConfig};
use crate::app::vault::provider::github_gist::{GitHubGistProvider, GitHubGistProviderConfig};
use crate::app::vault::provider::gitlab_snippet::{
    GitLabSnippetProvider, GitLabSnippetProviderConfig,
};
use crate::app::vault::provider::s3::{S3VaultProvider, S3VaultProviderConfig};
use crate::app::vault::provider::{VaultProvider, first_release_formal_provider_label};
use crate::app::vault::recovery::{
    RecoverySnapshotRecord, RecoverySource, persist_recovery_snapshot,
};
use crate::app::vault::snapshot::{
    apply_vault_snapshot, export_vault_snapshot, normalize_snapshot_secret_refs,
};
use crate::app::vault::sync_decision::{LocalSyncState, SyncAction, decide_sync_action};
use crate::app::vault::sync_service::{
    RemoteHeadSnapshot, VaultSyncExecution, VaultSyncIntent, VaultSyncService,
    VaultSyncServiceConfig, VaultSyncTrigger,
};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
#[cfg(target_os = "windows")]
use crate::app::window_geometry::{
    MonitorWorkArea, persisted_window_bounds_for_placement, resolve_startup_bounds,
};
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{
    ModalDragState, WindowController, apply_restored_window_size, parse_resize_direction,
    window_appearance,
};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
    work_area_from_hmonitor,
};
use crate::app::windows_icon;
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
    AssetModalState, AssetSshConnectionDraft, KeychainIdentityDraft, KeychainSshKeyDraft,
    RightPanelView, ShellViewModel, SnippetActivation, SshModalAction, SyncModalMode,
    SyncModalValidationState, SyncModalViewState, VaultPanelViewState, WorkspaceTabClosePlan,
    WorkspaceTabCloseScope,
};
use crate::theme::{ThemeMode, ThemeVariant};
use russh::keys::ssh_key::{LineEnding, rand_core::OsRng};
use russh::keys::{Algorithm, PrivateKey, PublicKey};

#[derive(Clone)]
pub(super) struct ShellSessionBridge {
    manager: SessionManager,
    terminal_defaults: TerminalRuntimeDefaults,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkspaceTerminalLinkClickCandidate {
    url: String,
}

const MAX_SSH_PROXY_CHAIN_DEPTH: usize = 8;
const WORKSPACE_PASTE_EDITOR_LINE_THRESHOLD: usize = 4;
const WORKSPACE_PASTE_EDITOR_CHAR_THRESHOLD: usize = 5 * 1024;
const FALLBACK_WORKSPACE_TERMINAL_CELL_WIDTH_PX: u32 = 10;
#[cfg(target_os = "windows")]
const FALLBACK_WORKSPACE_TERMINAL_CELL_HEIGHT_PX: u32 = 23;
#[cfg(not(target_os = "windows"))]
const FALLBACK_WORKSPACE_TERMINAL_CELL_HEIGHT_PX: u32 = 22;
const WORKSPACE_INPUT_PROJECTION_DEBOUNCE_MS: u64 = 12;
const WORKSPACE_SCROLL_VIEWPORT_PROJECTION_DEBOUNCE_MS: u64 = 4;
const WORKSPACE_SCROLL_THUMB_DRAG_PROJECTION_DEBOUNCE_MS: u64 = 8;
const WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS: u64 = 1_000;
const WORKSPACE_TERMINAL_CURSOR_BLINK_INTERVAL_MS: u64 = 600;
const EDGE_DRAG_THRESHOLD_PX: f32 = 4.0;

#[cfg(test)]
type WorkspaceTerminalPresenterFactory =
    dyn Fn(AppRuntimeProfile) -> Result<(Box<dyn TerminalPresenter>, TerminalRenderMode)>;
#[cfg(test)]
#[allow(non_camel_case_types)]
type TEST_WORKSPACE_TERMINAL_HOST_FACTORY = WorkspaceTerminalPresenterFactory;
#[cfg(test)]
#[allow(non_camel_case_types)]
type TEST_WORKSPACE_PROCESS_MEMORY_TRIMMER = dyn FnMut() -> bool;
#[cfg(test)]
#[allow(non_camel_case_types)]
type TEST_WORKSPACE_BACKEND_MEMORY_PURGER = dyn FnMut() -> bool;

thread_local! {
    static WORKSPACE_TERMINAL_RENDERER_HOST: RefCell<Option<TerminalRendererHost>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_NATIVE_TERMINAL_SURFACE: RefCell<Option<NativeTerminalSurface>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_TERMINAL_POINTER_STATE: RefCell<Option<workspace_terminal::WorkspaceTerminalPointerState>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_NATIVE_CURSOR_BLINK_STATE: RefCell<Option<WorkspaceNativeCursorBlinkState>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_NATIVE_CURSOR_BLINK_TIMER: RefCell<Option<Rc<Timer>>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE: RefCell<Option<(i32, i32)>> = const {
        RefCell::new(None)
    };
    static WORKSPACE_RUNTIME_PROFILE: RefCell<Option<AppRuntimeProfile>> = const {
        RefCell::new(None)
    };
    #[cfg(test)]
    static WORKSPACE_TEST_TERMINAL_PRESENTER_FACTORY: RefCell<Option<Box<TEST_WORKSPACE_TERMINAL_HOST_FACTORY>>> = const {
        RefCell::new(None)
    };
    #[cfg(test)]
    static WORKSPACE_TEST_PROCESS_MEMORY_TRIMMER_HOOK: RefCell<Option<Box<TEST_WORKSPACE_PROCESS_MEMORY_TRIMMER>>> = const {
        RefCell::new(None)
    };
    #[cfg(test)]
    static WORKSPACE_TEST_BACKEND_MEMORY_PURGER_HOOK: RefCell<Option<Box<TEST_WORKSPACE_BACKEND_MEMORY_PURGER>>> = const {
        RefCell::new(None)
    };
}

#[derive(Clone)]
struct PendingHostKeyApproval {
    profile: ConnectionProfile,
    public_key_openssh: String,
}

#[derive(Debug)]
enum SshModalBackgroundMessage {
    TestConnectionFinished {
        request_id: u64,
        profile: ConnectionProfile,
        result: Result<()>,
    },
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

const VAULT_AUTO_SYNC_DEBOUNCE_MS: u64 = 1_200;
const VAULT_PERIODIC_SYNC_INTERVAL_MS: u64 = 120_000;

#[derive(Clone)]
struct VaultSessionState {
    root_dir: PathBuf,
    provider_factory: Arc<dyn VaultProviderFactory>,
    bootstrap_template: Option<BootstrapBundle>,
    local_state: Option<LocalVaultBootstrapState>,
    unlocked_vault_key: Option<[u8; 32]>,
    decrypted_snapshot: Option<VaultSnapshot>,
}

#[derive(Clone)]
struct VaultProjectionUpdate {
    console_tree: AssetTree,
    snippet_tree: AssetTree,
    keychain_catalog: KeychainCatalog,
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
    terminal_defaults: TerminalRuntimeDefaults,
}

pub trait VaultProviderFactory: Send + Sync {
    fn build_provider(&self, remote: &BootstrapRemoteConfig) -> Result<Arc<dyn VaultProvider>>;

    fn build_provider_for_vault(
        &self,
        remote: &BootstrapRemoteConfig,
        _vault_root: &Path,
    ) -> Result<Arc<dyn VaultProvider>> {
        self.build_provider(remote)
    }
}

#[derive(Clone)]
pub struct VaultRuntimeOptions {
    pub root_dir: Option<PathBuf>,
    pub provider_factory: Arc<dyn VaultProviderFactory>,
    pub bootstrap_template: Option<BootstrapBundle>,
    pub git_repo_metadata_source: Arc<dyn GitRepositoryMetadataSource>,
}

impl Default for VaultRuntimeOptions {
    fn default() -> Self {
        Self {
            root_dir: None,
            provider_factory: Arc::new(DefaultVaultProviderFactory),
            bootstrap_template: None,
            git_repo_metadata_source: Arc::new(ReqwestGitRepositoryMetadataSource),
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
            ProviderKind::GitRepo => {
                Err(anyhow!("git repo provider requires a vault runtime root"))
            }
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

    fn build_provider_for_vault(
        &self,
        remote: &BootstrapRemoteConfig,
        vault_root: &Path,
    ) -> Result<Arc<dyn VaultProvider>> {
        match remote.provider {
            ProviderKind::GitRepo => {
                let BootstrapRemoteLocator::GitRepo { host_kind, .. } = &remote.locator else {
                    return Err(anyhow!(
                        "bootstrap remote `{}` is missing a Git repo locator",
                        remote.remote_id
                    ));
                };
                validate_first_release_git_host(*host_kind)?;
                Ok(Arc::new(GitRepoProvider::new(
                    GitRepoProviderConfig::from_bootstrap_remote(
                        remote,
                        git_remote_cache_dir(vault_root, remote.remote_id.as_str()),
                    )?,
                )?))
            }
            _ => self.build_provider(remote),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedPrivateKey {
    pub path: PathBuf,
    pub content: String,
}

#[doc(hidden)]
pub use crate::app::url_open::UrlOpenHandlerGuard;
#[doc(hidden)]
pub use workspace_terminal::WorkspaceTerminalLinkAffordance;

#[doc(hidden)]
pub fn workspace_terminal_openable_url_at_surface_for_test(
    surface: &TerminalSurfaceState,
    row: u32,
    col: u32,
) -> Option<String> {
    workspace_terminal::openable_url_at_surface(surface, row, col)
}

#[doc(hidden)]
pub fn workspace_terminal_link_affordance_for_test(
    surface: &TerminalSurfaceState,
    row: u32,
    col: u32,
    ctrl: bool,
) -> WorkspaceTerminalLinkAffordance {
    workspace_terminal::link_affordance_at_surface(surface, row, col, ctrl)
}

#[doc(hidden)]
pub fn install_url_open_handler_for_test<F>(handler: F) -> UrlOpenHandlerGuard
where
    F: Fn(&str) -> Result<()> + Send + Sync + 'static,
{
    crate::app::url_open::install_open_url_handler_for_test(handler)
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
        let terminal_defaults = self.terminal_defaults.clone();
        Box::pin(async move {
            let session = SshSessionRuntime::connect_with_credential_store(
                profile,
                session_id,
                attempt_id,
                event_tx,
                credential_store,
                terminal_defaults,
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
        let terminal_defaults = self.terminal_defaults.clone();
        Box::pin(async move {
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let runtime = SshSessionRuntime::connect_with_credential_store(
                profile,
                Uuid::new_v4(),
                Uuid::new_v4(),
                event_tx,
                credential_store,
                terminal_defaults,
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

fn update_native_terminal_modifier_state_from_modifiers(
    modifiers: &mut NativeTerminalModifierState,
    modifiers_state: slint::winit_030::winit::keyboard::ModifiersState,
) {
    modifiers.ctrl = modifiers_state.control_key();
    modifiers.shift = modifiers_state.shift_key();
    modifiers.alt = modifiers_state.alt_key();
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

fn workspace_sftp_path_edit_shortcut(
    key: &slint::winit_030::winit::keyboard::Key,
    modifiers: NativeTerminalModifierState,
) -> bool {
    if !modifiers.ctrl || modifiers.shift || modifiers.alt {
        return false;
    }

    match key {
        slint::winit_030::winit::keyboard::Key::Character(text) => {
            workspace_sftp_path_edit_shortcut_matches(text.as_str(), true, false, false)
        }
        _ => false,
    }
}

fn workspace_sftp_select_all_shortcut(
    key: &slint::winit_030::winit::keyboard::Key,
    modifiers: NativeTerminalModifierState,
) -> bool {
    if modifiers.shift || modifiers.alt {
        return false;
    }

    match key {
        slint::winit_030::winit::keyboard::Key::Character(text) => {
            workspace_sftp_select_all_shortcut_matches(
                text.as_str(),
                modifiers.ctrl,
                modifiers.shift,
                modifiers.alt,
            )
        }
        _ => false,
    }
}

fn workspace_sftp_clear_selection_shortcut(
    key: &slint::winit_030::winit::keyboard::Key,
    modifiers: NativeTerminalModifierState,
) -> bool {
    match key {
        slint::winit_030::winit::keyboard::Key::Named(
            slint::winit_030::winit::keyboard::NamedKey::Escape,
        ) => workspace_sftp_clear_selection_shortcut_matches(
            "escape",
            modifiers.ctrl,
            modifiers.shift,
            modifiers.alt,
        ),
        _ => false,
    }
}

pub fn workspace_sftp_path_edit_shortcut_matches(
    key: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    ctrl && !shift && !alt && (key.eq_ignore_ascii_case("l") || key == "\u{c}")
}

pub fn workspace_sftp_select_all_shortcut_matches(
    key: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    !shift && !alt && ((ctrl && key.eq_ignore_ascii_case("a")) || key == "\u{1}")
}

pub fn workspace_sftp_clear_selection_shortcut_matches(
    key: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    !ctrl && !shift && !alt && key.eq_ignore_ascii_case("escape")
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
    #[cfg(target_os = "windows")]
    {
        (1600, 960)
    }

    #[cfg(not(target_os = "windows"))]
    {
        (
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        )
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
            workspace_terminal::forward_workspace_session_paste(state, bridge, session_id, &script);
        }
        SnippetActivation::Run => {
            let runnable_script = if script.ends_with('\n') {
                script
            } else {
                format!("{script}\n")
            };
            workspace_terminal::forward_active_workspace_text_input(
                state,
                bridge,
                &runnable_script,
            );
        }
    }

    workspace_terminal::refresh_active_workspace_projection(window, state, bridge, follow_tracker);
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
        "folder" if active_sidebar_destination == SidebarDestination::Keychain => {
            ContextTargetKind::KeychainFolder
        }
        "folder" => ContextTargetKind::Folder,
        "identity" if active_sidebar_destination == SidebarDestination::Keychain => {
            ContextTargetKind::KeychainIdentity
        }
        "ssh-key" if active_sidebar_destination == SidebarDestination::Keychain => {
            ContextTargetKind::KeychainSshKey
        }
        "snippet-package" => ContextTargetKind::SnippetPackage,
        "snippet" => ContextTargetKind::Snippet,
        "blank" if active_sidebar_destination == SidebarDestination::Snippets => {
            ContextTargetKind::SnippetsBlankArea
        }
        "blank" | "keychain-blank"
            if active_sidebar_destination == SidebarDestination::Keychain =>
        {
            ContextTargetKind::KeychainBlankArea
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
    terminal_defaults: TerminalRuntimeDefaults,
) -> Rc<ShellSessionBridge> {
    Rc::new(ShellSessionBridge {
        terminal_defaults: terminal_defaults.clone(),
        manager: SessionManager::new_with_launcher(
            runtime_handle,
            Arc::new(LiveSessionRuntimeLauncher {
                credential_store,
                terminal_defaults,
            }),
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

fn workspace_tab_proxy_profile_is_safe(proxy: &ConnectionProxyProfile) -> bool {
    match proxy {
        ConnectionProxyProfile::None | ConnectionProxyProfile::SshAsset { .. } => true,
        ConnectionProxyProfile::Socks5 { password, .. }
        | ConnectionProxyProfile::Http { password, .. } => password.is_none(),
    }
}

fn workspace_tab_resolved_proxy_hop_is_safe(hop: &ResolvedProxyHop) -> bool {
    match hop {
        ResolvedProxyHop::Socks5 { password, .. } | ResolvedProxyHop::Http { password, .. } => {
            password.is_none()
        }
        ResolvedProxyHop::Ssh(upstream) => workspace_tab_connection_profile_is_safe(upstream),
    }
}

fn workspace_tab_connection_profile_is_safe(profile: &ConnectionProfile) -> bool {
    profile.password.is_none()
        && profile.private_key_content.is_none()
        && profile.passphrase.is_none()
        && workspace_tab_proxy_profile_is_safe(&profile.proxy)
        && profile
            .resolved_proxy_hops
            .iter()
            .all(workspace_tab_resolved_proxy_hop_is_safe)
}

pub(super) fn cloneable_workspace_tab_connection_profile(
    profile: &ConnectionProfile,
) -> Option<ConnectionProfile> {
    workspace_tab_connection_profile_is_safe(profile).then(|| profile.clone())
}

pub(super) fn runtime_cloneable_profile_for_saved_asset(
    state: &ShellViewModel,
    asset_id: &str,
) -> anyhow::Result<ConnectionProfile> {
    let profile = runtime_profile_for_saved_asset(state, asset_id)?;
    cloneable_workspace_tab_connection_profile(&profile).ok_or_else(|| {
        anyhow!(
            "saved ssh asset `{asset_id}` resolved unsafe inline secrets for workspace tab state"
        )
    })
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

fn active_edit_keychain_identity_id(state: &ShellViewModel) -> Option<String> {
    let Some(AssetModalState::NewKeychainIdentity {
        editing_item_id: Some(item_id),
        ..
    }) = &state.asset_modal_state
    else {
        return None;
    };

    Some(item_id.clone())
}

fn active_edit_keychain_ssh_key_id(state: &ShellViewModel) -> Option<String> {
    let Some(AssetModalState::NewKeychainSshKey {
        editing_item_id: Some(item_id),
        ..
    }) = &state.asset_modal_state
    else {
        return None;
    };

    Some(item_id.clone())
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

fn hydrate_edit_keychain_identity_secret_from_store(
    state: &mut ShellViewModel,
    credential_store: &dyn CredentialStore,
) {
    let Some(identity_id) = active_edit_keychain_identity_id(state) else {
        return;
    };

    let credential_ref = keychain_identity_credential_ref(identity_id.as_str());
    let password = load_keychain_identity_secret_bundle(credential_store, credential_ref.as_str())
        .ok()
        .and_then(|bundle| bundle.password);
    state.hydrate_edit_keychain_identity_secret(password);
}

fn hydrate_edit_keychain_ssh_key_secret_from_store(
    state: &mut ShellViewModel,
    credential_store: &dyn CredentialStore,
) {
    let Some(key_id) = active_edit_keychain_ssh_key_id(state) else {
        return;
    };

    let credential_ref = keychain_key_credential_ref(key_id.as_str());
    let private_key = load_keychain_key_secret_bundle(credential_store, credential_ref.as_str())
        .ok()
        .and_then(|bundle| bundle.private_key_content);
    state.hydrate_edit_keychain_ssh_key_secret(private_key);
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
    let Some(text) = workspace_terminal::system_clipboard_text() else {
        return;
    };
    apply_keychain_private_key_material(state, text);
}

fn paste_public_key_into_keychain_modal(state: &mut ShellViewModel) {
    let Some(text) = workspace_terminal::system_clipboard_text() else {
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
    workspace_terminal::set_system_clipboard_text(draft.public_key.trim())
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

fn persist_keychain_identity_secret(
    credential_store: &dyn CredentialStore,
    identity_id: &str,
    draft: &KeychainIdentityDraft,
) -> Result<()> {
    let credential_ref = keychain_identity_credential_ref(identity_id);
    let bundle = StoredKeychainIdentitySecretBundle {
        password: (draft.auth_kind.trim() == "password" && !draft.password.trim().is_empty())
            .then(|| draft.password.clone()),
    };
    persist_keychain_identity_secret_bundle(credential_store, credential_ref.as_str(), &bundle)
}

fn merge_workspace_tab_into_tabs(state: &mut ShellViewModel, mut next_tab: WorkspaceTab) {
    let mut tabs = state.workspace_tabs().to_vec();
    if let Some(existing) = tabs.iter_mut().find(|tab| tab.tab_id == next_tab.tab_id) {
        *existing = next_tab;
    } else if !next_tab.session_id.is_empty() {
        if let Some(existing) = tabs
            .iter_mut()
            .find(|tab| tab.session_id == next_tab.session_id)
        {
            next_tab.tab_id = existing.tab_id.clone();
            *existing = next_tab;
        } else {
            tabs.push(next_tab);
        }
    } else if !next_tab.asset_id.is_empty() {
        if let Some(existing) = tabs.iter_mut().find(|tab| {
            tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
                && tab.session_id.is_empty()
                && tab.asset_id == next_tab.asset_id
        }) {
            next_tab.tab_id = existing.tab_id.clone();
            *existing = next_tab;
        } else {
            tabs.push(next_tab);
        }
    } else {
        tabs.push(next_tab);
    }

    state.set_workspace_tabs(tabs);
}

fn merge_session_handle_into_tabs(
    state: &mut ShellViewModel,
    handle: &SessionHandle,
    connection_profile: Option<ConnectionProfile>,
) {
    let mut tabs = state.workspace_tabs().to_vec();
    if let Some(existing) = tabs
        .iter_mut()
        .find(|tab| tab.session_id == handle.session_id.to_string())
    {
        let tab_id = existing.tab_id.clone();
        let mut next = WorkspaceTab::from_session_with_tab_id(handle, tab_id);
        next.connection_profile = connection_profile;
        *existing = next;
    } else if let Some(existing) = tabs.iter_mut().find(|tab| {
        tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
            && tab.session_id.is_empty()
            && !tab.asset_id.is_empty()
            && tab.asset_id == handle.asset_id
    }) {
        let tab_id = existing.tab_id.clone();
        let mut next = WorkspaceTab::from_session_with_tab_id(handle, tab_id);
        next.connection_profile = connection_profile;
        *existing = next;
    } else {
        let mut next = WorkspaceTab::from_session(handle);
        next.connection_profile = connection_profile;
        tabs.push(next);
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

#[derive(Debug, Default)]
struct DeferredWorkspaceProjectionRefreshGate {
    scheduled: bool,
}

impl DeferredWorkspaceProjectionRefreshGate {
    fn mark_scheduled(&mut self) -> bool {
        if self.scheduled {
            false
        } else {
            self.scheduled = true;
            true
        }
    }

    fn clear(&mut self) {
        self.scheduled = false;
    }
}

#[derive(Debug, Default)]
struct DeferredWorkspaceScrollThumbDrag {
    scheduled: bool,
    latest_ratio: Option<f32>,
}

impl DeferredWorkspaceScrollThumbDrag {
    fn queue_ratio(&mut self, ratio: f32) -> bool {
        self.latest_ratio = Some(ratio.clamp(0.0, 1.0));
        if self.scheduled {
            false
        } else {
            self.scheduled = true;
            true
        }
    }

    fn take_latest_ratio(&mut self) -> Option<f32> {
        self.scheduled = false;
        self.latest_ratio.take()
    }
}

#[derive(Debug, Default)]
struct WorkspaceFollowTracker;

fn active_workspace_session_uuid(state: &ShellViewModel) -> Option<Uuid> {
    state
        .active_workspace_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorkspacePasteRequestOutcome {
    Ignored,
    Prompted,
    Sent,
}

struct WorkspaceScrollInput {
    delta_lines: i32,
    row: i32,
    col: i32,
    shift: bool,
    ctrl: bool,
    alt: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LauncherSshOpenIntent {
    RecentConnection,
    SavedSshPicker,
}

impl LauncherSshOpenIntent {
    fn session_mode(self) -> OpenSessionMode {
        OpenSessionMode::ForceNewTab
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingLauncherActivation {
    intent: LauncherSshOpenIntent,
    asset_id: String,
    requested_at: Instant,
}

fn open_session_with_profile(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    profile: ConnectionProfile,
    mode: OpenSessionMode,
) -> anyhow::Result<()> {
    let workspace_profile = cloneable_workspace_tab_connection_profile(&profile);
    let handle = bridge.manager.open_session(profile, mode)?;
    let is_new_connecting_attempt = handle.state == SessionState::Connecting;
    merge_session_handle_into_tabs(state, &handle, workspace_profile);
    if !is_new_connecting_attempt {
        let _ = workspace_terminal::sync_workspace_projection_from_manager(state, &bridge.manager);
    }
    Ok(())
}

fn show_failed_session_tab(
    state: &mut ShellViewModel,
    profile: &ConnectionProfile,
    message: impl Into<String>,
) {
    let tab = WorkspaceTab::terminal_error(
        format!("workspace-terminal-error:{}", Uuid::new_v4()),
        profile
            .asset_id
            .clone()
            .unwrap_or_else(|| format!("session-error:{}", Uuid::new_v4())),
        profile.name.clone(),
        profile.user.clone(),
        profile.host.clone(),
        profile.port,
        message.into(),
        cloneable_workspace_tab_connection_profile(profile),
    );
    merge_workspace_tab_into_tabs(state, tab);
}

fn show_failed_saved_asset_tab(
    state: &mut ShellViewModel,
    asset_id: &str,
    message: impl Into<String>,
) {
    let message = message.into();
    let connection_profile = runtime_cloneable_profile_for_saved_asset(state, asset_id).ok();
    let (title, username, host, port) = match (
        state.console_asset_tree().node(asset_id),
        state.console_asset_tree().ssh_connection_spec(asset_id),
    ) {
        (Some(node), Some(spec)) => {
            let port = if spec.port.trim().is_empty() {
                22
            } else {
                spec.port.trim().parse::<u16>().unwrap_or(22)
            };
            (
                node.title.clone(),
                spec.user.trim().to_string(),
                spec.host.trim().to_string(),
                port,
            )
        }
        (Some(node), None) => (node.title.clone(), String::new(), String::new(), 0),
        _ => ("SSH Connection".into(), String::new(), String::new(), 0),
    };

    let tab = WorkspaceTab::terminal_error(
        format!("workspace-terminal-error:{}", Uuid::new_v4()),
        asset_id.to_string(),
        title,
        username,
        host,
        port,
        message,
        connection_profile,
    );
    merge_workspace_tab_into_tabs(state, tab);
}

fn prompt_unknown_host_key(
    state: &mut ShellViewModel,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    profile: ConnectionProfile,
    error: &UnknownHostKeyError,
) {
    pending_host_key_approval
        .borrow_mut()
        .replace(PendingHostKeyApproval {
            profile,
            public_key_openssh: error.public_key_openssh.clone(),
        });
    state.open_ssh_host_key_prompt(
        format!("{}:{}", error.host, error.port),
        error.fingerprint.clone(),
    );
}

fn queue_modal_test_connection(
    manager: &SessionManager,
    runtime_handle: tokio::runtime::Handle,
    result_tx: &std::sync::mpsc::Sender<SshModalBackgroundMessage>,
    next_request_id: &Rc<Cell<u64>>,
    active_request_id: &Rc<RefCell<Option<u64>>>,
    profile: ConnectionProfile,
) {
    let request_id = next_request_id.get().saturating_add(1);
    next_request_id.set(request_id);
    active_request_id.borrow_mut().replace(request_id);

    let manager = manager.clone();
    let result_tx = result_tx.clone();
    runtime_handle.spawn(async move {
        let result = manager.probe_connection_async(profile.clone()).await;
        let _ = result_tx.send(SshModalBackgroundMessage::TestConnectionFinished {
            request_id,
            profile,
            result,
        });
    });
}

fn attempt_open_session_with_profile(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    _pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    profile: ConnectionProfile,
    mode: OpenSessionMode,
) -> anyhow::Result<()> {
    open_session_with_profile(state, bridge, profile.clone(), mode).inspect_err(|err| {
        show_failed_session_tab(state, &profile, err.to_string());
    })
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

fn register_launcher_activation(
    tracker: &Rc<RefCell<Option<PendingLauncherActivation>>>,
    intent: LauncherSshOpenIntent,
    asset_id: &str,
    now: Instant,
) -> bool {
    const DUPLICATE_GESTURE_WINDOW: Duration = Duration::from_millis(350);

    let should_skip = tracker
        .borrow()
        .as_ref()
        .map(|pending| {
            pending.intent == intent
                && pending.asset_id == asset_id
                && now.duration_since(pending.requested_at) <= DUPLICATE_GESTURE_WINDOW
        })
        .unwrap_or(false);

    if !should_skip {
        tracker.borrow_mut().replace(PendingLauncherActivation {
            intent,
            asset_id: asset_id.to_string(),
            requested_at: now,
        });
    }

    should_skip
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
                        } else if state.ssh_host_key_prompt_state.is_none() {
                            state.record_recent_saved_ssh_asset(asset_id);
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
    intent: LauncherSshOpenIntent,
) {
    match runtime_profile_for_saved_asset(state, asset_id) {
        Ok(profile) => {
            if let Some(session_bridge) = session_bridge {
                if let Err(err) = attempt_open_session_with_profile(
                    state,
                    session_bridge,
                    pending_host_key_approval,
                    profile,
                    intent.session_mode(),
                ) {
                    tracing::error!(
                        target: "app.quick_launch",
                        asset_id,
                        error = %err,
                        "failed to open saved ssh asset from quick launch"
                    );
                } else if state.ssh_host_key_prompt_state.is_none() {
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

fn activate_saved_ssh_picker_asset(
    window: &AppWindow,
    state: &mut ShellViewModel,
    session_bridge: Option<&ShellSessionBridge>,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    launcher_activation_tracker: &Rc<RefCell<Option<PendingLauncherActivation>>>,
    workspace_follow_tracker: &Rc<RefCell<WorkspaceFollowTracker>>,
    asset_id: &str,
) {
    match state.console_asset_tree().kind(asset_id) {
        Some(ConsoleAssetKind::Folder) => {
            state.toggle_saved_ssh_picker_expanded(asset_id);
            sync_saved_ssh_picker_state(window, state);
            return;
        }
        Some(ConsoleAssetKind::SshConnection) => {}
        _ => return,
    }

    if register_launcher_activation(
        launcher_activation_tracker,
        LauncherSshOpenIntent::SavedSshPicker,
        asset_id,
        Instant::now(),
    ) {
        return;
    }

    state.close_saved_ssh_picker();
    if state
        .active_workspace_tab()
        .is_some_and(|tab| tab.is_launcher())
    {
        state.close_workspace_launcher_tab();
    }
    open_saved_ssh_asset_from_quick_launch(
        state,
        session_bridge,
        pending_host_key_approval,
        asset_id,
        LauncherSshOpenIntent::SavedSshPicker,
    );
    sync_welcome_quick_launch_state(window, state);
    sync_workspace_tabs_with_manager(
        window,
        state,
        &mut workspace_follow_tracker.borrow_mut(),
        session_bridge.map(|bridge| &bridge.manager),
    );
    sync_workspace_terminal_runtime_defaults(window, session_bridge);
    schedule_workspace_terminal_runtime_defaults_sync(
        window,
        session_bridge.map(|bridge| bridge.terminal_defaults.clone()),
    );
    sync_saved_ssh_picker_state(window, state);
    windowing::sync_ssh_host_key_modal_state(window, state);
}

fn resolve_pending_host_key(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    ssh_modal_result_tx: &std::sync::mpsc::Sender<SshModalBackgroundMessage>,
    next_ssh_modal_test_request_id: &Rc<Cell<u64>>,
    active_ssh_modal_test_request_id: &Rc<RefCell<Option<u64>>>,
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
            state.finish_ssh_modal_action_error(err.to_string());
            return;
        }

        let Some(bridge) = bridge else {
            let message = "SSH session bridge is unavailable.".to_string();
            state.finish_ssh_modal_action_error(message);
            return;
        };

        queue_modal_test_connection(
            &bridge.manager,
            bridge.manager.runtime_handle(),
            ssh_modal_result_tx,
            next_ssh_modal_test_request_id,
            active_ssh_modal_test_request_id,
            pending.profile,
        );
        return;
    }

    let message = format!("Rejected unknown SSH host key for `{}`:{}.", host, port);
    state.finish_ssh_modal_action_error(message);
}

fn drain_ssh_modal_background_messages(
    state: &mut ShellViewModel,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    active_request_id: &Rc<RefCell<Option<u64>>>,
    result_rx: &std::sync::mpsc::Receiver<SshModalBackgroundMessage>,
) -> bool {
    let mut changed = false;

    loop {
        let Ok(message) = result_rx.try_recv() else {
            break;
        };

        match message {
            SshModalBackgroundMessage::TestConnectionFinished {
                request_id,
                profile,
                result,
            } => {
                if active_request_id.borrow().as_ref() != Some(&request_id) {
                    continue;
                }
                active_request_id.borrow_mut().take();
                if !matches!(
                    state.ssh_modal_action_state(),
                    crate::shell::view_model::SshModalActionState::Busy(
                        crate::shell::view_model::SshModalAction::TestConnection
                    )
                ) {
                    continue;
                }

                match result {
                    Ok(()) => state.finish_ssh_modal_action_success("Connection test succeeded."),
                    Err(err) => {
                        if let Some(unknown) = err.downcast_ref::<UnknownHostKeyError>() {
                            prompt_unknown_host_key(
                                state,
                                pending_host_key_approval,
                                profile,
                                unknown,
                            );
                        } else {
                            state.finish_ssh_modal_action_error(err.to_string());
                        }
                    }
                }
                changed = true;
            }
        }
    }

    changed
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

fn close_workspace_tab_by_id(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    tab_id: &str,
) -> bool {
    let Some(tab) = state
        .workspace_tabs()
        .iter()
        .find(|tab| tab.tab_id == tab_id)
        .cloned()
    else {
        return false;
    };

    if tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal {
        let has_dependent_sftp_tabs =
            state.has_workspace_sftp_tabs_for_terminal_session(tab.session_id.as_str());
        if has_dependent_sftp_tabs {
            if let Some(bridge) = bridge
                && let Ok(session_uuid) = Uuid::parse_str(tab.session_id.as_str())
            {
                let _ = bridge.manager.disconnect_session(session_uuid);
            }
            state.hide_workspace_terminal_session(tab.session_id.as_str());
            let _ = state.mark_workspace_sftp_sessions_disconnected(tab.session_id.as_str());
            return state.close_workspace_tab(tab_id);
        }

        return close_session_by_id(state, bridge, tab.session_id.as_str());
    }

    let linked_hidden_session_id = state
        .file_browser_sessions
        .get(tab.file_browser_session_id.as_str())
        .and_then(|browser_session| browser_session.linked_terminal_session_id.clone())
        .filter(|session_id| state.workspace_terminal_session_hidden(session_id));

    state
        .file_browser_sessions
        .remove(tab.file_browser_session_id.as_str());
    let closed = state.close_workspace_tab(tab_id);
    if !closed {
        return false;
    }

    if let Some(session_id) = linked_hidden_session_id
        && !state.has_workspace_sftp_tabs_for_terminal_session(session_id.as_str())
    {
        if let Some(bridge) = bridge
            && let Ok(session_uuid) = Uuid::parse_str(session_id.as_str())
        {
            let _ = bridge.manager.close_session(session_uuid);
        }
        state.unhide_workspace_terminal_session(session_id.as_str());
    }

    true
}

fn close_workspace_tabs_from_plan(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    plan: WorkspaceTabClosePlan,
) -> bool {
    let mut closed_any = false;
    for tab_id in &plan.victim_tab_ids {
        closed_any |= close_workspace_tab_by_id(state, bridge, tab_id.as_str());
    }
    if !closed_any {
        return false;
    }

    if let Some(next_active_tab_id) = plan.next_active_tab_id.as_deref() {
        let _ = state.activate_workspace_tab(next_active_tab_id);
    }

    true
}

fn sync_workspace_tab_items(window: &AppWindow, state: &ShellViewModel) {
    let tabs = state
        .workspace_tabs()
        .iter()
        .map(|tab| WorkspaceTabItem {
            tab_id: tab.tab_id.clone().into(),
            title: tab.display_name.clone().into(),
            subtitle: tab.subtitle.clone().into(),
            state: tab.state.clone().into(),
            enhanced_session_state: tab.enhanced_session_state.clone().into(),
            active: tab.active,
        })
        .collect::<Vec<_>>();

    window.set_workspace_tab_items(ModelRc::new(VecModel::from(tabs)));
}

fn sync_workspace_tab_context_menu_state(window: &AppWindow, state: &ShellViewModel) {
    let menu = state.workspace_tab_context_menu_state();
    window.set_workspace_tab_context_menu_open(menu.open);
    window.set_workspace_tab_context_menu_anchor_x(menu.anchor_x);
    window.set_workspace_tab_context_menu_anchor_y(menu.anchor_y);
    window.set_workspace_tab_context_menu_reconnect_enabled(menu.reconnect_enabled);
    window.set_workspace_tab_context_menu_clone_connection_enabled(menu.clone_connection_enabled);
    window.set_workspace_tab_context_menu_close_enabled(menu.close_enabled);
    window.set_workspace_tab_context_menu_copy_name_enabled(menu.copy_name_enabled);
    window.set_workspace_tab_context_menu_copy_host_enabled(menu.copy_host_enabled);
    window.set_workspace_tab_context_menu_close_others_enabled(menu.close_others_enabled);
    window.set_workspace_tab_context_menu_close_all_enabled(menu.close_all_enabled);
    window.set_workspace_tab_context_menu_close_right_enabled(menu.close_right_enabled);
    window.set_workspace_tab_context_menu_close_left_enabled(menu.close_left_enabled);
}

fn show_workspace_tab_tooltip(
    window: &AppWindow,
    state: &ShellViewModel,
    tab_id: &str,
    anchor_x: f32,
    anchor_y: f32,
) {
    let Some(tab) = state.workspace_tab_by_id(tab_id) else {
        clear_workspace_tab_tooltip(window);
        return;
    };

    window.set_workspace_tab_tooltip_text(tab.summary_tooltip_text().into());
    window.set_workspace_tab_tooltip_anchor_x(anchor_x);
    window.set_workspace_tab_tooltip_anchor_y(anchor_y);
    window.set_workspace_tab_tooltip_visible(true);
}

fn clear_workspace_tab_tooltip(window: &AppWindow) {
    window.set_workspace_tab_tooltip_visible(false);
    window.set_workspace_tab_tooltip_text("".into());
    window.set_workspace_tab_tooltip_anchor_x(0.0);
    window.set_workspace_tab_tooltip_anchor_y(0.0);
}

fn reconnect_workspace_tab_by_id(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    tab_id: &str,
) -> anyhow::Result<bool> {
    let Some(tab) = state.workspace_tab_by_id(tab_id).cloned() else {
        return Ok(false);
    };
    let Some(bridge) = bridge else {
        return Ok(false);
    };

    let active_before = state.active_workspace_tab_id().map(str::to_owned);
    let restore_active_after_reconnect = active_before.as_deref() != Some(tab_id);

    match tab.kind {
        crate::shell::tabs::WorkspaceTabKind::Terminal => {
            if let Ok(session_id) = Uuid::parse_str(tab.session_id.as_str()) {
                bridge.manager.retry_session(session_id)?;
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    state,
                    &bridge.manager,
                );
            } else if let Some(profile) = tab.connection_profile.clone() {
                if let Err(err) = open_session_with_profile(
                    state,
                    bridge,
                    profile.clone(),
                    OpenSessionMode::ForceNewTab,
                ) {
                    show_failed_session_tab(state, &profile, err.to_string());
                    return Err(err);
                }
            } else {
                return Ok(false);
            }
        }
        crate::shell::tabs::WorkspaceTabKind::Sftp => {
            let Some(session_id) = state
                .reconnect_workspace_sftp_tab(tab_id)
                .and_then(|session_id| Uuid::parse_str(session_id.as_str()).ok())
            else {
                return Ok(false);
            };
            bridge.manager.retry_session(session_id)?;
            state.hide_workspace_terminal_session(session_id.to_string().as_str());
        }
        crate::shell::tabs::WorkspaceTabKind::Launcher => return Ok(false),
    }

    if restore_active_after_reconnect && let Some(active_tab_id) = active_before.as_deref() {
        let _ = state.activate_workspace_tab(active_tab_id);
    }

    Ok(true)
}

fn resolve_workspace_tab_clone_profile(
    state: &ShellViewModel,
    tab_id: &str,
) -> anyhow::Result<ConnectionProfile> {
    if let Some(profile) = state.workspace_tab_connection_profile(tab_id) {
        return Ok(profile);
    }

    let asset_id = state
        .workspace_tab_saved_ssh_asset_id(tab_id)
        .with_context(|| format!("workspace tab `{tab_id}` has no saved SSH asset metadata"))?;
    runtime_cloneable_profile_for_saved_asset(state, asset_id.as_str())
}

fn clone_workspace_tab_by_id(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    tab_id: &str,
) -> anyhow::Result<bool> {
    let Some(bridge) = bridge else {
        state.set_context_menu_feedback("SSH session bridge is unavailable.");
        return Ok(false);
    };

    let profile = match resolve_workspace_tab_clone_profile(state, tab_id) {
        Ok(profile) => profile,
        Err(err) => {
            tracing::error!(
                target: "app.ssh",
                tab_id,
                error = %err,
                "failed to resolve workspace tab clone profile"
            );
            state.set_context_menu_feedback(format!("Failed to clone connection: {err}"));
            return Ok(false);
        }
    };

    if let Err(err) =
        open_session_with_profile(state, bridge, profile.clone(), OpenSessionMode::ForceNewTab)
    {
        show_failed_session_tab(state, &profile, err.to_string());
        state.set_context_menu_feedback(format!("Failed to clone connection: {err}"));
        return Err(err);
    }

    Ok(true)
}

fn sync_welcome_quick_launch_state(window: &AppWindow, state: &ShellViewModel) {
    let project_card = |item: crate::shell::quick_launch::QuickLaunchCardItem| QuickLaunchCardRow {
        asset_id: item.asset_id.clone().into(),
        title: item.title.into(),
        subtitle: item.subtitle.into(),
        badge: item.badge.into(),
        meta: item.meta.into(),
        time_label: item.time_label.into(),
        state_label: item.state_label.into(),
        icon_kind: item.icon_kind.into(),
        accent_kind: item.accent_kind.into(),
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
    window.set_open_saved_ssh_modal_can_open_selection(state.saved_ssh_picker_can_open_selection());
    window.set_open_saved_ssh_modal_query(state.saved_ssh_picker_query().into());
    window.set_open_saved_ssh_modal_items(ModelRc::new(VecModel::from(items)));
    sync_workspace_native_terminal_surface_geometry(window);
}

fn slint_color_from_rgba(rgba: u32) -> Color {
    let a = ((rgba >> 24) & 0xff) as u8;
    let r = ((rgba >> 16) & 0xff) as u8;
    let g = ((rgba >> 8) & 0xff) as u8;
    let b = (rgba & 0xff) as u8;
    Color::from_argb_u8(a, r, g, b)
}

fn terminal_rgb_to_rgba((red, green, blue): (u8, u8, u8)) -> u32 {
    0xff00_0000 | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

fn terminal_rgba_to_rgba((red, green, blue, alpha): (u8, u8, u8, f32)) -> u32 {
    let alpha = (alpha.clamp(0.0, 1.0) * 255.0).round() as u32;
    (alpha << 24) | (u32::from(red) << 16) | (u32::from(green) << 8) | u32::from(blue)
}

fn sync_shell_runtime_palette(window: &AppWindow, preset: ProjectedThemePreset) {
    window.set_shell_app_background(slint_color_from_rgba(0xff00_0000 | preset.app_background));
    window.set_shell_titlebar_background(slint_color_from_rgba(
        0xff00_0000 | preset.titlebar_background,
    ));
    window.set_shell_tabbar_background(slint_color_from_rgba(
        0xff00_0000 | preset.tabbar_background,
    ));
    window.set_shell_sidebar_background(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_background,
    ));
    window.set_shell_sidebar_panel_background(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_panel_background,
    ));
    window.set_shell_right_panel_background(slint_color_from_rgba(
        0xff00_0000 | preset.right_panel_background,
    ));
    window.set_shell_separator(slint_color_from_rgba(0xff00_0000 | preset.separator));
    window.set_shell_border(slint_color_from_rgba(0xff00_0000 | preset.border));
    window.set_shell_hairline(slint_color_from_rgba(0xff00_0000 | preset.hairline));
    window.set_shell_text_primary(slint_color_from_rgba(0xff00_0000 | preset.text_primary));
    window.set_shell_text_secondary(slint_color_from_rgba(0xff00_0000 | preset.text_secondary));
    window.set_shell_text_muted(slint_color_from_rgba(0xff00_0000 | preset.text_muted));
    window.set_shell_text_inactive(slint_color_from_rgba(0xff00_0000 | preset.text_inactive));
    window.set_shell_accent(slint_color_from_rgba(0xff00_0000 | preset.accent));
    window.set_shell_link_accent(slint_color_from_rgba(0xff00_0000 | preset.link_accent));
    window.set_shell_focus_ring(slint_color_from_rgba(0xff00_0000 | preset.focus_ring));
    window.set_shell_tab_active(slint_color_from_rgba(0xff00_0000 | preset.tab_active));
    window.set_shell_tab_inactive(slint_color_from_rgba(0xff00_0000 | preset.tab_inactive));
    window.set_shell_tab_hover(slint_color_from_rgba(0xff00_0000 | preset.tab_hover));
    window.set_shell_tab_active_indicator(slint_color_from_rgba(
        0xff00_0000 | preset.tab_active_indicator,
    ));
    window.set_shell_sidebar_item_hover(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_item_hover,
    ));
    window.set_shell_sidebar_item_selected(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_item_selected,
    ));
    window.set_shell_sidebar_item_selected_border(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_item_selected_border,
    ));
    window.set_shell_sidebar_item_focus_border(slint_color_from_rgba(
        0xff00_0000 | preset.sidebar_item_focus_border,
    ));
    window.set_shell_panel_scrollbar_track(slint_color_from_rgba(
        0xff00_0000 | preset.panel_scrollbar_track,
    ));
    window.set_shell_panel_scrollbar_thumb(slint_color_from_rgba(
        0xff00_0000 | preset.panel_scrollbar_thumb,
    ));
    window.set_shell_panel_scrollbar_thumb_active(slint_color_from_rgba(
        0xff00_0000 | preset.panel_scrollbar_thumb_active,
    ));
}

fn sync_workspace_terminal_shell_chrome(window: &AppWindow, preset: ProjectedThemePreset) {
    window.set_workspace_session_selection_surface(slint_color_from_rgba(terminal_rgba_to_rgba(
        preset.terminal.selection_bg,
    )));
    window.set_workspace_session_scrollbar_track(slint_color_from_rgba(terminal_rgb_to_rgba(
        preset.terminal.scrollbar_track,
    )));
    window.set_workspace_session_scrollbar_thumb(slint_color_from_rgba(terminal_rgb_to_rgba(
        preset.terminal.scrollbar_thumb,
    )));
    window.set_workspace_session_scrollbar_thumb_active(slint_color_from_rgba(
        terminal_rgb_to_rgba(preset.terminal.scrollbar_thumb_active),
    ));
    window.set_workspace_session_frame_surface(slint_color_from_rgba(
        0xff00_0000 | preset.terminal.frame_bg,
    ));
    window.set_workspace_session_frame_border(slint_color_from_rgba(terminal_rgb_to_rgba(
        preset.terminal.split,
    )));
}

fn clear_workspace_terminal_semantic_projection(window: &AppWindow) {
    sync_vec_model(
        window.get_workspace_session_command_blocks(),
        Vec::<TerminalCommandBlockRow>::new(),
        |model| window.set_workspace_session_command_blocks(model),
    );
    sync_vec_model(
        window.get_workspace_session_overview_markers(),
        Vec::<TerminalOverviewMarkerRow>::new(),
        |model| window.set_workspace_session_overview_markers(model),
    );
}

fn sync_workspace_terminal_semantic_projection(
    window: &AppWindow,
    command_blocks: Vec<TerminalCommandBlockRow>,
    overview_markers: Vec<TerminalOverviewMarkerRow>,
) {
    sync_vec_model(
        window.get_workspace_session_command_blocks(),
        command_blocks,
        |model| window.set_workspace_session_command_blocks(model),
    );
    sync_vec_model(
        window.get_workspace_session_overview_markers(),
        overview_markers,
        |model| window.set_workspace_session_overview_markers(model),
    );
}

fn project_terminal_command_blocks(blocks: &[CommandBlock]) -> Vec<TerminalCommandBlockRow> {
    blocks
        .iter()
        .map(|block| TerminalCommandBlockRow {
            start_row: i32::try_from(block.start_row).unwrap_or(i32::MAX),
            end_row: i32::try_from(block.end_row).unwrap_or(i32::MAX),
            status: match block.status {
                CommandBlockStatus::Running => "running",
                CommandBlockStatus::Success => "success",
                CommandBlockStatus::Failure => "failure",
            }
            .into(),
            label: block.command_text.clone().into(),
        })
        .collect()
}

fn project_terminal_overview_markers(markers: &[OverviewMarker]) -> Vec<TerminalOverviewMarkerRow> {
    markers
        .iter()
        .map(|marker| TerminalOverviewMarkerRow {
            row: i32::try_from(marker.row).unwrap_or(i32::MAX),
            kind: match marker.kind {
                OverviewMarkerKind::CommandRunning => "command_running",
                OverviewMarkerKind::CommandSuccess => "command_success",
                OverviewMarkerKind::CommandFailure => "command_failure",
            }
            .into(),
        })
        .collect()
}

fn analyze_workspace_terminal_semantic_projection(
    surface: &TerminalSurfaceState,
    settings: TerminalSemanticSettings,
) -> (Vec<TerminalCommandBlockRow>, Vec<TerminalOverviewMarkerRow>) {
    let frame_model = TerminalModelFrame::from_surface(surface, None);
    let annotations = analyze_semantic_annotations_with_settings(&frame_model, settings);
    (
        project_terminal_command_blocks(&annotations.command_blocks),
        project_terminal_overview_markers(&annotations.overview_markers),
    )
}

fn terminal_selection_overlay_rgba(theme_mode: ThemeMode, theme_variant: ThemeVariant) -> u32 {
    if theme_variant == ThemeVariant::PremiumDefault {
        selection_overlay_rgba(theme_mode)
    } else {
        selection_overlay_rgba_for(theme_mode, theme_variant)
    }
}

fn workspace_session_uses_host_selection_overlay(window: &AppWindow) -> bool {
    window.get_workspace_session_render_mode().as_str() == TerminalRenderMode::Bitmap.as_str()
}

fn active_workspace_terminal_selection(
    state: &ShellViewModel,
    surface: &TerminalSurfaceState,
) -> Option<TerminalAtlasSelection> {
    workspace_terminal::active_workspace_terminal_selection(state)
        .and_then(|selection| selection.project_to_viewport(surface))
        .map(|selection| {
            TerminalAtlasSelection::new(
                selection.start_row,
                selection.start_col,
                selection.end_row,
                selection.end_col,
            )
        })
}

enum WorkspaceTerminalLinkMouseDecision {
    Forward,
    LocalOnly,
    Open(String),
}

fn workspace_terminal_link_mouse_decision(
    state: &ShellViewModel,
    kind: TerminalMouseEventKind,
    button: TerminalMouseButton,
    row: u32,
    col: u32,
    ctrl: bool,
    candidate: &mut Option<WorkspaceTerminalLinkClickCandidate>,
) -> WorkspaceTerminalLinkMouseDecision {
    let Some(surface) = state.active_workspace_terminal_surface() else {
        candidate.take();
        return WorkspaceTerminalLinkMouseDecision::Forward;
    };

    if surface.alternate_screen_active || surface.mouse_grabbed || surface.application_cursor_keys {
        candidate.take();
        return WorkspaceTerminalLinkMouseDecision::Forward;
    }

    match (kind, button) {
        (TerminalMouseEventKind::Move, _) => {
            candidate.take();
            WorkspaceTerminalLinkMouseDecision::LocalOnly
        }
        (TerminalMouseEventKind::Down, TerminalMouseButton::Left) => {
            *candidate = ctrl
                .then(|| {
                    workspace_terminal::openable_url_at_active_workspace_surface(state, row, col)
                })
                .flatten()
                .map(|url| WorkspaceTerminalLinkClickCandidate { url });
            WorkspaceTerminalLinkMouseDecision::LocalOnly
        }
        (TerminalMouseEventKind::Up, TerminalMouseButton::Left) => {
            let current_url = ctrl
                .then(|| {
                    workspace_terminal::openable_url_at_active_workspace_surface(state, row, col)
                })
                .flatten();
            let decision = if candidate
                .as_ref()
                .zip(current_url.as_ref())
                .is_some_and(|(candidate, current_url)| candidate.url == *current_url)
            {
                WorkspaceTerminalLinkMouseDecision::Open(
                    current_url.expect("current URL should exist when the candidate matches"),
                )
            } else {
                WorkspaceTerminalLinkMouseDecision::LocalOnly
            };
            candidate.take();
            decision
        }
        _ => {
            candidate.take();
            WorkspaceTerminalLinkMouseDecision::Forward
        }
    }
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
    if profile.prefers_native_terminal_renderer() {
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

#[cfg(feature = "terminal-native-renderer")]
fn build_native_terminal_presenter() -> Result<Box<dyn TerminalPresenter>> {
    Ok(Box::new(WindowsNativePresenter::new()?))
}

#[cfg(not(feature = "terminal-native-renderer"))]
fn build_native_terminal_presenter() -> Result<Box<dyn TerminalPresenter>> {
    Err(anyhow!(
        "native terminal renderer is unavailable in this build"
    ))
}

fn resolve_workspace_terminal_presenter(
    profile: AppRuntimeProfile,
) -> Result<(Box<dyn TerminalPresenter>, TerminalRenderMode)> {
    #[cfg(test)]
    if let Some(result) = WORKSPACE_TEST_TERMINAL_PRESENTER_FACTORY
        .with(|cell| cell.borrow().as_ref().map(|factory| factory(profile)))
    {
        return result;
    }

    build_workspace_terminal_presenter(profile)
}

fn ensure_workspace_terminal_presenter(
    window: &AppWindow,
    profile: AppRuntimeProfile,
    scale_factor: f32,
) -> Result<()> {
    let mut initialized_render_mode = None;
    WORKSPACE_TERMINAL_RENDERER_HOST.with(|cell| -> Result<()> {
        let needs_init = cell.borrow().is_none();
        if needs_init {
            let (presenter, active_render_mode) =
                match resolve_workspace_terminal_presenter(profile) {
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
                                    .expect("bundled bitmap presenter should initialize after fallback"),
                            ) as Box<dyn TerminalPresenter>,
                            TerminalRenderMode::Bitmap,
                        )
                    }
                };
            let mut host = TerminalRendererHost::new(presenter, active_render_mode);
            host.set_raster_scale(scale_factor);
            *cell.borrow_mut() = Some(host);
            initialized_render_mode = Some(active_render_mode);
        }
        Ok(())
    })?;

    if let Some(active_render_mode) = initialized_render_mode {
        tracing::info!(
            target: "app.terminal",
            requested_render_mode = profile.terminal_render_mode_label(),
            active_render_mode = active_render_mode.as_str(),
            "initialized workspace terminal presenter"
        );
        match active_render_mode {
            TerminalRenderMode::Native => {
                WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
                    let mut surface = surface.borrow_mut();
                    if let Some(native_surface) = surface.as_ref() {
                        native_surface
                            .configure_present_path(window, profile.native_present_path());
                    } else {
                        let native_surface = NativeTerminalSurface::attach(window);
                        native_surface
                            .configure_present_path(window, profile.native_present_path());
                        *surface = Some(native_surface);
                    }
                });
            }
            TerminalRenderMode::Bitmap => {
                clear_workspace_retained_native_terminal_surface(window);
                WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
                    *surface.borrow_mut() = None;
                });
            }
        }
        window.set_workspace_session_native_frame_token(0);
    }

    Ok(())
}

#[cfg(test)]
fn with_workspace_terminal_presenter_factory_for_test<T>(
    factory: Box<WorkspaceTerminalPresenterFactory>,
    body: impl FnOnce() -> T,
) -> T {
    WORKSPACE_TEST_TERMINAL_PRESENTER_FACTORY.with(|cell| {
        let previous = cell.replace(Some(factory));
        let result = body();
        cell.replace(previous);
        result
    })
}

fn workspace_terminal_default_cell_size(scale_factor: f32) -> (u32, u32) {
    WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
        host.borrow_mut()
            .as_mut()
            .map(|host| {
                host.set_raster_scale(scale_factor);
                host.default_cell_size()
            })
            .unwrap_or((
                FALLBACK_WORKSPACE_TERMINAL_CELL_WIDTH_PX,
                FALLBACK_WORKSPACE_TERMINAL_CELL_HEIGHT_PX,
            ))
    })
}

fn present_surface_update_with_bitmap_fallback(
    host: &mut TerminalRendererHost,
    surface: &TerminalSurfaceState,
    options: TerminalRendererHostOptions,
    scale_factor: f32,
) -> Result<PresentedTerminalFrame> {
    match host.present_surface_update(surface, options.clone()) {
        Ok(frame) => Ok(frame),
        Err(first_err) => {
            let requested_render_mode = host.render_mode();
            tracing::error!(
                target: "app.terminal",
                session_id = surface.session_id.to_string(),
                requested_render_mode = requested_render_mode.as_str(),
                error = %first_err,
                "failed to render workspace terminal surface; retrying with bitmap presenter"
            );

            let mut fallback_host = TerminalRendererHost::new(
                Box::new(BitmapAtlasPresenter::new()?),
                TerminalRenderMode::Bitmap,
            );
            fallback_host.set_raster_scale(scale_factor);
            *host = fallback_host;

            match host.present_surface_update(surface, options) {
                Ok(frame) => {
                    tracing::warn!(
                        target: "app.terminal",
                        session_id = surface.session_id.to_string(),
                        requested_render_mode = requested_render_mode.as_str(),
                        fallback_render_mode = TerminalRenderMode::Bitmap.as_str(),
                        "workspace terminal presenter fell back to bitmap rendering after a render failure"
                    );
                    Ok(frame)
                }
                Err(retry_err) => {
                    tracing::error!(
                        target: "app.terminal",
                        session_id = surface.session_id.to_string(),
                        requested_render_mode = requested_render_mode.as_str(),
                        fallback_render_mode = TerminalRenderMode::Bitmap.as_str(),
                        error = %retry_err,
                        "bitmap presenter retry failed after render failure"
                    );
                    Err(retry_err)
                }
            }
        }
    }
}

fn window_scale_factor(window: &AppWindow) -> f32 {
    window.window().scale_factor().max(1.0)
}

fn workspace_blocks_native_terminal_surface(window: &AppWindow) -> bool {
    window.get_sync_modal_open()
        || window.get_settings_modal_open()
        || window.get_asset_modal_open()
        || window.get_asset_rename_modal_open()
        || window.get_asset_delete_confirm_modal_open()
        || window.get_ssh_host_key_modal_open()
        || window.get_workspace_paste_warning_modal_open()
        || window.get_open_saved_ssh_modal_open()
        || window.get_sftp_conflict_modal_open()
        || window.get_sftp_remote_file_modal_open()
}

fn workspace_native_terminal_rect(window: &AppWindow) -> NativeTerminalSurfaceRect {
    if workspace_blocks_native_terminal_surface(window) {
        return NativeTerminalSurfaceRect::default();
    }

    let scale_factor = window_scale_factor(window);
    NativeTerminalSurfaceRect {
        x: (window.get_layout_workspace_session_native_surface_x() * scale_factor).round() as i32,
        // The exported workspace terminal y is relative to the body host; child HWND geometry
        // needs client-area coordinates, so fold the custom titlebar height back in here.
        y: ((window.get_layout_titlebar_height()
            + window.get_layout_workspace_session_native_surface_y())
            * scale_factor)
            .round() as i32,
        width: (window.get_layout_workspace_session_native_surface_width() * scale_factor).round()
            as i32,
        height: (window.get_layout_workspace_session_native_surface_height() * scale_factor).round()
            as i32,
    }
}

fn should_forward_workspace_terminal_resize(window: &AppWindow, rows: i32, cols: i32) -> bool {
    let _ = window;
    rows > 0 && cols > 0
}

fn record_workspace_terminal_viewport_defaults(
    window: &AppWindow,
    bridge: &ShellSessionBridge,
    rows: i32,
    cols: i32,
) {
    let rect = workspace_native_terminal_rect(window);
    bridge.terminal_defaults.set_viewport_size(
        rows.max(1) as usize,
        cols.max(1) as usize,
        rect.width.max(0) as u32,
        rect.height.max(0) as u32,
    );
}

fn workspace_native_terminal_resize_target(
    rect: NativeTerminalSurfaceRect,
    cell_width_px: u32,
    cell_height_px: u32,
) -> Option<(i32, i32)> {
    if rect.width <= 0 || rect.height <= 0 || cell_width_px == 0 || cell_height_px == 0 {
        return None;
    }

    let rows = rect.height / i32::try_from(cell_height_px).ok()?;
    let cols = rect.width / i32::try_from(cell_width_px).ok()?;
    (rows > 0 && cols > 0).then_some((rows, cols))
}

fn sync_workspace_native_terminal_resize_backstop(
    window: &AppWindow,
    rect: NativeTerminalSurfaceRect,
    frame: &NativeTerminalFrame,
) {
    let Some((desired_rows, desired_cols)) =
        workspace_native_terminal_resize_target(rect, frame.cell_width_px, frame.cell_height_px)
    else {
        WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE.with(|pending| {
            pending.borrow_mut().take();
        });
        return;
    };

    let current_rows = i32::try_from(frame.presentable_frame.grid_rows).unwrap_or(i32::MAX);
    let current_cols = i32::try_from(frame.presentable_frame.grid_cols).unwrap_or(i32::MAX);
    if desired_rows == current_rows && desired_cols == current_cols {
        WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE.with(|pending| {
            pending.borrow_mut().take();
        });
        return;
    }

    let next_request = (desired_rows, desired_cols);
    let should_request = WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE.with(|pending| {
        let mut pending = pending.borrow_mut();
        if pending.as_ref() == Some(&next_request) {
            false
        } else {
            *pending = Some(next_request);
            true
        }
    });
    if !should_request {
        return;
    }

    tracing::trace!(
        target: "app.terminal",
        current_rows,
        current_cols,
        desired_rows,
        desired_cols,
        rect_width = rect.width,
        rect_height = rect.height,
        cell_width_px = frame.cell_width_px,
        cell_height_px = frame.cell_height_px,
        "native terminal viewport exceeded the projected grid; requesting host resize backstop"
    );
    let handle = window.as_weak();
    if let Err(err) = slint::invoke_from_event_loop(move || {
        if let Some(window) = handle.upgrade() {
            window.invoke_workspace_session_resize_requested(desired_rows, desired_cols);
        }
    }) {
        WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE.with(|pending| {
            pending.borrow_mut().take();
        });
        tracing::warn!(
            target: "app.terminal",
            error = %err,
            desired_rows,
            desired_cols,
            "failed to schedule native terminal resize backstop on the Slint event loop"
        );
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

fn clear_workspace_retained_native_terminal_surface(window: &AppWindow) {
    window.set_workspace_session_native_frame_token(0);
    WORKSPACE_PENDING_NATIVE_TERMINAL_RESIZE.with(|pending| {
        pending.borrow_mut().take();
    });
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        if let Some(surface) = surface.borrow().as_ref() {
            surface.clear_frame();
        }
    });
    clear_workspace_native_cursor_blink_state();
}

fn present_workspace_native_terminal_frame(window: &AppWindow, frame: NativeTerminalFrame) {
    window.set_workspace_session_native_frame_token(
        i32::try_from(frame.frame_token).unwrap_or(i32::MAX),
    );
    let presentable_frame = frame.presentable_frame.clone();
    tracing::trace!(
        target: "app.terminal",
        frame_token = frame.frame_token,
        shaped_rows = presentable_frame.shaped_row_count,
        monochrome_draws = presentable_frame.monochrome_glyph_draws.len(),
        color_draws = presentable_frame.color_glyph_draws.len(),
        glyph_cache_entries = presentable_frame.renderer_stats.glyph_cache_entries,
        selection_overlay_rects = presentable_frame.selection_overlay.rect_count,
        semantic_overlay_count = presentable_frame.semantic_overlays.len(),
        semantic_input_overlay_count = presentable_frame.semantic_input_overlays.len(),
        semantic_span_count = presentable_frame.semantic_spans.len(),
        command_block_count = presentable_frame.command_blocks.len(),
        overview_marker_count = presentable_frame.overview_markers.len(),
        underline_overlay_runs = presentable_frame.underline_overlay.run_count,
        ime_preview_active = presentable_frame.ime_preview_overlay.active,
        cursor_visible = presentable_frame.cursor.visible,
        cursor_overlay_visible = presentable_frame.cursor_overlay.visible,
        "presenting retained native terminal frame state"
    );
    let rect = workspace_native_terminal_rect(window);
    tracing::trace!(
        target: "app.terminal",
        frame_token = frame.frame_token,
        grid_rows = frame.presentable_frame.grid_rows,
        grid_cols = frame.presentable_frame.grid_cols,
        cell_width_px = frame.cell_width_px,
        cell_height_px = frame.cell_height_px,
        rect_width = rect.width,
        rect_height = rect.height,
        "projecting native terminal frame into host-owned surface"
    );
    sync_workspace_native_terminal_resize_backstop(window, rect, &frame);
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        if let Some(surface) = surface.borrow().as_ref() {
            surface.update_terminal_rect(rect);
            surface.present(frame);
        }
    });
}

fn clear_workspace_session_cursor_overlay(window: &AppWindow) {
    window.set_workspace_session_cursor_row(0);
    window.set_workspace_session_cursor_col(0);
    window.set_workspace_session_cursor_visible(false);
    window.set_workspace_session_cursor_blinking(false);
    window.set_workspace_session_cursor_shape("block".into());
}

fn clear_workspace_native_terminal_frame(window: &AppWindow) {
    window.set_workspace_session_surface_image(Image::default());
    clear_workspace_retained_native_terminal_surface(window);
}

fn clear_workspace_terminal_transient_caches() {
    WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
        let mut host = host.borrow_mut();
        if let Some(host) = host.as_mut() {
            host.clear_transient_caches();
        }
    });
}

fn release_workspace_terminal_renderer_resources() {
    WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
        host.borrow_mut().take();
    });
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        surface.borrow_mut().take();
    });
}

fn workspace_terminal_renderer_resources_retained() -> bool {
    let host_retained = WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| host.borrow().is_some());
    let native_surface_retained =
        WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| surface.borrow().is_some());
    host_retained || native_surface_retained
}

#[cfg(test)]
fn trim_workspace_process_memory() -> bool {
    WORKSPACE_TEST_PROCESS_MEMORY_TRIMMER_HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        if let Some(trim) = hook.as_mut() {
            trim()
        } else {
            crate::app::memory::trim_process_working_set()
        }
    })
}

#[cfg(not(test))]
fn trim_workspace_process_memory() -> bool {
    crate::app::memory::trim_process_working_set()
}

#[cfg(test)]
fn purge_workspace_backend_memory(window: Option<&AppWindow>) -> bool {
    WORKSPACE_TEST_BACKEND_MEMORY_PURGER_HOOK.with(|hook| {
        let mut hook = hook.borrow_mut();
        if let Some(purger) = hook.as_mut() {
            purger()
        } else {
            window.is_some_and(|window| {
                use i_slint_backend_winit::WinitWindowMemoryPurge;

                window.window().purge_winit_renderer_memory().is_ok()
            })
        }
    })
}

#[cfg(not(test))]
fn purge_workspace_backend_memory(window: Option<&AppWindow>) -> bool {
    window.is_some_and(|window| {
        use i_slint_backend_winit::WinitWindowMemoryPurge;

        window.window().purge_winit_renderer_memory().is_ok()
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkspaceTerminalActiveSurfaceFingerprint {
    session_id: Uuid,
    seqno: usize,
    viewport_offset_lines: u32,
}

impl WorkspaceTerminalActiveSurfaceFingerprint {
    fn from_surface(surface: &TerminalSurfaceState) -> Self {
        Self {
            session_id: surface.session_id,
            seqno: surface.seqno,
            viewport_offset_lines: surface.viewport_offset_lines,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkspaceNativeCursorBlinkFingerprint {
    session_id: Uuid,
    surface_seqno: usize,
}

impl WorkspaceNativeCursorBlinkFingerprint {
    fn from_surface(surface: &TerminalSurfaceState) -> Self {
        Self {
            session_id: surface.session_id,
            surface_seqno: surface.seqno,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct WorkspaceNativeCursorBlinkState {
    fingerprint: WorkspaceNativeCursorBlinkFingerprint,
    visible: bool,
}

fn clear_workspace_native_cursor_blink_state() {
    WORKSPACE_NATIVE_CURSOR_BLINK_STATE.with(|blink_state| {
        blink_state.borrow_mut().take();
    });
}

fn workspace_native_cursor_overlay_visible(
    surface: &TerminalSurfaceState,
    blink_state: &mut Option<WorkspaceNativeCursorBlinkState>,
) -> bool {
    if !surface.cursor.visible {
        blink_state.take();
        return false;
    }
    if !surface.cursor.blinking {
        blink_state.take();
        return true;
    }

    let fingerprint = WorkspaceNativeCursorBlinkFingerprint::from_surface(surface);
    match blink_state {
        Some(state) if state.fingerprint == fingerprint => state.visible,
        _ => {
            *blink_state = Some(WorkspaceNativeCursorBlinkState {
                fingerprint,
                visible: true,
            });
            true
        }
    }
}

fn workspace_native_cursor_overlay_visible_for_surface(surface: &TerminalSurfaceState) -> bool {
    WORKSPACE_NATIVE_CURSOR_BLINK_STATE.with(|blink_state| {
        let mut blink_state = blink_state.borrow_mut();
        workspace_native_cursor_overlay_visible(surface, &mut blink_state)
    })
}

fn advance_workspace_native_cursor_blink_state(
    active_surface: Option<&TerminalSurfaceState>,
    blink_state: &mut Option<WorkspaceNativeCursorBlinkState>,
) -> bool {
    let Some(surface) = active_surface else {
        blink_state.take();
        return false;
    };
    if !surface.cursor.visible || !surface.cursor.blinking {
        blink_state.take();
        return false;
    }

    let fingerprint = WorkspaceNativeCursorBlinkFingerprint::from_surface(surface);
    match blink_state {
        Some(state) if state.fingerprint == fingerprint => {
            state.visible = !state.visible;
            true
        }
        _ => {
            *blink_state = Some(WorkspaceNativeCursorBlinkState {
                fingerprint,
                visible: true,
            });
            false
        }
    }
}

fn advance_workspace_native_cursor_blink_phase(
    active_surface: Option<&TerminalSurfaceState>,
) -> bool {
    WORKSPACE_NATIVE_CURSOR_BLINK_STATE.with(|blink_state| {
        let mut blink_state = blink_state.borrow_mut();
        advance_workspace_native_cursor_blink_state(active_surface, &mut blink_state)
    })
}

fn update_workspace_terminal_active_idle_cache_shrink(
    active_surface: Option<&TerminalSurfaceState>,
    enabled: bool,
    now: Instant,
    active_surface_fingerprint: &mut Option<WorkspaceTerminalActiveSurfaceFingerprint>,
    active_surface_since: &mut Option<Instant>,
    active_idle_cache_shrunk: &mut bool,
) {
    if !enabled {
        active_surface_fingerprint.take();
        active_surface_since.take();
        *active_idle_cache_shrunk = false;
        return;
    }

    let Some(active_surface) = active_surface else {
        active_surface_fingerprint.take();
        active_surface_since.take();
        *active_idle_cache_shrunk = false;
        return;
    };

    let next_fingerprint = WorkspaceTerminalActiveSurfaceFingerprint::from_surface(active_surface);
    if active_surface_fingerprint.as_ref() != Some(&next_fingerprint) {
        *active_surface_fingerprint = Some(next_fingerprint);
        *active_surface_since = Some(now);
        *active_idle_cache_shrunk = false;
        return;
    }

    let Some(active_surface_since_at) = *active_surface_since else {
        *active_surface_since = Some(now);
        *active_idle_cache_shrunk = false;
        return;
    };
    if *active_idle_cache_shrunk
        || now.duration_since(active_surface_since_at)
            < Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS)
    {
        return;
    }

    clear_workspace_terminal_transient_caches();
    *active_idle_cache_shrunk = true;
}

fn rearm_workspace_terminal_no_surface_idle_shrink(
    now: Instant,
    no_surface_since: &mut Option<Instant>,
    idle_cache_shrunk: &mut bool,
) {
    *no_surface_since = Some(now);
    *idle_cache_shrunk = false;
}

fn update_workspace_terminal_idle_cache_shrink(
    window: Option<&AppWindow>,
    has_active_surface: bool,
    surface_disappeared: bool,
    now: Instant,
    no_surface_since: &mut Option<Instant>,
    idle_cache_shrunk: &mut bool,
) {
    if has_active_surface {
        no_surface_since.take();
        *idle_cache_shrunk = false;
        return;
    }

    let retained_renderer_resources = workspace_terminal_renderer_resources_retained();
    if surface_disappeared || (no_surface_since.is_none() && retained_renderer_resources) {
        clear_workspace_terminal_transient_caches();
        rearm_workspace_terminal_no_surface_idle_shrink(now, no_surface_since, idle_cache_shrunk);
        return;
    }

    let Some(no_surface_since_at) = *no_surface_since else {
        return;
    };
    if *idle_cache_shrunk
        || now.duration_since(no_surface_since_at)
            < Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS)
    {
        return;
    }

    clear_workspace_terminal_transient_caches();
    release_workspace_terminal_renderer_resources();
    let _ = purge_workspace_backend_memory(window);
    let _ = trim_workspace_process_memory();
    *idle_cache_shrunk = true;
}

#[cfg(test)]
fn sync_workspace_session_state(
    window: &AppWindow,
    state: &mut ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    sync_workspace_session_state_with_manager(window, state, follow_tracker, None);
}

const SSH_STATUS_PREVIEW_ENV: &str = "MICA_TERM_SSH_STATUS_PREVIEW";

#[derive(Debug, Clone)]
struct WorkspaceConnectionProgressView {
    headline: ConnectionHeadlineState,
    visual_state: ConnectionVisualState,
    current_hop: String,
    current_detail: String,
    page_mode: String,
    task_title: String,
    task_detail: String,
    prompt_host: String,
    prompt_fingerprint: String,
    warning_host: String,
    warning_expected: String,
    warning_current: String,
    hops: Vec<ConnectionProgressHopRow>,
    steps: Vec<ConnectionProgressStepRow>,
    main_fields: Vec<ConnectionProgressFieldRow>,
    detail_fields: Vec<ConnectionProgressFieldRow>,
    diagnostics: Vec<ConnectionProgressDiagnosticRow>,
}

fn ssh_status_preview_state() -> Option<ConnectionPreviewState> {
    std::env::var(SSH_STATUS_PREVIEW_ENV)
        .ok()
        .and_then(|value| ConnectionPreviewState::parse(&value))
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

fn connection_progress_visual_state_token(state: ConnectionVisualState) -> &'static str {
    match state {
        ConnectionVisualState::VerifyingHostKey => "verifying_host_key",
        ConnectionVisualState::Connecting => "connecting",
        ConnectionVisualState::HostKeyWarning => "host_key_warning",
        ConnectionVisualState::Failed => "failed",
        ConnectionVisualState::Connected => "connected",
    }
}

fn connection_hop_kind_token(kind: ConnectionHopKind) -> &'static str {
    match kind {
        ConnectionHopKind::Local => "local",
        ConnectionHopKind::JumpHost => "jump_host",
        ConnectionHopKind::Target => "target",
    }
}

fn connection_hop_state_token(state: ConnectionHopVisualState) -> &'static str {
    match state {
        ConnectionHopVisualState::Completed => "completed",
        ConnectionHopVisualState::Current => "current",
        ConnectionHopVisualState::Pending => "pending",
        ConnectionHopVisualState::Failed => "failed",
    }
}

fn connection_field_row(field: &ConnectionInfoField) -> ConnectionProgressFieldRow {
    ConnectionProgressFieldRow {
        label: field.label.clone().into(),
        value: field.value.clone().into(),
        copy_value: field.copy_value.clone().unwrap_or_default().into(),
        monospace: field.monospace,
    }
}

fn connection_hop_row(hop: &ConnectionHopStateItem) -> ConnectionProgressHopRow {
    ConnectionProgressHopRow {
        kind: connection_hop_kind_token(hop.kind).into(),
        state: connection_hop_state_token(hop.state).into(),
        label: hop.label.clone().into(),
        subtitle: hop.subtitle.clone().into(),
        host: hop.host.clone().into(),
        port: if hop.port == 0 {
            "".into()
        } else {
            hop.port.to_string().into()
        },
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

fn host_key_decision_task_detail() -> &'static str {
    "The authenticity of the target host cannot be established. Please verify the host key fingerprint below before continuing."
}

fn connection_progress_visual_state(
    attempt: &ConnectionAttemptState,
    current_step: Option<&ConnectionStepStateItem>,
) -> ConnectionVisualState {
    let has_changed_warning = attempt
        .diagnostics
        .iter()
        .any(|line| line.message.contains("host key changed"))
        || current_step.is_some_and(|step| step.detail.contains("host key changed"));
    if has_changed_warning {
        return ConnectionVisualState::HostKeyWarning;
    }
    if attempt.prompt.is_some()
        || current_step.is_some_and(|step| step.state == ConnectionStepState::Blocked)
    {
        return ConnectionVisualState::VerifyingHostKey;
    }
    if matches!(
        attempt.headline,
        ConnectionHeadlineState::Cancelled | ConnectionHeadlineState::Error
    ) || current_step.is_some_and(|step| {
        matches!(
            step.state,
            ConnectionStepState::Failed | ConnectionStepState::Cancelled
        )
    }) {
        return ConnectionVisualState::Failed;
    }
    if attempt.headline == ConnectionHeadlineState::Connected {
        return ConnectionVisualState::Connected;
    }
    ConnectionVisualState::Connecting
}

fn connection_progress_task_title(
    attempt: &ConnectionAttemptState,
    current_step: Option<&ConnectionStepStateItem>,
) -> String {
    match connection_progress_visual_state(attempt, current_step) {
        ConnectionVisualState::HostKeyWarning => "Host key changed".into(),
        ConnectionVisualState::VerifyingHostKey => "Verify host key".into(),
        ConnectionVisualState::Failed => "Connection failed".into(),
        ConnectionVisualState::Connected => "Connected".into(),
        ConnectionVisualState::Connecting => "Connecting".into(),
    }
}

fn connection_progress_task_detail(
    attempt: &ConnectionAttemptState,
    current_step: Option<&ConnectionStepStateItem>,
) -> String {
    if connection_progress_visual_state(attempt, current_step)
        == ConnectionVisualState::HostKeyWarning
    {
        return attempt
            .diagnostics
            .iter()
            .rev()
            .find(|line| line.message.contains("host key changed"))
            .map(|line| line.message.clone())
            .unwrap_or_else(|| {
                "The previously trusted host key no longer matches the key presented by the server."
                    .into()
            });
    }

    if attempt.prompt.is_some()
        || current_step.is_some_and(|step| {
            step.step_kind == "verify-host-key" && step.state == ConnectionStepState::Blocked
        })
    {
        return host_key_decision_task_detail().into();
    }

    current_step
        .map(|step| step.detail.clone())
        .or_else(|| attempt.diagnostics.last().map(|line| line.message.clone()))
        .unwrap_or_else(|| default_connection_progress_detail(attempt.headline).into())
}

fn connection_auth_label(profile: &ConnectionProfile) -> &'static str {
    match profile.auth_method {
        SshAuthMethod::Password => "Password",
        SshAuthMethod::PrivateKeyPath | SshAuthMethod::PrivateKeyContent => "Private key",
    }
}

fn connection_jump_host_summary(profile: &ConnectionProfile) -> String {
    let hosts = profile
        .resolved_proxy_hops
        .iter()
        .filter_map(|hop| match hop {
            ResolvedProxyHop::Ssh(upstream) => Some(format!("{}:{}", upstream.host, upstream.port)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if hosts.is_empty() {
        "Direct connection".into()
    } else if hosts.len() == 1 {
        hosts[0].clone()
    } else {
        hosts.join(" -> ")
    }
}

fn connection_profile_hops(
    profile: &ConnectionProfile,
    current_hop_label: &str,
    visual_state: ConnectionVisualState,
) -> Vec<ConnectionHopStateItem> {
    let mut hops = Vec::new();
    hops.push(ConnectionHopStateItem {
        kind: ConnectionHopKind::Local,
        label: "Local".into(),
        subtitle: "You".into(),
        host: "local".into(),
        port: 0,
        state: ConnectionHopVisualState::Completed,
    });
    let mut ssh_hops = profile
        .resolved_proxy_hops
        .iter()
        .filter_map(|hop| match hop {
            ResolvedProxyHop::Ssh(upstream) => Some(upstream.as_ref()),
            _ => None,
        })
        .collect::<Vec<_>>();
    for (index, upstream) in ssh_hops.iter_mut().enumerate() {
        hops.push(ConnectionHopStateItem {
            kind: ConnectionHopKind::JumpHost,
            label: format!("Jump Host {}", index + 1),
            subtitle: upstream.host.clone(),
            host: upstream.host.clone(),
            port: upstream.port,
            state: ConnectionHopVisualState::Pending,
        });
    }
    hops.push(ConnectionHopStateItem {
        kind: ConnectionHopKind::Target,
        label: "Target".into(),
        subtitle: profile.host.clone(),
        host: profile.host.clone(),
        port: profile.port,
        state: ConnectionHopVisualState::Pending,
    });

    let current_index = hops
        .iter()
        .position(|hop| hop.label == current_hop_label)
        .unwrap_or_else(|| hops.len().saturating_sub(1));
    let failed_state = matches!(
        visual_state,
        ConnectionVisualState::Failed | ConnectionVisualState::HostKeyWarning
    );
    for (index, hop) in hops.iter_mut().enumerate() {
        hop.state = if index < current_index {
            ConnectionHopVisualState::Completed
        } else if index == current_index {
            if failed_state {
                ConnectionHopVisualState::Failed
            } else {
                ConnectionHopVisualState::Current
            }
        } else {
            ConnectionHopVisualState::Pending
        };
    }
    if current_hop_label.is_empty() {
        if let Some(target) = hops.last_mut() {
            target.state = match visual_state {
                ConnectionVisualState::Failed | ConnectionVisualState::HostKeyWarning => {
                    ConnectionHopVisualState::Failed
                }
                _ => ConnectionHopVisualState::Current,
            };
        }
    }
    hops
}

fn parse_host_key_changed_message(message: &str) -> Option<(String, String, String)> {
    let prefix = "SSH host key changed for ";
    let remainder = message.strip_prefix(prefix)?;
    let (host, fingerprints) = remainder.split_once(" (expected ")?;
    let (expected, current) = fingerprints.split_once(", got ")?;
    let current = current.strip_suffix(')')?;
    Some((
        host.trim_matches('`').to_string(),
        expected.to_string(),
        current.to_string(),
    ))
}

fn current_hop_label_for_prompt(
    profile: Option<&ConnectionProfile>,
    prompt: Option<&crate::app::ssh::connection_progress::ConnectionHostKeyPrompt>,
    current_step: Option<&ConnectionStepStateItem>,
) -> String {
    if let Some(step) = current_step {
        if !step.hop_label.is_empty() {
            return step.hop_label.clone();
        }
    }
    let Some(prompt) = prompt else {
        return String::new();
    };
    let Some(profile) = profile else {
        return "Target".into();
    };
    for (index, hop) in profile.resolved_proxy_hops.iter().enumerate() {
        if let ResolvedProxyHop::Ssh(upstream) = hop {
            if upstream.host == prompt.host && upstream.port == prompt.port {
                return format!("Jump Host {}", index + 1);
            }
        }
    }
    "Target".into()
}

fn preview_connection_progress_view(
    preview: ConnectionPreviewState,
) -> WorkspaceConnectionProgressView {
    let fixture = preview.fixture();
    connection_progress_view_from_fixture(fixture)
}

fn connection_progress_view_from_fixture(
    fixture: ConnectionPreviewFixture,
) -> WorkspaceConnectionProgressView {
    WorkspaceConnectionProgressView {
        headline: fixture.headline,
        visual_state: fixture.visual_state,
        current_hop: fixture.current_hop_label,
        current_detail: fixture.current_detail,
        page_mode: match fixture.visual_state {
            ConnectionVisualState::VerifyingHostKey => "decision",
            ConnectionVisualState::HostKeyWarning => "warning",
            ConnectionVisualState::Failed => "troubleshooting",
            ConnectionVisualState::Connected => "connected",
            ConnectionVisualState::Connecting => "progressing",
        }
        .into(),
        task_title: fixture.task_title,
        task_detail: fixture.task_detail,
        prompt_host: fixture
            .prompt
            .as_ref()
            .map(|prompt| format!("{}:{}", prompt.host, prompt.port))
            .unwrap_or_default(),
        prompt_fingerprint: fixture
            .prompt
            .as_ref()
            .map(|prompt| prompt.fingerprint.clone())
            .unwrap_or_default(),
        warning_host: fixture.warning_host,
        warning_expected: fixture.warning_expected,
        warning_current: fixture.warning_current,
        hops: fixture.hops.iter().map(connection_hop_row).collect(),
        steps: fixture
            .progress_steps
            .iter()
            .map(|step| ConnectionProgressStepRow {
                state: connection_progress_step_state_token(step.state).into(),
                hop_label: step.hop_label.clone().into(),
                title: step.title.clone().into(),
                detail: step.detail.clone().into(),
            })
            .collect(),
        main_fields: fixture
            .main_fields
            .iter()
            .map(connection_field_row)
            .collect(),
        detail_fields: fixture
            .detail_fields
            .iter()
            .map(connection_field_row)
            .collect(),
        diagnostics: fixture
            .diagnostics
            .into_iter()
            .map(|text| ConnectionProgressDiagnosticRow { text: text.into() })
            .collect(),
    }
}

fn connection_progress_view_for_attempt(
    attempt: &ConnectionAttemptState,
    profile: Option<&ConnectionProfile>,
) -> WorkspaceConnectionProgressView {
    let current_step = active_connection_progress_step(attempt);
    let visual_state = connection_progress_visual_state(attempt, current_step);
    let task_title = connection_progress_task_title(attempt, current_step);
    let task_detail = connection_progress_task_detail(attempt, current_step);
    let current_hop = current_hop_label_for_prompt(profile, attempt.prompt.as_ref(), current_step);

    let mut warning_host = String::new();
    let mut warning_expected = String::new();
    let mut warning_current = String::new();
    if visual_state == ConnectionVisualState::HostKeyWarning {
        for message in attempt
            .diagnostics
            .iter()
            .map(|line| line.message.as_str())
            .rev()
        {
            if let Some((host, expected, current)) = parse_host_key_changed_message(message) {
                warning_host = host;
                warning_expected = expected;
                warning_current = current;
                break;
            }
        }
    }

    let mut main_fields = Vec::new();
    match visual_state {
        ConnectionVisualState::VerifyingHostKey => {
            let host = attempt
                .prompt
                .as_ref()
                .map(|prompt| prompt.host.clone())
                .or_else(|| profile.map(|profile| profile.host.clone()))
                .unwrap_or_default();
            let port = attempt
                .prompt
                .as_ref()
                .map(|prompt| prompt.port)
                .or_else(|| profile.map(|profile| profile.port))
                .unwrap_or(22);
            let fingerprint = attempt
                .prompt
                .as_ref()
                .map(|prompt| prompt.fingerprint.clone())
                .unwrap_or_default();
            main_fields.push(ConnectionProgressFieldRow {
                label: "Host".into(),
                value: host.clone().into(),
                copy_value: host.into(),
                monospace: false,
            });
            main_fields.push(ConnectionProgressFieldRow {
                label: "Port".into(),
                value: format!("{port} (SSH)").into(),
                copy_value: port.to_string().into(),
                monospace: false,
            });
            if !fingerprint.is_empty() {
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Fingerprint".into(),
                    value: fingerprint.clone().into(),
                    copy_value: fingerprint.into(),
                    monospace: true,
                });
            }
            if let Some(profile) = profile {
                let jump_summary = connection_jump_host_summary(profile);
                if jump_summary != "Direct connection" {
                    main_fields.push(ConnectionProgressFieldRow {
                        label: "Jump Host".into(),
                        value: jump_summary.clone().into(),
                        copy_value: jump_summary.into(),
                        monospace: false,
                    });
                }
            }
        }
        ConnectionVisualState::HostKeyWarning => {
            if let Some(profile) = profile {
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Host".into(),
                    value: profile.host.clone().into(),
                    copy_value: profile.host.clone().into(),
                    monospace: false,
                });
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Port".into(),
                    value: format!("{} (SSH)", profile.port).into(),
                    copy_value: profile.port.to_string().into(),
                    monospace: false,
                });
            }
            if !warning_expected.is_empty() {
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Previously trusted".into(),
                    value: warning_expected.clone().into(),
                    copy_value: warning_expected.clone().into(),
                    monospace: true,
                });
            }
            if !warning_current.is_empty() {
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Presented now".into(),
                    value: warning_current.clone().into(),
                    copy_value: warning_current.clone().into(),
                    monospace: true,
                });
            }
        }
        ConnectionVisualState::Failed => {
            if !current_hop.is_empty() {
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Failed at".into(),
                    value: current_hop.clone().into(),
                    copy_value: "".into(),
                    monospace: false,
                });
            }
            if let Some(profile) = profile {
                let (host, port) = if current_hop.starts_with("Jump Host") {
                    profile
                        .resolved_proxy_hops
                        .iter()
                        .filter_map(|hop| match hop {
                            ResolvedProxyHop::Ssh(upstream) => {
                                Some((upstream.host.clone(), upstream.port))
                            }
                            _ => None,
                        })
                        .next()
                        .unwrap_or_else(|| (profile.host.clone(), profile.port))
                } else {
                    (profile.host.clone(), profile.port)
                };
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Host".into(),
                    value: host.clone().into(),
                    copy_value: host.into(),
                    monospace: false,
                });
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Port".into(),
                    value: format!("{port} (SSH)").into(),
                    copy_value: port.to_string().into(),
                    monospace: false,
                });
            }
        }
        ConnectionVisualState::Connecting | ConnectionVisualState::Connected => {
            if let Some(profile) = profile {
                let jump_summary = connection_jump_host_summary(profile);
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Target".into(),
                    value: profile.host.clone().into(),
                    copy_value: profile.host.clone().into(),
                    monospace: false,
                });
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Path".into(),
                    value: (if jump_summary == "Direct connection" {
                        "Direct".to_string()
                    } else {
                        jump_summary.clone()
                    })
                    .into(),
                    copy_value: if jump_summary == "Direct connection" {
                        "".into()
                    } else {
                        jump_summary.into()
                    },
                    monospace: false,
                });
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Port".into(),
                    value: format!("{} (SSH)", profile.port).into(),
                    copy_value: profile.port.to_string().into(),
                    monospace: false,
                });
                main_fields.push(ConnectionProgressFieldRow {
                    label: "Auth".into(),
                    value: connection_auth_label(profile).into(),
                    copy_value: "".into(),
                    monospace: false,
                });
            }
        }
    }

    let detail_fields = profile
        .map(|profile| {
            vec![
                ConnectionProgressFieldRow {
                    label: "User".into(),
                    value: profile.user.clone().into(),
                    copy_value: "".into(),
                    monospace: false,
                },
                ConnectionProgressFieldRow {
                    label: "Authentication".into(),
                    value: connection_auth_label(profile).into(),
                    copy_value: "".into(),
                    monospace: false,
                },
                ConnectionProgressFieldRow {
                    label: "Port".into(),
                    value: profile.port.to_string().into(),
                    copy_value: "".into(),
                    monospace: false,
                },
                ConnectionProgressFieldRow {
                    label: "Strict host key checking".into(),
                    value: "On".into(),
                    copy_value: "".into(),
                    monospace: false,
                },
                ConnectionProgressFieldRow {
                    label: "Jump chain".into(),
                    value: connection_jump_host_summary(profile).into(),
                    copy_value: "".into(),
                    monospace: false,
                },
            ]
        })
        .unwrap_or_default();

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

    WorkspaceConnectionProgressView {
        headline: attempt.headline,
        visual_state,
        current_hop: current_hop.clone(),
        current_detail: task_detail.clone(),
        page_mode: match visual_state {
            ConnectionVisualState::VerifyingHostKey => "decision",
            ConnectionVisualState::HostKeyWarning => "warning",
            ConnectionVisualState::Failed => "troubleshooting",
            ConnectionVisualState::Connected => "connected",
            ConnectionVisualState::Connecting => "progressing",
        }
        .into(),
        task_title,
        task_detail,
        prompt_host: attempt
            .prompt
            .as_ref()
            .map(|prompt| format!("{}:{}", prompt.host, prompt.port))
            .unwrap_or_default(),
        prompt_fingerprint: attempt
            .prompt
            .as_ref()
            .map(|prompt| prompt.fingerprint.clone())
            .unwrap_or_default(),
        warning_host,
        warning_expected,
        warning_current,
        hops: profile
            .map(|profile| {
                connection_profile_hops(profile, current_hop.as_str(), visual_state)
                    .iter()
                    .map(connection_hop_row)
                    .collect()
            })
            .unwrap_or_default(),
        steps,
        main_fields,
        detail_fields,
        diagnostics,
    }
}

fn clear_workspace_connection_progress_state(window: &AppWindow) {
    window.set_workspace_session_connection_headline("".into());
    window.set_workspace_session_connection_visual_state("".into());
    window.set_workspace_session_connection_current_hop("".into());
    window.set_workspace_session_connection_current_detail("".into());
    window.set_workspace_session_connection_page_mode("".into());
    window.set_workspace_session_connection_task_title("".into());
    window.set_workspace_session_connection_task_detail("".into());
    window.set_workspace_session_host_key_prompt_host("".into());
    window.set_workspace_session_host_key_prompt_fingerprint("".into());
    window.set_workspace_session_host_key_warning_host("".into());
    window.set_workspace_session_host_key_warning_expected("".into());
    window.set_workspace_session_host_key_warning_current("".into());
    sync_vec_model(
        window.get_workspace_session_connection_hops(),
        Vec::<ConnectionProgressHopRow>::new(),
        |model| window.set_workspace_session_connection_hops(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_steps(),
        Vec::<ConnectionProgressStepRow>::new(),
        |model| window.set_workspace_session_connection_steps(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_main_fields(),
        Vec::<ConnectionProgressFieldRow>::new(),
        |model| window.set_workspace_session_connection_main_fields(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_detail_fields(),
        Vec::<ConnectionProgressFieldRow>::new(),
        |model| window.set_workspace_session_connection_detail_fields(model),
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
    if projected_workspace_session_host_mode(state, manager) != "connection-progress" {
        clear_workspace_connection_progress_state(window);
        return;
    }

    let view = if let Some(preview) = ssh_status_preview_state() {
        preview_connection_progress_view(preview)
    } else {
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
        let profile = manager.session_profile(session_id);
        connection_progress_view_for_attempt(&attempt, profile.as_ref())
    };

    window.set_workspace_session_connection_headline(
        connection_progress_headline_token(view.headline).into(),
    );
    window.set_workspace_session_connection_visual_state(
        connection_progress_visual_state_token(view.visual_state).into(),
    );
    window.set_workspace_session_connection_page_mode(view.page_mode.clone().into());
    window.set_workspace_session_connection_current_hop(view.current_hop.clone().into());
    window.set_workspace_session_connection_current_detail(view.current_detail.clone().into());
    window.set_workspace_session_connection_task_title(view.task_title.clone().into());
    window.set_workspace_session_connection_task_detail(view.task_detail.clone().into());
    window.set_workspace_session_host_key_prompt_host(view.prompt_host.clone().into());
    window
        .set_workspace_session_host_key_prompt_fingerprint(view.prompt_fingerprint.clone().into());
    window.set_workspace_session_host_key_warning_host(view.warning_host.clone().into());
    window.set_workspace_session_host_key_warning_expected(view.warning_expected.clone().into());
    window.set_workspace_session_host_key_warning_current(view.warning_current.clone().into());
    sync_vec_model(
        window.get_workspace_session_connection_hops(),
        view.hops,
        |model| window.set_workspace_session_connection_hops(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_steps(),
        view.steps,
        |model| window.set_workspace_session_connection_steps(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_main_fields(),
        view.main_fields,
        |model| window.set_workspace_session_connection_main_fields(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_detail_fields(),
        view.detail_fields,
        |model| window.set_workspace_session_connection_detail_fields(model),
    );
    sync_vec_model(
        window.get_workspace_session_connection_diagnostics(),
        view.diagnostics,
        |model| window.set_workspace_session_connection_diagnostics(model),
    );
}

fn projected_workspace_session_host_mode(
    state: &ShellViewModel,
    manager: Option<&SessionManager>,
) -> &'static str {
    if ssh_status_preview_state().is_some() {
        return "connection-progress";
    }

    let host_mode = state.workspace_session_host_mode();
    if host_mode != "session-error" {
        return host_mode;
    }

    let Some(manager) = manager else {
        return host_mode;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return host_mode;
    };

    if manager.connection_attempt(session_id).is_some() {
        return "connection-progress";
    }

    host_mode
}

fn sync_workspace_session_state_with_manager(
    window: &AppWindow,
    state: &mut ShellViewModel,
    _follow_tracker: &mut WorkspaceFollowTracker,
    manager: Option<&SessionManager>,
) {
    window
        .set_active_workspace_session_id(state.active_workspace_session_id().unwrap_or("").into());
    window.set_workspace_session_host_mode(
        projected_workspace_session_host_mode(state, manager).into(),
    );
    window.set_workspace_session_search_open(state.workspace_terminal_search_open());
    window.set_workspace_session_search_query(state.workspace_terminal_search_query().into());
    window.set_workspace_session_search_match_count(
        i32::try_from(state.workspace_terminal_search_match_count()).unwrap_or(i32::MAX),
    );
    window.set_workspace_session_search_focus_sequence(
        state.workspace_terminal_search_focus_sequence(),
    );
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
    sync_workspace_terminal_shell_chrome(
        window,
        if state.theme_variant == ThemeVariant::PremiumDefault {
            projected_theme_for_mode(state.theme_mode)
        } else {
            projected_theme_for(state.theme_mode, state.theme_variant)
        },
    );
    workspace_terminal::clear_invalid_active_workspace_terminal_selection(state);
    sync_workspace_terminal_surface_projection_only(window, state);
    sync_workspace_connection_progress_state(window, state, manager);

    if let Some(active_tab) = state.active_workspace_tab() {
        window.set_workspace_session_title(active_tab.title.clone().into());
        window.set_workspace_session_subtitle(active_tab.subtitle.clone().into());
        window.set_workspace_session_state(active_tab.state.clone().into());
        window.set_workspace_session_error_detail(active_tab.error_detail.clone().into());
        window.set_workspace_session_can_reconnect(active_tab.can_reconnect());
    } else {
        window.set_workspace_session_title("".into());
        window.set_workspace_session_subtitle("".into());
        window.set_workspace_session_state("".into());
        window.set_workspace_session_error_detail("".into());
        window.set_workspace_session_can_reconnect(false);
    }

    if let Some(preview) = ssh_status_preview_state() {
        let fixture = preview.fixture();
        window.set_workspace_session_title(fixture.session_title.into());
        window.set_workspace_session_subtitle("SSH preview".into());
        window.set_workspace_session_state("connecting".into());
        window.set_workspace_session_error_detail("".into());
        window.set_workspace_session_can_reconnect(false);
    }

    sftp::sync_workspace_sftp_state(window, state);
}

pub(super) fn sync_workspace_terminal_runtime_defaults(
    window: &AppWindow,
    session_bridge: Option<&ShellSessionBridge>,
) {
    sync_workspace_terminal_runtime_defaults_with_defaults(
        window,
        session_bridge.map(|bridge| &bridge.terminal_defaults),
    );
}

fn sync_workspace_terminal_runtime_defaults_with_defaults(
    window: &AppWindow,
    terminal_defaults: Option<&TerminalRuntimeDefaults>,
) {
    let Some(terminal_defaults) = terminal_defaults else {
        return;
    };
    let width = window.get_layout_workspace_session_preferred_surface_width();
    let height = window.get_layout_workspace_session_preferred_surface_height();
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let cell_width = window.get_workspace_session_cell_width();
    let cell_height = window.get_workspace_session_cell_height();
    if cell_width <= 0.0 || cell_height <= 0.0 {
        return;
    }

    let rows = (height / cell_height).floor().max(1.0) as usize;
    let cols = (width / cell_width).floor().max(1.0) as usize;
    let scale_factor = window_scale_factor(window);
    terminal_defaults.set_viewport_size(
        rows,
        cols,
        (width * scale_factor).round().max(0.0) as u32,
        (height * scale_factor).round().max(0.0) as u32,
    );
}

pub(super) fn schedule_workspace_terminal_runtime_defaults_sync(
    window: &AppWindow,
    terminal_defaults: Option<TerminalRuntimeDefaults>,
) {
    let Some(terminal_defaults) = terminal_defaults else {
        return;
    };
    let handle = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(window) = handle.upgrade() {
            sync_workspace_terminal_runtime_defaults_with_defaults(
                &window,
                Some(&terminal_defaults),
            );
        }
    });
}

fn sync_workspace_terminal_surface_projection_only(window: &AppWindow, state: &ShellViewModel) {
    let profile = WORKSPACE_RUNTIME_PROFILE
        .with(|profile| (*profile.borrow()).unwrap_or_else(AppRuntimeProfile::packaged));
    let scale_factor = window_scale_factor(window);
    window.set_workspace_session_device_scale_factor(scale_factor);
    if state.active_workspace_terminal_surface().is_some()
        && let Err(err) = ensure_workspace_terminal_presenter(window, profile, scale_factor)
    {
        tracing::error!(
            target: "app.terminal",
            error = %err,
            "failed to initialize workspace terminal presenter"
        );
    }
    let (default_cell_width_px, default_cell_height_px) =
        workspace_terminal_default_cell_size(scale_factor);
    window.set_workspace_session_cell_width(default_cell_width_px as f32 / scale_factor);
    window.set_workspace_session_cell_height(default_cell_height_px as f32 / scale_factor);
    sync_workspace_native_terminal_surface_geometry(window);
    let terminal_theme_preset = if state.theme_variant == ThemeVariant::PremiumDefault {
        projected_theme_for_mode(state.theme_mode)
    } else {
        projected_theme_for(state.theme_mode, state.theme_variant)
    };
    let search_query = if state.workspace_terminal_search_open()
        && !state.workspace_terminal_search_query().trim().is_empty()
    {
        Some(state.workspace_terminal_search_query().to_string())
    } else {
        None
    };
    workspace_terminal::sync_active_workspace_terminal_selection_projection(window, state);

    if let Some(surface) = state.active_workspace_terminal_surface() {
        window.set_workspace_session_alternate_screen_active(surface.alternate_screen_active);
        let selection = if workspace_session_uses_host_selection_overlay(window) {
            None
        } else {
            active_workspace_terminal_selection(state, surface)
        };
        let selection_overlay_rgba =
            terminal_selection_overlay_rgba(state.theme_mode, state.theme_variant);
        let mut native_frame_presented = false;
        let mut next_render_mode = None;
        let mut next_surface_seqno = None;
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            let mut host = host.borrow_mut();
            let Some(host) = host.as_mut() else {
                clear_workspace_native_terminal_frame(window);
                clear_workspace_terminal_semantic_projection(window);
                next_surface_seqno = Some(0);
                next_render_mode = Some(TerminalRenderMode::Bitmap);
                return;
            };
            host.set_raster_scale(scale_factor);
            match present_surface_update_with_bitmap_fallback(
                host,
                surface,
                TerminalRendererHostOptions {
                    selection,
                    selection_overlay_rgba,
                    theme_mode: state.theme_mode,
                    theme_variant: state.theme_variant,
                    input_highlighting_enabled: state
                        .settings_modal_terminal_input_highlighting_enabled(),
                    output_rule_highlighting_enabled: state
                        .settings_modal_terminal_output_rule_highlighting_enabled(),
                    output_rule_profile: state.settings_modal_terminal_output_rule_profile(),
                    command_decorations_enabled: state
                        .settings_modal_terminal_command_decorations_enabled(),
                    overview_markers_enabled: state
                        .settings_modal_terminal_overview_markers_enabled(),
                    search_query: search_query.clone(),
                    search_match_highlight: state.settings_modal_terminal_search_match_highlight(),
                },
                scale_factor,
            ) {
                Ok(PresentedTerminalFrame::Bitmap(frame)) => {
                    window.set_workspace_session_rows(
                        i32::try_from(frame.grid_rows).unwrap_or(i32::MAX),
                    );
                    window.set_workspace_session_cols(
                        i32::try_from(frame.grid_cols).unwrap_or(i32::MAX),
                    );
                    window.set_workspace_session_cell_width(
                        frame.cell_width_px as f32 / scale_factor,
                    );
                    window.set_workspace_session_cell_height(
                        frame.cell_height_px as f32 / scale_factor,
                    );
                    clear_workspace_retained_native_terminal_surface(window);
                    window.set_workspace_session_surface_image(frame.image);
                    let (command_blocks, overview_markers) =
                        analyze_workspace_terminal_semantic_projection(
                            surface,
                            TerminalSemanticSettings {
                                input_highlighting_enabled: state
                                    .settings_modal_terminal_input_highlighting_enabled(),
                                output_rule_highlighting_enabled: state
                                    .settings_modal_terminal_output_rule_highlighting_enabled(),
                                output_rule_profile: state
                                    .settings_modal_terminal_output_rule_profile(),
                                command_decorations_enabled: state
                                    .settings_modal_terminal_command_decorations_enabled(),
                                overview_markers_enabled: state
                                    .settings_modal_terminal_overview_markers_enabled(),
                                search_query: search_query.clone(),
                            },
                        );
                    sync_workspace_terminal_semantic_projection(
                        window,
                        command_blocks,
                        overview_markers,
                    );
                    next_surface_seqno = Some(i32::try_from(surface.seqno).unwrap_or(i32::MAX));
                    next_render_mode = Some(TerminalRenderMode::Bitmap);
                }
                Ok(PresentedTerminalFrame::Native(frame)) => {
                    let mut frame = *frame;
                    let cursor_overlay_visible =
                        workspace_native_cursor_overlay_visible_for_surface(surface);
                    frame.presentable_frame.cursor_overlay.visible = cursor_overlay_visible;
                    let presentable_frame = frame.presentable_frame.clone();
                    native_frame_presented = true;
                    let scale_factor = window_scale_factor(window);
                    window.set_workspace_session_rows(
                        i32::try_from(presentable_frame.grid_rows).unwrap_or(i32::MAX),
                    );
                    window.set_workspace_session_cols(
                        i32::try_from(presentable_frame.grid_cols).unwrap_or(i32::MAX),
                    );
                    window.set_workspace_session_cell_width(
                        frame.cell_width_px as f32 / scale_factor,
                    );
                    window.set_workspace_session_cell_height(
                        frame.cell_height_px as f32 / scale_factor,
                    );
                    sync_workspace_native_terminal_surface_geometry(window);
                    sync_workspace_terminal_semantic_projection(
                        window,
                        project_terminal_command_blocks(&presentable_frame.command_blocks),
                        project_terminal_overview_markers(&presentable_frame.overview_markers),
                    );
                    present_workspace_native_terminal_frame(window, frame);
                    next_surface_seqno = Some(i32::try_from(surface.seqno).unwrap_or(i32::MAX));
                    next_render_mode = Some(TerminalRenderMode::Native);
                }
                Err(err) => {
                    tracing::error!(
                        target: "app.terminal",
                        session_id = surface.session_id.to_string(),
                        error = %err,
                        "failed to render workspace terminal surface"
                    );
                    window.set_workspace_session_cell_width(
                        default_cell_width_px as f32 / scale_factor,
                    );
                    window.set_workspace_session_cell_height(
                        default_cell_height_px as f32 / scale_factor,
                    );
                    clear_workspace_native_terminal_frame(window);
                    clear_workspace_terminal_semantic_projection(window);
                    next_surface_seqno = Some(0);
                    next_render_mode = Some(TerminalRenderMode::Bitmap);
                }
            }
        });
        if native_frame_presented {
            clear_workspace_session_cursor_overlay(window);
        } else {
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
        }
        window.set_workspace_session_cursor_fg(slint_color_from_rgba(surface.cursor.fg_rgba));
        window.set_workspace_session_cursor_bg(slint_color_from_rgba(surface.cursor.bg_rgba));
        window.set_workspace_session_default_fg(slint_color_from_rgba(surface.default_fg_rgba));
        window.set_workspace_session_default_bg(slint_color_from_rgba(surface.default_bg_rgba));
        window.set_workspace_session_mouse_grabbed(surface.mouse_grabbed);
        let link_affordance = WORKSPACE_TERMINAL_POINTER_STATE.with(|pointer_state| {
            workspace_terminal::link_affordance_for_pointer(Some(surface), *pointer_state.borrow())
        });
        window.set_workspace_session_link_hovered(link_affordance.hovered);
        window.set_workspace_session_link_armed(link_affordance.armed);
        window.set_workspace_session_viewport_offset_lines(
            i32::try_from(surface.viewport_offset_lines).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_viewport_max_offset_lines(
            i32::try_from(surface.viewport_max_offset_lines).unwrap_or(i32::MAX),
        );
        window.set_workspace_session_viewport_at_bottom(surface.viewport_at_bottom);
        if let Some(next_surface_seqno) = next_surface_seqno {
            window.set_workspace_session_surface_seqno(next_surface_seqno);
        }
        if let Some(next_render_mode) = next_render_mode {
            window.set_workspace_session_render_mode(next_render_mode.as_str().into());
        }
    } else {
        let preset = terminal_theme_preset.terminal;
        clear_workspace_native_cursor_blink_state();
        window.set_workspace_session_alternate_screen_active(false);
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
        clear_workspace_terminal_semantic_projection(window);
        release_workspace_terminal_renderer_resources();
        WORKSPACE_TERMINAL_POINTER_STATE.with(|pointer_state| {
            pointer_state.borrow_mut().take();
        });
        window.set_workspace_session_mouse_grabbed(false);
        window.set_workspace_session_link_hovered(false);
        window.set_workspace_session_link_armed(false);
        window.set_workspace_session_viewport_offset_lines(0);
        window.set_workspace_session_viewport_max_offset_lines(0);
        window.set_workspace_session_viewport_at_bottom(true);
        window.set_workspace_session_surface_seqno(0);
        window.set_workspace_session_render_mode(TerminalRenderMode::Bitmap.as_str().into());
    }
}

fn sync_workspace_tabs(
    window: &AppWindow,
    state: &mut ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    sync_workspace_tabs_with_manager(window, state, follow_tracker, None);
}

fn sync_workspace_tabs_with_manager(
    window: &AppWindow,
    state: &mut ShellViewModel,
    follow_tracker: &mut WorkspaceFollowTracker,
    manager: Option<&SessionManager>,
) {
    sync_workspace_tab_items(window, state);
    sync_workspace_tab_context_menu_state(window, state);
    sync_workspace_session_state_with_manager(window, state, follow_tracker, manager);
}

fn sync_shell_state(
    window: &AppWindow,
    state: &mut ShellViewModel,
    effects: &dyn PlatformWindowEffects,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    shell_chrome::sync_top_status_bar_state(window, state, effects);
    windowing::sync_sync_modal_state(window, state);
    sftp::sync_right_panel_state(window, state);
    assets_keychain::sync_sidebar_state(window, state);
    sync_workspace_tabs(window, state, follow_tracker);
    assets_keychain::sync_assets_context_menu_state(window, state);
    assets_keychain::sync_asset_modal_state(window, state);
    sftp::sync_sftp_conflict_modal_state(window, state);
    sftp::sync_sftp_remote_file_modal_state(window, state);
    windowing::sync_ssh_host_key_modal_state(window, state);
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
        requested_assets_sidebar_width: state.requested_assets_sidebar_width(),
        requested_right_panel_width: state.requested_right_panel_width(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
    window.set_shell_body_height_cache(
        logical_height.saturating_sub(ShellMetrics::TITLEBAR_HEIGHT) as f32,
    );
    sync_workspace_native_terminal_surface_geometry(window);
    assets_keychain::update_context_menu_placement(window, state);
    assets_keychain::sync_assets_context_menu_state(window, state);
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

fn current_sync_timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:020}", elapsed.as_millis())
}

fn format_sync_timestamp_for_ui(raw: Option<&str>) -> String {
    let Some(raw) = raw.map(str::trim).filter(|value| !value.is_empty()) else {
        return "Unknown".into();
    };

    if let Some(date_time) = raw
        .parse::<i64>()
        .ok()
        .and_then(DateTime::<Utc>::from_timestamp_millis)
    {
        return date_time.format("%Y-%m-%d %H:%M").to_string();
    }

    if let Ok(date_time) = DateTime::parse_from_rfc3339(raw) {
        return date_time
            .with_timezone(&Utc)
            .format("%Y-%m-%d %H:%M")
            .to_string();
    }

    "Unknown".into()
}

fn sync_timestamp_after(candidate: &str, floor: Option<&str>) -> String {
    let Some(floor) = floor.filter(|value| !value.trim().is_empty()) else {
        return candidate.to_string();
    };

    if candidate > floor {
        return candidate.to_string();
    }

    floor
        .parse::<u128>()
        .ok()
        .and_then(|value| value.checked_add(1))
        .map(|value| format!("{value:020}"))
        .unwrap_or_else(|| candidate.to_string())
}

fn next_local_change_timestamp(local_state: &LocalVaultBootstrapState) -> String {
    let floor = [
        local_state.last_local_change_at.as_deref(),
        local_state.last_successful_push_at.as_deref(),
        local_state.last_successful_pull_at.as_deref(),
    ]
    .into_iter()
    .flatten()
    .max();

    sync_timestamp_after(current_sync_timestamp().as_str(), floor)
}

fn latest_local_sync_timestamp(vault: &VaultSessionState) -> Option<String> {
    let local_state = vault.local_state.as_ref()?;
    match (
        local_state.last_successful_push_at.as_ref(),
        local_state.last_successful_pull_at.as_ref(),
    ) {
        (Some(push), Some(pull)) => Some(if push >= pull {
            push.clone()
        } else {
            pull.clone()
        }),
        (Some(push), None) => Some(push.clone()),
        (None, Some(pull)) => Some(pull.clone()),
        (None, None) => None,
    }
}

fn payload_hash_from_encrypted_snapshot_sha(payload_sha256: &str) -> String {
    format!("sha256:{payload_sha256}")
}

fn local_sync_state_for_snapshot(
    local_state: &LocalVaultBootstrapState,
    local_snapshot_hash: String,
) -> LocalSyncState {
    LocalSyncState {
        base_revision: local_state
            .base_revision
            .clone()
            .or_else(|| local_state.current_revision.clone()),
        local_snapshot_hash: Some(local_snapshot_hash),
        last_local_change_at: local_state.last_local_change_at.clone(),
        last_successful_push_at: local_state.last_successful_push_at.clone(),
        last_successful_pull_at: local_state.last_successful_pull_at.clone(),
    }
}

fn persist_snapshot_recovery_record(
    recovery_root: &Path,
    vault_key: &[u8; 32],
    vault_id: &str,
    source: RecoverySource,
    base_revision: Option<String>,
    losing_revision: Option<String>,
    payload_hash: Option<String>,
    snapshot: &VaultSnapshot,
) -> Result<()> {
    let recovery_record = RecoverySnapshotRecord::new(
        vault_id.to_string(),
        source,
        current_sync_timestamp(),
        base_revision,
        losing_revision,
        payload_hash,
        snapshot.clone(),
    );
    persist_recovery_snapshot(recovery_root, vault_key, &recovery_record)?;
    Ok(())
}

fn persist_merge_conflict_recovery_snapshots(
    recovery_root: &Path,
    vault_key: &[u8; 32],
    vault_id: &str,
    base_revision: Option<String>,
    local_snapshot: &VaultSnapshot,
    remote_head: &VaultHead,
    remote_snapshot: &VaultSnapshot,
) -> Result<()> {
    persist_snapshot_recovery_record(
        recovery_root,
        vault_key,
        vault_id,
        RecoverySource::LocalConflictCopy,
        base_revision.clone(),
        base_revision,
        None,
        local_snapshot,
    )?;
    persist_snapshot_recovery_record(
        recovery_root,
        vault_key,
        vault_id,
        RecoverySource::RemoteConflictCopy,
        Some(remote_head.vault_revision.clone()),
        Some(remote_head.vault_revision.clone()),
        Some(remote_head.payload_hash.clone()),
        remote_snapshot,
    )
}

fn persist_merge_conflict_inbox_entries(
    conflict_root: &Path,
    vault_id: &str,
    conflicts: &[crate::app::vault::merge::MergeConflict],
    local_device_id: &str,
    remote_device_id: &str,
    captured_at: &str,
) -> Result<()> {
    if conflicts.is_empty() {
        return Ok(());
    }

    let entries = conflicts
        .iter()
        .map(|conflict| ConflictInboxEntry {
            vault_id: vault_id.to_string(),
            target_id: conflict.node_id.clone(),
            conflict_kind: conflict.message.clone(),
            local_device_id: local_device_id.to_string(),
            remote_device_id: remote_device_id.to_string(),
            captured_at: captured_at.to_string(),
        })
        .collect::<Vec<_>>();
    persist_conflict_entries(conflict_root, entries.as_slice())?;
    Ok(())
}

fn sync_modal_conflict_projection(vault: &VaultSessionState) -> (i32, String, bool) {
    let Some(vault_id) = configured_sync_bundle(vault)
        .map(|bundle| bundle.vault_id.as_str())
        .filter(|vault_id| !vault_id.trim().is_empty())
    else {
        return (0, String::new(), false);
    };

    let entries = load_conflict_entries(vault.root_dir.join("conflicts").as_path(), vault_id)
        .unwrap_or_default();
    let Some(latest) = entries.first() else {
        return (0, String::new(), false);
    };

    let count = entries.len().min(i32::MAX as usize) as i32;
    let summary = format!(
        "Latest: {} ({}) from {} against {}.",
        latest.target_id, latest.conflict_kind, latest.remote_device_id, latest.local_device_id
    );
    (count, summary, true)
}

fn update_vault_panel_for_local_state(state: &mut ShellViewModel, vault: &VaultSessionState) {
    let panel = state.vault_panel_state_mut();
    panel.primary_status_label = vault
        .local_state
        .as_ref()
        .map(|local_state| {
            if local_state
                .bundle
                .remotes
                .iter()
                .any(|remote| remote.role == RemoteRole::Primary)
            {
                "Primary configured"
            } else {
                "Primary not configured"
            }
        })
        .unwrap_or("Primary not configured")
        .into();
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
struct PersistedGitRepoCredentialMaterial {
    #[serde(default)]
    https_username: String,
    #[serde(default)]
    https_secret: String,
    #[serde(default)]
    personal_access_token: String,
    #[serde(default)]
    ssh_private_key: String,
    #[serde(default)]
    ssh_passphrase: String,
}

fn git_auth_mode_for_provider_auth(auth_kind: ProviderAuthKind) -> &'static str {
    match auth_kind {
        ProviderAuthKind::HttpsCredentials | ProviderAuthKind::Pat => "https",
        ProviderAuthKind::SshKey => "ssh",
        _ => "https",
    }
}

fn provider_auth_for_git_auth_mode(mode: &str) -> Result<ProviderAuthKind> {
    match mode.trim() {
        "" | "https" => Ok(ProviderAuthKind::Pat),
        "ssh" => Ok(ProviderAuthKind::SshKey),
        other => Err(anyhow!("Unsupported Git auth mode `{other}`")),
    }
}

fn load_git_repo_credential_material(
    credential_store: &dyn CredentialStore,
    credential_ref: Option<&str>,
    auth_kind: ProviderAuthKind,
) -> PersistedGitRepoCredentialMaterial {
    let Some(raw) = load_provider_credential(credential_store, credential_ref)
        .ok()
        .flatten()
    else {
        return PersistedGitRepoCredentialMaterial::default();
    };

    parse_git_repo_credential_material(&raw, auth_kind)
}

fn parse_git_repo_credential_material(
    raw: &str,
    auth_kind: ProviderAuthKind,
) -> PersistedGitRepoCredentialMaterial {
    serde_json::from_str::<PersistedGitRepoCredentialMaterial>(raw).unwrap_or_else(|_| {
        match auth_kind {
            ProviderAuthKind::SshKey => PersistedGitRepoCredentialMaterial {
                ssh_private_key: raw.into(),
                ..PersistedGitRepoCredentialMaterial::default()
            },
            _ => PersistedGitRepoCredentialMaterial {
                https_secret: raw.into(),
                personal_access_token: raw.into(),
                ..PersistedGitRepoCredentialMaterial::default()
            },
        }
    })
}

fn build_git_repo_credential_material(
    modal: &crate::shell::view_model::SyncModalViewState,
) -> Result<String> {
    serde_json::to_string(&PersistedGitRepoCredentialMaterial {
        https_username: modal.git_https_username.clone(),
        https_secret: modal.git_pat.clone(),
        personal_access_token: modal.git_pat.clone(),
        ssh_private_key: modal.git_ssh_private_key.clone(),
        ssh_passphrase: modal.git_ssh_passphrase.clone(),
    })
    .context("failed to encode git repo credential material")
}

fn hydrate_sync_modal_draft(
    state: &mut ShellViewModel,
    vault: &VaultSessionState,
    credential_store: &dyn CredentialStore,
) {
    state.reset_sync_modal_secret_visibility();
    let modal = state.sync_modal_state_mut();
    let bundle = configured_sync_bundle(vault);
    let primary = bundle.and_then(BootstrapBundle::primary_remote);
    let defaults = GitRepoRemoteDraft::default();
    modal.validation_state = SyncModalValidationState::Idle;
    modal.validation_message.clear();
    modal.git_provider_kind = defaults.host_kind.id().into();
    modal.git_remote_url = defaults.remote_url.clone();
    modal.git_base_url = defaults.base_url.clone();
    modal.git_api_base_url = defaults.api_base_url.clone();
    modal.git_namespace = defaults.namespace.clone();
    modal.git_repository = defaults.repository.clone();
    modal.git_branch = defaults.branch.clone();
    modal.git_root_path = defaults.root_path.clone();
    modal.git_auth_mode = git_auth_mode_for_provider_auth(defaults.auth_kind).into();
    modal.git_https_username.clear();
    modal.git_https_secret.clear();
    modal.git_pat.clear();
    modal.git_ssh_private_key.clear();
    modal.git_ssh_passphrase.clear();
    if let Some(remote) = primary
        && let BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url,
            branch,
            base_url,
            api_base_url,
            namespace,
            repository,
            root_path,
            ..
        } = &remote.locator
    {
        let credentials = load_git_repo_credential_material(
            credential_store,
            remote.credential_ref.as_deref(),
            remote.auth_kind,
        );
        modal.git_provider_kind = host_kind.id().into();
        modal.git_remote_url = remote_url.clone();
        modal.git_base_url = base_url
            .clone()
            .unwrap_or_else(|| defaults.base_url.clone());
        modal.git_api_base_url = api_base_url
            .clone()
            .unwrap_or_else(|| defaults.api_base_url.clone());
        modal.git_namespace = namespace.clone().unwrap_or_default();
        modal.git_repository = repository.clone().unwrap_or_default();
        modal.git_branch = branch.clone();
        modal.git_root_path = root_path
            .clone()
            .unwrap_or_else(|| defaults.root_path.clone());
        modal.git_auth_mode = git_auth_mode_for_provider_auth(remote.auth_kind).into();
        modal.git_https_username = credentials.https_username;
        modal.git_https_secret = credentials
            .personal_access_token
            .clone()
            .if_empty_then(credentials.https_secret.clone());
        modal.git_pat = credentials
            .personal_access_token
            .if_empty_then(credentials.https_secret);
        modal.git_ssh_private_key = credentials.ssh_private_key;
        modal.git_ssh_passphrase = credentials.ssh_passphrase;
        modal.provider_label = host_kind.label().into();
    }
    modal.master_password.clear();
}

fn build_sync_bundle_from_modal(
    state: &ShellViewModel,
    existing_bundle: Option<&BootstrapBundle>,
) -> Result<BootstrapBundle> {
    let modal = state.sync_modal_state();
    let host_kind = GitHostKind::from_id(modal.git_provider_kind.as_str());
    let git_remote_url = if !modal.git_remote_url.trim().is_empty() {
        modal.git_remote_url.trim().to_string()
    } else {
        let base_url = modal.git_base_url.trim().trim_end_matches('/');
        let namespace = modal.git_namespace.trim().trim_matches('/');
        let repository = modal.git_repository.trim().trim_matches('/');
        if base_url.is_empty() || namespace.is_empty() || repository.is_empty() {
            String::new()
        } else {
            format!("{base_url}/{namespace}/{repository}.git")
        }
    };
    let git_branch = modal.git_branch.trim();
    let auth_kind = provider_auth_for_git_auth_mode(modal.git_auth_mode.as_str())?;

    if git_remote_url.is_empty() {
        return Err(anyhow!(
            "Enter a base URL, owner/namespace, and repository before enabling sync"
        ));
    }
    if git_branch.is_empty() {
        return Err(anyhow!("Enter a Git branch before enabling sync"));
    }
    match auth_kind {
        ProviderAuthKind::Pat | ProviderAuthKind::HttpsCredentials => {
            if modal.git_https_username.trim().is_empty() {
                return Err(anyhow!("Enter an HTTPS username before enabling sync"));
            }
            if modal.git_pat.trim().is_empty() {
                return Err(anyhow!(
                    "Enter a Personal Access Token before enabling sync"
                ));
            }
        }
        ProviderAuthKind::SshKey => {
            if modal.git_ssh_private_key.trim().is_empty() {
                return Err(anyhow!("Enter an SSH private key before enabling sync"));
            }
        }
        _ => {}
    }

    let mut bundle = existing_bundle.cloned().unwrap_or_default();
    bundle.remotes = vec![BootstrapRemoteConfig {
        remote_id: sync_settings_remote_id(RemoteRole::Primary).into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url: git_remote_url,
            branch: git_branch.into(),
            base_url: Some(modal.git_base_url.trim().to_string()).filter(|value| !value.is_empty()),
            api_base_url: Some(modal.git_api_base_url.trim().to_string())
                .filter(|value| !value.is_empty()),
            namespace: Some(modal.git_namespace.trim().to_string())
                .filter(|value| !value.is_empty()),
            repository: Some(modal.git_repository.trim().to_string())
                .filter(|value| !value.is_empty()),
            root_path: Some(modal.git_root_path.trim().to_string())
                .filter(|value| !value.is_empty()),
            display_name: Some(format!(
                "{}/{}",
                modal.git_namespace.trim(),
                modal.git_repository.trim()
            ))
            .filter(|value| value != "/"),
        },
        credential_ref: Some(bootstrap_provider_credential_ref(sync_settings_remote_id(
            RemoteRole::Primary,
        ))),
        auth_kind,
        last_health: None,
    }];

    Ok(bundle)
}

fn sync_modal_validation_signature(modal: &crate::shell::view_model::SyncModalViewState) -> String {
    [
        modal.git_provider_kind.as_str(),
        modal.git_base_url.as_str(),
        modal.git_api_base_url.as_str(),
        modal.git_namespace.as_str(),
        modal.git_repository.as_str(),
        modal.git_branch.as_str(),
        modal.git_root_path.as_str(),
        modal.git_auth_mode.as_str(),
        modal.git_https_username.as_str(),
        modal.git_pat.as_str(),
        modal.git_remote_url.as_str(),
    ]
    .join("\n")
}

fn build_git_repo_validation_request(
    state: &ShellViewModel,
) -> Result<(BootstrapRemoteConfig, String)> {
    let modal = state.sync_modal_state();
    let host_kind = GitHostKind::from_id(modal.git_provider_kind.as_str());
    let base_url = modal.git_base_url.trim().trim_end_matches('/');
    let namespace = modal.git_namespace.trim().trim_matches('/');
    let repository = modal.git_repository.trim().trim_matches('/');
    let branch = modal.git_branch.trim();
    if base_url.is_empty() || namespace.is_empty() || repository.is_empty() {
        return Err(anyhow!(
            "Enter a base URL, owner/namespace, and repository before validating sync"
        ));
    }
    if branch.is_empty() {
        return Err(anyhow!("Enter a Git branch before validating sync"));
    }
    let remote = BootstrapRemoteConfig {
        remote_id: sync_settings_remote_id(RemoteRole::Primary).into(),
        role: RemoteRole::Primary,
        provider: ProviderKind::GitRepo,
        locator: BootstrapRemoteLocator::GitRepo {
            host_kind,
            remote_url: format!("{base_url}/{namespace}/{repository}.git"),
            branch: branch.into(),
            base_url: Some(base_url.to_string()),
            api_base_url: Some(modal.git_api_base_url.trim().to_string())
                .filter(|value| !value.is_empty()),
            namespace: Some(namespace.to_string()),
            repository: Some(repository.to_string()),
            root_path: Some(modal.git_root_path.trim().to_string())
                .filter(|value| !value.is_empty()),
            display_name: Some(format!("{namespace}/{repository}")),
        },
        credential_ref: None,
        auth_kind: ProviderAuthKind::Pat,
        last_health: None,
    };
    let access_token = state.sync_modal_state().git_pat.trim().to_string();
    if access_token.is_empty() {
        return Err(anyhow!(
            "Enter a Personal Access Token before validating the repository"
        ));
    }
    Ok((remote, access_token))
}

fn apply_sync_modal_validation_result(
    state: &mut ShellViewModel,
    draft_signature: &str,
    result: std::result::Result<GitRepositoryMetadata, String>,
) {
    if sync_modal_validation_signature(state.sync_modal_state()) != draft_signature {
        return;
    }

    let modal = state.sync_modal_state_mut();
    match result {
        Ok(metadata) => {
            modal.validation_state = SyncModalValidationState::Success;
            modal.validation_message = format!(
                "Validated private writable repository {}.",
                metadata.display_name
            );
            modal.error_text.clear();
            modal.target_label = metadata.display_name;
        }
        Err(error) => {
            modal.validation_state = SyncModalValidationState::BlockingError;
            modal.validation_message = "Repository validation failed.".into();
            modal.error_text = error;
        }
    }
}

trait StringEmptyExt {
    fn if_empty_then(self, fallback: String) -> String;
}

impl StringEmptyExt for String {
    fn if_empty_then(self, fallback: String) -> String {
        if self.trim().is_empty() {
            fallback
        } else {
            self
        }
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
fn windows_monitor_work_areas() -> Vec<MonitorWorkArea> {
    use windows_sys::Win32::Foundation::{BOOL, LPARAM, POINT, RECT};
    use windows_sys::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, HDC, MONITOR_DEFAULTTONEAREST, MonitorFromPoint,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    struct MonitorCollection {
        monitors: Vec<MonitorWorkArea>,
        seen: HashSet<(i32, i32, u32, u32)>,
    }

    impl MonitorCollection {
        unsafe fn push_hmonitor(&mut self, hmonitor: windows_sys::Win32::Graphics::Gdi::HMONITOR) {
            let Some(work_area) = work_area_from_hmonitor(hmonitor as isize) else {
                return;
            };
            if self
                .seen
                .insert((work_area.x, work_area.y, work_area.width, work_area.height))
            {
                self.monitors.push(work_area);
            }
        }
    }

    unsafe extern "system" fn collect_monitor_work_areas(
        hmonitor: windows_sys::Win32::Graphics::Gdi::HMONITOR,
        _hdc: HDC,
        _clip_rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let state = unsafe { &mut *(lparam as *mut MonitorCollection) };
        unsafe {
            state.push_hmonitor(hmonitor);
        }
        1
    }

    let mut collection = MonitorCollection {
        monitors: Vec::new(),
        seen: HashSet::new(),
    };

    unsafe {
        let mut cursor = POINT { x: 0, y: 0 };
        if GetCursorPos(&mut cursor) != 0 {
            collection.push_hmonitor(MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST));
        }

        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect_monitor_work_areas),
            (&mut collection as *mut MonitorCollection) as LPARAM,
        );
    }

    collection.monitors
}

#[cfg(target_os = "windows")]
fn apply_startup_window_bounds(window: &AppWindow, prefs: &UiPreferences) -> (u32, u32) {
    let desired_size = default_window_size();
    let monitors = windows_monitor_work_areas();
    let Some(bounds) = resolve_startup_bounds(prefs.window_bounds, desired_size, &monitors) else {
        apply_restored_window_size(window, desired_size);
        return desired_size;
    };

    apply_restored_window_size(window, (bounds.width, bounds.height));
    window
        .window()
        .set_position(slint::WindowPosition::Physical(
            slint::PhysicalPosition::new(bounds.x, bounds.y),
        ));
    (bounds.width, bounds.height)
}

#[cfg(not(target_os = "windows"))]
fn apply_startup_window_bounds(window: &AppWindow, _prefs: &UiPreferences) -> (u32, u32) {
    let size = default_window_size();
    apply_restored_window_size(window, size);
    size
}

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

fn merge_ui_preferences(existing: UiPreferences, mut next: UiPreferences) -> UiPreferences {
    if next.window_bounds.is_none() {
        next.window_bounds = existing.window_bounds;
    }
    next
}

fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store {
        let existing = store.load_or_default().unwrap_or_default();
        let next = merge_ui_preferences(existing, UiPreferences::from(state));
        if let Err(err) = store.save(&next) {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to save ui preferences"
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn save_window_bounds_preference(
    store: &Option<Rc<UiPreferencesStore>>,
    bounds: PersistedWindowBounds,
) {
    if let Some(store) = store {
        let mut prefs = store.load_or_default().unwrap_or_default();
        prefs.window_bounds = Some(bounds);
        if let Err(err) = store.save(&prefs) {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to save window bounds preference"
            );
        }
    }
}

#[cfg(target_os = "windows")]
fn save_restored_window_bounds_for_window(
    store: &Option<Rc<UiPreferencesStore>>,
    winit_window: &slint::winit_030::winit::window::Window,
) {
    let Some(placement) = query_true_window_placement(winit_window) else {
        return;
    };
    let Ok(position) = winit_window.outer_position() else {
        return;
    };
    let size = winit_window.outer_size();
    let monitors = windows_monitor_work_areas();
    let Some(bounds) = persisted_window_bounds_for_placement(
        placement,
        position.x,
        position.y,
        size.width,
        size.height,
        &monitors,
    ) else {
        return;
    };

    save_window_bounds_preference(store, bounds);
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

fn load_keychain_catalog(repo: &dyn KeychainCatalogRepository) -> KeychainCatalog {
    match repo.load() {
        Ok(catalog) => catalog,
        Err(err) => {
            tracing::error!(
                target: "config.keychain_catalog",
                error = %err,
                "failed to load keychain catalog"
            );
            KeychainCatalog::default()
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

fn save_keychain_catalog(
    repo: &dyn KeychainCatalogRepository,
    state: &ShellViewModel,
) -> Result<()> {
    repo.save(state.keychain_catalog())
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

fn save_keychain_catalog_if_available(
    repo: &Option<Rc<dyn KeychainCatalogRepository>>,
    state: &ShellViewModel,
) {
    if let Some(repo) = repo
        && let Err(err) = save_keychain_catalog(repo.as_ref(), state)
    {
        tracing::error!(
            target: "config.keychain_catalog",
            error = %err,
            "failed to save keychain catalog"
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

fn asset_catalog_repository_for_app() -> Result<Rc<dyn AssetCatalogRepository>> {
    let app_paths = app_root_paths_for_app()?;
    Ok(Rc::new(RedbAssetCatalogStore::new(app_paths.data_dir)))
}

fn keychain_catalog_repository_for_app() -> Result<Rc<dyn KeychainCatalogRepository>> {
    let app_paths = app_root_paths_for_app()?;
    Ok(Rc::new(RedbKeychainCatalogStore::new(app_paths.data_dir)))
}

fn transfer_store_for_app() -> Result<Arc<RedbTransferStore>> {
    let app_paths = app_root_paths_for_app()?;
    Ok(Arc::new(RedbTransferStore::new(app_paths.data_dir)))
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
                terminal_defaults: TerminalRuntimeDefaults::default(),
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
        None,
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

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_transfer_store(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    transfer_store: Arc<RedbTransferStore>,
) {
    let (session_runtime_guard, session_bridge) = match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let session_bridge = Rc::new(ShellSessionBridge {
                terminal_defaults: TerminalRuntimeDefaults::default(),
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
        Arc::new(LivePrivateKeyImporter),
        VaultRuntimeOptions::default(),
        Some(transfer_store),
    );
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_terminal_defaults(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    terminal_defaults: TerminalRuntimeDefaults,
) {
    bind_top_status_bar_with_injected_services_and_vault_runtime_and_terminal_defaults(
        window,
        store,
        effects,
        asset_repo,
        launcher,
        credential_store,
        Arc::new(LivePrivateKeyImporter),
        VaultRuntimeOptions::default(),
        terminal_defaults,
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
    bind_top_status_bar_with_injected_services_and_vault_runtime_and_terminal_defaults(
        window,
        store,
        effects,
        asset_repo,
        launcher,
        credential_store,
        private_key_importer,
        VaultRuntimeOptions::default(),
        TerminalRuntimeDefaults::default(),
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
    bind_top_status_bar_with_injected_services_and_vault_runtime_and_terminal_defaults(
        window,
        store,
        effects,
        asset_repo,
        launcher,
        credential_store,
        private_key_importer,
        vault_runtime,
        TerminalRuntimeDefaults::default(),
    );
}

#[allow(clippy::too_many_arguments)]
pub fn bind_top_status_bar_with_injected_services_and_vault_runtime_and_terminal_defaults(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
    private_key_importer: Arc<dyn PrivateKeyImporter>,
    vault_runtime: VaultRuntimeOptions,
    terminal_defaults: TerminalRuntimeDefaults,
) {
    let (session_runtime_guard, session_bridge) = match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let session_bridge = Rc::new(ShellSessionBridge {
                terminal_defaults,
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
        None,
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
            let terminal_defaults = TerminalRuntimeDefaults::default();
            let session_bridge = build_session_bridge(
                runtime.handle(),
                Arc::clone(&credential_store),
                terminal_defaults,
            );
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
                None,
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
                None,
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
    injected_transfer_store: Option<Arc<RedbTransferStore>>,
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
    let keychain_repo = if asset_repo.is_some() || std::env::var_os("MICA_TERM_APP_DIR").is_some() {
        match keychain_catalog_repository_for_app() {
            Ok(repo) => Some(repo),
            Err(err) => {
                tracing::error!(
                    target: "config.keychain_catalog",
                    error = %err,
                    "failed to resolve keychain catalog repository"
                );
                None
            }
        }
    } else {
        None
    };
    let transfer_store = injected_transfer_store.or_else(|| {
        if asset_repo.is_some() || std::env::var_os("MICA_TERM_APP_DIR").is_some() {
            match transfer_store_for_app() {
                Ok(store) => Some(store),
                Err(err) => {
                    tracing::error!(
                        target: "config.sftp_transfer_store",
                        error = %err,
                        "failed to resolve SFTP transfer store"
                    );
                    None
                }
            }
        } else {
            None
        }
    });
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    if let Some(repo) = asset_repo.as_ref() {
        let (console_tree, snippet_tree) =
            catalog_to_asset_trees(&load_asset_catalog(repo.as_ref()));
        initial_view_model.replace_console_asset_tree(console_tree);
        initial_view_model.replace_snippet_asset_tree(snippet_tree);
    }
    if let Some(repo) = keychain_repo.as_ref() {
        initial_view_model.replace_keychain_catalog(load_keychain_catalog(repo.as_ref()));
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
    if let Some(transfer_store) = transfer_store.as_ref() {
        match transfer_store.load_tasks() {
            Ok(tasks) => {
                let restored = restore_tasks_for_bootstrap(tasks.clone());
                if restored != tasks
                    && let Err(err) = transfer_store.save_tasks(&restored)
                {
                    tracing::error!(
                        target: "config.sftp_transfer_store",
                        error = %err,
                        "failed to persist normalized SFTP transfer recovery snapshot"
                    );
                }
                initial_view_model.sftp_transfer_tasks = restored;
                let _ = initial_view_model.recompute_sftp_queue_summary();
            }
            Err(err) => {
                tracing::error!(
                    target: "config.sftp_transfer_store",
                    error = %err,
                    "failed to load persisted SFTP transfers"
                );
            }
        }
    }
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.theme_variant = prefs.theme_variant;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    initial_view_model.set_right_panel_view(RightPanelView::from_id(&prefs.right_panel_view));
    if let Some(session_bridge) = session_bridge.as_ref() {
        session_bridge
            .terminal_defaults
            .set_scrollback_lines(prefs.terminal_scrollback_limit);
        session_bridge
            .terminal_defaults
            .set_theme(prefs.theme_mode, prefs.theme_variant);
    }
    initial_view_model
        .set_settings_modal_terminal_scrollback_limit(prefs.terminal_scrollback_limit as i32);
    initial_view_model.set_settings_modal_terminal_active_idle_shrink_enabled(
        prefs.terminal_active_idle_shrink_enabled,
    );
    initial_view_model.set_settings_modal_terminal_input_highlighting_enabled(
        prefs.terminal_input_highlighting_enabled,
    );
    initial_view_model.set_settings_modal_terminal_output_rule_highlighting_enabled(
        prefs.terminal_output_rule_highlighting_enabled,
    );
    initial_view_model.set_settings_modal_terminal_command_decorations_enabled(
        prefs.terminal_command_decorations_enabled,
    );
    initial_view_model.set_settings_modal_terminal_overview_markers_enabled(
        prefs.terminal_overview_markers_enabled,
    );
    initial_view_model
        .set_settings_modal_terminal_output_rule_profile(prefs.terminal_output_rule_profile.id());
    initial_view_model.set_settings_modal_terminal_search_match_highlight(
        prefs.terminal_search_match_highlight.id(),
    );
    initial_view_model
        .set_settings_modal_download_conflict_default(prefs.download_conflict_default.as_str());
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
    let mut initial_vault_session = VaultSessionState::new(
        vault_root_dir,
        Arc::clone(&vault_runtime.provider_factory),
        vault_runtime.bootstrap_template.clone(),
        initial_local_vault_state,
    );
    let initial_runtime_recovery_error =
        vault_sync::silently_restore_vault_session_from_runtime_key(
            &mut initial_view_model,
            &mut initial_vault_session,
            credential_store.as_ref(),
        );
    update_vault_panel_for_local_state(&mut initial_view_model, &initial_vault_session);
    vault_sync::update_sync_modal_for_local_state(&mut initial_view_model, &initial_vault_session);
    if let Some(error) = initial_runtime_recovery_error {
        vault_sync::set_sync_modal_error_without_opening(
            &mut initial_view_model,
            &initial_vault_session,
            error,
        );
    }
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let workspace_follow_tracker = Rc::new(RefCell::new(WorkspaceFollowTracker));
    let sftp_browser_controller = Rc::new(RefCell::new(SftpBrowserController::default()));
    let vault_session = Rc::new(RefCell::new(initial_vault_session));
    let git_repo_metadata_source = Arc::clone(&vault_runtime.git_repo_metadata_source);
    if let Some(session_bridge_ref) = session_bridge.as_ref()
        && let Err(err) = session_bridge_ref.manager.set_theme(
            view_model.borrow().theme_mode,
            view_model.borrow().theme_variant,
        )
    {
        tracing::error!(
            target: "app.ssh",
            error = %err,
            "failed to apply initial theme state to SSH session manager"
        );
    }
    let controller = Rc::new(WindowController::new(window));
    let modal_drag_state = Rc::new(RefCell::new(None::<ModalDragState>));
    let pending_host_key_approval = Rc::new(RefCell::new(None::<PendingHostKeyApproval>));
    let pending_workspace_paste_warning =
        Rc::new(RefCell::new(None::<PendingWorkspacePasteWarning>));
    let asset_click_tracker = Rc::new(RefCell::new(None::<PendingAssetClick>));
    let pending_double_click_activation = Rc::new(RefCell::new(None::<String>));
    let launcher_activation_tracker = Rc::new(RefCell::new(None::<PendingLauncherActivation>));
    WORKSPACE_RUNTIME_PROFILE.with(|runtime_profile| {
        *runtime_profile.borrow_mut() = Some(profile);
    });
    WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
        *host.borrow_mut() = None;
    });
    WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
        *surface.borrow_mut() = None;
    });

    let startup_window_size = apply_startup_window_bounds(window, &prefs);
    windowing::bind_windows_window_state_tracking(
        window,
        Rc::clone(&view_model),
        Rc::clone(&effects),
        store.clone(),
        session_bridge.clone(),
    );
    sync_shell_state(
        window,
        &mut view_model.borrow_mut(),
        effects.as_ref(),
        &mut workspace_follow_tracker.borrow_mut(),
    );
    windowing::sync_workspace_paste_warning_modal_state(window, None);
    {
        let mut state = view_model.borrow_mut();
        sync_shell_layout(
            window,
            &mut state,
            startup_window_size.0,
            startup_window_size.1,
        );
    }
    install_windows_frame_adapter(window);
    let sftp_browser_async_runtime = session_runtime_guard.as_ref().map(AppAsyncRuntime::handle);
    let (sftp_browser_result_tx, sftp_browser_result_rx) =
        std::sync::mpsc::channel::<sftp::SftpBrowserBackgroundMessage>();
    let sftp_browser_result_rx = Rc::new(RefCell::new(sftp_browser_result_rx));
    let (sftp_transfer_result_tx, sftp_transfer_result_rx) =
        std::sync::mpsc::channel::<sftp::SftpTransferBackgroundMessage>();
    let sftp_transfer_result_rx = Rc::new(RefCell::new(sftp_transfer_result_rx));
    let (sftp_local_action_result_tx, sftp_local_action_result_rx) =
        std::sync::mpsc::channel::<sftp::SftpLocalActionBackgroundMessage>();
    let sftp_local_action_result_rx = Rc::new(RefCell::new(sftp_local_action_result_rx));
    let (ssh_modal_result_tx, ssh_modal_result_rx) =
        std::sync::mpsc::channel::<SshModalBackgroundMessage>();
    let ssh_modal_result_rx = Rc::new(RefCell::new(ssh_modal_result_rx));
    let next_ssh_modal_test_request_id = Rc::new(Cell::new(0u64));
    let active_ssh_modal_test_request_id = Rc::new(RefCell::new(None::<u64>));
    let session_projection_timer = Rc::new(Timer::default());
    let input_projection_refresh_timer = Rc::new(Timer::default());
    let input_projection_refresh_gate = Rc::new(RefCell::new(
        DeferredWorkspaceProjectionRefreshGate::default(),
    ));
    let scroll_projection_refresh_timer = Rc::new(Timer::default());
    let scroll_projection_refresh_gate = Rc::new(RefCell::new(
        DeferredWorkspaceProjectionRefreshGate::default(),
    ));
    let scroll_thumb_drag_timer = Rc::new(Timer::default());
    let native_cursor_blink_timer = Rc::new(Timer::default());
    let deferred_scroll_thumb_drag =
        Rc::new(RefCell::new(DeferredWorkspaceScrollThumbDrag::default()));
    let workspace_terminal_active_surface_fingerprint = Rc::new(RefCell::new(
        None::<WorkspaceTerminalActiveSurfaceFingerprint>,
    ));
    let workspace_terminal_active_surface_since = Rc::new(RefCell::new(None::<Instant>));
    let workspace_terminal_active_idle_cache_shrunk = Rc::new(RefCell::new(false));
    let workspace_terminal_no_surface_since = Rc::new(RefCell::new(None::<Instant>));
    let workspace_terminal_idle_cache_shrunk = Rc::new(RefCell::new(false));
    WORKSPACE_NATIVE_CURSOR_BLINK_TIMER.with(|timer| {
        *timer.borrow_mut() = Some(Rc::clone(&native_cursor_blink_timer));
    });
    if let Some(session_bridge_ref) = session_bridge.as_ref() {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let manager = session_bridge_ref.manager.clone();
        let effects_ref = Rc::clone(&effects);
        let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
        let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
        let sftp_browser_controller_ref = Rc::clone(&sftp_browser_controller);
        let sftp_browser_result_rx_ref = Rc::clone(&sftp_browser_result_rx);
        let sftp_browser_result_tx_ref = sftp_browser_result_tx.clone();
        let sftp_transfer_result_rx_ref = Rc::clone(&sftp_transfer_result_rx);
        let sftp_local_action_result_rx_ref = Rc::clone(&sftp_local_action_result_rx);
        let ssh_modal_result_rx_ref = Rc::clone(&ssh_modal_result_rx);
        let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
        let active_ssh_modal_test_request_id_ref = Rc::clone(&active_ssh_modal_test_request_id);
        let sftp_browser_async_runtime_ref = sftp_browser_async_runtime.clone();
        let transfer_store_ref = transfer_store.clone();
        let workspace_terminal_active_surface_fingerprint_ref =
            Rc::clone(&workspace_terminal_active_surface_fingerprint);
        let workspace_terminal_active_surface_since_ref =
            Rc::clone(&workspace_terminal_active_surface_since);
        let workspace_terminal_active_idle_cache_shrunk_ref =
            Rc::clone(&workspace_terminal_active_idle_cache_shrunk);
        let workspace_terminal_no_surface_since_ref =
            Rc::clone(&workspace_terminal_no_surface_since);
        let workspace_terminal_idle_cache_shrunk_ref =
            Rc::clone(&workspace_terminal_idle_cache_shrunk);
        session_projection_timer.start(TimerMode::Repeated, Duration::from_millis(50), move || {
            let Some(window) = handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            let sftp_result_changed = {
                let receiver = sftp_browser_result_rx_ref.borrow();
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                sftp::drain_sftp_browser_background_messages(&mut state, &mut controller, &receiver)
            };
            let sftp_transfer_changed = {
                let receiver = sftp_transfer_result_rx_ref.borrow();
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                sftp::drain_sftp_transfer_background_messages(
                    &mut state,
                    &mut controller,
                    transfer_store_ref.as_ref(),
                    &manager,
                    sftp_browser_async_runtime_ref.as_ref(),
                    &sftp_browser_result_tx_ref,
                    &receiver,
                )
            };
            let sftp_local_action_changed = {
                let receiver = sftp_local_action_result_rx_ref.borrow();
                sftp::drain_sftp_local_action_background_messages(
                    &mut state,
                    transfer_store_ref.as_ref(),
                    &receiver,
                )
            };
            let ssh_modal_changed = {
                let receiver = ssh_modal_result_rx_ref.borrow();
                drain_ssh_modal_background_messages(
                    &mut state,
                    &pending_host_key_approval_ref,
                    &active_ssh_modal_test_request_id_ref,
                    &receiver,
                )
            };
            if sftp_transfer_changed || sftp_local_action_changed {
                shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
                sftp::sync_sftp_conflict_modal_state(&window, &state);
            }
            if ssh_modal_changed {
                assets_keychain::sync_asset_modal_state(&window, &state);
                windowing::sync_ssh_host_key_modal_state(&window, &state);
            }
            let had_active_surface = state.active_workspace_terminal_surface().is_some();
            let projection_delta =
                workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
            let surface_disappeared = projection_delta.surface_changed
                && had_active_surface
                && state.active_workspace_terminal_surface().is_none();
            let should_clear_pending_paste = pending_workspace_paste_warning_ref
                .borrow()
                .as_ref()
                .is_some_and(|pending| {
                    Some(pending.session_id) != active_workspace_session_uuid(&state)
                });
            if should_clear_pending_paste {
                pending_workspace_paste_warning_ref.borrow_mut().take();
                windowing::sync_workspace_paste_warning_modal_state(&window, None);
            }
            if projection_delta.tabs_changed {
                sync_workspace_tab_items(&window, &state);
                assets_keychain::sync_assets_context_menu_state(&window, &state);
                shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            }
            let mut right_panel_changed = sftp_result_changed || sftp_transfer_changed;
            if projection_delta.any_changed() {
                sync_workspace_session_state_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&manager),
                );
            }
            let (
                sftp_projection_changed,
                sftp_open_changed,
                sftp_retry_changed,
                sftp_follow_changed,
            ) = if state.show_right_panel {
                let sftp_projection_changed =
                    sftp::sync_active_sftp_projection_from_manager(&mut state, &manager);
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                let open_changed = sftp::ensure_active_sftp_browser_started(
                    &mut state,
                    &mut controller,
                    &manager,
                    sftp_browser_async_runtime_ref.as_ref(),
                    &sftp_browser_result_tx_ref,
                );
                let retry_changed = sftp::sync_active_sftp_browser_pending_request(
                    &mut state,
                    &mut controller,
                    &manager,
                    sftp_browser_async_runtime_ref.as_ref(),
                    &sftp_browser_result_tx_ref,
                );
                let follow_changed = sftp::sync_active_sftp_browser_follow_request(
                    &mut state,
                    &mut controller,
                    &manager,
                    sftp_browser_async_runtime_ref.as_ref(),
                    &sftp_browser_result_tx_ref,
                );
                (
                    sftp_projection_changed,
                    open_changed,
                    retry_changed,
                    follow_changed,
                )
            } else {
                (false, false, false, false)
            };
            if projection_delta.sftp_changed
                || sftp_projection_changed
                || sftp_open_changed
                || sftp_retry_changed
                || sftp_follow_changed
            {
                right_panel_changed = true;
            }
            let sftp_result_changed_after_quick_queue = {
                let receiver = sftp_browser_result_rx_ref.borrow();
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                sftp::drain_sftp_browser_background_messages(&mut state, &mut controller, &receiver)
            };
            if sftp_result_changed_after_quick_queue {
                right_panel_changed = true;
            }
            if right_panel_changed {
                sftp::sync_right_panel_state(&window, &mut state);
            }

            let (workspace_sftp_open_changed, workspace_sftp_retry_changed) = {
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                let workspace_sftp_open_changed =
                    sftp::ensure_active_workspace_sftp_browser_started(
                        &mut state,
                        &mut controller,
                        &manager,
                        sftp_browser_async_runtime_ref.as_ref(),
                        &sftp_browser_result_tx_ref,
                    );
                let workspace_sftp_retry_changed =
                    sftp::sync_active_workspace_sftp_browser_pending_request(
                        &mut state,
                        &mut controller,
                        &manager,
                        sftp_browser_async_runtime_ref.as_ref(),
                        &sftp_browser_result_tx_ref,
                    );
                (workspace_sftp_open_changed, workspace_sftp_retry_changed)
            };
            let sftp_result_changed_after_queue = {
                let receiver = sftp_browser_result_rx_ref.borrow();
                let mut controller = sftp_browser_controller_ref.borrow_mut();
                sftp::drain_sftp_browser_background_messages(&mut state, &mut controller, &receiver)
            };
            if sftp_result_changed_after_queue {
                sftp::sync_right_panel_state(&window, &mut state);
            }

            let workspace_sftp_browser_changed = workspace_sftp_open_changed
                || workspace_sftp_retry_changed
                || sftp_result_changed
                || sftp_result_changed_after_quick_queue
                || sftp_result_changed_after_queue;
            let workspace_sftp_projection_delta = if workspace_sftp_open_changed
                || workspace_sftp_retry_changed
                || sftp_result_changed
                || sftp_result_changed_after_queue
            {
                workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager)
            } else {
                WorkspaceProjectionDelta::default()
            };
            if workspace_sftp_projection_delta.tabs_changed {
                sync_workspace_tab_items(&window, &state);
                shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            }
            if workspace_sftp_projection_delta.any_changed() {
                sync_workspace_session_state_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&manager),
                );
                if workspace_sftp_projection_delta.sftp_changed {
                    sftp::sync_right_panel_state(&window, &mut state);
                }
            }
            if workspace_sftp_browser_changed {
                sftp::sync_workspace_sftp_state(&window, &mut state);
            }

            let now = Instant::now();
            let active_surface = state.active_workspace_terminal_surface();
            let has_active_surface = active_surface.is_some();
            let active_idle_shrink_enabled =
                state.settings_modal_terminal_active_idle_shrink_enabled();
            let mut active_surface_fingerprint =
                workspace_terminal_active_surface_fingerprint_ref.borrow_mut();
            let mut active_surface_since = workspace_terminal_active_surface_since_ref.borrow_mut();
            let mut active_idle_cache_shrunk =
                workspace_terminal_active_idle_cache_shrunk_ref.borrow_mut();
            update_workspace_terminal_active_idle_cache_shrink(
                active_surface,
                active_idle_shrink_enabled,
                now,
                &mut active_surface_fingerprint,
                &mut active_surface_since,
                &mut active_idle_cache_shrunk,
            );
            let mut no_surface_since = workspace_terminal_no_surface_since_ref.borrow_mut();
            let mut idle_cache_shrunk = workspace_terminal_idle_cache_shrunk_ref.borrow_mut();
            update_workspace_terminal_idle_cache_shrink(
                Some(&window),
                has_active_surface,
                surface_disappeared,
                now,
                &mut no_surface_since,
                &mut idle_cache_shrunk,
            );
        });
    }
    {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        native_cursor_blink_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(WORKSPACE_TERMINAL_CURSOR_BLINK_INTERVAL_MS),
            move || {
                let Some(window) = handle.upgrade() else {
                    clear_workspace_native_cursor_blink_state();
                    WORKSPACE_NATIVE_CURSOR_BLINK_TIMER.with(|timer| {
                        timer.borrow_mut().take();
                    });
                    return;
                };
                if window.get_workspace_session_render_mode().as_str()
                    != TerminalRenderMode::Native.as_str()
                {
                    clear_workspace_native_cursor_blink_state();
                    return;
                }
                let state = state.borrow();
                if advance_workspace_native_cursor_blink_phase(
                    state.active_workspace_terminal_surface(),
                ) {
                    sync_workspace_terminal_surface_projection_only(&window, &state);
                }
            },
        );
    }

    let vault_auto_sync_timer = Rc::new(Timer::default());
    let vault_periodic_sync_timer = Rc::new(Timer::default());
    let vault_sync_completion_timer = Rc::new(Timer::default());
    let async_runtime_handle = session_runtime_guard.as_ref().map(AppAsyncRuntime::handle);
    let vault_sync_service = Rc::new(VaultSyncService::new(
        VaultSyncServiceConfig::default().with_runtime_handle(async_runtime_handle.clone()),
    ));
    let (vault_sync_result_tx, vault_sync_result_rx) =
        std::sync::mpsc::channel::<vault_sync::VaultSyncBackgroundMessage>();
    let vault_sync_result_rx = Rc::new(RefCell::new(vault_sync_result_rx));
    let run_vault_sync_slot: Rc<RefCell<Option<Rc<dyn Fn(VaultSyncTrigger)>>>> =
        Rc::new(RefCell::new(None));
    let run_vault_sync: Rc<dyn Fn(VaultSyncTrigger)> = {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let store_ref = store.clone();
        let effects_ref = Rc::clone(&effects);
        let vault_session_ref = Rc::clone(&vault_session);
        let credential_store_ref = Arc::clone(&credential_store);
        let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
        let sync_service_ref = Rc::clone(&vault_sync_service);
        let git_repo_metadata_source_ref = Arc::clone(&git_repo_metadata_source);
        let auto_sync_timer_ref = Rc::clone(&vault_auto_sync_timer);
        let periodic_timer_keepalive = Rc::clone(&vault_periodic_sync_timer);
        let async_runtime_handle_ref = async_runtime_handle.clone();
        let vault_sync_result_tx_ref = vault_sync_result_tx.clone();
        let run_vault_sync_slot_ref = Rc::clone(&run_vault_sync_slot);
        Rc::new(move |trigger| {
            let _keep_run_vault_sync_slot_alive = &run_vault_sync_slot_ref;
            let _keep_periodic_timer_alive = &periodic_timer_keepalive;
            let Some(window) = handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            let mut vault = vault_session_ref.borrow_mut();
            let (width, height) = current_window_size(&window);
            sync_service_ref.set_remote_safety_status(
                vault
                    .local_state
                    .as_ref()
                    .map(|local_state| local_state.remote_safety_status)
                    .unwrap_or_default(),
            );
            let Some(plan) = sync_service_ref.begin_trigger(
                trigger,
                vault_sync::vault_background_sync_ready(&vault),
                vault_sync::vault_requires_initial_remote_sync(&vault),
            ) else {
                return;
            };
            let should_attempt_push = matches!(plan.execution, VaultSyncExecution::Push);
            let should_attempt_refresh = matches!(plan.execution, VaultSyncExecution::Refresh);

            if matches!(trigger, VaultSyncTrigger::Manual) {
                auto_sync_timer_ref.stop();
                state.start_sync_feedback(if should_attempt_push {
                    "Syncing pending changes..."
                } else {
                    "Checking remote sync..."
                });
                sync_shell_state(
                    &window,
                    &mut state,
                    effects_ref.as_ref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
                sync_shell_layout(&window, &mut state, width, height);
                save_ui_preferences(&store_ref, &state);
            }

            if let Some(runtime_handle) = async_runtime_handle_ref.clone() {
                let initial_state = (*state).clone();
                let worker_state = initial_state.clone();
                let worker_vault = (*vault).clone();
                let credential_store = Arc::clone(&credential_store_ref);
                let completion_tx = vault_sync_result_tx_ref.clone();
                let metadata_source = Arc::clone(&git_repo_metadata_source_ref);

                runtime_handle.spawn_blocking(move || {
                    let mut worker_state = worker_state;
                    let mut worker_vault = worker_vault;
                    let validation_result = if plan.revalidate_remote {
                        vault_sync::revalidate_primary_remote_for_sync(
                            &mut worker_state,
                            &mut worker_vault,
                            credential_store.as_ref(),
                            metadata_source.as_ref(),
                        )
                    } else {
                        Ok(())
                    };
                    let result = if let Err(err) = validation_result {
                        tracing::error!(
                            target: "app.vault",
                            error = %err,
                            vault_sync_trigger = ?trigger,
                            "vault sync remote revalidation failed in background worker"
                        );
                        Err(vault_sync::vault_sync_background_failure(
                            worker_state,
                            worker_vault,
                            false,
                        ))
                    } else if should_attempt_push {
                        match vault_sync::sync_local_vault(
                            &mut worker_state,
                            &mut worker_vault,
                            credential_store.as_ref(),
                        ) {
                            Ok(()) => Ok(vault_sync::vault_sync_background_success(
                                &initial_state,
                                worker_state,
                                worker_vault,
                                true,
                            )),
                            Err(err) => {
                                tracing::error!(
                                    target: "app.vault",
                                    error = %err,
                                    vault_sync_trigger = ?trigger,
                                    "vault sync failed in background worker"
                                );
                                vault_sync::set_sync_modal_error_without_opening(
                                    &mut worker_state,
                                    &worker_vault,
                                    err.to_string(),
                                );
                                Err(vault_sync::vault_sync_background_failure(
                                    worker_state,
                                    worker_vault,
                                    false,
                                ))
                            }
                        }
                    } else if should_attempt_refresh {
                        match vault_sync::refresh_local_vault_from_primary_remote_if_changed(
                            &mut worker_state,
                            &mut worker_vault,
                            credential_store.as_ref(),
                        ) {
                            Ok(_) => Ok(vault_sync::vault_sync_background_success(
                                &initial_state,
                                worker_state,
                                worker_vault,
                                false,
                            )),
                            Err(err) => {
                                tracing::error!(
                                    target: "app.vault",
                                    error = %err,
                                    vault_sync_trigger = ?trigger,
                                    "vault refresh failed in background worker"
                                );
                                vault_sync::set_sync_modal_error_without_opening(
                                    &mut worker_state,
                                    &worker_vault,
                                    err.to_string(),
                                );
                                Err(vault_sync::vault_sync_background_failure(
                                    worker_state,
                                    worker_vault,
                                    false,
                                ))
                            }
                        }
                    } else {
                        Ok(vault_sync::vault_sync_background_success(
                            &initial_state,
                            worker_state,
                            worker_vault,
                            false,
                        ))
                    };

                    let _ = completion_tx.send(vault_sync::VaultSyncBackgroundMessage::Completed {
                        trigger,
                        execution: plan.execution,
                        result,
                    });
                });
                return;
            }

            let validation_result = if plan.revalidate_remote {
                vault_sync::revalidate_primary_remote_for_sync(
                    &mut state,
                    &mut vault,
                    credential_store_ref.as_ref(),
                    git_repo_metadata_source_ref.as_ref(),
                )
                .map(|_| ())
            } else {
                Ok(())
            };

            let result = if let Err(err) = validation_result {
                Err(err)
            } else if should_attempt_push {
                vault_sync::sync_local_vault(&mut state, &mut vault, credential_store_ref.as_ref())
                    .map(|_| true)
            } else if should_attempt_refresh {
                vault_sync::refresh_local_vault_from_primary_remote_if_changed(
                    &mut state,
                    &mut vault,
                    credential_store_ref.as_ref(),
                )
            } else {
                Ok(false)
            };

            let rerun_trigger = sync_service_ref.finish(plan.execution, result.is_ok());

            match result {
                Ok(changed) => {
                    if matches!(trigger, VaultSyncTrigger::Manual) {
                        let feedback = if !state
                            .vault_panel_state()
                            .primary_status_label
                            .trim()
                            .is_empty()
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
                    vault_sync::set_sync_modal_error_without_opening(
                        &mut state,
                        &vault,
                        err.to_string(),
                    );
                    if matches!(trigger, VaultSyncTrigger::Manual) {
                        state.show_sync_feedback("Sync failed");
                    } else {
                        state.clear_sync_feedback();
                    }
                }
            }

            sync_shell_state(
                &window,
                &mut state,
                effects_ref.as_ref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
            sync_shell_layout(&window, &mut state, width, height);
            save_ui_preferences(&store_ref, &state);

            let next_trigger = rerun_trigger;
            drop(vault);
            drop(state);
            if let Some(trigger) = next_trigger
                && let Some(run_sync) = run_vault_sync_slot_ref.borrow().as_ref().cloned()
            {
                run_sync(trigger);
            }
        })
    };
    *run_vault_sync_slot.borrow_mut() = Some(Rc::clone(&run_vault_sync));
    let request_sync_modal_validation: Rc<dyn Fn()> = {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let store_ref = store.clone();
        let effects_ref = Rc::clone(&effects);
        let git_repo_metadata_source_ref = Arc::clone(&git_repo_metadata_source);
        let vault_sync_result_tx_ref = vault_sync_result_tx.clone();
        Rc::new(move || {
            let window = handle.unwrap();
            let mut state = state.borrow_mut();
            let draft_signature = sync_modal_validation_signature(state.sync_modal_state());
            match build_git_repo_validation_request(&state) {
                Ok((remote, access_token)) => {
                    let modal = state.sync_modal_state_mut();
                    modal.validation_state = SyncModalValidationState::Validating;
                    modal.validation_message =
                        "Validating repository visibility and write access...".into();
                    modal.error_text.clear();

                    let completion_tx = vault_sync_result_tx_ref.clone();
                    let metadata_source = Arc::clone(&git_repo_metadata_source_ref);
                    std::thread::spawn(move || {
                        let result = validate_remote_for_sync(
                            &remote,
                            metadata_source.as_ref(),
                            Some(access_token.as_str()),
                        )
                        .map_err(|err| err.to_string());
                        let _ = completion_tx.send(
                            vault_sync::VaultSyncBackgroundMessage::RemoteValidationCompleted {
                                draft_signature,
                                result,
                            },
                        );
                    });
                }
                Err(err) => {
                    let modal = state.sync_modal_state_mut();
                    modal.validation_state = SyncModalValidationState::BlockingError;
                    modal.validation_message = "Repository validation failed.".into();
                    modal.error_text = err.to_string();
                }
            }

            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            windowing::sync_sync_modal_state(&window, &state);
            save_ui_preferences(&store_ref, &state);
        })
    };
    {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let store_ref = store.clone();
        let effects_ref = Rc::clone(&effects);
        let vault_session_ref = Rc::clone(&vault_session);
        let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
        let sync_service_ref = Rc::clone(&vault_sync_service);
        let vault_sync_result_rx_ref = Rc::clone(&vault_sync_result_rx);
        let completion_timer_keepalive = Rc::clone(&vault_sync_completion_timer);
        let run_vault_sync_slot_ref = Rc::clone(&run_vault_sync_slot);
        vault_sync_completion_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let _keep_completion_timer_alive = &completion_timer_keepalive;
                let Some(window) = handle.upgrade() else {
                    return;
                };
                loop {
                    let message = {
                        let receiver = vault_sync_result_rx_ref.borrow();
                        receiver.try_recv().ok()
                    };
                    let Some(message) = message else {
                        break;
                    };
                    let rerun_trigger = {
                        let mut state = state.borrow_mut();
                        let mut vault = vault_session_ref.borrow_mut();
                        let (width, height) = current_window_size(&window);
                        let mut rerun_trigger = None;

                        match message {
                            vault_sync::VaultSyncBackgroundMessage::Completed {
                                trigger,
                                execution,
                                result,
                            } => match result {
                                Ok(success) => {
                                    if let Some(projection) = success.projection {
                                        state.replace_vault_projection(
                                            projection.console_tree,
                                            projection.snippet_tree,
                                            projection.keychain_catalog,
                                        );
                                    }
                                    vault.local_state = success.local_state;
                                    vault.decrypted_snapshot = success.decrypted_snapshot;
                                    update_vault_panel_for_local_state(&mut state, &vault);
                                    vault_sync::update_sync_modal_for_local_state(
                                        &mut state, &vault,
                                    );
                                    state.vault_panel_state_mut().primary_status_label =
                                        success.vault_panel_state.primary_status_label.clone();
                                    state.sync_modal_state_mut().status_text =
                                        success.sync_modal_state.status_text.clone();
                                    state.sync_modal_state_mut().error_text =
                                        success.sync_modal_state.error_text.clone();

                                    rerun_trigger = sync_service_ref
                                        .finish(execution, success.should_clear_dirty);

                                    if matches!(trigger, VaultSyncTrigger::Manual) {
                                        let feedback = if !success
                                            .vault_panel_state
                                            .primary_status_label
                                            .trim()
                                            .is_empty()
                                        {
                                            success.vault_panel_state.primary_status_label
                                        } else {
                                            "Sync completed".into()
                                        };
                                        state.show_sync_feedback(feedback);
                                    } else {
                                        state.clear_sync_feedback();
                                    }
                                }
                                Err(failure) => {
                                    if let Some(local_state) = failure.local_state {
                                        vault.local_state = Some(local_state);
                                    }
                                    update_vault_panel_for_local_state(&mut state, &vault);
                                    vault_sync::update_sync_modal_for_local_state(
                                        &mut state, &vault,
                                    );
                                    state.vault_panel_state_mut().primary_status_label =
                                        failure.vault_panel_state.primary_status_label.clone();
                                    state.sync_modal_state_mut().status_text =
                                        failure.sync_modal_state.status_text.clone();
                                    state.sync_modal_state_mut().error_text =
                                        failure.sync_modal_state.error_text.clone();

                                    rerun_trigger = sync_service_ref
                                        .finish(execution, failure.should_clear_dirty);

                                    if matches!(trigger, VaultSyncTrigger::Manual) {
                                        state.show_sync_feedback("Sync failed");
                                    } else {
                                        state.clear_sync_feedback();
                                    }
                                }
                            },
                            vault_sync::VaultSyncBackgroundMessage::RemoteHeadRefreshed {
                                snapshot,
                            } => {
                                sync_service_ref.finish_remote_head_refresh();
                                vault_sync::apply_remote_head_snapshot_to_sync_modal(
                                    &mut state, snapshot,
                                );
                            }
                            vault_sync::VaultSyncBackgroundMessage::RemoteValidationCompleted {
                                draft_signature,
                                result,
                            } => {
                                apply_sync_modal_validation_result(
                                    &mut state,
                                    draft_signature.as_str(),
                                    result,
                                );
                            }
                        }

                        sync_shell_state(
                            &window,
                            &mut state,
                            effects_ref.as_ref(),
                            &mut workspace_follow_tracker_ref.borrow_mut(),
                        );
                        sync_shell_layout(&window, &mut state, width, height);
                        save_ui_preferences(&store_ref, &state);

                        rerun_trigger
                    };

                    if let Some(trigger) = rerun_trigger {
                        if let Some(run_sync) = run_vault_sync_slot_ref.borrow().as_ref().cloned() {
                            run_sync(trigger);
                        }
                    }
                }
            },
        );
    }
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

    let right_panel_edge_drag_state = Rc::new(RefCell::new(None::<(f32, bool)>));

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
        state.toggle_right_panel();
        shell_chrome::sync_shell_side_regions(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let right_panel_edge_drag_state_ref = Rc::clone(&right_panel_edge_drag_state);
    window.on_right_panel_edge_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        shell_chrome::sync_shell_side_regions(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        *right_panel_edge_drag_state_ref.borrow_mut() = None;
    });

    let right_panel_edge_drag_state_ref = Rc::clone(&right_panel_edge_drag_state);
    window.on_right_panel_edge_drag_start_requested(move |width| {
        *right_panel_edge_drag_state_ref.borrow_mut() = Some((width, false));
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let right_panel_edge_drag_state_ref = Rc::clone(&right_panel_edge_drag_state);
    window.on_right_panel_edge_drag_move_requested(move |width| {
        {
            let mut drag_state = right_panel_edge_drag_state_ref.borrow_mut();
            let Some((start_width, drag_active)) = drag_state.as_mut() else {
                return;
            };
            if !*drag_active && (width - *start_width).abs() < EDGE_DRAG_THRESHOLD_PX {
                return;
            }
            *drag_active = true;
        }

        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.apply_right_panel_resize(width) {
            return;
        }

        shell_chrome::sync_shell_side_regions(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let right_panel_edge_drag_state_ref = Rc::clone(&right_panel_edge_drag_state);
    window.on_right_panel_edge_drag_end_requested(move |width| {
        let should_apply = {
            let mut drag_state = right_panel_edge_drag_state_ref.borrow_mut();
            let Some((start_width, drag_active)) = drag_state.take() else {
                return;
            };
            drag_active || (width - start_width).abs() >= EDGE_DRAG_THRESHOLD_PX
        };

        if !should_apply {
            return;
        }

        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.apply_right_panel_resize(width) {
            return;
        }

        shell_chrome::sync_shell_side_regions(
            &window,
            &mut state,
            effects_ref.as_ref(),
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
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_close_transfer_center_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_transfer_center();
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_transfer_center_pin_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.toggle_transfer_center_pin() {
            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_transfer_center_collapse_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.toggle_transfer_center_collapse() {
            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
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
        let sync_is_configured = vault
            .local_state
            .as_ref()
            .and_then(|local_state| local_state.bundle.primary_remote())
            .is_some();

        if sync_is_configured {
            drop(vault);
            drop(state);
            vault_auto_sync_timer_ref.stop();
            run_vault_sync_ref(VaultSyncTrigger::Manual);
            return;
        }
        vault_sync::update_sync_modal_for_local_state(&mut state, &vault);
        hydrate_sync_modal_draft(&mut state, &vault, credential_store_ref.as_ref());
        state.open_sync_modal();

        sync_shell_state(
            &window,
            &mut state,
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
    let vault_sync_service_ref = Rc::clone(&vault_sync_service);
    let vault_sync_result_tx_ref = vault_sync_result_tx.clone();
    window.on_open_sync_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let vault = vault_session_ref.borrow();
        hydrate_sync_modal_draft(&mut state, &vault, credential_store_ref.as_ref());
        vault_sync::update_sync_modal_for_local_state(&mut state, &vault);
        state.open_sync_modal();
        vault_sync::request_sync_modal_remote_head_refresh(
            &mut state,
            &vault,
            vault_sync_service_ref.as_ref(),
            Arc::clone(&credential_store_ref),
            &vault_sync_result_tx_ref,
        );
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        windowing::sync_sync_modal_state(&window, &state);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sync_modal_field(field.as_str(), value.to_string());
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        windowing::sync_sync_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_toggle_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sync_modal_toggle(field.as_str(), value);
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        windowing::sync_sync_modal_state(&window, &state);
    });

    let request_sync_modal_validation_ref = Rc::clone(&request_sync_modal_validation);
    window.on_sync_modal_validate_requested(move || {
        request_sync_modal_validation_ref();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(&effects);
    window.on_sync_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_sync_modal();
        shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        windowing::sync_sync_modal_state(&window, &state);
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
        if let Err(err) = vault_sync::submit_sync_modal_master_password(
            &mut state,
            &mut vault,
            credential_store_ref.as_ref(),
            &secret,
        ) {
            tracing::error!(target: "app.vault", error = %err, "failed to submit sync modal password");
            vault_sync::set_sync_modal_error(&mut state, &vault, err.to_string());
        } else {
            state.reset_sync_modal_secret_visibility();
        }
        sync_shell_state(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    window.on_sync_modal_sync_now_requested(move || {
        run_vault_sync_ref(VaultSyncTrigger::Manual);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    let vault_session_ref = Rc::clone(&vault_session);
    let credential_store_ref = Arc::clone(&credential_store);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let run_vault_sync_ref = Rc::clone(&run_vault_sync);
    let request_sync_modal_validation_ref = Rc::clone(&request_sync_modal_validation);
    window.on_sync_modal_primary_action_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let mut vault = vault_session_ref.borrow_mut();
        let (width, height) = current_window_size(&window);
        let master_password = state.sync_modal_state().master_password.clone();
        match state.sync_modal_state().mode {
            SyncModalMode::NotConfigured => {
                if state.sync_modal_state().validation_state != SyncModalValidationState::Success {
                    state.set_sync_modal_error(
                        "Validate the repository as private and writable before enabling sync.",
                    );
                } else if let Err(err) =
                    vault_sync::persist_sync_modal_settings(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    vault_sync::set_sync_modal_error(&mut state, &vault, err.to_string());
                } else if master_password.trim().is_empty() {
                    state.set_sync_modal_error("Enter a master password to enable sync.");
                } else {
                    let secret = secrecy::SecretString::new(master_password.into());
                    if let Err(err) = vault_sync::submit_sync_modal_master_password(
                        &mut state,
                        &mut vault,
                        credential_store_ref.as_ref(),
                        &secret,
                    ) {
                        tracing::error!(target: "app.vault", error = %err, "failed to enable sync from sync settings");
                        vault_sync::set_sync_modal_error(&mut state, &vault, err.to_string());
                    } else {
                        state.reset_sync_modal_secret_visibility();
                    }
                }
            }
            SyncModalMode::Paused => {
                drop(vault);
                drop(state);
                request_sync_modal_validation_ref();
                return;
            }
            SyncModalMode::Ready => {
                if let Err(err) =
                    vault_sync::persist_sync_modal_settings(&mut state, &mut vault, credential_store_ref.as_ref())
                {
                    vault_sync::set_sync_modal_error(&mut state, &vault, err.to_string());
                } else {
                    state.reset_sync_modal_secret_visibility();
                    drop(vault);
                    drop(state);
                    run_vault_sync_ref(VaultSyncTrigger::Manual);
                    return;
                }
            }
            SyncModalMode::SyncError => state.close_sync_modal(),
        }
        sync_shell_state(
            &window,
            &mut state,
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
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_sync_modal_secondary_action_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.close_sync_modal();
        sync_shell_state(
            &window,
            &mut state,
            effects_ref.as_ref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    sftp::bind_sftp_callbacks(
        window,
        &view_model,
        &store,
        &effects,
        transfer_store.as_ref(),
        &session_bridge,
        sftp_browser_async_runtime.as_ref(),
        &sftp_browser_result_tx,
        &sftp_transfer_result_tx,
        &sftp_local_action_result_tx,
        &workspace_follow_tracker,
        &sftp_browser_controller,
    );

    shell_chrome::bind_shell_chrome_callbacks(
        window,
        &view_model,
        &store,
        &effects,
        &session_bridge,
        &workspace_follow_tracker,
        &controller,
    );

    assets_keychain::bind_assets_keychain_callbacks(
        window,
        &view_model,
        &asset_repo,
        &session_bridge,
        &session_runtime_guard,
        sftp_browser_async_runtime.as_ref(),
        &sftp_browser_result_tx,
        &sftp_transfer_result_tx,
        &ssh_modal_result_tx,
        &sftp_browser_controller,
        &credential_store,
        &private_key_importer,
        &keychain_repo,
        &quick_launch_store,
        &vault_session,
        &workspace_follow_tracker,
        &pending_host_key_approval,
        &modal_drag_state,
        &vault_sync_service,
        &vault_auto_sync_timer,
        &run_vault_sync,
        &asset_click_tracker,
        &pending_double_click_activation,
        &next_ssh_modal_test_request_id,
        &active_ssh_modal_test_request_id,
    );

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
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_saved_ssh_picker_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    let launcher_activation_tracker_ref = Rc::clone(&launcher_activation_tracker);
    window.on_welcome_quick_launch_connect_requested(move |asset_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if register_launcher_activation(
            &launcher_activation_tracker_ref,
            LauncherSshOpenIntent::RecentConnection,
            asset_id.as_str(),
            Instant::now(),
        ) {
            return;
        }
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
            LauncherSshOpenIntent::RecentConnection,
        );
        sync_welcome_quick_launch_state(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_saved_ssh_picker_state(&window, &state);
        windowing::sync_ssh_host_key_modal_state(&window, &state);
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
    window.on_open_saved_ssh_modal_move_selection_requested(move |delta| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.move_saved_ssh_picker_selection(delta);
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
    let launcher_activation_tracker_ref = Rc::clone(&launcher_activation_tracker);
    window.on_open_saved_ssh_modal_asset_activated(move |asset_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        activate_saved_ssh_picker_asset(
            &window,
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            &launcher_activation_tracker_ref,
            &workspace_follow_tracker_ref,
            asset_id.as_str(),
        );
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    let launcher_activation_tracker_ref = Rc::clone(&launcher_activation_tracker);
    window.on_open_saved_ssh_modal_activate_selection_requested(move || {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let Some(asset_id) = state
            .saved_ssh_picker_selected_asset_id()
            .map(str::to_string)
        else {
            return;
        };
        activate_saved_ssh_picker_asset(
            &window,
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            &launcher_activation_tracker_ref,
            &workspace_follow_tracker_ref,
            asset_id.as_str(),
        );
        save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let ssh_modal_result_tx_ref = ssh_modal_result_tx.clone();
    let next_ssh_modal_test_request_id_ref = Rc::clone(&next_ssh_modal_test_request_id);
    let active_ssh_modal_test_request_id_ref = Rc::clone(&active_ssh_modal_test_request_id);
    window.on_ssh_host_key_modal_accept_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.accept_ssh_host_key_prompt();
        resolve_pending_host_key(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            &ssh_modal_result_tx_ref,
            &next_ssh_modal_test_request_id_ref,
            &active_ssh_modal_test_request_id_ref,
            true,
        );
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_workspace_tabs_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        windowing::sync_ssh_host_key_modal_state(&window, &state);
        assets_keychain::sync_asset_modal_state(&window, &state);
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
        assets_keychain::sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let ssh_modal_result_tx_ref = ssh_modal_result_tx.clone();
    let next_ssh_modal_test_request_id_ref = Rc::clone(&next_ssh_modal_test_request_id);
    let active_ssh_modal_test_request_id_ref = Rc::clone(&active_ssh_modal_test_request_id);
    window.on_ssh_host_key_modal_reject_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.reject_ssh_host_key_prompt();
        resolve_pending_host_key(
            &mut state,
            None,
            &pending_host_key_approval_ref,
            &ssh_modal_result_tx_ref,
            &next_ssh_modal_test_request_id_ref,
            &active_ssh_modal_test_request_id_ref,
            false,
        );
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_workspace_tabs_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            None,
        );
        windowing::sync_ssh_host_key_modal_state(&window, &state);
        assets_keychain::sync_asset_modal_state(&window, &state);
    });

    windowing::bind_windowing_callbacks(
        window,
        &view_model,
        &effects,
        &modal_drag_state,
        &controller,
    );

    let handle = window.as_weak();
    let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
    window.on_workspace_paste_warning_cancel_requested(move || {
        let window = handle.unwrap();
        pending_workspace_paste_warning_ref.borrow_mut().take();
        windowing::sync_workspace_paste_warning_modal_state(&window, None);
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
        windowing::sync_workspace_paste_warning_modal_state(&window, None);
        let Some(pending) = pending else {
            return;
        };
        if active_workspace_session_uuid(&state) != Some(pending.session_id) {
            return;
        }
        let text = if matches!(pending.prompt_mode, WorkspacePastePromptMode::Editor) {
            workspace_terminal::normalize_workspace_paste_text(&draft_text)
        } else {
            pending.text.clone()
        };

        workspace_terminal::forward_workspace_session_paste(
            &state,
            session_bridge_ref.as_deref(),
            pending.session_id,
            &text,
        );
        workspace_terminal::refresh_active_workspace_projection(
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
    let effects_ref = Rc::clone(&effects);
    window.on_workspace_tab_selected(move |tab_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        clear_workspace_tab_tooltip(&window);
        if state.activate_workspace_tab(tab_id.as_str()) {
            let defer_quick_browser_sync =
                state.show_right_panel && state.quick_browser_follows_active_terminal();
            if defer_quick_browser_sync {
                let _ = state.defer_quick_browser_follow_to_active_terminal();
            }
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = workspace_terminal::sync_active_workspace_surface_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                let (rows, cols) = state
                    .active_workspace_terminal_surface()
                    .map(|surface| (surface.rows as i32, surface.cols as i32))
                    .unwrap_or((24, 80));
                workspace_terminal::forward_active_workspace_resize(
                    &state,
                    Some(session_bridge),
                    rows,
                    cols,
                );
            }
            sync_workspace_tabs_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            if !defer_quick_browser_sync {
                sftp::sync_right_panel_state(&window, &mut state);
            }
            assets_keychain::sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_workspace_tab_hovered(move |tab_id, anchor_x, anchor_y| {
        let window = handle.unwrap();
        let state = state.borrow();
        show_workspace_tab_tooltip(&window, &state, tab_id.as_str(), anchor_x, anchor_y);
    });

    let handle = window.as_weak();
    window.on_workspace_tab_hover_ended(move |_tab_id| {
        let window = handle.unwrap();
        clear_workspace_tab_tooltip(&window);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_workspace_tab_context_menu_requested(move |tab_id, anchor_x, anchor_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        clear_workspace_tab_tooltip(&window);
        state.close_context_menu();
        if state.open_workspace_tab_context_menu(tab_id.as_str(), anchor_x, anchor_y) {
            sync_workspace_tab_context_menu_state(&window, &state);
            assets_keychain::sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_workspace_tab_context_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_workspace_tab_context_menu();
        sync_workspace_tab_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let workspace_terminal_no_surface_since_ref = Rc::clone(&workspace_terminal_no_surface_since);
    let workspace_terminal_idle_cache_shrunk_ref = Rc::clone(&workspace_terminal_idle_cache_shrunk);
    let effects_ref = Rc::clone(&effects);
    window.on_workspace_tab_close_requested(move |tab_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let had_active_surface = state.active_workspace_terminal_surface().is_some();
        if close_workspace_tab_by_id(&mut state, session_bridge_ref.as_deref(), tab_id.as_str()) {
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                let (rows, cols) = state
                    .active_workspace_terminal_surface()
                    .map(|surface| (surface.rows as i32, surface.cols as i32))
                    .unwrap_or((24, 80));
                workspace_terminal::forward_active_workspace_resize(
                    &state,
                    Some(session_bridge),
                    rows,
                    cols,
                );
            }
            let has_active_surface_after_close =
                state.active_workspace_terminal_surface().is_some();
            if had_active_surface && !has_active_surface_after_close {
                rearm_workspace_terminal_no_surface_idle_shrink(
                    Instant::now(),
                    &mut workspace_terminal_no_surface_since_ref.borrow_mut(),
                    &mut workspace_terminal_idle_cache_shrunk_ref.borrow_mut(),
                );
            }
            sync_workspace_tabs_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            assets_keychain::sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let effects_ref = Rc::clone(&effects);
    window.on_workspace_tab_reorder_requested(move |tab_id, target_index| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        clear_workspace_tab_tooltip(&window);
        state.close_workspace_tab_context_menu();
        sync_workspace_tab_context_menu_state(&window, &state);
        if state.reorder_workspace_tab(tab_id.as_str(), target_index.max(0) as usize) {
            sync_workspace_tabs_with_manager(
                &window,
                &mut state,
                &mut workspace_follow_tracker_ref.borrow_mut(),
                session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
            );
            shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let workspace_terminal_no_surface_since_ref = Rc::clone(&workspace_terminal_no_surface_since);
    let workspace_terminal_idle_cache_shrunk_ref = Rc::clone(&workspace_terminal_idle_cache_shrunk);
    let effects_ref = Rc::clone(&effects);
    window.on_workspace_tab_context_menu_action_invoked(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let anchor_tab_id = state
            .workspace_tab_context_menu_state()
            .anchor_tab_id
            .clone();
        state.close_workspace_tab_context_menu();
        clear_workspace_tab_tooltip(&window);
        sync_workspace_tab_context_menu_state(&window, &state);

        let Some(anchor_tab_id) = anchor_tab_id else {
            return;
        };

        match action_id.as_str() {
            "close" | "close-others" | "close-left" | "close-right" | "close-all" => {
                let scope = match action_id.as_str() {
                    "close" => WorkspaceTabCloseScope::One,
                    "close-others" => WorkspaceTabCloseScope::Others,
                    "close-left" => WorkspaceTabCloseScope::Left,
                    "close-right" => WorkspaceTabCloseScope::Right,
                    "close-all" => WorkspaceTabCloseScope::All,
                    _ => unreachable!(),
                };
                let plan = state.workspace_tab_close_plan(
                    (scope != WorkspaceTabCloseScope::All).then_some(anchor_tab_id.as_str()),
                    scope,
                );
                let Some(plan) = plan else {
                    return;
                };
                let had_active_surface = state.active_workspace_terminal_surface().is_some();
                if close_workspace_tabs_from_plan(&mut state, session_bridge_ref.as_deref(), plan) {
                    if let Some(session_bridge) = session_bridge_ref.as_ref() {
                        let _ = workspace_terminal::sync_workspace_projection_from_manager(
                            &mut state,
                            &session_bridge.manager,
                        );
                        let (rows, cols) = state
                            .active_workspace_terminal_surface()
                            .map(|surface| (surface.rows as i32, surface.cols as i32))
                            .unwrap_or((24, 80));
                        workspace_terminal::forward_active_workspace_resize(
                            &state,
                            Some(session_bridge),
                            rows,
                            cols,
                        );
                    }
                    let has_active_surface_after_close =
                        state.active_workspace_terminal_surface().is_some();
                    if had_active_surface && !has_active_surface_after_close {
                        rearm_workspace_terminal_no_surface_idle_shrink(
                            Instant::now(),
                            &mut workspace_terminal_no_surface_since_ref.borrow_mut(),
                            &mut workspace_terminal_idle_cache_shrunk_ref.borrow_mut(),
                        );
                    }
                    sync_workspace_tabs_with_manager(
                        &window,
                        &mut state,
                        &mut workspace_follow_tracker_ref.borrow_mut(),
                        session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                    );
                    shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
                    assets_keychain::sync_assets_context_menu_state(&window, &state);
                }
            }
            "copy-name" => {
                if let Some(text) = state.workspace_tab_copy_name_text(anchor_tab_id.as_str()) {
                    let _ = workspace_terminal::set_system_clipboard_text(text.as_str());
                }
            }
            "copy-host" => {
                if let Some(text) = state.workspace_tab_copy_host_text(anchor_tab_id.as_str()) {
                    let _ = workspace_terminal::set_system_clipboard_text(text.as_str());
                }
            }
            "reconnect" => {
                match reconnect_workspace_tab_by_id(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    anchor_tab_id.as_str(),
                ) {
                    Ok(true) => {
                        sync_workspace_tabs_with_manager(
                            &window,
                            &mut state,
                            &mut workspace_follow_tracker_ref.borrow_mut(),
                            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                        );
                        shell_chrome::sync_top_status_bar_state(
                            &window,
                            &state,
                            effects_ref.as_ref(),
                        );
                        sftp::sync_right_panel_state(&window, &mut state);
                    }
                    Ok(false) => {}
                    Err(err) => {
                        tracing::error!(
                            target: "app.ssh",
                            tab_id = anchor_tab_id.as_str(),
                            error = %err,
                            "failed to reconnect workspace tab"
                        );
                        sync_workspace_tabs_with_manager(
                            &window,
                            &mut state,
                            &mut workspace_follow_tracker_ref.borrow_mut(),
                            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                        );
                        shell_chrome::sync_top_status_bar_state(
                            &window,
                            &state,
                            effects_ref.as_ref(),
                        );
                    }
                }
            }
            "clone-connection" => {
                match clone_workspace_tab_by_id(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    anchor_tab_id.as_str(),
                ) {
                    Ok(true) => {
                        sync_workspace_tabs_with_manager(
                            &window,
                            &mut state,
                            &mut workspace_follow_tracker_ref.borrow_mut(),
                            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                        );
                        shell_chrome::sync_top_status_bar_state(
                            &window,
                            &state,
                            effects_ref.as_ref(),
                        );
                        sftp::sync_right_panel_state(&window, &mut state);
                        assets_keychain::sync_assets_context_menu_state(&window, &state);
                    }
                    Ok(false) => {
                        assets_keychain::sync_assets_context_menu_state(&window, &state);
                    }
                    Err(err) => {
                        tracing::error!(
                            target: "app.ssh",
                            tab_id = anchor_tab_id.as_str(),
                            error = %err,
                            "failed to clone workspace tab connection"
                        );
                        sync_workspace_tabs_with_manager(
                            &window,
                            &mut state,
                            &mut workspace_follow_tracker_ref.borrow_mut(),
                            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                        );
                        shell_chrome::sync_top_status_bar_state(
                            &window,
                            &state,
                            effects_ref.as_ref(),
                        );
                        assets_keychain::sync_assets_context_menu_state(&window, &state);
                    }
                }
            }
            _ => {}
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let pending_host_key_approval_ref = Rc::clone(&pending_host_key_approval);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let workspace_terminal_no_surface_since_ref = Rc::clone(&workspace_terminal_no_surface_since);
    let workspace_terminal_idle_cache_shrunk_ref = Rc::clone(&workspace_terminal_idle_cache_shrunk);
    let effects_ref = Rc::clone(&effects);
    let quick_launch_store_ref = quick_launch_store.clone();
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
                save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                );
                assets_keychain::sync_assets_context_menu_state(&window, &state);
                windowing::sync_ssh_host_key_modal_state(&window, &state);
            }
            "close-tab" => {
                let Some(tab_id) = state.active_workspace_tab_id().map(str::to_owned) else {
                    return;
                };
                let had_active_surface = state.active_workspace_terminal_surface().is_some();
                if close_workspace_tab_by_id(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    tab_id.as_str(),
                ) {
                    if let Some(session_bridge) = session_bridge_ref.as_ref() {
                        let _ = workspace_terminal::sync_workspace_projection_from_manager(
                            &mut state,
                            &session_bridge.manager,
                        );
                        let (rows, cols) = state
                            .active_workspace_terminal_surface()
                            .map(|surface| (surface.rows as i32, surface.cols as i32))
                            .unwrap_or((24, 80));
                        workspace_terminal::forward_active_workspace_resize(
                            &state,
                            Some(session_bridge),
                            rows,
                            cols,
                        );
                    }
                    let has_active_surface_after_close =
                        state.active_workspace_terminal_surface().is_some();
                    if had_active_surface && !has_active_surface_after_close {
                        rearm_workspace_terminal_no_surface_idle_shrink(
                            Instant::now(),
                            &mut workspace_terminal_no_surface_since_ref.borrow_mut(),
                            &mut workspace_terminal_idle_cache_shrunk_ref.borrow_mut(),
                        );
                    }
                    sync_workspace_tabs_with_manager(
                        &window,
                        &mut state,
                        &mut workspace_follow_tracker_ref.borrow_mut(),
                        session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                    );
                    assets_keychain::sync_assets_context_menu_state(&window, &state);
                }
            }
            "toggle-asset-search" => {
                state.activate_asset_search();
                assets_keychain::sync_assets_toolbar_state(&window, &state);
                assets_keychain::sync_console_assets(&window, &state);
                assets_keychain::sync_keychain_assets(&window, &state);
            }
            "toggle-global-menu" => {
                state.toggle_global_menu();
                window.set_show_global_menu(state.show_global_menu);
            }
            "toggle-workspace-focus-mode" => {
                shell_chrome::toggle_workspace_focus_mode_and_sync(
                    &window,
                    &mut state,
                    effects_ref.as_ref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                );
            }
            "select-all-sftp" => {
                if state.active_workspace_sftp_session().is_none() {
                    return;
                }
                if state.select_all_sftp_entries() {
                    sync_workspace_tabs_with_manager(
                        &window,
                        &mut state,
                        &mut workspace_follow_tracker_ref.borrow_mut(),
                        session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                    );
                }
            }
            "clear-selection-sftp" => {
                if state.active_workspace_sftp_session().is_none() {
                    return;
                }
                if state.clear_active_sftp_selection() {
                    sync_workspace_tabs_with_manager(
                        &window,
                        &mut state,
                        &mut workspace_follow_tracker_ref.borrow_mut(),
                        session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                    );
                }
            }
            action_id if action_id.starts_with("copy-connection-field:") => {
                let value = action_id
                    .split_once(':')
                    .map(|(_, value)| value)
                    .unwrap_or_default();
                if !value.is_empty() {
                    let _ = workspace_terminal::set_system_clipboard_text(value);
                }
            }
            "copy-connection-diagnostics" => {
                let preview_payload = ssh_status_preview_state().map(|preview| {
                    let fixture = preview.fixture();
                    fixture.diagnostics.join("\n")
                });
                let runtime_payload = || {
                    let session_bridge = session_bridge_ref.as_ref()?;
                    let session_id = active_workspace_session_uuid(&state)?;
                    let attempt = session_bridge.manager.connection_attempt(session_id)?;
                    Some(
                        attempt
                            .diagnostics
                            .iter()
                            .map(|line| line.message.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                };
                if let Some(payload) = preview_payload.or_else(runtime_payload) {
                    let _ = workspace_terminal::set_system_clipboard_text(payload.as_str());
                }
            }
            "update-trusted-host-key" => {
                tracing::warn!(
                    target: "app.ssh",
                    "host-key replacement flow is not implemented yet; showing preview-only action"
                );
            }
            "cancel-connection-attempt" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                let Some(session_id) = active_workspace_session_uuid(&state) else {
                    return;
                };
                let _ = session_bridge.manager.cancel_connection_attempt(session_id);
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
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
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            "reconnect-sftp-workspace" => {
                let Some(session_bridge) = session_bridge_ref.as_ref() else {
                    return;
                };
                if !state.reconnect_active_sftp_workspace() {
                    return;
                }
                let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
                    return;
                };
                let Some(session_id) = browser_session
                    .linked_terminal_session_id
                    .as_deref()
                    .and_then(|session_id| Uuid::parse_str(session_id).ok())
                else {
                    return;
                };
                if let Err(err) = session_bridge.manager.retry_session(session_id) {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to reconnect sftp workspace"
                    );
                    return;
                }
                state.hide_workspace_terminal_session(session_id.to_string().as_str());
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
                sftp::sync_right_panel_state(&window, &mut state);
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
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
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
                let _ = workspace_terminal::sync_workspace_projection_from_manager(
                    &mut state,
                    &session_bridge.manager,
                );
                sync_workspace_tabs_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
            _ => {}
        }
    });

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let input_projection_refresh_timer_ref = Rc::clone(&input_projection_refresh_timer);
    let input_projection_refresh_gate_ref = Rc::clone(&input_projection_refresh_gate);
    window.on_workspace_session_text_input(move |text| {
        if let Some(window) = window_handle.upgrade()
            && window.get_workspace_paste_warning_modal_open()
        {
            tracing::debug!(
                target: "app.ssh",
                text_len = text.len(),
                "ignored workspace terminal text input because the paste review modal is open"
            );
            return;
        }

        let mut state = view_model_ref.borrow_mut();
        workspace_terminal::forward_active_workspace_text_input(
            &state,
            session_bridge_ref.as_deref(),
            text.as_str(),
        );
        if let Some(window) = window_handle.upgrade() {
            if workspace_terminal::apply_local_input_projection_hint(&mut state) {
                workspace_terminal::refresh_projection_after_local_input_hint(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
            }
            workspace_terminal::schedule_workspace_input_projection_refresh(
                &window,
                Rc::clone(&view_model_ref),
                session_bridge_ref.clone(),
                Rc::clone(&workspace_follow_tracker_ref),
                Rc::clone(&input_projection_refresh_timer_ref),
                Rc::clone(&input_projection_refresh_gate_ref),
            );
        }
    });

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let input_projection_refresh_timer_ref = Rc::clone(&input_projection_refresh_timer);
    let input_projection_refresh_gate_ref = Rc::clone(&input_projection_refresh_gate);
    let effects_ref = Rc::clone(&effects);
    window.on_workspace_session_key_input(move |key, alt, ctrl, shift| {
        if let Some(window) = window_handle.upgrade()
            && window.get_workspace_paste_warning_modal_open()
        {
            if !window.get_workspace_paste_warning_editor_focused() && !alt && !ctrl && !shift {
                if key == "enter" {
                    window.invoke_workspace_paste_warning_confirm_requested();
                    return;
                }
                if key == "escape" {
                    window.invoke_workspace_paste_warning_cancel_requested();
                    return;
                }
            }

            tracing::debug!(
                target: "app.ssh",
                %key,
                alt,
                ctrl,
                shift,
                editor_focused = window.get_workspace_paste_warning_editor_focused(),
                "ignored workspace terminal key input because the paste review modal is open"
            );
            return;
        }

        let mut state = view_model_ref.borrow_mut();
        if ctrl
            && !alt
            && !shift
            && key.eq_ignore_ascii_case("l")
            && state.active_workspace_sftp_session().is_some()
            && state.begin_workspace_sftp_path_edit()
        {
            if let Some(window) = window_handle.upgrade() {
                sync_workspace_session_state_with_manager(
                    &window,
                    &mut state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
                );
            }
            return;
        }
        if key == "escape"
            && !alt
            && !ctrl
            && !shift
            && state.transfer_center_open()
            && !state.transfer_center_pinned()
        {
            state.close_transfer_center();
            if let Some(window) = window_handle.upgrade() {
                shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
            }
            return;
        }
        workspace_terminal::forward_active_workspace_key_input(
            &state,
            session_bridge_ref.as_deref(),
            key.as_str(),
            alt,
            ctrl,
            shift,
        );
        if let Some(window) = window_handle.upgrade() {
            if workspace_terminal::apply_local_input_projection_hint(&mut state) {
                workspace_terminal::refresh_projection_after_local_input_hint(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
            }
            workspace_terminal::schedule_workspace_input_projection_refresh(
                &window,
                Rc::clone(&view_model_ref),
                session_bridge_ref.clone(),
                Rc::clone(&workspace_follow_tracker_ref),
                Rc::clone(&input_projection_refresh_timer_ref),
                Rc::clone(&input_projection_refresh_gate_ref),
            );
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    window.on_workspace_session_resize_requested(move |rows, cols| {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let rect = workspace_native_terminal_rect(&window);
        tracing::trace!(
            target: "app.terminal",
            requested_rows = rows,
            requested_cols = cols,
            rect_width = rect.width,
            rect_height = rect.height,
            cell_width = window.get_workspace_session_cell_width(),
            cell_height = window.get_workspace_session_cell_height(),
            "workspace terminal host requested resize"
        );
        if !should_forward_workspace_terminal_resize(&window, rows, cols) {
            return;
        }
        if let Some(session_bridge) = session_bridge_ref.as_deref() {
            record_workspace_terminal_viewport_defaults(&window, session_bridge, rows, cols);
        }
        let state = state.borrow();
        workspace_terminal::forward_active_workspace_resize(
            &state,
            session_bridge_ref.as_deref(),
            rows,
            cols,
        );
    });

    let window_handle = window.as_weak();
    window.on_workspace_session_context_menu_open_changed(move |_open| {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        sync_workspace_native_terminal_surface_geometry(&window);
    });

    let state = Rc::clone(&view_model);
    let window_handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_search_open_requested(move || {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        state.open_workspace_terminal_search();
        sync_workspace_session_state_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let window_handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_search_close_requested(move || {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        state.close_workspace_terminal_search();
        sync_workspace_session_state_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let window_handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_search_query_changed(move |query| {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        state.set_workspace_terminal_search_query(query.to_string());
        sync_workspace_session_state_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    let window_handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    window.on_workspace_session_selection_changed(move || {
        let Some(window) = window_handle.upgrade() else {
            return;
        };
        let mut state = state.borrow_mut();
        workspace_terminal::sync_active_workspace_terminal_selection_from_window(
            &window, &mut state,
        );
        if workspace_session_uses_host_selection_overlay(&window) {
            return;
        }
        sync_workspace_session_state_with_manager(
            &window,
            &mut state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
    });

    let state = Rc::clone(&view_model);
    window.on_workspace_session_normalize_hit_col(move |row, col| {
        let state = state.borrow();
        workspace_terminal::normalize_active_workspace_hit_col(&state, row, col)
    });

    let state = Rc::clone(&view_model);
    window.on_workspace_session_normalize_selection_hit_col(move |row, col| {
        let state = state.borrow();
        workspace_terminal::normalize_active_workspace_selection_hit_col(&state, row, col)
    });

    let state = Rc::clone(&view_model);
    window.on_workspace_session_resolve_selection_gesture_range(
        move |gesture_mode, anchor_row, anchor_col, focus_row, focus_col| {
            let state = state.borrow();
            workspace_terminal::resolve_active_workspace_selection_gesture_range(
                &state,
                gesture_mode,
                anchor_row,
                anchor_col,
                focus_row,
                focus_col,
            )
        },
    );

    let state = Rc::clone(&view_model);
    let session_bridge_copy_ref = session_bridge.clone();
    window.on_workspace_session_copy_selection_requested(
        move |start_row, start_col, end_row, end_col| {
            let state = state.borrow();
            workspace_terminal::forward_active_workspace_copy_selection(
                &state,
                session_bridge_copy_ref.as_deref(),
                start_row,
                start_col,
                end_row,
                end_col,
            );
        },
    );

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let pending_workspace_paste_warning_ref = Rc::clone(&pending_workspace_paste_warning);
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let input_projection_refresh_timer_ref = Rc::clone(&input_projection_refresh_timer);
    let input_projection_refresh_gate_ref = Rc::clone(&input_projection_refresh_gate);
    window.on_workspace_session_paste_requested(move || {
        if let Some(window) = window_handle.upgrade()
            && window.get_workspace_paste_warning_modal_open()
        {
            tracing::warn!(
                target: "app.ssh",
                "ignored workspace paste request because the paste review modal is already open"
            );
            return;
        }

        let mut state = view_model_ref.borrow_mut();
        let outcome = workspace_terminal::forward_active_workspace_paste(
            &state,
            session_bridge_ref.as_deref(),
            pending_workspace_paste_warning_ref.as_ref(),
        );
        if let Some(window) = window_handle.upgrade() {
            let pending = pending_workspace_paste_warning_ref.borrow();
            windowing::sync_workspace_paste_warning_modal_state(&window, pending.as_ref());
            if matches!(outcome, WorkspacePasteRequestOutcome::Sent) {
                workspace_terminal::refresh_active_workspace_projection(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
                workspace_terminal::schedule_workspace_input_projection_refresh(
                    &window,
                    Rc::clone(&view_model_ref),
                    session_bridge_ref.clone(),
                    Rc::clone(&workspace_follow_tracker_ref),
                    Rc::clone(&input_projection_refresh_timer_ref),
                    Rc::clone(&input_projection_refresh_gate_ref),
                );
            }
        }
    });

    let state = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let workspace_link_click_candidate =
        Rc::new(RefCell::new(None::<WorkspaceTerminalLinkClickCandidate>));
    let workspace_link_click_candidate_ref = Rc::clone(&workspace_link_click_candidate);
    window.on_workspace_session_mouse_input(move |kind, button, row, col, shift, ctrl, alt| {
        let mut state = state.borrow_mut();
        let Some(kind) = workspace_terminal::parse_terminal_mouse_kind(kind.as_str()) else {
            tracing::warn!(
                target: "app.ssh",
                kind = %kind,
                "ignored unknown workspace terminal mouse kind"
            );
            return;
        };
        let Some(button) = workspace_terminal::parse_terminal_mouse_button(button.as_str()) else {
            tracing::warn!(
                target: "app.ssh",
                button = %button,
                "ignored unknown workspace terminal mouse button"
            );
            return;
        };
        let row = row.max(0) as u32;
        let col = col.max(0) as u32;
        WORKSPACE_TERMINAL_POINTER_STATE.with(|pointer_state| {
            *pointer_state.borrow_mut() =
                state.active_workspace_terminal_surface().map(|surface| {
                    workspace_terminal::WorkspaceTerminalPointerState {
                        session_id: surface.session_id,
                        row,
                        col,
                        ctrl,
                    }
                });
        });
        let link_affordance =
            workspace_terminal::link_affordance_at_active_workspace_surface(&state, row, col, ctrl);
        if let Some(window) = window_handle.upgrade() {
            window.set_workspace_session_link_hovered(link_affordance.hovered);
            window.set_workspace_session_link_armed(link_affordance.armed);
        }
        match workspace_terminal_link_mouse_decision(
            &state,
            kind,
            button,
            row,
            col,
            ctrl,
            &mut workspace_link_click_candidate_ref.borrow_mut(),
        ) {
            WorkspaceTerminalLinkMouseDecision::Open(url) => {
                if let Err(err) = crate::app::url_open::open_url(url.as_str()) {
                    tracing::warn!(
                        target: "app.ssh",
                        url,
                        error = %err,
                        "failed to open workspace terminal URL"
                    );
                }
                return;
            }
            WorkspaceTerminalLinkMouseDecision::LocalOnly => return,
            WorkspaceTerminalLinkMouseDecision::Forward => {}
        }
        workspace_terminal::forward_active_workspace_mouse_input(
            &state,
            session_bridge_ref.as_deref(),
            TerminalMouseInput {
                kind,
                button,
                row,
                col,
                shift,
                ctrl,
                alt,
            },
        );
        if let Some(window) = window_handle.upgrade() {
            workspace_terminal::refresh_active_workspace_projection(
                &window,
                &mut state,
                session_bridge_ref.as_deref(),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
        }
    });

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let scroll_projection_refresh_timer_ref = Rc::clone(&scroll_projection_refresh_timer);
    let scroll_projection_refresh_gate_ref = Rc::clone(&scroll_projection_refresh_gate);
    window.on_workspace_session_scroll_requested(move |delta_lines, row, col, shift, ctrl, alt| {
        {
            let state = view_model_ref.borrow();
            workspace_terminal::forward_active_workspace_scroll(
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
        }

        if let Some(window) = window_handle.upgrade() {
            workspace_terminal::schedule_workspace_scroll_projection_refresh(
                &window,
                Rc::clone(&view_model_ref),
                session_bridge_ref.clone(),
                Rc::clone(&workspace_follow_tracker_ref),
                Rc::clone(&scroll_projection_refresh_timer_ref),
                Rc::clone(&scroll_projection_refresh_gate_ref),
            );
        }
    });

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let scroll_thumb_drag_timer_ref = Rc::clone(&scroll_thumb_drag_timer);
    let deferred_scroll_thumb_drag_ref = Rc::clone(&deferred_scroll_thumb_drag);
    window.on_workspace_session_scroll_thumb_drag_requested(move |ratio| {
        if let Some(window) = window_handle.upgrade() {
            workspace_terminal::schedule_workspace_scroll_thumb_drag_update(
                &window,
                ratio,
                Rc::clone(&view_model_ref),
                session_bridge_ref.clone(),
                Rc::clone(&workspace_follow_tracker_ref),
                Rc::clone(&scroll_thumb_drag_timer_ref),
                Rc::clone(&deferred_scroll_thumb_drag_ref),
            );
        }
    });

    let view_model_ref = Rc::clone(&view_model);
    let session_bridge_ref = session_bridge.clone();
    let window_handle = window.as_weak();
    let workspace_follow_tracker_ref = Rc::clone(&workspace_follow_tracker);
    let scroll_projection_refresh_timer_ref = Rc::clone(&scroll_projection_refresh_timer);
    let scroll_projection_refresh_gate_ref = Rc::clone(&scroll_projection_refresh_gate);
    window.on_workspace_session_scroll_jump_requested(move |ratio| {
        {
            let state = view_model_ref.borrow();
            workspace_terminal::forward_active_workspace_scroll_ratio(
                &state,
                session_bridge_ref.as_deref(),
                ratio,
            );
        }
        if let Some(window) = window_handle.upgrade() {
            workspace_terminal::schedule_workspace_scroll_projection_refresh(
                &window,
                Rc::clone(&view_model_ref),
                session_bridge_ref.clone(),
                Rc::clone(&workspace_follow_tracker_ref),
                Rc::clone(&scroll_projection_refresh_timer_ref),
                Rc::clone(&scroll_projection_refresh_gate_ref),
            );
        }
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
            let session_bridge = build_session_bridge(
                async_runtime_handle.clone(),
                Arc::clone(&credential_store),
                TerminalRuntimeDefaults::default(),
            );
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
                None,
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

#[cfg(target_os = "windows")]
const FORCE_OPAQUE_HOST_WINDOW_ENV: &str = "MICA_TERM_FORCE_OPAQUE_HOST_WINDOW";

#[cfg(target_os = "windows")]
fn configure_window_creation_env_for_profile(profile: AppRuntimeProfile) {
    if profile.prefers_native_terminal_renderer()
        && matches!(
            profile.build_flavor,
            AppBuildFlavor::WindowsMainline | AppBuildFlavor::WindowsSoftwareCompat
        )
    {
        // Keep this process-wide hint available after window creation so renderer setup and
        // diagnostics can decide whether the shell host is actually opaque.
        unsafe {
            std::env::set_var(FORCE_OPAQUE_HOST_WINDOW_ENV, "1");
        }
    } else {
        unsafe {
            std::env::remove_var(FORCE_OPAQUE_HOST_WINDOW_ENV);
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn configure_window_creation_env_for_profile(_profile: AppRuntimeProfile) {}

pub fn run_with_profile(
    profile: AppRuntimeProfile,
    async_runtime_handle: tokio::runtime::Handle,
) -> Result<()> {
    configure_window_creation_env_for_profile(profile);
    configure_ui_font_fallbacks();
    let window = AppWindow::new()?;
    windows_icon::log_window_icon_state(&window, "after_window_new");
    log_ui_shell_font_diagnostics();
    window.set_window_title(runtime_window_title(profile).into());
    bind_top_status_bar_with_profile_and_async_handle(&window, profile, Some(async_runtime_handle));
    windows_icon::log_window_icon_state(&window, "before_window_run");
    window.run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::bootstrap::vault_sync::resolve_remote_for_sync;
    use crate::app::ssh::credentials::MemoryCredentialStore;
    use crate::app::ssh::profile::SshAuthMethod;
    use crate::app::ssh::runtime::{TerminalCellState, TerminalKeyEvent, TerminalSurfaceState};
    use crate::app::terminal_presenter::TerminalPresentationOptions;
    use anyhow::{Result, anyhow};
    use std::future::Future;
    use std::pin::Pin;

    #[derive(Clone, Default)]
    struct NoopLauncher;

    #[derive(Clone, Default)]
    struct SequencedSurfaceLauncher;

    struct NoopRuntimeControl;

    struct FailingPresenter;
    struct SizedPresenter((u32, u32));
    struct CacheTrackingPresenter {
        cached_rows: usize,
        clear_calls: usize,
    }

    #[test]
    fn merge_ui_preferences_preserves_existing_window_bounds() {
        let existing = UiPreferences {
            window_bounds: Some(PersistedWindowBounds { x: 100, y: 80 }),
            ..UiPreferences::default()
        };

        let next = UiPreferences {
            theme_mode: ThemeMode::Light,
            ..UiPreferences::default()
        };
        let expected_window_bounds = existing.window_bounds;

        let merged = merge_ui_preferences(existing, next);

        assert_eq!(merged.theme_mode, ThemeMode::Light);
        assert_eq!(merged.window_bounds, expected_window_bounds);
    }

    #[test]
    fn deferred_workspace_projection_refresh_gate_coalesces_redundant_requests() {
        let mut gate = DeferredWorkspaceProjectionRefreshGate::default();

        assert!(gate.mark_scheduled());
        assert!(
            !gate.mark_scheduled(),
            "repeated scroll refresh requests before the debounce timer fires should collapse into a single scheduled workspace projection refresh"
        );
    }

    #[test]
    fn deferred_workspace_projection_refresh_gate_reopens_after_clear() {
        let mut gate = DeferredWorkspaceProjectionRefreshGate::default();

        assert!(gate.mark_scheduled());
        gate.clear();
        assert!(
            gate.mark_scheduled(),
            "after a debounced scroll refresh runs, the next scroll interaction should be able to schedule a fresh workspace projection refresh"
        );
    }

    #[test]
    fn presenter_render_failure_falls_back_to_bitmap_presenter() -> Result<()> {
        let session_id = Uuid::new_v4();
        let surface = TerminalSurfaceState::from_visible_lines(
            session_id,
            1,
            4,
            12,
            vec!["prompt>".into(), "echo hi".into()],
        );
        let mut host =
            TerminalRendererHost::new(Box::new(FailingPresenter), TerminalRenderMode::Native);

        let frame = present_surface_update_with_bitmap_fallback(
            &mut host,
            &surface,
            TerminalRendererHostOptions::default(),
            1.0,
        )?;

        assert!(
            matches!(frame, PresentedTerminalFrame::Bitmap(_)),
            "when the requested presenter fails at runtime the workspace host should retry through the bitmap presenter instead of leaving the terminal blank"
        );
        assert_eq!(
            host.render_mode(),
            TerminalRenderMode::Bitmap,
            "after a presenter render failure the workspace host should stay on the bitmap presenter so later surface updates keep rendering visible terminal content"
        );

        Ok(())
    }

    #[test]
    fn resolve_workspace_terminal_presenter_can_use_test_override_factory() -> Result<()> {
        let (presenter, render_mode) = with_workspace_terminal_presenter_factory_for_test(
            Box::new(|_profile| {
                Ok((
                    Box::new(SizedPresenter((33, 44))) as Box<dyn TerminalPresenter>,
                    TerminalRenderMode::Bitmap,
                ))
            }),
            || resolve_workspace_terminal_presenter(AppRuntimeProfile::development()),
        )?;

        assert_eq!(
            presenter.default_cell_size(),
            (33, 44),
            "test overrides should be able to install a non-default presenter path instead of always forcing BitmapAtlasPresenter"
        );
        assert_eq!(
            render_mode,
            TerminalRenderMode::Bitmap,
            "test overrides should be able to request a specific render mode through the shared presenter resolution path"
        );
        Ok(())
    }

    fn with_bitmap_workspace_presenter_for_test<T>(body: impl FnOnce() -> T) -> T {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });
        WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
            *surface.borrow_mut() = None;
        });
        WORKSPACE_NATIVE_CURSOR_BLINK_STATE.with(|blink_state| {
            blink_state.borrow_mut().take();
        });
        let result = with_workspace_terminal_presenter_factory_for_test(
            Box::new(|_profile| {
                Ok((
                    Box::new(SizedPresenter((10, 22))) as Box<dyn TerminalPresenter>,
                    TerminalRenderMode::Bitmap,
                ))
            }),
            body,
        );
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });
        WORKSPACE_NATIVE_TERMINAL_SURFACE.with(|surface| {
            *surface.borrow_mut() = None;
        });
        WORKSPACE_NATIVE_CURSOR_BLINK_STATE.with(|blink_state| {
            blink_state.borrow_mut().take();
        });
        result
    }

    fn with_workspace_process_memory_trimmer_for_test<T>(
        trimmer: Box<TEST_WORKSPACE_PROCESS_MEMORY_TRIMMER>,
        body: impl FnOnce() -> T,
    ) -> T {
        WORKSPACE_TEST_PROCESS_MEMORY_TRIMMER_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(trimmer);
        });
        let result = body();
        WORKSPACE_TEST_PROCESS_MEMORY_TRIMMER_HOOK.with(|hook| {
            hook.borrow_mut().take();
        });
        result
    }

    fn with_workspace_backend_memory_purger_for_test<T>(
        purger: Box<TEST_WORKSPACE_BACKEND_MEMORY_PURGER>,
        body: impl FnOnce() -> T,
    ) -> T {
        WORKSPACE_TEST_BACKEND_MEMORY_PURGER_HOOK.with(|hook| {
            *hook.borrow_mut() = Some(purger);
        });
        let result = body();
        WORKSPACE_TEST_BACKEND_MEMORY_PURGER_HOOK.with(|hook| {
            hook.borrow_mut().take();
        });
        result
    }

    // Slint's generated `AppWindow::new()` setup runs close to libtest's default worker stack
    // limit, so the bitmap workspace presenter tests execute on a larger stack to avoid false
    // positive overflows from test-only setup locals.
    fn run_with_large_test_stack(body: impl FnOnce() + Send + 'static) {
        let handle = std::thread::Builder::new()
            .name("bootstrap-large-stack-test".into())
            .stack_size(8 * 1024 * 1024)
            .spawn(body)
            .expect("spawn bootstrap large-stack test");
        if let Err(panic) = handle.join() {
            std::panic::resume_unwind(panic);
        }
    }

    fn with_bitmap_workspace_presenter_on_large_stack_for_test(
        body: impl FnOnce() + Send + 'static,
    ) {
        run_with_large_test_stack(move || {
            with_bitmap_workspace_presenter_for_test(body);
        });
    }

    fn bitmap_workspace_terminal_state_fixture_for_test()
    -> (AppWindow, ShellViewModel, WorkspaceFollowTracker) {
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
        (window, state, WorkspaceFollowTracker::default())
    }

    fn seeded_bitmap_workspace_terminal_state_for_test()
    -> (AppWindow, ShellViewModel, WorkspaceFollowTracker) {
        let (window, mut state, mut follow_tracker) =
            bitmap_workspace_terminal_state_fixture_for_test();
        sync_workspace_session_state(&window, &mut state, &mut follow_tracker);
        (window, state, follow_tracker)
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

    impl TerminalPresenter for FailingPresenter {
        fn present(
            &mut self,
            _surface: &TerminalSurfaceState,
            _options: TerminalPresentationOptions,
        ) -> Result<PresentedTerminalFrame> {
            Err(anyhow!("simulated presenter failure"))
        }

        fn default_cell_size(&self) -> (u32, u32) {
            (10, 22)
        }
    }

    impl TerminalPresenter for SizedPresenter {
        fn present(
            &mut self,
            _surface: &TerminalSurfaceState,
            _options: TerminalPresentationOptions,
        ) -> Result<PresentedTerminalFrame> {
            Err(anyhow!(
                "sized presenter is only used for install-path tests"
            ))
        }

        fn default_cell_size(&self) -> (u32, u32) {
            self.0
        }
    }

    impl TerminalPresenter for CacheTrackingPresenter {
        fn present(
            &mut self,
            _surface: &TerminalSurfaceState,
            _options: TerminalPresentationOptions,
        ) -> Result<PresentedTerminalFrame> {
            Err(anyhow!(
                "cache tracking presenter is only used for cache-shrink helper tests"
            ))
        }

        fn default_cell_size(&self) -> (u32, u32) {
            (10, 22)
        }

        fn cache_stats(&self) -> crate::app::terminal_presenter::TerminalPresenterCacheStats {
            crate::app::terminal_presenter::TerminalPresenterCacheStats {
                previous_frame_rows: self.cached_rows,
                ..Default::default()
            }
        }

        fn clear_transient_caches(&mut self) {
            self.cached_rows = 0;
            self.clear_calls += 1;
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
    fn sync_timestamp_after_bumps_equal_floor_by_one() {
        assert_eq!(
            sync_timestamp_after("00000000000000000042", Some("00000000000000000042")),
            "00000000000000000043"
        );
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

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert!(!delta.surface_changed);
        assert_eq!(
            state.active_workspace_session_id(),
            Some(handle.session_id.to_string().as_str())
        );

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
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
        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert!(
            !delta.surface_changed,
            "initial projection should establish the active session id before surface hydration"
        );

        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
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
    fn workspace_projection_restores_clone_profile_from_saved_asset_metadata() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager =
            SessionManager::new_with_launcher(runtime.handle().clone(), Arc::new(NoopLauncher));
        let mut tree = crate::shell::assets::AssetTree::new();
        let asset_id = tree.insert_root_with_payload(
            crate::shell::assets::ConsoleAssetKind::SshConnection,
            "Prod Bastion",
            crate::shell::assets::AssetNodePayload::SshConnection(
                crate::shell::assets::AssetSshConnectionSpec {
                    host: "10.0.0.12".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
                    ..crate::shell::assets::AssetSshConnectionSpec::default()
                },
            ),
        );
        manager
            .open_session(
                sample_profile(asset_id.as_str()),
                OpenSessionMode::ForceNewTab,
            )
            .expect("open session");

        let mut state = ShellViewModel::default();
        state.replace_console_asset_tree(tree);

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);

        assert!(delta.tabs_changed);
        let tab = state
            .workspace_tabs()
            .first()
            .expect("projected workspace tab");
        let profile = tab
            .connection_profile
            .as_ref()
            .expect("saved SSH asset should repopulate a cloneable profile");
        assert_eq!(profile.asset_id.as_deref(), Some(asset_id.as_str()));
        assert_eq!(
            profile.credential_ref.as_deref(),
            Some("ssh/saved-secrets/asset-prod")
        );
        assert!(
            profile.password.is_none()
                && profile.private_key_content.is_none()
                && profile.passphrase.is_none(),
            "workspace projection must not copy raw SSH secrets into tab state"
        );
    }

    #[test]
    fn clone_connection_uses_saved_asset_metadata_when_tab_profile_is_missing() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager = SessionManager::new_with_launcher(
            runtime.handle().clone(),
            Arc::new(SequencedSurfaceLauncher),
        );
        let bridge = ShellSessionBridge {
            manager: manager.clone(),
            terminal_defaults: TerminalRuntimeDefaults::default(),
        };
        let mut tree = crate::shell::assets::AssetTree::new();
        let asset_id = tree.insert_root_with_payload(
            crate::shell::assets::ConsoleAssetKind::SshConnection,
            "Prod Bastion",
            crate::shell::assets::AssetNodePayload::SshConnection(
                crate::shell::assets::AssetSshConnectionSpec {
                    host: "10.0.0.12".into(),
                    user: "ops".into(),
                    port: "22".into(),
                    auth_method: "password".into(),
                    credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
                    ..crate::shell::assets::AssetSshConnectionSpec::default()
                },
            ),
        );
        let original = manager
            .open_session(
                sample_profile(asset_id.as_str()),
                OpenSessionMode::ForceNewTab,
            )
            .expect("open original session");
        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });

        let mut state = ShellViewModel::default();
        state.replace_console_asset_tree(tree);
        let _ = workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        let _ = workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        let tab_id = state.workspace_tabs()[0].tab_id.clone();

        let mut tabs = state.workspace_tabs().to_vec();
        tabs[0].connection_profile = None;
        state.set_workspace_tabs(tabs);

        assert!(
            clone_workspace_tab_by_id(&mut state, Some(&bridge), tab_id.as_str())
                .expect("clone connection should succeed")
        );

        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });
        let _ = workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        let _ = workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);

        let sessions = manager.ordered_sessions();
        assert_eq!(
            sessions.len(),
            2,
            "clone should create a second manager session"
        );
        assert_ne!(sessions[0].session_id, sessions[1].session_id);
        assert_eq!(sessions[0].asset_id.as_str(), sessions[1].asset_id.as_str());
        assert_eq!(sessions[0].asset_id.as_str(), asset_id.as_str());

        assert_eq!(state.workspace_tabs().len(), 2);
        let original_tab = state
            .workspace_tabs()
            .iter()
            .find(|tab| tab.session_id == original.session_id.to_string())
            .expect("original tab");
        assert_eq!(original_tab.state, "connected");

        let cloned_tab = state
            .workspace_tabs()
            .iter()
            .find(|tab| tab.session_id != original.session_id.to_string())
            .expect("cloned tab");
        assert_eq!(cloned_tab.asset_id.as_str(), asset_id.as_str());
        let cloned_profile = cloned_tab
            .connection_profile
            .as_ref()
            .expect("cloned tab should carry safe clone metadata");
        assert_eq!(cloned_profile.asset_id.as_deref(), Some(asset_id.as_str()));
        assert!(
            cloned_profile.password.is_none()
                && cloned_profile.private_key_content.is_none()
                && cloned_profile.passphrase.is_none()
        );
    }

    #[test]
    fn workspace_tab_from_session_projects_structured_metadata() {
        let session_id = Uuid::new_v4();
        let tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Disconnected,
            can_reconnect: true,
            enhanced_session_state: EnhancedSessionState::Enhanced,
        });

        assert_eq!(tab.tab_id, session_id.to_string());
        assert_eq!(tab.session_id, session_id.to_string());
        assert_eq!(tab.display_name, "Prod Bastion");
        assert_eq!(tab.host, "10.0.0.12");
        assert_eq!(tab.username, "ops");
        assert_eq!(tab.port, 22);
        assert_eq!(tab.connection_status, "disconnected");
        assert_eq!(tab.title, "Prod Bastion");
        assert_eq!(tab.subtitle, "");
    }

    #[test]
    fn workspace_active_tab_summary_exposes_structured_metadata() {
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

        let summary = state
            .active_workspace_tab_summary()
            .expect("active tab summary");
        assert_eq!(summary.tab_id, session_id.to_string());
        assert_eq!(summary.display_name, "Prod Bastion");
        assert_eq!(summary.host, "10.0.0.12");
        assert_eq!(summary.username, "ops");
        assert_eq!(summary.port, 22);
        assert_eq!(summary.connection_status, "connected");
    }

    #[test]
    fn workspace_projection_reorder_survives_projection_tick_and_preserves_active_tab() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager =
            SessionManager::new_with_launcher(runtime.handle().clone(), Arc::new(NoopLauncher));
        let first = manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open first session");
        let second = manager
            .open_session(sample_profile("asset-stage"), OpenSessionMode::ForceNewTab)
            .expect("open second session");
        let mut state = ShellViewModel::default();

        let initial_delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(initial_delta.tabs_changed);
        assert_eq!(
            state
                .workspace_tabs()
                .iter()
                .map(|tab| tab.tab_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                first.session_id.to_string().as_str(),
                second.session_id.to_string().as_str()
            ]
        );

        assert!(
            state.reorder_workspace_tab(second.session_id.to_string().as_str(), 0),
            "reorder should update the presentation order"
        );
        let active_before_projection = state
            .active_workspace_tab_id()
            .expect("active tab id after reorder")
            .to_string();
        assert_eq!(active_before_projection, first.session_id.to_string());
        assert_eq!(
            state
                .workspace_tabs()
                .iter()
                .map(|tab| tab.tab_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                second.session_id.to_string().as_str(),
                first.session_id.to_string().as_str()
            ]
        );

        let projection_delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(
            !projection_delta.tabs_changed,
            "projection should merge manager updates into the existing UI order instead of snapping back"
        );
        assert_eq!(
            state.active_workspace_tab_id(),
            Some(active_before_projection.as_str())
        );
        assert_eq!(
            state
                .workspace_tabs()
                .iter()
                .map(|tab| tab.tab_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                second.session_id.to_string().as_str(),
                first.session_id.to_string().as_str()
            ]
        );
    }

    #[test]
    fn workspace_close_fallback_uses_ui_order_after_reorder() {
        let first_session_id = Uuid::new_v4();
        let second_session_id = Uuid::new_v4();
        let third_session_id = Uuid::new_v4();
        let mut state = ShellViewModel::default();
        state.set_workspace_tabs(vec![
            WorkspaceTab::from_session(&SessionHandle {
                session_id: first_session_id,
                asset_id: "asset-a".into(),
                title: "A".into(),
                subtitle: "ops@10.0.0.1:22".into(),
                state: SessionState::Connected,
                can_reconnect: false,
                enhanced_session_state: EnhancedSessionState::Plain,
            }),
            WorkspaceTab::from_session(&SessionHandle {
                session_id: second_session_id,
                asset_id: "asset-b".into(),
                title: "B".into(),
                subtitle: "ops@10.0.0.2:22".into(),
                state: SessionState::Connected,
                can_reconnect: false,
                enhanced_session_state: EnhancedSessionState::Plain,
            }),
            WorkspaceTab::from_session(&SessionHandle {
                session_id: third_session_id,
                asset_id: "asset-c".into(),
                title: "C".into(),
                subtitle: "ops@10.0.0.3:22".into(),
                state: SessionState::Connected,
                can_reconnect: false,
                enhanced_session_state: EnhancedSessionState::Plain,
            }),
        ]);

        assert!(state.reorder_workspace_tab(third_session_id.to_string().as_str(), 0));
        assert!(state.activate_workspace_tab(first_session_id.to_string().as_str()));
        assert!(state.close_workspace_tab(first_session_id.to_string().as_str()));
        assert_eq!(
            state.active_workspace_tab_id(),
            Some(second_session_id.to_string().as_str()),
            "close fallback should follow the visible UI order and choose the right neighbor first"
        );
        assert_eq!(
            state
                .workspace_tabs()
                .iter()
                .map(|tab| tab.tab_id.as_str())
                .collect::<Vec<_>>(),
            vec![
                third_session_id.to_string().as_str(),
                second_session_id.to_string().as_str()
            ]
        );
    }

    #[test]
    fn workspace_merge_session_reuses_preserved_error_tab_slot() {
        let mut state = ShellViewModel::default();
        let tab_id = "workspace-terminal-error:prod".to_string();
        let profile = sample_profile("asset-prod");

        merge_workspace_tab_into_tabs(
            &mut state,
            WorkspaceTab::terminal_error(
                tab_id.clone(),
                "asset-prod",
                "Prod Bastion",
                "ops",
                "10.0.0.12",
                22,
                "connection failed",
                Some(profile.clone()),
            ),
        );
        assert_eq!(state.workspace_tabs().len(), 1);
        assert_eq!(state.workspace_tabs()[0].tab_id, tab_id);
        assert!(state.workspace_tabs()[0].session_id.is_empty());

        let session_id = Uuid::new_v4();
        merge_session_handle_into_tabs(
            &mut state,
            &SessionHandle {
                session_id,
                asset_id: "asset-prod".into(),
                title: "Prod Bastion".into(),
                subtitle: "ops@10.0.0.12:22".into(),
                state: SessionState::Connecting,
                can_reconnect: true,
                enhanced_session_state: EnhancedSessionState::Enhanced,
            },
            Some(profile),
        );

        assert_eq!(
            state.workspace_tabs().len(),
            1,
            "reconnect projection should bind the live session back into the preserved tab instead of creating a duplicate"
        );
        assert_eq!(state.workspace_tabs()[0].tab_id, tab_id);
        assert_eq!(state.workspace_tabs()[0].session_id, session_id.to_string());
        assert_eq!(state.active_workspace_tab_id(), Some(tab_id.as_str()));
    }

    #[test]
    fn workspace_projection_preserves_reconnected_error_tab_identity() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager =
            SessionManager::new_with_launcher(runtime.handle().clone(), Arc::new(NoopLauncher));
        let handle = manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open session");
        let mut state = ShellViewModel::default();
        let tab_id = "workspace-terminal-error:prod".to_string();

        state.set_workspace_tabs(vec![WorkspaceTab::terminal_error(
            tab_id.clone(),
            "asset-prod",
            "Prod Bastion",
            "ops",
            "10.0.0.12",
            22,
            "connection failed",
            Some(sample_profile("asset-prod")),
        )]);

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert_eq!(state.workspace_tabs().len(), 1);
        assert_eq!(state.workspace_tabs()[0].tab_id, tab_id);
        assert_eq!(
            state.workspace_tabs()[0].session_id,
            handle.session_id.to_string()
        );
        assert_eq!(state.active_workspace_tab_id(), Some(tab_id.as_str()));
    }

    #[test]
    fn workspace_projection_only_consumes_one_error_tab_per_live_session() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager =
            SessionManager::new_with_launcher(runtime.handle().clone(), Arc::new(NoopLauncher));
        let handle = manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open session");
        let mut state = ShellViewModel::default();
        let first_tab_id = "workspace-terminal-error:prod-a".to_string();
        let second_tab_id = "workspace-terminal-error:prod-b".to_string();
        let profile = sample_profile("asset-prod");

        state.set_workspace_tabs(vec![
            WorkspaceTab::terminal_error(
                first_tab_id.clone(),
                "asset-prod",
                "Prod Bastion",
                "ops",
                "10.0.0.12",
                22,
                "first failure",
                Some(profile.clone()),
            ),
            WorkspaceTab::terminal_error(
                second_tab_id.clone(),
                "asset-prod",
                "Prod Bastion",
                "ops",
                "10.0.0.12",
                22,
                "second failure",
                Some(profile),
            ),
        ]);

        let delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(delta.tabs_changed);
        assert_eq!(
            state.workspace_tabs().len(),
            2,
            "one live session should only consume one preserved error tab slot"
        );
        assert_eq!(state.workspace_tabs()[0].tab_id, first_tab_id);
        assert_eq!(
            state.workspace_tabs()[0].session_id,
            handle.session_id.to_string()
        );
        assert_eq!(state.workspace_tabs()[1].tab_id, second_tab_id);
        assert!(state.workspace_tabs()[1].session_id.is_empty());
        assert_eq!(state.workspace_tabs()[1].state, "error");
    }

    #[test]
    fn workspace_projection_switches_active_surface_when_projection_changes_active_tab() {
        let runtime = tokio::runtime::Runtime::new().expect("create tokio runtime");
        let manager = SessionManager::new_with_launcher(
            runtime.handle().clone(),
            Arc::new(SequencedSurfaceLauncher),
        );
        let first = manager
            .open_session(sample_profile("asset-prod"), OpenSessionMode::ForceNewTab)
            .expect("open first session");
        let second = manager
            .open_session(sample_profile("asset-stage"), OpenSessionMode::ForceNewTab)
            .expect("open second session");

        runtime.block_on(async {
            tokio::time::sleep(Duration::from_millis(20)).await;
        });

        let mut state = ShellViewModel::default();
        let initial_delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(initial_delta.tabs_changed);
        assert!(!initial_delta.surface_changed);

        let hydration_delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(hydration_delta.surface_changed);
        assert_eq!(
            state
                .active_workspace_terminal_surface()
                .map(|surface| surface.session_id),
            Some(first.session_id)
        );

        state.hide_workspace_terminal_session(first.session_id.to_string().as_str());
        let switch_delta =
            workspace_terminal::sync_workspace_projection_from_manager(&mut state, &manager);
        assert!(switch_delta.tabs_changed);
        assert!(
            switch_delta.surface_changed,
            "when projection promotes a different terminal tab to active, the visible surface should switch in the same tick"
        );
        assert_eq!(
            state.active_workspace_tab_id(),
            Some(second.session_id.to_string().as_str())
        );
        assert_eq!(
            state
                .active_workspace_terminal_surface()
                .map(|surface| surface.session_id),
            Some(second.session_id)
        );
    }

    #[test]
    fn workspace_session_state_refreshes_terminal_image_across_surface_updates() {
        with_bitmap_workspace_presenter_on_large_stack_for_test(|| {
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
            let mut initial_surface = TerminalSurfaceState::from_visible_lines(
                session_id,
                1,
                24,
                80,
                vec!["welcome".into()],
            );
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

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);
            let initial_lines_model = window.get_workspace_session_visible_lines();
            let initial_surface_seqno = window.get_workspace_session_surface_seqno();

            assert_eq!(
                window.get_workspace_session_native_frame_token(),
                0,
                "bitmap presentation paths should keep the native frame token cleared"
            );
            assert_eq!(initial_surface_seqno, 1);
            assert_eq!(
                window.get_workspace_session_render_mode().as_str(),
                "bitmap"
            );

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

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);

            assert_eq!(
                window.get_workspace_session_visible_lines(),
                initial_lines_model,
                "terminal visible line projection should reuse the same VecModel instance"
            );
            assert_eq!(
                window.get_workspace_session_native_frame_token(),
                0,
                "bitmap presentation paths should keep the native frame token cleared while surface seqno tracks the visible frame"
            );
            assert_ne!(
                window.get_workspace_session_surface_seqno(),
                initial_surface_seqno
            );
            assert_eq!(window.get_workspace_session_surface_seqno(), 2);
            assert_eq!(
                window.get_workspace_session_render_mode().as_str(),
                "bitmap"
            );
        });
    }

    #[test]
    fn bitmap_workspace_selection_projection_restores_host_mirror_from_rust_truth() {
        with_bitmap_workspace_presenter_on_large_stack_for_test(|| {
            let (window, mut state, mut follow_tracker) =
                seeded_bitmap_workspace_terminal_state_for_test();
            let surface = state
                .active_workspace_terminal_surface()
                .cloned()
                .expect("active bitmap surface");
            state.set_workspace_terminal_selection(Some(
                crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(
                    &surface,
                    crate::app::terminal_model::TerminalSelectionModel::new(0, 0, 0, 1),
                ),
            ));

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);

            assert!(
                window.get_workspace_session_selection_active(),
                "bitmap projection should mirror the Rust-owned terminal selection into the host selection properties before the local overlay paints"
            );
            assert_eq!(window.get_workspace_session_selection_start_row(), 0);
            assert_eq!(window.get_workspace_session_selection_end_col(), 1);

            window.set_workspace_session_selection_active(false);
            window.set_workspace_session_selection_start_row(-1);
            window.set_workspace_session_selection_start_col(-1);
            window.set_workspace_session_selection_end_row(-1);
            window.set_workspace_session_selection_end_col(-1);

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);

            assert!(
                window.get_workspace_session_selection_active(),
                "bitmap projection should restore the host-side mirror from Rust-owned selection truth during the next sync instead of dropping the live overlay when the Slint properties are clobbered"
            );
            assert_eq!(window.get_workspace_session_selection_start_row(), 0);
            assert_eq!(window.get_workspace_session_selection_end_col(), 1);
        });
    }

    #[test]
    fn workspace_session_state_clears_native_terminal_frame_when_surface_clears() {
        with_bitmap_workspace_presenter_on_large_stack_for_test(|| {
            let (window, mut state, mut follow_tracker) =
                seeded_bitmap_workspace_terminal_state_for_test();
            let initial_lines_model = window.get_workspace_session_visible_lines();
            assert_eq!(
                window.get_workspace_session_native_frame_token(),
                0,
                "bitmap presentation paths should keep the native frame token cleared even after publishing a visible terminal frame"
            );
            assert_eq!(window.get_workspace_session_surface_seqno(), 1);
            assert_eq!(
                window.get_workspace_session_render_mode().as_str(),
                "bitmap"
            );

            state.set_active_workspace_terminal_surface(None);
            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);

            assert_eq!(
                window.get_workspace_session_visible_lines(),
                initial_lines_model,
                "clearing the surface should keep reusing the visible line model"
            );
            assert_eq!(
                window.get_workspace_session_native_frame_token(),
                0,
                "clearing the surface should reset the retained native frame token"
            );
            assert_eq!(window.get_workspace_session_surface_seqno(), 0);
            assert_eq!(
                window.get_workspace_session_render_mode().as_str(),
                "bitmap"
            );
            assert_eq!(window.get_workspace_session_visible_lines().row_count(), 0);
            WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
                assert!(
                    host.borrow().is_none(),
                    "clearing the last active workspace terminal surface should also release the shared presenter host so terminal caches do not stay resident after the UI has no surface left to render"
                );
            });
        });
    }

    #[test]
    fn idle_cache_shrink_does_not_arm_without_a_surface_clear_transition() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });

        let now = Instant::now();
        let mut no_surface_since = None;
        let mut idle_cache_shrunk = false;

        update_workspace_terminal_idle_cache_shrink(
            None,
            false,
            false,
            now,
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert!(
            no_surface_since.is_none(),
            "cold startup without an active terminal surface should not automatically arm the no-surface idle shrink timer because there is no close/disappear transition to diagnose"
        );
        assert!(
            !idle_cache_shrunk,
            "without a surface-clear transition the idle shrink path should stay dormant"
        );
    }

    #[test]
    fn idle_cache_shrink_arms_after_surface_disappears() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });

        let now = Instant::now();
        let mut no_surface_since = None;
        let mut idle_cache_shrunk = true;

        update_workspace_terminal_idle_cache_shrink(
            None,
            false,
            true,
            now,
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert_eq!(
            no_surface_since,
            Some(now),
            "surface clear transitions should start the no-surface idle window from the moment the active terminal surface disappears"
        );
        assert!(
            !idle_cache_shrunk,
            "the immediate close-shrink should re-arm the delayed idle shrink so it can run later if the workspace still has no active surface"
        );
    }

    #[test]
    fn idle_cache_shrink_resets_when_surface_returns() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });

        let now = Instant::now();
        let mut no_surface_since = Some(now);
        let mut idle_cache_shrunk = true;

        update_workspace_terminal_idle_cache_shrink(
            None,
            true,
            false,
            now + Duration::from_millis(1),
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert!(
            no_surface_since.is_none(),
            "any visible terminal surface should cancel the no-surface idle shrink timer immediately"
        );
        assert!(
            !idle_cache_shrunk,
            "returning to an active surface should clear the idle-shrunk marker so the next real disappearance can schedule a fresh delayed shrink"
        );
    }

    #[test]
    fn idle_cache_shrink_marks_itself_after_threshold() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = None;
        });

        let now = Instant::now();
        let mut no_surface_since = Some(now);
        let mut idle_cache_shrunk = false;

        update_workspace_terminal_idle_cache_shrink(
            None,
            false,
            false,
            now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert_eq!(
            no_surface_since,
            Some(now),
            "idle shrink should preserve the original no-surface timestamp so diagnostics can report how long the workspace had been idle before the delayed shrink fired"
        );
        assert!(
            idle_cache_shrunk,
            "once the no-surface threshold elapses the idle shrink path should mark itself complete until an active surface returns"
        );
    }

    #[test]
    fn idle_cache_shrink_arms_when_surface_was_cleared_before_timer_observed_transition() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(SizedPresenter((10, 22))),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let mut no_surface_since = None;
        let mut idle_cache_shrunk = false;

        update_workspace_terminal_idle_cache_shrink(
            None,
            false,
            false,
            now,
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert_eq!(
            no_surface_since,
            Some(now),
            "when the UI already cleared the active terminal surface before the timer tick runs, retained renderer resources should still arm the no-surface idle timer so close-triggered memory release is not skipped in real tab-close flows"
        );
        assert!(
            !idle_cache_shrunk,
            "arming the delayed no-surface shrink from retained renderer resources should only schedule the later release, not mark the idle shrink complete immediately"
        );
    }

    #[test]
    fn idle_cache_shrink_releases_workspace_terminal_host_after_threshold() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(SizedPresenter((10, 22))),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let mut no_surface_since = Some(now);
        let mut idle_cache_shrunk = false;

        update_workspace_terminal_idle_cache_shrink(
            None,
            false,
            false,
            now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            assert!(
                host.borrow().is_none(),
                "after the no-surface idle threshold elapses, the workspace terminal presenter host should be dropped so terminal font/render state does not stay resident indefinitely"
            );
        });
    }

    #[test]
    fn idle_cache_shrink_trims_process_working_set_after_no_surface_threshold() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(SizedPresenter((10, 22))),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let mut no_surface_since = Some(now);
        let mut idle_cache_shrunk = false;
        let trim_calls = Rc::new(RefCell::new(0usize));
        let trim_calls_ref = Rc::clone(&trim_calls);

        with_workspace_process_memory_trimmer_for_test(
            Box::new(move || {
                *trim_calls_ref.borrow_mut() += 1;
                true
            }),
            || {
                update_workspace_terminal_idle_cache_shrink(
                    None,
                    false,
                    false,
                    now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
                    &mut no_surface_since,
                    &mut idle_cache_shrunk,
                );
            },
        );

        assert_eq!(
            *trim_calls.borrow(),
            1,
            "once the workspace stays without an active surface past the idle threshold, bootstrap should also request a process working-set trim so Windows can drop resident pages after renderer resources are released"
        );
    }

    #[test]
    fn idle_cache_shrink_requests_backend_purge_after_no_surface_threshold() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(SizedPresenter((10, 22))),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let mut no_surface_since = Some(now);
        let mut idle_cache_shrunk = false;
        let purge_calls = Rc::new(RefCell::new(0usize));
        let purge_calls_ref = Rc::clone(&purge_calls);

        with_workspace_backend_memory_purger_for_test(
            Box::new(move || {
                *purge_calls_ref.borrow_mut() += 1;
                true
            }),
            || {
                update_workspace_terminal_idle_cache_shrink(
                    None,
                    false,
                    false,
                    now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
                    &mut no_surface_since,
                    &mut idle_cache_shrunk,
                );
            },
        );

        assert_eq!(
            *purge_calls.borrow(),
            1,
            "once the workspace stays without an active surface past the idle threshold, bootstrap should also request a Slint backend purge so renderer-global caches can be reclaimed before only trimming the process working set"
        );
    }

    #[test]
    fn rearm_workspace_terminal_no_surface_idle_shrink_resets_the_delayed_trim_window() {
        let first_seen = Instant::now();
        let rearmed_at = first_seen + Duration::from_millis(25);
        let mut no_surface_since = Some(first_seen);
        let mut idle_cache_shrunk = true;

        rearm_workspace_terminal_no_surface_idle_shrink(
            rearmed_at,
            &mut no_surface_since,
            &mut idle_cache_shrunk,
        );

        assert_eq!(
            no_surface_since,
            Some(rearmed_at),
            "manual close paths that already cleared the renderer host should still stamp a fresh no-surface timestamp so the delayed process trim can run from the actual tab-close moment"
        );
        assert!(
            !idle_cache_shrunk,
            "re-arming the no-surface idle window should clear the completion flag so the delayed trim can fire again after the close transition"
        );
    }

    #[test]
    fn active_idle_cache_shrink_clears_transient_caches_without_releasing_visible_host() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(CacheTrackingPresenter {
                    cached_rows: 12,
                    clear_calls: 0,
                }),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let surface = TerminalSurfaceState::from_visible_lines(
            Uuid::new_v4(),
            7,
            24,
            80,
            vec!["welcome".into()],
        );
        let mut active_surface_fingerprint = None;
        let mut active_surface_since = None;
        let mut active_idle_cache_shrunk = false;

        update_workspace_terminal_active_idle_cache_shrink(
            Some(&surface),
            true,
            now,
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );
        update_workspace_terminal_active_idle_cache_shrink(
            Some(&surface),
            true,
            now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );

        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            let host = host.borrow();
            let host = host
                .as_ref()
                .expect("active idle shrink should keep the visible host resident");
            assert_eq!(
                host.cache_stats().previous_frame_rows,
                0,
                "active idle shrink should clear presenter transient caches once the visible surface stays stable past the threshold"
            );
        });
        assert_eq!(active_surface_since, Some(now));
        assert!(
            active_idle_cache_shrunk,
            "after the active idle threshold elapses the helper should mark the visible surface as shrunk until the surface changes again"
        );
    }

    #[test]
    fn active_idle_cache_shrink_resets_when_surface_changes_before_threshold() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(CacheTrackingPresenter {
                    cached_rows: 12,
                    clear_calls: 0,
                }),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let first_surface = TerminalSurfaceState::from_visible_lines(
            Uuid::new_v4(),
            7,
            24,
            80,
            vec!["welcome".into()],
        );
        let mut second_surface = first_surface.clone();
        second_surface.seqno += 1;
        second_surface.viewport_offset_lines = 3;
        let reset_at = now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS / 2);
        let before_threshold_again =
            reset_at + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS / 2);
        let mut active_surface_fingerprint = None;
        let mut active_surface_since = None;
        let mut active_idle_cache_shrunk = false;

        update_workspace_terminal_active_idle_cache_shrink(
            Some(&first_surface),
            true,
            now,
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );
        update_workspace_terminal_active_idle_cache_shrink(
            Some(&second_surface),
            true,
            reset_at,
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );
        update_workspace_terminal_active_idle_cache_shrink(
            Some(&second_surface),
            true,
            before_threshold_again,
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );

        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            let host = host.borrow();
            let host = host
                .as_ref()
                .expect("surface changes before the threshold should keep the host resident");
            assert_eq!(
                host.cache_stats().previous_frame_rows,
                12,
                "changing seqno or viewport before the threshold should reset the active idle timer instead of clearing caches immediately"
            );
        });
        assert_eq!(
            active_surface_since,
            Some(reset_at),
            "when the visible surface changes the active idle timer should restart from that newer surface fingerprint"
        );
        assert!(
            !active_idle_cache_shrunk,
            "surface changes before the threshold should prevent the active idle shrink from marking itself complete"
        );
    }

    #[test]
    fn active_idle_cache_shrink_stays_disabled_when_preference_is_off() {
        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            *host.borrow_mut() = Some(TerminalRendererHost::new(
                Box::new(CacheTrackingPresenter {
                    cached_rows: 12,
                    clear_calls: 0,
                }),
                TerminalRenderMode::Bitmap,
            ));
        });

        let now = Instant::now();
        let surface = TerminalSurfaceState::from_visible_lines(
            Uuid::new_v4(),
            7,
            24,
            80,
            vec!["welcome".into()],
        );
        let mut active_surface_fingerprint = None;
        let mut active_surface_since = None;
        let mut active_idle_cache_shrunk = false;

        update_workspace_terminal_active_idle_cache_shrink(
            Some(&surface),
            false,
            now,
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );
        update_workspace_terminal_active_idle_cache_shrink(
            Some(&surface),
            false,
            now + Duration::from_millis(WORKSPACE_TERMINAL_IDLE_CACHE_SHRINK_MS + 1),
            &mut active_surface_fingerprint,
            &mut active_surface_since,
            &mut active_idle_cache_shrunk,
        );

        WORKSPACE_TERMINAL_RENDERER_HOST.with(|host| {
            let host = host.borrow();
            let host = host
                .as_ref()
                .expect("disabling active idle shrink should not release the host");
            assert_eq!(
                host.cache_stats().previous_frame_rows,
                12,
                "the active idle shrink preference should suppress cache clearing even after the idle threshold elapses"
            );
        });
        assert!(active_surface_fingerprint.is_none());
        assert!(active_surface_since.is_none());
        assert!(
            !active_idle_cache_shrunk,
            "with the preference disabled the helper should stay inert and leave its shrink marker cleared"
        );
    }

    #[test]
    fn native_cursor_blink_state_resets_visible_when_surface_seqno_changes() {
        let session_id = Uuid::new_v4();
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 7, 24, 80, vec!["$".into()]);
        surface.cursor.visible = true;
        surface.cursor.blinking = true;

        let mut blink_state = None;

        assert!(
            workspace_native_cursor_overlay_visible(&surface, &mut blink_state),
            "the first native frame for a blinking cursor should publish the overlay as visible before any blink timer ticks"
        );
        assert!(
            advance_workspace_native_cursor_blink_state(Some(&surface), &mut blink_state),
            "the blink timer should be able to toggle the current native cursor phase once the initial state is established"
        );
        assert_eq!(
            blink_state,
            Some(WorkspaceNativeCursorBlinkState {
                fingerprint: WorkspaceNativeCursorBlinkFingerprint {
                    session_id,
                    surface_seqno: 7,
                },
                visible: false,
            }),
            "after one native blink tick the stored phase should flip to hidden for the current surface fingerprint"
        );

        let mut next_surface = surface.clone();
        next_surface.seqno = 8;

        assert!(
            workspace_native_cursor_overlay_visible(&next_surface, &mut blink_state),
            "publishing a newer native surface seqno should reset the blink phase to visible so fresh terminal activity does not keep the cursor hidden"
        );
        assert_eq!(
            blink_state,
            Some(WorkspaceNativeCursorBlinkState {
                fingerprint: WorkspaceNativeCursorBlinkFingerprint {
                    session_id,
                    surface_seqno: 8,
                },
                visible: true,
            }),
            "seqno changes should replace the old blink fingerprint and restore a visible cursor phase for the new frame"
        );
    }

    #[test]
    fn native_cursor_blink_state_clears_for_hidden_or_steady_cursor() {
        let session_id = Uuid::new_v4();
        let mut blinking_surface =
            TerminalSurfaceState::from_visible_lines(session_id, 7, 24, 80, vec!["$".into()]);
        blinking_surface.cursor.visible = true;
        blinking_surface.cursor.blinking = true;
        let mut blink_state = None;
        assert!(workspace_native_cursor_overlay_visible(
            &blinking_surface,
            &mut blink_state
        ));

        let mut hidden_surface = blinking_surface.clone();
        hidden_surface.cursor.visible = false;
        assert!(
            !workspace_native_cursor_overlay_visible(&hidden_surface, &mut blink_state),
            "hidden native cursors should suppress the overlay immediately instead of reusing a stale blink phase"
        );
        assert!(
            blink_state.is_none(),
            "hidden native cursors should also clear the retained blink state so a later visible frame starts from a known phase"
        );

        assert!(workspace_native_cursor_overlay_visible(
            &blinking_surface,
            &mut blink_state
        ));
        let mut steady_surface = blinking_surface.clone();
        steady_surface.cursor.blinking = false;
        assert!(
            workspace_native_cursor_overlay_visible(&steady_surface, &mut blink_state),
            "non-blinking native cursors should remain visible rather than inheriting the animated blink phase"
        );
        assert!(
            blink_state.is_none(),
            "steady native cursors should clear the blink state because no timer-driven phase should remain armed"
        );
    }

    #[test]
    fn native_cursor_blink_state_only_toggles_for_the_matching_surface() {
        let session_id = Uuid::new_v4();
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 7, 24, 80, vec!["$".into()]);
        surface.cursor.visible = true;
        surface.cursor.blinking = true;

        let mut blink_state = None;
        assert!(workspace_native_cursor_overlay_visible(
            &surface,
            &mut blink_state
        ));
        assert!(
            advance_workspace_native_cursor_blink_state(Some(&surface), &mut blink_state),
            "matching native blink state should toggle on each timer tick"
        );
        assert_eq!(
            blink_state.as_ref().map(|state| state.visible),
            Some(false),
            "after the first matching tick the blink phase should be hidden"
        );

        let mut other_surface = surface.clone();
        other_surface.seqno += 1;
        assert!(
            !advance_workspace_native_cursor_blink_state(Some(&other_surface), &mut blink_state),
            "a timer tick that sees a newer surface fingerprint should reset state instead of toggling the stale phase"
        );
        assert_eq!(
            blink_state,
            Some(WorkspaceNativeCursorBlinkState {
                fingerprint: WorkspaceNativeCursorBlinkFingerprint {
                    session_id,
                    surface_seqno: 8,
                },
                visible: true,
            }),
            "stale native blink state should be replaced with a visible phase for the newer surface fingerprint"
        );

        assert!(
            !advance_workspace_native_cursor_blink_state(None, &mut blink_state),
            "without an active native surface the blink timer should stay inert instead of toggling an orphaned phase"
        );
        assert!(
            blink_state.is_none(),
            "losing the active native surface should clear any retained blink phase"
        );
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
            workspace_terminal::workspace_paste_prompt_mode(&ShellViewModel::default(), ""),
            None
        );
        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                "echo hello\n"
            ),
            None
        );
        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                "echo hello\r\n"
            ),
            None
        );
        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                "echo hello\nwhoami"
            ),
            Some(WorkspacePastePromptMode::Confirm)
        );
        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                "echo hello\r\nwhoami"
            ),
            Some(WorkspacePastePromptMode::Confirm)
        );
        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                "echo hello\rwhoami"
            ),
            Some(WorkspacePastePromptMode::Confirm)
        );
    }

    #[test]
    fn workspace_paste_newline_normalizer_preserves_lf_input_and_intentional_blank_lines() {
        let lf_only = "sudo apt update && \\\n  sudo apt install -y curl && \\\n  echo done\n";
        assert_eq!(
            workspace_terminal::normalize_workspace_paste_text(lf_only),
            lf_only
        );

        assert_eq!(
            workspace_terminal::normalize_workspace_paste_text("\\\r\nnext"),
            "\\\nnext"
        );
        assert!(
            !workspace_terminal::normalize_workspace_paste_text("\\\r\nnext").contains("\\\n\n")
        );
        assert_eq!(
            workspace_terminal::normalize_workspace_paste_text("echo one\r\n\r\necho two\r\n"),
            "echo one\n\necho two\n"
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
            workspace_terminal::workspace_paste_prompt_mode(&state, "echo hello\nwhoami"),
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
            workspace_terminal::workspace_paste_prompt_mode(&state, "one\ntwo\nthree\nfour"),
            Some(WorkspacePastePromptMode::Editor)
        );
    }

    #[test]
    fn workspace_large_single_line_paste_uses_editor_prompt() {
        let long_paste = "x".repeat(WORKSPACE_PASTE_EDITOR_CHAR_THRESHOLD);

        assert_eq!(
            workspace_terminal::workspace_paste_prompt_mode(
                &ShellViewModel::default(),
                long_paste.as_str()
            ),
            Some(WorkspacePastePromptMode::Editor)
        );
    }

    #[test]
    fn local_input_projection_hint_snaps_scrollback_surface_to_bottom() {
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
        let mut surface = TerminalSurfaceState::from_visible_lines(
            session_id,
            7,
            24,
            80,
            vec!["offset 3".into()],
        );
        surface.viewport_offset_lines = 3;
        surface.viewport_max_offset_lines = 8;
        surface.viewport_at_bottom = false;
        state.set_active_workspace_terminal_surface(Some(surface));

        assert!(
            workspace_terminal::apply_local_input_projection_hint(&mut state),
            "local input should immediately collapse the projected scrollback gap so the terminal stops looking stuck above the latest output"
        );

        let surface = state
            .active_workspace_terminal_surface()
            .expect("updated surface");
        assert_eq!(surface.viewport_offset_lines, 0);
        assert!(surface.viewport_at_bottom);
        assert_eq!(surface.viewport_max_offset_lines, 8);
    }

    #[test]
    fn local_input_projection_hint_leaves_bottom_aligned_surface_unchanged() {
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
        let surface =
            TerminalSurfaceState::from_visible_lines(session_id, 3, 24, 80, vec!["$ ".into()]);
        state.set_active_workspace_terminal_surface(Some(surface));

        assert!(
            !workspace_terminal::apply_local_input_projection_hint(&mut state),
            "already-bottom surfaces should not trigger extra local projection work for every repeated key event"
        );
        let surface = state
            .active_workspace_terminal_surface()
            .expect("unchanged surface");
        assert_eq!(surface.seqno, 3);
        assert_eq!(surface.viewport_offset_lines, 0);
        assert!(surface.viewport_at_bottom);
    }

    #[test]
    fn terminal_key_event_parses_function_key_names() {
        assert_eq!(
            workspace_terminal::terminal_key_event("f1", false, false, false),
            Some(TerminalKeyEvent::function(1, false, false, false))
        );
        assert_eq!(
            workspace_terminal::terminal_key_event("f12", true, false, true),
            Some(TerminalKeyEvent::function(12, true, false, true))
        );
        assert_eq!(
            workspace_terminal::terminal_key_event("f24", false, true, false),
            Some(TerminalKeyEvent::function(24, false, true, false))
        );
    }

    #[test]
    fn terminal_key_event_preserves_plain_insert_key() {
        assert_eq!(
            workspace_terminal::terminal_key_event("insert", false, false, false),
            Some(TerminalKeyEvent::named("insert", false, false, false))
        );
    }

    #[test]
    fn workspace_terminal_detects_browser_safe_http_urls_without_trailing_punctuation() {
        let session_id = Uuid::new_v4();
        let row_text = "curl https://example.com/demo?q=1), then visit http://example.org/docs.";
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 4, 120, vec![row_text.into()]);
        surface.cells = ascii_cells_for_row(0, row_text);

        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, 8),
            Some("https://example.com/demo?q=1".into())
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, 50),
            Some("http://example.org/docs".into())
        );
    }

    #[test]
    fn workspace_terminal_detects_supported_explicit_url_schemes_but_not_paths_or_colon_pairs() {
        let session_id = Uuid::new_v4();
        let row_text = "ssh://host.example:22 ftp://ftp.example.org/pub /home/wwwroot/project/file.go:123 C:\\Users\\Qi\\Downloads key: value";
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 4, 160, vec![row_text.into()]);
        surface.cells = ascii_cells_for_row(0, row_text);

        let ssh_col = row_text.find("ssh://").expect("ssh scheme") as u32;
        let ftp_col = row_text.find("ftp://").expect("ftp scheme") as u32;
        let unix_path_col = row_text.find("/home/").expect("unix path") as u32;
        let windows_path_col = row_text.find("C:\\Users").expect("windows path") as u32;
        let key_value_col = row_text.find("key:").expect("key/value") as u32;

        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, ssh_col),
            Some("ssh://host.example:22".into())
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, ftp_col),
            Some("ftp://ftp.example.org/pub".into())
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, unix_path_col),
            None
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, windows_path_col),
            None
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, key_value_col),
            None
        );
    }

    #[test]
    fn workspace_terminal_ignores_non_browser_url_schemes() {
        let session_id = Uuid::new_v4();
        let row_text = "udp://:10086 -L tcp://:10086 -F relay+tls://38.54.71.181:10087";
        let mut surface =
            TerminalSurfaceState::from_visible_lines(session_id, 1, 4, 120, vec![row_text.into()]);
        surface.cells = ascii_cells_for_row(0, row_text);

        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, 1),
            None
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, 16),
            None
        );
        assert_eq!(
            workspace_terminal::openable_url_at_surface(&surface, 0, 34),
            None
        );
    }

    #[test]
    fn workspace_session_state_preserves_link_affordance_during_surface_refresh() {
        with_bitmap_workspace_presenter_on_large_stack_for_test(|| {
            i_slint_backend_testing::init_no_event_loop();

            let window = AppWindow::new().expect("create app window");
            let session_id = Uuid::new_v4();
            let row_text = "open https://example.com/docs";
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

            let mut initial_surface = TerminalSurfaceState::from_visible_lines(
                session_id,
                1,
                24,
                80,
                vec![row_text.into()],
            );
            initial_surface.cells = ascii_cells_for_row(0, row_text);
            state.set_active_workspace_terminal_surface(Some(initial_surface));
            let mut follow_tracker = WorkspaceFollowTracker::default();

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);
            WORKSPACE_TERMINAL_POINTER_STATE.with(|pointer_state| {
                *pointer_state.borrow_mut() =
                    Some(workspace_terminal::WorkspaceTerminalPointerState {
                        session_id,
                        row: 0,
                        col: 8,
                        ctrl: true,
                    });
            });
            window.set_workspace_session_link_hovered(true);
            window.set_workspace_session_link_armed(true);

            let mut refreshed_surface = TerminalSurfaceState::from_visible_lines(
                session_id,
                2,
                24,
                80,
                vec![row_text.into()],
            );
            refreshed_surface.cells = ascii_cells_for_row(0, row_text);
            state.set_active_workspace_terminal_surface(Some(refreshed_surface));

            sync_workspace_session_state(&window, &mut state, &mut follow_tracker);

            assert!(
                window.get_workspace_session_link_hovered(),
                "surface refreshes should not drop the active terminal link hover state while the cursor is still over the same browser-openable URL"
            );
            assert!(
                window.get_workspace_session_link_armed(),
                "surface refreshes should not drop the active terminal link armed state while the cursor is still over the same browser-openable URL"
            );
            WORKSPACE_TERMINAL_POINTER_STATE.with(|pointer_state| {
                pointer_state.borrow_mut().take();
            });
        });
    }

    fn ascii_cells_for_row(row: u32, text: &str) -> Vec<TerminalCellState> {
        text.chars()
            .enumerate()
            .map(|(col, ch)| TerminalCellState {
                row,
                col: col as u32,
                width: 1,
                text: ch.to_string(),
                bold: false,
                underline: false,
                fg_rgba: 0xffff_ffff,
                bg_rgba: 0xff0d_1117,
            })
            .collect()
    }
}
