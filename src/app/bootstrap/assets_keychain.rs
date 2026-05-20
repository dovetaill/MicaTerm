//! Bootstrap assets and keychain binder module.

use super::*;
use crate::shell::context_menu::context_menu_column_width_for_items;

pub(super) fn sync_sidebar_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_show_assets_sidebar(state.show_assets_sidebar);
    window.set_assets_sidebar_expanded_width(state.assets_sidebar_expanded_width_px());
    window.set_active_sidebar_destination(state.active_sidebar_destination.id().into());
    window.set_sidebar_items(ModelRc::new(VecModel::from(sidebar_items_for(state))));
    sync_assets_toolbar_state(window, state);
    sync_console_assets(window, state);
    sync_keychain_assets(window, state);
}

pub(super) fn sync_assets_toolbar_state(window: &AppWindow, state: &ShellViewModel) {
    let descriptor = toolbar_descriptor_for(state.active_sidebar_destination, state);
    window.set_asset_view_mode(state.asset_view_mode.id().into());
    window.set_asset_search_expanded(state.asset_search_expanded);
    let active_query = if state.active_sidebar_destination == SidebarDestination::Keychain {
        state.keychain_search_query.clone()
    } else {
        state.asset_search_query.clone()
    };
    window.set_assets_search_query(active_query.into());
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

pub(super) fn sync_assets_context_menu_state(window: &AppWindow, state: &ShellViewModel) {
    window.set_assets_context_menu_open(state.context_menu_open);
    window.set_assets_context_menu_anchor_x(state.context_menu_anchor_x);
    window.set_assets_context_menu_anchor_y(state.context_menu_anchor_y);
    window.set_assets_context_menu_origin_x(state.context_menu_origin_x);
    window.set_assets_context_menu_origin_y(state.context_menu_origin_y);
    window.set_assets_context_menu_column_width(context_menu_column_width_for(state));
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

fn context_menu_column_width_for(state: &ShellViewModel) -> f32 {
    context_menu_columns_for(state)
        .into_iter()
        .filter(|column| !column.is_empty())
        .map(|column| context_menu_column_width_for_items(column.as_slice()))
        .fold(CONTEXT_MENU_COLUMN_WIDTH, f32::max)
}

fn context_menu_child_width_for(state: &ShellViewModel) -> f32 {
    let column_width = context_menu_column_width_for(state);
    let child_count = context_menu_visible_column_count(state).saturating_sub(1) as f32;
    if child_count <= 0.0 {
        0.0
    } else {
        child_count * (column_width + CONTEXT_MENU_COLUMN_GAP)
    }
}

fn context_menu_column_rects_for(state: &ShellViewModel) -> [Option<Rect>; 3] {
    let columns = context_menu_columns_for(state);
    let column_width = context_menu_column_width_for(state);
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
                    column_width,
                ),
            y: state.context_menu_origin_y,
            width: column_width,
            height,
        });
    }

    rects
}

pub(super) fn update_context_menu_placement(window: &AppWindow, state: &mut ShellViewModel) {
    if !state.context_menu_open {
        state.set_context_menu_placement(0.0, 0.0, false);
        return;
    }

    let column_width = context_menu_column_width_for(state);
    let (host_width, host_height) = current_window_size(window);
    let (origin_x, origin_y, child_flows_left) = resolve_root_menu_origin(MenuPlacementInput {
        host_width: host_width as f32,
        host_height: host_height as f32,
        anchor_x: state.context_menu_anchor_x,
        anchor_y: state.context_menu_anchor_y,
        root_width: column_width,
        root_height: context_menu_overlay_height_for(state),
        child_width: context_menu_child_width_for(state),
    });

    state.set_context_menu_placement(origin_x, origin_y, child_flows_left);
}

pub(super) fn schedule_asset_modal_focus(window: &AppWindow) {
    let handle = window.as_weak();
    let _ = slint::invoke_from_event_loop(move || {
        let window = handle.unwrap();
        if window.get_asset_modal_open()
            || window.get_asset_rename_modal_open()
            || window.get_asset_delete_confirm_modal_open()
            || window.get_workspace_paste_warning_modal_open()
        {
            window.set_asset_modal_focus_sequence(window.get_asset_modal_focus_sequence() + 1);
        }
        if window.get_sftp_remote_file_modal_open() {
            window.set_sftp_remote_file_modal_focus_sequence(
                window.get_sftp_remote_file_modal_focus_sequence() + 1,
            );
        }
        if window.get_sftp_conflict_modal_open() {
            window.set_sftp_conflict_modal_focus_sequence(
                window.get_sftp_conflict_modal_focus_sequence() + 1,
            );
        }
    });
}

fn open_pending_snippet_create_modal(state: &mut ShellViewModel) {
    match state.take_pending_snippet_create_action() {
        Some(crate::shell::view_model::SnippetCreateAction::NewSnippet) => {
            state.open_new_snippet_modal(None);
        }
        Some(crate::shell::view_model::SnippetCreateAction::NewPackage) => {
            state.open_new_snippet_package_modal();
        }
        None => {}
    }
}

fn clear_asset_snippet_modal_fields(window: &AppWindow) {
    window.set_asset_snippet_modal_name("".into());
    window.set_asset_snippet_modal_script("".into());
    window.set_asset_snippet_modal_package("".into());
    sync_snippet_package_options(window, Vec::new());
    window.set_asset_snippet_modal_package_selected_label("".into());
    window.set_asset_snippet_package_modal_name("".into());
}

