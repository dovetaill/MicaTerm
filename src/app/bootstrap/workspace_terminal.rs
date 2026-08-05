//! Bootstrap workspace terminal module.

use super::*;
use crate::ClipboardImagePreviewItem;
use crate::app::clipboard::{
    ClipboardImagePreview, ClipboardImageSource, ClipboardPayload, EncodedClipboardImage,
    encode_clipboard_image, select_clipboard_payload, system_clipboard_image_source,
};
use crate::app::clipboard_image_paste::{
    ClipboardImageBindingContext, ClipboardImageCompletion, ClipboardImagePasteController,
    ClipboardImagePasteRegisterError, ClipboardImageUploadJob,
};
use crate::app::sftp::SftpRuntimeHandle;

pub(super) enum ClipboardImagePasteBackgroundMessage {
    Prepared {
        request_id: Uuid,
        result: std::result::Result<EncodedClipboardImage, String>,
    },
    Uploaded {
        request_id: Uuid,
        result: std::result::Result<String, String>,
    },
}

pub(super) type WorkspaceClipboardImagePasteController =
    ClipboardImagePasteController<SftpRuntimeHandle>;

pub(super) fn sync_clipboard_image_paste_preview(
    window: &AppWindow,
    controller: &WorkspaceClipboardImagePasteController,
    active_session_id: Option<Uuid>,
    fingerprint: &mut Option<(u64, Option<Uuid>)>,
) -> bool {
    let next_fingerprint = (controller.revision(), active_session_id);
    if *fingerprint == Some(next_fingerprint) {
        return false;
    }

    let items = active_session_id
        .map(|session_id| controller.projections(session_id))
        .unwrap_or_default()
        .into_iter()
        .map(|projection| ClipboardImagePreviewItem {
            request_id: projection.request_id.to_string().into(),
            thumbnail: projection
                .preview
                .as_ref()
                .map(slint_image_from_clipboard_preview)
                .unwrap_or_default(),
            source_width: i32::try_from(projection.source_width).unwrap_or(i32::MAX),
            source_height: i32::try_from(projection.source_height).unwrap_or(i32::MAX),
            status: projection.status.into(),
            detail: projection.detail.into(),
            paste_enabled: projection.paste_enabled,
            copy_enabled: projection.copy_enabled,
            collapsed: projection.collapsed,
        })
        .collect::<Vec<_>>();
    window.set_workspace_session_clipboard_image_preview_items(ModelRc::new(VecModel::from(items)));
    *fingerprint = Some(next_fingerprint);
    true
}

fn slint_image_from_clipboard_preview(preview: &ClipboardImagePreview) -> slint::Image {
    let mut pixels =
        slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(preview.width, preview.height);
    pixels
        .make_mut_bytes()
        .copy_from_slice(preview.rgba.as_slice());
    slint::Image::from_rgba8(pixels)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WorkspaceTerminalLinkAffordance {
    pub hovered: bool,
    pub armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkspaceTerminalPointerState {
    pub session_id: Uuid,
    pub row: u32,
    pub col: u32,
    pub ctrl: bool,
}

pub(super) fn sync_workspace_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> WorkspaceProjectionDelta {
    let mut claimed_manager_tab_ids = HashSet::new();
    let mut next_tabs = manager
        .ordered_sessions()
        .into_iter()
        .filter(|handle| {
            !state.workspace_terminal_session_hidden(handle.session_id.to_string().as_str())
        })
        .map(|handle| {
            let existing = state
                .workspace_tabs()
                .iter()
                .find(|tab| {
                    !claimed_manager_tab_ids.contains(&tab.tab_id)
                        && (tab.session_id == handle.session_id.to_string()
                            || (tab.kind == crate::shell::tabs::WorkspaceTabKind::Terminal
                                && tab.session_id.is_empty()
                                && !tab.asset_id.is_empty()
                                && tab.asset_id == handle.asset_id))
                })
                .cloned();
            let mut projected = existing
                .as_ref()
                .map(|tab| WorkspaceTab::from_session_with_tab_id(&handle, tab.tab_id.clone()))
                .unwrap_or_else(|| WorkspaceTab::from_session(&handle));
            if let Some(existing) = existing {
                claimed_manager_tab_ids.insert(existing.tab_id.clone());
                projected.connection_profile = existing
                    .connection_profile
                    .as_ref()
                    .and_then(super::cloneable_workspace_tab_connection_profile);
            }
            if projected.connection_profile.is_none() && !projected.asset_id.is_empty() {
                projected.connection_profile = super::runtime_cloneable_profile_for_saved_asset(
                    state,
                    projected.asset_id.as_str(),
                )
                .ok();
            }
            projected
        })
        .collect::<Vec<_>>();
    let manager_session_ids = next_tabs
        .iter()
        .map(|tab| tab.session_id.clone())
        .collect::<HashSet<_>>();
    let preserved_error_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| {
            tab.state == "error"
                && !manager_session_ids.contains(&tab.session_id)
                && !claimed_manager_tab_ids.contains(&tab.tab_id)
        })
        .cloned()
        .collect::<Vec<_>>();
    let preserved_launcher_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| tab.is_launcher())
        .cloned()
        .collect::<Vec<_>>();
    let preserved_sftp_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| tab.kind == crate::shell::tabs::WorkspaceTabKind::Sftp)
        .cloned()
        .collect::<Vec<_>>();
    next_tabs.extend(preserved_error_tabs);
    next_tabs.extend(preserved_launcher_tabs);
    next_tabs.extend(sync_workspace_sftp_tabs(
        state,
        manager,
        preserved_sftp_tabs,
    ));
    next_tabs = state.normalized_workspace_tabs_projection(next_tabs);
    let projected_active_session_id = next_tabs.iter().find(|tab| tab.active).and_then(|tab| {
        if !tab.uses_terminal_surface()
            && !tab.uses_connection_progress_surface()
            && !tab.can_reconnect()
        {
            return None;
        }

        Uuid::parse_str(tab.session_id.as_str()).ok()
    });
    let next_session_id = if state.active_workspace_tab_id().is_none()
        && state.active_workspace_terminal_surface().is_none()
    {
        None
    } else {
        projected_active_session_id
    };
    let current_surface_signature = state
        .active_workspace_terminal_surface()
        .map(TerminalSurfaceState::signature);
    let next_surface_signature =
        next_session_id.and_then(|session_id| manager.terminal_surface_signature(session_id));

    let tabs_changed = state.workspace_tabs() != next_tabs.as_slice();
    if tabs_changed {
        state.set_workspace_tabs(next_tabs);
    }

    let surface_changed = current_surface_signature != next_surface_signature;
    if surface_changed {
        let next_surface =
            next_session_id.and_then(|session_id| manager.terminal_surface(session_id));
        state.set_active_workspace_terminal_surface(next_surface);
    }

    let sftp_changed = super::sftp::sync_active_sftp_projection_from_manager(state, manager);

    WorkspaceProjectionDelta {
        tabs_changed,
        surface_changed,
        sftp_changed,
    }
}

