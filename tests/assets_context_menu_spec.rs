use mica_term::shell::assets::{AssetTree, ConsoleAssetKind};
use mica_term::shell::context_menu::{
    ContextMenuActionState, ContextTargetKind, MenuPlacementInput, Rect, SelectionContext,
    context_menu_column_height, resolve_action_tree, resolve_root_menu_origin,
    should_keep_corridor_open, visible_columns_for_path,
};
use mica_term::shell::view_model::ShellViewModel;

fn blank_selection() -> SelectionContext {
    SelectionContext {
        selected_ids: Vec::new(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    }
}

#[test]
fn blank_area_scene_only_exposes_minimal_create_actions() {
    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &blank_selection());
    let ids: Vec<_> = roots.iter().map(|node| node.id).collect();

    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
}

#[test]
fn snippets_blank_area_scene_only_exposes_minimal_create_actions() {
    let roots = resolve_action_tree(ContextTargetKind::SnippetsBlankArea, &blank_selection());
    let ids: Vec<_> = roots.iter().map(|node| node.id).collect();

    assert_eq!(ids, vec!["new-snippet", "new-package"]);
}

#[test]
fn blank_area_actions_expose_label_and_icon_metadata() {
    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &blank_selection());

    assert_eq!(roots[0].label, "New Folder");
    assert_eq!(roots[0].icon_id, "folder");
    assert_eq!(roots[1].label, "New SSH Connection");
    assert_eq!(roots[1].icon_id, "window-console");
}

#[test]
fn blank_area_menu_height_is_compact() {
    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &blank_selection());

    let height = context_menu_column_height(&roots);
    assert!(height < 160.0);
}

#[test]
fn blank_area_scene_omits_paste_and_other_legacy_actions() {
    let ids: Vec<_> = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            clipboard_has_asset_payload: true,
            ..blank_selection()
        },
    )
    .into_iter()
    .map(|node| node.id)
    .collect();

    assert!(!ids.contains(&"new-connection"));
    assert!(!ids.contains(&"paste-asset"));
    assert!(!ids.contains(&"batch-open"));
}

#[test]
fn ssh_context_menu_exposes_only_one_open_action() {
    let selection = SelectionContext {
        selected_ids: vec!["ssh-prod-01".into()],
        clipboard_has_asset_payload: true,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    };

    let ids: Vec<_> = resolve_action_tree(ContextTargetKind::SshConnection, &selection)
        .into_iter()
        .map(|node| node.id)
        .collect();

    assert!(ids.contains(&"open-connection"));
    assert!(!ids.contains(&"open-in-new-tab"));
    assert!(!ids.contains(&"close-connection"));
}

#[test]
fn blank_area_visible_columns_stay_flat_for_primary_leaf_selection() {
    let roots = resolve_action_tree(
        ContextTargetKind::BlankArea,
        &SelectionContext {
            clipboard_has_asset_payload: true,
            ..blank_selection()
        },
    );
    let open_index = roots
        .iter()
        .position(|node| node.id == "new-folder")
        .expect("blank-area menu should expose the new-folder row");

    let columns = visible_columns_for_path(&roots, &[open_index]);
    assert_eq!(columns[0].len(), 2);
    assert!(columns[1].is_empty());
    assert!(columns[2].is_empty());
}

#[test]
fn ssh_scene_exposes_flat_create_actions_without_connection_submenu() {
    let selection = SelectionContext {
        selected_ids: vec!["ssh-prod-01".into()],
        clipboard_has_asset_payload: true,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    };

    let ids: Vec<_> = resolve_action_tree(ContextTargetKind::SshConnection, &selection)
        .into_iter()
        .map(|node| node.id)
        .collect();

    assert!(ids.contains(&"new-folder"));
    assert!(ids.contains(&"new-ssh-connection"));
    assert!(!ids.contains(&"new-connection"));
}

