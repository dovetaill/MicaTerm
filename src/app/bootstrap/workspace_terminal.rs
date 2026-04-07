//! Bootstrap workspace terminal module.

use super::*;

pub(super) fn projected_active_workspace_session_id(
    state: &ShellViewModel,
    next_tabs: &[WorkspaceTab],
) -> Option<String> {
    state
        .active_workspace_session_id()
        .filter(|candidate| next_tabs.iter().any(|tab| tab.session_id == *candidate))
        .map(str::to_string)
        .or_else(|| {
            state
                .workspace_tabs()
                .iter()
                .find(|tab| {
                    tab.active
                        && next_tabs
                            .iter()
                            .any(|candidate| candidate.session_id == tab.session_id)
                })
                .map(|tab| tab.session_id.clone())
        })
        .or_else(|| next_tabs.first().map(|tab| tab.session_id.clone()))
}

pub(super) fn sync_workspace_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> WorkspaceProjectionDelta {
    let mut next_tabs = manager
        .ordered_sessions()
        .into_iter()
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
    next_tabs.extend(preserved_error_tabs);
    next_tabs.extend(preserved_launcher_tabs);
    let active_id = projected_active_workspace_session_id(state, &next_tabs);
    for tab in &mut next_tabs {
        tab.active = active_id.as_deref() == Some(tab.session_id.as_str());
    }
    let next_session_id = state
        .active_workspace_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok());
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
    state.active_workspace_terminal_surface()
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
    state.active_workspace_terminal_surface()
        .map(|surface| surface.normalize_selection_hit_col(safe_row, safe_col) as i32)
        .unwrap_or(col.max(0))
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
        Duration::from_millis(WORKSPACE_SCROLL_PROJECTION_DEBOUNCE_MS),
        move || {
            gate.borrow_mut().clear();
            let Some(window) = window_handle.upgrade() else {
                return;
            };
            let mut state = state.borrow_mut();
            // Keep refresh_active_workspace_surface_projection(...) as the legacy alias
            // for this surface-local refresh seam while callers migrate.
            refresh_active_terminal_surface_only(
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
        Duration::from_millis(WORKSPACE_SCROLL_PROJECTION_DEBOUNCE_MS),
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
            // Keep refresh_active_workspace_surface_projection(...) as the legacy alias
            // for this surface-local refresh seam while callers migrate.
            refresh_active_terminal_surface_only(
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
    start_row: i32,
    start_col: i32,
    end_row: i32,
    end_col: i32,
) {
    let Some(surface) = state.active_workspace_terminal_surface() else {
        return;
    };

    let text = surface.selection_text(
        start_row.max(0) as u32,
        start_col.max(0) as u32,
        end_row.max(0) as u32,
        end_col.max(0) as u32,
    );
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
    let logical_line_count = workspace_paste_logical_line_count(text);
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
        return WorkspacePasteRequestOutcome::Ignored;
    };
    let Some(text) = system_clipboard_text() else {
        return WorkspacePasteRequestOutcome::Ignored;
    };

    if let Some(prompt_mode) = workspace_paste_prompt_mode(state, &text) {
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