fn sync_workspace_sftp_tabs(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    tabs: Vec<WorkspaceTab>,
) -> Vec<WorkspaceTab> {
    tabs.into_iter()
        .map(|mut tab| {
            let linked_session_id_text = state
                .file_browser_sessions
                .get(tab.file_browser_session_id.as_str())
                .and_then(|browser_session| browser_session.linked_terminal_session_id.clone());
            let linked_session_state = linked_session_id_text
                .as_deref()
                .and_then(|session_id| Uuid::parse_str(session_id).ok())
                .and_then(|session_id| manager.session(session_id))
                .map(|session| session.state);
            let binding_disconnected = linked_session_id_text
                .as_deref()
                .and_then(|session_id| Uuid::parse_str(session_id).ok())
                .and_then(|session_id| manager.sftp_binding(session_id))
                .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected);
            let reconnecting_terminal = matches!(
                linked_session_state,
                Some(SessionState::Connecting | SessionState::WaitingUser)
            );

            let Some(browser_session) = state
                .file_browser_sessions
                .get_mut(tab.file_browser_session_id.as_str())
            else {
                tab.state = "disconnected".into();
                tab.error_detail = "SFTP workspace browser session is unavailable.".into();
                return tab;
            };

            if binding_disconnected && !reconnecting_terminal {
                browser_session.mark_disconnected();
            }

            tab.state = browser_session.mode.id().into();
            if browser_session.mode == SftpPanelMode::Disconnected {
                tab.error_detail =
                    "Reconnect the file workspace to restore remote browsing.".into();
            } else if let Some(last_error) = browser_session.last_error.as_deref() {
                tab.error_detail = last_error.to_string();
            } else {
                tab.error_detail.clear();
            }
            tab
        })
        .collect()
}

pub(super) fn snap_active_workspace_viewport_to_bottom_if_needed(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };
    snap_workspace_session_viewport_to_bottom_if_needed(state, &bridge.manager, session_id);
}

fn snap_workspace_session_viewport_to_bottom_if_needed(
    state: &ShellViewModel,
    manager: &SessionManager,
    session_id: Uuid,
) {
    if active_workspace_session_uuid(state) != Some(session_id) {
        return;
    }
    let needs_snap = state
        .active_workspace_terminal_surface()
        .is_some_and(|surface| !surface.viewport_at_bottom);
    if !needs_snap {
        return;
    }

    if let Err(err) = manager.scroll_session_to_bottom(session_id) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to snap workspace terminal viewport to bottom"
        );
    }
}

pub(super) fn apply_local_input_projection_hint(state: &mut ShellViewModel) -> bool {
    let Some(mut surface) = state.active_workspace_terminal_surface().cloned() else {
        return false;
    };
    if surface.viewport_at_bottom && surface.viewport_offset_lines == 0 {
        return false;
    }

    surface.viewport_offset_lines = 0;
    surface.viewport_at_bottom = true;
    state.set_active_workspace_terminal_surface(Some(surface));
    true
}

pub(super) fn normalize_active_workspace_hit_col(
    state: &ShellViewModel,
    row: i32,
    col: i32,
) -> i32 {
    let safe_row = row.max(0) as u32;
    let safe_col = col.max(0) as u32;
    state
        .active_workspace_terminal_surface()
        .map(|surface| surface.normalize_hit_col(safe_row, safe_col) as i32)
        .unwrap_or(col.max(0))
}

pub(super) fn normalize_active_workspace_selection_hit_col(
    state: &ShellViewModel,
    row: i32,
    col: i32,
) -> i32 {
    let safe_row = row.max(0) as u32;
    let safe_col = col.max(0) as u32;
    state
        .active_workspace_terminal_surface()
        .map(|surface| surface.normalize_selection_hit_col(safe_row, safe_col) as i32)
        .unwrap_or(col.max(0))
}

pub(super) fn clear_invalid_active_workspace_terminal_selection(
    state: &mut ShellViewModel,
) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface().cloned() else {
        let selection_changed = state.set_workspace_terminal_selection(None);
        let drag_changed = state.set_workspace_terminal_selection_drag(None);
        return selection_changed || drag_changed;
    };

    let mut changed = false;
    if state
        .workspace_terminal_selection_drag()
        .is_some_and(|drag| {
            drag.session_id == surface.session_id && !drag.matches_surface(&surface)
        })
    {
        changed |= state.clear_active_workspace_terminal_selection_drag();
        changed |= state.clear_active_workspace_terminal_selection();
    }

    if let Some(drag) = state.active_workspace_terminal_selection_drag() {
        let next = drag.selection_for_surface(&surface).map(|range| {
            crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(&surface, range)
        });
        changed |= state.set_workspace_terminal_selection(next);
    } else if state
        .active_workspace_terminal_selection()
        .is_some_and(|selection| !selection.matches_surface(&surface))
    {
        changed |= state.clear_active_workspace_terminal_selection();
    }

    changed
}

pub(super) fn active_workspace_terminal_selection(
    state: &ShellViewModel,
) -> Option<crate::app::terminal_model::WorkspaceTerminalSelection> {
    let surface = state.active_workspace_terminal_surface()?;
    state
        .active_workspace_terminal_selection()
        .filter(|selection| selection.matches_surface(surface))
}

pub(super) fn active_workspace_terminal_selection_buffer_range(
    state: &ShellViewModel,
) -> Option<crate::app::terminal_model::TerminalSelectionModel> {
    active_workspace_terminal_selection(state)
        .map(|selection| selection.range)
        .filter(|range| !empty_selection(*range))
}

pub(super) fn active_workspace_terminal_selection_drag_active(state: &ShellViewModel) -> bool {
    state.active_workspace_terminal_selection_drag().is_some()
}

fn empty_selection(range: crate::app::terminal_model::TerminalSelectionModel) -> bool {
    range.start_row == range.end_row && range.start_col == range.end_col
}

