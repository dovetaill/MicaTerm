//! ShellViewModel workspace domain impls.

use super::*;
use crate::app::ssh::profile::ConnectionProfile;
use crate::app::terminal_semantic::count_search_query_matches_in_lines;
use crate::shell::tabs::WorkspaceTabKind;

impl ShellViewModel {
    pub fn active_workspace_tab_id(&self) -> Option<&str> {
        self.active_workspace_tab_id.as_deref()
    }

    pub fn workspace_tabs(&self) -> &[WorkspaceTab] {
        &self.workspace_tabs
    }

    pub fn workspace_terminal_session_hidden(&self, session_id: &str) -> bool {
        self.hidden_workspace_terminal_session_ids
            .contains(session_id)
    }

    pub fn hide_workspace_terminal_session(&mut self, session_id: &str) {
        self.hidden_workspace_terminal_session_ids
            .insert(session_id.to_string());
    }

    pub fn unhide_workspace_terminal_session(&mut self, session_id: &str) {
        self.hidden_workspace_terminal_session_ids
            .remove(session_id);
    }

    pub fn set_workspace_tabs(&mut self, tabs: Vec<WorkspaceTab>) {
        self.workspace_tabs = tabs;
        self.normalize_workspace_tabs();
        self.sync_workspace_tab_context_menu_after_tab_change();
        let _ = self.recompute_sftp_queue_summary();
    }

    pub fn workspace_tab_by_id(&self, tab_id: &str) -> Option<&WorkspaceTab> {
        self.workspace_tabs.iter().find(|tab| tab.tab_id == tab_id)
    }

    pub fn tab_index_by_id(&self, tab_id: &str) -> Option<usize> {
        self.workspace_tabs
            .iter()
            .position(|tab| tab.tab_id == tab_id)
    }

    pub fn active_workspace_session_id(&self) -> Option<&str> {
        self.active_workspace_session_id.as_deref()
    }

    pub fn active_workspace_terminal_session_id(&self) -> Option<&str> {
        let tab = self.active_workspace_tab()?;
        if !tab.uses_terminal_surface()
            && !tab.uses_connection_progress_surface()
            && !tab.can_reconnect()
        {
            return None;
        }

        (!tab.session_id.is_empty()).then_some(tab.session_id.as_str())
    }

    pub fn active_workspace_terminal_surface(&self) -> Option<&TerminalSurfaceState> {
        let active_id = self.active_workspace_terminal_session_id()?;
        self.active_workspace_terminal_surface
            .as_ref()
            .filter(|surface| surface.session_id.to_string() == active_id)
    }

    pub fn set_active_workspace_terminal_surface(&mut self, surface: Option<TerminalSurfaceState>) {
        self.active_workspace_terminal_surface = surface;
    }

    pub fn active_workspace_tab(&self) -> Option<&WorkspaceTab> {
        let active_id = self.active_workspace_tab_id.as_deref()?;
        self.workspace_tab_by_id(active_id)
    }

    pub fn active_workspace_tab_summary(&self) -> Option<ActiveWorkspaceTabSummary> {
        let tab = self.active_workspace_tab()?;
        Some(ActiveWorkspaceTabSummary {
            tab_id: tab.tab_id.clone(),
            primary_summary_text: tab.primary_summary_text(),
            display_name: tab.display_name.clone(),
            host: tab.host.clone(),
            username: tab.username.clone(),
            port: tab.port,
            connection_status: tab.connection_status.clone(),
            connection_status_label: tab.connection_status_label().to_string(),
            tooltip_text: tab.summary_tooltip_text(),
        })
    }

    pub fn workspace_tab_context_menu_state(&self) -> &WorkspaceTabContextMenuState {
        &self.workspace_tab_context_menu_state
    }

    pub fn workspace_tab_copy_name_text(&self, tab_id: &str) -> Option<String> {
        let tab = self.workspace_tab_by_id(tab_id)?;
        (!tab.display_name.is_empty()).then(|| tab.display_name.clone())
    }

    pub fn workspace_tab_copy_host_text(&self, tab_id: &str) -> Option<String> {
        let tab = self.workspace_tab_by_id(tab_id)?;
        (!tab.host.is_empty()).then(|| tab.host.clone())
    }

