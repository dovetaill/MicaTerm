//! ShellViewModel SFTP domain impls.

use super::*;
use chrono::{DateTime, Utc};

const SFTP_PANEL_ROW_HEIGHT_PX: u32 = 44;
const SFTP_PANEL_DEFAULT_VIEWPORT_HEIGHT_PX: u32 = SFTP_PANEL_ROW_HEIGHT_PX * 10;
const SFTP_PANEL_WINDOW_OVERSCAN_ROWS: usize = 6;

fn sftp_parent_render_row() -> SftpPanelRenderRow {
    SftpPanelRenderRow {
        id: "__sftp_parent__".into(),
        name: "..".into(),
        meta_label: "Go to parent directory".into(),
        type_label: "Up".into(),
        modified_label: String::new(),
        size_label: String::new(),
        kind: "parent-directory".into(),
        selected: false,
    }
}

fn normalized_sftp_panel_viewport_offset_px(viewport_y: f32) -> u32 {
    (-viewport_y).max(0.0).round() as u32
}

fn normalized_sftp_panel_viewport_height_px(visible_height: f32) -> u32 {
    visible_height.max(0.0).round() as u32
}

fn recompute_sftp_panel_virtual_window(render_cache: &mut SftpPanelRenderCache) -> bool {
    let total_row_count = render_cache.rows.len();
    let previous_window_start_row = render_cache.window_start_row;
    let previous_window_end_row = render_cache.window_end_row;
    let previous_total_content_height_px = render_cache.total_content_height_px;
    let previous_top_spacer_height_px = render_cache.top_spacer_height_px;
    let previous_bottom_spacer_height_px = render_cache.bottom_spacer_height_px;

    render_cache.total_content_height_px = u32::try_from(total_row_count)
        .unwrap_or(u32::MAX)
        .saturating_mul(SFTP_PANEL_ROW_HEIGHT_PX);

    if total_row_count == 0 {
        render_cache.window_start_row = 0;
        render_cache.window_end_row = 0;
        render_cache.top_spacer_height_px = 0;
        render_cache.bottom_spacer_height_px = 0;
    } else {
        let viewport_height_px = render_cache
            .viewport_height_px
            .max(SFTP_PANEL_DEFAULT_VIEWPORT_HEIGHT_PX);
        let first_visible_row =
            usize::try_from(render_cache.viewport_offset_px / SFTP_PANEL_ROW_HEIGHT_PX)
                .unwrap_or(usize::MAX)
                .min(total_row_count.saturating_sub(1));
        let visible_row_count =
            usize::try_from(viewport_height_px.div_ceil(SFTP_PANEL_ROW_HEIGHT_PX).max(1))
                .unwrap_or(usize::MAX)
                .max(1);
        let desired_window_len = visible_row_count + (SFTP_PANEL_WINDOW_OVERSCAN_ROWS * 2);

        let mut window_start_row =
            first_visible_row.saturating_sub(SFTP_PANEL_WINDOW_OVERSCAN_ROWS);
        let mut window_end_row = (window_start_row + desired_window_len).min(total_row_count);
        if window_end_row == total_row_count {
            window_start_row = window_end_row.saturating_sub(desired_window_len);
        }
        if window_end_row <= window_start_row {
            window_end_row = (window_start_row + 1).min(total_row_count);
        }

        render_cache.window_start_row = window_start_row;
        render_cache.window_end_row = window_end_row;
        render_cache.top_spacer_height_px = u32::try_from(window_start_row)
            .unwrap_or(u32::MAX)
            .saturating_mul(SFTP_PANEL_ROW_HEIGHT_PX);
        render_cache.bottom_spacer_height_px =
            u32::try_from(total_row_count.saturating_sub(window_end_row))
                .unwrap_or(u32::MAX)
                .saturating_mul(SFTP_PANEL_ROW_HEIGHT_PX);
    }

    if render_cache.window_start_row != previous_window_start_row
        || render_cache.window_end_row != previous_window_end_row
    {
        render_cache.dirty_row_indices.clear();
        return true;
    }

    render_cache.total_content_height_px != previous_total_content_height_px
        || render_cache.top_spacer_height_px != previous_top_spacer_height_px
        || render_cache.bottom_spacer_height_px != previous_bottom_spacer_height_px
}

impl ShellViewModel {
    pub fn open_sftp_panel(&mut self) {
        self.right_panel_view = RightPanelView::Sftp;
        self.show_right_panel = true;
        self.show_global_menu = false;
    }

    pub fn set_file_browser_session(&mut self, session: FileBrowserSession) {
        let projection = self.build_sftp_panel_projection(&session);
        let previous_render_cache = self
            .sftp_panel_render_cache
            .get(session.file_browser_session_id.as_str());
        let render_cache = self.build_sftp_panel_render_cache(
            &session,
            projection.as_slice(),
            previous_render_cache,
        );
        self.sftp_panel_projection_cache
            .insert(session.file_browser_session_id.clone(), projection);
        self.sftp_panel_render_cache
            .insert(session.file_browser_session_id.clone(), render_cache);
        self.file_browser_sessions
            .insert(session.file_browser_session_id.clone(), session);
    }

    fn build_sftp_panel_projection(&self, session: &FileBrowserSession) -> Vec<SftpDirectoryEntry> {
        let mut projection = session.entries.clone();
        projection
            .sort_by(|left, right| compare_sftp_panel_entries(left, right, session.sort_state));
        projection
    }

    pub fn refresh_sftp_panel_projection_cache(&mut self, file_browser_session_id: &str) -> bool {
        let Some(session) = self
            .file_browser_sessions
            .get(file_browser_session_id)
            .cloned()
        else {
            return false;
        };
        let projection = self.build_sftp_panel_projection(&session);
        self.sftp_panel_projection_cache
            .insert(file_browser_session_id.to_string(), projection);
        let _ = self.refresh_sftp_panel_render_cache(file_browser_session_id);
        true
    }