pub(super) fn begin_active_workspace_terminal_selection(
    state: &mut ShellViewModel,
    gesture_mode: i32,
    row: i32,
    col: i32,
    selection_col: i32,
) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface().cloned() else {
        return false;
    };

    let safe_row = row.max(0) as u32;
    let safe_col = col.max(0) as u32;
    let safe_selection_col = selection_col.max(0) as u32;
    let mode = crate::app::ssh::runtime::TerminalSelectionGestureMode::from_code(gesture_mode);
    let drag = match mode {
        crate::app::ssh::runtime::TerminalSelectionGestureMode::Cell => {
            crate::app::terminal_model::WorkspaceTerminalSelectionDrag::cell_from_surface(
                &surface,
                safe_row,
                safe_col,
                safe_selection_col,
            )
        }
        crate::app::ssh::runtime::TerminalSelectionGestureMode::Word
        | crate::app::ssh::runtime::TerminalSelectionGestureMode::Line => {
            crate::app::terminal_model::WorkspaceTerminalSelectionDrag::expanded_from_surface(
                &surface,
                mode,
                safe_row,
                safe_col,
                safe_selection_col,
            )
        }
    };
    let next_selection = drag.selection_for_surface(&surface).map(|range| {
        crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(&surface, range)
    });

    state.set_workspace_terminal_selection_drag(Some(drag))
        || state.set_workspace_terminal_selection(next_selection)
}

pub(super) fn update_active_workspace_terminal_selection(
    state: &mut ShellViewModel,
    row: i32,
    col: i32,
    selection_col: i32,
) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface().cloned() else {
        return false;
    };
    let Some(mut drag) = state.active_workspace_terminal_selection_drag() else {
        return false;
    };

    drag.update_pointer(
        row.max(0) as u32,
        col.max(0) as u32,
        selection_col.max(0) as u32,
    );
    let next_selection = drag.selection_for_surface(&surface).map(|range| {
        crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(&surface, range)
    });

    state.set_workspace_terminal_selection_drag(Some(drag))
        || state.set_workspace_terminal_selection(next_selection)
}

pub(super) fn remember_active_workspace_terminal_selection_pointer(
    state: &mut ShellViewModel,
    row: i32,
    col: i32,
    selection_col: i32,
) -> bool {
    let Some(mut drag) = state.active_workspace_terminal_selection_drag() else {
        return false;
    };
    drag.update_pointer(
        row.max(0) as u32,
        col.max(0) as u32,
        selection_col.max(0) as u32,
    );
    state.set_workspace_terminal_selection_drag(Some(drag))
}

pub(super) fn finish_active_workspace_terminal_selection(state: &mut ShellViewModel) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface().cloned() else {
        let selection_changed = state.set_workspace_terminal_selection(None);
        let drag_changed = state.set_workspace_terminal_selection_drag(None);
        return selection_changed || drag_changed;
    };
    let Some(drag) = state.active_workspace_terminal_selection_drag() else {
        return false;
    };

    let next_selection = drag.selection_for_surface(&surface).map(|range| {
        crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(&surface, range)
    });
    let selection_changed = match next_selection {
        Some(selection) if !empty_selection(selection.range) => {
            state.set_workspace_terminal_selection(Some(selection))
        }
        _ => state.clear_active_workspace_terminal_selection(),
    };
    let drag_changed = state.clear_active_workspace_terminal_selection_drag();
    selection_changed || drag_changed
}

pub(super) fn select_all_active_workspace_terminal(state: &mut ShellViewModel) -> bool {
    let Some(surface) = state.active_workspace_terminal_surface().cloned() else {
        return false;
    };

    let end_row = surface
        .rows
        .saturating_add(surface.viewport_max_offset_lines)
        .saturating_sub(1);
    let next_selection = crate::app::terminal_model::WorkspaceTerminalSelection::from_surface(
        &surface,
        crate::app::terminal_model::TerminalSelectionModel::new(0, 0, end_row, surface.cols),
    );

    let selection_changed = state.set_workspace_terminal_selection(Some(next_selection));
    let drag_changed = state.clear_active_workspace_terminal_selection_drag();
    selection_changed || drag_changed
}

pub(super) fn sync_active_workspace_terminal_selection_projection(
    window: &AppWindow,
    state: &ShellViewModel,
) {
    let selection = active_workspace_terminal_selection_buffer_range(state);
    let drag_active = active_workspace_terminal_selection_drag_active(state);
    window.set_workspace_session_selection_active(selection.is_some());
    window.set_workspace_session_selection_drag_active(drag_active);
    window.set_workspace_session_selection_start_row(
        selection
            .map(|selection| i32::try_from(selection.start_row).unwrap_or(i32::MAX))
            .unwrap_or(-1),
    );
    window.set_workspace_session_selection_start_col(
        selection
            .map(|selection| i32::try_from(selection.start_col).unwrap_or(i32::MAX))
            .unwrap_or(-1),
    );
    window.set_workspace_session_selection_end_row(
        selection
            .map(|selection| i32::try_from(selection.end_row).unwrap_or(i32::MAX))
            .unwrap_or(-1),
    );
    window.set_workspace_session_selection_end_col(
        selection
            .map(|selection| i32::try_from(selection.end_col).unwrap_or(i32::MAX))
            .unwrap_or(-1),
    );
}

pub(super) fn openable_url_at_active_workspace_surface(
    state: &ShellViewModel,
    row: u32,
    col: u32,
) -> Option<String> {
    let surface = state.active_workspace_terminal_surface()?;
    openable_url_at_surface(surface, row, col)
}

pub(super) fn link_affordance_at_active_workspace_surface(
    state: &ShellViewModel,
    row: u32,
    col: u32,
    ctrl: bool,
) -> WorkspaceTerminalLinkAffordance {
    state
        .active_workspace_terminal_surface()
        .map(|surface| link_affordance_at_surface(surface, row, col, ctrl))
        .unwrap_or_default()
}

pub(super) fn link_affordance_for_pointer(
    surface: Option<&TerminalSurfaceState>,
    pointer: Option<WorkspaceTerminalPointerState>,
) -> WorkspaceTerminalLinkAffordance {
    let Some(surface) = surface else {
        return WorkspaceTerminalLinkAffordance::default();
    };
    let Some(pointer) = pointer else {
        return WorkspaceTerminalLinkAffordance::default();
    };
    if pointer.session_id != surface.session_id {
        return WorkspaceTerminalLinkAffordance::default();
    }

    link_affordance_at_surface(surface, pointer.row, pointer.col, pointer.ctrl)
}

