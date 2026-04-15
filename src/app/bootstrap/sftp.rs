//! Bootstrap SFTP binder module.

use super::*;

const SFTP_PARENT_ITEM_ID: &str = "__sftp_parent__";

fn sftp_parent_panel_item() -> SftpPanelItem {
    SftpPanelItem {
        id: SFTP_PARENT_ITEM_ID.into(),
        name: "..".into(),
        type_label: "Up".into(),
        modified_label: String::new().into(),
        size_label: String::new().into(),
        kind: "parent-directory".into(),
        selected: false,
    }
}

pub(super) fn sync_sftp_panel_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_sftp_panel_mode(state.sftp_panel_mode_id().into());
    window.set_sftp_panel_host_label(state.sftp_panel_host_label().into());
    window.set_sftp_panel_path(state.sftp_panel_path().into());
    window.set_sftp_panel_follow_mode(state.sftp_panel_follow_mode_id().into());
    window.set_sftp_panel_connection_badge(state.quick_browser_connection_badge().into());
    window.set_sftp_panel_binding_mode_label(state.quick_browser_binding_mode_label().into());
    window.set_sftp_panel_path_editing(state.quick_browser_path_editing());
    window.set_sftp_panel_can_go_back(state.sftp_panel_can_go_back());
    window.set_sftp_panel_can_go_forward(state.sftp_panel_can_go_forward());
    window.set_sftp_panel_can_go_up(state.sftp_panel_can_go_up());
    window.set_sftp_panel_actions_enabled(state.sftp_panel_actions_enabled());
    window.set_sftp_panel_sort_column(state.sftp_panel_sort_column_id().into());
    window.set_sftp_panel_sort_direction(state.sftp_panel_sort_direction_id().into());
    window.set_sftp_panel_name_column_width(state.sftp_panel_name_column_width_px());
    window.set_sftp_panel_type_column_width(state.sftp_panel_type_column_width_px());
    window.set_sftp_panel_modified_column_width(state.sftp_panel_modified_column_width_px());
    window.set_sftp_panel_size_column_width(state.sftp_panel_size_column_width_px());
    window.set_sftp_queue_drawer_open(state.sftp_queue_drawer_open());

    let mut items = Vec::new();
    if state.sftp_panel_can_go_up() {
        items.push(sftp_parent_panel_item());
    }
    items.extend(
        state
            .project_sftp_panel_entries(state.sftp_panel_entries())
            .iter()
            .map(|entry| SftpPanelItem {
                id: entry.id.as_str().into(),
                name: entry.name.as_str().into(),
                type_label: sftp_panel_entry_type_label(entry.kind).into(),
                modified_label: sftp_panel_entry_modified_label(entry).into(),
                size_label: sftp_panel_entry_size_label(entry).into(),
                kind: sftp_panel_entry_kind(entry.kind).into(),
                selected: state
                    .sftp_panel_selected_entry_ids()
                    .iter()
                    .any(|selected_id| selected_id == &entry.id),
            }),
    );
    sync_vec_model(window.get_sftp_panel_items(), items, |model| {
        window.set_sftp_panel_items(model)
    });

    let selected_ids = state
        .sftp_panel_selected_entry_ids()
        .iter()
        .map(|entry_id| SharedString::from(entry_id.as_str()))
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_sftp_panel_selected_entry_ids(),
        selected_ids,
        |model| window.set_sftp_panel_selected_entry_ids(model),
    );

    let queue = &state.sftp_queue_summary;
    window.set_sftp_panel_queue_active(i32::try_from(queue.active_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_failed(i32::try_from(queue.failed_count).unwrap_or(i32::MAX));
    window.set_sftp_panel_queue_current_session(
        i32::try_from(queue.current_session_count).unwrap_or(i32::MAX),
    );
}

pub(super) fn sync_right_panel_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_right_panel_view(state.right_panel_view_id().into());
    sync_sftp_panel_state(window, state);
}

