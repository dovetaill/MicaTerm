//! Bootstrap native windowing binder module.

use super::*;
use crate::app::windowing::{ModalOffset, begin_modal_drag, update_modal_drag};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::resolve_host_window_hwnd;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ExternalDropTarget {
    None,
    Sftp,
    Terminal,
}

#[cfg(target_os = "windows")]
pub(super) fn sync_windows_true_window_placement(
    window: &AppWindow,
    state: &Rc<RefCell<ShellViewModel>>,
    effects: &dyn PlatformWindowEffects,
    winit_window: &slint::winit_030::winit::window::Window,
) {
    let Some(next) = query_true_window_placement(winit_window) else {
        return;
    };

    let mut state = state.borrow_mut();
    if state.window_placement() == next {
        return;
    }

    state.set_window_placement(next);
    super::shell_chrome::sync_top_status_bar_state(window, &state, effects);
}

pub(super) fn bind_windows_window_state_tracking(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    _effects: Rc<dyn PlatformWindowEffects>,
    _ui_preferences_store: Option<Rc<UiPreferencesStore>>,
    session_bridge: Option<Rc<ShellSessionBridge>>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    let modifiers = Rc::new(RefCell::new(NativeTerminalModifierState::default()));
    let sftp_drop_hover_active = Rc::new(RefCell::new(false));
    let terminal_drop_hover_active = Rc::new(RefCell::new(false));
    let external_drop_paths = Rc::new(RefCell::new(Vec::<PathBuf>::new()));
    let external_drop_target = Rc::new(RefCell::new(ExternalDropTarget::None));
    let last_pointer_position = Rc::new(RefCell::new(None::<(f32, f32)>));
    let external_drop_hover_timer = Rc::new(Timer::default());
    let external_drop_flush_timer = Rc::new(Timer::default());
    let external_drop_flush_pending = Rc::new(Cell::new(false));
    window
        .window()
        .on_winit_window_event(move |_slint_window, event| {
            if matches!(event, winit::event::WindowEvent::Focused(false)) {
                *modifiers.borrow_mut() = NativeTerminalModifierState::default();
            }

            if let winit::event::WindowEvent::ModifiersChanged(modifier_event) = event {
                let mut modifier_state = modifiers.borrow_mut();
                update_native_terminal_modifier_state_from_modifiers(
                    &mut modifier_state,
                    modifier_event.state(),
                );
            }

            if let winit::event::WindowEvent::KeyboardInput {
                event: key_event,
                is_synthetic,
                ..
            } = event
            {
                let mut modifier_state = modifiers.borrow_mut();
                update_native_terminal_modifier_state(&mut modifier_state, key_event);
                let modifier_snapshot = *modifier_state;
                let clipboard_shortcut = if key_event.state == winit::event::ElementState::Pressed
                    && !key_event.repeat
                    && !is_synthetic
                {
                    native_terminal_clipboard_shortcut(&key_event.logical_key, modifier_snapshot)
                } else {
                    None
                };
                let sftp_path_edit_shortcut = key_event.state
                    == winit::event::ElementState::Pressed
                    && !key_event.repeat
                    && !is_synthetic
                    && workspace_sftp_path_edit_shortcut(&key_event.logical_key, modifier_snapshot);
                let sftp_local_action_shortcut = if key_event.state
                    == winit::event::ElementState::Pressed
                    && !key_event.repeat
                    && !is_synthetic
                {
                    if workspace_sftp_select_all_shortcut(&key_event.logical_key, modifier_snapshot)
                    {
                        Some("select-all-sftp")
                    } else if workspace_sftp_clear_selection_shortcut(
                        &key_event.logical_key,
                        modifier_snapshot,
                    ) {
                        Some("clear-selection-sftp")
                    } else {
                        None
                    }
                } else {
                    None
                };
                drop(modifier_state);

                if let Some(shortcut) = clipboard_shortcut {
                    let window = handle.unwrap();
                    if window.get_workspace_session_host_mode() == "terminal"
                        && !window.get_active_workspace_session_id().is_empty()
                    {
                        match shortcut {
                            NativeTerminalClipboardShortcut::Copy => {
                                let state = state.borrow();
                                if super::workspace_terminal::active_workspace_terminal_selection_buffer_range(
                                    &state,
                                )
                                .is_some()
                                {
                                    workspace_terminal::forward_active_workspace_copy_selection(
                                        &state,
                                        session_bridge.as_deref(),
                                        -1,
                                        -1,
                                        -1,
                                        -1,
                                    );
                                    return EventResult::PreventDefault;
                                }
                            }
                            NativeTerminalClipboardShortcut::Paste => {
                                if window.get_workspace_paste_warning_modal_open() {
                                    return EventResult::PreventDefault;
                                }
                                window.invoke_workspace_session_paste_requested();
                                return EventResult::PreventDefault;
                            }
                        }
                    }
                }

                if sftp_path_edit_shortcut {
                    let window = handle.unwrap();
                    if window.get_workspace_session_host_mode() == "sftp" {
                        window.invoke_workspace_sftp_path_edit_requested();
                        return EventResult::PreventDefault;
                    }
                }

                if let Some(action_id) = sftp_local_action_shortcut {
                    let window = handle.unwrap();
                    if window.get_workspace_session_host_mode() == "sftp"
                        && !window.get_workspace_sftp_path_editing()
                    {
                        window.invoke_workspace_session_local_action_requested(action_id.into());
                        return EventResult::PreventDefault;
                    }
                }
            }

            if let winit::event::WindowEvent::CursorMoved { position, .. } = event {
                *last_pointer_position.borrow_mut() =
                    Some(logical_pointer_from_physical(&handle.unwrap(), position.x, position.y));
                if !external_drop_paths.borrow().is_empty() {
                    refresh_external_drop_target(
                        &handle.unwrap(),
                        &sftp_drop_hover_active,
                        &terminal_drop_hover_active,
                        &external_drop_target,
                        *last_pointer_position.borrow(),
                    );
                }
            }

            if let winit::event::WindowEvent::HoveredFile(path) = event {
                let window = handle.unwrap();
                let was_empty = external_drop_paths.borrow().is_empty();
                {
                    let mut pending_paths = external_drop_paths.borrow_mut();
                    if !pending_paths.iter().any(|pending| pending == path) {
                        pending_paths.push(path.clone());
                    }
                }
                tracing::info!(
                    target: "app.drop",
                    path_count = external_drop_paths.borrow().len(),
                    "external file hover entered window"
                );
                refresh_external_drop_target(
                    &window,
                    &sftp_drop_hover_active,
                    &terminal_drop_hover_active,
                    &external_drop_target,
                    *last_pointer_position.borrow(),
                );
                if was_empty {
                    let hover_timer_ref = Rc::clone(&external_drop_hover_timer);
                    let hover_timer_closure_ref = Rc::clone(&hover_timer_ref);
                    let handle_ref = handle.clone();
                    let external_drop_paths_ref = Rc::clone(&external_drop_paths);
                    let external_drop_target_ref = Rc::clone(&external_drop_target);
                    let sftp_drop_hover_active_ref = Rc::clone(&sftp_drop_hover_active);
                    let terminal_drop_hover_active_ref = Rc::clone(&terminal_drop_hover_active);
                    let last_pointer_position_ref = Rc::clone(&last_pointer_position);
                    hover_timer_ref.start(
                        TimerMode::Repeated,
                        Duration::from_millis(16),
                        move || {
                            let Some(window) = handle_ref.upgrade() else {
                                hover_timer_closure_ref.stop();
                                return;
                            };
                            if external_drop_paths_ref.borrow().is_empty() {
                                hover_timer_closure_ref.stop();
                                return;
                            }
                            refresh_external_drop_target(
                                &window,
                                &sftp_drop_hover_active_ref,
                                &terminal_drop_hover_active_ref,
                                &external_drop_target_ref,
                                *last_pointer_position_ref.borrow(),
                            );
                        },
                    );
                }
            }

            if matches!(event, winit::event::WindowEvent::HoveredFileCancelled) {
                let window = handle.unwrap();
                if external_drop_flush_pending.get() {
                    tracing::debug!(
                        target: "app.drop",
                        "ignored hover-cancel while external drop flush is pending"
                    );
                    external_drop_hover_timer.stop();
                    return EventResult::Propagate;
                }
                external_drop_hover_timer.stop();
                external_drop_flush_timer.stop();
                clear_external_drop_state(
                    &window,
                    &external_drop_paths,
                    &external_drop_target,
                    &sftp_drop_hover_active,
                    &terminal_drop_hover_active,
                );
            }

            if let winit::event::WindowEvent::DroppedFile(path) = event {
                let window = handle.unwrap();
                external_drop_hover_timer.stop();
                {
                    let mut pending_paths = external_drop_paths.borrow_mut();
                    if !pending_paths.iter().any(|pending| pending == path) {
                        pending_paths.push(path.clone());
                    }
                }
                tracing::info!(
                    target: "app.drop",
                    path_count = external_drop_paths.borrow().len(),
                    "external file drop entered app event handler"
                );
                refresh_external_drop_target(
                    &window,
                    &sftp_drop_hover_active,
                    &terminal_drop_hover_active,
                    &external_drop_target,
                    *last_pointer_position.borrow(),
                );
                let drop_target = *external_drop_target.borrow();
                if drop_target == ExternalDropTarget::None {
                    external_drop_flush_pending.set(false);
                    tracing::info!(
                        target: "app.drop",
                        pointer_x = ?last_pointer_position.borrow().map(|position| position.0),
                        pointer_y = ?last_pointer_position.borrow().map(|position| position.1),
                        host_mode = window.get_workspace_session_host_mode().as_str(),
                        terminal_x = window.get_layout_workspace_session_native_surface_x(),
                        terminal_y = window.get_layout_titlebar_height()
                            + window.get_layout_workspace_session_native_surface_y(),
                        terminal_width = window.get_layout_workspace_session_native_surface_width(),
                        terminal_height = window.get_layout_workspace_session_native_surface_height(),
                        "external file drop had no matching app target"
                    );
                    external_drop_flush_timer.stop();
                    clear_external_drop_state(
                        &window,
                        &external_drop_paths,
                        &external_drop_target,
                        &sftp_drop_hover_active,
                        &terminal_drop_hover_active,
                    );
                    return EventResult::Propagate;
                }

                let timer_ref = Rc::clone(&external_drop_flush_timer);
                let handle = handle.clone();
                let sftp_drop_hover_active_ref = Rc::clone(&sftp_drop_hover_active);
                let terminal_drop_hover_active_ref = Rc::clone(&terminal_drop_hover_active);
                let external_drop_paths_ref = Rc::clone(&external_drop_paths);
                let external_drop_target_ref = Rc::clone(&external_drop_target);
                let external_drop_flush_pending_ref = Rc::clone(&external_drop_flush_pending);
                external_drop_flush_pending.set(true);
                timer_ref.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(24),
                    move || {
                        external_drop_flush_pending_ref.set(false);
                        let Some(window) = handle.upgrade() else {
                            return;
                        };
                        let pending_paths =
                            std::mem::take(&mut *external_drop_paths_ref.borrow_mut());
                        if pending_paths.is_empty() {
                            return;
                        }

                        let dropped_paths = pending_paths
                            .into_iter()
                            .map(|path| SharedString::from(path.to_string_lossy().to_string()))
                            .collect::<Vec<_>>();
                        match *external_drop_target_ref.borrow() {
                            ExternalDropTarget::Sftp => {
                                tracing::info!(
                                    target: "app.drop",
                                    path_count = dropped_paths.len(),
                                    target = "sftp",
                                    "routing external file drop"
                                );
                                window.set_sftp_panel_external_drop_paths(ModelRc::new(
                                    VecModel::from(dropped_paths),
                                ));
                                window.invoke_sftp_panel_external_drop_requested();
                            }
                            ExternalDropTarget::Terminal => {
                                tracing::info!(
                                    target: "app.drop",
                                    path_count = dropped_paths.len(),
                                    target = "terminal",
                                    "routing external file drop"
                                );
                                window.set_workspace_terminal_external_drop_paths(ModelRc::new(
                                    VecModel::from(dropped_paths),
                                ));
                                window.invoke_workspace_terminal_external_drop_requested();
                            }
                            ExternalDropTarget::None => {}
                        }
                        *external_drop_target_ref.borrow_mut() = ExternalDropTarget::None;
                        update_sftp_drop_hover_state(&window, &sftp_drop_hover_active_ref, false);
                        update_terminal_drop_hover_state(
                            &window,
                            &terminal_drop_hover_active_ref,
                            false,
                        );
                    },
                );
            }

            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) {
                #[cfg(target_os = "windows")]
                {
                    let window = handle.unwrap();
                    let _ = window.window().with_winit_window(|winit_window| {
                        sync_windows_true_window_placement(
                            &window,
                            &state,
                            _effects.as_ref(),
                            winit_window,
                        );
                        save_restored_window_bounds_for_window(
                            &_ui_preferences_store,
                            winit_window,
                        );
                    });
                }
            }

            EventResult::Propagate
        });
}

