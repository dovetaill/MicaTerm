//! Bootstrap shell chrome binder module.

use super::*;

fn transfer_was_skipped(task: &crate::app::sftp::TransferTask) -> bool {
    task.state == crate::app::sftp::TransferTaskState::Cancelled
        && task
            .error_message
            .as_deref()
            .is_some_and(|message| message.starts_with("Skipped "))
}

fn transfer_state_priority(state: crate::app::sftp::TransferTaskState) -> usize {
    match state {
        crate::app::sftp::TransferTaskState::Running => 0,
        crate::app::sftp::TransferTaskState::Queued => 1,
        crate::app::sftp::TransferTaskState::VerifyingResume => 2,
        crate::app::sftp::TransferTaskState::Paused => 3,
        crate::app::sftp::TransferTaskState::Interrupted => 4,
        crate::app::sftp::TransferTaskState::Failed => 5,
        crate::app::sftp::TransferTaskState::Conflict => 6,
        crate::app::sftp::TransferTaskState::Completed => 7,
        crate::app::sftp::TransferTaskState::Cancelled => 8,
    }
}

fn transfer_status_label(task: &crate::app::sftp::TransferTask) -> &'static str {
    if transfer_was_skipped(task) {
        return "Skipped";
    }
    if matches!(
        task.state,
        crate::app::sftp::TransferTaskState::Paused
            | crate::app::sftp::TransferTaskState::Interrupted
            | crate::app::sftp::TransferTaskState::Failed
    ) && task.resume_mode == crate::app::sftp::TransferResumeMode::RestartOnly
    {
        return "Restart required";
    }

    match task.state {
        crate::app::sftp::TransferTaskState::Queued => "Queued",
        crate::app::sftp::TransferTaskState::Running => "Running",
        crate::app::sftp::TransferTaskState::Paused => "Paused",
        crate::app::sftp::TransferTaskState::VerifyingResume => "Verifying",
        crate::app::sftp::TransferTaskState::Interrupted => "Interrupted",
        crate::app::sftp::TransferTaskState::Completed => "Completed",
        crate::app::sftp::TransferTaskState::Failed => "Failed",
        crate::app::sftp::TransferTaskState::Cancelled => "Cancelled",
        crate::app::sftp::TransferTaskState::Conflict => "Conflict",
    }
}

fn transfer_status_tone(state: crate::app::sftp::TransferTaskState) -> &'static str {
    match state {
        crate::app::sftp::TransferTaskState::Queued
        | crate::app::sftp::TransferTaskState::Running
        | crate::app::sftp::TransferTaskState::Paused
        | crate::app::sftp::TransferTaskState::VerifyingResume => "busy",
        crate::app::sftp::TransferTaskState::Completed => "success",
        crate::app::sftp::TransferTaskState::Interrupted
        | crate::app::sftp::TransferTaskState::Failed
        | crate::app::sftp::TransferTaskState::Conflict => "error",
        crate::app::sftp::TransferTaskState::Cancelled => "muted",
    }
}

fn transfer_direction_label(task: &crate::app::sftp::TransferTask) -> &'static str {
    match task.direction {
        crate::app::sftp::TransferDirection::Upload => "Upload",
        crate::app::sftp::TransferDirection::Download => "Download",
        crate::app::sftp::TransferDirection::Delete => "Delete",
        crate::app::sftp::TransferDirection::Move => "Move",
    }
}