    fn build_sftp_panel_render_cache(
        &self,
        session: &FileBrowserSession,
        projection: &[SftpDirectoryEntry],
        previous_render_cache: Option<&SftpPanelRenderCache>,
    ) -> SftpPanelRenderCache {
        let selected_entry_ids = session
            .selected_entry_ids
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let mut rows = Vec::with_capacity(projection.len() + 1);
        let mut row_index_by_entry_id = HashMap::with_capacity(projection.len());

        if session.can_navigate_up() {
            rows.push(sftp_parent_render_row());
        }

        for entry in projection {
            row_index_by_entry_id.insert(entry.id.clone(), rows.len());
            rows.push(build_sftp_panel_render_row(
                entry,
                selected_entry_ids.contains(entry.id.as_str()),
            ));
        }

        let mut render_cache = SftpPanelRenderCache {
            rows,
            row_index_by_entry_id,
            viewport_offset_px: previous_render_cache
                .map(|cache| cache.viewport_offset_px)
                .unwrap_or(0),
            viewport_height_px: previous_render_cache
                .map(|cache| cache.viewport_height_px)
                .unwrap_or(SFTP_PANEL_DEFAULT_VIEWPORT_HEIGHT_PX),
            window_start_row: 0,
            window_end_row: 0,
            total_content_height_px: 0,
            top_spacer_height_px: 0,
            bottom_spacer_height_px: 0,
            dirty_row_indices: Vec::new(),
            full_resync_required: true,
        };
        let _ = recompute_sftp_panel_virtual_window(&mut render_cache);
        render_cache
    }

    pub fn refresh_sftp_panel_render_cache(&mut self, file_browser_session_id: &str) -> bool {
        let Some(session) = self
            .file_browser_sessions
            .get(file_browser_session_id)
            .cloned()
        else {
            return false;
        };
        let projection = self
            .sftp_panel_projection_cache
            .get(file_browser_session_id)
            .cloned()
            .unwrap_or_else(|| self.build_sftp_panel_projection(&session));
        self.sftp_panel_render_cache.insert(
            file_browser_session_id.to_string(),
            self.build_sftp_panel_render_cache(
                &session,
                projection.as_slice(),
                self.sftp_panel_render_cache.get(file_browser_session_id),
            ),
        );
        true
    }

    pub fn set_sftp_session_state(&mut self, session_id: String, state: SftpSessionBindingState) {
        let mut session = self
            .file_browser_sessions
            .get(session_id.as_str())
            .cloned()
            .unwrap_or_else(|| {
                let mut session = FileBrowserSession::quick_browser(
                    HostProfileRef::new("active-session"),
                    state.current_path.clone(),
                );
                session.file_browser_session_id = session_id.clone();
                session
            });
        session.attach_terminal_session_id(session_id.clone());
        session.mode = state.mode;
        session.follow_mode = state.follow_mode;
        session.current_path = state.current_path;
        session.history = state.history;
        session.entries = state.entries;
        session.selected_entry_ids = state.selected_entry_ids;
        session.last_error = state.last_error;
        session.active_request_id = None;
        self.set_file_browser_session(session);
    }

    pub fn quick_browser_session_id(&self) -> Option<&str> {
        self.quick_browser_session_id
            .as_deref()
            .or(self.active_workspace_terminal_session_id())
    }

    pub fn quick_browser_session(&self) -> Option<&FileBrowserSession> {
        let session_id = self.quick_browser_session_id()?;
        self.file_browser_sessions.get(session_id)
    }

    pub(super) fn quick_browser_session_mut(&mut self) -> Option<&mut FileBrowserSession> {
        let session_id = self.quick_browser_session_id()?.to_string();
        self.file_browser_sessions.get_mut(&session_id)
    }

    pub fn active_workspace_sftp_session(&self) -> Option<&FileBrowserSession> {
        let tab = self.active_workspace_tab()?;
        if tab.kind != crate::shell::tabs::WorkspaceTabKind::Sftp {
            return None;
        }

        self.file_browser_sessions
            .get(tab.file_browser_session_id.as_str())
    }

    pub(super) fn active_workspace_sftp_session_mut(&mut self) -> Option<&mut FileBrowserSession> {
        let session_id = self
            .active_workspace_tab()
            .filter(|tab| tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp)
            .map(|tab| tab.file_browser_session_id.clone())?;
        self.file_browser_sessions.get_mut(&session_id)
    }

    pub fn active_file_browser_session_id(&self) -> Option<&str> {
        self.active_workspace_sftp_session()
            .map(|session| session.file_browser_session_id.as_str())
            .or_else(|| {
                self.quick_browser_session()
                    .map(|session| session.file_browser_session_id.as_str())
            })
    }

    pub fn active_sftp_session_state(&self) -> Option<&FileBrowserSession> {
        let session_id = self.active_file_browser_session_id()?;
        self.file_browser_sessions.get(session_id)
    }

    pub(super) fn active_sftp_session_state_mut(&mut self) -> Option<&mut FileBrowserSession> {
        let session_id = self.active_file_browser_session_id()?.to_string();
        self.file_browser_sessions.get_mut(&session_id)
    }

    fn active_sftp_panel_render_cache(&self) -> Option<&SftpPanelRenderCache> {
        let session_id = self.active_file_browser_session_id()?;
        self.sftp_panel_render_cache.get(session_id)
    }

