//! ShellViewModel projection domain impls.

use super::*;

fn visible(value: &str) -> bool {
    matches!(value, "visible" | "show" | "true")
}

fn reset_sync_modal_secret_visibility(modal: &mut SyncModalViewState) {
    modal.master_password_visible = false;
    modal.git_https_secret_visible = false;
    modal.git_ssh_passphrase_visible = false;
}

impl ShellViewModel {
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

    pub fn seed_test_asset(&mut self, kind: ConsoleAssetKind, label: impl Into<String>) {
        self.console_asset_tree.insert_root(kind, label);
    }

    pub fn replace_console_asset_tree(&mut self, tree: AssetTree) {
        self.console_asset_tree = tree;
        self.asset_tree_fully_expanded = false;
        self.selected_asset_ids.clear();
        self.focused_asset_id = None;
        if self
            .saved_ssh_picker_selected_asset_id
            .as_deref()
            .is_some_and(|asset_id| !self.console_asset_tree.contains(asset_id))
        {
            self.saved_ssh_picker_selected_asset_id = None;
        }
        if self.saved_ssh_picker_open && self.saved_ssh_picker_selected_asset_id.is_none() {
            self.saved_ssh_picker_selected_asset_id = self.first_saved_ssh_picker_asset_id();
        }
        self.clear_active_asset_rename_session();
        self.context_target_asset_id = None;
        self.close_context_menu();
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

    pub fn requested_assets_sidebar(&self) -> bool {
        self.show_assets_sidebar
    }

    pub fn requested_assets_sidebar_width(&self) -> u32 {
        self.assets_sidebar_expanded_width.round().max(0.0) as u32
    }

    pub fn requested_right_panel(&self) -> bool {
        self.show_right_panel
    }

    pub fn workspace_focus_mode(&self) -> bool {
        self.workspace_focus_mode
    }

    pub fn requested_right_panel_width(&self) -> u32 {
        self.right_panel_expanded_width.round().max(0.0) as u32
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

    pub fn sync_feedback_state(&self) -> &SyncFeedbackViewState {
        &self.sync_feedback_state
    }

    pub fn transfer_center_feedback_state(&self) -> &TransferCenterFeedbackViewState {
        &self.transfer_center_feedback_state
    }

    pub fn settings_modal_open(&self) -> bool {
        self.settings_modal_state.open
    }

    pub fn settings_modal_theme_variant(&self) -> crate::theme::ThemeVariant {
        self.theme_variant
    }

    pub fn settings_modal_theme_variant_id(&self) -> &'static str {
        self.theme_variant.id()
    }

    pub fn settings_modal_terminal_scrollback_limit(&self) -> usize {
        self.settings_modal_state.terminal_scrollback_limit
    }

    pub fn settings_modal_terminal_active_idle_shrink_enabled(&self) -> bool {
        self.settings_modal_state
            .terminal_active_idle_shrink_enabled
    }

    pub fn settings_modal_terminal_input_highlighting_enabled(&self) -> bool {
        self.settings_modal_state
            .terminal_input_highlighting_enabled
    }

    pub fn settings_modal_terminal_output_rule_highlighting_enabled(&self) -> bool {
        self.settings_modal_state
            .terminal_output_rule_highlighting_enabled
    }

    pub fn settings_modal_terminal_command_decorations_enabled(&self) -> bool {
        self.settings_modal_state
            .terminal_command_decorations_enabled
    }

    pub fn settings_modal_terminal_overview_markers_enabled(&self) -> bool {
        self.settings_modal_state.terminal_overview_markers_enabled
    }

    pub fn settings_modal_terminal_output_rule_profile(
        &self,
    ) -> crate::app::terminal_semantic::OutputRuleProfile {
        self.settings_modal_state.terminal_output_rule_profile
    }

    pub fn settings_modal_terminal_output_rule_profile_id(&self) -> &'static str {
        self.settings_modal_state.terminal_output_rule_profile.id()
    }

    pub fn settings_modal_terminal_search_match_highlight(
        &self,
    ) -> crate::theme::SearchMatchHighlightStrength {
        self.settings_modal_state.terminal_search_match_highlight
    }