fn format_transfer_bytes(bytes: u64) -> String {
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

fn transfer_progress_label(task: &crate::app::sftp::TransferTask) -> String {
    if task.bytes_total > 0 {
        let transferred = match task.state {
            crate::app::sftp::TransferTaskState::Completed => {
                task.bytes_total.max(task.bytes_transferred)
            }
            _ => task.bytes_transferred,
        };
        return format!(
            "{} / {}",
            format_transfer_bytes(transferred),
            format_transfer_bytes(task.bytes_total)
        );
    }

    match task.state {
        crate::app::sftp::TransferTaskState::Queued => "Pending".into(),
        crate::app::sftp::TransferTaskState::Running => "Working".into(),
        crate::app::sftp::TransferTaskState::Paused => "Paused".into(),
        crate::app::sftp::TransferTaskState::VerifyingResume => "Verifying resume".into(),
        crate::app::sftp::TransferTaskState::Interrupted => "Resume available".into(),
        crate::app::sftp::TransferTaskState::Completed => "Done".into(),
        crate::app::sftp::TransferTaskState::Cancelled => {
            if transfer_was_skipped(task) {
                "Skipped".into()
            } else {
                "Cancelled".into()
            }
        }
        crate::app::sftp::TransferTaskState::Failed
        | crate::app::sftp::TransferTaskState::Conflict => "Needs attention".into(),
    }
}

fn transfer_progress_value(task: &crate::app::sftp::TransferTask) -> f32 {
    if task.bytes_total > 0 {
        let transferred = match task.state {
            crate::app::sftp::TransferTaskState::Completed => {
                task.bytes_total.max(task.bytes_transferred)
            }
            _ => task.bytes_transferred,
        };
        return (transferred as f32 / task.bytes_total as f32).clamp(0.0, 1.0);
    }

    match task.state {
        crate::app::sftp::TransferTaskState::Completed => 1.0,
        _ => 0.0,
    }
}

fn transfer_error_summary(task: &crate::app::sftp::TransferTask) -> String {
    if task.state == crate::app::sftp::TransferTaskState::Conflict {
        return match &task.action {
            crate::app::sftp::TransferTaskAction::Download { .. }
            | crate::app::sftp::TransferTaskAction::DownloadDirectory { .. } => {
                "A local file with the same name already exists locally.".into()
            }
            crate::app::sftp::TransferTaskAction::Upload { .. }
            | crate::app::sftp::TransferTaskAction::UploadDirectory { .. }
            | crate::app::sftp::TransferTaskAction::Move => {
                "An item with the same name already exists at the destination.".into()
            }
            crate::app::sftp::TransferTaskAction::Delete { .. } => {
                "The target item already exists and needs a decision.".into()
            }
        };
    }

    task.error_message.clone().unwrap_or_default()
}

fn transfer_show_error(task: &crate::app::sftp::TransferTask) -> bool {
    task.state.needs_attention() && !transfer_error_summary(task).trim().is_empty()
}

fn transfer_attention_projection(
    state: &ShellViewModel,
    task: &crate::app::sftp::TransferTask,
) -> Option<(&'static str, &'static str)> {
    match task.state {
        crate::app::sftp::TransferTaskState::Running => Some(("pause", "Pause")),
        crate::app::sftp::TransferTaskState::Paused => {
            if task.resume_mode == crate::app::sftp::TransferResumeMode::RestartOnly {
                Some(("restart", "Restart"))
            } else {
                Some(("resume", "Resume"))
            }
        }
        crate::app::sftp::TransferTaskState::Interrupted
        | crate::app::sftp::TransferTaskState::Failed => state
            .transfer_task_retry_label(task.id.as_str())
            .map(|label| {
                (
                    if label == "Restart" {
                        "restart"
                    } else {
                        "resume"
                    },
                    label,
                )
            }),
        _ => None,
    }
}

fn transfer_can_open_file(task: &crate::app::sftp::TransferTask) -> bool {
    task.state == crate::app::sftp::TransferTaskState::Completed
        && match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path } => {
                crate::app::sftp::can_open_file_path_locally(local_path.as_path())
            }
            _ => false,
        }
}

fn transfer_can_open_folder(task: &crate::app::sftp::TransferTask) -> bool {
    task.state == crate::app::sftp::TransferTaskState::Completed
        && match &task.action {
            crate::app::sftp::TransferTaskAction::Download { local_path }
            | crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path } => {
                crate::app::sftp::can_open_folder_path_locally(local_path.as_path())
            }
            _ => false,
        }
}

fn transfer_can_remove(task: &crate::app::sftp::TransferTask) -> bool {
    !task.state.is_active()
}

fn transfer_task_title(task: &crate::app::sftp::TransferTask) -> String {
    let path = match task.direction {
        crate::app::sftp::TransferDirection::Upload | crate::app::sftp::TransferDirection::Move => {
            task.target_path.as_str()
        }
        crate::app::sftp::TransferDirection::Download
        | crate::app::sftp::TransferDirection::Delete => task.source_path.as_str(),
    };
    path.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .unwrap_or(path)
        .to_string()
}

fn transfer_task_detail(task: &crate::app::sftp::TransferTask) -> String {
    let direction = transfer_direction_label(task);
    let location = match &task.action {
        crate::app::sftp::TransferTaskAction::Upload { .. }
        | crate::app::sftp::TransferTaskAction::UploadDirectory { .. }
        | crate::app::sftp::TransferTaskAction::Move => task.target_path.clone(),
        crate::app::sftp::TransferTaskAction::Download { local_path }
        | crate::app::sftp::TransferTaskAction::DownloadDirectory { local_path } => {
            local_path.to_string_lossy().to_string()
        }
        crate::app::sftp::TransferTaskAction::Delete { .. } => task.source_path.clone(),
    };

    if location.trim().is_empty() {
        return direction.to_string();
    }

    match task.direction {
        crate::app::sftp::TransferDirection::Delete => format!("{direction} {location}"),
        _ => format!("{direction} to {location}"),
    }
}

