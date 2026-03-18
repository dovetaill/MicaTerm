use crate::app::window_state::WindowPlacementKind;
use crate::shell::assets::{AssetTree, AssetViewMode, ConsoleAssetKind, VisibleAssetRow};
use crate::shell::context_menu::ContextTargetKind;
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
    pub asset_tree_fully_expanded: bool,
    pub selected_asset_ids: Vec<String>,
    pub focused_asset_id: Option<String>,
    pub editing_asset_id: Option<String>,
    pub editing_asset_text: String,
    pub context_menu_open: bool,
    pub context_target_kind: Option<ContextTargetKind>,
    pub context_target_asset_id: Option<String>,
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
            asset_tree_fully_expanded: false,
            selected_asset_ids: Vec::new(),
            focused_asset_id: None,
            editing_asset_id: None,
            editing_asset_text: String::new(),
            context_menu_open: false,
            context_target_kind: None,
            context_target_asset_id: None,
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

    pub fn toggle_asset_tree_expansion(&mut self) {
        if self.asset_view_mode == AssetViewMode::Tree {
            self.asset_tree_fully_expanded = !self.asset_tree_fully_expanded;
        }
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

    pub fn visible_console_asset_rows(&self) -> Vec<VisibleAssetRow> {
        self.console_asset_tree
            .project_visible_rows(self.asset_view_mode, &self.asset_search_query)
    }

    pub fn handle_assets_create_action(&mut self, action_id: &str) {
        let Some(kind) = ConsoleAssetKind::from_create_action_id(action_id) else {
            return;
        };

        self.asset_create_menu_open = false;
        self.context_target_asset_id = None;

        let label = self.console_asset_tree.next_default_name_for_parent(None, kind);
        let asset_id = self.console_asset_tree.insert_root(kind, label.clone());
        self.selected_asset_ids = vec![asset_id.clone()];
        self.focused_asset_id = Some(asset_id.clone());
        self.editing_asset_id = Some(asset_id);
        self.editing_asset_text = label;
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

        let next_label = if self.editing_asset_text.trim().is_empty() {
            self.console_asset_tree
                .title(&asset_id)
                .unwrap_or_default()
                .to_string()
        } else {
            self.editing_asset_text.trim().to_string()
        };

        self.console_asset_tree.set_title(&asset_id, next_label);
        self.editing_asset_id = None;
        self.editing_asset_text.clear();
    }

    pub fn handle_blank_area_click(&mut self) {
        self.commit_active_asset_rename();
        self.selected_asset_ids.clear();
        self.focused_asset_id = None;
        self.context_menu_open = false;
        self.context_target_kind = None;
        self.context_target_asset_id = None;
    }

    pub fn select_asset(&mut self, asset_id: &str) {
        if !self.console_asset_tree.contains(asset_id) {
            return;
        }

        self.selected_asset_ids = vec![asset_id.to_string()];
        self.focused_asset_id = Some(asset_id.to_string());
        self.context_target_asset_id = Some(asset_id.to_string());
    }

    pub fn toggle_folder_expanded(&mut self, asset_id: &str) {
        if self.console_asset_tree.kind(asset_id) != Some(ConsoleAssetKind::Folder) {
            return;
        }

        let next = !self.console_asset_tree.is_expanded(asset_id).unwrap_or(false);
        self.console_asset_tree.set_expanded(asset_id, next);
    }

    pub fn open_context_menu_for_target(
        &mut self,
        target_kind: ContextTargetKind,
        target_id: Option<String>,
        _anchor_x: f32,
        _anchor_y: f32,
    ) {
        self.context_menu_open = true;
        self.context_target_kind = Some(target_kind);
        self.context_target_asset_id = target_id.clone();

        if let Some(target_id) = target_id {
            self.selected_asset_ids = vec![target_id.clone()];
            self.focused_asset_id = Some(target_id);
        } else {
            self.selected_asset_ids.clear();
            self.focused_asset_id = None;
        }
    }

    pub fn handle_context_menu_leaf_action(&mut self, action_id: &str) {
        let Some(kind) = ConsoleAssetKind::from_create_action_id(action_id) else {
            self.context_menu_open = false;
            self.context_target_kind = None;
            self.context_target_asset_id = None;
            return;
        };

        let parent_id = match (
            self.context_target_kind,
            self.context_target_asset_id.as_deref(),
        ) {
            (Some(ContextTargetKind::Folder), Some(asset_id))
                if self.console_asset_tree.contains(asset_id) =>
            {
                Some(asset_id.to_string())
            }
            _ => None,
        };

        if let Some(parent_id) = parent_id.as_deref() {
            self.console_asset_tree.set_expanded(parent_id, true);
        }

        let label = self
            .console_asset_tree
            .next_default_name_for_parent(parent_id.as_deref(), kind);
        let asset_id = if let Some(parent_id) = parent_id.as_deref() {
            self.console_asset_tree
                .insert_child(parent_id, kind, label.clone())
        } else {
            self.console_asset_tree.insert_root(kind, label.clone())
        };

        self.selected_asset_ids = vec![asset_id.clone()];
        self.focused_asset_id = Some(asset_id.clone());
        self.editing_asset_id = Some(asset_id);
        self.editing_asset_text = label;
        self.context_menu_open = false;
        self.context_target_kind = None;
        self.context_target_asset_id = None;
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
