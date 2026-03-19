//! Stateful shell view-model coverage for toolbar, sidebar, window toggles, and asset explorer state.

use mica_term::app::window_state::WindowPlacementKind;
use mica_term::shell::assets::{AssetViewMode, ConsoleAssetKind};
use mica_term::shell::context_menu::{
    ContextTargetKind, SelectionContext, resolve_action_tree, visible_columns_for_path,
};
use mica_term::shell::sidebar::SidebarDestination;
use mica_term::shell::view_model::{
    AssetModalState, AssetSshModalTab, ShellViewModel, WelcomeAction, welcome_actions,
};
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
fn shell_view_model_starts_with_context_menu_closed() {
    let view_model = ShellViewModel::default();

    assert!(!view_model.context_menu_open);
    assert_eq!(view_model.context_menu_target_kind, None);
    assert!(view_model.context_menu_open_path.is_empty());
    assert!(view_model.context_menu_feedback_text.is_empty());
}

#[test]
fn opening_new_folder_modal_does_not_insert_placeholder_row() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_folder_modal(None);

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(view_model.asset_modal_state.is_some());
}

#[test]
fn opening_new_folder_modal_commits_active_rename_and_clears_editing_state() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Folder 1".into());
    view_model.update_active_asset_rename_draft("Prod".into());

    view_model.open_new_folder_modal(None);

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows[0].id, asset_id);
    assert_eq!(rows[0].label, "Prod");
    assert_eq!(view_model.editing_asset_id, None);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewFolder {
            parent_id: None,
            ref draft_name,
        }) if draft_name.is_empty()
    ));
}

#[test]
fn toolbar_create_action_opens_root_folder_modal_and_closes_overlay_state() {
    let mut view_model = ShellViewModel::default();
    let folder_id = {
        view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
        view_model.visible_console_asset_rows()[0].id.clone()
    };
    view_model.toggle_asset_create_menu();
    view_model.open_context_menu_for_target(ContextTargetKind::Folder, Some(folder_id), 48.0, 64.0);

    view_model.handle_assets_create_action("new-folder");

    assert!(!view_model.asset_create_menu_open);
    assert!(!view_model.context_menu_open);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewFolder {
            parent_id: None,
            ref draft_name,
        }) if draft_name.is_empty()
    ));
    assert_eq!(view_model.context_target_asset_id, None);
}

#[test]
fn confirming_new_folder_modal_inserts_root_node_and_selects_it() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_folder_modal(None);
    view_model.update_new_folder_modal_name("Infra".into());

    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "Infra");
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(rows[0].id.as_str())
    );
    assert!(view_model.asset_modal_state.is_none());
}

#[test]
fn folder_targeted_create_modal_auto_expands_parent_on_confirm() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_new_folder_modal(Some(folder_id.clone()));
    view_model.update_new_folder_modal_name("Bastions".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, folder_id);
    assert_eq!(rows[1].label, "Bastions");
    assert_eq!(rows[1].depth, 1);
}

#[test]
fn opening_new_ssh_modal_does_not_insert_placeholder_row() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { .. })
    ));
}

#[test]
fn opening_new_ssh_modal_commits_active_rename_and_clears_editing_state() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Folder 1".into());
    view_model.update_active_asset_rename_draft("Prod".into());

    view_model.open_new_ssh_modal(None);

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows[0].id, asset_id);
    assert_eq!(rows[0].label, "Prod");
    assert_eq!(view_model.editing_asset_id, None);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            parent_id: None,
            active_tab: AssetSshModalTab::Standard,
            ref draft,
        }) if draft.name.is_empty()
            && draft.host.is_empty()
            && draft.port == "22"
    ));
}

#[test]
fn confirming_new_ssh_modal_requires_name_and_host() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    assert!(view_model.can_confirm_asset_modal());
}

#[test]
fn cancelling_child_targeted_modal_clears_stale_context_target() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_new_ssh_modal(Some(folder_id.clone()));
    assert_eq!(
        view_model.context_target_asset_id.as_deref(),
        Some(folder_id.as_str())
    );

    view_model.cancel_asset_modal();

    assert!(view_model.asset_modal_state.is_none());
    assert_eq!(view_model.context_target_asset_id, None);
}

#[test]
fn stale_missing_folder_context_target_falls_back_to_root_ssh_modal() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some("missing-folder".into()),
        48.0,
        64.0,
    );

    view_model.handle_context_menu_leaf_action("new-ssh-connection");

    assert!(!view_model.context_menu_open);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            parent_id: None,
            active_tab: AssetSshModalTab::Standard,
            ..
        })
    ));
    assert_eq!(view_model.context_target_asset_id, None);
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
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some("ssh-prod-01")
    );
    assert!(view_model.context_menu_open_path.is_empty());
}

