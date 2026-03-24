//! Wires the Slint window to runtime state, persisted preferences, and native window hooks during startup.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use directories::ProjectDirs;
use slint::{ComponentHandle, ModelRc, Timer, TimerMode, VecModel};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AppWindow;
use crate::AssetsContextMenuItem;
use crate::ConsoleAssetItem;
use crate::WorkspaceTabItem;
use crate::app::app_paths::{AppRootPathInputs, resolve_app_root_paths};
use crate::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    RedbAssetCatalogStore, asset_tree_to_catalog, catalog_to_asset_tree,
};
use crate::app::async_runtime::AppAsyncRuntime;
use crate::app::runtime_profile::AppRuntimeProfile;
use crate::app::ssh::profile::ConnectionProfile;
use crate::app::ssh::runtime::{SessionRuntimeEvent, SshSessionRuntime};
use crate::app::ssh::session_manager::{
    OpenSessionMode, SessionHandle, SessionManager, SessionRuntimeLauncher,
};
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{
    ModalDragState, ModalOffset, WindowController, apply_restored_window_size, begin_modal_drag,
    parse_resize_direction, update_modal_drag, window_appearance,
};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
};
use crate::shell::assets::AssetDisclosureState;
use crate::shell::context_menu::{
    CONTEXT_MENU_COLUMN_GAP, CONTEXT_MENU_COLUMN_WIDTH, ContextMenuActionNode,
    ContextMenuActionState, ContextTargetKind, MenuPlacementInput, Rect, SelectionContext,
    context_menu_column_height, context_menu_column_offset, resolve_action_tree,
    resolve_root_menu_origin, should_keep_corridor_open, visible_columns_for_path,
};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for, toolbar_descriptor_for};
use crate::shell::tabs::WorkspaceTab;
use crate::shell::view_model::{
    AssetModalState, AssetSshConnectionDraft, ShellViewModel, SshModalAction,
};
use crate::theme::ThemeMode;

#[derive(Clone)]
struct ShellSessionBridge {
    manager: SessionManager,
}

#[derive(Clone, Default)]
struct LiveSessionRuntimeLauncher;

impl SessionRuntimeLauncher for LiveSessionRuntimeLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        session_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move {
            let _session = SshSessionRuntime::connect(profile, session_id, event_tx).await?;
            Ok(())
        })
    }

    fn probe(
        &self,
        profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move {
            let (event_tx, _event_rx) = mpsc::unbounded_channel();
            let runtime = SshSessionRuntime::connect(profile, Uuid::new_v4(), event_tx).await?;
            runtime.disconnect()?;
            Ok(())
        })
    }
}

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn runtime_window_title(_profile: AppRuntimeProfile) -> String {
    app_title().to_owned()
}

pub fn startup_failure_message(_profile: AppRuntimeProfile, err: &str) -> Option<String> {
    Some(format!(
        "Mica Term failed to initialize winit-femtovg-wgpu: {err}"
    ))
}

pub fn default_window_size() -> (u32, u32) {
    (
        ShellMetrics::WINDOW_DEFAULT_WIDTH,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT,
    )
}

#[cfg(target_os = "windows")]
fn sync_windows_true_window_placement(
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
    sync_top_status_bar_state(window, &state, effects);
}

#[cfg(target_os = "windows")]
fn bind_windows_window_state_tracking(
    window: &AppWindow,
    state: Rc<RefCell<ShellViewModel>>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    use slint::ComponentHandle;
    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};

    let handle = window.as_weak();
    window
        .window()
        .on_winit_window_event(move |_slint_window, event| {
            // Win32 snap/maximize state can drift from declarative UI state, so re-sample it when
            // the platform reports geometry-affecting events.
            if matches!(
                event,
                winit::event::WindowEvent::Moved(_)
                    | winit::event::WindowEvent::Resized(_)
                    | winit::event::WindowEvent::ScaleFactorChanged { .. }
            ) {
                let window = handle.unwrap();
                let _ = window.window().with_winit_window(|winit_window| {
                    sync_windows_true_window_placement(
                        &window,
                        &state,
                        effects.as_ref(),
                        winit_window,
                    );
                });
            }

            EventResult::Propagate
        });
}

#[cfg(not(target_os = "windows"))]
fn bind_windows_window_state_tracking(
    _window: &AppWindow,
    _state: Rc<RefCell<ShellViewModel>>,
    _effects: Rc<dyn PlatformWindowEffects>,
) {
}

fn sync_theme_and_window_effects(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    window.set_dark_mode(state.theme_mode == ThemeMode::Dark);
    window.window().request_redraw();

    let request = build_native_window_appearance_request(state.theme_mode, window_appearance());
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

fn sync_top_status_bar_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_theme_and_window_effects(window, state, effects);
    window.set_show_right_panel(state.show_right_panel);
    window.set_show_global_menu(state.show_global_menu);
    window.set_is_window_maximized(state.is_window_maximized());
    window.set_is_window_active(state.is_window_active);
    window.set_is_window_always_on_top(state.is_always_on_top);
}

fn sync_sidebar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_assets_sidebar(state.show_assets_sidebar);
    window.set_active_sidebar_destination(state.active_sidebar_destination.id().into());
    window.set_sidebar_items(ModelRc::new(VecModel::from(sidebar_items_for(state))));
    sync_assets_toolbar_state(window, state);
    sync_console_assets(window, state);
}