fn clear_asset_ssh_modal_fields(window: &AppWindow) {
    window.set_asset_ssh_modal_name("".into());
    window.set_asset_ssh_modal_host("".into());
    window.set_asset_ssh_modal_user("".into());
    window.set_asset_ssh_modal_port("22".into());
    window.set_asset_ssh_modal_auth_source("manual".into());
    window.set_asset_ssh_modal_auth_method("password".into());
    sync_ssh_keychain_identity_options(window, Vec::new());
    window.set_asset_ssh_modal_keychain_identity_selected_label("".into());
    window.set_asset_ssh_modal_keychain_identity_username("".into());
    window.set_asset_ssh_modal_keychain_identity_auth_summary("".into());
    window.set_asset_ssh_modal_private_key_source("content".into());
    window.set_asset_ssh_modal_password("".into());
    window.set_asset_ssh_modal_private_key_content("".into());
    window.set_asset_ssh_modal_private_key_path("".into());
    window.set_asset_ssh_modal_passphrase("".into());
    window.set_asset_ssh_modal_password_visible(false);
    window.set_asset_ssh_modal_passphrase_visible(false);
    window.set_asset_ssh_modal_remark("".into());
    window.set_asset_ssh_modal_environment("".into());
    window.set_asset_ssh_modal_proxy_type("none".into());
    window.set_asset_ssh_modal_proxy_socks5_host("".into());
    window.set_asset_ssh_modal_proxy_socks5_port("".into());
    window.set_asset_ssh_modal_proxy_socks5_username("".into());
    window.set_asset_ssh_modal_proxy_socks5_password("".into());
    window.set_asset_ssh_modal_proxy_socks5_password_visible(false);
    window.set_asset_ssh_modal_proxy_ssh_asset_id("".into());
    sync_ssh_proxy_target_options(window, Vec::new());
    window.set_asset_ssh_modal_proxy_ssh_selected_label("".into());
    window.set_asset_ssh_modal_proxy_method("".into());
}

pub(super) fn sync_asset_modal_state(window: &AppWindow, state: &ShellViewModel) {
    sync_keychain_modal_defaults(window);
    match &state.asset_modal_state {
        Some(AssetModalState::NewFolder { draft_name, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-folder".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpNewFile { draft_name }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-file".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpNewFolder { draft_name }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-folder".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name(draft_name.clone().into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewSnippet { draft, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-snippet".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_snippet_modal_name(draft.name.clone().into());
            window.set_asset_snippet_modal_script(draft.script.clone().into());
            window.set_asset_snippet_modal_package(draft.package.clone().into());
            let mut package_options = vec!["No Package".to_string()];
            package_options.extend(state.snippet_package_option_labels());
            sync_snippet_package_options(window, package_options);
            window.set_asset_snippet_modal_package_selected_label(
                if draft.package.trim().is_empty() {
                    "No Package"
                } else {
                    draft.package.as_str()
                }
                .into(),
            );
            window.set_asset_snippet_package_modal_name("".into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewSnippetPackage { draft_name, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-snippet-package".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            window.set_asset_snippet_modal_name("".into());
            window.set_asset_snippet_modal_script("".into());
            window.set_asset_snippet_modal_package("".into());
            sync_snippet_package_options(window, Vec::new());
            window.set_asset_snippet_modal_package_selected_label("".into());
            window.set_asset_snippet_package_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::NewKeychainIdentity { draft, .. }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-keychain-identity".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            clear_asset_ssh_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_keychain_identity_modal_name(draft.name.clone().into());
            window.set_keychain_identity_modal_username(draft.username.clone().into());
            window.set_keychain_identity_modal_auth_kind(draft.auth_kind.clone().into());
            window.set_keychain_identity_modal_password(draft.password.clone().into());
            window.set_keychain_identity_modal_password_visible(draft.password_visible);
            window.set_keychain_identity_modal_ssh_key_label(draft.ssh_key_label.clone().into());
            window.set_keychain_identity_modal_remark(draft.remark.clone().into());
        }
        Some(AssetModalState::NewKeychainSshKey {
            draft,
            editing_item_id,
            ..
        }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-keychain-ssh-key".into());
            window.set_asset_ssh_modal_dialog_title(if editing_item_id.is_some() {
                "Edit SSH Key".into()
            } else {
                "New SSH Key".into()
            });
            window.set_asset_modal_can_confirm(state.asset_create_modal_can_confirm());
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
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_keychain_ssh_key_modal_name(draft.name.clone().into());
            window.set_keychain_ssh_key_modal_private_key(draft.private_key.clone().into());
            window.set_keychain_ssh_key_modal_public_key(draft.public_key.clone().into());
            window.set_keychain_ssh_key_modal_fingerprint(draft.fingerprint.clone().into());
        }
        Some(AssetModalState::NewSshConnection {
            draft,
            editing_asset_id,
            ..
        }) => {
            window.set_asset_modal_open(true);
            window.set_asset_modal_kind("new-ssh-connection".into());
            window.set_asset_ssh_modal_dialog_title(
                if editing_asset_id.is_some() {
                    "Edit SSH Connection"
                } else {
                    "New SSH Connection"
                }
                .into(),
            );
            window.set_asset_modal_can_confirm(state.ssh_modal_save_enabled());
            window.set_asset_modal_validation_message(
                state.asset_create_modal_validation_message().into(),
            );
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            window.set_asset_ssh_modal_name(draft.name.clone().into());
            window.set_asset_ssh_modal_host(draft.host.clone().into());
            window.set_asset_ssh_modal_user(draft.user.clone().into());
            window.set_asset_ssh_modal_port(draft.port.clone().into());
            window.set_asset_ssh_modal_auth_source(draft.auth_source.clone().into());
            window.set_asset_ssh_modal_auth_method(draft.auth_method.clone().into());
            sync_ssh_keychain_identity_options(window, state.ssh_keychain_identity_option_labels());
            window.set_asset_ssh_modal_keychain_identity_selected_label(
                state.ssh_keychain_identity_selected_label().into(),
            );
            window.set_asset_ssh_modal_keychain_identity_username(
                state.ssh_keychain_identity_selected_username().into(),
            );
            window.set_asset_ssh_modal_keychain_identity_auth_summary(
                state.ssh_keychain_identity_selected_auth_summary().into(),
            );
            window.set_asset_ssh_modal_private_key_source(draft.private_key_source.clone().into());
            window.set_asset_ssh_modal_password(draft.password.clone().into());
            window
                .set_asset_ssh_modal_private_key_content(draft.private_key_content.clone().into());
            window.set_asset_ssh_modal_private_key_path(draft.private_key_path.clone().into());
            window.set_asset_ssh_modal_passphrase(draft.passphrase.clone().into());
            window.set_asset_ssh_modal_password_visible(draft.password_visible);
            window.set_asset_ssh_modal_passphrase_visible(draft.passphrase_visible);
            window.set_asset_ssh_modal_remark(draft.remark.clone().into());
            window.set_asset_ssh_modal_environment(draft.environment.clone().into());
            window.set_asset_ssh_modal_proxy_type(draft.proxy_type.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_host(draft.proxy_socks5_host.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_port(draft.proxy_socks5_port.clone().into());
            window.set_asset_ssh_modal_proxy_socks5_username(
                draft.proxy_socks5_username.clone().into(),
            );
            window.set_asset_ssh_modal_proxy_socks5_password(
                draft.proxy_socks5_password.clone().into(),
            );
            window.set_asset_ssh_modal_proxy_socks5_password_visible(
                draft.proxy_socks5_password_visible,
            );
            window.set_asset_ssh_modal_proxy_ssh_asset_id(draft.proxy_ssh_asset_id.clone().into());
            sync_ssh_proxy_target_options(window, state.ssh_proxy_target_option_labels());
            window.set_asset_ssh_modal_proxy_ssh_selected_label(
                state.ssh_proxy_target_selected_label().into(),
            );
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
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(true);
            window.set_asset_rename_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_validation_message(
                state.asset_rename_modal_validation_message().into(),
            );
            window.set_asset_rename_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpRenameEntry { draft_name, .. }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(true);
            window.set_asset_rename_modal_name(draft_name.clone().into());
            window.set_asset_rename_modal_validation_message(
                state.asset_rename_modal_validation_message().into(),
            );
            window.set_asset_rename_modal_can_confirm(state.can_confirm_asset_modal());
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::DeleteAssetConfirm {
            label,
            descendant_count,
            ..
        }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(true);
            window.set_asset_delete_confirm_target_label(label.clone().into());
            window.set_asset_delete_confirm_descendant_count(*descendant_count as i32);
            clear_asset_ssh_modal_fields(window);
        }
        Some(AssetModalState::SftpDeleteEntriesConfirm {
            label,
            descendant_count,
            ..
        }) => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(true);
            window.set_asset_delete_confirm_target_label(label.clone().into());
            window.set_asset_delete_confirm_descendant_count(*descendant_count as i32);
            clear_asset_ssh_modal_fields(window);
        }
        None => {
            window.set_asset_modal_open(false);
            window.set_asset_modal_kind("".into());
            window.set_asset_ssh_modal_dialog_title("New SSH Connection".into());
            window.set_asset_modal_can_confirm(false);
            window.set_asset_modal_validation_message("".into());
            window.set_asset_ssh_modal_connect_family_enabled(false);
            window.set_asset_ssh_modal_feedback_state("idle".into());
            window.set_asset_ssh_modal_feedback_message("".into());
            window.set_asset_folder_modal_name("".into());
            clear_asset_snippet_modal_fields(window);
            window.set_asset_rename_modal_open(false);
            window.set_asset_rename_modal_name("".into());
            window.set_asset_rename_modal_validation_message("".into());
            window.set_asset_rename_modal_can_confirm(false);
            window.set_asset_delete_confirm_modal_open(false);
            window.set_asset_delete_confirm_target_label("".into());
            window.set_asset_delete_confirm_descendant_count(0);
            clear_asset_ssh_modal_fields(window);
        }
    }
    super::sync_workspace_native_terminal_surface_geometry(window);
}

fn sync_ssh_proxy_target_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_ssh_modal_proxy_ssh_options(),
        rows,
        |model| window.set_asset_ssh_modal_proxy_ssh_options(model),
    );
}

pub(super) fn sync_ssh_keychain_identity_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_ssh_modal_keychain_identity_options(),
        rows,
        |model| window.set_asset_ssh_modal_keychain_identity_options(model),
    );
}

