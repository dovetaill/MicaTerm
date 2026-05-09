//! Bootstrap workspace terminal module.

use super::*;

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
    let mut next_tabs = manager
        .ordered_sessions()
        .into_iter()
        .filter(|handle| {
            !state.workspace_terminal_session_hidden(handle.session_id.to_string().as_str())
        })
        .map(|handle| WorkspaceTab::from_session(&handle))
        .collect::<Vec<_>>();
    let manager_session_ids = next_tabs
        .iter()
        .map(|tab| tab.session_id.clone())
        .collect::<HashSet<_>>();
    let manager_asset_ids = next_tabs
        .iter()
        .map(|tab| tab.asset_id.clone())
        .collect::<HashSet<_>>();
    let preserved_error_tabs = state
        .workspace_tabs()
        .iter()
        .filter(|tab| {
            tab.state == "error"
                && !manager_session_ids.contains(&tab.session_id)
                && !manager_asset_ids.contains(&tab.asset_id)
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
    let next_session_id = next_tabs.iter().find(|tab| tab.active).and_then(|tab| {
        if !tab.uses_terminal_surface()
            && !tab.uses_connection_progress_surface()
            && !tab.can_reconnect()
        {
            return None;
        }

        Uuid::parse_str(tab.session_id.as_str()).ok()
    });
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
            let binding_disconnected = linked_session_id_text
                .as_deref()
                .and_then(|session_id| Uuid::parse_str(session_id).ok())
                .and_then(|session_id| manager.sftp_binding(session_id))
                .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected);

            let Some(browser_session) = state
                .file_browser_sessions
                .get_mut(tab.file_browser_session_id.as_str())
            else {
                tab.state = "disconnected".into();
                tab.error_detail = "SFTP workspace browser session is unavailable.".into();
                return tab;
            };

            if binding_disconnected {
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
    let needs_snap = state
        .active_workspace_terminal_surface()
        .is_some_and(|surface| !surface.viewport_at_bottom);
    if !needs_snap {
        return;
    }

    if let Err(err) = bridge.manager.scroll_session_to_bottom(session_id) {
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
    if let Err(err) = bridge.manager.resize_session(session_id, rows, cols) {
        tracing::error!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            rows,
            cols,
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

    let start_row = start_row.max(0) as u32;
    let start_col = start_col.max(0) as u32;
    let end_row = end_row.max(0) as u32;
    let end_col = end_col.max(0) as u32;
    let text = active_workspace_session_uuid(state)
        .zip(bridge)
        .and_then(|(session_id, bridge)| {
            bridge
                .manager
                .selection_text_from_buffer_rows(session_id, start_row, start_col, end_row, end_col)
                .ok()
        })
        .unwrap_or_else(|| {
            surface.selection_text_from_buffer_rows(start_row, start_col, end_row, end_col)
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

pub(super) fn normalized_paste_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(super) fn workspace_paste_logical_line_count(text: &str) -> usize {
    let normalized = normalized_paste_newlines(text);
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
    let normalized = normalized_paste_newlines(text);
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
    state: &ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    pending_warning: &RefCell<Option<PendingWorkspacePasteWarning>>,
) -> WorkspacePasteRequestOutcome {
    let Some(session_id) = active_workspace_session_uuid(state) else {
        tracing::warn!(
            target: "app.ssh",
            "ignored workspace paste request because no active terminal session is selected"
        );
        return WorkspacePasteRequestOutcome::Ignored;
    };
    let Some(text) = system_clipboard_text() else {
        tracing::warn!(
            target: "app.ssh",
            session_id = session_id.to_string(),
            "ignored workspace paste request because clipboard text could not be read"
        );
        return WorkspacePasteRequestOutcome::Ignored;
    };

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
    forward_workspace_session_paste(state, bridge, session_id, &text);
    WorkspacePasteRequestOutcome::Sent
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
    event: TerminalMouseInput,
) {
    let Some(bridge) = bridge else {
        return;
    };
    let Some(session_id) = active_workspace_session_uuid(state) else {
        return;
    };

    snap_active_workspace_viewport_to_bottom_if_needed(state, Some(bridge));

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
