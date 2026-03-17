use mica_term::shell::context_menu::{
    ContextMenuActionState, ContextTargetKind, MenuPlacementInput, Rect, SelectionContext,
    resolve_action_tree, resolve_root_menu_origin, should_keep_corridor_open,
    visible_columns_for_path,
};
use mica_term::shell::view_model::ShellViewModel;

#[test]
fn blank_area_scene_only_exposes_minimal_create_actions() {
    let selection = SelectionContext {
        selected_ids: Vec::new(),
        clipboard_has_asset_payload: false,
        target_mutable: true,
        target_has_active_connection: false,
    };

    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &selection);
    let ids: Vec<_> = roots.iter().map(|node| node.id).collect();

    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
}

#[test]
fn blank_area_scene_omits_paste_and_other_legacy_actions() {
    let selection = SelectionContext {
        selected_ids: Vec::new(),
        clipboard_has_asset_payload: true,
        target_mutable: true,
        target_has_active_connection: false,
    };

    let ids: Vec<_> = resolve_action_tree(ContextTargetKind::BlankArea, &selection)
        .into_iter()
        .map(|node| node.id)
        .collect();

    assert!(!ids.contains(&"new-connection"));
    assert!(!ids.contains(&"paste-asset"));
    assert!(!ids.contains(&"batch-open"));
}

#[test]
fn resolver_returns_ssh_actions_with_planned_proxy_tools() {
    let selection = SelectionContext {
        selected_ids: vec!["ssh-prod-01".into()],
        clipboard_has_asset_payload: true,
        target_mutable: true,
        target_has_active_connection: true,
    };

    let ids: Vec<_> = resolve_action_tree(ContextTargetKind::SshConnection, &selection)
        .into_iter()
        .map(|node| node.id)
        .collect();

    assert!(ids.contains(&"close-connection"));
    assert!(ids.contains(&"open-in-new-tab"));
    assert!(ids.contains(&"proxy-chrome-via-server"));
}

#[test]
fn blank_area_visible_columns_stay_flat_for_primary_leaf_selection() {
    let selection = SelectionContext {
        selected_ids: Vec::new(),
        clipboard_has_asset_payload: true,
        target_mutable: true,
        target_has_active_connection: false,
    };

    let roots = resolve_action_tree(ContextTargetKind::BlankArea, &selection);
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
        target_has_active_connection: true,
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
        target_has_active_connection: true,
    };

    let roots = resolve_action_tree(ContextTargetKind::SshConnection, &selection);
    let proxy = roots
        .iter()
        .find(|node| node.id == "proxy-chrome-via-server")
        .expect("ssh menu should expose the proxy chrome action");

    assert_eq!(proxy.state, ContextMenuActionState::Planned);
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
            target_has_active_connection: true,
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