    pub fn settings_modal_terminal_search_match_highlight_id(&self) -> &'static str {
        self.settings_modal_state
            .terminal_search_match_highlight
            .id()
    }

    pub fn workspace_terminal_search_open(&self) -> bool {
        self.workspace_terminal_search_open
    }

    pub fn workspace_terminal_search_query(&self) -> &str {
        &self.workspace_terminal_search_query
    }

    pub fn workspace_terminal_search_focus_sequence(&self) -> i32 {
        self.workspace_terminal_search_focus_sequence
    }

    pub fn settings_modal_download_conflict_default(
        &self,
    ) -> crate::app::ui_preferences::DownloadConflictDefault {
        self.settings_modal_state.download_conflict_default
    }

    pub fn settings_modal_download_conflict_default_id(&self) -> &'static str {
        self.settings_modal_state.download_conflict_default.as_str()
    }

    pub fn start_sync_feedback(&mut self, text: impl Into<String>) {
        self.sync_feedback_state.text = text.into();
        self.sync_feedback_state.running = true;
        self.sync_feedback_state.sequence = self.sync_feedback_state.sequence.saturating_add(1);
    }

    pub fn show_sync_feedback(&mut self, text: impl Into<String>) {
        self.sync_feedback_state.text = text.into();
        self.sync_feedback_state.running = false;
        self.sync_feedback_state.sequence = self.sync_feedback_state.sequence.saturating_add(1);
    }

    pub fn clear_sync_feedback(&mut self) {
        self.sync_feedback_state.running = false;
    }

    pub fn show_transfer_center_feedback(
        &mut self,
        tone: impl Into<String>,
        text: impl Into<String>,
    ) {
        self.transfer_center_feedback_state.tone = tone.into();
        self.transfer_center_feedback_state.text = text.into();
        self.transfer_center_feedback_state.sequence = self
            .transfer_center_feedback_state
            .sequence
            .saturating_add(1);
    }

    pub fn right_panel_view_id(&self) -> &'static str {
        self.right_panel_view.id()
    }

    pub fn set_right_panel_view(&mut self, value: RightPanelView) {
        self.right_panel_view = value;
    }

    pub fn open_settings_panel(&mut self) {
        self.settings_modal_state.open = true;
        self.show_global_menu = false;
    }

    pub fn close_settings_modal(&mut self) {
        self.settings_modal_state.open = false;
        self.show_global_menu = false;
    }

    pub fn set_settings_modal_theme_variant(&mut self, value: &str) {
        self.theme_variant = crate::theme::ThemeVariant::from_id(value);
    }

    pub fn set_settings_modal_terminal_scrollback_limit(&mut self, value: i32) {
        let value = value.max(1) as usize;
        self.settings_modal_state.terminal_scrollback_limit = value;
    }

    pub fn set_settings_modal_terminal_active_idle_shrink_enabled(&mut self, value: bool) {
        self.settings_modal_state
            .terminal_active_idle_shrink_enabled = value;
    }

    pub fn set_settings_modal_terminal_input_highlighting_enabled(&mut self, value: bool) {
        self.settings_modal_state
            .terminal_input_highlighting_enabled = value;
    }

    pub fn set_settings_modal_terminal_output_rule_highlighting_enabled(&mut self, value: bool) {
        self.settings_modal_state
            .terminal_output_rule_highlighting_enabled = value;
    }

    pub fn set_settings_modal_terminal_command_decorations_enabled(&mut self, value: bool) {
        self.settings_modal_state
            .terminal_command_decorations_enabled = value;
    }

    pub fn set_settings_modal_terminal_overview_markers_enabled(&mut self, value: bool) {
        self.settings_modal_state.terminal_overview_markers_enabled = value;
    }

    pub fn set_settings_modal_terminal_output_rule_profile(&mut self, value: &str) {
        self.settings_modal_state.terminal_output_rule_profile =
            crate::app::terminal_semantic::OutputRuleProfile::from_id(value);
    }

    pub fn set_settings_modal_terminal_search_match_highlight(&mut self, value: &str) {
        self.settings_modal_state.terminal_search_match_highlight =
            crate::theme::SearchMatchHighlightStrength::from_id(value);
    }

    pub fn open_workspace_terminal_search(&mut self) {
        self.workspace_terminal_search_open = true;
        self.workspace_terminal_search_focus_sequence = self
            .workspace_terminal_search_focus_sequence
            .saturating_add(1);
    }

    pub fn close_workspace_terminal_search(&mut self) {
        self.workspace_terminal_search_open = false;
    }

    pub fn set_workspace_terminal_search_query(&mut self, value: String) {
        self.workspace_terminal_search_query = value;
        if !self.workspace_terminal_search_query.is_empty() {
            self.workspace_terminal_search_open = true;
        }
    }

    pub fn set_settings_modal_download_conflict_default(&mut self, value: &str) {
        self.settings_modal_state.download_conflict_default =
            crate::app::ui_preferences::DownloadConflictDefault::from_str(value);
    }

    pub fn open_sync_modal(&mut self) {
        self.reset_sync_modal_secret_visibility();
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
        self.reset_sync_modal_secret_visibility();
        self.sync_modal_state.open = false;
        self.show_global_menu = false;
    }

    pub fn reset_sync_modal_secret_visibility(&mut self) {
        reset_sync_modal_secret_visibility(&mut self.sync_modal_state);
    }

    pub fn update_sync_modal_field(&mut self, field: &str, value: String) {
        let modal = self.sync_modal_state_mut();
        let clears_error = !matches!(
            field,
            "git-https-secret-visibility"
                | "git-ssh-passphrase-visibility"
                | "master-password-visibility"
        );
        match field {
            "git-remote-url" => modal.git_remote_url = value,
            "git-branch" => modal.git_branch = value,
            "git-auth-mode" => {
                if modal.git_auth_mode != value {
                    modal.git_auth_mode = value;
                    reset_sync_modal_secret_visibility(modal);
                }
            }
            "git-https-username" => modal.git_https_username = value,
            "git-https-secret" => modal.git_https_secret = value,
            "git-https-secret-visibility" => modal.git_https_secret_visible = visible(&value),
            "git-ssh-private-key" => modal.git_ssh_private_key = value,
            "git-ssh-passphrase" => modal.git_ssh_passphrase = value,
            "git-ssh-passphrase-visibility" => {
                modal.git_ssh_passphrase_visible = visible(&value);
            }
            "master-password" => modal.master_password = value,
            "master-password-visibility" => modal.master_password_visible = visible(&value),
            _ => return,
        }
        if clears_error {
            modal.error_text.clear();
        }
    }

    pub fn update_sync_modal_toggle(&mut self, _field: &str, _value: bool) {
        self.sync_modal_state_mut().error_text.clear();
    }

    pub fn toggle_right_panel(&mut self) {
        if self.workspace_focus_mode && !self.show_right_panel {
            self.exit_workspace_focus_mode();
            if self.show_right_panel {
                return;
            }

            self.show_right_panel = true;
            self.right_panel_view = RightPanelView::Sftp;
            return;
        }

        self.show_right_panel = !self.show_right_panel;
        self.right_panel_view = RightPanelView::Sftp;
    }

    pub fn right_panel_expanded_width_px(&self) -> f32 {
        self.right_panel_expanded_width
    }

    pub fn set_right_panel_expanded_width(&mut self, width_px: f32) -> bool {
        let width_px = width_px.clamp(
            ShellMetrics::RIGHT_PANEL_MIN_WIDTH as f32,
            ShellMetrics::RIGHT_PANEL_MAX_WIDTH as f32,
        );
        if (self.right_panel_expanded_width - width_px).abs() <= f32::EPSILON {
            return false;
        }

        self.right_panel_expanded_width = width_px;
        true
    }

    pub fn apply_right_panel_resize(&mut self, width_px: f32) -> bool {
        if self.workspace_focus_mode
            && !self.show_right_panel
            && width_px >= ShellMetrics::RIGHT_PANEL_COLLAPSE_THRESHOLD as f32
        {
            self.exit_workspace_focus_mode();
            let _ = self.set_right_panel_expanded_width(width_px);
            self.show_right_panel = true;
            self.right_panel_view = RightPanelView::Sftp;
            return true;
        }

        if width_px < ShellMetrics::RIGHT_PANEL_COLLAPSE_THRESHOLD as f32 {
            if !self.show_right_panel {
                return false;
            }

            self.show_right_panel = false;
            self.right_panel_view = RightPanelView::Sftp;
            return true;
        }

        let width_changed = self.set_right_panel_expanded_width(width_px);
        let reopened = !self.show_right_panel;
        self.show_right_panel = true;
        self.right_panel_view = RightPanelView::Sftp;
        width_changed || reopened
    }

    pub fn transfer_center_open(&self) -> bool {
        self.transfer_center_open
    }

    pub fn transfer_center_pinned(&self) -> bool {
        self.transfer_center_pinned
    }

    pub fn transfer_center_collapsed(&self) -> bool {
        self.transfer_center_collapsed
    }

    pub fn toggle_transfer_center(&mut self) {
        self.transfer_center_open = !self.transfer_center_open;
    }

    pub fn close_transfer_center(&mut self) {
        self.transfer_center_open = false;
    }

    pub fn toggle_transfer_center_pin(&mut self) -> bool {
        self.transfer_center_pinned = !self.transfer_center_pinned;
        true
    }

    pub fn toggle_transfer_center_collapse(&mut self) -> bool {
        self.transfer_center_collapsed = !self.transfer_center_collapsed;
        true
    }

    pub fn transfer_center_filter_id(&self) -> &'static str {
        self.transfer_center_filter.id()
    }

    pub fn toggle_transfer_center_filter(&mut self, filter_id: &str) -> bool {
        let Some(filter) = TransferCenterFilter::from_id(filter_id) else {
            return false;
        };
        let next = if self.transfer_center_filter == filter {
            TransferCenterFilter::All
        } else {
            filter
        };
        if self.transfer_center_filter == next {
            return false;
        }
        self.transfer_center_filter = next;
        true
    }

    pub fn transfer_center_includes_task(&self, task: &crate::app::sftp::TransferTask) -> bool {
        self.transfer_center_filter.matches(task)
    }

    pub fn sftp_transfer_tasks(&self) -> &[crate::app::sftp::TransferTask] {
        &self.sftp_transfer_tasks
    }

    pub fn transfer_task_local_open_file_path(&self, task_id: &str) -> Option<std::path::PathBuf> {
        let task = self.transfer_task_by_id(task_id)?;
        if task.state != crate::app::sftp::TransferTaskState::Completed {
            return None;
        }
        match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path }
                if crate::app::sftp::can_open_file_path_locally(local_path.as_path()) =>
            {
                Some(local_path.clone())
            }
            _ => None,
        }
    }

    pub fn transfer_task_local_open_folder_path(
        &self,
        task_id: &str,
    ) -> Option<std::path::PathBuf> {
        let task = self.transfer_task_by_id(task_id)?;
        if task.state != crate::app::sftp::TransferTaskState::Completed {
            return None;
        }
        match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path }
                if crate::app::sftp::can_open_folder_path_locally(local_path.as_path()) =>
            {
                Some(local_path.clone())
            }
            crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path }
                if crate::app::sftp::can_open_folder_path_locally(local_path.as_path()) =>
            {
                Some(local_path.clone())
            }
            _ => None,
        }
    }

    pub fn transfer_task_local_remove_path(&self, task_id: &str) -> Option<std::path::PathBuf> {
        let task = self.transfer_task_by_id(task_id)?;
        if task.state != crate::app::sftp::TransferTaskState::Completed {
            return None;
        }

        match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path }
            | crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path }
                if local_path.exists() =>
            {
                Some(local_path.clone())
            }
            _ => None,
        }
    }

    pub fn transfer_task_remove_tooltip(&self, task_id: &str) -> String {
        let Some(task) = self.transfer_task_by_id(task_id) else {
            return "Remove this record from the transfer center".into();
        };

        if task.state == crate::app::sftp::TransferTaskState::Completed {
            match &task.action {
                crate::app::sftp::TransferTaskAction::Download { local_path }
                | crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path } => {
                    if local_path.exists() {
                        return "Trash the downloaded artifact and remove this record".into();
                    }
                    return "The local file is already missing; remove this record only".into();
                }
                _ => {}
            }
        }

        "Remove this record from the transfer center".into()
    }

    pub fn transfer_task_remove_missing_download(&self, task_id: &str) -> bool {
        let Some(task) = self.transfer_task_by_id(task_id) else {
            return false;
        };
        if task.state != crate::app::sftp::TransferTaskState::Completed {
            return false;
        }

        match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path }
            | crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path } => {
                !local_path.exists()
            }
            _ => false,
        }
    }

    pub fn remove_transfer_task(&mut self, task_id: &str) -> bool {
        let before_len = self.sftp_transfer_tasks.len();
        self.sftp_transfer_tasks.retain(|task| task.id != task_id);
        if self.sftp_transfer_tasks.len() == before_len {
            return false;
        }
        if self.sftp_conflict_modal_state.task_id.as_deref() == Some(task_id)
            || self
                .sftp_conflict_modal_state
                .batch_task_ids
                .iter()
                .any(|id| id == task_id)
        {
            self.sftp_conflict_modal_state = SftpConflictModalState::default();
        }
        let _ = self.recompute_sftp_queue_summary();
        true
    }

    pub fn clear_completed_transfer_tasks(&mut self) -> bool {
        let before_len = self.sftp_transfer_tasks.len();
        self.sftp_transfer_tasks
            .retain(|task| task.state != crate::app::sftp::TransferTaskState::Completed);
        if self.sftp_transfer_tasks.len() == before_len {
            return false;
        }
        let _ = self.recompute_sftp_queue_summary();
        true
    }

    pub fn merge_sftp_transfer_tasks(&mut self, tasks: &[crate::app::sftp::TransferTask]) -> bool {
        let mut changed = false;
        for next_task in tasks {
            if let Some(current_task) = self
                .sftp_transfer_tasks
                .iter_mut()
                .find(|task| task.id == next_task.id)
            {
                if current_task != next_task {
                    *current_task = next_task.clone();
                    changed = true;
                }
            } else {
                self.sftp_transfer_tasks.push(next_task.clone());
                changed = true;
            }
        }

        changed
    }

    pub fn recompute_sftp_queue_summary(&mut self) -> bool {
        let next = crate::app::sftp::TransferQueueSummary::from_tasks(
            &self.sftp_transfer_tasks,
            self.active_workspace_terminal_session_id(),
        );
        if self.sftp_queue_summary == next {
            return false;
        }
        self.sftp_queue_summary = next;
        true
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
}