fn logical_pointer_from_physical(window: &AppWindow, x: f64, y: f64) -> (f32, f32) {
    let scale_factor = window.window().scale_factor().max(1.0) as f32;
    (x as f32 / scale_factor, y as f32 / scale_factor)
}

fn refresh_external_drop_target(
    window: &AppWindow,
    sftp_drop_hover_active: &Rc<RefCell<bool>>,
    terminal_drop_hover_active: &Rc<RefCell<bool>>,
    external_drop_target: &Rc<RefCell<ExternalDropTarget>>,
    cached_pointer: Option<(f32, f32)>,
) {
    let drop_target = resolve_external_drop_target(
        window,
        resolve_external_drop_pointer(window, cached_pointer),
    );
    *external_drop_target.borrow_mut() = drop_target;
    update_sftp_drop_hover_state(
        window,
        sftp_drop_hover_active,
        drop_target == ExternalDropTarget::Sftp,
    );
    update_terminal_drop_hover_state(
        window,
        terminal_drop_hover_active,
        drop_target == ExternalDropTarget::Terminal,
    );
}

fn clear_external_drop_state(
    window: &AppWindow,
    external_drop_paths: &Rc<RefCell<Vec<PathBuf>>>,
    external_drop_target: &Rc<RefCell<ExternalDropTarget>>,
    sftp_drop_hover_active: &Rc<RefCell<bool>>,
    terminal_drop_hover_active: &Rc<RefCell<bool>>,
) {
    external_drop_paths.borrow_mut().clear();
    *external_drop_target.borrow_mut() = ExternalDropTarget::None;
    update_sftp_drop_hover_state(window, sftp_drop_hover_active, false);
    update_terminal_drop_hover_state(window, terminal_drop_hover_active, false);
}

