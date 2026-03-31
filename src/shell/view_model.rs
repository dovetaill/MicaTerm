//! Central shell state mirrored into Slint properties and mutated by UI callbacks.

use std::collections::{BTreeSet, HashMap};

use crate::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainNodePayload, KeychainSshKeySpec,
};
use crate::app::quick_launch_preferences::{QuickLaunchPreferences, record_recent_asset_id};
use crate::app::sftp::{
    SftpDirectoryEntry, SftpFollowMode, SftpPanelMode, SftpSessionBindingState,
    TransferQueueSummary,
};
use crate::app::ssh::credentials::{
    SshCredentialKind, keychain_key_credential_ref, ssh_credential_ref,
};
use crate::app::ssh::runtime::TerminalSurfaceState;
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
use crate::shell::quick_launch::{
    QUICK_LAUNCH_RECENT_LIMIT, QuickLaunchAssetRecord, QuickLaunchCardItem, QuickLaunchDetailItem,
    QuickLaunchGroupItem, collect_quick_launch_records, group_id_for_asset,
    matches_quick_launch_query, project_card_item, project_detail_item,
};
use crate::shell::sidebar::SidebarDestination;
use crate::shell::tabs::WorkspaceTab;
use crate::theme::ThemeMode;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncModalMode {
    NotConfigured,
    Locked,
    UnlockedButRemoteIncomplete,
    Ready,
    SyncError,
}

impl SyncModalMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::NotConfigured => "not-configured",
            Self::Locked => "locked",
            Self::UnlockedButRemoteIncomplete => "unlocked-but-remote-incomplete",
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
    pub primary_action_label: String,
    pub secondary_action_label: String,
}

impl Default for SyncModalViewState {
    fn default() -> Self {
        Self {
            open: false,
            mode: SyncModalMode::NotConfigured,
            title: "Sync".into(),
            headline: "Set up sync".into(),
            status_text: "Configure a Gitee remote to enable sync.".into(),
            error_text: String::new(),
            provider_label: "Gitee".into(),
            target_label: String::new(),
            primary_action_label: "Set up sync".into(),
            secondary_action_label: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultPanelViewState {
    pub title: String,
    pub lock_state_label: String,
    pub primary_status_label: String,
    pub primary_action_label: String,
    pub secondary_action_label: String,
    pub tertiary_action_label: String,
    pub sync_now_label: String,
    pub export_bootstrap_label: String,
    pub import_bootstrap_label: String,
}

impl Default for VaultPanelViewState {
    fn default() -> Self {
        Self {
            title: "Sync & Vault".into(),
            lock_state_label: "Locked".into(),
            primary_status_label: "Primary not configured".into(),
            primary_action_label: "Set".into(),
            secondary_action_label: "Change".into(),
            tertiary_action_label: "Lock now".into(),
            sync_now_label: "Sync now".into(),
            export_bootstrap_label: "Export bootstrap".into(),
            import_bootstrap_label: "Import bootstrap".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SftpConflictModalState {
    pub open: bool,
    pub source_path: String,
    pub target_path: String,
    pub can_resume: bool,
    pub apply_to_all: bool,
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
    NewKeychainSshKey {
        parent_id: Option<String>,
        draft: KeychainSshKeyDraft,
    },
    NewSshConnection {
        parent_id: Option<String>,
        editing_asset_id: Option<String>,
        draft: AssetSshConnectionDraft,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub right_panel_view: RightPanelView,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
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
    quick_launch_search_query: String,
    quick_launch_selected_asset_id: Option<String>,
    quick_launch_active_group_id: Option<String>,
    saved_ssh_picker_open: bool,
    saved_ssh_picker_query: String,
    saved_ssh_picker_selected_asset_id: Option<String>,
    pub sftp_sessions: HashMap<String, SftpSessionBindingState>,
    pub sftp_queue_summary: TransferQueueSummary,
    pub sftp_queue_drawer_open: bool,
    workspace_tabs: Vec<WorkspaceTab>,
    active_workspace_session_id: Option<String>,
    active_workspace_terminal_surface: Option<TerminalSurfaceState>,
    pending_ssh_modal_action: Option<PendingSshModalAction>,
    pending_snippet_create_action: Option<SnippetCreateAction>,
    pending_snippet_activation: Option<PendingSnippetActivation>,
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
    sync_modal_state: SyncModalViewState,
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
            right_panel_view: RightPanelView::Sftp,
            show_global_menu: false,
            show_assets_sidebar: true,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
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
            quick_launch_search_query: String::new(),
            quick_launch_selected_asset_id: None,
            quick_launch_active_group_id: None,
            saved_ssh_picker_open: false,
            saved_ssh_picker_query: String::new(),
            saved_ssh_picker_selected_asset_id: None,
            sftp_sessions: HashMap::new(),
            sftp_queue_summary: TransferQueueSummary::default(),
            sftp_queue_drawer_open: false,
            workspace_tabs: Vec::new(),
            active_workspace_session_id: None,
            active_workspace_terminal_surface: None,
            pending_ssh_modal_action: None,
            pending_snippet_create_action: None,
            pending_snippet_activation: None,
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
            sync_modal_state: SyncModalViewState::default(),
            vault_panel_state: VaultPanelViewState::default(),
            console_asset_tree: AssetTree::new(),
            snippet_asset_tree: AssetTree::new(),
            keychain_catalog: KeychainCatalog::default(),
            keychain_expanded_ids: BTreeSet::new(),
            window_placement: WindowPlacementKind::Restored,
        }
    }
}

impl ShellViewModel {
    fn create_asset_modal_validation(
        &self,
        parent_id: Option<&str>,
        draft_name: &str,
    ) -> AssetNameValidation {
        self.console_asset_tree
            .validate_name_in_parent(parent_id, draft_name, None)
    }

    fn rename_asset_modal_validation(
        &self,
        asset_id: &str,
        draft_name: &str,
    ) -> AssetNameValidation {
        let parent_id = self.console_asset_tree.parent_id(asset_id).flatten();
        self.console_asset_tree
            .validate_name_in_parent(parent_id, draft_name, Some(asset_id))
    }

    fn sftp_name_validation(
        &self,
        draft_name: &str,
        editing_entry_id: Option<&str>,
    ) -> AssetNameValidation {
        let trimmed = draft_name.trim();
        if trimmed.is_empty() {
            return AssetNameValidation::Empty;
        }

        let duplicate = self
            .active_sftp_session_state()
            .into_iter()
            .flat_map(|state| state.entries.iter())
            .filter(|entry| Some(entry.id.as_str()) != editing_entry_id)
            .any(|entry| entry.name.trim() == trimmed);

        if duplicate {
            AssetNameValidation::Duplicate
        } else {
            AssetNameValidation::Valid
        }
    }

    pub fn asset_rename_modal_validation_message(&self) -> String {
        match &self.asset_modal_state {
            Some(AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            }) => asset_name_validation_message(
                self.rename_asset_modal_validation(asset_id, draft_name),
            ),
            Some(AssetModalState::SftpRenameEntry {
                entry_id,
                draft_name,
                ..
            }) => asset_name_validation_message(
                self.sftp_name_validation(draft_name, Some(entry_id.as_str())),
            ),
            _ => String::new(),
        }
    }

    pub fn asset_create_modal_validation_message(&self) -> String {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder {
                parent_id,
                draft_name,
            }) => asset_name_validation_message(
                self.create_asset_modal_validation(parent_id.as_deref(), draft_name),
            ),
            Some(AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            }) => self.snippet_modal_validation_message(
                parent_package_id.as_deref(),
                editing_asset_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            }) => asset_name_validation_message(self.snippet_asset_tree.validate_name_in_parent(
                None,
                draft_name,
                editing_asset_id.as_deref(),
            )),
            Some(AssetModalState::NewKeychainSshKey { parent_id, draft }) => {
                self.keychain_ssh_key_modal_validation_message(parent_id.as_deref(), draft)
            }
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => self.ssh_modal_validation_message(
                parent_id.as_deref(),
                editing_asset_id.as_deref(),
                draft,
            ),
            Some(AssetModalState::SftpNewFolder { draft_name }) => {
                asset_name_validation_message(self.sftp_name_validation(draft_name, None))
            }
            _ => String::new(),
        }
    }