pub(super) fn openable_url_at_surface(
    surface: &TerminalSurfaceState,
    row: u32,
    col: u32,
) -> Option<String> {
    if !surface_allows_link_affordance(surface) {
        return None;
    }

    url_token_hit_at_surface(surface, row, col).map(|(_, _, url)| url)
}

pub(super) fn link_affordance_at_surface(
    surface: &TerminalSurfaceState,
    row: u32,
    col: u32,
    ctrl: bool,
) -> WorkspaceTerminalLinkAffordance {
    if !surface_allows_link_affordance(surface) {
        return WorkspaceTerminalLinkAffordance::default();
    }

    if openable_url_at_surface(surface, row, col).is_some() {
        WorkspaceTerminalLinkAffordance {
            hovered: true,
            armed: ctrl,
        }
    } else {
        WorkspaceTerminalLinkAffordance::default()
    }
}

fn surface_allows_link_affordance(surface: &TerminalSurfaceState) -> bool {
    !surface.alternate_screen_active && !surface.mouse_grabbed && !surface.application_cursor_keys
}

fn url_token_hit_at_surface(
    surface: &TerminalSurfaceState,
    row: u32,
    col: u32,
) -> Option<(u32, u32, String)> {
    if surface.cols == 0 {
        return None;
    }

    let safe_col = col.min(surface.cols.saturating_sub(1));
    let _ = token_char_at_surface(surface, row, safe_col)?;

    let mut start_col = safe_col;
    while start_col > 0 && token_char_at_surface(surface, row, start_col - 1).is_some() {
        start_col -= 1;
    }

    let mut end_col = safe_col;
    while end_col + 1 < surface.cols && token_char_at_surface(surface, row, end_col + 1).is_some() {
        end_col += 1;
    }

    let token = (start_col..=end_col)
        .filter_map(|candidate_col| token_char_at_surface(surface, row, candidate_col))
        .collect::<String>();
    let trimmed = trim_openable_url_token(token.as_str())?;
    let trimmed_width = trimmed.chars().count() as u32;
    Some((
        start_col,
        start_col.saturating_add(trimmed_width),
        trimmed.to_string(),
    ))
}

fn token_char_at_surface(surface: &TerminalSurfaceState, row: u32, col: u32) -> Option<char> {
    let cell = surface.cells.iter().find(|cell| {
        cell.row == row && cell.col == col && cell.width == 1 && !cell.text.trim().is_empty()
    })?;
    let mut chars = cell.text.chars();
    let ch = chars.next()?;
    if chars.next().is_some() || ch.is_whitespace() {
        return None;
    }
    Some(ch)
}

fn trim_openable_url_token(token: &str) -> Option<&str> {
    let trimmed = token.trim_end_matches(|ch: char| {
        matches!(
            ch,
            '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}' | '>' | '"' | '\''
        )
    });

    let minimum_len = supported_openable_url_scheme_len(trimmed)?;

    (trimmed.len() > minimum_len).then_some(trimmed)
}

fn supported_openable_url_scheme_len(token: &str) -> Option<usize> {
    const SUPPORTED_SCHEMES: &[&str] = &[
        "https://", "http://", "ssh://", "ftp://", "ftps://", "sftp://",
    ];

    SUPPORTED_SCHEMES
        .iter()
        .find(|scheme| token.starts_with(**scheme))
        .map(|scheme| scheme.len())
}

pub(super) fn refresh_projection_after_local_input_hint(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let _ = sync_active_workspace_surface_projection_from_manager(state, &bridge.manager);
    sync_workspace_session_state_with_manager(window, state, follow_tracker, Some(&bridge.manager));
}

pub(super) fn refresh_active_terminal_surface_only(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some(bridge) = bridge else {
        return;
    };
    if sync_active_workspace_surface_projection_from_manager(state, &bridge.manager) {
        sync_workspace_session_state_with_manager(
            window,
            state,
            follow_tracker,
            Some(&bridge.manager),
        );
    }
}

pub(super) fn refresh_active_terminal_scroll_projection_only(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    _follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some(bridge) = bridge else {
        return;
    };
    if sync_active_workspace_surface_projection_from_manager(state, &bridge.manager) {
        super::sync_workspace_terminal_surface_projection_only(window, state);
    }
}

pub(super) fn refresh_active_workspace_surface_projection(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    refresh_active_terminal_surface_only(window, state, bridge, follow_tracker);
}

pub(super) fn sync_active_workspace_surface_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return false;
    };
    let current_surface_signature = state
        .active_workspace_terminal_surface()
        .map(TerminalSurfaceState::signature);
    let next_surface_signature = manager.terminal_surface_signature(session_id);
    if current_surface_signature == next_surface_signature {
        return false;
    }
    let next_surface = manager.terminal_surface(session_id);
    state.set_active_workspace_terminal_surface(next_surface);
    true
}

pub(super) fn refresh_active_workspace_projection(
    window: &AppWindow,
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    follow_tracker: &mut WorkspaceFollowTracker,
) {
    let Some(bridge) = bridge else {
        return;
    };

    let projection = sync_workspace_projection_from_manager(state, &bridge.manager);
    if projection.any_changed() {
        sync_workspace_tabs_with_manager(window, state, follow_tracker, Some(&bridge.manager));
        if projection.sftp_changed {
            super::sftp::sync_right_panel_state(window, state);
        }
    }
}

pub(super) fn schedule_workspace_input_projection_refresh(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    bridge: Option<Rc<ShellSessionBridge>>,
    follow_tracker: Rc<RefCell<WorkspaceFollowTracker>>,
    timer: Rc<Timer>,
    gate: Rc<RefCell<DeferredWorkspaceProjectionRefreshGate>>,
) {
    {
        let mut gate = gate.borrow_mut();
        if !gate.mark_scheduled() {
            return;
        }
    }

    let window_handle = window.as_weak();
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(WORKSPACE_INPUT_PROJECTION_DEBOUNCE_MS),
        move || {
            gate.borrow_mut().clear();
            let Some(window) = window_handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            refresh_active_workspace_surface_projection(
                &window,
                &mut state,
                bridge.as_deref(),
                &mut follow_tracker.borrow_mut(),
            );
        },
    );
}