#[test]
fn ssh_scene_marks_proxy_chrome_as_planned_but_clickable() {
    let selection = SelectionContext {
        selected_ids: vec!["ssh-prod-01".into()],
        clipboard_has_asset_payload: true,
        target_mutable: true,
        selected_file_count: 0,
        selected_directory_count: 0,
    };

    let roots = resolve_action_tree(ContextTargetKind::SshConnection, &selection);
    let proxy = roots
        .iter()
        .find(|node| node.id == "proxy-chrome-via-server")
        .expect("ssh menu should expose the proxy chrome action");

    assert_eq!(proxy.state, ContextMenuActionState::Planned);
}

#[test]
fn open_action_stays_enabled_for_ssh_assets() {
    let actions = resolve_action_tree(
        ContextTargetKind::SshConnection,
        &SelectionContext {
            selected_ids: vec!["ssh-prod-01".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );

    let open = actions
        .iter()
        .find(|node| node.id == "open-connection")
        .expect("open action should exist");

    assert_eq!(open.state, ContextMenuActionState::Enabled);
}

#[test]
fn folder_target_exposes_flat_create_actions() {
    let actions = resolve_action_tree(
        ContextTargetKind::Folder,
        &SelectionContext {
            selected_ids: vec!["folder-1".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );

    let new_folder = actions
        .iter()
        .find(|action| action.id == "new-folder")
        .expect("folder target should expose new-folder");
    let new_ssh = actions
        .iter()
        .find(|action| action.id == "new-ssh-connection")
        .expect("folder target should expose new-ssh-connection");

    assert!(new_folder.children.is_empty());
    assert!(new_ssh.children.is_empty());
}

#[test]
fn snippet_target_exposes_paste_run_edit_and_delete_actions() {
    let actions = resolve_action_tree(
        ContextTargetKind::Snippet,
        &SelectionContext {
            selected_ids: vec!["snippet-1".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );
    let ids: Vec<_> = actions.iter().map(|action| action.id).collect();

    assert!(ids.contains(&"paste-snippet"));
    assert!(ids.contains(&"run-snippet"));
    assert!(ids.contains(&"edit-snippet"));
    assert!(ids.contains(&"delete-snippet"));
}

#[test]
fn snippet_package_target_omits_run_and_paste_but_keeps_package_actions() {
    let actions = resolve_action_tree(
        ContextTargetKind::SnippetPackage,
        &SelectionContext {
            selected_ids: vec!["package-1".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );
    let ids: Vec<_> = actions.iter().map(|action| action.id).collect();

    assert!(ids.contains(&"new-snippet"));
    assert!(ids.contains(&"new-package"));
    assert!(ids.contains(&"edit-package"));
    assert!(ids.contains(&"delete-package"));
    assert!(!ids.contains(&"run-snippet"));
    assert!(!ids.contains(&"paste-snippet"));
}

#[test]
fn folder_and_ssh_context_menus_keep_rename_and_delete_as_enabled_leaf_actions() {
    let folder_actions = resolve_action_tree(
        ContextTargetKind::Folder,
        &SelectionContext {
            selected_ids: vec!["folder-1".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );
    let ssh_actions = resolve_action_tree(
        ContextTargetKind::SshConnection,
        &SelectionContext {
            selected_ids: vec!["ssh-1".into()],
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );

    for actions in [&folder_actions, &ssh_actions] {
        let rename = actions
            .iter()
            .find(|action| action.id == "rename-asset")
            .expect("rename action should exist");
        let delete = actions
            .iter()
            .find(|action| action.id == "delete-asset")
            .expect("delete action should exist");

        assert_eq!(rename.state, ContextMenuActionState::Enabled);
        assert!(rename.children.is_empty());
        assert_eq!(delete.state, ContextMenuActionState::Enabled);
        assert!(delete.children.is_empty());
    }
}

#[test]
fn root_menu_flips_left_when_anchor_is_near_right_edge() {
    let (origin_x, origin_y, child_flows_left) = resolve_root_menu_origin(MenuPlacementInput {
        host_width: 760.0,
        host_height: 640.0,
        anchor_x: 748.0,
        anchor_y: 632.0,
        root_width: 224.0,
        root_height: 320.0,
        child_width: 0.0,
    });

    assert!((origin_x - 524.0).abs() < f32::EPSILON);
    assert!((origin_y - 312.0).abs() < f32::EPSILON);
    assert!(!child_flows_left);
}

#[test]
fn submenu_flips_left_when_secondary_column_would_overflow() {
    let (origin_x, origin_y, child_flows_left) = resolve_root_menu_origin(MenuPlacementInput {
        host_width: 760.0,
        host_height: 640.0,
        anchor_x: 500.0,
        anchor_y: 160.0,
        root_width: 224.0,
        root_height: 320.0,
        child_width: 232.0,
    });

    assert!((origin_x - 268.0).abs() < f32::EPSILON);
    assert!((origin_y - 160.0).abs() < f32::EPSILON);
    assert!(child_flows_left);
}

#[test]
fn corridor_logic_keeps_submenu_open_while_pointer_moves_toward_child_column() {
    let parent_rect = Rect {
        x: 120.0,
        y: 96.0,
        width: 224.0,
        height: 32.0,
    };
    let child_rect = Rect {
        x: 352.0,
        y: 80.0,
        width: 224.0,
        height: 160.0,
    };

    assert!(should_keep_corridor_open(
        (348.0, 124.0),
        parent_rect,
        child_rect,
    ));
}

#[test]
fn corridor_keeps_pointer_alive_between_parent_and_child_columns() {
    let parent = Rect {
        x: 100.0,
        y: 100.0,
        width: 224.0,
        height: 120.0,
    };
    let child = Rect {
        x: 332.0,
        y: 100.0,
        width: 224.0,
        height: 120.0,
    };

    assert!(should_keep_corridor_open((320.0, 140.0), parent, child));
    assert!(!should_keep_corridor_open((320.0, 40.0), parent, child));
}

#[test]
fn esc_closes_context_menu() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(ContextTargetKind::BlankArea, None, 24.0, 36.0);

    view_model.handle_context_menu_escape();

    assert!(!view_model.context_menu_open);
    assert!(view_model.context_menu_open_path.is_empty());
}

#[test]
fn right_key_leaves_flat_blank_area_menu_on_primary_column() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(ContextTargetKind::BlankArea, None, 24.0, 36.0);

    view_model.navigate_context_menu_right();

    assert!(view_model.context_menu_open_path.is_empty());
}

#[test]
fn invoking_run_snippet_leaf_action_records_explicit_run_activation() {
    let mut snippet_tree = AssetTree::new();
    let snippet_id = snippet_tree.insert_root(ConsoleAssetKind::Snippet, "Deploy prod");

    let mut view_model = ShellViewModel::default();
    view_model.replace_snippet_asset_tree(snippet_tree);
    view_model.open_context_menu_for_target(
        ContextTargetKind::Snippet,
        Some(snippet_id),
        120.0,
        160.0,
    );

    view_model.handle_context_menu_leaf_action("run-snippet");

    assert_eq!(
        view_model.pending_snippet_activation(),
        Some(mica_term::shell::view_model::SnippetActivation::Run)
    );
}

#[test]
fn invoking_planned_action_sets_feedback_text_without_closing_documentation_gap() {
    let mut view_model = ShellViewModel::default();
    view_model.open_context_menu_for_target(
        ContextTargetKind::SshConnection,
        Some("ssh-prod-01".into()),
        144.0,
        188.0,
    );

    let roots = resolve_action_tree(
        ContextTargetKind::SshConnection,
        &SelectionContext {
            selected_ids: view_model.selected_asset_ids.clone(),
            clipboard_has_asset_payload: false,
            target_mutable: true,
            selected_file_count: 0,
            selected_directory_count: 0,
        },
    );
    let proxy_index = roots
        .iter()
        .position(|node| node.id == "proxy-chrome-via-server")
        .expect("ssh menu should expose the proxy chrome action");

    view_model.set_context_menu_open_path(vec![proxy_index]);
    view_model.invoke_current_context_menu_item();

    assert!(view_model.context_menu_open);
    assert_eq!(
        view_model.context_menu_feedback_text,
        "Proxy Chrome via Server is not wired yet."
    );
}