fn resolve_external_drop_target(
    window: &AppWindow,
    pointer: Option<(f32, f32)>,
) -> ExternalDropTarget {
    if sftp_drop_target_contains(window, pointer) {
        ExternalDropTarget::Sftp
    } else if workspace_terminal_drop_target_contains(window, pointer) {
        ExternalDropTarget::Terminal
    } else if window.get_workspace_session_host_mode() == "terminal"
        && !window.get_active_workspace_session_id().is_empty()
    {
        ExternalDropTarget::Terminal
    } else {
        ExternalDropTarget::None
    }
}

fn resolve_external_drop_pointer(
    window: &AppWindow,
    cached_pointer: Option<(f32, f32)>,
) -> Option<(f32, f32)> {
    #[cfg(target_os = "windows")]
    {
        if let Some(pointer) = query_windows_drop_pointer(window) {
            return Some(pointer);
        }
    }

    let _ = window;
    cached_pointer
}

#[cfg(target_os = "windows")]
fn query_windows_drop_pointer(window: &AppWindow) -> Option<(f32, f32)> {
    use windows_sys::Win32::Foundation::{HWND, POINT};
    use windows_sys::Win32::Graphics::Gdi::ScreenToClient;
    use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;

    let hwnd = resolve_host_window_hwnd(window).map(|value| value as HWND)?;
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        if GetCursorPos(&mut point) == 0 {
            return None;
        }
        if ScreenToClient(hwnd, &mut point) == 0 {
            return None;
        }
    }
    Some(logical_pointer_from_physical(
        window,
        f64::from(point.x),
        f64::from(point.y),
    ))
}

