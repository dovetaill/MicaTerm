//! Wires the Slint window to runtime state, persisted preferences, and native window hooks during startup.

use std::cell::RefCell;
use std::rc::Rc;

use anyhow::Result;
use slint::{ComponentHandle, ModelRc, VecModel};

use crate::AppWindow;
use crate::AssetsContextMenuItem;
use crate::ConsoleAssetItem;
use crate::app::runtime_profile::AppRuntimeProfile;
use crate::app::ui_preferences::{UiPreferences, UiPreferencesStore};
use crate::app::window_effects::{
    PlatformWindowEffects, build_native_window_appearance_request, default_platform_window_effects,
};
use crate::app::window_state::WindowPlacementKind;
use crate::app::windowing::{
    WindowController, apply_restored_window_size, parse_resize_direction, window_appearance,
};
#[cfg(target_os = "windows")]
use crate::app::windows_frame::{
    CaptionButtonGeometry, install_window_frame_adapter, query_true_window_placement,
};
use crate::shell::context_menu::{
    CONTEXT_MENU_COLUMN_GAP, CONTEXT_MENU_COLUMN_WIDTH, ContextMenuActionNode,
    ContextMenuActionState, ContextTargetKind, MenuPlacementInput, Rect, SelectionContext,
    context_menu_column_height, context_menu_column_offset, resolve_action_tree,
    resolve_root_menu_origin, should_keep_corridor_open, visible_columns_for_path,
};
use crate::shell::layout::{ShellLayoutInput, resolve_shell_layout};
use crate::shell::metrics::ShellMetrics;
use crate::shell::sidebar::{SidebarDestination, sidebar_items_for, toolbar_descriptor_for};
use crate::shell::view_model::{AssetModalState, ShellViewModel};
use crate::theme::ThemeMode;

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
    window.set_asset_rename_active(state.editing_asset_id.is_some());
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
            window.set_asset_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
        Some(AssetModalState::NewSshConnection {
            active_tab, draft, ..
        }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-ssh-connection".into());
            window.set_asset_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_ssh_modal_active_tab(active_tab.id().into());
            window.set_asset_ssh_modal_name(draft.name.clone().into());
            window.set_asset_ssh_modal_host(draft.host.clone().into());
            window.set_asset_ssh_modal_user(draft.user.clone().into());
            window.set_asset_ssh_modal_port(draft.port.clone().into());
            window.set_asset_ssh_modal_environment(draft.environment.clone().into());
            window.set_asset_ssh_modal_proxy_method(draft.proxy_method.clone().into());
        }
        None => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_folder_modal_name("".into());
            window.set_asset_ssh_modal_active_tab("standard".into());
            window.set_asset_ssh_modal_name("".into());
            window.set_asset_ssh_modal_host("".into());
            window.set_asset_ssh_modal_user("".into());
            window.set_asset_ssh_modal_port("22".into());
            window.set_asset_ssh_modal_environment("".into());
            window.set_asset_ssh_modal_proxy_method("".into());
        }
    }
}