fn sync_assets_toolbar_state(window: &AppWindow, state: &ShellViewModel) {
    let descriptor = toolbar_descriptor_for(state.active_sidebar_destination, state);
    window.set_asset_view_mode(state.asset_view_mode.id().into());
    window.set_asset_search_expanded(state.asset_search_expanded);
    window.set_assets_search_query(state.asset_search_query.clone().into());
    window.set_asset_create_menu_open(state.asset_create_menu_open);
    window.set_asset_uses_create_popover(descriptor.uses_create_popover);
    window.set_asset_tree_fully_expanded(state.asset_tree_fully_expanded);
    window.set_asset_primary_create_action_id(
        descriptor.primary_create_action_id.unwrap_or("").into(),
    );
    window.set_asset_primary_create_tooltip(descriptor.primary_create_tooltip.into());
    window.set_asset_search_tooltip(descriptor.search_tooltip.into());
    window.set_asset_view_mode_tooltip(descriptor.view_mode_tooltip.into());
    window.set_asset_tree_expansion_tooltip(descriptor.tree_expansion_tooltip.into());
    window.set_asset_show_tree_controls(descriptor.show_tree_controls);
    window.set_asset_tree_controls_enabled(descriptor.tree_controls_enabled);
}

fn sync_assets_context_menu_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_assets_context_menu_open(state.context_menu_open);
    window.set_assets_context_menu_anchor_x(state.context_menu_anchor_x);
    window.set_assets_context_menu_anchor_y(state.context_menu_anchor_y);
    window.set_assets_context_menu_origin_x(state.context_menu_origin_x);
    window.set_assets_context_menu_origin_y(state.context_menu_origin_y);
    window.set_assets_context_menu_child_flows_left(state.context_menu_child_flows_left);
    window.set_assets_context_menu_primary_items(ModelRc::new(VecModel::from(
        context_menu_primary_items_for(state),
    )));
    window.set_assets_context_menu_secondary_items(ModelRc::new(VecModel::from(
        context_menu_secondary_items_for(state),
    )));
    window.set_assets_context_menu_tertiary_items(ModelRc::new(VecModel::from(
        context_menu_tertiary_items_for(state),
    )));
    window.set_context_menu_feedback_text(state.context_menu_feedback_text.clone().into());
}