pub(super) fn schedule_workspace_scroll_projection_refresh(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    bridge: Option<Rc<ShellSessionBridge>>,
    follow_tracker: Rc<RefCell<WorkspaceFollowTracker>>,
    timer: Rc<Timer>,
    gate: Rc<RefCell<DeferredWorkspaceProjectionRefreshGate>>,
) {
    {
        let mut gate = gate.borrow_mut();
        if !gate.mark_scheduled() {
            return;
        }
    }

    let window_handle = window.as_weak();
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(WORKSPACE_SCROLL_VIEWPORT_PROJECTION_DEBOUNCE_MS),
        move || {
            gate.borrow_mut().clear();
            let Some(window) = window_handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            refresh_active_terminal_scroll_projection_only(
                &window,
                &mut state,
                bridge.as_deref(),
                &mut follow_tracker.borrow_mut(),
            );
        },
    );
}

pub(super) fn schedule_workspace_scroll_thumb_drag_update(
    window: &AppWindow,
    ratio: f32,
    state: Rc<RefCell<ShellViewModel>>,
    bridge: Option<Rc<ShellSessionBridge>>,
    follow_tracker: Rc<RefCell<WorkspaceFollowTracker>>,
    timer: Rc<Timer>,
    deferred_drag: Rc<RefCell<DeferredWorkspaceScrollThumbDrag>>,
) {
    {
        let mut deferred_drag = deferred_drag.borrow_mut();
        if !deferred_drag.queue_ratio(ratio) {
            return;
        }
    }

    let window_handle = window.as_weak();
    timer.start(
        TimerMode::SingleShot,
        Duration::from_millis(WORKSPACE_SCROLL_THUMB_DRAG_PROJECTION_DEBOUNCE_MS),
        move || {
            let ratio = {
                let mut deferred_drag = deferred_drag.borrow_mut();
                let Some(ratio) = deferred_drag.take_latest_ratio() else {
                    return;
                };
                ratio
            };
            let Some(window) = window_handle.upgrade() else {
                return;
            };
            {
                let state = state.borrow();
                forward_active_workspace_scroll_ratio(&state, bridge.as_deref(), ratio);
            }
            let mut state = state.borrow_mut();
            refresh_active_terminal_scroll_projection_only(
                &window,
                &mut state,
                bridge.as_deref(),
                &mut follow_tracker.borrow_mut(),
            );
        },
    );
}

pub(super) fn forward_active_workspace_text_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    text: &str,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };
    if text.is_empty() {
        return;
    }

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));
    image_paste_controller.note_terminal_input(session_id);

    if let Err(err) = bridge
        .manager
        .send_session_text_input(session_id, text.to_string())
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to forward workspace terminal text input"
        );
    }
}

pub(super) fn forward_active_workspace_key_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let Some(event) = terminal_key_event(key_name, alt, ctrl, shift) else {
        return;
    };

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));
    image_paste_controller.note_terminal_input(session_id);

    if let Err(err) = bridge.manager.send_session_key_input(session_id, event) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            key = key_name,
            error = %err,
            "failed to forward workspace terminal key input"
        );
    }
}

pub(super) fn terminal_key_event(
    key_name: &str,
    alt: bool,
    ctrl: bool,
    shift: bool,
) -> Option<TerminalKeyEvent> {
    if let Some(number) = key_name
        .strip_prefix('f')
        .and_then(|suffix| suffix.parse::<u8>().ok())
        .filter(|number| (1..=24).contains(number))
    {
        return Some(TerminalKeyEvent::function(number, alt, ctrl, shift));
    }

    if key_name.chars().count() == 1 {
        return key_name
            .chars()
            .next()
            .map(|ch| TerminalKeyEvent::character(ch, alt, ctrl, shift));
    }

    match key_name {
        "enter" => Some(TerminalKeyEvent::named("enter", alt, ctrl, shift)),
        "tab" => Some(TerminalKeyEvent::named("tab", alt, ctrl, shift)),
        "escape" => Some(TerminalKeyEvent::named("escape", alt, ctrl, shift)),
        "backspace" => Some(TerminalKeyEvent::named("backspace", alt, ctrl, shift)),
        "insert" => Some(TerminalKeyEvent::named("insert", alt, ctrl, shift)),
        "delete" => Some(TerminalKeyEvent::named("delete", alt, ctrl, shift)),
        "up" => Some(TerminalKeyEvent::named("up", alt, ctrl, shift)),
        "down" => Some(TerminalKeyEvent::named("down", alt, ctrl, shift)),
        "left" => Some(TerminalKeyEvent::named("left", alt, ctrl, shift)),
        "right" => Some(TerminalKeyEvent::named("right", alt, ctrl, shift)),
        "home" => Some(TerminalKeyEvent::named("home", alt, ctrl, shift)),
        "end" => Some(TerminalKeyEvent::named("end", alt, ctrl, shift)),
        "page-up" => Some(TerminalKeyEvent::named("page-up", alt, ctrl, shift)),
        "page-down" => Some(TerminalKeyEvent::named("page-down", alt, ctrl, shift)),
        _ => None,
    }
}

pub(super) fn forward_active_workspace_resize(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    rows: i32,
    cols: i32,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let rows = rows.max(1) as u32;
    let cols = cols.max(1) as u32;
    let viewport = bridge.terminal_defaults.viewport_metrics();
    if let Err(err) = bridge
        .manager
        .resize_session_with_viewport(session_id, rows, cols, viewport)
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            rows,
            cols,
            viewport_pixel_width = viewport.pixel_width,
            viewport_pixel_height = viewport.pixel_height,
            viewport_dpi = viewport.dpi,
            error = %err,
            "failed to forward workspace terminal resize"
        );
    }
}

pub(super) fn set_system_clipboard_text(text: &str) -> Result<()> {
    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text(text, slint::platform::Clipboard::DefaultClipboard);
        Ok(())
    })
    .map_err(anyhow::Error::from)
}

pub(super) fn system_clipboard_text() -> Option<String> {
    i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .ok()
    .flatten()
}

fn system_clipboard_payload() -> Result<Option<ClipboardPayload>> {
    let image = system_clipboard_image_source()?;
    Ok(select_clipboard_payload(image, system_clipboard_text))
}