    fn update_sftp_panel_render_selection_cache(
        &mut self,
        file_browser_session_id: &str,
        previous_selection: &[String],
        next_selection: &[String],
    ) -> bool {
        let Some(render_cache) = self
            .sftp_panel_render_cache
            .get_mut(file_browser_session_id)
        else {
            return false;
        };

        let next_selection = next_selection
            .iter()
            .map(String::as_str)
            .collect::<HashSet<_>>();
        let mut dirty_row_indices = previous_selection
            .iter()
            .map(String::as_str)
            .chain(next_selection.iter().copied())
            .filter_map(|entry_id| render_cache.row_index_by_entry_id.get(entry_id).copied())
            .collect::<Vec<_>>();
        dirty_row_indices.sort_unstable();
        dirty_row_indices.dedup();
        if dirty_row_indices.is_empty() {
            return false;
        }

        for &index in &dirty_row_indices {
            if let Some(row) = render_cache.rows.get_mut(index) {
                row.selected = next_selection.contains(row.id.as_str());
            }
        }
        render_cache.dirty_row_indices = dirty_row_indices
            .into_iter()
            .filter(|index| {
                *index >= render_cache.window_start_row && *index < render_cache.window_end_row
            })
            .map(|index| index - render_cache.window_start_row)
            .collect();
        render_cache.full_resync_required = false;
        true
    }

    pub(super) fn replace_active_sftp_selection(&mut self, next_selection: Vec<String>) -> bool {
        let (file_browser_session_id, previous_selection) = {
            let Some(state) = self.active_sftp_session_state_mut() else {
                return false;
            };
            if state.selected_entry_ids == next_selection {
                return false;
            }
            let previous_selection =
                std::mem::replace(&mut state.selected_entry_ids, next_selection.clone());
            (state.file_browser_session_id.clone(), previous_selection)
        };
        self.update_sftp_panel_render_selection_cache(
            file_browser_session_id.as_str(),
            previous_selection.as_slice(),
            next_selection.as_slice(),
        );
        true
    }

    pub fn select_sftp_panel_entry(&mut self, entry_id: &str) -> bool {
        if !self.replace_active_sftp_selection(vec![entry_id.to_string()]) {
            return false;
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
        self.quick_browser_session()
            .map(|state| state.mode.id())
            .unwrap_or(SftpPanelMode::Empty.id())
    }

    pub fn sftp_panel_projected_entries(&self) -> &[SftpDirectoryEntry] {
        let Some(session_id) = self.quick_browser_session_id() else {
            return &[];
        };
        if let Some(entries) = self.sftp_panel_projection_cache.get(session_id) {
            entries.as_slice()
        } else {
            self.quick_browser_session()
                .map(|state| state.entries.as_slice())
                .unwrap_or(&[])
        }
    }

    pub fn active_sftp_panel_render_rows(&self) -> &[SftpPanelRenderRow] {
        self.active_sftp_panel_render_cache()
            .and_then(|cache| cache.rows.get(cache.window_start_row..cache.window_end_row))
            .unwrap_or(&[])
    }

    pub fn active_sftp_panel_total_row_count(&self) -> usize {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.rows.len())
            .unwrap_or(0)
    }