fn sync_asset_modal_state(window: &AppWindow, state: &ShellViewModel) {
    match &state.asset_modal_state {
        Some(AssetModalState::NewFolder { draft_name, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-folder".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_auth_method("password".into());
            window.set_asset_ssh_modal_private_key_source("content".into());
            window.set_asset_ssh_modal_password("".into());
            window.set_asset_ssh_modal_private_key_content("".into());
            window.set_asset_ssh_modal_private_key_path("".into());
            window.set_asset_ssh_modal_passphrase("".into());
            window.set_asset_ssh_modal_remark("".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
        Some(AssetModalState::NewSshConnection {
            active_tab, draft, ..
        }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-ssh-connection".into());
            window.set_asset_modal_can_confirm(state.ssh_modal_save_enabled());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_folder_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_active_tab(active_tab.id().into());
            window.set_asset_ssh_modal_name(draft.name.clone().into());
            window.set_asset_ssh_modal_host(draft.host.clone().into());
            window.set_asset_ssh_modal_user(draft.user.clone().into());
            window.set_asset_ssh_modal_port(draft.port.clone().into());
            window.set_asset_ssh_modal_auth_method(draft.auth_method.clone().into());
            window.set_asset_ssh_modal_private_key_source(draft.private_key_source.clone().into());
            window.set_asset_ssh_modal_password(draft.password.clone().into());
            window
                .set_asset_ssh_modal_private_key_content(draft.private_key_content.clone().into());
            window.set_asset_ssh_modal_private_key_path(draft.private_key_path.clone().into());
            window.set_asset_ssh_modal_passphrase(draft.passphrase.clone().into());
            window.set_asset_ssh_modal_remark(draft.remark.clone().into());
            window.set_asset_ssh_modal_environment(draft.environment.clone().into());
            window.set_asset_ssh_modal_proxy_method(draft.proxy_method.clone().into());
            window.set_asset_ssh_modal_connect_family_enabled(
                state.ssh_modal_connect_family_enabled(),
            );
            window.set_asset_ssh_modal_feedback_state(state.ssh_modal_feedback_state_id().into());
            window.set_asset_ssh_modal_feedback_message(state.ssh_modal_feedback_message().into());
        }
        Some(AssetModalState::RenameAsset { draft_name, .. }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_rename_modal_open(true);
            window.set_asset_rename_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_validation_message(
                state.asset_rename_modal_validation_message().into(),
            );
            window.set_asset_rename_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_auth_method("password".into());
            window.set_asset_ssh_modal_private_key_source("content".into());
            window.set_asset_ssh_modal_password("".into());
            window.set_asset_ssh_modal_private_key_content("".into());
            window.set_asset_ssh_modal_private_key_path("".into());
            window.set_asset_ssh_modal_passphrase("".into());
            window.set_asset_ssh_modal_remark("".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
        Some(AssetModalState::DeleteAssetConfirm {
            label,
            descendant_count,
            ..
        }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(true);
            window.set_asset_delete_confirm_target_label(label.clone().into());
            window.set_asset_delete_confirm_descendant_count(*descendant_count as i32);
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_auth_method("password".into());
            window.set_asset_ssh_modal_private_key_source("content".into());
            window.set_asset_ssh_modal_password("".into());
            window.set_asset_ssh_modal_private_key_content("".into());
            window.set_asset_ssh_modal_private_key_path("".into());
            window.set_asset_ssh_modal_passphrase("".into());
            window.set_asset_ssh_modal_remark("".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
        None => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_auth_method("password".into());
            window.set_asset_ssh_modal_private_key_source("content".into());
            window.set_asset_ssh_modal_password("".into());
            window.set_asset_ssh_modal_private_key_content("".into());
            window.set_asset_ssh_modal_private_key_path("".into());
            window.set_asset_ssh_modal_passphrase("".into());
            window.set_asset_ssh_modal_remark("".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
    }
}

fn sync_ssh_host_key_modal_state(window: &AppWindow, state: &ShellViewModel) {
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
}

fn schedule_asset_modal_focus(window: &AppWindow) {
    let handle = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        let window = handle.unwrap();
        if window.get_asset_modal_open()
            || window.get_asset_rename_modal_open()
            || window.get_asset_delete_confirm_modal_open()
        {
            window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);
        }
    });
}

fn parse_context_target_kind(value: &str) -> ContextTargetKind {
    match value {
        "ssh" => ContextTargetKind::SshConnection,
        "folder" => ContextTargetKind::Folder,
        _ => ContextTargetKind::BlankArea,
    }
}

fn build_session_bridge(runtime_handle: tokio::runtime::Handle) -> Rc<ShellSessionBridge> {
    Rc::new(ShellSessionBridge {
        manager: SessionManager::new_with_launcher(
            runtime_handle,
            Arc::new(LiveSessionRuntimeLauncher),
        ),
    })
}

fn selection_context_for(state: &ShellViewModel) -> SelectionContext {
    state.context_menu_selection()
}

fn profile_for_saved_asset(state: &ShellViewModel, asset_id: &str) -> Option<ConnectionProfile> {
    let node = state.console_asset_tree().node(asset_id)?;
    let spec = state.console_asset_tree().ssh_connection_spec(asset_id)?;
    ConnectionProfile::from_saved_asset(asset_id, &node.title, spec).ok()
}

fn temporary_session_asset_id_for_draft(_draft: &AssetSshConnectionDraft) -> String {
    format!("session:{}", Uuid::new_v4())
}

fn merge_session_handle_into_tabs(state: &mut ShellViewModel, handle: &SessionHandle) {
    let mut tabs = state.workspace_tabs().to_vec();
    let next_tab = WorkspaceTab::from_session(handle);

    if let Some(existing) = tabs
        .iter_mut()
        .find(|tab| tab.session_id == next_tab.session_id)
    {
        *existing = next_tab;
    } else {
        tabs.push(next_tab);
    }

    state.set_workspace_tabs(tabs);
    let _ = state.activate_workspace_session(handle.session_id.to_string().as_str());
}

fn sync_workspace_projection_from_manager(
    state: &mut ShellViewModel,
    manager: &SessionManager,
) -> bool {
    let next_tabs = manager
        .ordered_sessions()
        .into_iter()
        .map(|handle| WorkspaceTab::from_session(&handle))
        .collect::<Vec<_>>();
    let next_surface = state
        .active_workspace_session_id()
        .and_then(|session_id| Uuid::parse_str(session_id).ok())
        .and_then(|session_id| manager.terminal_surface(session_id));

    let tabs_changed = state.workspace_tabs() != next_tabs.as_slice();
    if tabs_changed {
        state.set_workspace_tabs(next_tabs);
    }

    let surface_changed = state.active_workspace_terminal_surface().cloned() != next_surface;
    if surface_changed {
        state.set_active_workspace_terminal_surface(next_surface);
    }

    tabs_changed || surface_changed
}

fn open_session_with_profile(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    profile: ConnectionProfile,
    mode: OpenSessionMode,
) -> anyhow::Result<()> {
    let handle = bridge.manager.open_session(profile, mode)?;
    let resolved = bridge.manager.session(handle.session_id).unwrap_or(handle);
    merge_session_handle_into_tabs(state, &resolved);
    let _ = sync_workspace_projection_from_manager(state, &bridge.manager);
    Ok(())
}

fn target_session_id_for_asset(state: &ShellViewModel, asset_id: &str) -> Option<String> {
    state
        .active_workspace_tab()
        .filter(|tab| tab.asset_id == asset_id)
        .map(|tab| tab.session_id.clone())
        .or_else(|| {
            state
                .workspace_tabs()
                .iter()
                .find(|tab| tab.asset_id == asset_id)
                .map(|tab| tab.session_id.clone())
        })
}

fn disconnect_session_for_asset(
    state: &mut ShellViewModel,
    bridge: &ShellSessionBridge,
    asset_id: &str,
) -> bool {
    let Some(session_id) = target_session_id_for_asset(state, asset_id) else {
        return false;
    };
    let Ok(session_uuid) = Uuid::parse_str(&session_id) else {
        return false;
    };
    let Some(handle) = bridge.manager.disconnect_session(session_uuid) else {
        return false;
    };
    merge_session_handle_into_tabs(state, &handle);
    let _ = sync_workspace_projection_from_manager(state, &bridge.manager);
    true
}

fn close_session_by_id(
    state: &mut ShellViewModel,
    bridge: Option<&ShellSessionBridge>,
    session_id: &str,
) -> bool {
    if let Some(bridge) = bridge
        && let Ok(session_uuid) = Uuid::parse_str(session_id)
    {
        let _ = bridge.manager.close_session(session_uuid);
    }
    state.close_workspace_session_with_fallback(session_id)
}

fn context_menu_roots_for(state: &ShellViewModel) -> Vec<ContextMenuActionNode> {
    let Some(target_kind) = state.context_menu_target_kind else {
        return Vec::new();
    };

    if !state.context_menu_open {
        return Vec::new();
    }

    resolve_action_tree(target_kind, &selection_context_for(state))
}

fn context_menu_columns_for(state: &ShellViewModel) -> [Vec<ContextMenuActionNode>; 3] {
    let roots = context_menu_roots_for(state);
    visible_columns_for_path(&roots, &state.context_menu_open_path)
}

fn context_menu_items_to_model(
    items: Vec<ContextMenuActionNode>,
    open_index: Option<usize>,
) -> Vec<AssetsContextMenuItem> {
    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| AssetsContextMenuItem {
            id: item.id.into(),
            label: item.label.into(),
            icon_id: item.icon_id.into(),
            enabled: item.state != ContextMenuActionState::Disabled,
            planned: item.state == ContextMenuActionState::Planned,
            has_children: !item.children.is_empty(),
            open: open_index == Some(index),
            divider_before: item.divider_before,
        })
        .collect()
}

fn context_menu_primary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(
        columns[0].clone(),
        state.context_menu_open_path.first().copied(),
    )
}

fn context_menu_secondary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(
        columns[1].clone(),
        state.context_menu_open_path.get(1).copied(),
    )
}

fn context_menu_tertiary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(columns[2].clone(), None)
}

fn context_menu_hover_path_for(
    state: &ShellViewModel,
    column_index: usize,
    row_index: usize,
) -> Vec<usize> {
    let columns = context_menu_columns_for(state);

    match column_index {
        0 => columns[0]
            .get(row_index)
            .map(|node| {
                if node.children.is_empty() {
                    Vec::new()
                } else {
                    vec![row_index]
                }
            })
            .unwrap_or_default(),
        1 => {
            let Some(first_index) = state.context_menu_open_path.first().copied() else {
                return Vec::new();
            };

            columns[1]
                .get(row_index)
                .map(|node| {
                    if node.children.is_empty() {
                        vec![first_index]
                    } else {
                        vec![first_index, row_index]
                    }
                })
                .unwrap_or_else(|| vec![first_index])
        }
        _ => state.context_menu_open_path.clone(),
    }
}

fn context_menu_action_entry_for(
    state: &ShellViewModel,
    action_id: &str,
) -> Option<(Vec<usize>, ContextMenuActionNode)> {
    find_context_menu_action_entry(&context_menu_roots_for(state), action_id, Vec::new())
}

fn find_context_menu_action_entry(
    nodes: &[ContextMenuActionNode],
    action_id: &str,
    prefix: Vec<usize>,
) -> Option<(Vec<usize>, ContextMenuActionNode)> {
    for (index, node) in nodes.iter().enumerate() {
        let mut path = prefix.clone();
        path.push(index);

        if node.id == action_id {
            return Some((path, node.clone()));
        }

        if let Some(found) = find_context_menu_action_entry(&node.children, action_id, path) {
            return Some(found);
        }
    }

    None
}

fn context_menu_visible_column_count(state: &ShellViewModel) -> usize {
    context_menu_columns_for(state)
        .into_iter()
        .take_while(|column| !column.is_empty())
        .count()
}

fn context_menu_overlay_height_for(state: &ShellViewModel) -> f32 {
    context_menu_columns_for(state)
        .into_iter()
        .filter(|column| !column.is_empty())
        .map(|column| context_menu_column_height(column.as_slice()))
        .fold(0.0, f32::max)
}

fn context_menu_child_width_for(state: &ShellViewModel) -> f32 {
    let child_count = context_menu_visible_column_count(state).saturating_sub(1) as f32;
    if child_count <= 0.0 {
        0.0
    } else {
        child_count * (CONTEXT_MENU_COLUMN_WIDTH + CONTEXT_MENU_COLUMN_GAP)
    }
}

fn context_menu_column_rects_for(state: &ShellViewModel) -> [Option<Rect>; 3] {
    let columns = context_menu_columns_for(state);
    let visible_column_count = columns
        .iter()
        .take_while(|column| !column.is_empty())
        .count();
    let mut rects = [None, None, None];

    for column_index in 0..visible_column_count {
        let height = context_menu_column_height(columns[column_index].as_slice());
        rects[column_index] = Some(Rect {
            x: state.context_menu_origin_x
                + context_menu_column_offset(
                    column_index,
                    visible_column_count,
                    state.context_menu_child_flows_left,
                ),
            y: state.context_menu_origin_y,
            width: CONTEXT_MENU_COLUMN_WIDTH,
            height,
        });
    }

    rects
}

fn update_context_menu_placement(window: &AppWindow, state: &mut ShellViewModel) {
    if !state.context_menu_open {
        state.set_context_menu_placement(0.0, 0.0, false);
        return;
    }

    let (host_width, host_height) = current_window_size(window);
    let (origin_x, origin_y, child_flows_left) = resolve_root_menu_origin(MenuPlacementInput {
        host_width: host_width as f32,
        host_height: host_height as f32,
        anchor_x: state.context_menu_anchor_x,
        anchor_y: state.context_menu_anchor_y,
        root_width: CONTEXT_MENU_COLUMN_WIDTH,
        root_height: context_menu_overlay_height_for(state),
        child_width: context_menu_child_width_for(state),
    });

    state.set_context_menu_placement(origin_x, origin_y, child_flows_left);
}

fn sync_console_assets(window: &AppWindow, state: &ShellViewModel) {
    let rows = state
        .visible_console_asset_rows()
        .into_iter()
        .map(|row| ConsoleAssetItem {
            id: row.id.clone().into(),
            kind: row.kind.id().into(),
            label: row.label.clone().into(),
            depth: row.depth as i32,
            has_children: row.has_children,
            expanded: row.expanded,
            selected: state.selected_asset_ids.iter().any(|id| id == &row.id),
            focused: state.focused_asset_id.as_deref() == Some(row.id.as_str()),
            disclosure_state: match row.disclosure_state {
                AssetDisclosureState::None => "none",
                AssetDisclosureState::Collapsed => "collapsed",
                AssetDisclosureState::Expanded => "expanded",
            }
            .into(),
            path_hint: row.path_hint.clone().unwrap_or_default().into(),
            compact_flat_mode: state.asset_view_mode.id() == "flat",
        })
        .collect::<Vec<_>>();

    window.set_console_asset_items(ModelRc::new(VecModel::from(rows)));
}

fn sync_workspace_tabs(window: &AppWindow, state: &ShellViewModel) {
    let tabs = state
        .workspace_tabs()
        .iter()
        .map(|tab| WorkspaceTabItem {
            session_id: tab.session_id.clone().into(),
            title: tab.title.clone().into(),
            subtitle: tab.subtitle.clone().into(),
            state: tab.state.clone().into(),
            active: tab.active,
        })
        .collect::<Vec<_>>();

    window.set_workspace_tab_items(ModelRc::new(VecModel::from(tabs)));
    window
        .set_active_workspace_session_id(state.active_workspace_session_id().unwrap_or("").into());
    window.set_workspace_session_host_mode(state.workspace_session_host_mode().into());

    if let Some(active_tab) = state.active_workspace_tab() {
        window.set_workspace_session_title(active_tab.title.clone().into());
        window.set_workspace_session_subtitle(active_tab.subtitle.clone().into());
        window.set_workspace_session_state(active_tab.state.clone().into());
        window.set_workspace_session_can_reconnect(active_tab.can_reconnect());
        window.set_workspace_session_screen_text(state.workspace_terminal_screen_text().into());
        window.set_workspace_session_surface_seqno(
            i32::try_from(state.workspace_terminal_surface_seqno()).unwrap_or(i32::MAX),
        );
    } else {
        window.set_workspace_session_title("".into());
        window.set_workspace_session_subtitle("".into());
        window.set_workspace_session_state("".into());
        window.set_workspace_session_can_reconnect(false);
        window.set_workspace_session_screen_text("".into());
        window.set_workspace_session_surface_seqno(0);
    }
}

fn sync_shell_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_top_status_bar_state(window, state, effects);
    sync_sidebar_state(window, state);
    sync_workspace_tabs(window, state);
    sync_assets_context_menu_state(window, state);
    sync_asset_modal_state(window, state);
    sync_ssh_host_key_modal_state(window, state);
}

