//! ShellViewModel workspace domain impls.

use super::*;
use crate::app::terminal_semantic::count_search_query_matches_in_lines;

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
        let _ = self.recompute_sftp_queue_summary();
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
        self.workspace_tabs
            .iter()
            .find(|tab| tab.tab_id == active_id)
    }

    pub fn activate_workspace_tab(&mut self, tab_id: &str) -> bool {
        if !self.workspace_tabs.iter().any(|tab| tab.tab_id == tab_id) {
            return false;
        }

        self.active_workspace_tab_id = Some(tab_id.to_string());
        self.normalize_workspace_tabs();
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
}