pub(super) fn forward_active_workspace_copy_selection(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
) {
    let Some(surface) = state.active_workspace_terminal_surface() else {
        return;
    };

    let selection = active_workspace_terminal_selection_buffer_range(state).unwrap_or_else(|| {
        crate::app::terminal_model::TerminalSelectionModel::new(
            start_row.max(0) as u32,
            start_col.max(0) as u32,
            end_row.max(0) as u32,
            end_col.max(0) as u32,
        )
    });
    let text = active_workspace_session_uuid(state)
        .zip(bridge)
        .and_then(|(session_id, bridge)| {
            bridge
                .manager
                .selection_text_from_buffer_rows(
                    session_id,
                    selection.start_row,
                    selection.start_col,
                    selection.end_row,
                    selection.end_col,
                )
                .ok()
        })
        .unwrap_or_else(|| {
            surface.selection_text_from_buffer_rows(
                selection.start_row,
                selection.start_col,
                selection.end_row,
                selection.end_col,
            )
        });
    if text.is_empty() {
        return;
    }

    if let Err(err) = set_system_clipboard_text(&text) {
        tracing::error!(
            target: "app.ssh",
            error = %err,
            "failed to copy workspace terminal selection to clipboard"
        );
    }
}

pub(super) fn normalize_workspace_paste_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn workspace_paste_logical_line_count(text: &str) -> usize {
    let normalized = normalize_workspace_paste_text(text);
    let trimmed = normalized.trim_end_matches('\n');
    if trimmed.is_empty() {
        return usize::from(!text.is_empty());
    }

    trimmed.split('\n').count()
}

pub(super) fn workspace_paste_prompt_mode(
    state: &ShellViewModel,
    text: &str,
) -> Option<WorkspacePastePromptMode> {
    let normalized = normalize_workspace_paste_text(text);
    let logical_line_count = workspace_paste_logical_line_count(text);
    if normalized.chars().count() >= WORKSPACE_PASTE_EDITOR_CHAR_THRESHOLD {
        return Some(WorkspacePastePromptMode::Editor);
    }
    if logical_line_count < 2 {
        return None;
    }

    if logical_line_count >= WORKSPACE_PASTE_EDITOR_LINE_THRESHOLD {
        return Some(WorkspacePastePromptMode::Editor);
    }

    if state
        .active_workspace_terminal_surface()
        .is_some_and(|surface| surface.bracketed_paste_enabled)
    {
        None
    } else {
        Some(WorkspacePastePromptMode::Confirm)
    }
}

pub(super) fn forward_workspace_session_paste(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    session_id: Uuid,
    text: &str,
) {
    let Some(bridge) = bridge else {
        return;
    };
    if text.is_empty() {
        return;
    }

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));
    image_paste_controller.note_terminal_input(session_id);

    if let Err(err) = bridge
        .manager
        .send_session_paste(session_id, text.to_string())
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            error = %err,
            "failed to forward workspace terminal paste"
        );
    }
}

pub(super) fn forward_active_workspace_paste(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    pending_warning: &RefCell<Option<PendingWorkspacePasteWarning>>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    image_paste_result_tx: &std::sync::mpsc::Sender<ClipboardImagePasteBackgroundMessage>,
    image_preparation_gate: &Arc<tokio::sync::Semaphore>,
) -> WorkspacePasteRequestOutcome {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        tracing::warn!(
            target: "app.ssh",
            "ignored workspace paste request because no active terminal session is selected"
        );
        return WorkspacePasteRequestOutcome::Ignored;
    };
    let payload = match system_clipboard_payload() {
        Ok(Some(payload)) => payload,
        Ok(None) => {
            tracing::warn!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                "ignored workspace paste request because clipboard text could not be read"
            );
            return WorkspacePasteRequestOutcome::Ignored;
        }
        Err(error) => {
            tracing::warn!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                error = %error,
                "failed to inspect the clipboard image payload"
            );
            show_clipboard_image_upload_error(state, error.to_string());
            return WorkspacePasteRequestOutcome::Failed;
        }
    };

    let text = match payload {
        ClipboardPayload::Text(text) => text,
        ClipboardPayload::Image(image) => {
            let Some(bridge) = bridge else {
                show_clipboard_image_upload_error(
                    state,
                    "the SSH session bridge is unavailable".to_string(),
                );
                return WorkspacePasteRequestOutcome::Failed;
            };
            let runtime_handle = bridge.manager.runtime_handle();
            let (binding_id, sftp_runtime) = match bridge.manager.sftp_runtime_binding(session_id) {
                Ok(binding) => binding,
                Err(error) => {
                    show_clipboard_image_upload_error(state, error.to_string());
                    return WorkspacePasteRequestOutcome::Failed;
                }
            };
            pending_warning.borrow_mut().take();
            let request_id =
                match image_paste_controller.register(session_id, binding_id, sftp_runtime) {
                    Ok(request_id) => request_id,
                    Err(ClipboardImagePasteRegisterError::QueueFull) => {
                        show_clipboard_image_upload_error(
                            state,
                            "the clipboard image queue already contains 8 requests".to_string(),
                        );
                        return WorkspacePasteRequestOutcome::Failed;
                    }
                };
            schedule_clipboard_image_preparation(
                runtime_handle,
                request_id,
                image,
                image_paste_result_tx.clone(),
                Arc::clone(image_preparation_gate),
            );
            return WorkspacePasteRequestOutcome::UploadScheduled;
        }
    };
    let text = normalize_workspace_paste_text(&text);

    if let Some(prompt_mode) = workspace_paste_prompt_mode(state, &text) {
        tracing::info!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            logical_line_count = workspace_paste_logical_line_count(&text),
            character_count = text.chars().count(),
            prompt_mode = ?prompt_mode,
            "workspace paste requires confirmation before sending to the terminal"
        );
        *pending_warning.borrow_mut() = Some(PendingWorkspacePasteWarning {
            session_id,
            logical_line_count: workspace_paste_logical_line_count(&text),
            text,
            prompt_mode,
        });
        return WorkspacePasteRequestOutcome::Prompted;
    }

    pending_warning.borrow_mut().take();
    forward_workspace_session_paste(state, bridge, image_paste_controller, session_id, &text);
    WorkspacePasteRequestOutcome::Sent
}