fn sync_shell_layout(
    window: &AppWindow,
    state: &mut ShellViewModel,
    logical_width: u32,
    logical_height: u32,
) {
    // Rust owns the responsive policy so Slint can consume stable booleans instead of repeating
    // width-threshold logic in multiple components.
    let layout = resolve_shell_layout(ShellLayoutInput {
        window_width: logical_width.max(ShellMetrics::WINDOW_MIN_WIDTH),
        request_assets_sidebar: state.requested_assets_sidebar(),
        request_right_panel: state.requested_right_panel(),
    });

    window.set_effective_show_assets_sidebar(layout.show_assets_sidebar);
    window.set_effective_show_right_panel(layout.show_right_panel);
    window.set_shell_body_height_cache(
        logical_height.saturating_sub(ShellMetrics::TITLEBAR_HEIGHT) as f32,
    );
    update_context_menu_placement(window, state);
    sync_assets_context_menu_state(window, state);
}

fn current_window_size(window: &AppWindow) -> (u32, u32) {
    let size = window.window().size();
    (size.width, size.height)
}

#[cfg(target_os = "windows")]
const WINDOW_FRAME_RESERVED_RESIZE_BAND: i32 = 10;

#[cfg(target_os = "windows")]
fn install_windows_frame_adapter(window: &AppWindow) {
    use slint::winit_030::WinitWindowAccessor;

    // The native subclass needs the live maximize-button geometry from Slint so Windows snap
    // layouts still target the custom titlebar button.
    let placement = query_true_window_placement_from_app(window);
    let maximize_button = CaptionButtonGeometry {
        x: window.get_layout_titlebar_maximize_button_x() as i32,
        y: window.get_layout_titlebar_maximize_button_y() as i32,
        width: window.get_layout_titlebar_maximize_button_width() as i32,
        height: window.get_layout_titlebar_maximize_button_height() as i32,
    };

    let _ = window.window().with_winit_window(|winit_window| {
        install_window_frame_adapter(
            winit_window,
            maximize_button,
            placement,
            WINDOW_FRAME_RESERVED_RESIZE_BAND,
        );
    });
}