fn sync_snippet_package_options(window: &AppWindow, labels: Vec<String>) {
    let rows = labels
        .into_iter()
        .map(SharedString::from)
        .collect::<Vec<_>>();
    sync_vec_model(
        window.get_asset_snippet_modal_package_options(),
        rows,
        |model| window.set_asset_snippet_modal_package_options(model),
    );
}

pub(super) fn sync_console_assets(window: &AppWindow, state: &ShellViewModel) {
    let project_rows = |rows: Vec<crate::shell::assets::VisibleAssetRow>| {
        rows.into_iter()
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
            .collect::<Vec<_>>()
    };

    window.set_console_asset_items(ModelRc::new(VecModel::from(project_rows(
        state.visible_console_asset_rows(),
    ))));
    window.set_snippet_asset_items(ModelRc::new(VecModel::from(project_rows(
        state.visible_snippet_rows(),
    ))));
    sync_welcome_quick_launch_state(window, state);
    sync_saved_ssh_picker_state(window, state);
}

pub(super) fn sync_keychain_assets(window: &AppWindow, state: &ShellViewModel) {
    let rows = state
        .visible_keychain_rows()
        .into_iter()
        .map(|row| ConsoleAssetItem {
            id: row.id.clone().into(),
            kind: row.kind.id().into(),
            label: row.label.clone().into(),
            depth: row.depth as i32,
            has_children: row.has_children,
            expanded: row.expanded,
            selected: state
                .selected_keychain_ids
                .iter()
                .any(|selected_id| selected_id == &row.id),
            focused: state.focused_keychain_id.as_deref() == Some(row.id.as_str()),
            disclosure_state: match row.disclosure_state {
                AssetDisclosureState::None => "none",
                AssetDisclosureState::Collapsed => "collapsed",
                AssetDisclosureState::Expanded => "expanded",
            }
            .into(),
            path_hint: row.path_hint.unwrap_or_default().into(),
            compact_flat_mode: false,
        })
        .collect::<Vec<_>>();

    window.set_keychain_asset_items(ModelRc::new(VecModel::from(rows)));
}