    pub fn workspace_tab_connection_profile(&self, tab_id: &str) -> Option<ConnectionProfile> {
        self.workspace_tab_by_id(tab_id)
            .and_then(|tab| tab.connection_profile.clone())
    }

    pub fn workspace_tab_saved_ssh_asset_id(&self, tab_id: &str) -> Option<String> {
        let tab = self.workspace_tab_by_id(tab_id)?;
        if tab.kind != WorkspaceTabKind::Terminal {
            return None;
        }

        let asset_id = tab.asset_id.trim();
        if asset_id.is_empty() {
            return None;
        }

        (self.console_asset_tree.kind(asset_id) == Some(ConsoleAssetKind::SshConnection)
            && self
                .console_asset_tree
                .ssh_connection_spec(asset_id)
                .is_some())
        .then(|| asset_id.to_string())
    }

    pub fn workspace_tab_close_plan(
        &self,
        anchor_tab_id: Option<&str>,
        scope: WorkspaceTabCloseScope,
    ) -> Option<WorkspaceTabClosePlan> {
        let order = self.normalized_workspace_tab_order_for_tabs(&self.workspace_tabs);
        if order.is_empty() {
            return None;
        }

        let victim_tab_ids = match scope {
            WorkspaceTabCloseScope::All => order.clone(),
            WorkspaceTabCloseScope::One
            | WorkspaceTabCloseScope::Others
            | WorkspaceTabCloseScope::Left
            | WorkspaceTabCloseScope::Right => {
                let anchor_tab_id = anchor_tab_id?;
                let anchor_index = order.iter().position(|tab_id| tab_id == anchor_tab_id)?;
                match scope {
                    WorkspaceTabCloseScope::One => vec![anchor_tab_id.to_string()],
                    WorkspaceTabCloseScope::Others => order
                        .iter()
                        .filter(|tab_id| tab_id.as_str() != anchor_tab_id)
                        .cloned()
                        .collect(),
                    WorkspaceTabCloseScope::Left => order[..anchor_index].to_vec(),
                    WorkspaceTabCloseScope::Right => order[(anchor_index + 1)..].to_vec(),
                    WorkspaceTabCloseScope::All => unreachable!(),
                }
            }
        };

        if victim_tab_ids.is_empty() {
            return None;
        }

        let next_active_tab_id =
            self.next_active_workspace_tab_id_after_batch_close(&order, &victim_tab_ids);
        Some(WorkspaceTabClosePlan {
            victim_tab_ids,
            next_active_tab_id,
        })
    }

    pub fn open_workspace_tab_context_menu(
        &mut self,
        tab_id: &str,
        anchor_x: f32,
        anchor_y: f32,
    ) -> bool {
        let Some(menu_state) =
            self.resolve_workspace_tab_context_menu_state(tab_id, anchor_x, anchor_y)
        else {
            return false;
        };

        self.workspace_tab_context_menu_state = menu_state;
        true
    }

    pub fn close_workspace_tab_context_menu(&mut self) {
        self.workspace_tab_context_menu_state = WorkspaceTabContextMenuState::default();
    }

    pub fn activate_workspace_tab(&mut self, tab_id: &str) -> bool {
        if self.workspace_tab_by_id(tab_id).is_none() {
            return false;
        }

        self.active_workspace_tab_id = Some(tab_id.to_string());
        self.normalize_workspace_tabs();
        self.close_workspace_tab_context_menu();
        let _ = self.recompute_sftp_queue_summary();
        true
    }

    pub fn activate_workspace_session(&mut self, session_id: &str) -> bool {
        let Some(tab_id) = self
            .workspace_tabs
            .iter()
            .find(|tab| tab.session_id == session_id)
            .map(|tab| tab.tab_id.clone())
        else {
            return false;
        };

        self.activate_workspace_tab(tab_id.as_str())
    }

    pub fn close_workspace_session(&mut self, session_id: &str) -> bool {
        let Some(tab_id) = self
            .workspace_tabs
            .iter()
            .find(|tab| tab.session_id == session_id)
            .map(|tab| tab.tab_id.clone())
        else {
            return false;
        };

        self.close_workspace_tab(tab_id.as_str())
    }