pub(super) fn sync_sftp_remote_file_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let editor = state.sftp_remote_file_editor_state();
    window.set_sftp_remote_file_modal_open(editor.open);
    window.set_sftp_remote_file_modal_title(editor.title.clone().into());
    window.set_sftp_remote_file_modal_path(editor.remote_path.clone().into());
    window.set_sftp_remote_file_modal_content(editor.content.clone().into());
    window.set_sftp_remote_file_modal_status_text(editor.status_text.clone().into());
    window.set_sftp_remote_file_modal_error_text(editor.error_text.clone().into());
    window.set_sftp_remote_file_modal_can_save(state.sftp_remote_file_editor_can_save());
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn sftp_panel_entry_type_label(kind: SftpDirectoryEntryKind) -> &'static str {
    match kind {
        SftpDirectoryEntryKind::Directory => "Folder",
        SftpDirectoryEntryKind::Symlink => "Link",
        SftpDirectoryEntryKind::Unknown => "Unknown",
        SftpDirectoryEntryKind::File => "File",
    }
}

pub(super) fn sftp_panel_entry_modified_label(
    entry: &crate::app::sftp::SftpDirectoryEntry,
) -> String {
    let Some(unix_seconds) = entry.modified_unix_seconds else {
        return String::new();
    };
    let Some(timestamp) = DateTime::<Utc>::from_timestamp(unix_seconds as i64, 0) else {
        return String::new();
    };
    timestamp.format("%Y-%m-%d %H:%M").to_string()
}

pub(super) fn sftp_panel_entry_size_label(entry: &crate::app::sftp::SftpDirectoryEntry) -> String {
    entry.size_bytes.map(format_binary_size).unwrap_or_default()
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

pub(super) fn sftp_panel_entry_kind(kind: SftpDirectoryEntryKind) -> &'static str {
    match kind {
        SftpDirectoryEntryKind::Directory => "directory",
        SftpDirectoryEntryKind::File => "file",
        SftpDirectoryEntryKind::Symlink => "symlink",
        SftpDirectoryEntryKind::Unknown => "unknown",
    }
}

pub(super) fn sync_active_sftp_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> bool {
    let Some(active_session_id_text) = state
        .active_workspace_terminal_session_id()
        .map(str::to_string)
    else {
        return false;
    };
    if !state.quick_browser_follows_active_terminal() && state.quick_browser_session().is_some() {
        return false;
    }

    let Some(session_id) = Uuid::parse_str(&active_session_id_text).ok() else {
        return false;
    };

    let binding = manager.sftp_binding(session_id);
    let cwd = manager.current_working_directory(session_id);
    let Some(binding) = binding else {
        return false;
    };

    state.quick_browser_session_id = Some(active_session_id_text.clone());
    let host_profile_ref = state
        .active_workspace_tab()
        .map(|tab| {
            crate::app::sftp::HostProfileRef::with_label(tab.asset_id.clone(), tab.title.clone())
        })
        .unwrap_or_else(|| crate::app::sftp::HostProfileRef::new("active-session"));
    let initial_path = cwd.clone().unwrap_or_else(|| "/".to_string());
    let session_state = state
        .file_browser_sessions
        .entry(active_session_id_text.clone())
        .or_insert_with(|| {
            let mut session = crate::app::sftp::FileBrowserSession::quick_browser(
                host_profile_ref.clone(),
                initial_path,
            );
            session.file_browser_session_id = active_session_id_text.clone();
            session.attach_terminal_session_id(active_session_id_text.clone());
            session
        });
    session_state.attach_terminal_session_id(active_session_id_text);
    let before = session_state.clone();

    match binding.mode() {
        SftpPanelMode::Disconnected => session_state.mark_disconnected(),
        _ if matches!(
            session_state.mode,
            SftpPanelMode::Empty | SftpPanelMode::Disconnected
        ) =>
        {
            session_state.mark_connecting()
        }
        _ => {}
    }

    if let Some(cwd) = cwd {
        if session_state.current_path.is_empty() {
            session_state.reenable_follow(cwd);
        } else if session_state.follow_mode == SftpFollowMode::FollowCwd {
            session_state.follow_terminal_path(cwd);
        }
    }

    before != *session_state
}