fn sftp_drop_target_contains(window: &AppWindow, pointer: Option<(f32, f32)>) -> bool {
    let Some((pointer_x, pointer_y)) = pointer else {
        return false;
    };

    let origin_x = window.get_layout_sftp_drop_target_x();
    let origin_y = window.get_layout_sftp_drop_target_y();
    let width = window.get_layout_sftp_drop_target_width();
    let height = window.get_layout_sftp_drop_target_height();
    if width <= 0.0 || height <= 0.0 {
        return false;
    }

    pointer_x >= origin_x
        && pointer_x <= origin_x + width
        && pointer_y >= origin_y
        && pointer_y <= origin_y + height
}

fn workspace_terminal_drop_target_contains(
    window: &AppWindow,
    pointer: Option<(f32, f32)>,
) -> bool {
    if window.get_workspace_session_host_mode() != "terminal" {
        return false;
    }

    let Some((pointer_x, pointer_y)) = pointer else {
        return false;
    };

    let origin_x = window.get_layout_workspace_session_native_surface_x();
    let origin_y = window.get_layout_titlebar_height()
        + window.get_layout_workspace_session_native_surface_y();
    let width = window.get_layout_workspace_session_native_surface_width();
    let height = window.get_layout_workspace_session_native_surface_height();
    if width <= 0.0 || height <= 0.0 {
        return false;
    }

    pointer_x >= origin_x
        && pointer_x <= origin_x + width
        && pointer_y >= origin_y
        && pointer_y <= origin_y + height
}