    pub fn asset_create_modal_can_confirm(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder {
                parent_id,
                draft_name,
            }) => {
                self.create_asset_modal_validation(parent_id.as_deref(), draft_name)
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            }) => self
                .snippet_modal_validation_message(
                    parent_package_id.as_deref(),
                    editing_asset_id.as_deref(),
                    draft,
                )
                .is_empty(),
            Some(AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            }) => {
                self.snippet_asset_tree.validate_name_in_parent(
                    None,
                    draft_name,
                    editing_asset_id.as_deref(),
                ) == AssetNameValidation::Valid
            }
            Some(AssetModalState::NewKeychainSshKey { parent_id, draft }) => {
                self.keychain_ssh_key_modal_can_confirm(parent_id.as_deref(), draft)
            }
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => {
                self.ssh_modal_can_confirm(parent_id.as_deref(), editing_asset_id.as_deref(), draft)
            }
            Some(AssetModalState::SftpNewFolder { draft_name }) => {
                self.sftp_name_validation(draft_name, None) == AssetNameValidation::Valid
            }
            _ => false,
        }
    }

    pub fn requested_assets_sidebar(&self) -> bool {
        self.show_assets_sidebar
    }

    pub fn requested_right_panel(&self) -> bool {
        self.show_right_panel
    }

    pub fn sync_modal_open(&self) -> bool {
        self.sync_modal_state.open
    }

    pub fn sync_modal_state(&self) -> &SyncModalViewState {
        &self.sync_modal_state
    }

    pub fn sync_modal_state_mut(&mut self) -> &mut SyncModalViewState {
        &mut self.sync_modal_state
    }

    pub fn right_panel_view_id(&self) -> &'static str {
        self.right_panel_view.id()
    }

    pub fn set_right_panel_view(&mut self, value: RightPanelView) {
        self.right_panel_view = value;
    }

    pub fn open_settings_panel(&mut self) {
        self.open_sftp_panel();
    }

    pub fn open_sync_modal(&mut self) {
        self.sync_modal_state.open = true;
        self.show_global_menu = false;
    }

    pub fn set_sync_modal_error(&mut self, error: impl Into<String>) {
        self.sync_modal_state.open = true;
        self.sync_modal_state.error_text = error.into();
    }

    pub fn clear_sync_modal_error(&mut self) {
        self.sync_modal_state.error_text.clear();
    }

    pub fn close_sync_modal(&mut self) {
        self.sync_modal_state.open = false;
        self.show_global_menu = false;
    }

    pub fn open_appearance_panel(&mut self) {
        self.open_sftp_panel();
    }

    pub fn open_sftp_panel(&mut self) {
        self.right_panel_view = RightPanelView::Sftp;
        self.show_right_panel = true;
        self.show_global_menu = false;
    }

    pub fn set_sftp_session_state(
        &mut self,
        session_id: impl Into<String>,
        state: SftpSessionBindingState,
    ) {
        self.sftp_sessions.insert(session_id.into(), state);
    }

    pub fn active_sftp_session_state(&self) -> Option<&SftpSessionBindingState> {
        let session_id = self.active_workspace_session_id.as_deref()?;
        self.sftp_sessions.get(session_id)
    }

    fn active_sftp_session_state_mut(&mut self) -> Option<&mut SftpSessionBindingState> {
        let session_id = self.active_workspace_session_id.clone()?;
        Some(self.sftp_sessions.entry(session_id).or_default())
    }

    pub fn sftp_panel_mode_id(&self) -> &'static str {
        if self.active_workspace_session_id.is_none() {
            return SftpPanelMode::Empty.id();
        }

        self.active_sftp_session_state()
            .map(|state| state.mode.id())
            .unwrap_or(SftpPanelMode::Empty.id())
    }

    pub fn sftp_panel_host_label(&self) -> String {
        self.active_workspace_tab()
            .map(|tab| tab.title.clone())
            .unwrap_or_default()
    }

    pub fn sftp_panel_path(&self) -> String {
        self.active_sftp_session_state()
            .map(|state| state.current_path.clone())
            .unwrap_or_default()
    }

    pub fn sftp_panel_follow_mode_id(&self) -> &'static str {
        self.active_sftp_session_state()
            .map(|state| state.follow_mode.id())
            .unwrap_or(SftpFollowMode::FollowCwd.id())
    }

    pub fn sftp_panel_can_go_back(&self) -> bool {
        self.active_sftp_session_state()
            .map(|state| state.history.can_back())
            .unwrap_or(false)
    }

    pub fn sftp_panel_can_go_forward(&self) -> bool {
        self.active_sftp_session_state()
            .map(|state| state.history.can_forward())
            .unwrap_or(false)
    }

    pub fn sftp_panel_can_go_up(&self) -> bool {
        self.active_sftp_session_state()
            .map(SftpSessionBindingState::can_navigate_up)
            .unwrap_or(false)
    }

    pub fn sftp_panel_actions_enabled(&self) -> bool {
        self.active_sftp_session_state()
            .map(|state| {
                matches!(
                    state.mode,
                    SftpPanelMode::Ready | SftpPanelMode::Loading | SftpPanelMode::Connecting
                )
            })
            .unwrap_or(false)
    }

    pub fn sftp_panel_entries(&self) -> &[SftpDirectoryEntry] {
        self.active_sftp_session_state()
            .map(|state| state.entries.as_slice())
            .unwrap_or(&[])
    }

    pub fn sftp_panel_selected_entry_ids(&self) -> &[String] {
        self.active_sftp_session_state()
            .map(|state| state.selected_entry_ids.as_slice())
            .unwrap_or(&[])
    }

    pub fn submit_sftp_panel_path(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return false;
        }

        let Some(state) = self.active_sftp_session_state_mut() else {
            return false;
        };
        state.navigate_manual(trimmed.to_string());
        true
    }

    pub fn navigate_sftp_panel_back(&mut self) -> bool {
        self.active_sftp_session_state_mut()
            .map(SftpSessionBindingState::navigate_back)
            .unwrap_or(false)
    }

    pub fn navigate_sftp_panel_forward(&mut self) -> bool {
        self.active_sftp_session_state_mut()
            .map(SftpSessionBindingState::navigate_forward)
            .unwrap_or(false)
    }

    pub fn navigate_sftp_panel_up(&mut self) -> bool {
        self.active_sftp_session_state_mut()
            .map(SftpSessionBindingState::navigate_up)
            .unwrap_or(false)
    }

    pub fn retry_sftp_panel(&mut self) -> bool {
        let Some(state) = self.active_sftp_session_state_mut() else {
            return false;
        };
        state.mark_connecting();
        true
    }

    pub fn refresh_sftp_panel(&mut self) -> bool {
        let Some(state) = self.active_sftp_session_state_mut() else {
            return false;
        };
        state.mark_loading();
        true
    }

    pub fn reenable_sftp_follow(&mut self) -> bool {
        let Some(state) = self.active_sftp_session_state_mut() else {
            return false;
        };
        let path = if state.current_path.is_empty() {
            "/".to_string()
        } else {
            state.current_path.clone()
        };
        state.reenable_follow(path);
        true
    }

    pub fn sftp_queue_drawer_open(&self) -> bool {
        self.sftp_queue_drawer_open
    }

    pub fn toggle_sftp_queue_drawer(&mut self) {
        self.sftp_queue_drawer_open = !self.sftp_queue_drawer_open;
    }

    pub fn sftp_conflict_modal_state(&self) -> &SftpConflictModalState {
        &self.sftp_conflict_modal_state
    }

    pub fn vault_panel_state(&self) -> &VaultPanelViewState {
        &self.vault_panel_state
    }

    pub fn vault_panel_state_mut(&mut self) -> &mut VaultPanelViewState {
        &mut self.vault_panel_state
    }

    pub fn workspace_tabs(&self) -> &[WorkspaceTab] {
        &self.workspace_tabs
    }

    pub fn set_workspace_tabs(&mut self, tabs: Vec<WorkspaceTab>) {
        self.workspace_tabs = tabs;
        self.normalize_workspace_tabs();
    }

    pub fn active_workspace_session_id(&self) -> Option<&str> {
        self.active_workspace_session_id.as_deref()
    }

    pub fn active_workspace_terminal_surface(&self) -> Option<&TerminalSurfaceState> {
        let active_id = self.active_workspace_session_id.as_deref()?;
        self.active_workspace_terminal_surface
            .as_ref()
            .filter(|surface| surface.session_id.to_string() == active_id)
    }

    pub fn set_active_workspace_terminal_surface(&mut self, surface: Option<TerminalSurfaceState>) {
        self.active_workspace_terminal_surface = surface;
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

    pub fn active_workspace_tab(&self) -> Option<&WorkspaceTab> {
        let active_id = self.active_workspace_session_id.as_deref()?;
        self.workspace_tabs
            .iter()
            .find(|tab| tab.session_id == active_id)
    }

    pub fn activate_workspace_session(&mut self, session_id: &str) -> bool {
        if !self
            .workspace_tabs
            .iter()
            .any(|tab| tab.session_id == session_id)
        {
            return false;
        }

        self.active_workspace_session_id = Some(session_id.to_string());
        self.normalize_workspace_tabs();
        true
    }

    pub fn close_workspace_session(&mut self, session_id: &str) -> bool {
        self.close_workspace_session_with_fallback(session_id)
    }

    pub fn open_workspace_launcher_tab(&mut self) {
        if self.workspace_tabs.iter().any(WorkspaceTab::is_launcher) {
            let _ = self.activate_workspace_session("workspace-launcher");
            return;
        }

        let mut launcher = WorkspaceTab::launcher();
        launcher.active = true;
        self.workspace_tabs.push(launcher);
        self.active_workspace_session_id = Some("workspace-launcher".into());
        self.normalize_workspace_tabs();
    }

    pub fn close_workspace_launcher_tab(&mut self) -> bool {
        self.close_workspace_session_with_fallback("workspace-launcher")
    }

    pub fn close_workspace_session_with_fallback(&mut self, session_id: &str) -> bool {
        let Some(removed_index) = self
            .workspace_tabs
            .iter()
            .position(|tab| tab.session_id == session_id)
        else {
            return false;
        };

        let removed_was_active = self.active_workspace_session_id.as_deref() == Some(session_id);
        self.workspace_tabs.remove(removed_index);

        if removed_was_active {
            self.active_workspace_session_id = self
                .workspace_tabs
                .get(removed_index)
                .or_else(|| {
                    removed_index
                        .checked_sub(1)
                        .and_then(|index| self.workspace_tabs.get(index))
                })
                .map(|tab| tab.session_id.clone());
        }

        self.normalize_workspace_tabs();
        true
    }

    pub fn active_workspace_session_can_close(&self) -> bool {
        self.active_workspace_tab().is_some()
    }

    pub fn active_workspace_session_can_reconnect(&self) -> bool {
        self.active_workspace_tab()
            .map(WorkspaceTab::can_reconnect)
            .unwrap_or(false)
    }

    pub fn workspace_terminal_surface_ready(&self) -> bool {
        self.active_workspace_terminal_surface().is_some()
    }

    pub fn workspace_terminal_surface_seqno(&self) -> usize {
        self.active_workspace_terminal_surface()
            .map(|surface| surface.seqno)
            .unwrap_or_default()
    }

    pub fn workspace_terminal_visible_lines(&self) -> Vec<String> {
        self.active_workspace_terminal_surface()
            .map(|surface| surface.visible_lines.clone())
            .unwrap_or_default()
    }

    pub fn workspace_session_host_mode(&self) -> &'static str {
        match self.active_workspace_tab() {
            None => "welcome",
            Some(tab) if tab.is_launcher() => "welcome",
            Some(tab) if tab.uses_terminal_surface() => "terminal",
            Some(tab) if tab.uses_connection_progress_surface() => "connection-progress",
            Some(_) => "session-error",
        }
    }

    pub fn saved_ssh_picker_open(&self) -> bool {
        self.saved_ssh_picker_open
    }

    pub fn saved_ssh_picker_query(&self) -> &str {
        &self.saved_ssh_picker_query
    }

    pub fn open_saved_ssh_picker(&mut self) {
        self.saved_ssh_picker_open = true;
        self.saved_ssh_picker_query.clear();
        self.saved_ssh_picker_selected_asset_id = self.first_saved_ssh_picker_asset_id();
    }

    pub fn close_saved_ssh_picker(&mut self) {
        self.saved_ssh_picker_open = false;
        self.saved_ssh_picker_query.clear();
        self.saved_ssh_picker_selected_asset_id = None;
    }

    pub fn set_saved_ssh_picker_query(&mut self, query: String) {
        self.saved_ssh_picker_query = query;
        self.saved_ssh_picker_selected_asset_id = self.first_saved_ssh_picker_asset_id();
    }

    pub fn select_saved_ssh_picker_asset(&mut self, asset_id: String) {
        if self
            .saved_ssh_picker_items()
            .iter()
            .any(|item| item.id == asset_id)
        {
            self.saved_ssh_picker_selected_asset_id = Some(asset_id);
        }
    }

    pub fn toggle_saved_ssh_picker_expanded(&mut self, asset_id: &str) {
        if self.console_asset_tree.kind(asset_id) != Some(ConsoleAssetKind::Folder) {
            return;
        }

        let next = !self
            .console_asset_tree
            .is_expanded(asset_id)
            .unwrap_or(false);
        self.console_asset_tree.set_expanded(asset_id, next);
    }

    pub fn saved_ssh_picker_items(&self) -> Vec<SavedSshPickerItem> {
        let mut rows = Vec::new();
        let query_active = !self.saved_ssh_picker_query.trim().is_empty();
        for root_id in self.console_asset_tree.root_ids() {
            self.collect_saved_ssh_picker_rows(root_id, 0, query_active, &mut rows);
        }
        rows
    }

    pub fn quick_launch_preferences(&self) -> &QuickLaunchPreferences {
        &self.quick_launch_preferences
    }

    pub fn quick_launch_search_query(&self) -> &str {
        &self.quick_launch_search_query
    }

    pub fn quick_launch_selected_asset_id(&self) -> Option<&str> {
        self.quick_launch_selected_asset_id.as_deref()
    }

    pub fn quick_launch_active_group_id(&self) -> Option<&str> {
        self.quick_launch_active_group_id.as_deref()
    }

    pub fn apply_quick_launch_preferences(&mut self, prefs: QuickLaunchPreferences) {
        self.quick_launch_selected_asset_id = prefs.last_selected_asset_id.clone();
        self.quick_launch_preferences = prefs;
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn record_recent_saved_ssh_asset(&mut self, asset_id: &str) {
        if self
            .console_asset_tree
            .ssh_connection_spec(asset_id)
            .is_none()
        {
            return;
        }

        self.quick_launch_preferences.recent_asset_ids = record_recent_asset_id(
            self.quick_launch_preferences.recent_asset_ids.clone(),
            asset_id,
            QUICK_LAUNCH_RECENT_LIMIT,
        );
        self.quick_launch_preferences.last_selected_asset_id = Some(asset_id.to_string());
        self.quick_launch_selected_asset_id = Some(asset_id.to_string());
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn toggle_quick_launch_favorite(&mut self, asset_id: &str) {
        if self
            .console_asset_tree
            .ssh_connection_spec(asset_id)
            .is_none()
        {
            return;
        }

        if let Some(index) = self
            .quick_launch_preferences
            .favorite_asset_ids
            .iter()
            .position(|current| current == asset_id)
        {
            self.quick_launch_preferences
                .favorite_asset_ids
                .remove(index);
        } else {
            self.quick_launch_preferences
                .favorite_asset_ids
                .insert(0, asset_id.to_string());
        }

        self.ensure_quick_launch_selection();
    }

    pub fn select_quick_launch_asset(&mut self, asset_id: String) {
        if self
            .console_asset_tree
            .ssh_connection_spec(&asset_id)
            .is_none()
        {
            return;
        }

        self.quick_launch_preferences.last_selected_asset_id = Some(asset_id.clone());
        self.quick_launch_selected_asset_id = Some(asset_id);
        self.sync_quick_launch_group_from_selected();
        self.ensure_quick_launch_selection();
    }

    pub fn set_quick_launch_search_query(&mut self, query: String) {
        self.quick_launch_search_query = query;
        self.ensure_quick_launch_selection();
    }

    pub fn quick_launch_recent_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.ordered_quick_launch_cards_from_ids(
            &self.quick_launch_preferences.recent_asset_ids,
            &records,
        )
    }

    pub fn quick_launch_favorite_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.ordered_quick_launch_cards_from_ids(
            &self.quick_launch_preferences.favorite_asset_ids,
            &records,
        )
    }

    pub fn quick_launch_group_items(&self) -> Vec<QuickLaunchGroupItem> {
        let records = self.matching_quick_launch_records();
        let mut groups = Vec::<QuickLaunchGroupItem>::new();
        let mut positions = HashMap::<String, usize>::new();

        for record in records {
            let Some(group) = record.group else {
                continue;
            };

            if let Some(position) = positions.get(&group.id).copied() {
                groups[position].count += 1;
            } else {
                positions.insert(group.id.clone(), groups.len());
                groups.push(QuickLaunchGroupItem {
                    group_id: group.id,
                    label: group.label,
                    count: 1,
                });
            }
        }

        groups
    }

    pub fn quick_launch_visible_group_items(&self) -> Vec<QuickLaunchCardItem> {
        let records = self.matching_quick_launch_records();
        self.visible_group_records(&records)
            .into_iter()
            .map(|record| {
                project_card_item(
                    &record,
                    self.is_quick_launch_favorite(record.asset_id.as_str()),
                )
            })
            .collect()
    }

    pub fn quick_launch_selected_detail(&self) -> Option<QuickLaunchDetailItem> {
        let selected_asset_id = self.quick_launch_selected_asset_id.as_deref()?;
        let records = self.quick_launch_records();
        let record = records
            .iter()
            .find(|record| record.asset_id == selected_asset_id)?;

        Some(project_detail_item(
            record,
            self.quick_launch_recent_label(selected_asset_id),
        ))
    }

    pub fn ensure_quick_launch_selection(&mut self) {
        let records = self.matching_quick_launch_records();
        let visible_asset_ids = self.visible_asset_ids_from_records(&records);
        if self
            .quick_launch_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| {
                visible_asset_ids
                    .iter()
                    .any(|visible_asset_id| visible_asset_id == asset_id)
            })
        {
            self.sync_quick_launch_group_from_selected();
            return;
        }

        self.quick_launch_selected_asset_id =
            self.first_visible_quick_launch_asset_id_from_records(&records);
        self.quick_launch_preferences.last_selected_asset_id =
            self.quick_launch_selected_asset_id.clone();
        self.sync_quick_launch_group_from_selected();
    }

    pub fn toggle_right_panel(&mut self) {
        self.show_right_panel = !self.show_right_panel;
        self.right_panel_view = RightPanelView::Sftp;
    }

    pub fn toggle_global_menu(&mut self) {
        self.show_global_menu = !self.show_global_menu;
    }

    pub fn close_global_menu(&mut self) {
        self.show_global_menu = false;
    }

    pub fn toggle_assets_sidebar(&mut self) {
        self.show_assets_sidebar = !self.show_assets_sidebar;
    }

    pub fn select_sidebar_destination(&mut self, destination: SidebarDestination) {
        self.active_sidebar_destination = destination;
        self.show_assets_sidebar = true;
        if destination != SidebarDestination::Console {
            self.asset_create_menu_open = false;
        }
    }

    pub fn window_placement(&self) -> WindowPlacementKind {
        self.window_placement
    }

    pub fn set_window_placement(&mut self, value: WindowPlacementKind) {
        self.window_placement = value;
    }

    pub fn is_window_maximized(&self) -> bool {
        self.window_placement.is_maximized()
    }

    pub fn set_window_active(&mut self, value: bool) {
        self.is_window_active = value;
    }

    pub fn toggle_theme_mode(&mut self) {
        self.theme_mode = self.theme_mode.toggled();
    }

    pub fn toggle_always_on_top(&mut self) {
        self.is_always_on_top = !self.is_always_on_top;
    }

    pub fn toggle_asset_view_mode(&mut self) {
        self.asset_view_mode = self.asset_view_mode.toggle();
    }

    pub fn toggle_asset_search(&mut self) {
        self.activate_asset_search();
    }

    pub fn activate_asset_search(&mut self) {
        self.asset_search_expanded = true;
        self.asset_create_menu_open = false;
    }

    pub fn close_asset_search(&mut self) {
        self.asset_search_expanded = false;
    }

    pub fn set_asset_search_query(&mut self, query: String) {
        self.asset_search_query = query;
    }

    pub fn collapse_asset_search_if_empty(&mut self) {
        if self.asset_search_query.is_empty() {
            self.asset_search_expanded = false;
        }
    }

    pub fn dismiss_empty_asset_search_on_shell_interaction(&mut self) -> bool {
        if self.asset_search_expanded && self.asset_search_query.is_empty() {
            self.asset_search_expanded = false;
            true
        } else {
            false
        }
    }

    pub fn toggle_asset_tree_expansion(&mut self) {
        if self.asset_view_mode != AssetViewMode::Tree {
            return;
        }

        self.asset_tree_fully_expanded = !self.asset_tree_fully_expanded;
        self.console_asset_tree
            .set_all_expanded(self.asset_tree_fully_expanded);
    }

    pub fn toggle_asset_create_menu(&mut self) {
        if self.asset_create_menu_open {
            self.asset_create_menu_open = false;
        } else {
            self.asset_create_menu_open = true;
            self.asset_search_expanded = false;
        }
    }

    pub fn close_asset_create_menu(&mut self) {
        self.asset_create_menu_open = false;
    }

    pub fn open_new_folder_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .console_asset_tree
            .next_default_name_for_parent(parent_id.as_deref(), ConsoleAssetKind::Folder);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewFolder {
            parent_id,
            draft_name,
        });
    }

    pub fn open_sftp_new_folder_modal(&mut self) {
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.asset_modal_state = Some(AssetModalState::SftpNewFolder {
            draft_name: self.next_default_sftp_folder_name(),
        });
    }
    pub fn open_new_snippet_modal(&mut self, parent_package_id: Option<String>) {
        let parent_package_id = self.normalize_snippet_package_parent_id(parent_package_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .snippet_asset_tree
            .next_default_name_for_parent(parent_package_id.as_deref(), ConsoleAssetKind::Snippet);
        let package = parent_package_id
            .as_deref()
            .and_then(|asset_id| self.snippet_asset_tree.title(asset_id))
            .unwrap_or_default()
            .to_string();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_package_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewSnippet {
            parent_package_id,
            editing_asset_id: None,
            draft: AssetSnippetDraft {
                name: draft_name,
                script: String::new(),
                package,
            },
        });
    }

    pub fn open_edit_snippet_modal(&mut self, asset_id: String) {
        let Some(node) = self.snippet_asset_tree.node(&asset_id).cloned() else {
            return;
        };
        let AssetNodePayload::Snippet(spec) = node.payload else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::NewSnippet {
            parent_package_id: spec.package_id.clone(),
            editing_asset_id: Some(asset_id),
            draft: AssetSnippetDraft {
                name: node.title,
                script: spec.script,
                package: spec
                    .package_id
                    .as_deref()
                    .and_then(|package_id| self.snippet_asset_tree.title(package_id))
                    .unwrap_or_default()
                    .to_string(),
            },
        });
    }

    pub fn open_new_snippet_package_modal(&mut self) {
        self.dismiss_active_asset_rename();
        let draft_name = self
            .snippet_asset_tree
            .next_default_name_for_parent(None, ConsoleAssetKind::SnippetPackage);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = None;
        self.asset_modal_state = Some(AssetModalState::NewSnippetPackage {
            editing_asset_id: None,
            draft_name,
        });
    }

    pub fn open_edit_snippet_package_modal(&mut self, asset_id: String) {
        if self.snippet_asset_tree.kind(&asset_id) != Some(ConsoleAssetKind::SnippetPackage) {
            return;
        }
        let Some(original_name) = self.snippet_asset_tree.title(&asset_id).map(str::to_string)
        else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::NewSnippetPackage {
            editing_asset_id: Some(asset_id),
            draft_name: original_name,
        });
    }

    pub fn open_new_keychain_ssh_key_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_keychain_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.asset_modal_state = Some(AssetModalState::NewKeychainSshKey {
            draft: KeychainSshKeyDraft {
                name: next_default_keychain_name_for_parent(
                    &self.keychain_catalog,
                    parent_id.as_deref(),
                    KeychainItemKind::SshKey,
                ),
                ..KeychainSshKeyDraft::default()
            },
            parent_id,
        });
    }

    pub fn update_new_folder_modal_name(&mut self, value: String) {
        let Some(modal_state) = self.asset_modal_state.as_mut() else {
            return;
        };

        match modal_state {
            AssetModalState::NewFolder { draft_name, .. }
            | AssetModalState::SftpNewFolder { draft_name } => {
                *draft_name = value;
            }
            _ => {}
        }
    }

    pub fn update_snippet_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewSnippet { draft, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        match field {
            "name" => draft.name = value,
            "script" => draft.script = value,
            "package" => draft.package = value,
            _ => {}
        }
    }

    pub fn update_keychain_ssh_key_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewKeychainSshKey { draft, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        match field {
            "name" => draft.name = value,
            "private_key" => draft.private_key = value,
            "public_key" => draft.public_key = value,
            "fingerprint" => draft.fingerprint = value,
            _ => {}
        }
    }

    pub fn update_snippet_package_modal_name(&mut self, value: String) {
        let Some(AssetModalState::NewSnippetPackage { draft_name, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        *draft_name = value;
    }

    pub fn open_new_ssh_modal(&mut self, parent_id: Option<String>) {
        let parent_id = self.normalize_folder_parent_id(parent_id);
        self.dismiss_active_asset_rename();
        let draft_name = self
            .console_asset_tree
            .next_default_name_for_parent(parent_id.as_deref(), ConsoleAssetKind::SshConnection);
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        self.asset_modal_state = Some(AssetModalState::NewSshConnection {
            parent_id,
            editing_asset_id: None,
            draft: AssetSshConnectionDraft {
                name: draft_name,
                ..AssetSshConnectionDraft::default()
            },
        });
    }

    pub fn open_edit_ssh_modal(&mut self, asset_id: String) {
        let Some(node) = self.console_asset_tree.node(&asset_id).cloned() else {
            return;
        };
        let AssetNodePayload::SshConnection(spec) = node.payload else {
            return;
        };
        let auth_method = if spec.auth_method.trim().is_empty() {
            "password".to_string()
        } else {
            spec.auth_method.clone()
        };
        let auth_source = normalized_ssh_auth_source(&spec.auth_source).to_string();
        let private_key_source = if spec.private_key_source.trim().is_empty() {
            "content".to_string()
        } else {
            spec.private_key_source.clone()
        };
        let (
            proxy_type,
            proxy_socks5_host,
            proxy_socks5_port,
            proxy_socks5_username,
            proxy_ssh_asset_id,
        ) = match &spec.proxy {
            AssetSshProxySpec::None => (
                "none".to_string(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            ),
            AssetSshProxySpec::Socks5(proxy) => (
                "socks5".to_string(),
                proxy.host.clone(),
                proxy.port.clone(),
                proxy.username.clone(),
                String::new(),
            ),
            AssetSshProxySpec::Http(proxy) => (
                "http".to_string(),
                proxy.host.clone(),
                proxy.port.clone(),
                proxy.username.clone(),
                String::new(),
            ),
            AssetSshProxySpec::SshAsset { asset_id } => (
                "ssh-asset".to_string(),
                String::new(),
                String::new(),
                String::new(),
                asset_id.clone(),
            ),
        };
        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        self.asset_modal_state = Some(AssetModalState::NewSshConnection {
            parent_id: node.parent_id,
            editing_asset_id: Some(asset_id),
            draft: AssetSshConnectionDraft {
                name: node.title,
                host: spec.host,
                user: spec.user,
                port: spec.port,
                auth_source,
                keychain_identity_id: spec.keychain_identity_id.unwrap_or_default(),
                auth_method,
                private_key_source,
                password: String::new(),
                private_key_content: String::new(),
                private_key_path: spec.private_key_path,
                passphrase: String::new(),
                password_visible: false,
                remark: spec.remark,
                environment: spec.environment,
                proxy_type,
                proxy_socks5_host,
                proxy_socks5_port,
                proxy_socks5_username,
                proxy_socks5_password: String::new(),
                proxy_socks5_password_visible: false,
                proxy_ssh_asset_id,
                proxy_method: spec.proxy_method,
                validation_message: String::new(),
            },
        });
    }

    pub fn hydrate_edit_ssh_modal_secret(
        &mut self,
        password: Option<String>,
        private_key_content: Option<String>,
        passphrase: Option<String>,
        inline_error: Option<String>,
    ) {
        let Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(_),
            draft,
            ..
        }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        draft.password = password.unwrap_or_default();
        draft.private_key_content = private_key_content.unwrap_or_default();
        draft.passphrase = passphrase.unwrap_or_default();
        draft.password_visible = false;
        draft.proxy_socks5_password_visible = false;
        draft.validation_message = inline_error.unwrap_or_default();
    }

    pub fn open_rename_asset_modal(&mut self, asset_id: String) {
        if !self.console_asset_tree.contains(&asset_id) {
            return;
        }

        let Some(original_name) = self.console_asset_tree.title(&asset_id).map(str::to_string)
        else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::RenameAsset {
            asset_id,
            original_name: original_name.clone(),
            draft_name: original_name,
        });
    }

    pub fn open_sftp_rename_entry_modal(&mut self, entry_id: String) {
        let Some(entry) = self
            .active_sftp_session_state()
            .and_then(|state| state.entries.iter().find(|entry| entry.id == entry_id))
            .cloned()
        else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        if let Some(state) = self.active_sftp_session_state_mut() {
            state.selected_entry_ids = vec![entry.id.clone()];
        }
        self.context_target_asset_id = Some(entry.id.clone());
        self.asset_modal_state = Some(AssetModalState::SftpRenameEntry {
            entry_id: entry.id,
            original_name: entry.name.clone(),
            draft_name: entry.name,
        });
    }

    pub fn update_rename_asset_modal_name(&mut self, value: String) {
        let Some(modal_state) = self.asset_modal_state.as_mut() else {
            return;
        };

        match modal_state {
            AssetModalState::RenameAsset { draft_name, .. }
            | AssetModalState::SftpRenameEntry { draft_name, .. } => {
                *draft_name = value;
            }
            _ => {}
        }
    }

    pub fn open_delete_asset_confirm(&mut self, asset_id: String) {
        let snippet_first = self.active_sidebar_destination == SidebarDestination::Snippets;
        let asset_summary = if snippet_first {
            self.snippet_asset_tree
                .title(&asset_id)
                .map(str::to_string)
                .zip(self.snippet_asset_tree.descendant_count(&asset_id))
                .or_else(|| {
                    self.console_asset_tree
                        .title(&asset_id)
                        .map(str::to_string)
                        .zip(self.console_asset_tree.descendant_count(&asset_id))
                })
        } else {
            self.console_asset_tree
                .title(&asset_id)
                .map(str::to_string)
                .zip(self.console_asset_tree.descendant_count(&asset_id))
                .or_else(|| {
                    self.snippet_asset_tree
                        .title(&asset_id)
                        .map(str::to_string)
                        .zip(self.snippet_asset_tree.descendant_count(&asset_id))
                })
        };
        let Some((label, descendant_count)) = asset_summary else {
            return;
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        self.focused_asset_id = Some(asset_id.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.context_target_asset_id = Some(asset_id.clone());
        self.asset_modal_state = Some(AssetModalState::DeleteAssetConfirm {
            asset_id,
            label,
            descendant_count,
        });
    }

    pub fn open_sftp_delete_confirm(&mut self, entry_ids: Vec<String>) {
        let Some(state) = self.active_sftp_session_state() else {
            return;
        };
        let selected_entries = state
            .entries
            .iter()
            .filter(|entry| entry_ids.iter().any(|id| id == &entry.id))
            .cloned()
            .collect::<Vec<_>>();
        if selected_entries.is_empty() {
            return;
        }

        let label = if selected_entries.len() == 1 {
            selected_entries[0].name.clone()
        } else {
            format!("{} items", selected_entries.len())
        };

        self.dismiss_active_asset_rename();
        self.close_context_menu();
        self.close_asset_create_menu();
        if let Some(state) = self.active_sftp_session_state_mut() {
            state.selected_entry_ids = entry_ids.clone();
        }
        self.context_target_asset_id = entry_ids.first().cloned();
        self.asset_modal_state = Some(AssetModalState::SftpDeleteEntriesConfirm {
            entry_ids,
            label,
            descendant_count: 0,
        });
    }

    pub fn update_ssh_modal_field(&mut self, field: &str, value: String) {
        let selected_proxy_asset_id = if field == "proxy_ssh_asset_label" {
            self.resolve_ssh_proxy_target_asset_id_from_label(value.as_str())
        } else if field == "keychain_identity_label" {
            self.resolve_ssh_keychain_identity_id_from_label(value.as_str())
        } else {
            None
        };
        let Some(AssetModalState::NewSshConnection { draft, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };
        match field {
            "name" => draft.name = value,
            "host" => draft.host = value,
            "user" => draft.user = value,
            "port" => draft.port = value,
            "auth_source" => {
                if value == SSH_AUTH_SOURCE_MANUAL || value == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
                    draft.auth_source = value;
                    if draft.auth_source == SSH_AUTH_SOURCE_KEYCHAIN_IDENTITY {
                        draft.password.clear();
                        draft.private_key_content.clear();
                        draft.passphrase.clear();
                        draft.password_visible = false;
                    }
                }
            }
            "keychain_identity_label" => {
                draft.keychain_identity_id = selected_proxy_asset_id.unwrap_or_default();
            }
            "keychain_identity_id" => draft.keychain_identity_id = value,
            "auth_method" => {
                if matches!(value.as_str(), "password" | "private-key") {
                    draft.auth_method = value;
                }
            }
            "private_key_source" => {
                if matches!(value.as_str(), "content" | "path") {
                    draft.private_key_source = value;
                }
            }
            "password" => draft.password = value,
            "private_key_content" => {
                if !value.trim().is_empty() {
                    draft.auth_method = "private-key".into();
                    draft.private_key_source = "content".into();
                    draft.private_key_path.clear();
                }
                draft.private_key_content = value;
            }
            "private_key_path" => draft.private_key_path = value,
            "passphrase" => draft.passphrase = value,
            "password_visibility" => {
                draft.password_visible = matches!(value.as_str(), "visible" | "show" | "true");
            }
            "remark" => draft.remark = value,
            "environment" => draft.environment = value,
            "proxy_type" => {
                if matches!(value.as_str(), "none" | "socks5" | "http" | "ssh-asset") {
                    draft.proxy_type = value;
                }
            }
            "proxy_socks5_host" => draft.proxy_socks5_host = value,
            "proxy_socks5_port" => draft.proxy_socks5_port = value,
            "proxy_socks5_username" => draft.proxy_socks5_username = value,
            "proxy_socks5_password" => draft.proxy_socks5_password = value,
            "proxy_ssh_asset_label" => {
                draft.proxy_ssh_asset_id = selected_proxy_asset_id.unwrap_or_default();
            }
            "proxy_socks5_password_visibility" => {
                draft.proxy_socks5_password_visible =
                    matches!(value.as_str(), "visible" | "show" | "true");
            }
            "proxy_ssh_asset_id" => draft.proxy_ssh_asset_id = value,
            "proxy_method" => draft.proxy_method = value,
            _ => {}
        }

        draft.validation_message.clear();
        self.ssh_modal_action_state = SshModalActionState::Idle;
    }

    pub fn update_ssh_modal_name(&mut self, value: String) {
        self.update_ssh_modal_field("name", value);
    }

    pub fn update_ssh_modal_host(&mut self, value: String) {
        self.update_ssh_modal_field("host", value);
    }

    pub fn begin_ssh_modal_action(&mut self, action_id: &str) -> bool {
        if self.ssh_modal_is_busy() {
            return false;
        }

        let Some(AssetModalState::NewSshConnection {
            parent_id,
            editing_asset_id,
            draft,
            ..
        }) = self.asset_modal_state.as_ref()
        else {
            return false;
        };

        let draft = draft.clone();
        let validation_message = self.ssh_modal_submit_validation_message(
            parent_id.as_deref(),
            editing_asset_id.as_deref(),
            &draft,
        );

        if let Some(AssetModalState::NewSshConnection { draft, .. }) =
            self.asset_modal_state.as_mut()
        {
            draft.validation_message = validation_message.clone().unwrap_or_default();
        }

        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        if validation_message.is_some() {
            return false;
        }

        let action = match action_id {
            "save" => SshModalAction::Save,
            "connect" => SshModalAction::Connect,
            "test" => SshModalAction::TestConnection,
            "save-and-connect" => SshModalAction::SaveAndConnect,
            _ => {
                return false;
            }
        };

        if matches!(
            action,
            SshModalAction::Connect
                | SshModalAction::TestConnection
                | SshModalAction::SaveAndConnect
        ) && !self.ssh_modal_connect_family_enabled()
        {
            return false;
        }

        self.pending_ssh_modal_action = Some(PendingSshModalAction { action, draft });
        self.ssh_modal_action_state = SshModalActionState::Busy(action);
        true
    }

    pub fn can_confirm_asset_modal(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder { .. })
            | Some(AssetModalState::SftpNewFolder { .. })
            | Some(AssetModalState::NewSnippet { .. })
            | Some(AssetModalState::NewSnippetPackage { .. })
            | Some(AssetModalState::NewKeychainSshKey { .. })
            | Some(AssetModalState::NewSshConnection { .. }) => {
                self.asset_create_modal_can_confirm()
            }
            Some(AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            }) => {
                self.rename_asset_modal_validation(asset_id, draft_name)
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::SftpRenameEntry {
                entry_id,
                draft_name,
                ..
            }) => {
                self.sftp_name_validation(draft_name, Some(entry_id.as_str()))
                    == AssetNameValidation::Valid
            }
            Some(AssetModalState::SftpDeleteEntriesConfirm { .. }) => true,
            Some(AssetModalState::DeleteAssetConfirm { .. }) => true,
            None => false,
        }
    }

    pub fn confirm_asset_modal(&mut self) -> bool {
        let Some(modal_state) = self.asset_modal_state.clone() else {
            return false;
        };

        let (parent_id, kind, draft_label, payload) = match modal_state {
            AssetModalState::NewFolder {
                parent_id,
                draft_name,
            } => (
                parent_id,
                ConsoleAssetKind::Folder,
                draft_name,
                AssetNodePayload::Folder,
            ),
            AssetModalState::SftpNewFolder { draft_name } => {
                if self.sftp_name_validation(&draft_name, None) != AssetNameValidation::Valid {
                    return false;
                }

                let Some(session_id) = self.active_workspace_session_id().map(str::to_string)
                else {
                    return false;
                };
                let path = sftp_child_path(self.sftp_panel_path().as_str(), draft_name.trim());
                let entry_id = format!("sftp-dir-{}", path);
                let next_entry = SftpDirectoryEntry {
                    id: entry_id.clone(),
                    name: draft_name.trim().to_string(),
                    path,
                    kind: crate::app::sftp::SftpDirectoryEntryKind::Directory,
                    size_bytes: None,
                };

                if let Some(state) = self.sftp_sessions.get_mut(&session_id) {
                    state.entries.push(next_entry);
                    state.selected_entry_ids = vec![entry_id.clone()];
                }
                self.context_target_asset_id = Some(entry_id);
                self.asset_modal_state = None;
                return true;
            }
            AssetModalState::NewSnippet {
                parent_package_id,
                editing_asset_id,
                draft,
            } => {
                if !self
                    .snippet_modal_validation_message(
                        parent_package_id.as_deref(),
                        editing_asset_id.as_deref(),
                        &draft,
                    )
                    .is_empty()
                {
                    return false;
                }

                let resolved_parent_id =
                    self.resolve_snippet_package_id_by_label(draft.package.trim());
                if let Some(asset_id) = editing_asset_id {
                    self.snippet_asset_tree
                        .set_title(&asset_id, draft.name.trim().to_string());
                    if !self.snippet_asset_tree.set_snippet_spec(
                        &asset_id,
                        crate::shell::assets::AssetSnippetSpec {
                            script: draft.script,
                            package_id: resolved_parent_id,
                        },
                    ) {
                        return false;
                    }
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    return true;
                }

                (
                    resolved_parent_id.clone(),
                    ConsoleAssetKind::Snippet,
                    draft.name,
                    AssetNodePayload::Snippet(crate::shell::assets::AssetSnippetSpec {
                        script: draft.script,
                        package_id: resolved_parent_id.clone(),
                    }),
                )
            }
            AssetModalState::NewSnippetPackage {
                editing_asset_id,
                draft_name,
            } => {
                if self.snippet_asset_tree.validate_name_in_parent(
                    None,
                    &draft_name,
                    editing_asset_id.as_deref(),
                ) != AssetNameValidation::Valid
                {
                    return false;
                }

                if let Some(asset_id) = editing_asset_id {
                    self.snippet_asset_tree
                        .set_title(&asset_id, draft_name.trim().to_string());
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    return true;
                }

                (
                    None,
                    ConsoleAssetKind::SnippetPackage,
                    draft_name,
                    AssetNodePayload::SnippetPackage,
                )
            }
            AssetModalState::NewKeychainSshKey { parent_id, draft } => {
                if !self.keychain_ssh_key_modal_can_confirm(parent_id.as_deref(), &draft) {
                    return false;
                }

                let item_id = create_keychain_node(
                    &mut self.keychain_catalog,
                    parent_id.as_deref(),
                    KeychainItemKind::SshKey,
                    Some(draft.name.trim()),
                );
                let trimmed_public_key = draft.public_key.trim().to_string();
                let credential_ref = (!draft.private_key.trim().is_empty())
                    .then(|| keychain_key_credential_ref(item_id.as_str()));
                let comment = trimmed_public_key
                    .split_whitespace()
                    .nth(2)
                    .unwrap_or_default()
                    .to_string();
                let algorithm = trimmed_public_key
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string();
                let spec = KeychainSshKeySpec {
                    algorithm,
                    fingerprint: draft.fingerprint.trim().to_string(),
                    public_key: trimmed_public_key,
                    comment,
                    credential_ref,
                    remark: String::new(),
                };
                if let Some(node) = self.keychain_catalog.nodes.get_mut(&item_id) {
                    node.payload = KeychainNodePayload::SshKey(spec);
                }
                if let Some(parent_id) = parent_id {
                    self.keychain_expanded_ids.insert(parent_id);
                }
                self.selected_keychain_ids = vec![item_id.clone()];
                self.focused_keychain_id = Some(item_id);
                self.asset_modal_state = None;
                self.pending_ssh_modal_action = None;
                self.ssh_modal_action_state = SshModalActionState::Idle;
                return true;
            }
            AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            } => {
                if self
                    .ssh_modal_submit_validation_message(
                        parent_id.as_deref(),
                        editing_asset_id.as_deref(),
                        &draft,
                    )
                    .is_some()
                {
                    return false;
                }

                let label = draft.name.trim().to_string();
                if let Some(asset_id) = editing_asset_id {
                    let existing_spec = self.console_asset_tree.ssh_connection_spec(&asset_id);
                    let payload = build_saved_ssh_connection_spec(&asset_id, &draft, existing_spec);

                    self.console_asset_tree
                        .set_title(&asset_id, label.trim().to_string());
                    if !self
                        .console_asset_tree
                        .set_ssh_connection_spec(&asset_id, payload)
                    {
                        return false;
                    }
                    self.selected_asset_ids = vec![asset_id.clone()];
                    self.focused_asset_id = Some(asset_id.clone());
                    self.context_target_asset_id = Some(asset_id);
                    self.asset_modal_state = None;
                    self.pending_ssh_modal_action = None;
                    self.ssh_modal_action_state = SshModalActionState::Idle;
                    return true;
                }

                let payload = AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                    host: draft.host,
                    user: draft.user,
                    port: draft.port,
                    auth_method: draft.auth_method,
                    auth_source: SSH_AUTH_SOURCE_MANUAL.into(),
                    keychain_identity_id: None,
                    private_key_source: draft.private_key_source,
                    private_key_path: draft.private_key_path,
                    environment: draft.environment,
                    proxy: AssetSshProxySpec::None,
                    proxy_method: draft.proxy_method,
                    remark: draft.remark,
                    credential_ref: None,
                });
                (parent_id, ConsoleAssetKind::SshConnection, label, payload)
            }
            AssetModalState::SftpRenameEntry {
                entry_id,
                draft_name,
                ..
            } => {
                if self.sftp_name_validation(&draft_name, Some(entry_id.as_str()))
                    != AssetNameValidation::Valid
                {
                    return false;
                }

                let current_path = self.sftp_panel_path();
                let next_name = draft_name.trim().to_string();
                if let Some(state) = self.active_sftp_session_state_mut()
                    && let Some(entry) = state.entries.iter_mut().find(|entry| entry.id == entry_id)
                {
                    entry.name = next_name.clone();
                    entry.path = sftp_child_path(current_path.as_str(), next_name.as_str());
                    state.selected_entry_ids = vec![entry.id.clone()];
                    self.context_target_asset_id = Some(entry.id.clone());
                    self.asset_modal_state = None;
                    return true;
                }

                return false;
            }
            AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            } => {
                if !self.can_confirm_asset_modal() {
                    return false;
                }

                self.console_asset_tree
                    .set_title(&asset_id, draft_name.trim().to_string());
                self.focused_asset_id = Some(asset_id.clone());
                self.selected_asset_ids = vec![asset_id.clone()];
                self.context_target_asset_id = Some(asset_id);
                self.asset_modal_state = None;
                return true;
            }
            AssetModalState::DeleteAssetConfirm { .. } => {
                return self.confirm_delete_asset();
            }
            AssetModalState::SftpDeleteEntriesConfirm { .. } => {
                return self.confirm_delete_asset();
            }
        };

        let use_snippet_tree = kind.domain() == crate::shell::assets::AssetDomain::Snippets;
        let label = if draft_label.trim().is_empty() {
            if use_snippet_tree {
                self.snippet_asset_tree
                    .next_default_name_for_parent(parent_id.as_deref(), kind)
            } else {
                self.console_asset_tree
                    .next_default_name_for_parent(parent_id.as_deref(), kind)
            }
        } else {
            let validation = if use_snippet_tree {
                self.snippet_asset_tree.validate_name_in_parent(
                    parent_id.as_deref(),
                    &draft_label,
                    None,
                )
            } else {
                self.create_asset_modal_validation(parent_id.as_deref(), &draft_label)
            };
            if validation != AssetNameValidation::Valid {
                return false;
            }
            draft_label.trim().to_string()
        };
        let asset_id = if use_snippet_tree {
            if let Some(parent_id) = parent_id.as_deref() {
                let asset_id = self
                    .snippet_asset_tree
                    .insert_child_with_payload(parent_id, kind, label, payload);
                self.snippet_asset_tree.set_expanded(parent_id, true);
                asset_id
            } else {
                self.snippet_asset_tree
                    .insert_root_with_payload(kind, label, payload)
            }
        } else {
            if let Some(parent_id) = parent_id.as_deref() {
                let asset_id = self
                    .console_asset_tree
                    .insert_child_with_payload(parent_id, kind, label, payload);
                self.console_asset_tree.set_expanded(parent_id, true);
                asset_id
            } else {
                self.console_asset_tree
                    .insert_root_with_payload(kind, label, payload)
            }
        };

        if let Some(AssetModalState::NewSshConnection { draft, .. }) = &self.asset_modal_state {
            let payload = build_saved_ssh_connection_spec(&asset_id, draft, None);
            let _ = self
                .console_asset_tree
                .set_ssh_connection_spec(&asset_id, payload);
        }

        self.selected_asset_ids = vec![asset_id.clone()];
        self.focused_asset_id = Some(asset_id.clone());
        self.context_target_asset_id = Some(asset_id);
        self.asset_modal_state = None;
        self.pending_ssh_modal_action = None;
        self.ssh_modal_action_state = SshModalActionState::Idle;
        true
    }

    pub fn confirm_delete_asset(&mut self) -> bool {
        if let Some(AssetModalState::SftpDeleteEntriesConfirm { entry_ids, .. }) =
            self.asset_modal_state.clone()
        {
            let should_clear_context_target = self
                .context_target_asset_id
                .as_deref()
                .is_some_and(|entry_id| entry_ids.iter().any(|removed_id| removed_id == entry_id));
            let Some(state) = self.active_sftp_session_state_mut() else {
                return false;
            };

            let before_len = state.entries.len();
            state
                .entries
                .retain(|entry| !entry_ids.iter().any(|id| id == &entry.id));
            state
                .selected_entry_ids
                .retain(|selected_id| !entry_ids.iter().any(|entry_id| entry_id == selected_id));
            if state.entries.len() == before_len {
                return false;
            }

            if should_clear_context_target {
                self.context_target_asset_id = state.selected_entry_ids.first().cloned();
            }
            self.asset_modal_state = None;
            return true;
        }

        let Some(AssetModalState::DeleteAssetConfirm { asset_id, .. }) =
            self.asset_modal_state.clone()
        else {
            return false;
        };

        let snippet_first = self.active_sidebar_destination == SidebarDestination::Snippets;
        let removed = if snippet_first {
            if self.snippet_asset_tree.remove_subtree(&asset_id).is_some() {
                self.selected_asset_ids.clear();
                self.focused_asset_id = None;
                self.context_target_asset_id = None;
                true
            } else {
                self.remove_asset_subtree(&asset_id)
            }
        } else {
            if self.remove_asset_subtree(&asset_id) {
                true
            } else if self.snippet_asset_tree.remove_subtree(&asset_id).is_some() {
                self.selected_asset_ids.clear();
                self.focused_asset_id = None;
                self.context_target_asset_id = None;
                true
            } else {
                false
            }
        };
        if removed {
            self.asset_modal_state = None;
            self.pending_snippet_activation = None;
            return true;
        }

        false
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

    pub fn visible_console_asset_rows(&self) -> Vec<VisibleAssetRow> {
        self.console_asset_tree
            .project_visible_rows(self.asset_view_mode, &self.asset_search_query)
    }

    pub fn visible_snippet_rows(&self) -> Vec<VisibleAssetRow> {
        self.snippet_asset_tree
            .project_visible_rows(self.asset_view_mode, &self.asset_search_query)
    }

    pub fn visible_keychain_rows(&self) -> Vec<crate::shell::keychain::VisibleKeychainRow> {
        project_keychain_rows(
            &self.keychain_catalog,
            &self.keychain_expanded_ids,
            &self.keychain_search_query,
        )
    }

    pub fn handle_assets_create_action(&mut self, action_id: &str) {
        match action_id {
            "new-folder" if self.active_sidebar_destination == SidebarDestination::Keychain => {
                self.create_keychain_item(None, KeychainItemKind::Folder);
            }
            "new-identity" => {
                self.create_keychain_item(None, KeychainItemKind::Identity);
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
            return;
        }

        if current.children.is_empty() {
            self.handle_context_menu_leaf_action(current.id);
        } else if current.state == ContextMenuActionState::Planned {
            self.set_context_menu_feedback(format!("{} is not wired yet.", current.label));
        }
    }

    pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {
        if self
            .context_menu_target_kind
            .is_some_and(is_sftp_context_target)
        {
            self.handle_sftp_context_menu_leaf_action(action_id);
            return;
        }

        if matches!(
            action_id,
            "new-snippet" | "new-package" | "new-snippet-package"
        ) {
            match action_id {
                "new-snippet" => {
                    let parent_id = match (
                        self.context_menu_target_kind,
                        self.context_target_asset_id.as_deref(),
                    ) {
                        (Some(ContextTargetKind::SnippetPackage), Some(asset_id))
                            if self.snippet_asset_tree.contains(asset_id) =>
                        {
                            Some(asset_id.to_string())
                        }
                        _ => None,
                    };
                    self.open_new_snippet_modal(parent_id);
                }
                "new-package" | "new-snippet-package" => self.open_new_snippet_package_modal(),
                _ => {}
            }
            return;
        }

        if ConsoleAssetKind::from_create_action_id(action_id).is_some() {
            let parent_id = match (
                self.context_menu_target_kind,
                self.context_target_asset_id.as_deref(),
            ) {
                (Some(ContextTargetKind::Folder), Some(asset_id))
                    if self.console_asset_tree.contains(asset_id) =>
                {
                    Some(asset_id.to_string())
                }
                _ => None,
            };

            match action_id {
                "new-folder" => self.open_new_folder_modal(parent_id),
                "new-ssh-connection" => self.open_new_ssh_modal(parent_id),
                _ => {}
            }
            return;
        }

        let roots = self.context_menu_roots();
        let Some(action) = find_action_node_by_id(&roots, action_id) else {
            return;
        };

        match action.state {
            ContextMenuActionState::Planned => {
                self.set_context_menu_feedback(format!("{} is not wired yet.", action.label));
            }
            ContextMenuActionState::Enabled => match action_id {
                "edit-connection" => {
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.console_asset_tree.contains(asset_id))
                    {
                        self.open_edit_ssh_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "rename-asset" => {
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.console_asset_tree.contains(asset_id))
                    {
                        if self.console_asset_tree.kind(&asset_id)
                            == Some(ConsoleAssetKind::SshConnection)
                        {
                            self.open_edit_ssh_modal(asset_id);
                        } else {
                            self.open_rename_asset_modal(asset_id);
                        }
                    } else {
                        self.close_context_menu();
                    }
                }
                "delete-asset" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.console_asset_tree.contains(asset_id)
                                || self.snippet_asset_tree.contains(asset_id)
                        })
                    {
                        self.open_delete_asset_confirm(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.open_edit_snippet_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "edit-package" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::SnippetPackage)
                        })
                    {
                        self.open_edit_snippet_package_modal(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "delete-snippet" | "delete-package" => {
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.snippet_asset_tree.contains(asset_id))
                    {
                        self.open_delete_asset_confirm(asset_id);
                    } else {
                        self.close_context_menu();
                    }
                }
                "paste-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.begin_snippet_activation(&asset_id, SnippetActivation::Paste);
                        self.close_context_menu();
                    } else {
                        self.close_context_menu();
                    }
                }
                "run-snippet" => {
                    if let Some(asset_id) =
                        self.context_target_asset_id.clone().filter(|asset_id| {
                            self.snippet_asset_tree.kind(asset_id)
                                == Some(ConsoleAssetKind::Snippet)
                        })
                    {
                        self.begin_snippet_activation(&asset_id, SnippetActivation::Run);
                        self.close_context_menu();
                    } else {
                        self.close_context_menu();
                    }
                }
                _ => self.close_context_menu(),
            },
            ContextMenuActionState::Disabled => {}
        }
    }

    pub fn set_context_menu_feedback(&mut self, text: impl Into<String>) {
        self.context_menu_feedback_text = text.into();
    }

    fn handle_sftp_context_menu_leaf_action(&mut self, action_id: &str) {
        match action_id {
            "new-folder" => self.open_sftp_new_folder_modal(),
            "rename-sftp-entry" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.open_sftp_rename_entry_modal(entry_id);
                } else {
                    self.close_context_menu();
                }
            }
            "delete-sftp-entry" => {
                if let Some(entry_id) = self.context_target_asset_id.clone() {
                    self.open_sftp_delete_confirm(vec![entry_id]);
                } else {
                    self.close_context_menu();
                }
            }
            "delete-selected" => {
                let entry_ids = self.sftp_panel_selected_entry_ids().to_vec();
                self.open_sftp_delete_confirm(entry_ids);
            }
            "refresh-sftp" => {
                let _ = self.refresh_sftp_panel();
                self.close_context_menu();
            }
            _ => self.close_context_menu(),
        }
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

    pub fn seed_test_asset(&mut self, kind: ConsoleAssetKind, label: impl Into<String>) {
        self.console_asset_tree.insert_root(kind, label);
    }

    pub fn replace_console_asset_tree(&mut self, tree: AssetTree) {
        self.console_asset_tree = tree;
        self.asset_tree_fully_expanded = false;
        self.selected_asset_ids.clear();
        self.focused_asset_id = None;
        if self
            .quick_launch_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| !self.console_asset_tree.contains(asset_id))
        {
            self.quick_launch_selected_asset_id = None;
        }
        if self
            .quick_launch_active_group_id
            .as_deref()
            .is_some_and(|group_id| !self.console_asset_tree.contains(group_id))
        {
            self.quick_launch_active_group_id = None;
        }
        if self
            .saved_ssh_picker_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| !self.console_asset_tree.contains(asset_id))
        {
            self.saved_ssh_picker_selected_asset_id = None;
        }
        self.clear_active_asset_rename_session();
        self.context_target_asset_id = None;
        self.close_context_menu();
        self.ensure_quick_launch_selection();
    }

    pub fn console_asset_tree(&self) -> &AssetTree {
        &self.console_asset_tree
    }

    pub fn snippet_asset_tree(&self) -> &AssetTree {
        &self.snippet_asset_tree
    }

    pub fn asset_kind(&self, asset_id: &str) -> Option<ConsoleAssetKind> {
        match self.active_sidebar_destination {
            SidebarDestination::Snippets => self
                .snippet_asset_tree
                .kind(asset_id)
                .or_else(|| self.console_asset_tree.kind(asset_id)),
            SidebarDestination::Console | SidebarDestination::Keychain => self
                .console_asset_tree
                .kind(asset_id)
                .or_else(|| self.snippet_asset_tree.kind(asset_id)),
        }
    }

    pub fn replace_snippet_asset_tree(&mut self, tree: AssetTree) {
        self.snippet_asset_tree = tree;
        self.pending_snippet_create_action = None;
        self.pending_snippet_activation = None;
    }

    fn quick_launch_records(&self) -> Vec<QuickLaunchAssetRecord> {
        collect_quick_launch_records(&self.console_asset_tree)
    }

    fn matching_quick_launch_records(&self) -> Vec<QuickLaunchAssetRecord> {
        self.quick_launch_records()
            .into_iter()
            .filter(|record| {
                matches_quick_launch_query(record, self.quick_launch_search_query.as_str())
            })
            .collect()
    }

    fn ordered_quick_launch_cards_from_ids(
        &self,
        ids: &[String],
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<QuickLaunchCardItem> {
        self.ordered_quick_launch_asset_ids_from_preferences(ids, records)
            .into_iter()
            .filter_map(|asset_id| {
                records
                    .iter()
                    .find(|record| record.asset_id == asset_id)
                    .map(|record| {
                        project_card_item(record, self.is_quick_launch_favorite(asset_id.as_str()))
                    })
            })
            .collect()
    }

    fn ordered_quick_launch_asset_ids_from_preferences(
        &self,
        ids: &[String],
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<String> {
        let mut ordered = Vec::new();
        let mut seen = BTreeSet::new();

        for asset_id in ids {
            if !seen.insert(asset_id.clone()) {
                continue;
            }
            if records.iter().any(|record| record.asset_id == *asset_id) {
                ordered.push(asset_id.clone());
            }
        }

        ordered
    }

    fn visible_group_records(
        &self,
        records: &[QuickLaunchAssetRecord],
    ) -> Vec<QuickLaunchAssetRecord> {
        let Some(group_id) = self.active_quick_launch_group_id_for_records(records) else {
            return records.to_vec();
        };

        records
            .iter()
            .filter(|record| {
                record
                    .group
                    .as_ref()
                    .is_some_and(|group| group.id == group_id)
            })
            .cloned()
            .collect()
    }

    fn active_quick_launch_group_id_for_records<'a>(
        &'a self,
        records: &[QuickLaunchAssetRecord],
    ) -> Option<&'a str> {
        self.quick_launch_active_group_id
            .as_deref()
            .filter(|group_id| {
                records.iter().any(|record| {
                    record
                        .group
                        .as_ref()
                        .is_some_and(|group| group.id == *group_id)
                })
            })
    }

    fn visible_asset_ids_from_records(&self, records: &[QuickLaunchAssetRecord]) -> Vec<String> {
        self.visible_group_records(records)
            .into_iter()
            .map(|record| record.asset_id)
            .collect()
    }

    fn first_visible_quick_launch_asset_id_from_records(
        &self,
        records: &[QuickLaunchAssetRecord],
    ) -> Option<String> {
        self.ordered_quick_launch_asset_ids_from_preferences(
            &self.quick_launch_preferences.recent_asset_ids,
            records,
        )
        .into_iter()
        .next()
        .or_else(|| {
            self.ordered_quick_launch_asset_ids_from_preferences(
                &self.quick_launch_preferences.favorite_asset_ids,
                records,
            )
            .into_iter()
            .next()
        })
        .or_else(|| {
            self.visible_group_records(records)
                .into_iter()
                .map(|record| record.asset_id)
                .next()
        })
    }

    fn is_quick_launch_favorite(&self, asset_id: &str) -> bool {
        self.quick_launch_preferences
            .favorite_asset_ids
            .iter()
            .any(|favorite_id| favorite_id == asset_id)
    }

    fn quick_launch_recent_label(&self, asset_id: &str) -> String {
        self.quick_launch_preferences
            .recent_asset_ids
            .iter()
            .position(|recent_id| recent_id == asset_id)
            .map(|index| format!("Recent #{}", index + 1))
            .unwrap_or_default()
    }

    fn sync_quick_launch_group_from_selected(&mut self) {
        self.quick_launch_active_group_id = self
            .quick_launch_selected_asset_id
            .as_deref()
            .and_then(|asset_id| group_id_for_asset(&self.console_asset_tree, asset_id));
    }

    fn first_saved_ssh_picker_asset_id(&self) -> Option<String> {
        self.saved_ssh_picker_items()
            .into_iter()
            .find(|item| item.kind == ConsoleAssetKind::SshConnection.id())
            .map(|item| item.id)
    }

    fn collect_saved_ssh_picker_rows(
        &self,
        node_id: &str,
        depth: usize,
        query_active: bool,
        rows: &mut Vec<SavedSshPickerItem>,
    ) -> bool {
        let Some(node) = self.console_asset_tree.node(node_id) else {
            return false;
        };

        match node.kind {
            ConsoleAssetKind::SshConnection => {
                if !self.saved_ssh_picker_matches(node_id) {
                    return false;
                }

                rows.push(SavedSshPickerItem {
                    id: node.id.clone(),
                    kind: node.kind.id().into(),
                    label: node.title.clone(),
                    depth,
                    has_children: false,
                    expanded: false,
                    selected: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    focused: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    disclosure_state: "none".into(),
                    path_hint: String::new(),
                    compact_flat_mode: false,
                });
                true
            }
            ConsoleAssetKind::Folder => {
                let mut child_rows = Vec::new();
                let mut has_matching_descendants = false;
                for child_id in &node.children {
                    has_matching_descendants |= self.collect_saved_ssh_picker_rows(
                        child_id,
                        depth + 1,
                        query_active,
                        &mut child_rows,
                    );
                }

                if !has_matching_descendants {
                    return false;
                }

                rows.push(SavedSshPickerItem {
                    id: node.id.clone(),
                    kind: node.kind.id().into(),
                    label: node.title.clone(),
                    depth,
                    has_children: true,
                    expanded: query_active || node.expanded,
                    selected: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    focused: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    disclosure_state: if query_active || node.expanded {
                        "expanded".into()
                    } else {
                        "collapsed".into()
                    },
                    path_hint: String::new(),
                    compact_flat_mode: false,
                });

                if query_active || node.expanded {
                    rows.extend(child_rows);
                }
                true
            }
            ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => false,
        }
    }

    fn saved_ssh_picker_matches(&self, node_id: &str) -> bool {
        let Some(spec) = self.console_asset_tree.ssh_connection_spec(node_id) else {
            return false;
        };

        let query = self.saved_ssh_picker_query.trim().to_ascii_lowercase();
        if query.is_empty() {
            return true;
        }

        let title = self.console_asset_tree.title(node_id).unwrap_or_default();
        [
            title,
            spec.host.as_str(),
            spec.user.as_str(),
            spec.environment.as_str(),
            spec.remark.as_str(),
        ]
        .into_iter()
        .any(|value| value.to_ascii_lowercase().contains(&query))
    }

    pub fn replace_vault_projection(
        &mut self,
        console_tree: AssetTree,
        snippet_tree: AssetTree,
        keychain_catalog: KeychainCatalog,
    ) {
        self.replace_console_asset_tree(console_tree);
        self.replace_snippet_asset_tree(snippet_tree);
        self.replace_keychain_catalog(keychain_catalog);
    }

    pub fn clear_vault_projection(&mut self) {
        self.replace_vault_projection(
            AssetTree::new(),
            AssetTree::new(),
            KeychainCatalog::default(),
        );
    }

    pub fn keychain_catalog(&self) -> &KeychainCatalog {
        &self.keychain_catalog
    }

    pub fn replace_keychain_catalog(&mut self, catalog: KeychainCatalog) {
        self.keychain_expanded_ids = catalog
            .root_ids
            .iter()
            .filter(|node_id| {
                catalog.nodes.get(*node_id).is_some_and(|node| {
                    matches!(
                        node.payload,
                        crate::app::keychain::KeychainNodePayload::Folder
                    )
                })
            })
            .cloned()
            .collect();
        self.keychain_catalog = catalog;
        self.selected_keychain_ids.clear();
        self.focused_keychain_id = None;
        self.keychain_search_query.clear();
    }

    pub fn set_keychain_search_query(&mut self, query: String) {
        self.keychain_search_query = query;
    }

    pub fn toggle_keychain_folder_expanded(&mut self, item_id: &str) {
        if !self
            .keychain_catalog
            .nodes
            .get(item_id)
            .is_some_and(|node| {
                matches!(
                    node.payload,
                    crate::app::keychain::KeychainNodePayload::Folder
                )
            })
        {
            return;
        }

        if !self.keychain_expanded_ids.insert(item_id.to_string()) {
            self.keychain_expanded_ids.remove(item_id);
        }
    }

    pub fn select_keychain_item(&mut self, item_id: &str) {
        if !self.keychain_catalog.nodes.contains_key(item_id) {
            return;
        }

        self.selected_keychain_ids = vec![item_id.to_string()];
        self.focused_keychain_id = Some(item_id.to_string());
        self.asset_create_menu_open = false;
    }

    pub fn create_keychain_item(
        &mut self,
        parent_id: Option<String>,
        kind: KeychainItemKind,
    ) -> String {
        let parent_id = self.normalize_keychain_folder_parent_id(parent_id);
        let item_id =
            create_keychain_node(&mut self.keychain_catalog, parent_id.as_deref(), kind, None);
        if let Some(parent_id) = parent_id {
            self.keychain_expanded_ids.insert(parent_id);
        }
        self.selected_keychain_ids = vec![item_id.clone()];
        self.focused_keychain_id = Some(item_id.clone());
        self.asset_create_menu_open = false;
        item_id
    }

    pub fn rename_keychain_item(&mut self, item_id: &str, title: &str) -> anyhow::Result<()> {
        rename_keychain_node(&mut self.keychain_catalog, item_id, title)
    }

    pub fn delete_keychain_item(&mut self, item_id: &str) -> Result<bool, KeychainDeleteError> {
        let removed = delete_keychain_node(
            &mut self.keychain_catalog,
            item_id,
            &self.console_asset_tree,
        )?;
        if removed.removed_ids.is_empty() {
            return Ok(false);
        }

        self.selected_keychain_ids.retain(|selected_id| {
            !removed
                .removed_ids
                .iter()
                .any(|removed_id| removed_id == selected_id)
        });
        if self
            .focused_keychain_id
            .as_deref()
            .is_some_and(|focused_id| {
                removed
                    .removed_ids
                    .iter()
                    .any(|removed_id| removed_id == focused_id)
            })
        {
            self.focused_keychain_id = None;
        }
        Ok(true)
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

    fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {
        let Some(target_kind) = self.context_menu_target_kind else {
            return Vec::new();
        };

        resolve_action_tree(target_kind, &self.context_menu_selection())
    }

    pub fn context_menu_selection(&self) -> SelectionContext {
        if self
            .context_menu_target_kind
            .is_some_and(is_sftp_context_target)
        {
            return SelectionContext {
                selected_ids: self.sftp_panel_selected_entry_ids().to_vec(),
                clipboard_has_asset_payload: false,
                target_mutable: matches!(self.sftp_panel_mode_id(), "ready"),
            };
        }

        SelectionContext {
            selected_ids: self.selected_asset_ids.clone(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
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
        let active_id = self
            .active_workspace_session_id
            .as_deref()
            .filter(|candidate| {
                self.workspace_tabs
                    .iter()
                    .any(|tab| tab.session_id == *candidate)
            })
            .map(str::to_string)
            .or_else(|| {
                self.workspace_tabs
                    .iter()
                    .find(|tab| tab.active)
                    .map(|tab| tab.session_id.clone())
            })
            .or_else(|| {
                self.workspace_tabs
                    .first()
                    .map(|tab| tab.session_id.clone())
            });

        for tab in &mut self.workspace_tabs {
            tab.active = active_id.as_deref() == Some(tab.session_id.as_str());
        }

        self.active_workspace_session_id = active_id;
        if self.active_workspace_terminal_surface().is_none() {
            self.active_workspace_terminal_surface = None;
        }
        self.show_welcome = self.workspace_tabs.is_empty();
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

    fn keychain_ssh_key_modal_validation_message(
        &self,
        parent_id: Option<&str>,
        draft: &KeychainSshKeyDraft,
    ) -> String {
        asset_name_validation_message(self.keychain_name_validation(parent_id, &draft.name, None))
    }

    fn keychain_ssh_key_modal_can_confirm(
        &self,
        parent_id: Option<&str>,
        draft: &KeychainSshKeyDraft,
    ) -> bool {
        self.keychain_name_validation(parent_id, &draft.name, None) == AssetNameValidation::Valid
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

fn build_saved_ssh_connection_spec(
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
        .then(|| saved_ssh_credential_ref(asset_id, existing_spec));
    let credential_ref = if uses_saved_auth_secret || uses_saved_proxy_secret {
        saved_secret_ref.clone()
    } else {
        None
    };
    let mut proxy = build_draft_proxy_spec(draft);
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

fn build_draft_proxy_spec(draft: &AssetSshConnectionDraft) -> AssetSshProxySpec {
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

fn saved_ssh_credential_ref(
    asset_id: &str,
    existing_spec: Option<&AssetSshConnectionSpec>,
) -> String {
    existing_spec
        .and_then(|spec| spec.credential_ref.clone())
        .unwrap_or_else(|| ssh_credential_ref(asset_id, SshCredentialKind::SavedSecrets))
}

pub fn welcome_actions() -> &'static [WelcomeAction] {
    &[
        WelcomeAction::NewConnection,
        WelcomeAction::OpenRecent,
        WelcomeAction::Snippets,
        WelcomeAction::Sftp,
    ]
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
