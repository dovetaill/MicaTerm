//! Central shell state mirrored into Slint properties and mutated by UI callbacks.

mod asset_modal_executor;
mod assets;
mod context_menu_dispatcher;
mod keychain;
mod projection;
mod quick_launch;
mod sftp;
mod ssh_modal;
mod validation;
mod workspace;

use self::asset_modal_executor::normalized_keychain_identity_auth_kind_id;
pub use self::asset_modal_executor::welcome_actions;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};

use crate::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainNodePayload, KeychainSshKeySpec,
};
use crate::app::quick_launch_preferences::QuickLaunchPreferences;
use crate::app::sftp::{
    FILE_BROWSER_MODIFIED_COLUMN_MIN_PX, FILE_BROWSER_SIZE_COLUMN_MIN_PX,
    FILE_BROWSER_TYPE_COLUMN_MIN_PX, FileBrowserSession, FileBrowserSortColumn,
    FileBrowserSortDirection, FileBrowserSortState, HostProfileRef, SftpDirectoryEntry,
    SftpFollowMode, SftpPanelMode, SftpSessionBindingState, TransferQueueSummary,
};
use crate::app::ssh::credentials::{
    SshCredentialKind, keychain_identity_credential_ref, keychain_key_credential_ref,
    ssh_credential_ref,
};
use crate::app::ssh::runtime::TerminalSurfaceState;
use crate::app::terminal_semantic::OutputRuleProfile;
use crate::app::ui_preferences::DownloadConflictDefault;
use crate::app::window_state::WindowPlacementKind;
use crate::shell::assets::{
    AssetNameValidation, AssetNodePayload, AssetSocks5ProxySpec, AssetSshConnectionSpec,
    AssetSshProxySpec, AssetTree, AssetViewMode, ConsoleAssetKind,
    SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY, SSH_AUTH_SOURCE_MANUAL, VisibleAssetRow,
    normalized_ssh_auth_source, resolve_committed_name,
};
use crate::shell::context_menu::{
    ContextMenuActionNode, ContextMenuActionState, ContextTargetKind, SelectionContext,
    resolve_action_tree,
};
use crate::shell::keychain::{
    KeychainDeleteError, KeychainItemKind, create_keychain_node, delete_keychain_node,
    next_default_name_for_parent as next_default_keychain_name_for_parent, project_keychain_rows,
    rename_keychain_node,
};
use crate::shell::metrics::ShellMetrics;
use crate::shell::quick_launch::{
    QUICK_LAUNCH_RECENT_LIMIT, QuickLaunchAssetRecord, QuickLaunchCardItem,
    collect_quick_launch_records, format_recent_time_label, project_connected_card_item,
    project_recent_card_item,
};
use crate::shell::sidebar::SidebarDestination;
use crate::shell::tabs::WorkspaceTab;
use crate::theme::{SearchMatchHighlightStrength, ThemeMode, ThemeVariant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    NewConnection,
    OpenRecent,
    Snippets,
    Sftp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSshPickerItem {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    pub selected: bool,
    pub focused: bool,
    pub disclosure_state: String,
    pub path_hint: String,
    pub compact_flat_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RightPanelView {
    Appearance,
    Sftp,
}

impl RightPanelView {
    pub fn id(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Sftp => "sftp",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value {
            "sftp" => Self::Sftp,
            _ => Self::Sftp,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransferCenterFilter {
    #[default]
    All,
    Running,
    Queued,
    Paused,
    Failed,
    Completed,
}

impl TransferCenterFilter {
    pub fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Running => "running",
            Self::Queued => "queued",
            Self::Paused => "paused",
            Self::Failed => "failed",
            Self::Completed => "completed",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "all" => Some(Self::All),
            "running" => Some(Self::Running),
            "queued" => Some(Self::Queued),
            "paused" => Some(Self::Paused),
            "failed" => Some(Self::Failed),
            "completed" => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn matches(self, task: &crate::app::sftp::TransferTask) -> bool {
        match self {
            Self::All => true,
            Self::Running => task.state == crate::app::sftp::TransferTaskState::Running,
            Self::Queued => task.state == crate::app::sftp::TransferTaskState::Queued,
            Self::Paused => task.state == crate::app::sftp::TransferTaskState::Paused,
            Self::Failed => task.state.needs_attention(),
            Self::Completed => task.state == crate::app::sftp::TransferTaskState::Completed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncModalMode {
    NotConfigured,
    Ready,
    SyncError,
}

impl SyncModalMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::NotConfigured => "not-configured",
            Self::Ready => "ready",
            Self::SyncError => "sync-error",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncModalViewState {
    pub open: bool,
    pub mode: SyncModalMode,
    pub title: String,
    pub headline: String,
    pub status_text: String,
    pub error_text: String,
    pub provider_label: String,
    pub target_label: String,
    pub conflict_count: i32,
    pub conflict_summary: String,
    pub conflict_review_available: bool,
    pub primary_action_label: String,
    pub secondary_action_label: String,
    pub git_remote_url: String,
    pub git_branch: String,
    pub git_auth_mode: String,
    pub git_https_username: String,
    pub git_https_secret: String,
    pub git_https_secret_visible: bool,
    pub git_ssh_private_key: String,
    pub git_ssh_passphrase: String,
    pub git_ssh_passphrase_visible: bool,
    pub master_password: String,
    pub master_password_visible: bool,
    pub local_last_sync_text: String,
    pub remote_last_update_text: String,
    pub primary_revision_text: String,
    pub remote_status_text: String,
    pub remote_status_loading: bool,
}

impl Default for SyncModalViewState {
    fn default() -> Self {
        Self {
            open: false,
            mode: SyncModalMode::NotConfigured,
            title: "Sync Settings".into(),
            headline: "Configure sync".into(),
            status_text: "Configure a Gitee Git remote to enable sync.".into(),
            error_text: String::new(),
            provider_label: "Gitee".into(),
            target_label: String::new(),
            conflict_count: 0,
            conflict_summary: String::new(),
            conflict_review_available: false,
            primary_action_label: "Save and enable".into(),
            secondary_action_label: "Close".into(),
            git_remote_url: String::new(),
            git_branch: "main".into(),
            git_auth_mode: "https".into(),
            git_https_username: String::new(),
            git_https_secret: String::new(),
            git_https_secret_visible: false,
            git_ssh_private_key: String::new(),
            git_ssh_passphrase: String::new(),
            git_ssh_passphrase_visible: false,
            master_password: String::new(),
            master_password_visible: false,
            local_last_sync_text: "Never synced".into(),
            remote_last_update_text: "Unknown".into(),
            primary_revision_text: "Unknown".into(),
            remote_status_text: String::new(),
            remote_status_loading: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SyncFeedbackViewState {
    pub text: String,
    pub sequence: i32,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferCenterFeedbackViewState {
    pub text: String,
    pub tone: String,
    pub sequence: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsModalViewState {
    pub open: bool,
    pub terminal_scrollback_limit: usize,
    pub terminal_active_idle_shrink_enabled: bool,
    pub terminal_input_highlighting_enabled: bool,
    pub terminal_output_rule_highlighting_enabled: bool,
    pub terminal_command_decorations_enabled: bool,
    pub terminal_overview_markers_enabled: bool,
    pub terminal_output_rule_profile: OutputRuleProfile,
    pub terminal_search_match_highlight: SearchMatchHighlightStrength,
    pub download_conflict_default: DownloadConflictDefault,
}

impl Default for SettingsModalViewState {
    fn default() -> Self {
        Self {
            open: false,
            terminal_scrollback_limit: 1500,
            terminal_active_idle_shrink_enabled: true,
            terminal_input_highlighting_enabled: true,
            terminal_output_rule_highlighting_enabled: true,
            terminal_command_decorations_enabled: true,
            terminal_overview_markers_enabled: false,
            terminal_output_rule_profile: OutputRuleProfile::Focused,
            terminal_search_match_highlight: SearchMatchHighlightStrength::Balanced,
            download_conflict_default: DownloadConflictDefault::Ask,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPanelViewState {
    pub title: String,
    pub primary_status_label: String,
}

impl Default for VaultPanelViewState {
    fn default() -> Self {
        Self {
            title: "Sync & Vault".into(),
            primary_status_label: "Primary not configured".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SftpConflictModalKind {
    #[default]
    Remote,
    Download,
}

impl SftpConflictModalKind {
    pub fn id(self) -> &'static str {
        match self {
            Self::Remote => "remote",
            Self::Download => "download",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SftpConflictModalState {
    pub open: bool,
    pub kind: SftpConflictModalKind,
    pub task_id: Option<String>,
    pub source_path: String,
    pub target_path: String,
    pub batch_task_ids: Vec<String>,
    pub apply_to_batch: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SftpRemoteFileEditorState {
    pub open: bool,
    pub session_id: Option<String>,
    pub remote_path: String,
    pub title: String,
    pub content: String,
    pub saved_content: String,
    pub status_text: String,
    pub error_text: String,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetModalState {
    NewFolder {
        parent_id: Option<String>,
        draft_name: String,
    },
    NewSnippet {
        parent_package_id: Option<String>,
        editing_asset_id: Option<String>,
        draft: AssetSnippetDraft,
    },
    NewSnippetPackage {
        editing_asset_id: Option<String>,
        draft_name: String,
    },
    NewKeychainIdentity {
        parent_id: Option<String>,
        editing_item_id: Option<String>,
        draft: KeychainIdentityDraft,
    },
    NewKeychainSshKey {
        parent_id: Option<String>,
        editing_item_id: Option<String>,
        draft: KeychainSshKeyDraft,
    },
    NewSshConnection {
        parent_id: Option<String>,
        editing_asset_id: Option<String>,
        draft: AssetSshConnectionDraft,
    },
    SftpNewFile {
        draft_name: String,
    },
    SftpNewFolder {
        draft_name: String,
    },
    SftpRenameEntry {
        entry_id: String,
        original_name: String,
        draft_name: String,
    },
    SftpDeleteEntriesConfirm {
        entry_ids: Vec<String>,
        label: String,
        descendant_count: usize,
    },
    RenameAsset {
        asset_id: String,
        original_name: String,
        draft_name: String,
    },
    DeleteAssetConfirm {
        asset_id: String,
        label: String,
        descendant_count: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSshConnectionDraft {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: String,
    pub auth_source: String,
    pub keychain_identity_id: String,
    pub auth_method: String,
    pub private_key_source: String,
    pub password: String,
    pub private_key_content: String,
    pub private_key_path: String,
    pub passphrase: String,
    pub password_visible: bool,
    pub passphrase_visible: bool,
    pub remark: String,
    pub environment: String,
    pub proxy_type: String,
    pub proxy_socks5_host: String,
    pub proxy_socks5_port: String,
    pub proxy_socks5_username: String,
    pub proxy_socks5_password: String,
    pub proxy_socks5_password_visible: bool,
    pub proxy_ssh_asset_id: String,
    pub proxy_method: String,
    pub validation_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetSnippetDraft {
    pub name: String,
    pub script: String,
    pub package: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeychainIdentityDraft {
    pub name: String,
    pub username: String,
    pub auth_kind: String,
    pub password: String,
    pub password_visible: bool,
    pub ssh_key_id: String,
    pub ssh_key_label: String,
    pub remark: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct KeychainSshKeyDraft {
    pub name: String,
    pub private_key: String,
    pub public_key: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SshProxyTargetOption {
    asset_id: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SshKeychainIdentityOption {
    identity_id: String,
    label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct KeychainSshKeyOption {
    key_id: String,
    label: String,
}

impl Default for AssetSshConnectionDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            user: String::new(),
            port: "22".into(),
            auth_source: SSH_AUTH_SOURCE_MANUAL.into(),
            keychain_identity_id: String::new(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            password: String::new(),
            private_key_content: String::new(),
            private_key_path: String::new(),
            passphrase: String::new(),
            password_visible: false,
            passphrase_visible: false,
            remark: String::new(),
            environment: String::new(),
            proxy_type: "none".into(),
            proxy_socks5_host: String::new(),
            proxy_socks5_port: String::new(),
            proxy_socks5_username: String::new(),
            proxy_socks5_password: String::new(),
            proxy_socks5_password_visible: false,
            proxy_ssh_asset_id: String::new(),
            proxy_method: String::new(),
            validation_message: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SshHostKeyPromptState {
    pub host: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshModalAction {
    Save,
    Connect,
    TestConnection,
    SaveAndConnect,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SshModalActionState {
    Idle,
    Busy(SshModalAction),
    Success(String),
    Error(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingSshModalAction {
    pub action: SshModalAction,
    pub draft: AssetSshConnectionDraft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetCreateAction {
    NewSnippet,
    NewPackage,
}

impl SnippetCreateAction {
    fn from_action_id(action_id: &str) -> Option<Self> {
        match action_id {
            "new-snippet" => Some(Self::NewSnippet),
            "new-snippet-package" | "new-package" => Some(Self::NewPackage),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnippetActivation {
    Paste,
    Run,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingSnippetActivation {
    snippet_id: String,
    mode: SnippetActivation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingSftpContextAction {
    OpenRemote {
        entry_id: String,
    },
    OpenLocal {
        entry_id: String,
    },
    EditLocally {
        entry_id: String,
    },
    Refresh,
    CreateFile {
        path: String,
        refresh_path: String,
    },
    CreateFolder {
        path: String,
        refresh_path: String,
    },
    RenameEntry {
        from: String,
        to: String,
        refresh_path: String,
    },
    DeleteEntries {
        entries: Vec<SftpDirectoryEntry>,
        refresh_path: String,
    },
    UploadFiles,
    UploadFolder,
    DownloadSelection {
        entry_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct QuickBrowserState {
    pub follows_active_terminal: bool,
    pub pending_terminal_session_id: Option<String>,
    pub path_editing: bool,
    pub drop_target_active: bool,
    pub sort_state: FileBrowserSortState,
    pub column_layout: crate::app::sftp::FileBrowserColumnLayout,
}

impl Default for QuickBrowserState {
    fn default() -> Self {
        Self {
            follows_active_terminal: true,
            pending_terminal_session_id: None,
            path_editing: false,
            drop_target_active: false,
            sort_state: FileBrowserSortState::default(),
            column_layout: crate::app::sftp::FileBrowserColumnLayout::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpPanelRenderRow {
    pub id: String,
    pub name: String,
    pub meta_label: String,
    pub type_label: String,
    pub modified_label: String,
    pub size_label: String,
    pub kind: String,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SftpPanelRenderCache {
    pub rows: Vec<SftpPanelRenderRow>,
    pub row_index_by_entry_id: HashMap<String, usize>,
    pub viewport_offset_px: u32,
    pub viewport_height_px: u32,
    pub window_start_row: usize,
    pub window_end_row: usize,
    pub total_content_height_px: u32,
    pub top_spacer_height_px: u32,
    pub bottom_spacer_height_px: u32,
    pub dirty_row_indices: Vec<usize>,
    pub full_resync_required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub workspace_focus_mode: bool,
    pub transfer_center_open: bool,
    pub transfer_center_pinned: bool,
    pub transfer_center_collapsed: bool,
    pub transfer_center_filter: TransferCenterFilter,
    pub right_panel_view: RightPanelView,
    pub right_panel_expanded_width: f32,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    saved_workspace_focus_assets_sidebar: bool,
    saved_workspace_focus_right_panel: bool,
    pub assets_sidebar_expanded_width: f32,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub theme_variant: ThemeVariant,
    pub is_always_on_top: bool,
    pub asset_view_mode: AssetViewMode,
    pub asset_search_expanded: bool,
    pub asset_search_query: String,
    pub keychain_search_query: String,
    pub asset_create_menu_open: bool,
    pub asset_modal_state: Option<AssetModalState>,
    pub ssh_host_key_prompt_state: Option<SshHostKeyPromptState>,
    pub asset_tree_fully_expanded: bool,
    pub selected_asset_ids: Vec<String>,
    pub focused_asset_id: Option<String>,
    pub selected_keychain_ids: Vec<String>,
    pub focused_keychain_id: Option<String>,
    quick_launch_preferences: QuickLaunchPreferences,
    saved_ssh_picker_open: bool,
    saved_ssh_picker_query: String,
    saved_ssh_picker_selected_asset_id: Option<String>,
    pub file_browser_sessions: HashMap<String, FileBrowserSession>,
    pub sftp_panel_projection_cache: HashMap<String, Vec<SftpDirectoryEntry>>,
    pub sftp_panel_render_cache: HashMap<String, SftpPanelRenderCache>,
    pub sftp_panel_last_rendered_session_id: Option<String>,
    pub quick_browser_session_id: Option<String>,
    pub quick_browser_state: QuickBrowserState,
    pub sftp_queue_summary: TransferQueueSummary,
    pub sftp_transfer_tasks: Vec<crate::app::sftp::TransferTask>,
    pub sftp_queue_drawer_open: bool,
    workspace_tabs: Vec<WorkspaceTab>,
    hidden_workspace_terminal_session_ids: HashSet<String>,
    active_workspace_tab_id: Option<String>,
    active_workspace_session_id: Option<String>,
    active_workspace_terminal_surface: Option<TerminalSurfaceState>,
    workspace_terminal_search_open: bool,
    workspace_terminal_search_query: String,
    workspace_terminal_search_focus_sequence: i32,
    pending_ssh_modal_action: Option<PendingSshModalAction>,
    pending_snippet_create_action: Option<SnippetCreateAction>,
    pending_snippet_activation: Option<PendingSnippetActivation>,
    pending_sftp_context_action: Option<PendingSftpContextAction>,
    ssh_modal_action_state: SshModalActionState,
    pub editing_asset_id: Option<String>,
    pub editing_asset_text: String,
    pub context_menu_open: bool,
    pub context_menu_target_kind: Option<ContextTargetKind>,
    pub context_target_asset_id: Option<String>,
    pub context_menu_anchor_x: f32,
    pub context_menu_anchor_y: f32,
    pub context_menu_origin_x: f32,
    pub context_menu_origin_y: f32,
    pub context_menu_child_flows_left: bool,
    pub context_menu_open_path: Vec<usize>,
    pub context_menu_feedback_text: String,
    sftp_conflict_modal_state: SftpConflictModalState,
    sftp_remote_file_editor_state: SftpRemoteFileEditorState,
    sync_modal_state: SyncModalViewState,
    settings_modal_state: SettingsModalViewState,
    sync_feedback_state: SyncFeedbackViewState,
    transfer_center_feedback_state: TransferCenterFeedbackViewState,
    vault_panel_state: VaultPanelViewState,
    console_asset_tree: AssetTree,
    snippet_asset_tree: AssetTree,
    keychain_catalog: KeychainCatalog,
    keychain_expanded_ids: BTreeSet<String>,
    window_placement: WindowPlacementKind,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            workspace_focus_mode: false,
            transfer_center_open: false,
            transfer_center_pinned: false,
            transfer_center_collapsed: false,
            transfer_center_filter: TransferCenterFilter::All,
            right_panel_view: RightPanelView::Sftp,
            right_panel_expanded_width: ShellMetrics::RIGHT_PANEL_DEFAULT_WIDTH as f32,
            show_global_menu: false,
            show_assets_sidebar: true,
            saved_workspace_focus_assets_sidebar: true,
            saved_workspace_focus_right_panel: false,
            assets_sidebar_expanded_width: ShellMetrics::ASSETS_SIDEBAR_DEFAULT_WIDTH as f32,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
            theme_variant: ThemeVariant::PremiumDefault,
            is_always_on_top: false,
            asset_view_mode: AssetViewMode::Tree,
            asset_search_expanded: false,
            asset_search_query: String::new(),
            keychain_search_query: String::new(),
            asset_create_menu_open: false,
            asset_modal_state: None,
            ssh_host_key_prompt_state: None,
            asset_tree_fully_expanded: false,
            selected_asset_ids: Vec::new(),
            focused_asset_id: None,
            selected_keychain_ids: Vec::new(),
            focused_keychain_id: None,
            quick_launch_preferences: QuickLaunchPreferences::default(),
            saved_ssh_picker_open: false,
            saved_ssh_picker_query: String::new(),
            saved_ssh_picker_selected_asset_id: None,
            file_browser_sessions: HashMap::new(),
            sftp_panel_projection_cache: HashMap::new(),
            sftp_panel_render_cache: HashMap::new(),
            sftp_panel_last_rendered_session_id: None,
            quick_browser_session_id: None,
            quick_browser_state: QuickBrowserState::default(),
            sftp_queue_summary: TransferQueueSummary::default(),
            sftp_transfer_tasks: Vec::new(),
            sftp_queue_drawer_open: false,
            workspace_tabs: Vec::new(),
            hidden_workspace_terminal_session_ids: HashSet::new(),
            active_workspace_tab_id: None,
            active_workspace_session_id: None,
            active_workspace_terminal_surface: None,
            workspace_terminal_search_open: false,
            workspace_terminal_search_query: String::new(),
            workspace_terminal_search_focus_sequence: 0,
            pending_ssh_modal_action: None,
            pending_snippet_create_action: None,
            pending_snippet_activation: None,
            pending_sftp_context_action: None,
            ssh_modal_action_state: SshModalActionState::Idle,
            editing_asset_id: None,
            editing_asset_text: String::new(),
            context_menu_open: false,
            context_menu_target_kind: None,
            context_target_asset_id: None,
            context_menu_anchor_x: 0.0,
            context_menu_anchor_y: 0.0,
            context_menu_origin_x: 0.0,
            context_menu_origin_y: 0.0,
            context_menu_child_flows_left: false,
            context_menu_open_path: Vec::new(),
            context_menu_feedback_text: String::new(),
            sftp_conflict_modal_state: SftpConflictModalState::default(),
            sftp_remote_file_editor_state: SftpRemoteFileEditorState::default(),
            sync_modal_state: SyncModalViewState::default(),
            settings_modal_state: SettingsModalViewState::default(),
            sync_feedback_state: SyncFeedbackViewState::default(),
            transfer_center_feedback_state: TransferCenterFeedbackViewState::default(),
            vault_panel_state: VaultPanelViewState::default(),
            console_asset_tree: AssetTree::new(),
            snippet_asset_tree: AssetTree::new(),
            keychain_catalog: KeychainCatalog::default(),
            keychain_expanded_ids: BTreeSet::new(),
            window_placement: WindowPlacementKind::Restored,
        }
    }
}

fn compare_sftp_panel_entries(
    left: &SftpDirectoryEntry,
    right: &SftpDirectoryEntry,
    sort_state: FileBrowserSortState,
) -> Ordering {
    sftp_directory_group(left.kind)
        .cmp(&sftp_directory_group(right.kind))
        .then_with(|| match (sort_state.column, sort_state.direction) {
            (Some(column), Some(direction)) => {
                compare_sftp_panel_column(left, right, column, direction)
            }
            _ => compare_sftp_panel_names(left, right, FileBrowserSortDirection::Asc),
        })
        .then_with(|| compare_sftp_panel_names(left, right, FileBrowserSortDirection::Asc))
        .then_with(|| left.path.cmp(&right.path))
}

fn compare_sftp_panel_column(
    left: &SftpDirectoryEntry,
    right: &SftpDirectoryEntry,
    column: FileBrowserSortColumn,
    direction: FileBrowserSortDirection,
) -> Ordering {
    match column {
        FileBrowserSortColumn::Name => compare_sftp_panel_names(left, right, direction),
        FileBrowserSortColumn::Type => compare_sftp_panel_type(left.kind, right.kind, direction),
        FileBrowserSortColumn::Modified => compare_sftp_panel_optional_u64(
            left.modified_unix_seconds,
            right.modified_unix_seconds,
            direction,
        ),
        FileBrowserSortColumn::Size => {
            compare_sftp_panel_optional_u64(left.size_bytes, right.size_bytes, direction)
        }
    }
}

fn compare_sftp_panel_names(
    left: &SftpDirectoryEntry,
    right: &SftpDirectoryEntry,
    direction: FileBrowserSortDirection,
) -> Ordering {
    compare_sftp_panel_text(left.name.as_str(), right.name.as_str(), direction)
}

fn compare_sftp_panel_type(
    left: crate::app::sftp::SftpDirectoryEntryKind,
    right: crate::app::sftp::SftpDirectoryEntryKind,
    direction: FileBrowserSortDirection,
) -> Ordering {
    let ordering = sftp_kind_rank(left).cmp(&sftp_kind_rank(right));
    match direction {
        FileBrowserSortDirection::Asc => ordering,
        FileBrowserSortDirection::Desc => ordering.reverse(),
    }
}

fn compare_sftp_panel_optional_u64(
    left: Option<u64>,
    right: Option<u64>,
    direction: FileBrowserSortDirection,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => match direction {
            FileBrowserSortDirection::Asc => left.cmp(&right),
            FileBrowserSortDirection::Desc => right.cmp(&left),
        },
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_sftp_panel_text(
    left: &str,
    right: &str,
    direction: FileBrowserSortDirection,
) -> Ordering {
    let ordering = left
        .to_ascii_lowercase()
        .cmp(&right.to_ascii_lowercase())
        .then_with(|| left.cmp(right));
    match direction {
        FileBrowserSortDirection::Asc => ordering,
        FileBrowserSortDirection::Desc => ordering.reverse(),
    }
}

fn sftp_directory_group(kind: crate::app::sftp::SftpDirectoryEntryKind) -> u8 {
    match kind {
        crate::app::sftp::SftpDirectoryEntryKind::Directory => 0,
        _ => 1,
    }
}

fn sftp_kind_rank(kind: crate::app::sftp::SftpDirectoryEntryKind) -> u8 {
    match kind {
        crate::app::sftp::SftpDirectoryEntryKind::Directory => 0,
        crate::app::sftp::SftpDirectoryEntryKind::File => 1,
        crate::app::sftp::SftpDirectoryEntryKind::Symlink => 2,
        crate::app::sftp::SftpDirectoryEntryKind::Unknown => 3,
    }
}

impl ShellViewModel {
    pub fn open_appearance_panel(&mut self) {
        self.open_sftp_panel();
    }

    pub fn pending_ssh_modal_action(&self) -> Option<&PendingSshModalAction> {
        self.pending_ssh_modal_action.as_ref()
    }

    pub fn take_pending_ssh_modal_action(&mut self) -> Option<PendingSshModalAction> {
        self.pending_ssh_modal_action.take()
    }

    pub fn ssh_modal_action_state(&self) -> &SshModalActionState {
        &self.ssh_modal_action_state
    }

    pub fn ssh_modal_feedback_state_id(&self) -> &'static str {
        match self.ssh_modal_action_state {
            SshModalActionState::Idle => "idle",
            SshModalActionState::Busy(_) => "busy",
            SshModalActionState::Success(_) => "success",
            SshModalActionState::Error(_) => "error",
        }
    }

    pub fn ssh_modal_feedback_message(&self) -> String {
        match &self.ssh_modal_action_state {
            SshModalActionState::Idle => String::new(),
            SshModalActionState::Busy(action) => match action {
                SshModalAction::Save => "Saving connection...".into(),
                SshModalAction::TestConnection => "Testing connection...".into(),
                SshModalAction::Connect => "Opening temporary session...".into(),
                SshModalAction::SaveAndConnect => "Saving connection and opening session...".into(),
            },
            SshModalActionState::Success(message) | SshModalActionState::Error(message) => {
                message.clone()
            }
        }
    }

    pub fn ssh_modal_connect_family_enabled(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => {
                self.ssh_modal_submit_validation_message(
                    parent_id.as_deref(),
                    editing_asset_id.as_deref(),
                    draft,
                )
                .is_none()
                    && !self.ssh_modal_is_busy()
            }
            _ => false,
        }
    }

    pub fn ssh_modal_save_enabled(&self) -> bool {
        matches!(
            self.asset_modal_state,
            Some(AssetModalState::NewSshConnection { .. })
        ) && self.asset_create_modal_can_confirm()
            && !self.ssh_modal_is_busy()
    }

    pub fn finish_ssh_modal_action_success(&mut self, message: impl Into<String>) {
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Success(message.into());
    }

    pub fn finish_ssh_modal_action_error(&mut self, message: impl Into<String>) {
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Error(message.into());
    }

    pub fn cancel_asset_modal(&mut self) {
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        self.asset_modal_state = None;
        self.context_target_asset_id = None;
    }

    pub fn open_ssh_host_key_prompt(
        &mut self,
        host: impl Into<String>,
        fingerprint: impl Into<String>,
    ) {
        self.close_context_menu();
        self.close_asset_create_menu();
        self.ssh_host_key_prompt_state = Some(SshHostKeyPromptState {
            host: host.into(),
            fingerprint: fingerprint.into(),
        });
    }

    pub fn accept_ssh_host_key_prompt(&mut self) -> bool {
        self.clear_ssh_host_key_prompt()
    }

    pub fn reject_ssh_host_key_prompt(&mut self) -> bool {
        self.clear_ssh_host_key_prompt()
    }

    pub fn handle_assets_create_action(&mut self, action_id: &str) {
        match action_id {
            "new-folder" if self.active_sidebar_destination == SidebarDestination::Keychain => {
                self.create_keychain_item(None, KeychainItemKind::Folder);
            }
            "new-identity" => {
                self.open_new_keychain_identity_modal(None);
            }
            "new-ssh-key" => {
                self.open_new_keychain_ssh_key_modal(None);
            }
            "new-folder" => self.open_new_folder_modal(None),
            "new-ssh-connection" => self.open_new_ssh_modal(None),
            _ => {}
        }
    }

    pub fn handle_snippet_create_action(&mut self, action_id: &str) {
        let Some(action) = SnippetCreateAction::from_action_id(action_id) else {
            return;
        };

        self.pending_snippet_create_action = Some(action);
        self.close_context_menu();
        self.close_asset_create_menu();
    }

    pub fn pending_snippet_create_action(&self) -> Option<SnippetCreateAction> {
        self.pending_snippet_create_action
    }

    pub fn take_pending_snippet_create_action(&mut self) -> Option<SnippetCreateAction> {
        self.pending_snippet_create_action.take()
    }

    pub fn begin_snippet_activation(&mut self, snippet_id: &str, mode: SnippetActivation) {
        if self.snippet_asset_tree.kind(snippet_id) != Some(ConsoleAssetKind::Snippet) {
            return;
        }

        self.pending_snippet_activation = Some(PendingSnippetActivation {
            snippet_id: snippet_id.to_string(),
            mode,
        });
    }

    pub fn pending_snippet_activation(&self) -> Option<SnippetActivation> {
        self.pending_snippet_activation
            .as_ref()
            .map(|pending| pending.mode)
    }

    pub fn take_pending_snippet_activation(&mut self) -> Option<(String, SnippetActivation)> {
        self.pending_snippet_activation
            .take()
            .map(|pending| (pending.snippet_id, pending.mode))
    }

    pub fn snippet_script(&self, snippet_id: &str) -> Option<&str> {
        self.snippet_asset_tree
            .snippet_spec(snippet_id)
            .map(|spec| spec.script.as_str())
    }

    pub fn snippet_package_option_labels(&self) -> Vec<String> {
        self.snippet_asset_tree
            .root_ids()
            .iter()
            .filter(|asset_id| {
                self.snippet_asset_tree.kind(asset_id.as_str())
                    == Some(ConsoleAssetKind::SnippetPackage)
            })
            .filter_map(|asset_id| self.snippet_asset_tree.title(asset_id.as_str()))
            .map(ToString::to_string)
            .collect()
    }

    pub fn begin_asset_rename_session(&mut self, asset_id: String, initial_text: String) {
        if !self.console_asset_tree.contains(&asset_id) {
            return;
        }

        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.editing_asset_id = Some(asset_id);
        self.editing_asset_text = initial_text;
    }

    pub fn update_active_asset_rename_draft(&mut self, text: String) {
        if self.editing_asset_id.is_some() {
            self.editing_asset_text = text;
        }
    }

    pub fn commit_active_asset_rename(&mut self) {
        let Some(asset_id) = self.editing_asset_id.clone() else {
            return;
        };

        let Some(kind) = self.console_asset_tree.kind(&asset_id) else {
            self.clear_active_asset_rename_session();
            return;
        };
        let parent_id = self.console_asset_tree.parent_id(&asset_id).flatten();
        let sibling_items = self
            .console_asset_tree
            .sibling_items_for_parent(parent_id, Some(asset_id.as_str()));
        let next_label = resolve_committed_name(kind, &self.editing_asset_text, &sibling_items);

        self.console_asset_tree.set_title(&asset_id, next_label);
        self.clear_active_asset_rename_session();
    }

    pub fn cancel_active_asset_rename(&mut self) {
        self.clear_active_asset_rename_session();
    }

    pub fn dismiss_active_asset_rename(&mut self) {
        self.commit_active_asset_rename();
    }

    pub fn update_asset_rename_draft(&mut self, asset_id: &str, text: String) {
        if self.editing_asset_id.as_deref() == Some(asset_id) {
            self.update_active_asset_rename_draft(text);
        }
    }

    pub fn commit_asset_rename(&mut self, asset_id: &str, text: String) {
        if self.editing_asset_id.as_deref() == Some(asset_id) {
            self.update_active_asset_rename_draft(text);
            self.commit_active_asset_rename();
        }
    }

    pub fn cancel_asset_rename(&mut self, asset_id: &str) {
        if self.editing_asset_id.as_deref() == Some(asset_id) {
            self.cancel_active_asset_rename();
        }
    }

    pub fn handle_blank_area_click(&mut self) {
        self.commit_active_asset_rename();
        self.selected_asset_ids.clear();
        self.focused_asset_id = None;
        self.close_context_menu();
        self.context_target_asset_id = None;
    }

    pub fn remove_asset_subtree(&mut self, asset_id: &str) -> bool {
        let next_focus_target = self.next_focus_target_after_removal(asset_id);
        let Some(removed_summary) = self.console_asset_tree.remove_subtree(asset_id) else {
            return false;
        };

        let removed_ids = removed_summary.removed_ids;
        self.selected_asset_ids.retain(|selected_id| {
            !removed_ids
                .iter()
                .any(|removed_id| removed_id == selected_id)
        });

        if self.editing_asset_id.as_deref().is_some_and(|editing_id| {
            removed_ids
                .iter()
                .any(|removed_id| removed_id == editing_id)
        }) {
            self.clear_active_asset_rename_session();
        }

        if let Some(next_focus_target) = next_focus_target {
            self.focused_asset_id = Some(next_focus_target.clone());
            self.selected_asset_ids = vec![next_focus_target.clone()];
            self.context_target_asset_id = Some(next_focus_target);
        } else {
            self.focused_asset_id = None;
            self.selected_asset_ids.clear();
            self.context_target_asset_id = None;
        }

        true
    }

    pub fn select_asset(&mut self, asset_id: &str) {
        let exists = match self.active_sidebar_destination {
            SidebarDestination::Snippets => self.snippet_asset_tree.contains(asset_id),
            SidebarDestination::Console | SidebarDestination::Keychain => {
                self.console_asset_tree.contains(asset_id)
            }
        };
        if !exists {
            return;
        }

        self.selected_asset_ids = vec![asset_id.to_string()];
        self.focused_asset_id = Some(asset_id.to_string());
        self.context_target_asset_id = Some(asset_id.to_string());
        self.asset_create_menu_open = false;
    }

    pub fn toggle_folder_expanded(&mut self, asset_id: &str) {
        let Some(kind) = self.asset_kind(asset_id) else {
            return;
        };

        let next = match kind {
            ConsoleAssetKind::Folder => {
                let next = !self
                    .console_asset_tree
                    .is_expanded(asset_id)
                    .unwrap_or(false);
                self.console_asset_tree.set_expanded(asset_id, next);
                next
            }
            ConsoleAssetKind::SnippetPackage => {
                let next = !self
                    .snippet_asset_tree
                    .is_expanded(asset_id)
                    .unwrap_or(false);
                self.snippet_asset_tree.set_expanded(asset_id, next);
                next
            }
            ConsoleAssetKind::SshConnection | ConsoleAssetKind::Snippet => return,
        };
        self.asset_tree_fully_expanded = self.asset_tree_fully_expanded && next;
    }

    pub fn open_context_menu_for_target(
        &mut self,
        target_kind: ContextTargetKind,
        target_id: Option<String>,
        anchor_x: f32,
        anchor_y: f32,
    ) {
        let target_kind =
            self.resolve_context_target_kind_for_selection(target_kind, target_id.as_deref());
        self.context_menu_open = true;
        self.context_menu_target_kind = Some(target_kind);
        self.context_target_asset_id = target_id.clone();
        self.context_menu_anchor_x = anchor_x;
        self.context_menu_anchor_y = anchor_y;
        self.context_menu_origin_x = anchor_x;
        self.context_menu_origin_y = anchor_y;
        self.context_menu_child_flows_left = false;
        self.context_menu_open_path.clear();
        self.context_menu_feedback_text.clear();
        self.asset_create_menu_open = false;

        if is_sftp_context_target(target_kind) {
            match target_id.clone() {
                Some(target_id)
                    if matches!(target_kind, ContextTargetKind::SftpMultiSelection)
                        && self.active_sftp_session_state().is_some_and(|state| {
                            state
                                .selected_entry_ids
                                .iter()
                                .any(|selected_id| selected_id == &target_id)
                        }) => {}
                Some(target_id) => {
                    if let Some(state) = self.active_sftp_session_state_mut() {
                        state.selected_entry_ids = vec![target_id.clone()];
                    }
                }
                None => {
                    if let Some(state) = self.active_sftp_session_state_mut() {
                        state.selected_entry_ids.clear();
                    }
                }
            }
            return;
        }

        if is_keychain_context_target(target_kind) {
            match target_id {
                Some(target_id) => {
                    if self.selected_keychain_ids.is_empty()
                        || !self.selected_keychain_ids.iter().any(|id| id == &target_id)
                    {
                        self.selected_keychain_ids = vec![target_id.clone()];
                    }
                    self.focused_keychain_id = Some(target_id);
                }
                None => {
                    self.selected_keychain_ids.clear();
                    self.focused_keychain_id = None;
                }
            }
            self.selected_asset_ids.clear();
            self.focused_asset_id = None;
            return;
        }

        match target_id {
            Some(target_id) => {
                if self.selected_asset_ids.is_empty()
                    || !self.selected_asset_ids.iter().any(|id| id == &target_id)
                {
                    self.selected_asset_ids = vec![target_id.clone()];
                }
                self.focused_asset_id = Some(target_id);
            }
            None => {
                self.selected_asset_ids.clear();
                self.focused_asset_id = None;
            }
        }
    }

    pub fn close_context_menu(&mut self) {
        self.context_menu_open = false;
        self.context_menu_target_kind = None;
        self.context_menu_origin_x = 0.0;
        self.context_menu_origin_y = 0.0;
        self.context_menu_child_flows_left = false;
        self.context_menu_open_path.clear();
        self.context_menu_feedback_text.clear();
    }

    pub fn set_context_menu_open_path(&mut self, path: Vec<usize>) {
        self.context_menu_open_path = path;
    }

    pub fn hover_context_menu_path(&mut self, path: Vec<usize>) {
        self.context_menu_open_path = path;
    }

    pub fn truncate_context_menu_open_path(&mut self, len: usize) {
        self.context_menu_open_path.truncate(len);
    }

    pub fn handle_context_menu_escape(&mut self) {
        self.close_context_menu();
    }

    pub fn navigate_context_menu_left(&mut self) {
        self.context_menu_open_path.pop();
    }

    pub fn navigate_context_menu_right(&mut self) {
        let roots = self.context_menu_roots();

        if self.context_menu_open_path.is_empty() {
            if let Some(index) = roots.iter().position(|node| !node.children.is_empty()) {
                self.context_menu_open_path = vec![index];
            }
            return;
        }

        let Some(current) = action_node_at_path(&roots, &self.context_menu_open_path) else {
            return;
        };

        if current
            .children
            .iter()
            .any(|node| !node.children.is_empty())
        {
            self.context_menu_open_path.push(0);
        }
    }

    pub fn invoke_current_context_menu_item(&mut self) {
        let roots = self.context_menu_roots();
        let Some(current) = current_action_node(&roots, &self.context_menu_open_path) else {
            return;
        };

        if current.state == ContextMenuActionState::Disabled {
            self.handle_context_menu_leaf_action(current.id);
            return;
        }

        if current.children.is_empty() {
            self.handle_context_menu_leaf_action(current.id);
        } else if current.state == ContextMenuActionState::Planned {
            self.set_context_menu_feedback(format!("{} is not wired yet.", current.label));
        }
    }

    pub fn set_context_menu_feedback(&mut self, text: impl Into<String>) {
        self.context_menu_feedback_text = text.into();
    }

    pub fn set_context_menu_placement(
        &mut self,
        origin_x: f32,
        origin_y: f32,
        child_flows_left: bool,
    ) {
        self.context_menu_origin_x = origin_x;
        self.context_menu_origin_y = origin_y;
        self.context_menu_child_flows_left = child_flows_left;
    }

    pub fn ssh_proxy_target_option_labels(&self) -> Vec<String> {
        self.ssh_proxy_target_options()
            .into_iter()
            .map(|option| option.label)
            .collect()
    }

    pub fn ssh_keychain_identity_option_labels(&self) -> Vec<String> {
        self.ssh_keychain_identity_options()
            .into_iter()
            .map(|option| option.label)
            .collect()
    }

    pub fn ssh_keychain_identity_selected_label(&self) -> String {
        let Some(AssetModalState::NewSshConnection { draft, .. }) = &self.asset_modal_state else {
            return String::new();
        };

        self.ssh_keychain_identity_options()
            .into_iter()
            .find(|option| option.identity_id == draft.keychain_identity_id.trim())
            .map(|option| option.label)
            .unwrap_or_default()
    }

    pub fn ssh_keychain_identity_selected_username(&self) -> String {
        let Some(identity) = self.selected_ssh_keychain_identity() else {
            return String::new();
        };
        identity.username.clone()
    }

    pub fn ssh_keychain_identity_selected_auth_summary(&self) -> String {
        let Some(identity) = self.selected_ssh_keychain_identity() else {
            return String::new();
        };

        match identity.auth_kind {
            KeychainIdentityAuthKind::Password => "Password".into(),
            KeychainIdentityAuthKind::SshKey => {
                let key_title = identity
                    .ssh_key_id
                    .as_deref()
                    .and_then(|key_id| self.keychain_catalog.nodes.get(key_id))
                    .map(|node| node.title.clone())
                    .filter(|title| !title.trim().is_empty());
                match key_title {
                    Some(title) => format!("SSH Key · {title}"),
                    None => "SSH Key".into(),
                }
            }
        }
    }

    pub fn ssh_proxy_target_selected_label(&self) -> String {
        let Some(AssetModalState::NewSshConnection { draft, .. }) = &self.asset_modal_state else {
            return String::new();
        };

        self.ssh_proxy_target_options()
            .into_iter()
            .find(|option| option.asset_id == draft.proxy_ssh_asset_id.trim())
            .map(|option| option.label)
            .unwrap_or_default()
    }

    fn ssh_proxy_target_options(&self) -> Vec<SshProxyTargetOption> {
        let editing_asset_id = match &self.asset_modal_state {
            Some(AssetModalState::NewSshConnection {
                editing_asset_id: Some(asset_id),
                ..
            }) => Some(asset_id.as_str()),
            _ => None,
        };
        ssh_proxy_target_options_for_tree(&self.console_asset_tree, editing_asset_id)
    }

    fn ssh_keychain_identity_options(&self) -> Vec<SshKeychainIdentityOption> {
        ssh_keychain_identity_options_for_catalog(&self.keychain_catalog)
    }

    fn keychain_ssh_key_options(&self) -> Vec<KeychainSshKeyOption> {
        keychain_ssh_key_options_for_catalog(&self.keychain_catalog)
    }

    fn selected_ssh_keychain_identity(
        &self,
    ) -> Option<&crate::app::keychain::KeychainIdentitySpec> {
        let Some(AssetModalState::NewSshConnection { draft, .. }) = &self.asset_modal_state else {
            return None;
        };
        let identity_id = draft.keychain_identity_id.trim();
        match self
            .keychain_catalog
            .nodes
            .get(identity_id)
            .map(|node| &node.payload)
        {
            Some(KeychainNodePayload::Identity(identity)) => Some(identity),
            _ => None,
        }
    }

    fn resolve_ssh_proxy_target_asset_id_from_label(&self, label: &str) -> Option<String> {
        self.ssh_proxy_target_options()
            .into_iter()
            .find(|option| option.label == label.trim())
            .map(|option| option.asset_id)
    }

    fn resolve_ssh_keychain_identity_id_from_label(&self, label: &str) -> Option<String> {
        self.ssh_keychain_identity_options()
            .into_iter()
            .find(|option| option.label == label.trim())
            .map(|option| option.identity_id)
    }

    pub fn context_menu_selection(&self) -> SelectionContext {
        if self
            .context_menu_target_kind
            .is_some_and(is_sftp_context_target)
        {
            let (selected_file_count, selected_directory_count) = self
                .active_sftp_session_state()
                .map(|state| {
                    state
                        .entries
                        .iter()
                        .filter(|entry| {
                            state
                                .selected_entry_ids
                                .iter()
                                .any(|selected_id| selected_id == &entry.id)
                        })
                        .fold((0usize, 0usize), |(files, directories), entry| {
                            if entry.kind == crate::app::sftp::SftpDirectoryEntryKind::Directory {
                                (files, directories + 1)
                            } else {
                                (files + 1, directories)
                            }
                        })
                })
                .unwrap_or((0, 0));
            return SelectionContext {
                selected_ids: self.sftp_panel_selected_entry_ids().to_vec(),
                clipboard_has_asset_payload: false,
                target_mutable: matches!(self.sftp_panel_mode_id(), "ready"),
                selected_file_count,
                selected_directory_count,
            };
        }

        if self
            .context_menu_target_kind
            .is_some_and(is_keychain_context_target)
        {
            return SelectionContext {
                selected_ids: self.selected_keychain_ids.clone(),
                clipboard_has_asset_payload: false,
                target_mutable: true,
                selected_file_count: 0,
                selected_directory_count: 0,
            };
        }

        SelectionContext {
            selected_ids: self.selected_asset_ids.clone(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        }
    }

    fn resolve_context_target_kind_for_selection(
        &self,
        target_kind: ContextTargetKind,
        target_id: Option<&str>,
    ) -> ContextTargetKind {
        if !is_sftp_context_target(target_kind) {
            return target_kind;
        }

        let Some(target_id) = target_id else {
            return target_kind;
        };
        let Some(state) = self.active_sftp_session_state() else {
            return target_kind;
        };

        if state.selected_entry_ids.len() > 1
            && state
                .selected_entry_ids
                .iter()
                .any(|selected_id| selected_id == target_id)
        {
            ContextTargetKind::SftpMultiSelection
        } else {
            target_kind
        }
    }

    fn next_focus_target_after_removal(&self, removed_root_id: &str) -> Option<String> {
        let rows = self.visible_console_asset_rows();
        let removed_index = rows.iter().position(|row| row.id == removed_root_id)?;
        let removed_parent_id = self.console_asset_tree.parent_id(removed_root_id).flatten();

        rows.iter()
            .skip(removed_index + 1)
            .find(|row| self.console_asset_tree.parent_id(&row.id).flatten() == removed_parent_id)
            .map(|row| row.id.clone())
            .or_else(|| {
                rows[..removed_index]
                    .iter()
                    .rev()
                    .find(|row| {
                        self.console_asset_tree.parent_id(&row.id).flatten() == removed_parent_id
                    })
                    .map(|row| row.id.clone())
            })
            .or_else(|| removed_parent_id.map(ToOwned::to_owned))
    }

    fn clear_active_asset_rename_session(&mut self) {
        self.editing_asset_id = None;
        self.editing_asset_text.clear();
    }

    fn clear_ssh_host_key_prompt(&mut self) -> bool {
        self.ssh_host_key_prompt_state.take().is_some()
    }

    fn normalize_workspace_tabs(&mut self) {
        let active_tab_id = self
            .active_workspace_tab_id
            .as_deref()
            .filter(|candidate| {
                self.workspace_tabs
                    .iter()
                    .any(|tab| tab.tab_id == *candidate)
            })
            .map(str::to_string)
            .or_else(|| {
                self.workspace_tabs
                    .iter()
                    .find(|tab| tab.active)
                    .map(|tab| tab.tab_id.clone())
            })
            .or_else(|| self.workspace_tabs.first().map(|tab| tab.tab_id.clone()));

        for tab in &mut self.workspace_tabs {
            tab.active = active_tab_id.as_deref() == Some(tab.tab_id.as_str());
        }

        self.active_workspace_tab_id = active_tab_id.clone();
        self.active_workspace_session_id = active_tab_id
            .as_deref()
            .and_then(|active_id| {
                self.workspace_tabs
                    .iter()
                    .find(|tab| tab.tab_id == active_id)
                    .map(|tab| tab.session_id.clone())
            })
            .filter(|session_id| !session_id.is_empty());
        if self.active_workspace_terminal_surface().is_none() {
            self.active_workspace_terminal_surface = None;
        }
        self.show_welcome = self.workspace_tabs.is_empty();
        let next_summary = crate::app::sftp::TransferQueueSummary::from_tasks(
            &self.sftp_transfer_tasks,
            self.active_workspace_terminal_session_id(),
        );
        self.sftp_queue_summary = next_summary;
    }

    fn normalize_folder_parent_id(&self, parent_id: Option<String>) -> Option<String> {
        parent_id.filter(|asset_id| {
            self.console_asset_tree.kind(asset_id.as_str()) == Some(ConsoleAssetKind::Folder)
        })
    }

    fn normalize_snippet_package_parent_id(&self, parent_id: Option<String>) -> Option<String> {
        parent_id.filter(|asset_id| {
            self.snippet_asset_tree.kind(asset_id.as_str())
                == Some(ConsoleAssetKind::SnippetPackage)
        })
    }

    fn resolve_snippet_package_id_by_label(&self, label: &str) -> Option<String> {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            return None;
        }

        self.snippet_asset_tree
            .root_ids()
            .iter()
            .find(|asset_id| {
                self.snippet_asset_tree.kind(asset_id.as_str())
                    == Some(ConsoleAssetKind::SnippetPackage)
                    && self.snippet_asset_tree.title(asset_id.as_str()) == Some(trimmed)
            })
            .cloned()
    }

    fn normalize_keychain_folder_parent_id(&self, parent_id: Option<String>) -> Option<String> {
        parent_id.filter(|node_id| {
            self.keychain_catalog
                .nodes
                .get(node_id)
                .is_some_and(|node| {
                    matches!(
                        node.payload,
                        crate::app::keychain::KeychainNodePayload::Folder
                    )
                })
        })
    }
}

fn asset_name_validation_message(validation: AssetNameValidation) -> String {
    match validation {
        AssetNameValidation::Valid => String::new(),
        AssetNameValidation::Empty => "Name is required.".into(),
        AssetNameValidation::Duplicate => "Name already exists in this folder.".into(),
    }
}

fn is_sftp_context_target(target_kind: ContextTargetKind) -> bool {
    matches!(
        target_kind,
        ContextTargetKind::SftpBlankArea
            | ContextTargetKind::SftpDirectory
            | ContextTargetKind::SftpFile
            | ContextTargetKind::SftpMultiSelection
    )
}

fn is_keychain_context_target(target_kind: ContextTargetKind) -> bool {
    matches!(
        target_kind,
        ContextTargetKind::KeychainBlankArea
            | ContextTargetKind::KeychainFolder
            | ContextTargetKind::KeychainIdentity
            | ContextTargetKind::KeychainSshKey
    )
}

fn sftp_child_path(parent: &str, name: &str) -> String {
    let trimmed_name = name.trim();
    let trimmed_parent = parent.trim().trim_end_matches('/');
    if trimmed_parent.is_empty() || trimmed_parent == "/" {
        format!("/{trimmed_name}")
    } else {
        format!("{trimmed_parent}/{trimmed_name}")
    }
}

impl ShellViewModel {
    fn next_default_sftp_file_name(&self) -> String {
        let Some(state) = self.active_sftp_session_state() else {
            return "new-file.txt".into();
        };

        let mut candidate_index = 1usize;
        loop {
            let candidate = if candidate_index == 1 {
                "new-file.txt".to_string()
            } else {
                format!("new-file-{candidate_index}.txt")
            };
            if state.entries.iter().all(|entry| entry.name != candidate) {
                return candidate;
            }
            candidate_index += 1;
        }
    }

    fn next_default_sftp_folder_name(&self) -> String {
        let Some(state) = self.active_sftp_session_state() else {
            return "New Folder".into();
        };

        let mut candidate_index = 1usize;
        loop {
            let candidate = if candidate_index == 1 {
                "New Folder".to_string()
            } else {
                format!("New Folder {candidate_index}")
            };
            if state.entries.iter().all(|entry| entry.name != candidate) {
                return candidate;
            }
            candidate_index += 1;
        }
    }

    fn snippet_modal_validation_message(
        &self,
        parent_package_id: Option<&str>,
        editing_asset_id: Option<&str>,
        draft: &AssetSnippetDraft,
    ) -> String {
        let resolved_parent_id =
            match self.resolve_snippet_package_id_by_label(draft.package.trim()) {
                Some(parent_id) => Some(parent_id),
                None if draft.package.trim().is_empty() => None,
                None if parent_package_id.is_some() => parent_package_id.map(ToOwned::to_owned),
                None => return "Package does not exist.".into(),
            };

        let name_message =
            asset_name_validation_message(self.snippet_asset_tree.validate_name_in_parent(
                resolved_parent_id.as_deref(),
                &draft.name,
                editing_asset_id,
            ));
        if !name_message.is_empty() {
            return name_message;
        }
        if draft.script.trim().is_empty() {
            return "Script is required.".into();
        }

        String::new()
    }

    fn keychain_name_validation(
        &self,
        parent_id: Option<&str>,
        candidate: &str,
        exclude_id: Option<&str>,
    ) -> AssetNameValidation {
        let trimmed = candidate.trim();
        if trimmed.is_empty() {
            return AssetNameValidation::Empty;
        }

        let sibling_ids = parent_id
            .and_then(|parent_id| {
                self.keychain_catalog
                    .nodes
                    .get(parent_id)
                    .map(|node| node.child_ids.as_slice())
            })
            .unwrap_or(self.keychain_catalog.root_ids.as_slice());
        if sibling_ids
            .iter()
            .filter(|node_id| Some(node_id.as_str()) != exclude_id)
            .any(|node_id| {
                self.keychain_catalog
                    .nodes
                    .get(node_id)
                    .is_some_and(|node| node.title.trim() == trimmed)
            })
        {
            AssetNameValidation::Duplicate
        } else {
            AssetNameValidation::Valid
        }
    }

    fn keychain_descendant_count(&self, node_id: &str) -> Option<usize> {
        let node = self.keychain_catalog.nodes.get(node_id)?;
        let mut count = 0;
        for child_id in &node.child_ids {
            count += 1;
            count += self.keychain_descendant_count(child_id).unwrap_or_default();
        }
        Some(count)
    }

    fn keychain_ssh_key_modal_validation_message(
        &self,
        parent_id: Option<&str>,
        editing_item_id: Option<&str>,
        draft: &KeychainSshKeyDraft,
    ) -> String {
        asset_name_validation_message(self.keychain_name_validation(
            parent_id,
            &draft.name,
            editing_item_id,
        ))
    }

    fn keychain_identity_modal_validation_message(
        &self,
        parent_id: Option<&str>,
        editing_item_id: Option<&str>,
        draft: &KeychainIdentityDraft,
    ) -> String {
        let name_message = asset_name_validation_message(self.keychain_name_validation(
            parent_id,
            &draft.name,
            editing_item_id,
        ));
        if !name_message.is_empty() {
            return name_message;
        }

        if draft.username.trim().is_empty() {
            return "Username is required.".into();
        }

        match normalized_keychain_identity_auth_kind_id(draft.auth_kind.as_str()) {
            "password" => {
                if draft.password.trim().is_empty() {
                    return "Password is required.".into();
                }
            }
            "ssh-key" => {
                if draft.ssh_key_id.trim().is_empty() {
                    return "SSH key selection is required.".into();
                }
                let Some(node) = self.keychain_catalog.nodes.get(draft.ssh_key_id.trim()) else {
                    return "Selected SSH key was not found.".into();
                };
                let KeychainNodePayload::SshKey(spec) = &node.payload else {
                    return "Selected SSH key was not found.".into();
                };
                if spec
                    .credential_ref
                    .as_deref()
                    .is_none_or(|value| value.trim().is_empty())
                {
                    return "Selected SSH key must include private key material.".into();
                }
            }
            _ => {}
        }

        String::new()
    }

    fn keychain_identity_modal_can_confirm(
        &self,
        parent_id: Option<&str>,
        editing_item_id: Option<&str>,
        draft: &KeychainIdentityDraft,
    ) -> bool {
        self.keychain_identity_modal_validation_message(parent_id, editing_item_id, draft)
            .is_empty()
    }

    fn keychain_ssh_key_modal_can_confirm(
        &self,
        parent_id: Option<&str>,
        editing_item_id: Option<&str>,
        draft: &KeychainSshKeyDraft,
    ) -> bool {
        self.keychain_name_validation(parent_id, &draft.name, editing_item_id)
            == AssetNameValidation::Valid
    }

    fn ssh_modal_validation_message(
        &self,
        parent_id: Option<&str>,
        editing_asset_id: Option<&str>,
        draft: &AssetSshConnectionDraft,
    ) -> String {
        let name_message =
            asset_name_validation_message(self.console_asset_tree.validate_name_in_parent(
                parent_id,
                &draft.name,
                editing_asset_id,
            ));
        if !name_message.is_empty() {
            return name_message;
        }

        if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY
            && draft.keychain_identity_id.trim().is_empty()
        {
            return "Keychain identity is required.".into();
        }

        draft.validation_message.clone()
    }

    fn ssh_modal_can_confirm(
        &self,
        parent_id: Option<&str>,
        editing_asset_id: Option<&str>,
        draft: &AssetSshConnectionDraft,
    ) -> bool {
        self.console_asset_tree
            .validate_name_in_parent(parent_id, &draft.name, editing_asset_id)
            == AssetNameValidation::Valid
            && self
                .ssh_modal_submit_validation_message(parent_id, editing_asset_id, draft)
                .is_none()
    }

    fn ssh_modal_submit_validation_message(
        &self,
        parent_id: Option<&str>,
        editing_asset_id: Option<&str>,
        draft: &AssetSshConnectionDraft,
    ) -> Option<String> {
        let name_message =
            asset_name_validation_message(self.console_asset_tree.validate_name_in_parent(
                parent_id,
                &draft.name,
                editing_asset_id,
            ));
        if !name_message.is_empty() {
            return Some(name_message);
        }

        if draft.host.trim().is_empty() {
            return Some("Host is required.".into());
        }

        if draft.auth_source == SSH_AUTH_SOURCE_MANUAL && draft.user.trim().is_empty() {
            return Some("User is required.".into());
        }

        if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
            if draft.keychain_identity_id.trim().is_empty() {
                return Some("Keychain identity is required.".into());
            }
        } else {
            match draft.auth_method.as_str() {
                "password" => {
                    if draft.password.trim().is_empty() {
                        return Some("Password is required.".into());
                    }
                }
                "private-key" => match draft.private_key_source.as_str() {
                    "path" => {
                        if draft.private_key_path.trim().is_empty() {
                            return Some("Private key path is required.".into());
                        }
                    }
                    "content" => {
                        if draft.private_key_content.trim().is_empty() {
                            return Some("Private key content is required.".into());
                        }
                    }
                    _ => return Some("Private key source is required.".into()),
                },
                _ => return Some("Authentication method is required.".into()),
            }
        }

        if draft.auth_source == SSH_AUTH_SOURCE_MANUAL {
            match draft.auth_method.as_str() {
                "password" | "private-key" => {}
                _ => {
                    return Some("Authentication method is required.".into());
                }
            }
        }

        match draft.proxy_type.as_str() {
            "" | "none" => {}
            "socks5" | "http" => {
                let proxy_label = if draft.proxy_type == "http" {
                    "HTTP"
                } else {
                    "SOCKS5"
                };
                if draft.proxy_socks5_host.trim().is_empty() {
                    return Some(format!("{proxy_label} proxy host is required."));
                }
                if draft.proxy_socks5_port.trim().is_empty() {
                    return Some(format!("{proxy_label} proxy port is required."));
                }
                if draft.proxy_socks5_port.trim().parse::<u16>().is_err() {
                    return Some(format!("{proxy_label} proxy port must be a valid number."));
                }
            }
            "ssh-asset" => {
                let upstream_asset_id = draft.proxy_ssh_asset_id.trim();
                if upstream_asset_id.is_empty() {
                    return Some("Upstream SSH connection is required.".into());
                }
                if editing_asset_id.is_some_and(|editing_id| editing_id == upstream_asset_id) {
                    return Some("Upstream SSH connection cannot reference itself.".into());
                }
            }
            _ => return Some("Proxy type is invalid.".into()),
        }

        None
    }

    pub fn set_ssh_modal_feedback(&mut self, message: impl Into<String>) {
        self.finish_ssh_modal_action_error(message);
    }

    fn ssh_modal_is_busy(&self) -> bool {
        matches!(self.ssh_modal_action_state, SshModalActionState::Busy(_))
    }
}

fn ssh_proxy_target_options_for_tree(
    tree: &AssetTree,
    excluded_asset_id: Option<&str>,
) -> Vec<SshProxyTargetOption> {
    let mut entries = Vec::new();
    collect_ssh_proxy_target_entries(tree, tree.root_ids(), excluded_asset_id, &mut entries);

    let mut title_counts = HashMap::<String, usize>::new();
    for (_, title) in &entries {
        *title_counts.entry(title.clone()).or_default() += 1;
    }

    entries
        .into_iter()
        .map(|(asset_id, title)| SshProxyTargetOption {
            label: if title_counts.get(&title).copied().unwrap_or_default() > 1 {
                format!("{title} · {asset_id}")
            } else {
                title
            },
            asset_id,
        })
        .collect()
}

fn ssh_keychain_identity_options_for_catalog(
    catalog: &KeychainCatalog,
) -> Vec<SshKeychainIdentityOption> {
    let mut entries = catalog
        .nodes
        .values()
        .filter_map(|node| match node.payload {
            KeychainNodePayload::Identity(_) => Some((node.id.clone(), node.title.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));

    let mut title_counts = HashMap::<String, usize>::new();
    for (_, title) in &entries {
        *title_counts.entry(title.clone()).or_default() += 1;
    }

    entries
        .into_iter()
        .map(|(identity_id, title)| SshKeychainIdentityOption {
            label: if title_counts.get(&title).copied().unwrap_or_default() > 1 {
                format!("{title} · {identity_id}")
            } else {
                title
            },
            identity_id,
        })
        .collect()
}

fn keychain_ssh_key_options_for_catalog(catalog: &KeychainCatalog) -> Vec<KeychainSshKeyOption> {
    let mut entries = catalog
        .nodes
        .values()
        .filter_map(|node| match node.payload {
            KeychainNodePayload::SshKey(_) => Some((node.id.clone(), node.title.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));

    let mut title_counts = HashMap::<String, usize>::new();
    for (_, title) in &entries {
        *title_counts.entry(title.clone()).or_default() += 1;
    }

    entries
        .into_iter()
        .map(|(key_id, title)| KeychainSshKeyOption {
            label: if title_counts.get(&title).copied().unwrap_or_default() > 1 {
                format!("{title} · {key_id}")
            } else {
                title
            },
            key_id,
        })
        .collect()
}

fn collect_ssh_proxy_target_entries(
    tree: &AssetTree,
    node_ids: &[String],
    excluded_asset_id: Option<&str>,
    output: &mut Vec<(String, String)>,
) {
    for node_id in node_ids {
        let Some(node) = tree.node(node_id) else {
            continue;
        };

        if node.kind == ConsoleAssetKind::SshConnection
            && excluded_asset_id != Some(node.id.as_str())
        {
            output.push((node.id.clone(), node.title.clone()));
        }

        collect_ssh_proxy_target_entries(tree, &node.children, excluded_asset_id, output);
    }
}

fn current_action_node<'a>(
    roots: &'a [ContextMenuActionNode],
    open_path: &[usize],
) -> Option<&'a ContextMenuActionNode> {
    if open_path.is_empty() {
        roots.first()
    } else {
        action_node_at_path(roots, open_path)
    }
}

fn action_node_at_path<'a>(
    roots: &'a [ContextMenuActionNode],
    open_path: &[usize],
) -> Option<&'a ContextMenuActionNode> {
    let (first, rest) = open_path.split_first()?;
    let mut current = roots.get(*first)?;

    for index in rest {
        current = current.children.get(*index)?;
    }

    Some(current)
}

fn find_action_node_by_id<'a>(
    nodes: &'a [ContextMenuActionNode],
    action_id: &str,
) -> Option<&'a ContextMenuActionNode> {
    for node in nodes {
        if node.id == action_id {
            return Some(node);
        }

        if let Some(found) = find_action_node_by_id(&node.children, action_id) {
            return Some(found);
        }
    }

    None
}
