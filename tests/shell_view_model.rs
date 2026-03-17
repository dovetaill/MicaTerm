//! Stateful shell view-model coverage for toolbar, sidebar, and window toggles.

use mica_term::app::window_state::WindowPlacementKind;
use mica_term::shell::assets::AssetViewMode;
use mica_term::shell::context_menu::{ContextTargetKind, SelectionContext, resolve_action_tree, visible_columns_for_path};
use mica_term::shell::sidebar::SidebarDestination;
use mica_term::shell::view_model::ShellViewModel;
use mica_term::theme::ThemeMode;

#[test]
fn shell_view_model_starts_in_welcome_mode_with_right_panel_hidden() {
    let view_model = ShellViewModel::default();
    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
    assert!(view_model.show_assets_sidebar);
    assert_eq!(
        view_model.active_sidebar_destination,
        SidebarDestination::Console
    );
}

#[test]
fn shell_view_model_tracks_top_status_bar_state() {
    let mut view_model = ShellViewModel::default();

    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
    assert!(!view_model.show_global_menu);
    assert!(!view_model.is_window_maximized());
    assert!(view_model.is_window_active);

    view_model.toggle_right_panel();
    assert!(view_model.show_right_panel);

    view_model.toggle_global_menu();
    assert!(view_model.show_global_menu);

    view_model.close_global_menu();
    assert!(!view_model.show_global_menu);

    view_model.set_window_placement(WindowPlacementKind::Maximized);
    assert!(view_model.is_window_maximized());

    view_model.set_window_active(false);
    assert!(!view_model.is_window_active);
}

#[test]
fn shell_view_model_tracks_window_placement_without_chrome_state() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.window_placement(), WindowPlacementKind::Restored);
    assert!(!view_model.is_window_maximized());

    view_model.set_window_placement(WindowPlacementKind::Maximized);
    assert_eq!(
        view_model.window_placement(),
        WindowPlacementKind::Maximized
    );
    assert!(view_model.is_window_maximized());
}

#[test]
fn shell_view_model_tracks_titlebar_theme_and_pin_state() {
    let mut view_model = ShellViewModel::default();

    assert_eq!(view_model.theme_mode, ThemeMode::Dark);
    assert!(!view_model.is_always_on_top);

    view_model.toggle_theme_mode();
    assert_eq!(view_model.theme_mode, ThemeMode::Light);

    view_model.toggle_always_on_top();
    assert!(view_model.is_always_on_top);
}

#[test]
fn shell_view_model_starts_with_assets_toolbar_defaults() {
    let view_model = ShellViewModel::default();

    assert_eq!(view_model.asset_view_mode, AssetViewMode::Tree);
    assert!(!view_model.asset_search_expanded);
    assert!(view_model.asset_search_query.is_empty());
    assert!(!view_model.asset_create_menu_open);
}

#[test]
fn shell_view_model_starts_with_context_menu_closed() {
    let view_model = ShellViewModel::default();

    assert!(!view_model.context_menu_open);
    assert_eq!(view_model.context_menu_target_kind, None);
    assert!(view_model.context_menu_open_path.is_empty());
    assert!(view_model.context_menu_feedback_text.is_empty());
}

#[test]
fn opening_context_menu_tracks_target_anchor_and_resets_open_path() {
    let mut view_model = ShellViewModel::default();
    view_model.context_menu_open_path = vec![1, 2];

    view_model.open_context_menu_for_target(
        ContextTargetKind::SshConnection,
        Some("ssh-prod-01".into()),
        128.0,
        256.0,
    );

    assert!(view_model.context_menu_open);
    assert_eq!(
        view_model.context_menu_target_kind,
        Some(ContextTargetKind::SshConnection)
    );
    assert_eq!(view_model.context_menu_anchor_x, 128.0);
    assert_eq!(view_model.context_menu_anchor_y, 256.0);
    assert_eq!(view_model.selected_asset_ids, vec!["ssh-prod-01"]);
    assert!(view_model.context_menu_open_path.is_empty());
}

#[test]
fn selecting_submenu_path_updates_visible_columns() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(ContextTargetKind::BlankArea, None, 24.0, 36.0);

    let roots = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            selected_ids: view_model.selected_asset_ids.clone(),
            clipboard_has_asset_payload: true,
            target_mutable: true,
            target_has_active_connection: false,
        },
    );
    let new_connection_index = roots
        .iter()
        .position(|node| node.id == "new-connection")
        .expect("blank-area menu should expose the new-connection submenu");

    view_model.set_context_menu_open_path(vec![new_connection_index]);

    let columns = visible_columns_for_path(&roots, &view_model.context_menu_open_path);
    let secondary_ids: Vec<_> = columns[1].iter().map(|node| node.id).collect();

    assert_eq!(
        secondary_ids,
        vec!["ssh", "local-terminal", "serial", "telnet", "ssh-tunnel"]
    );
}

#[test]
fn closing_context_menu_clears_open_path_but_keeps_selection() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some("folder-favorites".into()),
        72.0,
        108.0,
    );
    view_model.set_context_menu_open_path(vec![1]);

    view_model.close_context_menu();

    assert!(!view_model.context_menu_open);
    assert!(view_model.context_menu_open_path.is_empty());
    assert_eq!(view_model.selected_asset_ids, vec!["folder-favorites"]);
}