fn project_transfer_center_items(state: &ShellViewModel) -> Vec<TransferCenterItem> {
    let mut indexed = state
        .sftp_transfer_tasks()
        .iter()
        .enumerate()
        .filter(|(_, task)| state.transfer_center_includes_task(task))
        .map(|(index, task)| (index, task))
        .collect::<Vec<_>>();
    indexed.sort_by_key(|(index, task)| {
        (
            transfer_state_priority(task.state),
            usize::MAX.saturating_sub(*index),
        )
    });

    indexed
        .into_iter()
        .map(|(_, task)| {
            let session_ready = state.has_connected_terminal_session(task.session_id.as_str());
            let (attention_action, attention_label) =
                transfer_attention_projection(state, task).unwrap_or(("", ""));
            TransferCenterItem {
                id: task.id.clone().into(),
                title: transfer_task_title(task).into(),
                host_label: state
                    .transfer_task_host_label(task.session_id.as_str())
                    .into(),
                direction_label: transfer_direction_label(task).into(),
                detail: transfer_task_detail(task).into(),
                status_label: transfer_status_label(task).into(),
                status_tone: transfer_status_tone(task.state).into(),
                progress_value: transfer_progress_value(task),
                progress_label: transfer_progress_label(task).into(),
                error_summary: transfer_error_summary(task).into(),
                error_tooltip: transfer_error_summary(task).into(),
                show_error: transfer_show_error(task),
                can_show_error: transfer_show_error(task),
                attention_action: attention_action.into(),
                attention_label: attention_label.into(),
                can_retry: state.transfer_task_can_retry(task.id.as_str()),
                can_resolve_conflict: session_ready
                    && task.state == crate::app::sftp::TransferTaskState::Conflict,
                can_open_workspace: session_ready
                    && task.state == crate::app::sftp::TransferTaskState::Conflict,
                can_open_file: transfer_can_open_file(task),
                can_open_folder: transfer_can_open_folder(task),
                can_remove: transfer_can_remove(task),
                remove_tooltip: state.transfer_task_remove_tooltip(task.id.as_str()).into(),
            }
        })
        .collect()
}

fn native_window_appearance_request_for_workspace(
    theme_mode: ThemeMode,
) -> crate::app::window_effects::NativeWindowAppearanceRequest {
    let mut request = build_native_window_appearance_request(theme_mode, window_appearance());
    let profile = super::WORKSPACE_RUNTIME_PROFILE
        .with(|profile| (*profile.borrow()).unwrap_or_else(AppRuntimeProfile::packaged));

    if profile.prefers_native_terminal_renderer()
        && matches!(
            profile.build_flavor,
            AppBuildFlavor::WindowsMainline | AppBuildFlavor::WindowsSoftwareCompat
        )
    {
        request.backdrop = crate::app::window_effects::BackdropPreference::None;
    }

    request
}

pub(super) fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    window.window().request_redraw();

    let request = native_window_appearance_request_for_workspace(state.theme_mode);
    let report = effects.apply_to_app_window(window, &request);

    if matches!(
        report.backdrop_status,
        crate::app::window_effects::BackdropApplyStatus::Failed
    ) {
        tracing::error!(
            target: "app.window",
            theme = ?request.theme,
            backdrop = ?request.backdrop,
            backdrop_error = %report.backdrop_error.as_deref().unwrap_or("unknown"),
            "failed to apply native window appearance"
        );
    }
}