pub(super) fn project_sftp_browser_state_into_view_model(
    state: &mut ShellViewModel,
    browser_session_id: &str,
    browser_state: &SftpBrowserSessionState,
) -> bool {
    let mut next = state
        .file_browser_sessions
        .get(browser_session_id)
        .cloned()
        .unwrap_or_else(|| {
            let mut session = crate::app::sftp::FileBrowserSession::quick_browser(
                crate::app::sftp::HostProfileRef::new("active-session"),
                browser_state.current_path.clone(),
            );
            session.file_browser_session_id = browser_session_id.to_string();
            session
        });
    next.mode = browser_state.mode;
    next.follow_mode = browser_state.follow_mode;
    next.current_path = browser_state.current_path.clone();
    next.history = browser_state.history.clone();
    next.entries = browser_state.entries.clone();
    next.selected_entry_ids = browser_state.selected_entry_ids.clone();
    next.last_error = browser_state.last_error.clone();
    next.active_request_id = browser_state.active_request_id;
    if state.file_browser_sessions.get(browser_session_id) == Some(&next) {
        return false;
    }
    state.set_file_browser_session(next);
    true
}

pub(super) fn execute_sftp_browser_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
    request: SftpBrowserLoadRequest,
) -> bool {
    match manager.sftp_read_dir(request.session_id, request.path.as_str()) {
        Ok(entries) => controller.apply_loaded_directory_for_browser_session(
            request.file_browser_session_id.as_str(),
            request.request_id,
            request.path.as_str(),
            entries,
        ),
        Err(err) => {
            if manager
                .sftp_binding(request.session_id)
                .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
            {
                controller.mark_disconnected_browser_session(request.file_browser_session_id.as_str());
            } else {
                controller.apply_load_error_for_browser_session(
                    request.file_browser_session_id.as_str(),
                    request.request_id,
                    request.path.as_str(),
                    err.to_string(),
                );
            }
        }
    }

    controller
        .browser_session_state(request.file_browser_session_id.as_str())
        .is_some_and(|browser_state| {
            project_sftp_browser_state_into_view_model(
                state,
                request.file_browser_session_id.as_str(),
                browser_state,
            )
        })
}

pub(super) fn sftp_remote_file_title(remote_path: &str) -> String {
    remote_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Remote File")
        .to_string()
}

pub(super) fn open_sftp_remote_file_editor_for_entry(
    state: &mut ShellViewModel,
    manager: &SessionManager,
    session_id: Uuid,
    remote_path: &str,
) {
    match manager.sftp_download_file(session_id, remote_path) {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(text) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                text,
                "Editing remote text file".to_string(),
                String::new(),
            ),
            Err(err) => state.open_sftp_remote_file_editor(
                session_id.to_string(),
                remote_path.to_string(),
                sftp_remote_file_title(remote_path),
                String::from_utf8_lossy(err.as_bytes()).into_owned(),
                "View only".to_string(),
                "Only UTF-8 text files can be edited online right now.".to_string(),
            ),
        },
        Err(err) => state.open_sftp_remote_file_editor(
            session_id.to_string(),
            remote_path.to_string(),
            sftp_remote_file_title(remote_path),
            String::new(),
            "Open failed".to_string(),
            format!("Failed to open remote file: {err}"),
        ),
    }
}

pub(super) fn initial_sftp_browser_path(
    manager: &SessionManager,
    session_id: Uuid,
) -> Option<String> {
    if let Some(cwd) = manager.current_working_directory(session_id) {
        return Some(cwd);
    }

    manager
        .sftp_binding(session_id)
        .filter(|binding| binding.mode() != SftpPanelMode::Disconnected)
        .map(|_| "/".to_string())
}