fn schedule_clipboard_image_preparation(
    runtime_handle: tokio::runtime::Handle,
    request_id: Uuid,
    image: ClipboardImageSource,
    result_tx: std::sync::mpsc::Sender<ClipboardImagePasteBackgroundMessage>,
    preparation_gate: Arc<tokio::sync::Semaphore>,
) {
    runtime_handle.spawn(async move {
        let permit = match preparation_gate.acquire_owned().await {
            Ok(permit) => permit,
            Err(error) => {
                let _ = result_tx.send(ClipboardImagePasteBackgroundMessage::Prepared {
                    request_id,
                    result: Err(format!("clipboard image preparation queue closed: {error}")),
                });
                return;
            }
        };
        let result = match tokio::task::spawn_blocking(move || encode_clipboard_image(image)).await
        {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) => Err(format!("clipboard image worker failed: {error}")),
        };
        drop(permit);
        let _ =
            result_tx.send(ClipboardImagePasteBackgroundMessage::Prepared { request_id, result });
    });
}

fn schedule_prepared_clipboard_image_upload(
    runtime_handle: tokio::runtime::Handle,
    job: ClipboardImageUploadJob<SftpRuntimeHandle>,
    result_tx: std::sync::mpsc::Sender<ClipboardImagePasteBackgroundMessage>,
) {
    runtime_handle.spawn(async move {
        let result = job
            .runtime
            .upload_clipboard_png(job.session_id, job.png_bytes)
            .await
            .map_err(|error| error.to_string());
        tracing::debug!(
            target: "app.sftp",
            request_id = job.request_id.to_string(),
            session_id = job.session_id.to_string(),
            binding_id = job.binding_id.to_string(),
            width = job.width,
            height = job.height,
            encoded_bytes = job.encoded_bytes,
            "finished clipboard image upload task"
        );
        let _ = result_tx.send(ClipboardImagePasteBackgroundMessage::Uploaded {
            request_id: job.request_id,
            result,
        });
    });
}

pub(super) fn drain_clipboard_image_paste_messages(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    controller: &mut WorkspaceClipboardImagePasteController,
    result_rx: &std::sync::mpsc::Receiver<ClipboardImagePasteBackgroundMessage>,
    result_tx: &std::sync::mpsc::Sender<ClipboardImagePasteBackgroundMessage>,
) -> bool {
    let revision_before = controller.revision();
    while let Ok(message) = result_rx.try_recv() {
        match message {
            ClipboardImagePasteBackgroundMessage::Prepared { request_id, result } => match result {
                Ok(encoded) => {
                    controller.mark_prepared(request_id, encoded);
                }
                Err(error) => {
                    if controller.mark_preparation_failed(request_id, error.clone()) {
                        tracing::error!(
                            target: "app.sftp",
                            request_id = request_id.to_string(),
                            error,
                            "clipboard image preparation failed"
                        );
                        show_clipboard_image_upload_error(state, error);
                    }
                }
            },
            ClipboardImagePasteBackgroundMessage::Uploaded { request_id, result } => match result {
                Ok(remote_path) => {
                    let Some(binding_context) =
                        controller.active_upload_binding_context(request_id)
                    else {
                        continue;
                    };
                    if !clipboard_image_binding_is_current(manager, binding_context) {
                        let error = "the originating terminal connection changed or closed before the upload completed".to_string();
                        if controller.mark_connection_invalid(request_id, error.clone()) {
                            show_clipboard_image_upload_error(state, error);
                        }
                        continue;
                    }
                    match controller.mark_upload_succeeded(request_id, remote_path) {
                        ClipboardImageCompletion::AutoInsert(action) => {
                            controller.note_terminal_input(action.session_id);
                            let quoted_path = posix_shell_quote(action.remote_path.as_str());
                            match manager.send_session_paste_if_sftp_binding_current(
                                action.session_id,
                                action.binding_id,
                                quoted_path,
                            ) {
                                Ok(true) => {
                                    controller.mark_inserted(action.request_id, Instant::now());
                                    snap_workspace_session_viewport_to_bottom_if_needed(
                                        state,
                                        manager,
                                        action.session_id,
                                    );
                                    tracing::info!(
                                        target: "app.sftp",
                                        request_id = action.request_id.to_string(),
                                        session_id = action.session_id.to_string(),
                                        binding_id = action.binding_id.to_string(),
                                        "uploaded a clipboard image and safely pasted its remote path"
                                    );
                                }
                                Ok(false) => {
                                    let error = "the originating terminal connection changed or closed before the upload completed".to_string();
                                    controller
                                        .mark_connection_invalid(action.request_id, error.clone());
                                    show_clipboard_image_upload_error(state, error);
                                }
                                Err(error) => {
                                    tracing::error!(
                                        target: "app.ssh",
                                        request_id = action.request_id.to_string(),
                                        session_id = action.session_id.to_string(),
                                        binding_id = action.binding_id.to_string(),
                                        error = %error,
                                        "failed to paste the uploaded clipboard image path"
                                    );
                                    controller.mark_connection_invalid(
                                        action.request_id,
                                        error.to_string(),
                                    );
                                    show_clipboard_image_upload_error(state, error.to_string());
                                }
                            }
                        }
                        ClipboardImageCompletion::Stale => {
                            tracing::info!(
                                target: "app.sftp",
                                request_id = request_id.to_string(),
                                "uploaded a clipboard image but retained the path because terminal input changed"
                            );
                        }
                        ClipboardImageCompletion::Ignored => {}
                    }
                }
                Err(error) => {
                    if controller.mark_upload_failed(request_id, error.clone()) {
                        tracing::error!(
                            target: "app.sftp",
                            request_id = request_id.to_string(),
                            error,
                            "clipboard image upload failed"
                        );
                        show_clipboard_image_upload_error(state, error);
                    }
                }
            },
        }
    }
    for binding_context in controller.stale_binding_contexts() {
        if !clipboard_image_binding_is_current(manager, binding_context) {
            let error =
                "the originating terminal connection changed after the image upload completed"
                    .to_string();
            if controller.mark_connection_invalid(binding_context.request_id, error.clone()) {
                show_clipboard_image_upload_error(state, error);
            }
        }
    }
    controller.expire_success(Instant::now());
    if let Some(job) = controller.take_next_upload() {
        schedule_prepared_clipboard_image_upload(manager.runtime_handle(), job, result_tx.clone());
    }
    controller.revision() != revision_before
}

fn clipboard_image_binding_is_current(
    manager: &SessionManager,
    context: ClipboardImageBindingContext,
) -> bool {
    manager.session(context.session_id).is_some()
        && manager
            .sftp_binding(context.session_id)
            .is_some_and(|binding| {
                binding.binding_id() == context.binding_id && binding.runtime().is_some()
            })
}

