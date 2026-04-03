//! ShellViewModel SFTP domain impls.

use super::*;

impl ShellViewModel {
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

    pub(super) fn active_sftp_session_state_mut(&mut self) -> Option<&mut SftpSessionBindingState> {
        let session_id = self.active_workspace_session_id.clone()?;
        Some(self.sftp_sessions.entry(session_id).or_default())
    }

    pub fn select_sftp_panel_entry(&mut self, entry_id: &str) -> bool {
        {
            let Some(state) = self.active_sftp_session_state_mut() else {
                return false;
            };
            if state.selected_entry_ids.len() == 1 && state.selected_entry_ids[0] == entry_id {
                return false;
            }
            state.selected_entry_ids = vec![entry_id.to_string()];
        }
        self.context_target_asset_id = Some(entry_id.to_string());
        true
    }

    pub fn active_sftp_entry(&self, entry_id: &str) -> Option<&SftpDirectoryEntry> {
        self.active_sftp_session_state()?
            .entries
            .iter()
            .find(|entry| entry.id == entry_id)
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

    pub fn sftp_panel_sort_column_id(&self) -> &'static str {
        self.sftp_panel_sort_state
            .column
            .map(SftpPanelSortColumn::id)
            .unwrap_or("default")
    }

    pub fn sftp_panel_sort_direction_id(&self) -> &'static str {
        self.sftp_panel_sort_state
            .direction
            .map(SftpPanelSortDirection::id)
            .unwrap_or("none")
    }

    pub fn cycle_sftp_panel_sort(&mut self, column_id: &str) -> bool {
        let Some(column) = SftpPanelSortColumn::from_id(column_id) else {
            return false;
        };

        self.sftp_panel_sort_state = match self.sftp_panel_sort_state {
            SftpPanelSortState {
                column: Some(active_column),
                direction: Some(SftpPanelSortDirection::Asc),
            } if active_column == column => SftpPanelSortState {
                column: Some(column),
                direction: Some(SftpPanelSortDirection::Desc),
            },
            SftpPanelSortState {
                column: Some(active_column),
                direction: Some(SftpPanelSortDirection::Desc),
            } if active_column == column => SftpPanelSortState::default(),
            _ => SftpPanelSortState {
                column: Some(column),
                direction: Some(SftpPanelSortDirection::Asc),
            },
        };
        true
    }

    pub fn sftp_panel_name_column_width_px(&self) -> f32 {
        self.sftp_panel_column_layout.name_px
    }

    pub fn sftp_panel_type_column_width_px(&self) -> f32 {
        self.sftp_panel_column_layout.type_px
    }

    pub fn sftp_panel_modified_column_width_px(&self) -> f32 {
        self.sftp_panel_column_layout.modified_px
    }

    pub fn sftp_panel_size_column_width_px(&self) -> f32 {
        self.sftp_panel_column_layout.size_px
    }

    pub fn set_sftp_panel_column_width(&mut self, column_id: &str, width_px: f32) -> bool {
        match column_id {
            "name" => self.sftp_panel_column_layout.name_px = width_px.max(0.0),
            "type" => self.sftp_panel_column_layout.type_px = width_px.max(SFTP_TYPE_COLUMN_MIN_PX),
            "modified" => {
                self.sftp_panel_column_layout.modified_px =
                    width_px.max(SFTP_MODIFIED_COLUMN_MIN_PX);
            }
            "size" => self.sftp_panel_column_layout.size_px = width_px.max(SFTP_SIZE_COLUMN_MIN_PX),
            _ => return false,
        }
        true
    }

    pub fn project_sftp_panel_entries<'a>(
        &self,
        entries: &'a [SftpDirectoryEntry],
    ) -> Vec<&'a SftpDirectoryEntry> {
        let mut projection = entries.iter().collect::<Vec<_>>();
        projection.sort_by(|left, right| {
            compare_sftp_panel_entries(left, right, self.sftp_panel_sort_state)
        });
        projection
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

    pub fn sftp_remote_file_editor_state(&self) -> &SftpRemoteFileEditorState {
        &self.sftp_remote_file_editor_state
    }

    pub fn open_sftp_remote_file_editor(
        &mut self,
        session_id: impl Into<String>,
        remote_path: impl Into<String>,
        title: impl Into<String>,
        content: impl Into<String>,
        status_text: impl Into<String>,
        error_text: impl Into<String>,
    ) {
        let content = content.into();
        self.sftp_remote_file_editor_state = SftpRemoteFileEditorState {
            open: true,
            session_id: Some(session_id.into()),
            remote_path: remote_path.into(),
            title: title.into(),
            saved_content: content.clone(),
            content,
            status_text: status_text.into(),
            error_text: error_text.into(),
        };
    }

    pub fn close_sftp_remote_file_editor(&mut self) {
        self.sftp_remote_file_editor_state = SftpRemoteFileEditorState::default();
    }

    pub fn update_sftp_remote_file_editor_content(&mut self, value: String) {
        if !self.sftp_remote_file_editor_state.open {
            return;
        }
        self.sftp_remote_file_editor_state.content = value;
        self.sftp_remote_file_editor_state.error_text.clear();
    }

    pub fn sftp_remote_file_editor_can_save(&self) -> bool {
        let editor = &self.sftp_remote_file_editor_state;
        editor.open
            && editor.error_text.is_empty()
            && editor.content != editor.saved_content
            && editor.session_id.is_some()
            && !editor.remote_path.is_empty()
    }

    pub fn sftp_remote_file_editor_save_payload(&self) -> Option<(String, String, String)> {
        let editor = &self.sftp_remote_file_editor_state;
        if !self.sftp_remote_file_editor_can_save() {
            return None;
        }

        Some((
            editor.session_id.clone()?,
            editor.remote_path.clone(),
            editor.content.clone(),
        ))
    }

    pub fn mark_sftp_remote_file_editor_saved(&mut self) {
        if !self.sftp_remote_file_editor_state.open {
            return;
        }
        self.sftp_remote_file_editor_state.saved_content =
            self.sftp_remote_file_editor_state.content.clone();
        self.sftp_remote_file_editor_state.status_text = "Changes saved to the remote file.".into();
        self.sftp_remote_file_editor_state.error_text.clear();
    }

    pub fn set_sftp_remote_file_editor_error(&mut self, message: impl Into<String>) {
        if !self.sftp_remote_file_editor_state.open {
            return;
        }
        self.sftp_remote_file_editor_state.error_text = message.into();
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
}
