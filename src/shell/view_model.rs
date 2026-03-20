//! Central shell state mirrored into Slint properties and mutated by UI callbacks.

use crate::app::window_state::WindowPlacementKind;
use crate::shell::assets::{
    AssetTree, AssetViewMode, ConsoleAssetKind, VisibleAssetRow, resolve_committed_name,
};
use crate::shell::context_menu::{
    ContextMenuActionNode, ContextMenuActionState, ContextTargetKind, SelectionContext,
    resolve_action_tree,
};
use crate::shell::sidebar::SidebarDestination;
use crate::theme::ThemeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    NewConnection,
    OpenRecent,
    Snippets,
    Sftp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssetModalState {
    NewFolder {
        parent_id: Option<String>,
        draft_name: String,
    },
    NewSshConnection {
        parent_id: Option<String>,
        active_tab: AssetSshModalTab,
        draft: AssetSshConnectionDraft,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetSshModalTab {
    Standard,
    Tunnel,
    Proxy,
    Environment,
    Advanced,
}

impl AssetSshModalTab {
    pub fn id(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Tunnel => "tunnel",
            Self::Proxy => "proxy",
            Self::Environment => "environment",
            Self::Advanced => "advanced",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "tunnel" => Some(Self::Tunnel),
            "proxy" => Some(Self::Proxy),
            "environment" => Some(Self::Environment),
            "advanced" => Some(Self::Advanced),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssetSshConnectionDraft {
    pub name: String,
    pub host: String,
    pub user: String,
    pub port: String,
    pub environment: String,
    pub proxy_method: String,
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
    pub asset_tree_fully_expanded: bool,
    pub selected_asset_ids: Vec<String>,
    pub focused_asset_id: Option<String>,
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
            asset_tree_fully_expanded: false,
            selected_asset_ids: Vec::new(),
            focused_asset_id: None,
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
    pub fn requested_assets_sidebar(&self) -> bool {
        self.show_assets_sidebar
    }

    pub fn requested_right_panel(&self) -> bool {
        self.show_right_panel
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
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewFolder {
            parent_id,
            draft_name: String::new(),
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
        self.close_context_menu();
        self.close_asset_create_menu();
        self.context_target_asset_id = parent_id.clone();
        self.asset_modal_state = Some(AssetModalState::NewSshConnection {
            parent_id,
            active_tab: AssetSshModalTab::Standard,
            draft: AssetSshConnectionDraft {
                port: "22".into(),
                ..AssetSshConnectionDraft::default()
            },
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
            "environment" => draft.environment = value,
            "proxy_method" => draft.proxy_method = value,
            _ => {}
        }
    }

    pub fn update_ssh_modal_name(&mut self, value: String) {
        self.update_ssh_modal_field("name", value);
    }

    pub fn update_ssh_modal_host(&mut self, value: String) {
        self.update_ssh_modal_field("host", value);
    }

    pub fn select_ssh_modal_tab(&mut self, tab: &str) {
        let Some(next_tab) = AssetSshModalTab::from_id(tab) else {
            return;
        };

        let Some(AssetModalState::NewSshConnection { active_tab, .. }) =
            self.asset_modal_state.as_mut()
        else {
            return;
        };

        *active_tab = next_tab;
    }

    pub fn can_confirm_asset_modal(&self) -> bool {
        match &self.asset_modal_state {
            Some(AssetModalState::NewFolder { draft_name, .. }) => !draft_name.trim().is_empty(),
            Some(AssetModalState::NewSshConnection { draft, .. }) => {
                !draft.name.trim().is_empty() && !draft.host.trim().is_empty()
            }
            None => false,
        }
    }

    pub fn confirm_asset_modal(&mut self) {
        let Some(modal_state) = self.asset_modal_state.clone() else {
            return;
        };

        let (parent_id, kind, draft_label) = match modal_state {
            AssetModalState::NewFolder {
                parent_id,
                draft_name,
            } => {
                if draft_name.trim().is_empty() {
                    return;
                }
                (parent_id, ConsoleAssetKind::Folder, draft_name)
            }
            AssetModalState::NewSshConnection { parent_id, draft, .. } => {
                if draft.name.trim().is_empty() || draft.host.trim().is_empty() {
                    return;
                }
                (parent_id, ConsoleAssetKind::SshConnection, draft.name)
            }
        };

        let sibling_items = self
            .console_asset_tree
            .sibling_items_for_parent(parent_id.as_deref(), None);
        let label = resolve_committed_name(kind, &draft_label, &sibling_items);
        let asset_id = if let Some(parent_id) = parent_id.as_deref() {
            let asset_id = self.console_asset_tree.insert_child(parent_id, kind, label);
            self.console_asset_tree.set_expanded(parent_id, true);
            asset_id
        } else {
            self.console_asset_tree.insert_root(kind, label)
        };

        self.selected_asset_ids = vec![asset_id.clone()];
        self.focused_asset_id = Some(asset_id.clone());
        self.context_target_asset_id = Some(asset_id);
        self.asset_modal_state = None;
    }

    pub fn cancel_asset_modal(&mut self) {
        self.asset_modal_state = None;
        self.context_target_asset_id = None;
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

        let next = !self.console_asset_tree.is_expanded(asset_id).unwrap_or(false);
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

        if current.children.iter().any(|node| !node.children.is_empty()) {
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
            ContextMenuActionState::Enabled => self.close_context_menu(),
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

    fn context_menu_roots(&self) -> Vec<ContextMenuActionNode> {
        let Some(target_kind) = self.context_menu_target_kind else {
            return Vec::new();
        };

        resolve_action_tree(target_kind, &self.context_menu_selection())
    }

    fn context_menu_selection(&self) -> SelectionContext {
        SelectionContext {
            selected_ids: self.selected_asset_ids.clone(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            target_has_active_connection: true,
        }
    }

    fn clear_active_asset_rename_session(&mut self) {
        self.editing_asset_id = None;
        self.editing_asset_text.clear();
    }

    fn normalize_folder_parent_id(&self, parent_id: Option<String>) -> Option<String> {
        parent_id.filter(|asset_id| {
            self.console_asset_tree.kind(asset_id.as_str()) == Some(ConsoleAssetKind::Folder)
        })
    }
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