    pub fn close_workspace_tab(&mut self, tab_id: &str) -> bool {
        self.close_workspace_tab_with_fallback(tab_id)
    }

    pub fn reorder_workspace_tab(&mut self, tab_id: &str, target_index: usize) -> bool {
        if self.workspace_tab_by_id(tab_id).is_none() {
            return false;
        }

        self.normalize_workspace_tabs();
        let Some(current_index) = self.workspace_tab_order.iter().position(|id| id == tab_id)
        else {
            return false;
        };

        let mut next_order = self.workspace_tab_order.clone();
        let moved_tab_id = next_order.remove(current_index);
        let clamped_index = target_index.min(next_order.len());
        next_order.insert(clamped_index, moved_tab_id);
        if next_order == self.workspace_tab_order {
            return false;
        }

        self.workspace_tab_order = next_order;
        self.normalize_workspace_tabs();
        self.sync_workspace_tab_context_menu_after_tab_change();
        true
    }

    pub fn close_workspace_session_with_fallback(&mut self, session_id: &str) -> bool {
        self.close_workspace_session(session_id)
    }

    pub fn open_workspace_launcher_tab(&mut self) {
        if self.workspace_tabs.iter().any(WorkspaceTab::is_launcher) {
            let _ = self.activate_workspace_tab("workspace-launcher");
            return;
        }

        let mut launcher = WorkspaceTab::launcher();
        launcher.active = true;
        self.workspace_tabs.push(launcher);
        self.active_workspace_tab_id = Some("workspace-launcher".into());
        self.normalize_workspace_tabs();
    }

    pub fn close_workspace_launcher_tab(&mut self) -> bool {
        self.close_workspace_tab_with_fallback("workspace-launcher")
    }