#[cfg(not(target_os = "windows"))]
fn install_windows_frame_adapter(_window: &AppWindow) {}

#[cfg(target_os = "windows")]
fn query_true_window_placement_from_app(window: &AppWindow) -> WindowPlacementKind {
    use slint::winit_030::WinitWindowAccessor;

    window
        .window()
        .with_winit_window(query_true_window_placement)
        .flatten()
        .unwrap_or(WindowPlacementKind::Unknown)
}

fn load_ui_preferences(store: &Option<Rc<UiPreferencesStore>>) -> UiPreferences {
    match store {
        Some(store) => match store.load_or_default() {
            Ok(prefs) => prefs,
            Err(err) => {
                tracing::error!(
                    target: "config.preferences",
                    error = %err,
                    "failed to load ui preferences"
                );
                UiPreferences::default()
            }
        },
        None => UiPreferences::default(),
    }
}

fn save_ui_preferences(store: &Option<Rc<UiPreferencesStore>>, state: &ShellViewModel) {
    if let Some(store) = store
        && let Err(err) = store.save(&UiPreferences::from(state))
    {
        tracing::error!(
            target: "config.preferences",
            error = %err,
            "failed to save ui preferences"
        );
    }
}

fn empty_asset_catalog() -> PersistedAssetCatalog {
    PersistedAssetCatalog {
        schema_version: ASSET_CATALOG_SCHEMA_VERSION,
        root_ids: Vec::new(),
        nodes: BTreeMap::new(),
    }
}

fn load_asset_catalog(repo: &dyn AssetCatalogRepository) -> PersistedAssetCatalog {
    match repo.load() {
        Ok(catalog) => catalog,
        Err(err) => {
            tracing::error!(
                target: "config.assets_catalog",
                error = %err,
                "failed to load asset catalog"
            );
            empty_asset_catalog()
        }
    }
}

fn save_asset_catalog(repo: &dyn AssetCatalogRepository, state: &ShellViewModel) -> Result<()> {
    let catalog = asset_tree_to_catalog(state.console_asset_tree());
    repo.save(&catalog)
}