fn schedule_asset_modal_focus(window: &AppWindow) {
    let handle = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        let window = handle.unwrap();
        if window.get_asset_modal_open() {
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

fn selection_context_for(state: &ShellViewModel) -> SelectionContext {
    SelectionContext {
        selected_ids: state.selected_asset_ids.clone(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        target_has_active_connection: true,
    }
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
    items.into_iter()
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
    context_menu_items_to_model(columns[0].clone(), state.context_menu_open_path.first().copied())
}

fn context_menu_secondary_items_for(state: &ShellViewModel) -> Vec<AssetsContextMenuItem> {
    let columns = context_menu_columns_for(state);
    context_menu_items_to_model(columns[1].clone(), state.context_menu_open_path.get(1).copied())
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
            renaming: state.editing_asset_id.as_deref() == Some(row.id.as_str()),
            rename_text: if state.editing_asset_id.as_deref() == Some(row.id.as_str()) {
                state.editing_asset_text.clone().into()
            } else {
                "".into()
            },
            path_hint: row.path_hint.clone().unwrap_or_default().into(),
            show_disclosure: row.show_disclosure,
            compact_flat_mode: state.asset_view_mode.id() == "flat",
        })
        .collect::<Vec<_>>();

    window.set_console_asset_items(ModelRc::new(VecModel::from(rows)));
}

fn sync_shell_state(
    window: &AppWindow,
    state: &ShellViewModel,
    effects: &dyn PlatformWindowEffects,
) {
    sync_top_status_bar_state(window, state, effects);
    sync_sidebar_state(window, state);
    sync_assets_context_menu_state(window, state);
    sync_asset_modal_state(window, state);
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

pub fn bind_top_status_bar_with_store_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        AppRuntimeProfile::mainline(),
        effects,
    );
}

pub fn bind_top_status_bar_with_store_and_profile_and_effects(
    window: &AppWindow,
    store: Option<UiPreferencesStore>,
    _profile: AppRuntimeProfile,
    effects: Rc<dyn PlatformWindowEffects>,
) {
    let store = store.map(Rc::new);
    let prefs = load_ui_preferences(&store);
    let mut initial_view_model = ShellViewModel::default();
    initial_view_model.theme_mode = prefs.theme_mode;
    initial_view_model.is_always_on_top = prefs.always_on_top;
    let view_model = Rc::new(RefCell::new(initial_view_model));
    let controller = Rc::new(WindowController::new(window));

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

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_right_panel_requested(move || {
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
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
        state.close_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_collapse_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_active_asset_rename();
        state.collapse_asset_search_if_empty();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_view_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
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
        state.dismiss_active_asset_rename();
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
    window.on_close_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.cancel_asset_modal();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_confirm_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.confirm_asset_modal();
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
    window.on_asset_rename_text_changed(move |asset_id, text| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_asset_rename_draft(asset_id.as_str(), text.to_string());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_rename_commit_requested(move |asset_id, text| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.commit_asset_rename(asset_id.as_str(), text.to_string());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_rename_cancel_requested(move |asset_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.cancel_asset_rename(asset_id.as_str());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_asset_selected(move |item_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.select_asset(item_id.as_str());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
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
        state.dismiss_active_asset_rename();
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
        if state.editing_asset_id.is_some() {
            state.dismiss_active_asset_rename();
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        } else if state.dismiss_empty_asset_search_on_shell_interaction() {
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_dismiss_active_asset_rename_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.editing_asset_id.is_some() {
            state.dismiss_active_asset_rename();
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        }
    });

    let state = Rc::clone(&view_model);
    let handle = window.as_weak();
    window.on_assets_context_menu_action_invoked(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();

        if let Some((path, action)) = context_menu_action_entry_for(&state, action_id.as_str()) {
            if !action.children.is_empty() {
                state.set_context_menu_open_path(path);
            } else {
                state.handle_context_menu_leaf_action(action_id.as_str());
            }
        }

        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
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
    );
}

pub fn bind_top_status_bar_with_profile(window: &AppWindow, profile: AppRuntimeProfile) {
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

    bind_top_status_bar_with_store_and_profile_and_effects(
        window,
        store,
        profile,
        default_platform_window_effects(),
    );
}

pub fn bind_top_status_bar(window: &AppWindow) {
    bind_top_status_bar_with_profile(window, AppRuntimeProfile::mainline());
}

pub fn run() -> Result<()> {
    run_with_profile(AppRuntimeProfile::mainline())
}

pub fn run_with_profile(profile: AppRuntimeProfile) -> Result<()> {
    let window = AppWindow::new()?;
    window.set_window_title(runtime_window_title(profile).into());
    bind_top_status_bar_with_profile(&window, profile);
    window.run()?;
    Ok(())
}