    pub fn active_sftp_panel_visible_row_range(&self) -> std::ops::Range<usize> {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.window_start_row..cache.window_end_row)
            .unwrap_or(0..0)
    }

    pub fn active_sftp_panel_render_dirty_indices(&self) -> &[usize] {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.dirty_row_indices.as_slice())
            .unwrap_or(&[])
    }

    pub fn active_sftp_panel_render_requires_full_resync(&self) -> bool {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.full_resync_required)
            .unwrap_or(self.sftp_panel_last_rendered_session_id.is_some())
    }

    pub fn update_active_sftp_panel_viewport(
        &mut self,
        viewport_y: f32,
        visible_height: f32,
    ) -> bool {
        let Some(active_session_id) = self.active_file_browser_session_id().map(str::to_owned)
        else {
            return false;
        };
        let Some(render_cache) = self
            .sftp_panel_render_cache
            .get_mut(active_session_id.as_str())
        else {
            return false;
        };

        let next_viewport_offset_px = normalized_sftp_panel_viewport_offset_px(viewport_y);
        let next_viewport_height_px = normalized_sftp_panel_viewport_height_px(visible_height);
        if render_cache.viewport_offset_px == next_viewport_offset_px
            && render_cache.viewport_height_px == next_viewport_height_px
        {
            return false;
        }

        render_cache.viewport_offset_px = next_viewport_offset_px;
        render_cache.viewport_height_px = next_viewport_height_px;
        let window_changed = recompute_sftp_panel_virtual_window(render_cache);
        if window_changed {
            render_cache.full_resync_required = true;
        }
        window_changed
    }

    pub fn sftp_panel_total_content_height_px(&self) -> f32 {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.total_content_height_px as f32)
            .unwrap_or(0.0)
    }

    pub fn sftp_panel_top_spacer_height_px(&self) -> f32 {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.top_spacer_height_px as f32)
            .unwrap_or(0.0)
    }

    pub fn sftp_panel_bottom_spacer_height_px(&self) -> f32 {
        self.active_sftp_panel_render_cache()
            .map(|cache| cache.bottom_spacer_height_px as f32)
            .unwrap_or(0.0)
    }

    pub fn mark_active_sftp_panel_render_clean(&mut self) -> bool {
        let active_session_id = self.active_file_browser_session_id().map(str::to_owned);
        if let Some(session_id) = active_session_id.as_deref()
            && let Some(render_cache) = self.sftp_panel_render_cache.get_mut(session_id)
        {
            render_cache.full_resync_required = false;
            render_cache.dirty_row_indices.clear();
        }
        let changed = self.sftp_panel_last_rendered_session_id != active_session_id;
        self.sftp_panel_last_rendered_session_id = active_session_id;
        changed || self.active_sftp_panel_render_cache().is_some()
    }

    pub fn sftp_panel_host_label(&self) -> String {
        self.quick_browser_session()
            .map(|state| state.host_profile_ref.label.clone())
            .unwrap_or_default()
    }

    pub fn sftp_panel_path(&self) -> String {
        self.quick_browser_session()
            .map(|state| state.current_path.clone())
            .unwrap_or_default()
    }

    pub fn sftp_panel_follow_mode_id(&self) -> &'static str {
        self.quick_browser_session()
            .map(|state| state.follow_mode.id())
            .unwrap_or(SftpFollowMode::FollowCwd.id())
    }

    pub fn sftp_panel_can_go_back(&self) -> bool {
        self.quick_browser_session()
            .map(|state| state.history.can_back())
            .unwrap_or(false)
    }

    pub fn sftp_panel_can_go_forward(&self) -> bool {
        self.quick_browser_session()
            .map(|state| state.history.can_forward())
            .unwrap_or(false)
    }

    pub fn sftp_panel_can_go_up(&self) -> bool {
        self.quick_browser_session()
            .map(FileBrowserSession::can_navigate_up)
            .unwrap_or(false)
    }

    pub fn sftp_panel_actions_enabled(&self) -> bool {
        self.quick_browser_session()
            .map(|state| {
                matches!(
                    state.mode,
                    SftpPanelMode::Ready | SftpPanelMode::Loading | SftpPanelMode::Connecting
                )
            })
            .unwrap_or(false)
    }

    pub fn quick_browser_accepts_external_drop(&self) -> bool {
        self.show_right_panel
            && self.right_panel_view == RightPanelView::Sftp
            && self.quick_browser_linked_terminal_session_id().is_some()
            && self.quick_browser_session().is_some_and(|state| {
                state.mode == SftpPanelMode::Ready && !state.current_path.trim().is_empty()
            })
    }

    pub fn quick_browser_drop_target_active(&self) -> bool {
        self.quick_browser_state.drop_target_active && self.quick_browser_accepts_external_drop()
    }

    pub fn set_quick_browser_drop_target_active(&mut self, active: bool) -> bool {
        let next_active = active && self.quick_browser_accepts_external_drop();
        if self.quick_browser_state.drop_target_active == next_active {
            return false;
        }
        self.quick_browser_state.drop_target_active = next_active;
        true
    }

    pub fn sftp_panel_sort_column_id(&self) -> &'static str {
        self.quick_browser_session()
            .map(|state| state.sort_state)
            .unwrap_or(self.quick_browser_state.sort_state)
            .column
            .map(FileBrowserSortColumn::id)
            .unwrap_or("default")
    }

    pub fn sftp_panel_sort_direction_id(&self) -> &'static str {
        self.quick_browser_session()
            .map(|state| state.sort_state)
            .unwrap_or(self.quick_browser_state.sort_state)
            .direction
            .map(FileBrowserSortDirection::id)
            .unwrap_or("none")
    }

    pub fn cycle_sftp_panel_sort(&mut self, column_id: &str) -> bool {
        let Some(column) = FileBrowserSortColumn::from_id(column_id) else {
            return false;
        };
        let next_sort_state = match self
            .quick_browser_session()
            .map(|state| state.sort_state)
            .unwrap_or(self.quick_browser_state.sort_state)
        {
            FileBrowserSortState {
                column: Some(active_column),
                direction: Some(FileBrowserSortDirection::Asc),
            } if active_column == column => FileBrowserSortState {
                column: Some(column),
                direction: Some(FileBrowserSortDirection::Desc),
            },
            FileBrowserSortState {
                column: Some(active_column),
                direction: Some(FileBrowserSortDirection::Desc),
            } if active_column == column => FileBrowserSortState::default(),
            _ => FileBrowserSortState {
                column: Some(column),
                direction: Some(FileBrowserSortDirection::Asc),
            },
        };

        if let Some(session_id) = {
            if let Some(state) = self.quick_browser_session_mut() {
                state.sort_state = next_sort_state;
                Some(state.file_browser_session_id.clone())
            } else {
                None
            }
        } {
            self.refresh_sftp_panel_projection_cache(session_id.as_str());
        } else {
            self.quick_browser_state.sort_state = next_sort_state;
        }
        true
    }

    pub fn set_sftp_panel_sort_column(&mut self, column_id: &str) -> bool {
        let Some(column) = FileBrowserSortColumn::from_id(column_id) else {
            return false;
        };
        let next_sort_state = FileBrowserSortState {
            column: Some(column),
            direction: Some(FileBrowserSortDirection::Asc),
        };

        if let Some(session_id) = {
            if let Some(state) = self.quick_browser_session_mut() {
                if state.sort_state == next_sort_state {
                    return false;
                }
                state.sort_state = next_sort_state;
                Some(state.file_browser_session_id.clone())
            } else {
                None
            }
        } {
            self.refresh_sftp_panel_projection_cache(session_id.as_str());
        } else {
            if self.quick_browser_state.sort_state == next_sort_state {
                return false;
            }
            self.quick_browser_state.sort_state = next_sort_state;
        }
        true
    }

    pub fn select_all_sftp_entries(&mut self) -> bool {
        let next_selection = self
            .active_sftp_session_state()
            .map(|state| {
                state
                    .entries
                    .iter()
                    .map(|entry| entry.id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if next_selection.is_empty() {
            return false;
        }

        let first_selected_id = next_selection.first().cloned();
        if !self.replace_active_sftp_selection(next_selection) {
            return false;
        }
        self.context_target_asset_id = first_selected_id;
        true
    }

    pub fn sftp_panel_name_column_width_px(&self) -> f32 {
        self.quick_browser_session()
            .map(|state| state.column_layout.name_px)
            .unwrap_or(self.quick_browser_state.column_layout.name_px)
    }

    pub fn sftp_panel_type_column_width_px(&self) -> f32 {
        self.quick_browser_session()
            .map(|state| state.column_layout.type_px)
            .unwrap_or(self.quick_browser_state.column_layout.type_px)
    }

    pub fn sftp_panel_modified_column_width_px(&self) -> f32 {
        self.quick_browser_session()
            .map(|state| state.column_layout.modified_px)
            .unwrap_or(self.quick_browser_state.column_layout.modified_px)
    }

    pub fn sftp_panel_size_column_width_px(&self) -> f32 {
        self.quick_browser_session()
            .map(|state| state.column_layout.size_px)
            .unwrap_or(self.quick_browser_state.column_layout.size_px)
    }

    pub fn set_sftp_panel_column_width(&mut self, column_id: &str, width_px: f32) -> bool {
        let layout = if let Some(state) = self.quick_browser_session_mut() {
            &mut state.column_layout
        } else {
            &mut self.quick_browser_state.column_layout
        };
        match column_id {
            "name" => layout.name_px = width_px.max(0.0),
            "type" => layout.type_px = width_px.max(FILE_BROWSER_TYPE_COLUMN_MIN_PX),
            "modified" => {
                layout.modified_px = width_px.max(FILE_BROWSER_MODIFIED_COLUMN_MIN_PX);
            }
            "size" => layout.size_px = width_px.max(FILE_BROWSER_SIZE_COLUMN_MIN_PX),
            _ => return false,
        }
        true
    }

    pub fn project_sftp_panel_entries<'a>(
        &self,
        entries: &'a [SftpDirectoryEntry],
    ) -> Vec<&'a SftpDirectoryEntry> {
        let sort_state = self
            .quick_browser_session()
            .map(|state| state.sort_state)
            .unwrap_or(self.quick_browser_state.sort_state);
        let mut projection = entries.iter().collect::<Vec<_>>();
        projection.sort_by(|left, right| compare_sftp_panel_entries(left, right, sort_state));
        projection
    }

    pub fn sftp_panel_entries(&self) -> &[SftpDirectoryEntry] {
        self.quick_browser_session()
            .map(|state| state.entries.as_slice())
            .unwrap_or(&[])
    }

    pub fn sftp_panel_selected_entry_ids(&self) -> &[String] {
        self.quick_browser_session()
            .map(|state| state.selected_entry_ids.as_slice())
            .unwrap_or(&[])
    }

    pub fn submit_sftp_panel_path(&mut self, path: impl Into<String>) -> bool {
        let path = path.into();
        let trimmed = path.trim();
        if trimmed.is_empty() {
            return false;
        }

        let Some(session_id) = ({
            let Some(state) = self.quick_browser_session_mut() else {
                return false;
            };
            state.navigate_manual(trimmed.to_string());
            Some(state.file_browser_session_id.clone())
        }) else {
            return false;
        };
        self.quick_browser_state.path_editing = false;
        let _ = self.refresh_sftp_panel_render_cache(session_id.as_str());
        true
    }

    pub fn navigate_sftp_panel_back(&mut self) -> bool {
        let Some(session_id) = ({
            let Some(state) = self.quick_browser_session_mut() else {
                return false;
            };
            if !state.navigate_back() {
                return false;
            }
            Some(state.file_browser_session_id.clone())
        }) else {
            return false;
        };
        let _ = self.refresh_sftp_panel_render_cache(session_id.as_str());
        true
    }

    pub fn navigate_sftp_panel_forward(&mut self) -> bool {
        let Some(session_id) = ({
            let Some(state) = self.quick_browser_session_mut() else {
                return false;
            };
            if !state.navigate_forward() {
                return false;
            }
            Some(state.file_browser_session_id.clone())
        }) else {
            return false;
        };
        let _ = self.refresh_sftp_panel_render_cache(session_id.as_str());
        true
    }

    pub fn navigate_sftp_panel_up(&mut self) -> bool {
        let Some(session_id) = ({
            let Some(state) = self.quick_browser_session_mut() else {
                return false;
            };
            if !state.navigate_up() {
                return false;
            }
            Some(state.file_browser_session_id.clone())
        }) else {
            return false;
        };
        let _ = self.refresh_sftp_panel_render_cache(session_id.as_str());
        true
    }

    pub fn retry_sftp_panel(&mut self) -> bool {
        let Some(state) = self.quick_browser_session_mut() else {
            return false;
        };
        state.mark_connecting();
        true
    }

    pub fn refresh_sftp_panel(&mut self) -> bool {
        let Some(state) = self.quick_browser_session_mut() else {
            return false;
        };
        state.mark_loading();
        true
    }

    pub fn reenable_sftp_follow(&mut self) -> bool {
        let Some(session_id) = ({
            let Some(state) = self.quick_browser_session_mut() else {
                return false;
            };
            let path = if state.current_path.is_empty() {
                "/".to_string()
            } else {
                state.current_path.clone()
            };
            state.reenable_follow(path);
            Some(state.file_browser_session_id.clone())
        }) else {
            return false;
        };
        self.quick_browser_state.path_editing = false;
        let _ = self.refresh_sftp_panel_render_cache(session_id.as_str());
        true
    }

    pub fn quick_browser_connection_badge(&self) -> String {
        self.quick_browser_session()
            .map(|state| state.host_profile_ref.label.clone())
            .unwrap_or_default()
    }

    pub fn take_pending_sftp_context_action(&mut self) -> Option<PendingSftpContextAction> {
        self.pending_sftp_context_action.take()
    }

    pub fn quick_browser_follows_active_terminal(&self) -> bool {
        self.quick_browser_state.follows_active_terminal
    }

    pub fn quick_browser_linked_terminal_session_id(&self) -> Option<&str> {
        if self.quick_browser_state.follows_active_terminal
            && let Some(pending_session_id) = self
                .quick_browser_state
                .pending_terminal_session_id
                .as_deref()
        {
            return Some(pending_session_id);
        }
        self.quick_browser_session()
            .and_then(|state| state.linked_terminal_session_id.as_deref())
            .or_else(|| self.quick_browser_session_id())
    }

    pub fn quick_browser_binding_mode_label(&self) -> &'static str {
        if self.quick_browser_state.follows_active_terminal {
            "Follow"
        } else {
            "Locked"
        }
    }

    pub fn quick_browser_path_editing(&self) -> bool {
        self.quick_browser_state.path_editing
    }

    pub fn defer_quick_browser_follow_to_active_terminal(&mut self) -> bool {
        if !self.quick_browser_state.follows_active_terminal {
            return false;
        }
        let Some(active_session_id) = self.active_workspace_terminal_session_id() else {
            return false;
        };
        if self.quick_browser_session_id.as_deref() == Some(active_session_id)
            && self
                .quick_browser_state
                .pending_terminal_session_id
                .as_deref()
                == Some(active_session_id)
        {
            return false;
        }
        self.quick_browser_state.pending_terminal_session_id = Some(active_session_id.to_string());
        true
    }

    pub fn toggle_quick_browser_binding_mode(&mut self) -> bool {
        self.quick_browser_state.follows_active_terminal =
            !self.quick_browser_state.follows_active_terminal;
        self.quick_browser_state.pending_terminal_session_id = None;
        self.quick_browser_state.path_editing = false;
        self.quick_browser_state.drop_target_active = false;
        true
    }

    pub fn begin_quick_browser_path_edit(&mut self) -> bool {
        if self.quick_browser_session().is_none() {
            return false;
        }
        self.quick_browser_state.path_editing = true;
        true
    }

    pub fn has_workspace_sftp_tabs_for_terminal_session(&self, session_id: &str) -> bool {
        self.workspace_tabs.iter().any(|tab| {
            tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp
                && self
                    .file_browser_sessions
                    .get(tab.file_browser_session_id.as_str())
                    .and_then(|browser_session| {
                        browser_session.linked_terminal_session_id.as_deref()
                    })
                    == Some(session_id)
        })
    }

    pub fn has_connected_terminal_session(&self, session_id: &str) -> bool {
        self.workspace_tabs.iter().any(|tab| {
            tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
                && tab.session_id == session_id
                && tab.state == "connected"
        })
    }

    pub fn transfer_task_by_id(&self, task_id: &str) -> Option<&crate::app::sftp::TransferTask> {
        self.sftp_transfer_tasks
            .iter()
            .find(|task| task.id == task_id)
    }

    pub fn open_transfer_conflict_modal(&mut self, task_id: &str) -> bool {
        let Some(task) = self.transfer_task_by_id(task_id).cloned() else {
            return false;
        };
        if task.state != crate::app::sftp::TransferTaskState::Conflict {
            return false;
        }
        let batch_task_ids = self.transfer_conflict_batch_task_ids(&task);

        self.sftp_conflict_modal_state = SftpConflictModalState {
            open: true,
            kind: conflict_modal_kind(&task),
            task_id: Some(task.id),
            source_path: task.source_path,
            target_path: task.target_path,
            batch_task_ids,
            apply_to_batch: false,
        };
        true
    }

    pub fn close_sftp_conflict_modal(&mut self) -> bool {
        if !self.sftp_conflict_modal_state.open {
            return false;
        }
        self.sftp_conflict_modal_state = SftpConflictModalState::default();
        true
    }

    pub fn active_sftp_conflict_tasks(&self) -> Vec<crate::app::sftp::TransferTask> {
        let task_ids = if self.sftp_conflict_modal_state.apply_to_batch
            && self.sftp_conflict_modal_state.batch_task_ids.len() > 1
        {
            self.sftp_conflict_modal_state.batch_task_ids.clone()
        } else {
            self.sftp_conflict_modal_state
                .task_id
                .iter()
                .cloned()
                .collect()
        };

        task_ids
            .into_iter()
            .filter_map(|task_id| self.transfer_task_by_id(task_id.as_str()).cloned())
            .filter(|task| task.state == crate::app::sftp::TransferTaskState::Conflict)
            .collect()
    }

    pub fn current_sftp_conflict_task(&self) -> Option<crate::app::sftp::TransferTask> {
        let task_id = self.sftp_conflict_modal_state.task_id.as_deref()?;
        self.transfer_task_by_id(task_id)
            .cloned()
            .filter(|task| task.state == crate::app::sftp::TransferTaskState::Conflict)
    }

    pub fn sftp_conflict_modal_batch_conflict_count(&self) -> i32 {
        let related = self
            .sftp_conflict_modal_state
            .batch_task_ids
            .len()
            .saturating_sub(1);
        i32::try_from(related).unwrap_or(i32::MAX)
    }

    pub fn sftp_conflict_modal_apply_to_batch(&self) -> bool {
        self.sftp_conflict_modal_state.apply_to_batch
            && self.sftp_conflict_modal_state.batch_task_ids.len() > 1
    }

    pub fn sftp_conflict_modal_kind_id(&self) -> &'static str {
        self.sftp_conflict_modal_state.kind.id()
    }

    pub fn set_sftp_conflict_modal_apply_to_batch(&mut self, apply_to_batch: bool) -> bool {
        if !self.sftp_conflict_modal_state.open {
            return false;
        }
        let next_value = apply_to_batch && self.sftp_conflict_modal_state.batch_task_ids.len() > 1;
        if self.sftp_conflict_modal_state.apply_to_batch == next_value {
            return false;
        }
        self.sftp_conflict_modal_state.apply_to_batch = next_value;
        true
    }

    pub fn mark_workspace_sftp_sessions_disconnected(&mut self, session_id: &str) -> bool {
        let mut changed = false;
        for tab in self
            .workspace_tabs
            .iter()
            .filter(|tab| tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp)
        {
            let Some(browser_session) = self
                .file_browser_sessions
                .get_mut(tab.file_browser_session_id.as_str())
            else {
                continue;
            };
            if browser_session.linked_terminal_session_id.as_deref() != Some(session_id) {
                continue;
            }
            if browser_session.mode != SftpPanelMode::Disconnected {
                browser_session.mark_disconnected();
                changed = true;
            }
        }
        changed
    }

    pub fn expand_quick_browser_to_workspace(
        &mut self,
    ) -> Option<crate::shell::tabs::WorkspaceTabId> {
        let quick_browser = self.quick_browser_session()?.clone();
        let workspace_session = quick_browser.clone_for_workspace();
        let tab_id = format!("workspace-{}", workspace_session.file_browser_session_id);
        let mut tab = WorkspaceTab::sftp(
            tab_id.clone(),
            workspace_session.file_browser_session_id.clone(),
            format!("Files: {}", workspace_session.host_profile_ref.label),
        );
        tab.state = workspace_session.mode.id().into();
        self.set_file_browser_session(workspace_session);
        self.workspace_tabs.push(tab);
        let _ = self.activate_workspace_tab(tab_id.as_str());
        Some(tab_id)
    }

    pub fn open_transfer_task_in_sftp_workspace(&mut self, task_id: &str) -> bool {
        let Some(task) = self.transfer_task_by_id(task_id).cloned() else {
            return false;
        };
        if !self.has_connected_terminal_session(task.session_id.as_str()) {
            return false;
        }
        let Some(remote_dir) = transfer_task_workspace_dir(&task) else {
            return false;
        };
        let session_id = task.session_id.clone();

        if let Some(existing_tab) = self.workspace_tabs.iter().find(|tab| {
            tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp
                && self
                    .file_browser_sessions
                    .get(tab.file_browser_session_id.as_str())
                    .and_then(|browser_session| {
                        browser_session.linked_terminal_session_id.as_deref()
                    })
                    == Some(session_id.as_str())
        }) {
            let browser_session_id = existing_tab.file_browser_session_id.clone();
            let tab_id = existing_tab.tab_id.clone();
            if let Some(browser_session) = self.file_browser_sessions.get_mut(&browser_session_id) {
                browser_session.navigate_manual(remote_dir);
                browser_session.selected_entry_ids.clear();
                browser_session.last_error = None;
            }
            let _ = self.refresh_sftp_panel_projection_cache(browser_session_id.as_str());
            let _ = self.activate_workspace_tab(tab_id.as_str());
            if !self.transfer_center_pinned {
                self.transfer_center_open = false;
            }
            return true;
        }

        let mut workspace_session = self
            .file_browser_sessions
            .get(session_id.as_str())
            .cloned()
            .map(|session| session.clone_for_workspace())
            .unwrap_or_else(|| {
                FileBrowserSession::quick_browser(
                    HostProfileRef::new(self.transfer_task_host_label(session_id.as_str())),
                    remote_dir.clone(),
                )
            });
        workspace_session.attach_terminal_session_id(session_id);
        workspace_session.navigate_manual(remote_dir);
        let tab_id = format!("workspace-{}", workspace_session.file_browser_session_id);
        let mut tab = WorkspaceTab::sftp(
            tab_id.clone(),
            workspace_session.file_browser_session_id.clone(),
            format!("Files: {}", workspace_session.host_profile_ref.label),
        );
        tab.state = workspace_session.mode.id().into();
        self.set_file_browser_session(workspace_session);
        self.workspace_tabs.push(tab);
        let _ = self.activate_workspace_tab(tab_id.as_str());
        if !self.transfer_center_pinned {
            self.transfer_center_open = false;
        }
        true
    }

    pub fn reconnect_active_sftp_workspace(&mut self) -> bool {
        let Some(active_tab_id) = self.active_workspace_tab_id().map(str::to_owned) else {
            return false;
        };
        let Some(browser_session) = self.active_workspace_sftp_session_mut() else {
            return false;
        };
        browser_session.mark_connecting();
        if let Some(tab) = self
            .workspace_tabs
            .iter_mut()
            .find(|tab| tab.tab_id == active_tab_id)
        {
            tab.state = SftpPanelMode::Connecting.id().into();
            tab.error_detail.clear();
        }
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

    pub fn transfer_task_host_label(&self, session_id: &str) -> String {
        self.file_browser_sessions
            .get(session_id)
            .map(|session| session.host_profile_ref.label.clone())
            .or_else(|| {
                self.workspace_tabs
                    .iter()
                    .find(|tab| {
                        tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
                            && tab.session_id == session_id
                    })
                    .map(|tab| tab.title.clone())
            })
            .unwrap_or_else(|| "Remote Files".into())
    }

    fn transfer_conflict_batch_task_ids(
        &self,
        task: &crate::app::sftp::TransferTask,
    ) -> Vec<String> {
        let scope_dir = transfer_conflict_scope_dir(task);
        let kind = conflict_modal_kind(task);

        self.sftp_transfer_tasks
            .iter()
            .filter(|candidate| {
                candidate.state == crate::app::sftp::TransferTaskState::Conflict
                    && candidate.session_id == task.session_id
                    && candidate.direction == task.direction
                    && conflict_modal_kind(candidate) == kind
                    && transfer_conflict_scope_dir(candidate) == scope_dir
            })
            .map(|candidate| candidate.id.clone())
            .collect()
    }
}