fn quick_browser_terminal_session_uuid(state: &ShellViewModel) -> Option<Uuid> {
    state
        .quick_browser_linked_terminal_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
}

pub(super) fn ensure_active_sftp_browser_started(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_some() {
        return false;
    }

    initial_sftp_browser_path(manager, session_id).is_some_and(|path| {
        let request = controller.open(session_id, path.as_str());
        execute_sftp_browser_request(state, controller, manager, request)
    })
}

pub(super) fn open_active_sftp_browser_for_current_session(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    if !state.quick_browser_follows_active_terminal() && state.quick_browser_session().is_some() {
        return false;
    }

    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    if controller.session_state(session_id).is_none() {
        return ensure_active_sftp_browser_started(state, controller, manager);
    }

    let request = if controller.session_state(session_id).is_some() {
        controller.session_activated(session_id)
    } else {
        None
    };
    request.is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

pub(super) fn sync_active_sftp_browser_follow_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };

    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        controller.mark_disconnected(session_id);
        let browser_session_id = session_id.to_string();
        return controller
            .session_state(session_id)
            .is_some_and(|browser_state| {
                project_sftp_browser_state_into_view_model(
                    state,
                    browser_session_id.as_str(),
                    browser_state,
                )
            });
    }

    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.follow_mode != SftpFollowMode::FollowCwd {
        return false;
    }

    let Some(cwd) = manager.current_working_directory(session_id) else {
        return false;
    };
    if browser_state.current_path == cwd {
        return false;
    }

    controller
        .follow_cwd(session_id, cwd.as_str())
        .is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

pub(super) fn sync_active_sftp_browser_pending_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(session_id) = quick_browser_terminal_session_uuid(state) else {
        return false;
    };
    let Some(browser_state) = controller.session_state(session_id) else {
        return false;
    };
    if browser_state.mode != SftpPanelMode::Connecting {
        return false;
    }
    if manager
        .sftp_binding(session_id)
        .is_some_and(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    controller
        .pending_request(session_id)
        .is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

pub(super) fn ensure_active_workspace_sftp_browser_started(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
        return false;
    };
    let needs_restart = matches!(
        browser_session.mode,
        SftpPanelMode::Connecting | SftpPanelMode::Disconnected
    );
    if controller
        .browser_session_state(browser_session.file_browser_session_id.as_str())
        .is_some_and(|browser_state| {
            browser_state.mode != SftpPanelMode::Disconnected && !needs_restart
        })
    {
        return false;
    }
    let Some(session_id) = browser_session
        .linked_terminal_session_id
        .as_deref()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
    else {
        return false;
    };
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    let request = controller.open_file_browser_session(browser_session);
    execute_sftp_browser_request(state, controller, manager, request)
}

pub(super) fn sync_active_workspace_sftp_browser_pending_request(
    state: &mut ShellViewModel,
    controller: &mut SftpBrowserController,
    manager: &SessionManager,
) -> bool {
    let Some(browser_session) = state.active_workspace_sftp_session().cloned() else {
        return false;
    };
    let Some(session_id) = browser_session
        .linked_terminal_session_id
        .as_deref()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
    else {
        return false;
    };
    let Some(browser_state) = controller.browser_session_state(browser_session.file_browser_session_id.as_str()) else {
        return false;
    };
    if browser_state.mode != SftpPanelMode::Connecting {
        return false;
    }
    if manager
        .sftp_binding(session_id)
        .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected)
    {
        return false;
    }

    controller
        .pending_request_for_browser_session(
            browser_session.file_browser_session_id.as_str(),
            session_id,
        )
        .is_some_and(|request| execute_sftp_browser_request(state, controller, manager, request))
}