fn update_sftp_drop_hover_state(window: &AppWindow, hover_state: &Rc<RefCell<bool>>, active: bool) {
    if *hover_state.borrow() == active {
        return;
    }

    *hover_state.borrow_mut() = active;
    window.invoke_sftp_panel_external_drop_hover_changed(active);
}

fn update_terminal_drop_hover_state(
    window: &AppWindow,
    hover_state: &Rc<RefCell<bool>>,
    active: bool,
) {
    if *hover_state.borrow() == active {
        return;
    }

    *hover_state.borrow_mut() = active;
    window.invoke_workspace_terminal_external_drop_hover_changed(active);
}

pub(super) fn sync_sync_modal_state(window: &AppWindow, state: &ShellViewModel) {
    let modal = state.sync_modal_state();

    window.set_sync_modal_open(modal.open);
    window.set_sync_modal_mode(modal.mode.id().into());
    window.set_sync_modal_title(modal.title.clone().into());
    window.set_sync_modal_headline(modal.headline.clone().into());
    window.set_sync_modal_status_text(modal.status_text.clone().into());
    window.set_sync_modal_error_text(modal.error_text.clone().into());
    window.set_sync_modal_provider_label(modal.provider_label.clone().into());
    window.set_sync_modal_target_label(modal.target_label.clone().into());
    window.set_sync_modal_conflict_count(modal.conflict_count);
    window.set_sync_modal_conflict_summary(modal.conflict_summary.clone().into());
    window.set_sync_modal_primary_action_label(modal.primary_action_label.clone().into());
    window.set_sync_modal_secondary_action_label(modal.secondary_action_label.clone().into());
    window.set_sync_modal_validation_state(modal.validation_state.id().into());
    window.set_sync_modal_validation_message(modal.validation_message.clone().into());
    window.set_sync_modal_git_provider_kind(modal.git_provider_kind.clone().into());
    window.set_sync_modal_git_remote_url(modal.git_remote_url.clone().into());
    window.set_sync_modal_git_base_url(modal.git_base_url.clone().into());
    window.set_sync_modal_git_api_base_url(modal.git_api_base_url.clone().into());
    window.set_sync_modal_git_namespace(modal.git_namespace.clone().into());
    window.set_sync_modal_git_repository(modal.git_repository.clone().into());
    window.set_sync_modal_git_branch(modal.git_branch.clone().into());
    window.set_sync_modal_git_root_path(modal.git_root_path.clone().into());
    window.set_sync_modal_git_auth_mode(modal.git_auth_mode.clone().into());
    window.set_sync_modal_git_https_username(modal.git_https_username.clone().into());
    window.set_sync_modal_git_https_secret(modal.git_https_secret.clone().into());
    window.set_sync_modal_git_pat(modal.git_pat.clone().into());
    window.set_sync_modal_git_https_secret_visible(modal.git_https_secret_visible);
    window.set_sync_modal_git_ssh_private_key(modal.git_ssh_private_key.clone().into());
    window.set_sync_modal_git_ssh_passphrase(modal.git_ssh_passphrase.clone().into());
    window.set_sync_modal_git_ssh_passphrase_visible(modal.git_ssh_passphrase_visible);
    window.set_sync_modal_master_password(modal.master_password.clone().into());
    window.set_sync_modal_master_password_visible(modal.master_password_visible);
    window.set_sync_modal_local_last_sync_text(modal.local_last_sync_text.clone().into());
    window.set_sync_modal_remote_last_update_text(modal.remote_last_update_text.clone().into());
    window.set_sync_modal_primary_revision_text(modal.primary_revision_text.clone().into());
    window.set_sync_modal_remote_status_text(modal.remote_status_text.clone().into());
    window.set_sync_modal_remote_status_loading(modal.remote_status_loading);
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn sync_ssh_host_key_modal_state(window: &AppWindow, state: &ShellViewModel) {
    match &state.ssh_host_key_prompt_state {
        Some(prompt) => {
            window.set_ssh_host_key_modal_open(true);
            window.set_ssh_host_key_modal_host(prompt.host.clone().into());
            window.set_ssh_host_key_modal_fingerprint(prompt.fingerprint.clone().into());
        }
        None => {
            window.set_ssh_host_key_modal_open(false);
            window.set_ssh_host_key_modal_host("".into());
            window.set_ssh_host_key_modal_fingerprint("".into());
        }
    }
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn sync_workspace_paste_warning_modal_state(
    window: &AppWindow,
    pending: Option<&PendingWorkspacePasteWarning>,
) {
    match pending {
        Some(pending) => {
            window.set_workspace_paste_warning_line_count(
                i32::try_from(pending.logical_line_count).unwrap_or(i32::MAX),
            );
            window.set_workspace_paste_warning_editor_mode(matches!(
                pending.prompt_mode,
                WorkspacePastePromptMode::Editor
            ));
            window.set_workspace_paste_warning_text(pending.text.clone().into());
            window.set_workspace_paste_warning_modal_open(true);
        }
        None => {
            window.set_workspace_paste_warning_modal_open(false);
            window.set_workspace_paste_warning_line_count(0);
            window.set_workspace_paste_warning_editor_mode(false);
            window.set_workspace_paste_warning_text("".into());
        }
    }
    super::sync_workspace_native_terminal_surface_geometry(window);
}

pub(super) fn bind_windowing_callbacks(
    window: &AppWindow,
    view_model: &Rc<RefCell<ShellViewModel>>,
    effects: &Rc<dyn PlatformWindowEffects>,
    modal_drag_state: &Rc<RefCell<Option<ModalDragState>>>,
    controller: &Rc<WindowController<AppWindow>>,
) {
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, height| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        sync_shell_layout(&window, &mut state, width as u32, height as u32);
        install_windows_frame_adapter(&window);
    });

    let controller_ref = Rc::clone(controller);
    window.on_close_requested(move || {
        let _ = controller_ref.close();
    });

    let controller_ref = Rc::clone(controller);
    window.on_drag_requested(move || {
        let _ = controller_ref.drag();
    });

    let controller_ref = Rc::clone(controller);
    window.on_drag_resize_requested(move |direction| {
        if let Some(direction) = parse_resize_direction(direction.as_str()) {
            let _ = controller_ref.drag_resize(direction);
        }
    });

    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(modal_drag_state);
    window.on_blocking_modal_drag_requested(move |pointer_x, pointer_y| {
        let window = handle.unwrap();
        let current_offset = ModalOffset {
            x: window.get_blocking_modal_offset_x(),
            y: window.get_blocking_modal_offset_y(),
        };
        *modal_drag_state_ref.borrow_mut() =
            Some(begin_modal_drag(pointer_x, pointer_y, current_offset));
    });

    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(modal_drag_state);
    window.on_blocking_modal_drag_moved(move |pointer_x, pointer_y| {
        let Some(drag_state) = *modal_drag_state_ref.borrow() else {
            return;
        };
        let window = handle.unwrap();
        let next_offset = update_modal_drag(drag_state, pointer_x, pointer_y);
        window.set_blocking_modal_offset_x(next_offset.x);
        window.set_blocking_modal_offset_y(next_offset.y);
    });

    let modal_drag_state_ref = Rc::clone(modal_drag_state);
    window.on_blocking_modal_drag_ended(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let modal_drag_state_ref = Rc::clone(modal_drag_state);
    window.on_blocking_modal_focus_restore_requested(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(controller);
    let effects_ref = Rc::clone(effects);
    window.on_drag_double_clicked(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next = controller_ref.toggle_maximize(state.is_window_maximized());
        let next = if next {
            WindowPlacementKind::Maximized
        } else {
            WindowPlacementKind::Restored
        };
        state.set_window_placement(next);
        super::shell_chrome::sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });
}