fn conflict_modal_kind(task: &crate::app::sftp::TransferTask) -> SftpConflictModalKind {
    match &task.action {
        crate::app::sftp::TransferTaskAction::Download { .. }
        | crate::app::sftp::TransferTaskAction::DownloadDirectory { .. } => {
            SftpConflictModalKind::Download
        }
        _ => SftpConflictModalKind::Remote,
    }
}

fn transfer_conflict_scope_dir(task: &crate::app::sftp::TransferTask) -> Option<String> {
    match &task.action {
        crate::app::sftp::TransferTaskAction::Download { .. }
        | crate::app::sftp::TransferTaskAction::DownloadDirectory { .. } => {
            local_parent_dir(task.target_path.as_str())
        }
        _ => remote_parent_dir(task.target_path.as_str()),
    }
}

fn transfer_task_workspace_dir(task: &crate::app::sftp::TransferTask) -> Option<String> {
    let path = match task.direction {
        crate::app::sftp::TransferDirection::Upload | crate::app::sftp::TransferDirection::Move => {
            task.target_path.as_str()
        }
        crate::app::sftp::TransferDirection::Download
        | crate::app::sftp::TransferDirection::Delete => task.source_path.as_str(),
    };
    remote_parent_dir(path)
}

fn build_sftp_panel_render_row(entry: &SftpDirectoryEntry, selected: bool) -> SftpPanelRenderRow {
    let kind = sftp_panel_entry_kind(entry);
    let type_label = sftp_panel_entry_type_label(kind);
    let size_label = sftp_panel_entry_size_label(entry);
    let modified_label = sftp_panel_entry_modified_label(entry);

    SftpPanelRenderRow {
        id: entry.id.clone(),
        name: entry.name.clone(),
        meta_label: sftp_panel_entry_meta_label(type_label, &size_label, &modified_label),
        type_label: type_label.to_string(),
        modified_label,
        size_label,
        kind: kind.to_string(),
        selected,
    }
}