    pub fn close_workspace_tab_with_fallback(&mut self, tab_id: &str) -> bool {
        let Some(removed_index) = self
            .workspace_tabs
            .iter()
            .position(|tab| tab.tab_id == tab_id)
        else {
            return false;
        };

        let removed_was_active = self.active_workspace_tab_id.as_deref() == Some(tab_id);
        self.workspace_tabs.remove(removed_index);

        if removed_was_active {
            self.active_workspace_tab_id = self
                .workspace_tabs
                .get(removed_index)
                .or_else(|| {
                    removed_index
                        .checked_sub(1)
                        .and_then(|index| self.workspace_tabs.get(index))
                })
                .map(|tab| tab.tab_id.clone());
        }

        self.normalize_workspace_tabs();
        self.sync_workspace_tab_context_menu_after_tab_change();
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

    pub fn workspace_tab_can_reconnect(&self, tab_id: &str) -> bool {
        self.workspace_tab_by_id(tab_id)
            .map(WorkspaceTab::can_reconnect)
            .unwrap_or(false)
    }

    pub fn workspace_tab_can_clone_connection(&self, tab_id: &str) -> bool {
        self.workspace_tab_by_id(tab_id).is_some_and(|tab| {
            tab.kind == WorkspaceTabKind::Terminal
                && (tab.can_clone_connection()
                    || self.workspace_tab_saved_ssh_asset_id(tab_id).is_some())
        })
    }

    pub fn active_workspace_session_enhanced_state(&self) -> &str {
        self.active_workspace_tab()
            .map(|tab| tab.enhanced_session_state.as_str())
            .unwrap_or("")
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

    pub fn workspace_terminal_search_match_count(&self) -> usize {
        if !self.workspace_terminal_search_open || self.workspace_terminal_search_query.is_empty() {
            return 0;
        }

        count_search_query_matches_in_lines(
            &self.workspace_terminal_visible_lines(),
            &self.workspace_terminal_search_query,
        )
    }

    pub fn workspace_session_host_mode(&self) -> &'static str {
        match self.active_workspace_tab() {
            None => "welcome",
            Some(tab) if tab.is_launcher() => "welcome",
            Some(tab) if tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp => "sftp",
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

    pub fn saved_ssh_picker_selected_asset_id(&self) -> Option<&str> {
        self.saved_ssh_picker_selected_asset_id.as_deref()
    }

    pub fn saved_ssh_picker_can_open_selection(&self) -> bool {
        self.saved_ssh_picker_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| {
                self.console_asset_tree.kind(asset_id) == Some(ConsoleAssetKind::SshConnection)
            })
    }

    pub fn open_saved_ssh_picker(&mut self) {
        self.saved_ssh_picker_open = true;
        self.saved_ssh_picker_query.clear();
        self.saved_ssh_picker_selected_asset_id = self.first_saved_ssh_picker_asset_id();
        if let Some(asset_id) = self.saved_ssh_picker_selected_asset_id.clone() {
            self.expand_saved_ssh_picker_path(asset_id.as_str());
        }
    }

    pub fn close_saved_ssh_picker(&mut self) {
        self.saved_ssh_picker_open = false;
        self.saved_ssh_picker_query.clear();
        self.saved_ssh_picker_selected_asset_id = None;
    }

    pub fn set_saved_ssh_picker_query(&mut self, query: String) {
        self.saved_ssh_picker_query = query;
        self.saved_ssh_picker_selected_asset_id = self.first_saved_ssh_picker_asset_id();
        if self.saved_ssh_picker_query.trim().is_empty() {
            if let Some(asset_id) = self.saved_ssh_picker_selected_asset_id.clone() {
                self.expand_saved_ssh_picker_path(asset_id.as_str());
            }
        }
    }

    pub fn select_saved_ssh_picker_asset(&mut self, asset_id: String) {
        if self
            .saved_ssh_picker_items()
            .iter()
            .any(|item| item.id == asset_id)
        {
            self.expand_saved_ssh_picker_path(asset_id.as_str());
            self.saved_ssh_picker_selected_asset_id = Some(asset_id);
        }
    }

    pub fn move_saved_ssh_picker_selection(&mut self, delta: i32) {
        if delta == 0 {
            return;
        }

        let rows = self.saved_ssh_picker_items();
        if rows.is_empty() {
            return;
        }

        let selected_row_index = self
            .saved_ssh_picker_selected_asset_id
            .as_deref()
            .and_then(|selected_id| rows.iter().position(|item| item.id == selected_id));
        let next_row_index = selected_row_index
            .unwrap_or(0)
            .saturating_add_signed(delta as isize)
            .clamp(0, rows.len() - 1);

        let next_id = rows[next_row_index].id.clone();
        self.saved_ssh_picker_selected_asset_id = Some(next_id.clone());
        self.expand_saved_ssh_picker_path(next_id.as_str());
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
            if query_active {
                self.collect_saved_ssh_picker_search_rows(root_id, &mut rows);
            } else {
                self.collect_saved_ssh_picker_tree_rows(root_id, 0, &mut rows);
            }
        }
        rows
    }

    pub(crate) fn first_saved_ssh_picker_asset_id(&self) -> Option<String> {
        let query_active = !self.saved_ssh_picker_query.trim().is_empty();
        for root_id in self.console_asset_tree.root_ids() {
            let next = if query_active {
                self.first_matching_saved_ssh_picker_asset(root_id)
            } else {
                self.first_saved_ssh_picker_asset_in_tree(root_id)
            };
            if next.is_some() {
                return next;
            }
        }
        None
    }

    fn collect_saved_ssh_picker_tree_rows(
        &self,
        node_id: &str,
        depth: usize,
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
                    path_hint: self.saved_ssh_picker_secondary_text(node.id.as_str()),
                    compact_flat_mode: false,
                });
                true
            }
            ConsoleAssetKind::Folder => {
                let mut child_rows = Vec::new();
                let mut has_matching_descendants = false;
                for child_id in &node.children {
                    has_matching_descendants |= self.collect_saved_ssh_picker_tree_rows(
                        child_id,
                        depth + 1,
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
                    expanded: node.expanded,
                    selected: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    focused: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    disclosure_state: if node.expanded {
                        "expanded".into()
                    } else {
                        "collapsed".into()
                    },
                    path_hint: String::new(),
                    compact_flat_mode: false,
                });

                if node.expanded {
                    rows.extend(child_rows);
                }
                true
            }
            ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => false,
        }
    }

    fn collect_saved_ssh_picker_search_rows(
        &self,
        node_id: &str,
        rows: &mut Vec<SavedSshPickerItem>,
    ) {
        let Some(node) = self.console_asset_tree.node(node_id) else {
            return;
        };

        match node.kind {
            ConsoleAssetKind::SshConnection => {
                if !self.saved_ssh_picker_matches(node_id) {
                    return;
                }

                rows.push(SavedSshPickerItem {
                    id: node.id.clone(),
                    kind: node.kind.id().into(),
                    label: node.title.clone(),
                    depth: 0,
                    has_children: false,
                    expanded: false,
                    selected: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    focused: self.saved_ssh_picker_selected_asset_id.as_deref()
                        == Some(node.id.as_str()),
                    disclosure_state: "none".into(),
                    path_hint: self.saved_ssh_picker_search_secondary_text(node.id.as_str()),
                    compact_flat_mode: true,
                });
            }
            ConsoleAssetKind::Folder => {
                for child_id in &node.children {
                    self.collect_saved_ssh_picker_search_rows(child_id, rows);
                }
            }
            ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => {}
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

    fn saved_ssh_picker_secondary_text(&self, node_id: &str) -> String {
        let Some(spec) = self.console_asset_tree.ssh_connection_spec(node_id) else {
            return String::new();
        };

        let mut secondary = format!("{}@{}", spec.user.trim(), spec.host.trim());
        let port = spec.port.trim();
        if !port.is_empty() && port != "22" {
            secondary.push(':');
            secondary.push_str(port);
        }
        secondary
    }

    fn saved_ssh_picker_search_secondary_text(&self, node_id: &str) -> String {
        let secondary = self.saved_ssh_picker_secondary_text(node_id);
        let group_path = self.saved_ssh_picker_group_path(node_id);
        if group_path.is_empty() {
            secondary
        } else if secondary.is_empty() {
            group_path
        } else {
            format!("{secondary} - {group_path}")
        }
    }

    fn saved_ssh_picker_group_path(&self, node_id: &str) -> String {
        let mut labels = Vec::new();
        let mut cursor = self.console_asset_tree.parent_id(node_id).flatten();

        while let Some(parent_id) = cursor {
            let Some(node) = self.console_asset_tree.node(parent_id) else {
                break;
            };
            if node.kind == ConsoleAssetKind::Folder {
                labels.push(node.title.clone());
            }
            cursor = self.console_asset_tree.parent_id(parent_id).flatten();
        }

        labels.reverse();
        labels.join(" / ")
    }

    fn first_saved_ssh_picker_asset_in_tree(&self, node_id: &str) -> Option<String> {
        let node = self.console_asset_tree.node(node_id)?;
        match node.kind {
            ConsoleAssetKind::SshConnection => Some(node.id.clone()),
            ConsoleAssetKind::Folder => node
                .children
                .iter()
                .find_map(|child_id| self.first_saved_ssh_picker_asset_in_tree(child_id)),
            ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => None,
        }
    }

    fn first_matching_saved_ssh_picker_asset(&self, node_id: &str) -> Option<String> {
        let node = self.console_asset_tree.node(node_id)?;
        match node.kind {
            ConsoleAssetKind::SshConnection => self
                .saved_ssh_picker_matches(node_id)
                .then(|| node.id.clone()),
            ConsoleAssetKind::Folder => node
                .children
                .iter()
                .find_map(|child_id| self.first_matching_saved_ssh_picker_asset(child_id)),
            ConsoleAssetKind::SnippetPackage | ConsoleAssetKind::Snippet => None,
        }
    }

    fn expand_saved_ssh_picker_path(&mut self, asset_id: &str) {
        let mut parent_chain = Vec::new();
        let mut cursor = self.console_asset_tree.parent_id(asset_id).flatten();
        while let Some(parent_id) = cursor {
            parent_chain.push(parent_id.to_string());
            cursor = self.console_asset_tree.parent_id(parent_id).flatten();
        }

        for parent_id in parent_chain {
            self.console_asset_tree
                .set_expanded(parent_id.as_str(), true);
        }
    }

    fn resolve_workspace_tab_context_menu_state(
        &self,
        tab_id: &str,
        anchor_x: f32,
        anchor_y: f32,
    ) -> Option<WorkspaceTabContextMenuState> {
        let tab = self.workspace_tab_by_id(tab_id)?;
        let close_left_enabled = self
            .workspace_tab_close_plan(Some(tab_id), WorkspaceTabCloseScope::Left)
            .is_some();
        let close_right_enabled = self
            .workspace_tab_close_plan(Some(tab_id), WorkspaceTabCloseScope::Right)
            .is_some();

        Some(WorkspaceTabContextMenuState {
            open: true,
            anchor_tab_id: Some(tab.tab_id.clone()),
            anchor_x,
            anchor_y,
            reconnect_enabled: tab.can_reconnect(),
            clone_connection_enabled: self.workspace_tab_can_clone_connection(tab_id),
            close_enabled: true,
            copy_name_enabled: !tab.display_name.is_empty(),
            copy_host_enabled: !tab.host.is_empty(),
            close_others_enabled: self.workspace_tabs.len() > 1,
            close_all_enabled: !self.workspace_tabs.is_empty(),
            close_left_enabled,
            close_right_enabled,
        })
    }

    fn sync_workspace_tab_context_menu_after_tab_change(&mut self) {
        if !self.workspace_tab_context_menu_state.open {
            return;
        }

        let Some(anchor_tab_id) = self.workspace_tab_context_menu_state.anchor_tab_id.clone()
        else {
            self.close_workspace_tab_context_menu();
            return;
        };
        let anchor_x = self.workspace_tab_context_menu_state.anchor_x;
        let anchor_y = self.workspace_tab_context_menu_state.anchor_y;
        let Some(menu_state) = self.resolve_workspace_tab_context_menu_state(
            anchor_tab_id.as_str(),
            anchor_x,
            anchor_y,
        ) else {
            self.close_workspace_tab_context_menu();
            return;
        };

        self.workspace_tab_context_menu_state = menu_state;
    }

    fn next_active_workspace_tab_id_after_batch_close(
        &self,
        order: &[String],
        victim_tab_ids: &[String],
    ) -> Option<String> {
        if order.is_empty() {
            return None;
        }

        let victim_ids = victim_tab_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        let survivors = order
            .iter()
            .filter(|tab_id| !victim_ids.contains(tab_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        if survivors.is_empty() {
            return None;
        }

        if let Some(active_tab_id) = self
            .active_workspace_tab_id()
            .filter(|tab_id| !victim_ids.contains(*tab_id))
        {
            return Some(active_tab_id.to_string());
        }

        let Some(active_index) = self
            .active_workspace_tab_id()
            .and_then(|active_tab_id| order.iter().position(|tab_id| tab_id == active_tab_id))
        else {
            return survivors.first().cloned();
        };

        for tab_id in order.iter().skip(active_index + 1) {
            if !victim_ids.contains(tab_id.as_str()) {
                return Some(tab_id.clone());
            }
        }

        for tab_id in order[..active_index].iter().rev() {
            if !victim_ids.contains(tab_id.as_str()) {
                return Some(tab_id.clone());
            }
        }

        survivors.first().cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ssh::profile::{ConnectionProfile, ConnectionProxyProfile, SshAuthMethod};
    use crate::shell::assets::{
        AssetNodePayload, AssetSshConnectionSpec, AssetTree, ConsoleAssetKind,
    };
    use crate::shell::tabs::WorkspaceTabKind;

    fn terminal_tab(tab_id: &str, label: &str, state: &str) -> WorkspaceTab {
        terminal_tab_with_metadata(tab_id, label, state, format!("asset-{tab_id}"), None)
    }

    fn terminal_tab_with_metadata(
        tab_id: &str,
        label: &str,
        state: &str,
        asset_id: String,
        connection_profile: Option<ConnectionProfile>,
    ) -> WorkspaceTab {
        WorkspaceTab {
            tab_id: tab_id.into(),
            session_id: tab_id.into(),
            file_browser_session_id: String::new(),
            asset_id,
            display_name: label.into(),
            host: format!("{tab_id}.example.com"),
            username: "ops".into(),
            port: 22,
            connection_status: state.into(),
            title: label.into(),
            subtitle: String::new(),
            state: state.into(),
            enhanced_session_state: String::new(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Terminal,
            reconnectable: matches!(state, "disconnected" | "error")
                || connection_profile.is_some(),
            connection_profile,
        }
    }

    fn safe_connection_profile(asset_id: &str) -> ConnectionProfile {
        ConnectionProfile {
            asset_id: Some(asset_id.into()),
            name: "Prod Bastion".into(),
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: 22,
            auth_method: SshAuthMethod::Password,
            credential_ref: Some(format!("ssh/saved-secrets/{asset_id}")),
            private_key_path: None,
            password: None,
            private_key_content: None,
            passphrase: None,
            proxy: ConnectionProxyProfile::None,
            resolved_proxy_hops: Vec::new(),
            remark: String::new(),
        }
    }

    fn saved_ssh_tree_with_asset(title: &str) -> (AssetTree, String) {
        let mut tree = AssetTree::new();
        let asset_id = tree.insert_root_with_payload(
            ConsoleAssetKind::SshConnection,
            title,
            AssetNodePayload::SshConnection(AssetSshConnectionSpec {
                host: "10.0.0.12".into(),
                user: "ops".into(),
                port: "22".into(),
                auth_method: "password".into(),
                credential_ref: Some("ssh/saved-secrets/prod".into()),
                ..AssetSshConnectionSpec::default()
            }),
        );
        (tree, asset_id)
    }

    #[test]
    fn workspace_tab_context_menu_disables_single_tab_range_actions() {
        let mut state = ShellViewModel::default();
        state.set_workspace_tabs(vec![terminal_tab("tab-a", "Prod", "connected")]);

        assert!(state.open_workspace_tab_context_menu("tab-a", 128.0, 48.0));

        let menu = state.workspace_tab_context_menu_state();
        assert!(menu.open);
        assert_eq!(menu.anchor_tab_id.as_deref(), Some("tab-a"));
        assert!(menu.close_enabled);
        assert!(menu.copy_name_enabled);
        assert!(menu.copy_host_enabled);
        assert!(menu.close_all_enabled);
        assert!(!menu.reconnect_enabled);
        assert!(!menu.clone_connection_enabled);
        assert!(!menu.close_others_enabled);
        assert!(!menu.close_left_enabled);
        assert!(!menu.close_right_enabled);
    }

    #[test]
    fn workspace_tab_context_menu_uses_reordered_ui_edges_for_enablement() {
        let mut state = ShellViewModel::default();
        state.set_workspace_tabs(vec![
            terminal_tab("tab-a", "A", "connected"),
            terminal_tab("tab-b", "B", "connected"),
            terminal_tab("tab-c", "C", "connected"),
        ]);
        assert!(state.reorder_workspace_tab("tab-c", 0));

        assert!(state.open_workspace_tab_context_menu("tab-c", 40.0, 16.0));
        let first_menu = state.workspace_tab_context_menu_state();
        assert!(!first_menu.close_left_enabled);
        assert!(first_menu.close_right_enabled);

        assert!(state.open_workspace_tab_context_menu("tab-b", 220.0, 16.0));
        let last_menu = state.workspace_tab_context_menu_state();
        assert!(last_menu.close_left_enabled);
        assert!(!last_menu.close_right_enabled);
    }

    #[test]
    fn workspace_tab_context_menu_enables_clone_for_connected_terminal_with_profile() {
        let mut state = ShellViewModel::default();
        state.set_workspace_tabs(vec![terminal_tab_with_metadata(
            "tab-a",
            "Prod",
            "connected",
            "asset-prod".into(),
            Some(safe_connection_profile("asset-prod")),
        )]);

        assert!(state.open_workspace_tab_context_menu("tab-a", 128.0, 48.0));
        assert!(
            state
                .workspace_tab_context_menu_state()
                .clone_connection_enabled
        );
    }

    #[test]
    fn workspace_tab_context_menu_enables_clone_for_saved_asset_without_cached_profile() {
        let mut state = ShellViewModel::default();
        let (tree, asset_id) = saved_ssh_tree_with_asset("Prod Bastion");
        state.replace_console_asset_tree(tree);
        state.set_workspace_tabs(vec![terminal_tab_with_metadata(
            "tab-a",
            "Prod",
            "connected",
            asset_id,
            None,
        )]);

        assert!(state.open_workspace_tab_context_menu("tab-a", 128.0, 48.0));
        assert!(
            state
                .workspace_tab_context_menu_state()
                .clone_connection_enabled
        );
    }

    #[test]
    fn workspace_tab_context_menu_disables_clone_without_saved_or_runtime_metadata() {
        let mut state = ShellViewModel::default();
        let mut synthetic = terminal_tab_with_metadata(
            "tab-terminal",
            "Scratch",
            "connected",
            "session:temporary".into(),
            None,
        );
        synthetic.reconnectable = false;
        state.set_workspace_tabs(vec![
            synthetic,
            WorkspaceTab::sftp("tab-sftp", "browser-1", "Browser"),
            WorkspaceTab::launcher(),
        ]);

        assert!(state.open_workspace_tab_context_menu("tab-terminal", 20.0, 12.0));
        assert!(
            !state
                .workspace_tab_context_menu_state()
                .clone_connection_enabled
        );
        assert!(state.open_workspace_tab_context_menu("tab-sftp", 20.0, 12.0));
        assert!(
            !state
                .workspace_tab_context_menu_state()
                .clone_connection_enabled
        );
        assert!(state.open_workspace_tab_context_menu("workspace-launcher", 20.0, 12.0));
        assert!(
            !state
                .workspace_tab_context_menu_state()
                .clone_connection_enabled
        );
    }

    #[test]
    fn workspace_close_scope_plans_freeze_ui_order_and_final_active() {
        let mut state = ShellViewModel::default();
        state.set_workspace_tabs(vec![
            terminal_tab("tab-a", "A", "connected"),
            terminal_tab("tab-b", "B", "connected"),
            terminal_tab("tab-c", "C", "connected"),
            terminal_tab("tab-d", "D", "connected"),
        ]);
        assert!(state.reorder_workspace_tab("tab-c", 0));
        assert!(state.reorder_workspace_tab("tab-d", 2));
        assert!(state.activate_workspace_tab("tab-b"));

        let close_others = state
            .workspace_tab_close_plan(Some("tab-a"), WorkspaceTabCloseScope::Others)
            .expect("close others plan");
        assert_eq!(
            close_others.victim_tab_ids,
            vec![
                "tab-c".to_string(),
                "tab-d".to_string(),
                "tab-b".to_string()
            ]
        );
        assert_eq!(close_others.next_active_tab_id.as_deref(), Some("tab-a"));

        let close_left = state
            .workspace_tab_close_plan(Some("tab-d"), WorkspaceTabCloseScope::Left)
            .expect("close left plan");
        assert_eq!(
            close_left.victim_tab_ids,
            vec!["tab-c".to_string(), "tab-a".to_string()]
        );
        assert_eq!(close_left.next_active_tab_id.as_deref(), Some("tab-b"));

        let close_right = state
            .workspace_tab_close_plan(Some("tab-a"), WorkspaceTabCloseScope::Right)
            .expect("close right plan");
        assert_eq!(
            close_right.victim_tab_ids,
            vec!["tab-d".to_string(), "tab-b".to_string()]
        );
        assert_eq!(close_right.next_active_tab_id.as_deref(), Some("tab-a"));

        let close_all = state
            .workspace_tab_close_plan(None, WorkspaceTabCloseScope::All)
            .expect("close all plan");
        assert_eq!(
            close_all.victim_tab_ids,
            vec![
                "tab-c".to_string(),
                "tab-a".to_string(),
                "tab-d".to_string(),
                "tab-b".to_string()
            ]
        );
        assert_eq!(close_all.next_active_tab_id, None);
    }
}