fn sync_keychain_modal_defaults(window: &AppWindow) {
    window.set_keychain_identity_modal_name("".into());
    window.set_keychain_identity_modal_username("".into());
    window.set_keychain_identity_modal_auth_kind("password".into());
    window.set_keychain_identity_modal_password("".into());
    window.set_keychain_identity_modal_password_visible(false);
    window.set_keychain_identity_modal_ssh_key_label("".into());
    window.set_keychain_identity_modal_remark("".into());
    window.set_keychain_ssh_key_modal_name("".into());
    window.set_keychain_ssh_key_modal_private_key("".into());
    window.set_keychain_ssh_key_modal_public_key("".into());
    window.set_keychain_ssh_key_modal_fingerprint("".into());
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bind_assets_keychain_callbacks(
    window: &AppWindow,
    view_model: &Rc<RefCell<ShellViewModel>>,
    asset_repo: &Option<Rc<dyn AssetCatalogRepository>>,
    session_bridge: &Option<Rc<ShellSessionBridge>>,
    session_runtime_guard: &Option<AppAsyncRuntime>,
    sftp_async_runtime: Option<&tokio::runtime::Handle>,
    sftp_result_tx: &std::sync::mpsc::Sender<super::sftp::SftpBrowserBackgroundMessage>,
    sftp_transfer_result_tx: &std::sync::mpsc::Sender<super::sftp::SftpTransferBackgroundMessage>,
    ssh_modal_result_tx: &std::sync::mpsc::Sender<super::SshModalBackgroundMessage>,
    sftp_browser_controller: &Rc<RefCell<SftpBrowserController>>,
    credential_store: &Arc<dyn CredentialStore>,
    private_key_importer: &Arc<dyn PrivateKeyImporter>,
    keychain_repo: &Option<Rc<dyn KeychainCatalogRepository>>,
    quick_launch_store: &Option<Rc<QuickLaunchPreferencesStore>>,
    vault_session: &Rc<RefCell<VaultSessionState>>,
    workspace_follow_tracker: &Rc<RefCell<WorkspaceFollowTracker>>,
    pending_host_key_approval: &Rc<RefCell<Option<PendingHostKeyApproval>>>,
    modal_drag_state: &Rc<RefCell<Option<ModalDragState>>>,
    vault_sync_service: &Rc<VaultSyncService>,
    vault_auto_sync_timer: &Rc<Timer>,
    run_vault_sync: &Rc<dyn Fn(VaultSyncTrigger)>,
    asset_click_tracker: &Rc<RefCell<Option<PendingAssetClick>>>,
    pending_double_click_activation: &Rc<RefCell<Option<String>>>,
    next_ssh_modal_test_request_id: &Rc<Cell<u64>>,
    active_ssh_modal_test_request_id: &Rc<RefCell<Option<u64>>>,
) {
    let sftp_async_runtime = sftp_async_runtime.cloned();
    let sftp_result_tx = sftp_result_tx.clone();
    let sftp_transfer_result_tx = sftp_transfer_result_tx.clone();
    let ssh_modal_result_tx = ssh_modal_result_tx.clone();
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.activate_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_assets_search_query_changed(move |query| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.set_keychain_search_query(query.to_string());
        } else {
            state.set_asset_search_query(query.to_string());
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_close_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_search();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_collapse_assets_search_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.collapse_asset_search_if_empty();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_view_mode_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_view_mode();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_tree_expansion_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.toggle_asset_tree_expansion();
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.toggle_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_close_assets_create_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_asset_create_menu();
        sync_assets_toolbar_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let keychain_repo_ref = keychain_repo.clone();
    let vault_session_ref = Rc::clone(vault_session);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    window.on_assets_create_action_selected(move |action_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();
        let keychain_node_count_before = state.keychain_catalog().nodes.len();
        state.dismiss_empty_asset_search_on_shell_interaction();
        if state.active_sidebar_destination == SidebarDestination::Snippets {
            state.handle_snippet_create_action(action_id.as_str());
            open_pending_snippet_create_modal(&mut state);
        } else {
            state.handle_assets_create_action(action_id.as_str());
        }
        if state.keychain_catalog().nodes.len() > keychain_node_count_before {
            save_keychain_catalog_if_available(&keychain_repo_ref, &state);
            let mut vault = vault_session_ref.borrow_mut();
            vault_sync::mark_local_vault_dirty_and_arm_sync(
                &mut state,
                &mut vault,
                &vault_sync_service_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(modal_drag_state);
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

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let keychain_repo_ref = keychain_repo.clone();
    let credential_store_ref = Arc::clone(credential_store);
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let sftp_async_runtime_ref = sftp_async_runtime.clone();
    let sftp_result_tx_ref = sftp_result_tx.clone();
    let sftp_transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    let vault_session_ref = Rc::clone(vault_session);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    window.on_confirm_asset_modal_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let pending_identity_draft = match state.asset_modal_state.as_ref() {
            Some(AssetModalState::NewKeychainIdentity { draft, .. }) => Some(draft.clone()),
            _ => None,
        };
        let pending_keychain_draft = match state.asset_modal_state.as_ref() {
            Some(AssetModalState::NewKeychainSshKey { draft, .. }) => Some(draft.clone()),
            _ => None,
        };
        let should_save_keychain_catalog =
            pending_identity_draft.is_some() || pending_keychain_draft.is_some();
        let is_sftp_create_modal = matches!(
            state.asset_modal_state.as_ref(),
            Some(AssetModalState::SftpNewFile { .. } | AssetModalState::SftpNewFolder { .. })
        );
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            if is_sftp_create_modal {
                let mut sftp_browser_controller = sftp_browser_controller_ref.borrow_mut();
                let _ = super::sftp::apply_pending_sftp_context_action(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut sftp_browser_controller,
                    sftp_async_runtime_ref.as_ref(),
                    &sftp_result_tx_ref,
                    &sftp_transfer_result_tx_ref,
                );
            } else if let Some(draft) = pending_identity_draft.as_ref()
                && let Some(identity_id) = state.focused_keychain_id.clone()
                && let Err(err) = persist_keychain_identity_secret(
                    credential_store_ref.as_ref(),
                    identity_id.as_str(),
                    draft,
                )
            {
                tracing::error!(
                    target: "app.keychain",
                    identity_id,
                    error = %err,
                    "failed to persist keychain identity secret bundle"
                );
            }
            if let Some(draft) = pending_keychain_draft.as_ref()
                && let Some(key_id) = state.focused_keychain_id.clone()
                && let Err(err) = persist_keychain_ssh_key_secret(
                    credential_store_ref.as_ref(),
                    key_id.as_str(),
                    draft,
                )
            {
                tracing::error!(
                    target: "app.keychain",
                    key_id,
                    error = %err,
                    "failed to persist keychain SSH key secret bundle"
                );
            }
            if !is_sftp_create_modal {
                save_asset_catalog_if_available(&asset_repo_ref, &state);
                if should_save_keychain_catalog {
                    save_keychain_catalog_if_available(&keychain_repo_ref, &state);
                }
                let mut vault = vault_session_ref.borrow_mut();
                vault_sync::mark_local_vault_dirty_and_arm_sync(
                    &mut state,
                    &mut vault,
                    &vault_sync_service_ref,
                    &vault_auto_sync_timer_ref,
                    Rc::clone(&run_vault_sync_ref),
                );
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_rename_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_rename_asset_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let keychain_repo_ref = keychain_repo.clone();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let sftp_async_runtime_ref = sftp_async_runtime.clone();
    let sftp_result_tx_ref = sftp_result_tx.clone();
    let sftp_transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    let vault_session_ref = Rc::clone(vault_session);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    window.on_confirm_asset_rename_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let should_save_keychain_catalog = matches!(
            state.asset_modal_state.as_ref(),
            Some(AssetModalState::RenameAsset { asset_id, .. })
                if state.keychain_catalog().nodes.contains_key(asset_id)
        );
        let is_sftp_rename_modal = matches!(
            state.asset_modal_state.as_ref(),
            Some(AssetModalState::SftpRenameEntry { .. })
        );
        let did_mutate = state.confirm_asset_modal();
        if did_mutate {
            if is_sftp_rename_modal {
                let mut sftp_browser_controller = sftp_browser_controller_ref.borrow_mut();
                let _ = super::sftp::apply_pending_sftp_context_action(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut sftp_browser_controller,
                    sftp_async_runtime_ref.as_ref(),
                    &sftp_result_tx_ref,
                    &sftp_transfer_result_tx_ref,
                );
            } else {
                save_asset_catalog_if_available(&asset_repo_ref, &state);
                if should_save_keychain_catalog {
                    save_keychain_catalog_if_available(&keychain_repo_ref, &state);
                }
                let mut vault = vault_session_ref.borrow_mut();
                vault_sync::mark_local_vault_dirty_and_arm_sync(
                    &mut state,
                    &mut vault,
                    &vault_sync_service_ref,
                    &vault_auto_sync_timer_ref,
                    Rc::clone(&run_vault_sync_ref),
                );
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let keychain_repo_ref = keychain_repo.clone();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let sftp_async_runtime_ref = sftp_async_runtime.clone();
    let sftp_result_tx_ref = sftp_result_tx.clone();
    let sftp_transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    let vault_session_ref = Rc::clone(vault_session);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    window.on_confirm_delete_asset_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let should_save_keychain_catalog = matches!(
            state.asset_modal_state.as_ref(),
            Some(AssetModalState::DeleteAssetConfirm { asset_id, .. })
                if state.keychain_catalog().nodes.contains_key(asset_id)
        );
        let is_sftp_delete_modal = matches!(
            state.asset_modal_state.as_ref(),
            Some(AssetModalState::SftpDeleteEntriesConfirm { .. })
        );
        let did_mutate = state.confirm_delete_asset();
        if did_mutate {
            if is_sftp_delete_modal {
                let mut sftp_browser_controller = sftp_browser_controller_ref.borrow_mut();
                let _ = super::sftp::apply_pending_sftp_context_action(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut sftp_browser_controller,
                    sftp_async_runtime_ref.as_ref(),
                    &sftp_result_tx_ref,
                    &sftp_transfer_result_tx_ref,
                );
            } else {
                save_asset_catalog_if_available(&asset_repo_ref, &state);
                if should_save_keychain_catalog {
                    save_keychain_catalog_if_available(&keychain_repo_ref, &state);
                }
                let mut vault = vault_session_ref.borrow_mut();
                vault_sync::mark_local_vault_dirty_and_arm_sync(
                    &mut state,
                    &mut vault,
                    &vault_sync_service_ref,
                    &vault_auto_sync_timer_ref,
                    Rc::clone(&run_vault_sync_ref),
                );
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_folder_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_new_folder_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_snippet_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_snippet_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_snippet_package_modal_name_changed(move |value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_snippet_package_modal_name(value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_ssh_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_ssh_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_keychain_identity_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_keychain_identity_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_keychain_identity_modal_action_requested(move |action| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if action.as_str() == "use-existing-ssh-key" {
            state.select_first_keychain_identity_modal_ssh_key();
        }
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_keychain_ssh_key_modal_draft_changed(move |field, value| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.update_keychain_ssh_key_modal_field(field.as_str(), value.to_string());
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let asset_repo_ref = asset_repo.clone();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(pending_host_key_approval);
    let credential_store_ref = Arc::clone(credential_store);
    let private_key_importer_ref = Arc::clone(private_key_importer);
    let vault_session_ref = Rc::clone(vault_session);
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    let ssh_modal_result_tx_ref = ssh_modal_result_tx.clone();
    let next_ssh_modal_test_request_id_ref = Rc::clone(next_ssh_modal_test_request_id);
    let active_ssh_modal_test_request_id_ref = Rc::clone(active_ssh_modal_test_request_id);
    window.on_asset_ssh_modal_action_requested(move |action| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if action.as_str() == "import-private-key" {
            if let Err(err) =
                import_private_key_into_ssh_modal(&mut state, private_key_importer_ref.as_ref())
            {
                state.finish_ssh_modal_action_error(err.to_string());
            }
            sync_asset_modal_state(&window, &state);
            return;
        }
        let accepted = state.begin_ssh_modal_action(action.as_str());
        let pending_action = state.take_pending_ssh_modal_action();
        let mut did_mutate = false;
        let mut catalog_persisted_in_action = false;

        if let Some(request) = pending_action {
            match request.action {
                SshModalAction::Save => {
                    let previous_state = (*state).clone();
                    let existing_saved_spec = match &state.asset_modal_state {
                        Some(AssetModalState::NewSshConnection {
                            editing_asset_id: Some(asset_id),
                            ..
                        }) => state
                            .console_asset_tree()
                            .ssh_connection_spec(asset_id)
                            .cloned(),
                        _ => None,
                    };
                    did_mutate = state.confirm_asset_modal();
                    if !did_mutate {
                        state.finish_ssh_modal_action_error("Failed to save connection.");
                    } else if let Some(asset_id) = state.focused_asset_id.clone() {
                        if let Err(err) = validate_saved_modal_profile(&state, &asset_id) {
                            *state = previous_state;
                            did_mutate = false;
                            state.finish_ssh_modal_action_error(err.to_string());
                        } else if let Some(saved_spec) =
                            state
                                .console_asset_tree()
                                .ssh_connection_spec(&asset_id)
                                .cloned()
                            && let Err(err) = sync_saved_ssh_secrets(
                                credential_store_ref.as_ref(),
                                &request.draft,
                                existing_saved_spec.as_ref(),
                                &saved_spec,
                            )
                        {
                            *state = previous_state;
                            did_mutate = false;
                            state.finish_ssh_modal_action_error(err.to_string());
                        }
                    }
                }
                SshModalAction::TestConnection => {
                    match runtime_profile_for_modal_action(&state, &request.draft) {
                        Ok(profile) => {
                            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                                queue_modal_test_connection(
                                    &session_bridge.manager,
                                    session_bridge.manager.runtime_handle(),
                                    &ssh_modal_result_tx_ref,
                                    &next_ssh_modal_test_request_id_ref,
                                    &active_ssh_modal_test_request_id_ref,
                                    profile,
                                );
                            } else {
                                state.finish_ssh_modal_action_error(
                                    "SSH session bridge is unavailable.",
                                );
                            }
                        }
                        Err(err) => state.finish_ssh_modal_action_error(err.to_string()),
                    }
                }
                SshModalAction::Connect => {
                    match runtime_profile_for_modal_action(&state, &request.draft) {
                        Ok(mut profile) => {
                            profile.asset_id = Some(temporary_session_asset_id_for_profile(&profile));
                            if let Some(session_bridge) = session_bridge_ref.as_ref() {
                                if let Err(err) = attempt_open_session_with_profile(
                                    &mut state,
                                    session_bridge.as_ref(),
                                    &pending_host_key_approval_ref,
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
                    }
                }
                SshModalAction::SaveAndConnect => {
                    let previous_state = (*state).clone();
                    let existing_saved_spec = match &state.asset_modal_state {
                        Some(AssetModalState::NewSshConnection {
                            editing_asset_id: Some(asset_id),
                            ..
                        }) => state
                            .console_asset_tree()
                            .ssh_connection_spec(asset_id)
                            .cloned(),
                        _ => None,
                    };
                    did_mutate = state.confirm_asset_modal();
                    if did_mutate {
                        if let Some(asset_id) = state.focused_asset_id.clone() {
                            if let Err(err) = validate_saved_modal_profile(&state, &asset_id) {
                                *state = previous_state;
                                did_mutate = false;
                                state.finish_ssh_modal_action_error(err.to_string());
                            } else if let Some(saved_spec) = state
                                .console_asset_tree()
                                .ssh_connection_spec(&asset_id)
                                .cloned()
                            {
                                if let Err(err) = sync_saved_ssh_secrets(
                                    credential_store_ref.as_ref(),
                                    &request.draft,
                                    existing_saved_spec.as_ref(),
                                    &saved_spec,
                                ) {
                                    *state = previous_state;
                                    did_mutate = false;
                                    state.finish_ssh_modal_action_error(err.to_string());
                                } else {
                                    if let Some(repo) = asset_repo_ref.as_ref()
                                        && let Err(err) = save_asset_catalog(repo.as_ref(), &state)
                                    {
                                        *state = previous_state;
                                        did_mutate = false;
                                        state.finish_ssh_modal_action_error(err.to_string());
                                    } else {
                                        catalog_persisted_in_action = asset_repo_ref.is_some();
                                        match runtime_profile_for_saved_asset(&state, &asset_id) {
                                            Ok(profile) => {
                                                if let Some(session_bridge) = session_bridge_ref.as_ref()
                                                    && let Err(err) = attempt_open_session_with_profile(
                                                        &mut state,
                                                        session_bridge.as_ref(),
                                                        &pending_host_key_approval_ref,
                                                        profile,
                                                        OpenSessionMode::ActivateExisting,
                                                    )
                                                {
                                                    tracing::error!(
                                                        target: "app.ssh",
                                                        error = %err,
                                                        "failed to open ssh session from modal action"
                                                    );
                                                    state.finish_ssh_modal_action_error(err.to_string());
                                                } else {
                                                    state.cancel_asset_modal();
                                                }
                                            }
                                            Err(err) => {
                                                state.finish_ssh_modal_action_error(err.to_string());
                                            }
                                        }
                                    }
                                }
                            } else {
                                *state = previous_state;
                                did_mutate = false;
                                state.finish_ssh_modal_action_error(
                                    "Failed to resolve saved secret target after saving connection.",
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

        if did_mutate && !catalog_persisted_in_action {
            save_asset_catalog_if_available(&asset_repo_ref, &state);
        }
        if did_mutate {
            let mut vault = vault_session_ref.borrow_mut();
            vault_sync::mark_local_vault_dirty_and_arm_sync(
                &mut state,
                &mut vault,
                &vault_sync_service_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_assets_context_menu_state(&window, &state);
        sync_asset_modal_state(&window, &state);
        windowing::sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let modal_drag_state_ref = Rc::clone(modal_drag_state);
    let pending_host_key_approval_ref = Rc::clone(pending_host_key_approval);
    let session_bridge_ref = session_bridge.clone();
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let ssh_modal_result_tx_ref = ssh_modal_result_tx.clone();
    let next_ssh_modal_test_request_id_ref = Rc::clone(next_ssh_modal_test_request_id);
    let active_ssh_modal_test_request_id_ref = Rc::clone(active_ssh_modal_test_request_id);
    window.on_ssh_host_key_modal_accept_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        modal_drag_state_ref.borrow_mut().take();
        state.accept_ssh_host_key_prompt();
        resolve_pending_host_key(
            &mut state,
            session_bridge_ref.as_deref(),
            &pending_host_key_approval_ref,
            &ssh_modal_result_tx_ref,
            &next_ssh_modal_test_request_id_ref,
            &active_ssh_modal_test_request_id_ref,
            true,
        );
        window.set_blocking_modal_offset_x(0.0);
        window.set_blocking_modal_offset_y(0.0);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        windowing::sync_ssh_host_key_modal_state(&window, &state);
        sync_asset_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let private_key_importer_ref = Arc::clone(private_key_importer);
    window.on_keychain_ssh_key_modal_action_requested(move |action| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let result = match action.as_str() {
            "import-private-key" => import_private_key_into_keychain_modal(
                &mut state,
                private_key_importer_ref.as_ref(),
            ),
            "import-public-key" => {
                import_public_key_into_keychain_modal(&mut state, private_key_importer_ref.as_ref())
            }
            "paste-private-key" => {
                paste_private_key_into_keychain_modal(&mut state);
                Ok(())
            }
            "paste-public-key" => {
                paste_public_key_into_keychain_modal(&mut state);
                Ok(())
            }
            "generate-key-pair" => generate_key_pair_into_keychain_modal(&mut state),
            "copy-public-key" => copy_public_key_from_keychain_modal(&state),
            _ => Ok(()),
        };
        if let Err(err) = result {
            tracing::error!(
                target: "app.keychain",
                action = action.as_str(),
                error = %err,
                "failed to handle keychain SSH key modal action"
            );
        }
        sync_asset_modal_state(&window, &state);
    });
    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(pending_host_key_approval);
    let asset_click_tracker_ref = Rc::clone(asset_click_tracker);
    let pending_double_click_activation_ref = Rc::clone(pending_double_click_activation);
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_asset_selected(move |item_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.select_keychain_item(item_id.as_str());
            asset_click_tracker_ref.borrow_mut().take();
            pending_double_click_activation_ref.borrow_mut().take();
        } else {
            state.select_asset(item_id.as_str());
            let should_activate =
                register_asset_click(&asset_click_tracker_ref, item_id.as_str(), Instant::now());
            if should_activate {
                pending_double_click_activation_ref
                    .borrow_mut()
                    .replace(item_id.to_string());
                activate_asset(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &pending_host_key_approval_ref,
                    item_id.as_str(),
                );
                apply_pending_snippet_activation(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
                save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_assets_context_menu_state(&window, &state);
        windowing::sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let pending_host_key_approval_ref = Rc::clone(pending_host_key_approval);
    let asset_click_tracker_ref = Rc::clone(asset_click_tracker);
    let pending_double_click_activation_ref = Rc::clone(pending_double_click_activation);
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_asset_activated(move |item_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            asset_click_tracker_ref.borrow_mut().take();
            pending_double_click_activation_ref.borrow_mut().take();
            state.select_keychain_item(item_id.as_str());
        } else {
            asset_click_tracker_ref.borrow_mut().take();
            state.select_asset(item_id.as_str());
            let skip_duplicate = pending_double_click_activation_ref
                .borrow()
                .as_ref()
                .map(|asset_id| asset_id == item_id.as_str())
                .unwrap_or(false);
            pending_double_click_activation_ref.borrow_mut().take();
            if !skip_duplicate {
                activate_asset(
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &pending_host_key_approval_ref,
                    item_id.as_str(),
                );
                apply_pending_snippet_activation(
                    &window,
                    &mut state,
                    session_bridge_ref.as_deref(),
                    &mut workspace_follow_tracker_ref.borrow_mut(),
                );
                save_quick_launch_preferences_from_state(&quick_launch_store_ref, &state);
            }
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        sync_assets_context_menu_state(&window, &state);
        windowing::sync_ssh_host_key_modal_state(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_toggle_expanded_requested(move |item_id| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.active_sidebar_destination == SidebarDestination::Keychain {
            state.toggle_keychain_folder_expanded(item_id.as_str());
        } else {
            state.toggle_folder_expanded(item_id.as_str());
        }
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_asset_context_menu_requested(move |target_id, target_kind, anchor_x, anchor_y| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let active_sidebar_destination = state.active_sidebar_destination;
        state.dismiss_empty_asset_search_on_shell_interaction();
        state.open_context_menu_for_target(
            parse_context_target_kind(target_kind.as_str(), active_sidebar_destination),
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

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_shell_interaction_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        if state.dismiss_empty_asset_search_on_shell_interaction() {
            sync_assets_toolbar_state(&window, &state);
            sync_console_assets(&window, &state);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let session_runtime_guard_ref = session_runtime_guard.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let pending_host_key_approval_ref = Rc::clone(pending_host_key_approval);
    let credential_store_ref = Arc::clone(credential_store);
    let workspace_follow_tracker_ref = Rc::clone(workspace_follow_tracker);
    let keychain_repo_ref = keychain_repo.clone();
    let vault_session_ref = Rc::clone(vault_session);
    let vault_sync_service_ref = Rc::clone(vault_sync_service);
    let vault_auto_sync_timer_ref = Rc::clone(vault_auto_sync_timer);
    let run_vault_sync_ref = Rc::clone(run_vault_sync);
    let sftp_async_runtime_ref = sftp_async_runtime.clone();
    let sftp_result_tx_ref = sftp_result_tx.clone();
    let sftp_transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    let quick_launch_store_ref = quick_launch_store.clone();
    window.on_assets_context_menu_action_invoked(move |action_id| {
        let _keep_runtime_alive = &session_runtime_guard_ref;
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_modal_open = state.asset_modal_state.is_some();
        let was_sftp_remote_file_modal_open = state.sftp_remote_file_editor_state().open;
        let keychain_node_count_before = state.keychain_catalog().nodes.len();

        if let Some((path, action)) = context_menu_action_entry_for(&state, action_id.as_str()) {
            if !action.children.is_empty() {
                state.set_context_menu_open_path(path);
            } else if action.state == ContextMenuActionState::Enabled {
                match action_id.as_str() {
                    "open-connection" => {
                        let target_asset_id = state.context_target_asset_id.clone();
                        state.close_context_menu();
                        if let Some(asset_id) = target_asset_id {
                            activate_asset(
                                &mut state,
                                session_bridge_ref.as_deref(),
                                &pending_host_key_approval_ref,
                                &asset_id,
                            );
                            save_quick_launch_preferences_from_state(
                                &quick_launch_store_ref,
                                &state,
                            );
                        }
                    }
                    _ => state.handle_context_menu_leaf_action(action_id.as_str()),
                }
            } else {
                state.handle_context_menu_leaf_action(action_id.as_str());
            }
        }
        {
            let mut sftp_browser_controller = sftp_browser_controller_ref.borrow_mut();
            let _ = super::sftp::apply_pending_sftp_context_action(
                &mut state,
                session_bridge_ref.as_deref(),
                &mut sftp_browser_controller,
                sftp_async_runtime_ref.as_ref(),
                &sftp_result_tx_ref,
                &sftp_transfer_result_tx_ref,
            );
        }
        if state.keychain_catalog().nodes.len() > keychain_node_count_before {
            save_keychain_catalog_if_available(&keychain_repo_ref, &state);
            let mut vault = vault_session_ref.borrow_mut();
            vault_sync::mark_local_vault_dirty_and_arm_sync(
                &mut state,
                &mut vault,
                &vault_sync_service_ref,
                &vault_auto_sync_timer_ref,
                Rc::clone(&run_vault_sync_ref),
            );
        }

        apply_pending_snippet_activation(
            &window,
            &mut state,
            session_bridge_ref.as_deref(),
            &mut workspace_follow_tracker_ref.borrow_mut(),
        );
        hydrate_edit_ssh_modal_secret_from_store(&mut state, credential_store_ref.as_ref());
        hydrate_edit_keychain_identity_secret_from_store(&mut state, credential_store_ref.as_ref());
        hydrate_edit_keychain_ssh_key_secret_from_store(&mut state, credential_store_ref.as_ref());
        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        sync_keychain_assets(&window, &state);
        sync_workspace_tabs_with_manager(
            &window,
            &state,
            &mut workspace_follow_tracker_ref.borrow_mut(),
            session_bridge_ref.as_deref().map(|bridge| &bridge.manager),
        );
        sync_workspace_terminal_runtime_defaults(&window, session_bridge_ref.as_deref());
        schedule_workspace_terminal_runtime_defaults_sync(
            &window,
            session_bridge_ref
                .as_ref()
                .map(|bridge| bridge.terminal_defaults.clone()),
        );
        super::sftp::sync_right_panel_state(&window, &mut state);
        sync_asset_modal_state(&window, &state);
        super::sftp::sync_sftp_remote_file_modal_state(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
        windowing::sync_ssh_host_key_modal_state(&window, &state);
        if !was_modal_open && state.asset_modal_state.is_some() {
            schedule_asset_modal_focus(&window);
        }
        if !was_sftp_remote_file_modal_open && state.sftp_remote_file_editor_state().open {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    let session_bridge_ref = session_bridge.clone();
    let sftp_browser_controller_ref = Rc::clone(sftp_browser_controller);
    let sftp_async_runtime_ref = sftp_async_runtime.clone();
    let sftp_result_tx_ref = sftp_result_tx.clone();
    let sftp_transfer_result_tx_ref = sftp_transfer_result_tx.clone();
    window.on_assets_context_menu_key_pressed(move |command| {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        let was_sftp_remote_file_modal_open = state.sftp_remote_file_editor_state().open;

        match command.as_str() {
            "escape" => state.handle_context_menu_escape(),
            "left" => state.navigate_context_menu_left(),
            "right" => state.navigate_context_menu_right(),
            "enter" => state.invoke_current_context_menu_item(),
            _ => {}
        }
        {
            let mut sftp_browser_controller = sftp_browser_controller_ref.borrow_mut();
            let _ = super::sftp::apply_pending_sftp_context_action(
                &mut state,
                session_bridge_ref.as_deref(),
                &mut sftp_browser_controller,
                sftp_async_runtime_ref.as_ref(),
                &sftp_result_tx_ref,
                &sftp_transfer_result_tx_ref,
            );
        }

        sync_assets_toolbar_state(&window, &state);
        sync_console_assets(&window, &state);
        super::sftp::sync_right_panel_state(&window, &mut state);
        super::sftp::sync_sftp_remote_file_modal_state(&window, &state);
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
        if !was_sftp_remote_file_modal_open && state.sftp_remote_file_editor_state().open {
            schedule_asset_modal_focus(&window);
        }
    });

    let state = Rc::clone(view_model);
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

    let state = Rc::clone(view_model);
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

    let state = Rc::clone(view_model);
    let handle = window.as_weak();
    window.on_close_assets_context_menu_requested(move || {
        let window = handle.unwrap();
        let mut state = state.borrow_mut();
        state.close_context_menu();
        update_context_menu_placement(&window, &mut state);
        sync_assets_context_menu_state(&window, &state);
    });
}