pub(super) fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_transfer_center_open(state.transfer_center_open());
    window.set_transfer_center_pinned(state.transfer_center_pinned());
    window.set_transfer_center_collapsed(state.transfer_center_collapsed());
    window.set_transfer_queue_total(
        i32::try_from(state.sftp_queue_summary.total_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_active(
        i32::try_from(state.sftp_queue_summary.active_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_queued(
        i32::try_from(state.sftp_queue_summary.queued_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_running(
        i32::try_from(state.sftp_queue_summary.running_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_paused(
        i32::try_from(state.sftp_queue_summary.paused_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_failed(
        i32::try_from(state.sftp_queue_summary.failed_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_completed(
        i32::try_from(state.sftp_queue_summary.completed_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_queue_current_session(
        i32::try_from(state.sftp_queue_summary.current_session_count).unwrap_or(i32::MAX),
    );
    window.set_transfer_center_active_filter(state.transfer_center_filter_id().into());
    let transfer_items = project_transfer_center_items(state);
    super::sync_vec_model(
        window.get_transfer_center_items(),
        transfer_items,
        |model| window.set_transfer_center_items(model),
    );
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized());
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
    window.set_settings_modal_open(state.settings_modal_open());
    window.set_settings_modal_terminal_scrollback_limit(
        i32::try_from(state.settings_modal_terminal_scrollback_limit()).unwrap_or(i32::MAX),
    );
    window.set_settings_modal_terminal_active_idle_shrink_enabled(
        state.settings_modal_terminal_active_idle_shrink_enabled(),
    );
    window.set_settings_modal_download_conflict_default(
        state.settings_modal_download_conflict_default_id().into(),
    );
    window.set_sync_feedback_text(state.sync_feedback_state().text.clone().into());
    window.set_sync_feedback_sequence(state.sync_feedback_state().sequence);
    window.set_sync_feedback_running(state.sync_feedback_state().running);
    window.set_transfer_center_feedback_text(
        state.transfer_center_feedback_state().text.clone().into(),
    );
    window.set_transfer_center_feedback_tone(
        state.transfer_center_feedback_state().tone.clone().into(),
    );
    window.set_transfer_center_feedback_sequence(state.transfer_center_feedback_state().sequence);
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn bind_shell_chrome_callbacks(
    window: &AppWindow,
    view_model: &Rc<RefCell<ShellViewModel>>,
    store: &Option<Rc<UiPreferencesStore>>,
    effects: &Rc<dyn PlatformWindowEffects>,
    session_bridge: &Option<Rc<ShellSessionBridge>>,
    workspace_follow_tracker: &Rc<RefCell<WorkspaceFollowTracker>>,
    controller: &Rc<WindowController<AppWindow>>,
) {
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    window.on_open_settings_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_settings_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sftp::sync_right_panel_state(&window, &mut state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let effects_ref = Rc::clone(effects);
    window.on_settings_modal_close_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_settings_modal();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    window.on_settings_modal_terminal_scrollback_limit_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_settings_modal_terminal_scrollback_limit(value);
        if let Some(session_bridge) = session_bridge_ref.as_deref() {
            session_bridge
                .terminal_defaults
                .set_scrollback_lines(state.settings_modal_terminal_scrollback_limit());
        }
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    window.on_settings_modal_terminal_active_idle_shrink_enabled_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_settings_modal_terminal_active_idle_shrink_enabled(value);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    window.on_settings_modal_download_conflict_default_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_settings_modal_download_conflict_default(value.as_str());
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    window.on_open_appearance_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_appearance_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sftp::sync_right_panel_state(&window, &mut state);
        sync_shell_layout(&window, &mut state, width, height);
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_close_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(effects);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_theme_mode();
        if let Some(session_bridge) = session_bridge_ref.as_deref() {
            if let Err(err) = session_bridge.manager.set_theme_mode(state.theme_mode) {
                tracing::error!(
                    target: "app.ssh",
                    error = %err,
                    theme_mode = ?state.theme_mode,
                    "failed to synchronize theme mode into SSH sessions"
                );
            }
            workspace_terminal::refresh_active_terminal_surface_only(
                &window,
                &mut state,
                Some(session_bridge),
                &mut workspace_follow_tracker_ref.borrow_mut(),
            );
            if state.active_workspace_terminal_surface().is_none() {
                sync_workspace_session_state_with_manager(
                    &window,
                    &state,
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                    Some(&session_bridge.manager),
                );
            }
        }
        sync_theme_and_window_effects(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    window.on_toggle_window_always_on_top_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_always_on_top();
        window.set_is_window_always_on_top(state.is_always_on_top);
        save_ui_preferences(&store_ref, &state);
    });

    let controller_ref = Rc::clone(controller);
    window.on_minimize_requested(move || {
        controller_ref.minimize();
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(controller);
    let effects_ref = Rc::clone(effects);
    window.on_maximize_toggle_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_sidebar_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_assets_sidebar();
        assets_keychain::sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_sidebar_destination_selected(move |destination_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        let destination = SidebarDestination::from_id(destination_id.as_str())
            .unwrap_or(SidebarDestination::Console);
        state.select_sidebar_destination(destination);
        assets_keychain::sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });
}