fn sftp_panel_entry_type_label(kind: &str) -> &'static str {
    match kind {
        "directory" => "Folder",
        "symlink" => "Link",
        "archive" => "Archive",
        "image" => "Image",
        "config" => "Config",
        "executable" => "Executable",
        "unknown" => "Unknown",
        _ => "File",
    }
}

fn sftp_panel_entry_modified_label(entry: &SftpDirectoryEntry) -> String {
    let Some(unix_seconds) = entry.modified_unix_seconds else {
        return String::new();
    };
    let Some(timestamp) = DateTime::<Utc>::from_timestamp(unix_seconds as i64, 0) else {
        return String::new();
    };
    timestamp.format("%Y-%m-%d %H:%M").to_string()
}

fn sftp_panel_entry_size_label(entry: &SftpDirectoryEntry) -> String {
    entry.size_bytes.map(format_binary_size).unwrap_or_default()
}

fn sftp_panel_entry_meta_label(type_label: &str, size_label: &str, modified_label: &str) -> String {
    if type_label.is_empty() {
        return modified_label.to_string();
    }
    if !size_label.is_empty() && !modified_label.is_empty() {
        return format!("{type_label} · {size_label} · {modified_label}");
    }
    if !size_label.is_empty() {
        return format!("{type_label} · {size_label}");
    }
    if !modified_label.is_empty() {
        return format!("{type_label} · {modified_label}");
    }

    type_label.to_string()
}