fn save_asset_catalog_if_available(
    repo: &Option<Rc<dyn AssetCatalogRepository>>,
    state: &ShellViewModel,
) {
    if let Some(repo) = repo
        && let Err(err) = save_asset_catalog(repo.as_ref(), state)
    {
        tracing::error!(
            target: "config.assets_catalog",
            error = %err,
            "failed to save asset catalog"
        );
    }
}

fn asset_catalog_repository_for_app() -> Result<Rc<dyn AssetCatalogRepository>> {
    let project_dirs = ProjectDirs::from("dev", "MicaTerm", "MicaTerm")
        .context("project directories are unavailable")?;
    let executable_dir = std::env::current_exe()?
        .parent()
        .context("executable directory is unavailable")?
        .to_path_buf();
    let app_paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: std::env::var_os("MICA_TERM_APP_DIR").map(PathBuf::from),
        executable_dir,
        standard_local_data_dir: project_dirs.data_local_dir().join("MicaTerm"),
        portable_marker_name: ".mica-term-portable",
    })?;

    Ok(Rc::new(RedbAssetCatalogStore::new(app_paths.data_dir)))
}

pub fn bind_top_status_bar_with_store_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo(window, store, effects, None);
}

pub fn bind_top_status_bar_with_store_and_effects_and_asset_repo(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::mainline(),
        effects,
        asset_repo,
    );
}

pub fn bind_top_status_bar_with_store_and_profile_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    _profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
) {
    let (session_runtime_guard, session_bridge) = match AppAsyncRuntime::new() {
        Ok(runtime) => {
            let session_bridge = build_session_bridge(runtime.handle());
            (Some(runtime), Some(session_bridge))
        }
        Err(err) => {
            tracing::error!(
                target: "app.ssh",
                error = %err,
                "failed to create default app async runtime for shell services"
            );
            (None, None)
        }
    };
    bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
        window,
        store,
        _profile,
        effects,
        asset_repo,
        session_bridge,
        session_runtime_guard,
    );
}