#[test]
fn selecting_primary_leaf_path_keeps_blank_area_menu_flat() {
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
    let new_folder_index = roots
        .iter()
        .position(|node| node.id == "new-folder")
        .expect("blank-area menu should expose the new-folder row");

    view_model.set_context_menu_open_path(vec![new_folder_index]);

    let columns = visible_columns_for_path(&roots, &view_model.context_menu_open_path);
    assert!(columns[1].is_empty());
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

#[test]
fn confirming_duplicate_default_ssh_name_uses_first_missing_positive_index() {
    let mut view_model = ShellViewModel::default();

    view_model.handle_assets_create_action("new-ssh-connection");
    view_model.update_ssh_modal_name("SSH Connection 1".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.confirm_asset_modal();
    view_model.handle_assets_create_action("new-ssh-connection");
    view_model.update_ssh_modal_name("SSH Connection 1".into());
    view_model.update_ssh_modal_host("10.0.0.13".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].kind, ConsoleAssetKind::SshConnection);
    assert_eq!(rows[0].label, "SSH Connection 1");
    assert_eq!(rows[1].label, "SSH Connection 2");
}

#[test]
fn context_menu_create_action_opens_folder_modal_and_closes_menu() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(ContextTargetKind::BlankArea, None, 32.0, 48.0);

    view_model.handle_context_menu_leaf_action("new-folder");

    assert!(!view_model.context_menu_open);
    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewFolder { .. })
    ));
}

#[test]
fn confirming_duplicate_default_folder_name_uses_smallest_missing_positive_index() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 2");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 3");

    view_model.handle_assets_create_action("new-folder");
    view_model.update_new_folder_modal_name("Folder 2".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].label, "Folder 1");
}

#[test]
fn dismissing_active_rename_commits_current_draft() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.begin_asset_rename_session(asset_id, "Folder 1".into());
    view_model.update_active_asset_rename_draft("Prod".into());

    view_model.dismiss_active_asset_rename();

    assert_eq!(view_model.visible_console_asset_rows()[0].label, "Prod");
    assert_eq!(view_model.editing_asset_id, None);
}

#[test]
fn renaming_to_existing_same_type_default_name_uses_next_missing_index() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 2");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Infra");
    let asset_id = view_model.visible_console_asset_rows()[2].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Infra".into());

    view_model.update_asset_rename_draft(&asset_id, "Folder 1".into());
    view_model.commit_asset_rename(&asset_id, "Folder 1".into());

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows[2].label, "Folder 3");
}

#[test]
fn blank_rename_fallback_ignores_non_strict_numbered_labels() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 01");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 2");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let asset_id = view_model.visible_console_asset_rows()[2].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Prod".into());

    view_model.update_asset_rename_draft(&asset_id, "   ".into());
    view_model.commit_asset_rename(&asset_id, "   ".into());

    let row = view_model
        .visible_console_asset_rows()
        .into_iter()
        .find(|row| row.id == asset_id)
        .unwrap();
    assert_eq!(row.label, "Folder 1");
}

#[test]
fn renaming_to_existing_custom_name_uses_smallest_missing_numeric_suffix() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod 1");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod 3");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Infra");
    let asset_id = view_model.visible_console_asset_rows()[3].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Infra".into());

    view_model.update_asset_rename_draft(&asset_id, "Prod".into());
    view_model.commit_asset_rename(&asset_id, "Prod".into());

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows[3].label, "Prod 2");
}

#[test]
fn cancelling_inline_rename_keeps_default_label_and_exits_editing() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.begin_asset_rename_session(asset_id, "Folder 1".into());
    view_model.update_active_asset_rename_draft("Prod".into());

    view_model.cancel_active_asset_rename();

    assert_eq!(view_model.visible_console_asset_rows()[0].label, "Folder 1");
    assert_eq!(view_model.editing_asset_id, None);
}

#[test]
fn blank_area_click_commits_rename_and_clears_selection_and_focus() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.begin_asset_rename_session(asset_id, "Folder 1".into());
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
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.select_asset(&asset_id);

    assert_eq!(view_model.focused_asset_id.as_deref(), Some(asset_id.as_str()));
    assert_eq!(view_model.selected_asset_ids, vec![asset_id]);
    assert!(!view_model.asset_create_menu_open);
}

#[test]
fn folder_context_create_confirm_auto_expands_parent_and_projects_child_row() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(folder_id.clone()),
        48.0,
        64.0,
    );
    view_model.handle_context_menu_leaf_action("new-ssh-connection");
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { .. })
    ));
    assert_eq!(view_model.visible_console_asset_rows().len(), 1);

    view_model.update_ssh_modal_name("SSH Connection 1".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, folder_id);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].kind, ConsoleAssetKind::SshConnection);
}