fn format_binary_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn sftp_panel_entry_kind(entry: &SftpDirectoryEntry) -> &'static str {
    match entry.kind {
        crate::app::sftp::SftpDirectoryEntryKind::Directory => "directory",
        crate::app::sftp::SftpDirectoryEntryKind::Symlink => "symlink",
        crate::app::sftp::SftpDirectoryEntryKind::Unknown => "unknown",
        crate::app::sftp::SftpDirectoryEntryKind::File => {
            classify_sftp_file_visual_kind(entry.name.as_str())
        }
    }
}

fn classify_sftp_file_visual_kind(name: &str) -> &'static str {
    let lowercase = name.to_ascii_lowercase();
    if lowercase.ends_with(".tar.gz")
        || lowercase.ends_with(".tgz")
        || lowercase.ends_with(".tar.bz2")
        || lowercase.ends_with(".tbz2")
        || lowercase.ends_with(".tar.xz")
        || lowercase.ends_with(".txz")
        || has_any_suffix(
            lowercase.as_str(),
            &[".zip", ".tar", ".gz", ".bz2", ".xz", ".7z", ".rar", ".jar"],
        )
    {
        return "archive";
    }
    if has_any_suffix(
        lowercase.as_str(),
        &[
            ".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".svg", ".ico", ".tif", ".tiff",
            ".avif",
        ],
    ) {
        return "image";
    }
    if lowercase.starts_with(".env")
        || matches!(
            lowercase.as_str(),
            "dockerfile" | "compose.yml" | "compose.yaml" | "makefile" | "justfile"
        )
        || has_any_suffix(
            lowercase.as_str(),
            &[
                ".json",
                ".yaml",
                ".yml",
                ".toml",
                ".ini",
                ".cfg",
                ".conf",
                ".config",
                ".service",
                ".xml",
                ".lock",
                ".properties",
            ],
        )
    {
        return "config";
    }
    if matches!(
        lowercase.as_str(),
        "gradlew" | "configure" | "install" | "run" | "start" | "deploy"
    ) || has_any_suffix(
        lowercase.as_str(),
        &[
            ".sh",
            ".bash",
            ".zsh",
            ".fish",
            ".ps1",
            ".cmd",
            ".bat",
            ".exe",
            ".bin",
            ".run",
            ".appimage",
        ],
    ) {
        return "executable";
    }
    "file"
}

fn has_any_suffix(value: &str, suffixes: &[&str]) -> bool {
    suffixes.iter().any(|suffix| value.ends_with(suffix))
}

fn local_parent_dir(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }

    std::path::Path::new(trimmed)
        .parent()
        .map(|parent| parent.to_string_lossy().to_string())
}

fn remote_parent_dir(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed == "/" {
        return Some("/".into());
    }

    let normalized = trimmed.trim_end_matches('/');
    if normalized.is_empty() {
        return Some("/".into());
    }
    match normalized.rsplit_once('/') {
        Some(("", _)) => Some("/".into()),
        Some((parent, _)) => Some(parent.to_string()),
        None => Some("/".into()),
    }
}
