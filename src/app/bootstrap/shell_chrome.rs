//! Bootstrap shell chrome binder module.

use super::*;

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
    window.set_transfer_queue_total(
        i32::try_from(state.sftp_queue_summary.total_count).unwrap_or(i32::MAX),
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
    window.set_sync_feedback_text(state.sync_feedback_state().text.clone().into());
    window.set_sync_feedback_sequence(state.sync_feedback_state().sequence);
    window.set_sync_feedback_running(state.sync_feedback_state().running);
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
        sftp::sync_right_panel_state(&window, &state);
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
    window.on_open_appearance_panel_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let (width, height) = current_window_size(&window);
        state.open_appearance_panel();
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
        sftp::sync_right_panel_state(&window, &state);
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
