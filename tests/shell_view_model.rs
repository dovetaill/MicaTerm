use mica_term::app::window_state::WindowPlacementKind;
use mica_term::shell::assets::{AssetViewMode, ConsoleAssetKind};
use mica_term::shell::context_menu::ContextTargetKind;
use mica_term::shell::sidebar::SidebarDestination;
use mica_term::shell::view_model::{ShellViewModel, WelcomeAction, welcome_actions};
use mica_term::theme::ThemeMode;

#[test]
fn welcome_actions_match_the_approved_order() {
    assert_eq!(
        welcome_actions(),
        &[
            WelcomeAction::NewConnection,
            WelcomeAction::OpenRecent,
            WelcomeAction::Snippets,
            WelcomeAction::Sftp,
        ]
    );
}

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
fn blank_area_click_commits_rename_and_clears_selection_and_focus() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.update_active_asset_rename_draft("Infra".into());

    view_model.handle_blank_area_click();

    assert!(view_model.selected_asset_ids.is_empty());
    assert_eq!(view_model.focused_asset_id, None);
    assert_eq!(view_model.editing_asset_id, None);
    assert_eq!(view_model.visible_console_asset_rows()[0].label, "Infra");
}

#[test]
fn selecting_an_asset_updates_focus_without_opening_context_menu() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.commit_active_asset_rename();
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.select_asset(&asset_id);

    assert_eq!(view_model.focused_asset_id.as_deref(), Some(asset_id.as_str()));
    assert_eq!(view_model.selected_asset_ids, vec![asset_id]);
    assert!(!view_model.asset_create_menu_open);
}

#[test]
fn folder_context_create_inserts_child_and_expands_parent() {
    let mut view_model = ShellViewModel::default();
    view_model.handle_assets_create_action("new-folder");
    view_model.commit_active_asset_rename();
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(folder_id.clone()),
        48.0,
        64.0,
    );
    view_model.handle_context_menu_leaf_action("new-ssh-connection");

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, folder_id);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].kind, ConsoleAssetKind::SshConnection);
}