pub(super) fn bind_sftp_callbacks(
    window: &AppWindow,
    view_model: &Rc<RefCell<ShellViewModel>>,
    store: &Option<Rc<UiPreferencesStore>>,
    effects: &Rc<dyn PlatformWindowEffects>,
    session_bridge: &Option<Rc<ShellSessionBridge>>,
    workspace_follow_tracker: &Rc<RefCell<WorkspaceFollowTracker>>,
    sftp_browser_controller: &Rc<RefCell<SftpBrowserController>>,
) {
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_open_sftp_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_sftp_panel();
        if let Some(session_bridge) = session_bridge_ref.as_ref() {
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            let _ = open_active_sftp_browser_for_current_session(
                &mut state,
                &mut controller,
                &session_bridge.manager,
            );
        }
        super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sync_right_panel_state(&window, &state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_sftp_panel_expand_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.expand_quick_browser_to_workspace().is_none() {
            return;
        }
        super::sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_right_panel_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_binding_mode_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.toggle_quick_browser_binding_mode() {
            return;
        }

        if state.quick_browser_follows_active_terminal()
            && let Some(session_bridge) = session_bridge_ref.as_ref()
        {
            let _ = sync_active_sftp_projection_from_manager(&mut state, &session_bridge.manager);
            let mut controller = sftp_browser_controller_ref.borrow_mut();
            let _ = open_active_sftp_browser_for_current_session(
                &mut state,
                &mut controller,
                &session_bridge.manager,
            );
        }

        sync_right_panel_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_path_edit_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.begin_quick_browser_path_edit() {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_context_menu_requested(
        move |target_id, target_kind, anchor_x, anchor_y| {
            let window = handle.unwrap();
            let mut state = state.borrow_mut();
            state.open_context_menu_for_target(
                parse_context_target_kind(target_kind.as_str(), SidebarDestination::Console),
                if target_id.is_empty() {
                    None
                } else {
                    Some(target_id.to_string())
                },
                anchor_x,
                anchor_y,
            );
            super::assets_keychain::update_context_menu_placement(&window, &mut state);
            super::assets_keychain::sync_assets_context_menu_state(&window, &state);
        },
    );

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_item_selected(move |entry_id| {
        if entry_id.as_str() == SFTP_PARENT_ITEM_ID {
            return;
        }
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.select_sftp_panel_entry(entry_id.as_str()) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_item_activated(move |entry_id, item_kind| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let is_parent_row = entry_id.as_str() == SFTP_PARENT_ITEM_ID
            || item_kind.as_str() == "parent-directory";
        let selection_changed = if is_parent_row {
            false
        } else {
            state.select_sftp_panel_entry(entry_id.as_str())
        };
        let entry = state.active_sftp_entry(entry_id.as_str()).cloned();
        let mut panel_changed = selection_changed;
        let was_modal_open = state.sftp_remote_file_editor_state().open;

        if is_parent_row {
            if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
            {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    panel_changed |= execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    );
                }
            } else {
                panel_changed |= state.navigate_sftp_panel_up();
            }
        } else if let Some(entry) = entry {
            if item_kind.as_str() == "directory" || entry.kind == SftpDirectoryEntryKind::Directory
            {
                if let Some(session_bridge) = session_bridge_ref.as_ref()
                    && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
                {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, entry.path.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    panel_changed |= execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    );
                }
            } else if let Some(session_bridge) = session_bridge_ref.as_ref()
                && let Some(session_id) = quick_browser_terminal_session_uuid(&state)
            {
                open_sftp_remote_file_editor_for_entry(
                    &mut state,
                    &session_bridge.manager,
                    session_id,
                    entry.path.as_str(),
                );
            }
        }

        if panel_changed {
            sync_right_panel_state(&window, &state);
        }
        sync_sftp_remote_file_modal_state(&window, &state);
        if !was_modal_open && state.sftp_remote_file_editor_state().open {
            super::assets_keychain::schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_open_queue_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_sftp_queue_drawer();
        sync_right_panel_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_path_submitted(move |path| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let trimmed = path.trim();
                if trimmed.is_empty() {
                    false
                } else {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.navigate(session_id, trimmed)
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                }
            } else {
                false
            }
        } else {
            state.submit_sftp_panel_path(path.to_string())
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_back_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_back(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_back()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_forward_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_forward(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_forward()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_up_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.navigate_up(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.navigate_sftp_panel_up()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_refresh_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                let request = {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    controller.refresh(session_id)
                };
                if let Some(request) = request {
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.refresh_sftp_panel()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_retry_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let retried = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                if let Err(err) = session_bridge.manager.retry_session(session_id) {
                    tracing::error!(
                        target: "app.ssh",
                        session_id = session_id.to_string(),
                        error = %err,
                        "failed to retry active SSH session from SFTP panel"
                    );
                    false
                } else {
                    let projection = workspace_terminal::sync_workspace_projection_from_manager(
                        &mut state,
                        &session_bridge.manager,
                    );
                    let browser_changed = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        if let Some(request) = controller.retry(session_id) {
                            if session_bridge.manager.sftp_binding(session_id).is_some_and(
                                |binding| binding.mode() != SftpPanelMode::Disconnected,
                            ) {
                                execute_sftp_browser_request(
                                    &mut state,
                                    &mut controller,
                                    &session_bridge.manager,
                                    request,
                                )
                            } else {
                                let browser_session_id = session_id.to_string();
                                controller
                                    .session_state(session_id)
                                    .is_some_and(|browser_state| {
                                        project_sftp_browser_state_into_view_model(
                                            &mut state,
                                            browser_session_id.as_str(),
                                            browser_state,
                                        )
                                    })
                            }
                        } else {
                            false
                        }
                    };
                    browser_changed
                        || projection.sftp_changed
                        || projection.tabs_changed
                        || projection.surface_changed
                }
            } else {
                false
            }
        } else {
            state.retry_sftp_panel()
        };
        if retried {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    window.on_sftp_panel_reenable_follow_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let changed = if let Some(session_bridge) = session_bridge_ref.as_ref() {
            if let Some(session_id) = quick_browser_terminal_session_uuid(&state) {
                if let Some(cwd) = session_bridge.manager.current_working_directory(session_id) {
                    let request = {
                        let mut controller = sftp_browser_controller_ref.borrow_mut();
                        controller.open(session_id, cwd.as_str())
                    };
                    let mut controller = sftp_browser_controller_ref.borrow_mut();
                    execute_sftp_browser_request(
                        &mut state,
                        &mut controller,
                        &session_bridge.manager,
                        request,
                    )
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            state.reenable_sftp_follow()
        };
        if changed {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_sort_requested(move |column_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.cycle_sftp_panel_sort(column_id.as_str()) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_panel_column_width_change_requested(move |column_id, width| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.set_sftp_panel_column_width(column_id.as_str(), width) {
            sync_right_panel_state(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_sftp_remote_file_editor();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sftp_remote_file_modal_content_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_sftp_remote_file_editor_content(value.to_string());
        sync_sftp_remote_file_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    window.on_sftp_remote_file_modal_save_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if let Some((session_id, remote_path, content)) =
            state.sftp_remote_file_editor_save_payload()
            && let Some(session_bridge) = session_bridge_ref.as_ref()
        {
            match Uuid::parse_str(session_id.as_str())
                .map_err(anyhow::Error::from)
                .and_then(|session_id| {
                    session_bridge.manager.sftp_upload_file(
                        session_id,
                        remote_path.as_str(),
                        content.into_bytes(),
                    )
                }) {
                Ok(_) => state.mark_sftp_remote_file_editor_saved(),
                Err(err) => state.set_sftp_remote_file_editor_error(format!(
                    "Failed to save remote file: {err}"
                )),
            }
        }
        sync_sftp_remote_file_modal_state(&window, &state);
    });
}
