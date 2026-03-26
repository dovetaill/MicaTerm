//! Central shell state mirrored into Slint properties and mutated by UI callbacks.

use crate::app::ssh::credentials::{SshCredentialKind, ssh_credential_ref};
use crate::app::ssh::runtime::TerminalSurfaceState;
use crate::app::window_state::WindowPlacementKind;
use crate::shell::assets::{
    AssetNameValidation, AssetNodePayload, AssetSshConnectionSpec, AssetTree, AssetViewMode,
    ConsoleAssetKind, VisibleAssetRow, resolve_committed_name,
};
use crate::shell::context_menu::{
    ContextMenuActionNode, ContextMenuActionState, ContextTargetKind, SelectionContext,
    resolve_action_tree,
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

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetModalState {
    NewFolder {
        parent_id: Option<String>,
        draft_name: String,
    },
    NewSshConnection {
        parent_id: Option<String>,
        editing_asset_id: Option<String>,
        draft: AssetSshConnectionDraft,
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
    pub auth_method: String,
    pub private_key_source: String,
    pub password: String,
    pub private_key_content: String,
    pub private_key_path: String,
    pub passphrase: String,
    pub password_visible: bool,
    pub remark: String,
    pub environment: String,
    pub proxy_method: String,
    pub validation_message: String,
}

impl Default for AssetSshConnectionDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            host: String::new(),
            user: String::new(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            password: String::new(),
            private_key_content: String::new(),
            private_key_path: String::new(),
            passphrase: String::new(),
            password_visible: false,
            remark: String::new(),
            environment: String::new(),
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

#[derive(Debug, Clone, PartialEq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_global_menu: bool,
    pub show_assets_sidebar: bool,
    pub active_sidebar_destination: SidebarDestination,
    pub is_window_active: bool,
    pub theme_mode: ThemeMode,
    pub is_always_on_top: bool,
    pub asset_view_mode: AssetViewMode,
    pub asset_search_expanded: bool,
    pub asset_search_query: String,
    pub asset_create_menu_open: bool,
    pub asset_modal_state: Option<AssetModalState>,
    pub ssh_host_key_prompt_state: Option<SshHostKeyPromptState>,
    pub asset_tree_fully_expanded: bool,
    pub selected_asset_ids: Vec<String>,
    pub focused_asset_id: Option<String>,
    workspace_tabs: Vec<WorkspaceTab>,
    active_workspace_session_id: Option<String>,
    active_workspace_terminal_surface: Option<TerminalSurfaceState>,
    pending_ssh_modal_action: Option<PendingSshModalAction>,
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
    console_asset_tree: AssetTree,
    window_placement: WindowPlacementKind,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_global_menu: false,
            show_assets_sidebar: true,
            active_sidebar_destination: SidebarDestination::Console,
            is_window_active: true,
            theme_mode: ThemeMode::Dark,
            is_always_on_top: false,
            asset_view_mode: AssetViewMode::Tree,
            asset_search_expanded: false,
            asset_search_query: String::new(),
            asset_create_menu_open: false,
            asset_modal_state: None,
            ssh_host_key_prompt_state: None,
            asset_tree_fully_expanded: false,
            selected_asset_ids: Vec::new(),
            focused_asset_id: None,
            workspace_tabs: Vec::new(),
            active_workspace_session_id: None,
            active_workspace_terminal_surface: None,
            pending_ssh_modal_action: None,
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
            console_asset_tree: AssetTree::new(),
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

    pub fn asset_rename_modal_validation_message(&self) -> String {
        match &self.asset_modal_state {
            Some(AssetModalState::RenameAsset {
                asset_id,
                draft_name,
                ..
            }) => asset_name_validation_message(
                self.rename_asset_modal_validation(asset_id, draft_name),
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
            Some(AssetModalState::NewSshConnection {
                parent_id,
                editing_asset_id,
                draft,
                ..
            }) => {
                self.ssh_modal_can_confirm(parent_id.as_deref(), editing_asset_id.as_deref(), draft)
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
            Some(tab) if tab.uses_terminal_surface() => "terminal",
            Some(_) => "session-error",
        }
    }

    pub fn toggle_right_panel(&mut self) {
        self.show_right_panel = !self.show_right_panel;
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

    pub fn update_new_folder_modal_name(&mut self, value: String) {
        let Some(AssetModalState::NewFolder { draft_name, .. }) = self.asset_modal_state.as_mut()
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
        let private_key_source = if spec.private_key_source.trim().is_empty() {
            "content".to_string()
        } else {
            spec.private_key_source.clone()
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
                auth_method,
                private_key_source,
                password: String::new(),
                private_key_content: String::new(),
                private_key_path: spec.private_key_path,
                passphrase: String::new(),
                password_visible: false,
                remark: spec.remark,
                environment: spec.environment,
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

    pub fn update_rename_asset_modal_name(&mut self, value: String) {
        let Some(AssetModalState::RenameAsset { draft_name, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };

        *draft_name = value;
    }

    pub fn open_delete_asset_confirm(&mut self, asset_id: String) {
        if !self.console_asset_tree.contains(&asset_id) {
            return;
        }

        let Some(label) = self.console_asset_tree.title(&asset_id).map(str::to_string) else {
            return;
        };
        let Some(descendant_count) = self.console_asset_tree.descendant_count(&asset_id) else {
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

    pub fn update_ssh_modal_field(&mut self, field: &str, value: String) {
        let Some(AssetModalState::NewSshConnection { draft, .. }) = self.asset_modal_state.as_mut()
        else {
            return;
        };
        match field {
            "name" => draft.name = value,
            "host" => draft.host = value,
            "user" => draft.user = value,
            "port" => draft.port = value,
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
            "private_key_content" => draft.private_key_content = value,
            "private_key_path" => draft.private_key_path = value,
            "passphrase" => draft.passphrase = value,
            "password_visibility" => {
                draft.password_visible = matches!(value.as_str(), "visible" | "show" | "true");
            }
            "remark" => draft.remark = value,
            "environment" => draft.environment = value,
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
                    private_key_source: draft.private_key_source,
                    private_key_path: draft.private_key_path,
                    environment: draft.environment,
                    proxy_method: draft.proxy_method,
                    remark: draft.remark,
                    credential_ref: None,
                });
                (parent_id, ConsoleAssetKind::SshConnection, label, payload)
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
        };

        let label = if draft_label.trim().is_empty() {
            self.console_asset_tree
                .next_default_name_for_parent(parent_id.as_deref(), kind)
        } else {
            if self.create_asset_modal_validation(parent_id.as_deref(), &draft_label)
                != AssetNameValidation::Valid
            {
                return false;
            }
            draft_label.trim().to_string()
        };
        let asset_id = if let Some(parent_id) = parent_id.as_deref() {
            let asset_id = self
                .console_asset_tree
                .insert_child_with_payload(parent_id, kind, label, payload);
            self.console_asset_tree.set_expanded(parent_id, true);
            asset_id
        } else {
            self.console_asset_tree
                .insert_root_with_payload(kind, label, payload)
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
        let Some(AssetModalState::DeleteAssetConfirm { asset_id, .. }) =
            self.asset_modal_state.clone()
        else {
            return false;
        };

        if self.remove_asset_subtree(&asset_id) {
            self.asset_modal_state = None;
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

    pub fn handle_assets_create_action(&mut self, action_id: &str) {
        match action_id {
            "new-folder" => self.open_new_folder_modal(None),
            "new-ssh-connection" => self.open_new_ssh_modal(None),
            _ => {}
        }
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
        if !self.console_asset_tree.contains(asset_id) {
            return;
        }

        self.selected_asset_ids = vec![asset_id.to_string()];
        self.focused_asset_id = Some(asset_id.to_string());
        self.context_target_asset_id = Some(asset_id.to_string());
        self.asset_create_menu_open = false;
    }

    pub fn toggle_folder_expanded(&mut self, asset_id: &str) {
        if self.console_asset_tree.kind(asset_id) != Some(ConsoleAssetKind::Folder) {
            return;
        }

        let next = !self
            .console_asset_tree
            .is_expanded(asset_id)
            .unwrap_or(false);
        self.console_asset_tree.set_expanded(asset_id, next);
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
                    if let Some(asset_id) = self
                        .context_target_asset_id
                        .clone()
                        .filter(|asset_id| self.console_asset_tree.contains(asset_id))
                    {
                        self.open_delete_asset_confirm(asset_id);
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
        self.clear_active_asset_rename_session();
        self.context_target_asset_id = None;
        self.close_context_menu();
    }

    pub fn console_asset_tree(&self) -> &AssetTree {
        &self.console_asset_tree
    }

    fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {
        let Some(target_kind) = self.context_menu_target_kind else {
            return Vec::new();
        };

        resolve_action_tree(target_kind, &self.context_menu_selection())
    }

    pub fn context_menu_selection(&self) -> SelectionContext {
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
}

fn asset_name_validation_message(validation: AssetNameValidation) -> String {
    match validation {
        AssetNameValidation::Valid => String::new(),
        AssetNameValidation::Empty => "Name is required.".into(),
        AssetNameValidation::Duplicate => "Name already exists in this folder.".into(),
    }
}

impl ShellViewModel {
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

        if draft.user.trim().is_empty() {
            return Some("User is required.".into());
        }

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
    let uses_saved_secret = match draft.auth_method.as_str() {
        "password" => !draft.password.trim().is_empty(),
        "private-key" if draft.private_key_source == "content" => {
            !draft.private_key_content.trim().is_empty()
        }
        "private-key" if draft.private_key_source == "path" => !draft.passphrase.trim().is_empty(),
        _ => false,
    };
    let credential_ref = match draft.auth_method.as_str() {
        "password" | "private-key" if uses_saved_secret => {
            Some(saved_ssh_credential_ref(asset_id, existing_spec))
        }
        _ => None,
    };

    AssetSshConnectionSpec {
        host: draft.host.clone(),
        user: draft.user.clone(),
        port: draft.port.clone(),
        auth_method: draft.auth_method.clone(),
        private_key_source: draft.private_key_source.clone(),
        private_key_path: draft.private_key_path.clone(),
        environment: draft.environment.clone(),
        proxy_method: draft.proxy_method.clone(),
        remark: draft.remark.clone(),
        credential_ref,
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