pub(super) fn paste_stale_clipboard_image_path(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    controller: &mut WorkspaceClipboardImagePasteController,
    request_id: Uuid,
) -> bool {
    let Some(active_session_id) = active_workspace_session_uuid(state) else {
        return false;
    };
    let Some(action) = controller.stale_paste_action(request_id, active_session_id) else {
        return false;
    };
    let Some(bridge) = bridge else {
        let error = "the SSH session bridge is unavailable".to_string();
        controller.mark_connection_invalid(request_id, error.clone());
        show_clipboard_image_upload_error(state, error);
        return true;
    };

    controller.note_terminal_input(action.session_id);
    let quoted_path = posix_shell_quote(action.remote_path.as_str());
    match bridge.manager.send_session_paste_if_sftp_binding_current(
        action.session_id,
        action.binding_id,
        quoted_path,
    ) {
        Ok(true) => {
            controller.mark_inserted(request_id, Instant::now());
            snap_workspace_session_viewport_to_bottom_if_needed(
                state,
                &bridge.manager,
                action.session_id,
            );
            true
        }
        Ok(false) => {
            let error =
                "the originating terminal connection changed or closed before the path was pasted"
                    .to_string();
            controller.mark_connection_invalid(request_id, error.clone());
            show_clipboard_image_upload_error(state, error);
            true
        }
        Err(error) => {
            tracing::error!(
                target: "app.ssh",
                request_id = request_id.to_string(),
                session_id = action.session_id.to_string(),
                binding_id = action.binding_id.to_string(),
                error = %error,
                "failed to paste a retained clipboard image path"
            );
            controller.mark_connection_invalid(request_id, error.to_string());
            show_clipboard_image_upload_error(state, error.to_string());
            true
        }
    }
}

pub(super) fn copy_stale_clipboard_image_path(
    controller: &mut WorkspaceClipboardImagePasteController,
    request_id: Uuid,
) -> bool {
    let Some(remote_path) = controller.copy_path(request_id) else {
        return false;
    };
    if let Err(error) = set_system_clipboard_text(remote_path.as_str()) {
        tracing::error!(
            target: "app.clipboard",
            request_id = request_id.to_string(),
            error = %error,
            "failed to copy a retained clipboard image path"
        );
        controller.mark_copy_failed(request_id, error.to_string());
    }
    true
}

fn show_clipboard_image_upload_error(state: &mut ShellViewModel, error: String) {
    if !state.transfer_center_open() {
        state.toggle_transfer_center();
    }
    state.show_transfer_center_feedback("error", format!("Clipboard image upload failed: {error}"));
}

pub(super) fn posix_shell_quote(value: &str) -> String {
    if value.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn forward_active_workspace_scroll_ratio(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    ratio: f32,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    if let Err(err) = bridge.manager.scroll_session_to_ratio(session_id, ratio) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            ratio,
            error = %err,
            "failed to update workspace terminal scrollback ratio"
        );
    }
}

pub(super) fn forward_active_workspace_mouse_input(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    event: TerminalMouseInput,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));
    image_paste_controller.note_terminal_input(session_id);

    if let Err(err) = bridge.manager.send_session_mouse_input(session_id, event) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            row = event.row,
            col = event.col,
            error = %err,
            "failed to forward workspace terminal mouse input"
        );
    }
}

pub(super) fn forward_active_workspace_scroll(
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    image_paste_controller: &mut WorkspaceClipboardImagePasteController,
    input: WorkspaceScrollInput,
) {
    if input.delta_lines == 0 {
        return;
    }

    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    let mouse_grabbed = state
        .active_workspace_terminal_surface()
        .map(|surface| surface.mouse_grabbed)
        .unwrap_or(false);

    if mouse_grabbed {
        let button = if input.delta_lines > 0 {
            TerminalMouseButton::WheelUp
        } else {
            TerminalMouseButton::WheelDown
        };
        let event = TerminalMouseInput {
            kind: TerminalMouseEventKind::Scroll,
            button,
            row: input.row.max(0) as u32,
            col: input.col.max(0) as u32,
            shift: input.shift,
            ctrl: input.ctrl,
            alt: input.alt,
        };
        image_paste_controller.note_terminal_input(session_id);
        if let Err(err) = bridge.manager.send_session_mouse_input(session_id, event) {
            tracing::error!(
                target: "app.ssh",
                session_id = session_id.to_string(),
                delta_lines = input.delta_lines,
                row = input.row,
                col = input.col,
                error = %err,
                "failed to forward workspace terminal wheel input"
            );
        }
        return;
    }

    if let Err(err) = bridge
        .manager
        .scroll_session_viewport(session_id, input.delta_lines)
    {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            delta_lines = input.delta_lines,
            error = %err,
            "failed to update workspace terminal local scrollback"
        );
    }
}

pub(super) fn parse_terminal_mouse_kind(value: &str) -> Option<TerminalMouseEventKind> {
    match value {
        "down" => Some(TerminalMouseEventKind::Down),
        "up" => Some(TerminalMouseEventKind::Up),
        "move" => Some(TerminalMouseEventKind::Move),
        _ => None,
    }
}

pub(super) fn parse_terminal_mouse_button(value: &str) -> Option<TerminalMouseButton> {
    match value {
        "left" => Some(TerminalMouseButton::Left),
        "middle" => Some(TerminalMouseButton::Middle),
        "right" => Some(TerminalMouseButton::Right),
        "none" => Some(TerminalMouseButton::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_workspace_paste_text, posix_shell_quote};

    #[test]
    fn workspace_paste_normalizer_is_idempotent_and_strips_carriage_returns() {
        let raw = "sudo apt update && \\\r\n  sudo apt install -y curl\r\n\r\necho done\r";
        let normalized = normalize_workspace_paste_text(raw);

        assert_eq!(
            normalized,
            "sudo apt update && \\\n  sudo apt install -y curl\n\necho done\n"
        );
        assert!(!normalized.contains('\r'));
        assert_eq!(normalize_workspace_paste_text(&normalized), normalized);
    }

    #[test]
    fn uploaded_remote_paths_are_posix_shell_quoted_without_a_newline() {
        assert_eq!(
            posix_shell_quote("/home/test/image.png"),
            "'/home/test/image.png'"
        );
        assert_eq!(
            posix_shell_quote("/home/test/it's ready.png"),
            "'/home/test/it'\\''s ready.png'"
        );
        assert_eq!(posix_shell_quote(""), "''");
        assert!(!posix_shell_quote("/tmp/image.png").contains('\n'));
    }
}