fn bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    _profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    session_bridge: Option<Rc<ShellSessionBridge>>,
    session_runtime_guard: Option<AppAsyncRuntime>,
) {
    let store = store.map(Rc::new);
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    if let Some(repo) = asset_repo.as_ref() {
        initial_view_model
            .replace_console_asset_tree(catalog_to_asset_tree(&load_asset_catalog(repo.as_ref())));
    }
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let controller = Rc::new(WindowController::new(window));
    let modal_drag_state = Rc::new(RefCell::new(None::<ModalDragState>));

    apply_restored_window_size(window, default_window_size());
    bind_windows_window_state_tracking(window, Rc::clone(&view_model), Rc::clone(&effects));
    sync_shell_state(window, &view_model.borrow(), effects.as_ref());
    {
        let mut state = view_model.borrow_mut();
        sync_shell_layout(
            window,
            &mut state,
            ShellMetrics::WINDOW_DEFAULT_WIDTH,
            ShellMetrics::WINDOW_DEFAULT_HEIGHT,
        );
    }
    install_windows_frame_adapter(window);
    let session_projection_timer = Rc::new(Timer::default());
    if let Some(session_bridge_ref) = session_bridge.as_ref() {
        let state = Rc::clone(&view_model);
        let handle = window.as_weak();
        let manager = session_bridge_ref.manager.clone();
        session_projection_timer.start(
            TimerMode::Repeated,
            Duration::from_millis(50),
            move || {
                let Some(window) = handle.upgrade() else {
                    return;
                };
                let mut state = state.borrow_mut();
                if sync_workspace_projection_from_manager(&mut state, &manager) {
                    sync_workspace_tabs(&window, &state);
                    sync_assets_context_menu_state(&window, &state);
                }
            },
        );
    }

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_projection_timer_ref = Rc::clone(&session_projection_timer);
    window.on_toggle_right_panel_requested(move || {
        let _keep_session_projection_timer_alive = &session_projection_timer_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_right_panel();
        window.set_show_right_panel(state.show_right_panel);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_global_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_global_menu();
        window.set_show_global_menu(state.show_global_menu);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    let effects_ref = Rc::clone(&effects);
    window.on_toggle_theme_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_theme_mode();
        sync_theme_and_window_effects(&window, &state, effects_ref.as_ref());
        save_ui_preferences(&store_ref, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let store_ref = store.clone();
    window.on_toggle_window_always_on_top_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_always_on_top();
        window.set_is_window_always_on_top(state.is_always_on_top);
        save_ui_preferences(&store_ref, &state);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_minimize_requested(move || {
        controller_ref.minimize();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    let effects_ref = Rc::clone(&effects);
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

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_sidebar_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_assets_sidebar();
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_sidebar_destination_selected(move |destination_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        let destination = SidebarDestination::from_id(destination_id.as_str())
            .unwrap_or(SidebarDestination::Console);
        state.select_sidebar_destination(destination);
        sync_sidebar_state(&window, &state);
        let (width, height) = current_window_size(&window);
        sync_shell_layout(&window, &mut state, width, height);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.activate_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_search_query_changed(move |query| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.set_asset_search_query(query.to_string());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_collapse_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.collapse_asset_search_if_empty();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_view_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_view_mode();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_tree_expansion_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_tree_expansion();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_create_action_selected(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.handle_assets_create_action(action_id.as_str());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_close_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.cancel_asset_modal();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    window.on_confirm_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_rename_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_rename_asset_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    window.on_confirm_asset_rename_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    window.on_confirm_delete_asset_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let did_mutate = state.confirm_delete_asset();
        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_folder_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_new_folder_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_ssh_modal_tab_selected(move |tab| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_ssh_modal_tab(tab.as_str());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_ssh_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_ssh_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    window.on_asset_ssh_modal_action_requested(move |action| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let accepted = state.begin_ssh_modal_action(action.as_str());
        let pending_action = state.take_pending_ssh_modal_action();
        let mut did_mutate = false;

        if let Some(request) = pending_action {
            match request.action {
                SshModalAction::Save => {
                    did_mutate = state.confirm_asset_modal();
                    if !did_mutate {
                        state.finish_ssh_modal_action_error("Failed to save connection.");
                    }
                }
                SshModalAction::TestConnection => {
                    match ConnectionProfile::from_draft(&request.draft) {
                        Ok(profile) => {
                            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                                match session_bridge.manager.probe_connection(profile) {
                                    Ok(()) => state.finish_ssh_modal_action_success(
                                        "Connection test succeeded.",
                                    ),
                                    Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                                }
                            } else {
                                state.finish_ssh_modal_action_error(
                                    "SSH session bridge is unavailable.",
                                );
                            }
                        }
                        Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                    }
                }
                SshModalAction::Connect => match ConnectionProfile::from_draft(&request.draft) {
                    Ok(mut profile) => {
                        profile.asset_id =
                            Some(temporary_session_asset_id_for_draft(&request.draft));
                        if let Some(session_bridge) = session_bridge_ref.as_ref() {
                            if let Err(err) = open_session_with_profile(
                                &mut state,
                                session_bridge.as_ref(),
                                profile,
                                OpenSessionMode::ActivateExisting,
                            ) {
                                tracing::error!(
                                    target: "app.ssh",
                                    error = %err,
                                    "failed to open temporary ssh session from modal action"
                                );
                                state.finish_ssh_modal_action_error(err.to_string());
                            } else {
                                state.cancel_asset_modal();
                            }
                        } else {
                            state.finish_ssh_modal_action_error(
                                "SSH session bridge is unavailable.",
                            );
                        }
                    }
                    Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                },
                SshModalAction::SaveAndConnect => {
                    did_mutate = state.confirm_asset_modal();
                    if did_mutate {
                        let profile = state
                            .focused_asset_id
                            .clone()
                            .and_then(|asset_id| profile_for_saved_asset(&state, &asset_id));

                        if let Some(profile) = profile {
                            if let Some(session_bridge) = session_bridge_ref.as_ref()
                                && let Err(err) = open_session_with_profile(
                                    &mut state,
                                    session_bridge.as_ref(),
                                    profile,
                                    OpenSessionMode::ActivateExisting,
                                )
                            {
                                tracing::error!(
                                    target: "app.ssh",
                                    error = %err,
                                    "failed to open ssh session from modal action"
                                );
                            }
                        } else {
                            state.finish_ssh_modal_action_error(
                                "Failed to resolve saved connection profile.",
                            );
                        }
                    } else {
                        state.finish_ssh_modal_action_error(
                            "Failed to save connection before opening session.",
                        );
                    }
                }
            }
        } else if accepted {
            state.finish_ssh_modal_action_error("SSH modal action did not produce a request.");
        }

        if did_mutate {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs(&window, &state);
        sync_assets_context_menu_state(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_ssh_host_key_modal_accept_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.accept_ssh_host_key_prompt();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_ssh_host_key_modal_reject_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.reject_ssh_host_key_prompt();
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_ssh_host_key_modal_state(&window, &state);
    });

    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
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
    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_drag_moved(move |pointer_x, pointer_y| {
        let Some(drag_state) = *modal_drag_state_ref.borrow() else {
            return;
        };
        let window = handle.unwrap();
        let next_offset = update_modal_drag(drag_state, pointer_x, pointer_y);
        window.set_blocking_modal_offset_x(next_offset.x);
        window.set_blocking_modal_offset_y(next_offset.y);
    });

    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_drag_ended(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let modal_drag_state_ref = Rc::clone(&modal_drag_state);
    window.on_blocking_modal_focus_restore_requested(move || {
        modal_drag_state_ref.borrow_mut().take();
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    window.on_workspace_tab_selected(move |session_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.activate_workspace_session(session_id.as_str()) {
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
            }
            sync_workspace_tabs(&window, &state);
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    window.on_workspace_tab_close_requested(move |session_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if close_session_by_id(
            &mut state,
            session_bridge_ref.as_deref(),
            session_id.as_str(),
        ) {
            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                let _ = sync_workspace_projection_from_manager(&mut state, &session_bridge.manager);
            }
            sync_workspace_tabs(&window, &state);
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    window.on_asset_selected(move |item_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_asset(item_id.as_str());
        if let Some(profile) = profile_for_saved_asset(&state, item_id.as_str())
            && let Some(session_bridge) = session_bridge_ref.as_ref()
            && let Err(err) = open_session_with_profile(
                &mut state,
                session_bridge.as_ref(),
                profile,
                OpenSessionMode::ActivateExisting,
            )
        {
            tracing::error!(
                target: "app.ssh",
                asset_id = item_id.as_str(),
                error = %err,
                "failed to open ssh session for selected asset"
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs(&window, &state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_expanded_requested(move |item_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_folder_expanded(item_id.as_str());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_context_menu_requested(move |target_id, target_kind, anchor_x, anchor_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.open_context_menu_for_target(
            parse_context_target_kind(target_kind.as_str()),
            if target_id.is_empty() {
                None
            } else {
                Some(target_id.to_string())
            },
            anchor_x,
            anchor_y,
        );
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_interaction_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.dismiss_empty_asset_search_on_shell_interaction() {
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    window.on_assets_context_menu_action_invoked(move |action_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();

        if let Some((path, action)) = context_menu_action_entry_for(&state, action_id.as_str()) {
            if !action.children.is_empty() {
                state.set_context_menu_open_path(path);
            } else if action.state == ContextMenuActionState::Enabled {
                match action_id.as_str() {
                    "open-in-new-tab" => {
                        let target_asset_id = state.context_target_asset_id.clone();
                        state.close_context_menu();
                        if let Some(asset_id) = target_asset_id
                            && let Some(profile) = profile_for_saved_asset(&state, &asset_id)
                            && let Some(session_bridge) = session_bridge_ref.as_ref()
                            && let Err(err) = open_session_with_profile(
                                &mut state,
                                session_bridge.as_ref(),
                                profile,
                                OpenSessionMode::ForceNewTab,
                            )
                        {
                            tracing::error!(
                                target: "app.ssh",
                                asset_id = asset_id.as_str(),
                                error = %err,
                                "failed to open ssh session in a new tab"
                            );
                        }
                    }
                    "close-connection" => {
                        let target_asset_id = state.context_target_asset_id.clone();
                        state.close_context_menu();
                        if let Some(asset_id) = target_asset_id
                            && let Some(session_bridge) = session_bridge_ref.as_ref()
                        {
                            let _ =
                                disconnect_session_for_asset(&mut state, session_bridge, &asset_id);
                        }
                    }
                    _ => state.handle_context_menu_leaf_action(action_id.as_str()),
                }
            } else {
                state.handle_context_menu_leaf_action(action_id.as_str());
            }
        }

        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs(&window, &state);
        sync_asset_modal_state(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_key_pressed(move |command| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();

        match command.as_str() {
            "escape" => state.handle_context_menu_escape(),
            "left" => state.navigate_context_menu_left(),
            "right" => state.navigate_context_menu_right(),
            "enter" => state.invoke_current_context_menu_item(),
            _ => {}
        }

        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_row_hovered(move |column_index, row_index| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let next_path =
            context_menu_hover_path_for(&state, column_index as usize, row_index as usize);
        state.hover_context_menu_path(next_path);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_pointer_moved(move |pointer_x, pointer_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if !state.context_menu_open {
            return;
        }

        let pointer = (pointer_x, pointer_y);
        let rects = context_menu_column_rects_for(&state);
        let original_path = state.context_menu_open_path.clone();

        if state.context_menu_open_path.len() >= 2
            && let (Some(parent_rect), Some(child_rect)) = (rects[1], rects[2])
            && !should_keep_corridor_open(pointer, parent_rect, child_rect)
        {
            state.truncate_context_menu_open_path(1);
        }

        if !state.context_menu_open_path.is_empty()
            && let (Some(parent_rect), Some(child_rect)) = (rects[0], rects[1])
        {
            let keep_open = should_keep_corridor_open(pointer, parent_rect, child_rect);
            if !keep_open {
                state.truncate_context_menu_open_path(0);
            }
        }

        if state.context_menu_open_path != original_path {
            update_context_menu_placement(&window, &mut state);
            sync_assets_context_menu_state(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_close_assets_context_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_context_menu();
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_shell_layout_invalidated(move |width, height| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        sync_shell_layout(&window, &mut state, width as u32, height as u32);
        install_windows_frame_adapter(&window);
    });

    let controller_ref = Rc::clone(&controller);
    window.on_close_requested(move || {
        let _ = controller_ref.close();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_requested(move || {
        let _ = controller_ref.drag();
    });

    let controller_ref = Rc::clone(&controller);
    window.on_drag_resize_requested(move |direction| {
        if let Some(direction) = parse_resize_direction(direction.as_str()) {
            let _ = controller_ref.drag_resize(direction);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    let controller_ref = Rc::clone(&controller);
    let effects_ref = Rc::clone(&effects);
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
        sync_top_status_bar_state(&window, &state, effects_ref.as_ref());
    });
}

pub fn bind_top_status_bar_with_store(window: &AppWindow, store: Option<UiPreferencesStore>) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::mainline(),
        default_platform_window_effects(),
        None,
    );
}

pub fn bind_top_status_bar_with_profile(window: &AppWindow, profile: AppRuntimeProfile) {
    bind_top_status_bar_with_profile_and_async_handle(window, profile, None);
}

fn bind_top_status_bar_with_profile_and_async_handle(
    window: &AppWindow,
    profile: AppRuntimeProfile,
    async_runtime_handle: Option<tokio::runtime::Handle>,
) {
    let store = match UiPreferencesStore::for_app() {
        Ok(store) => Some(store),
        Err(err) => {
            tracing::error!(
                target: "config.preferences",
                error = %err,
                "failed to resolve ui preferences store"
            );
            None
        }
    };
    let asset_repo = match asset_catalog_repository_for_app() {
        Ok(repo) => Some(repo),
        Err(err) => {
            tracing::error!(
                target: "config.assets_catalog",
                error = %err,
                "failed to resolve asset catalog repository"
            );
            None
        }
    };

    let effects = default_platform_window_effects();
    match async_runtime_handle {
        Some(async_runtime_handle) => {
            let session_bridge = build_session_bridge(async_runtime_handle);
            bind_top_status_bar_with_store_and_profile_and_effects_and_session_bridge(
                window,
                store,
                profile,
                effects,
                asset_repo,
                Some(session_bridge),
                None,
            );
        }
        None => {
            bind_top_status_bar_with_store_and_profile_and_effects(
                window, store, profile, effects, asset_repo,
            );
        }
    }
}

pub fn bind_top_status_bar(window: &AppWindow) {
    bind_top_status_bar_with_profile(window, AppRuntimeProfile::mainline());
}

pub fn run() -> Result<()> {
    let async_runtime = AppAsyncRuntime::new()?;
    run_with_profile(AppRuntimeProfile::mainline(), async_runtime.handle())
}

pub fn run_with_profile(
    profile: AppRuntimeProfile,
    async_runtime_handle: tokio::runtime::Handle,
) -> Result<()> {
    let window = AppWindow::new()?;
    window.set_window_title(runtime_window_title(profile).into());
    bind_top_status_bar_with_profile_and_async_handle(&window, profile, Some(async_runtime_handle));
    window.run()?;
    Ok(())
}
