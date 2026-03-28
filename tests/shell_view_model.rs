//! Stateful shell view-model coverage for toolbar, sidebar, window toggles, and asset explorer state.

use std::fs;

use mica_term::app::window_state::WindowPlacementKind;
use mica_term::shell::assets::{
    AssetNameValidation, AssetNodePayload, AssetSocks5ProxySpec, AssetSshConnectionSpec,
    AssetSshProxySpec, AssetTree, AssetViewMode, ConsoleAssetKind,
};
use mica_term::shell::context_menu::{
    ContextTargetKind, SelectionContext, resolve_action_tree, visible_columns_for_path,
};
use mica_term::shell::sidebar::SidebarDestination;
use mica_term::shell::view_model::{
    AssetModalState, ShellViewModel, SshModalAction, SshModalActionState, WelcomeAction,
    welcome_actions,
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
fn asset_modal_backdrop_click_does_not_dismiss_blocking_modal() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        app_window.contains("asset-modal-dismiss-layer := TouchArea"),
        "app window should still project a modal backdrop interception layer"
    );
    assert!(
        app_window.contains("clicked => {\n        }") || app_window.contains("clicked => { }"),
        "blocking modal backdrop should intercept clicks without dismissing the modal"
    );
}

#[test]
fn new_ssh_modal_state_no_longer_tracks_top_level_tab_enum() {
    let view_model = fs::read_to_string("src/shell/view_model.rs").expect("read shell view model");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        !view_model.contains("pub enum AssetSshModalTab"),
        "ssh modal state should stop exposing a top-level tab enum"
    );
    assert!(
        !view_model.contains("active_tab: AssetSshModalTab"),
        "new ssh modal state should no longer carry active tab state"
    );
    assert!(
        !app_window.contains("asset-ssh-modal-active-tab"),
        "window state bridge should not project a top-level ssh modal tab property"
    );
    assert!(
        !app_window.contains("callback asset-ssh-modal-tab-selected(string);"),
        "window callback bridge should not expose top-level ssh modal tab selection"
    );
}

#[test]
fn new_ssh_modal_is_a_grouped_single_page_form() {
    let ssh_modal = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(
        !ssh_modal.contains("\"Standard\""),
        "ssh modal should not keep the legacy Standard tab in the grouped layout"
    );
    assert!(
        !ssh_modal.contains("\"Environment\""),
        "ssh modal should not keep the legacy Environment tab in the grouped layout"
    );
    assert!(
        !ssh_modal.contains("\"Advanced\""),
        "ssh modal should not keep the legacy Advanced tab in the grouped layout"
    );
    assert!(
        !ssh_modal
            .contains("Leave password / private key / passphrase blank to keep the saved secret."),
        "ssh modal should stop advertising leave-blank secret retention copy"
    );
    assert!(
        !ssh_modal.contains("Clear Saved Secret"),
        "ssh modal should remove the legacy clear-secret affordance"
    );
    assert!(ssh_modal.contains("label: \"Password\""));
    assert!(ssh_modal.contains("text: \"Proxy\""));
    assert!(ssh_modal.contains("text: \"Proxy Type\""));
    assert!(
        ssh_modal.contains("trailing-action-text: root.password-visible ? \"Hide\" : \"Show\"")
    );
}

#[test]
fn esc_closes_standard_asset_modals_but_host_key_prompt_remains_explicit_reject_path() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(
        app_window.contains("root.close-asset-modal-requested();"),
        "standard asset modals should still close via the shared close path"
    );
    assert!(
        app_window.contains("root.ssh-host-key-modal-reject-requested();"),
        "host key prompt should keep an explicit reject path"
    );
    assert!(
        !app_window.contains("asset-modal-dismiss-layer := TouchArea {\n        x: 0px;\n        y: titlebar.height;\n        width: root.width;\n        height: root.height - titlebar.height;\n        enabled: root.asset-modal-open || root.asset-rename-modal-open || root.asset-delete-confirm-modal-open || root.ssh-host-key-modal-open;\n\n        clicked => {\n            if root.ssh-host-key-modal-open {\n                root.ssh-host-key-modal-reject-requested();\n            } else {\n                root.close-asset-modal-requested();\n            }\n        }\n    }"),
        "backdrop clicks must not route host-key rejection or standard modal close anymore"
    );
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
        }) if draft_name == "Folder 1"
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
        }) if draft_name == "Folder 1"
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
fn new_ssh_modal_defaults_proxy_type_to_none() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.proxy_type == "none"
                && draft.proxy_socks5_host.is_empty()
                && draft.proxy_socks5_port.is_empty()
                && draft.proxy_socks5_username.is_empty()
                && draft.proxy_socks5_password.is_empty()
                && !draft.proxy_socks5_password_visible
                && draft.proxy_ssh_asset_id.is_empty()
    ));
}

#[test]
fn selecting_socks5_proxy_updates_only_socks5_draft_fields() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.update_ssh_modal_field("proxy_ssh_asset_id", "asset-upstream".into());
    view_model.update_ssh_modal_field("proxy_type", "socks5".into());
    view_model.update_ssh_modal_field("proxy_socks5_host", "proxy.example.net".into());
    view_model.update_ssh_modal_field("proxy_socks5_port", "1080".into());
    view_model.update_ssh_modal_field("proxy_socks5_username", "ops-proxy".into());
    view_model.update_ssh_modal_field("proxy_socks5_password", "proxy-secret".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.proxy_type == "socks5"
                && draft.proxy_socks5_host == "proxy.example.net"
                && draft.proxy_socks5_port == "1080"
                && draft.proxy_socks5_username == "ops-proxy"
                && draft.proxy_socks5_password == "proxy-secret"
                && draft.password == "secret"
                && draft.proxy_ssh_asset_id == "asset-upstream"
    ));
}

#[test]
fn selecting_ssh_asset_proxy_stores_upstream_asset_id() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_field("proxy_type", "ssh-asset".into());
    view_model.update_ssh_modal_field("proxy_ssh_asset_id", "asset-bastion".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.proxy_type == "ssh-asset" && draft.proxy_ssh_asset_id == "asset-bastion"
    ));
}

#[test]
fn ssh_modal_validation_requires_http_proxy_host_and_port() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_field("host", "10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.update_ssh_modal_field("proxy_type", "http".into());

    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("proxy_socks5_host", "proxy.example.net".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("proxy_socks5_port", "8080".into());
    assert!(view_model.can_confirm_asset_modal());
}

#[test]
fn editing_connection_cannot_proxy_through_itself() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Target Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.41".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: String::new(),
            credential_ref: None,
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id.clone());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.update_ssh_modal_field("proxy_type", "ssh-asset".into());
    view_model.update_ssh_modal_field("proxy_ssh_asset_id", asset_id);

    assert!(!view_model.can_confirm_asset_modal());
}

#[test]
fn switching_proxy_type_clears_stale_validation_text() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);
    assert!(!view_model.begin_ssh_modal_action("save"));
    view_model.update_ssh_modal_field("proxy_type", "socks5".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.proxy_type == "socks5" && draft.validation_message.is_empty()
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
            ref draft,
            ..
        }) if draft.name == "SSH Connection 1"
            && draft.host.is_empty()
            && draft.port == "22"
    ));
}

#[test]
fn confirming_new_ssh_modal_requires_host_user_and_password_by_default() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_host("10.0.0.12".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("user", "ops".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("password", "secret".into());
    assert!(view_model.can_confirm_asset_modal());
}

#[test]
fn ssh_modal_default_draft_starts_with_password_auth_and_port_22() {
    let mut view_model = ShellViewModel::default();

    view_model.open_new_ssh_modal(None);

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.auth_method == "password"
            && draft.port == "22"
            && draft.private_key_source == "content"
    ));
}

#[test]
fn ssh_modal_validation_requires_name_host_user_and_active_auth_payload() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);

    view_model.update_ssh_modal_field("host", "10.0.0.12".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("user", "ops".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("password", "secret".into());
    assert!(view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("auth_method", "private-key".into());
    view_model.update_ssh_modal_field("password", "".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("private_key_source", "path".into());
    view_model.update_ssh_modal_field("private_key_path", "/tmp/id_ed25519".into());
    assert!(view_model.can_confirm_asset_modal());
}

#[test]
fn switching_auth_method_clears_irrelevant_validation_errors() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_field("host", "10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());

    view_model.update_ssh_modal_field("auth_method", "private-key".into());
    view_model.update_ssh_modal_field("private_key_source", "content".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field("password", "stale-password".into());
    assert!(!view_model.can_confirm_asset_modal());

    view_model.update_ssh_modal_field(
        "private_key_content",
        "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
    );
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
            ref draft,
            ..
        })
            if draft.name == "SSH Connection 1"
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
    assert_eq!(view_model.focused_asset_id.as_deref(), Some("ssh-prod-01"));
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
fn new_ssh_modal_prefills_next_dash_suffix_name() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::SshConnection, "SSH Connection 1");

    view_model.open_new_ssh_modal(None);

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.name == "SSH Connection 1-1"
    ));
    assert!(!view_model.can_confirm_asset_modal());
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
fn new_folder_modal_prefills_next_dash_suffix_name() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");

    view_model.open_new_folder_modal(None);

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewFolder { ref draft_name, .. })
            if draft_name == "Folder 1-1"
    ));
    assert!(view_model.can_confirm_asset_modal());
}

#[test]
fn create_validation_rejects_duplicate_name_across_kinds() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");

    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());

    assert_eq!(
        view_model.asset_create_modal_validation_message(),
        "Name already exists in this folder."
    );
    assert!(!view_model.asset_create_modal_can_confirm());
}

#[test]
fn conflicting_manual_input_keeps_user_text_and_disables_confirm() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::SshConnection, "SSH Connection 1");

    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("SSH Connection 1".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].label, "SSH Connection 1");
    assert_eq!(
        view_model.asset_create_modal_validation_message(),
        "Name already exists in this folder."
    );
    assert!(!view_model.asset_create_modal_can_confirm());
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.name == "SSH Connection 1"
    ));
}

#[test]
fn connect_action_creates_temporary_session_request_without_persisting_asset() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    assert!(view_model.begin_ssh_modal_action("connect"));

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(matches!(
        view_model.pending_ssh_modal_action(),
        Some(request) if request.action == SshModalAction::Connect
    ));
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { .. })
    ));
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Busy(SshModalAction::Connect)
    ));
}

#[test]
fn test_connection_action_does_not_create_workspace_tab() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    assert!(view_model.begin_ssh_modal_action("test"));
    assert!(matches!(
        view_model.pending_ssh_modal_action(),
        Some(request) if request.action == SshModalAction::TestConnection
    ));
    assert!(view_model.workspace_tabs().is_empty());
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Busy(SshModalAction::TestConnection)
    ));
}

#[test]
fn save_action_records_save_request_without_creating_asset_or_workspace_tab() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    assert!(view_model.begin_ssh_modal_action("save"));

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert!(view_model.workspace_tabs().is_empty());
    assert!(matches!(
        view_model.pending_ssh_modal_action(),
        Some(request) if request.action == SshModalAction::Save
    ));
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Busy(SshModalAction::Save)
    ));
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { .. })
    ));
}

#[test]
fn saving_private_key_path_with_passphrase_assigns_saved_credential_ref() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("auth_method", "private-key".into());
    view_model.update_ssh_modal_field("private_key_source", "path".into());
    view_model.update_ssh_modal_field("private_key_path", "/tmp/id_ed25519".into());
    view_model.update_ssh_modal_field("passphrase", "hunter2".into());

    assert!(view_model.confirm_asset_modal());

    let rows = view_model.visible_console_asset_rows();
    let asset_id = rows[0].id.clone();
    let spec = view_model
        .console_asset_tree()
        .ssh_connection_spec(asset_id.as_str())
        .expect("saved ssh spec");

    assert_eq!(spec.auth_method, "private-key");
    assert_eq!(spec.private_key_source, "path");
    assert_eq!(spec.private_key_path, "/tmp/id_ed25519");
    assert_eq!(
        spec.credential_ref.as_deref(),
        Some(format!("ssh/saved-secrets/{asset_id}").as_str())
    );
}

#[test]
fn ssh_modal_accepts_full_connect_action_family() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    for (action_id, expected) in [
        ("save", SshModalAction::Save),
        ("connect", SshModalAction::Connect),
        ("test", SshModalAction::TestConnection),
        ("save-and-connect", SshModalAction::SaveAndConnect),
    ] {
        assert!(view_model.begin_ssh_modal_action(action_id));
        assert!(matches!(
            view_model.pending_ssh_modal_action(),
            Some(request) if request.action == expected
        ));
        assert!(matches!(
            view_model.ssh_modal_action_state(),
            SshModalActionState::Busy(action) if *action == expected
        ));
        view_model.finish_ssh_modal_action_error("reset between action ids");
    }
}

#[test]
fn invalid_draft_disables_connect_family_actions() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);

    assert!(!view_model.ssh_modal_connect_family_enabled());
    assert!(!view_model.begin_ssh_modal_action("test"));
    assert!(!view_model.begin_ssh_modal_action("connect"));
    assert!(!view_model.begin_ssh_modal_action("save-and-connect"));
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Idle
    ));
    assert_eq!(
        view_model.asset_create_modal_validation_message(),
        "Host is required."
    );
}

#[test]
fn connect_family_enablement_depends_on_connection_minimum_fields() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);

    assert!(!view_model.ssh_modal_connect_family_enabled());

    view_model.update_ssh_modal_field("host", "10.0.0.12".into());
    assert!(!view_model.ssh_modal_connect_family_enabled());

    view_model.update_ssh_modal_field("user", "ops".into());
    assert!(!view_model.ssh_modal_connect_family_enabled());

    view_model.update_ssh_modal_field("password", "secret".into());
    assert!(view_model.ssh_modal_connect_family_enabled());

    view_model.begin_ssh_modal_action("test");
    assert!(!view_model.ssh_modal_connect_family_enabled());
}

#[test]
fn busy_action_blocks_duplicate_ssh_modal_submissions() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    assert!(view_model.begin_ssh_modal_action("connect"));
    assert!(!view_model.begin_ssh_modal_action("save-and-connect"));
    assert!(matches!(
        view_model.pending_ssh_modal_action(),
        Some(request) if request.action == SshModalAction::Connect
    ));
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Busy(SshModalAction::Connect)
    ));
}

#[test]
fn beginning_modal_action_marks_state_busy_until_result_is_applied() {
    let mut view_model = ShellViewModel::default();
    view_model.open_new_ssh_modal(None);
    view_model.update_ssh_modal_name("Prod Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());

    assert!(view_model.begin_ssh_modal_action("test"));
    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Busy(SshModalAction::TestConnection)
    ));
    assert_eq!(
        view_model.ssh_modal_feedback_message(),
        "Testing connection..."
    );

    view_model.finish_ssh_modal_action_success("Connection test completed.");

    assert!(matches!(
        view_model.ssh_modal_action_state(),
        SshModalActionState::Success(message) if message == "Connection test completed."
    ));
    assert_eq!(
        view_model.ssh_modal_feedback_message(),
        "Connection test completed."
    );
    assert!(view_model.pending_ssh_modal_action().is_none());
}

#[test]
fn unchanged_rename_value_is_treated_as_valid() {
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root(ConsoleAssetKind::Folder, "Prod");

    assert_eq!(
        tree.validate_name_in_parent(None, "Prod", Some(asset_id.as_str())),
        AssetNameValidation::Valid
    );
}

#[test]
fn editing_existing_name_to_original_value_remains_valid() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_rename_asset_modal(asset_id);
    view_model.update_rename_asset_modal_name("Prod".into());

    assert!(view_model.can_confirm_asset_modal());
    assert_eq!(view_model.asset_rename_modal_validation_message(), "");
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
fn edit_connection_opens_modal_with_prefilled_non_secret_fields() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::SshAsset {
                asset_id: "asset-upstream".into(),
            },
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/private-key/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id.clone());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(ref editing_asset_id),
            ref draft,
            ..
        }) if editing_asset_id == &asset_id
            && draft.name == "Prod Bastion"
            && draft.host == "10.0.0.12"
            && draft.user == "ops"
            && draft.port == "2022"
            && draft.auth_method == "private-key"
            && draft.private_key_source == "path"
            && draft.private_key_path == "/tmp/id_ed25519"
            && draft.environment == "prod"
            && draft.proxy_type == "ssh-asset"
            && draft.proxy_ssh_asset_id == "asset-upstream"
            && draft.remark == "Primary entry point"
            && draft.password.is_empty()
            && draft.private_key_content.is_empty()
            && draft.passphrase.is_empty()
    ));
}

#[test]
fn edit_connection_opens_modal_with_prefilled_socks5_proxy_fields() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::Socks5(AssetSocks5ProxySpec {
                host: "proxy.example.net".into(),
                port: "1080".into(),
                username: "ops-proxy".into(),
                password_credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
            }),
            proxy_method: String::new(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/private-key/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id.clone());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(ref editing_asset_id),
            ref draft,
            ..
        }) if editing_asset_id == &asset_id
            && draft.proxy_type == "socks5"
            && draft.proxy_socks5_host == "proxy.example.net"
            && draft.proxy_socks5_port == "1080"
            && draft.proxy_socks5_username == "ops-proxy"
            && draft.proxy_socks5_password.is_empty()
    ));
}

#[test]
fn editing_saved_path_modal_switches_to_inline_content_when_private_key_content_is_supplied() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "private-key".into(),
            private_key_source: "path".into(),
            private_key_path: "/tmp/id_ed25519".into(),
            environment: String::new(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "Legacy path asset".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id.clone());
    view_model.update_ssh_modal_field(
        "private_key_content",
        "-----BEGIN OPENSSH PRIVATE KEY-----".into(),
    );

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(ref editing_asset_id),
            ref draft,
            ..
        }) if editing_asset_id == &asset_id
            && draft.auth_method == "private-key"
            && draft.private_key_source == "content"
            && draft.private_key_content == "-----BEGIN OPENSSH PRIVATE KEY-----"
    ));
}

#[test]
fn editing_saved_ssh_modal_keeps_password_fields_hidden_until_secret_hydration_exists() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: String::new(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "Saved credential".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id);

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.password.is_empty()
                && draft.private_key_content.is_empty()
                && draft.passphrase.is_empty()
                && !draft.password_visible
    ));
}

#[test]
fn editing_saved_ssh_modal_allows_direct_password_editing_after_secret_hydration() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: String::new(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "Saved credential".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id);
    view_model.hydrate_edit_ssh_modal_secret(Some("secret".into()), None, None, None);
    view_model.update_ssh_modal_field("password", "rotated-secret".into());

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection { ref draft, .. })
            if draft.password == "rotated-secret"
                && draft.private_key_content.is_empty()
                && draft.passphrase.is_empty()
                && !draft.password_visible
    ));
    assert!(view_model.asset_create_modal_can_confirm());
}

#[test]
fn hydrating_edit_ssh_modal_secret_updates_active_draft_and_inline_error() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "22".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: String::new(),
            proxy: AssetSshProxySpec::None,
            proxy_method: String::new(),
            remark: "Saved credential".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);

    view_model.open_edit_ssh_modal(asset_id.clone());
    view_model.hydrate_edit_ssh_modal_secret(
        Some("secret".into()),
        None,
        None,
        Some("missing keyring entry".into()),
    );

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(ref editing_asset_id),
            ref draft,
            ..
        }) if editing_asset_id == &asset_id
            && draft.password == "secret"
            && draft.private_key_content.is_empty()
            && draft.passphrase.is_empty()
            && !draft.password_visible
            && draft.validation_message == "missing keyring entry"
    ));
}

#[test]
fn edit_connection_context_action_routes_to_ssh_edit_modal() {
    let mut view_model = ShellViewModel::default();
    let mut tree = AssetTree::new();
    let asset_id = tree.insert_root_with_payload(
        ConsoleAssetKind::SshConnection,
        "Prod Bastion",
        AssetNodePayload::SshConnection(AssetSshConnectionSpec {
            host: "10.0.0.12".into(),
            user: "ops".into(),
            port: "2022".into(),
            auth_method: "password".into(),
            private_key_source: "content".into(),
            private_key_path: String::new(),
            environment: "prod".into(),
            proxy: AssetSshProxySpec::None,
            proxy_method: "jump-host".into(),
            remark: "Primary entry point".into(),
            credential_ref: Some("ssh/saved-secrets/asset-prod".into()),
        }),
    );
    view_model.replace_console_asset_tree(tree);
    view_model.open_context_menu_for_target(
        ContextTargetKind::SshConnection,
        Some(asset_id.clone()),
        96.0,
        160.0,
    );

    view_model.handle_context_menu_leaf_action("edit-connection");

    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::NewSshConnection {
            editing_asset_id: Some(ref editing_asset_id),
            ..
        }) if editing_asset_id == &asset_id
    ));
}

#[test]
fn renaming_to_existing_default_name_uses_dash_suffix_after_base_collision() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 1");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Folder 2");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Infra");
    let asset_id = view_model.visible_console_asset_rows()[2].id.clone();
    view_model.begin_asset_rename_session(asset_id.clone(), "Infra".into());

    view_model.update_asset_rename_draft(&asset_id, "Folder 1".into());
    view_model.commit_asset_rename(&asset_id, "Folder 1".into());

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows[2].label, "Folder 1-1");
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

    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(asset_id.as_str())
    );
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
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.confirm_asset_modal();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, folder_id);
    assert_eq!(rows[1].depth, 1);
    assert_eq!(rows[1].kind, ConsoleAssetKind::SshConnection);
}

#[test]
fn deleting_selected_row_focuses_next_sibling_then_previous_then_parent() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Alpha");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Beta");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Gamma");

    let initial_rows = view_model.visible_console_asset_rows();
    let alpha_id = initial_rows[0].id.clone();
    let beta_id = initial_rows[1].id.clone();
    let gamma_id = initial_rows[2].id.clone();

    view_model.select_asset(&beta_id);
    assert!(view_model.remove_asset_subtree(&beta_id));
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(gamma_id.as_str())
    );
    assert_eq!(view_model.selected_asset_ids, vec![gamma_id.clone()]);

    view_model.select_asset(&gamma_id);
    assert!(view_model.remove_asset_subtree(&gamma_id));
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(alpha_id.as_str())
    );
    assert_eq!(view_model.selected_asset_ids, vec![alpha_id.clone()]);

    view_model.open_new_ssh_modal(Some(alpha_id.clone()));
    view_model.update_ssh_modal_name("Child".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.confirm_asset_modal();
    let child_id = view_model
        .visible_console_asset_rows()
        .into_iter()
        .find(|row| row.depth == 1 && row.label == "Child")
        .expect("child row should be projected")
        .id;

    view_model.select_asset(&child_id);
    assert!(view_model.remove_asset_subtree(&child_id));
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(alpha_id.as_str())
    );
    assert_eq!(view_model.selected_asset_ids, vec![alpha_id]);
}

#[test]
fn deleting_last_root_row_clears_focus_and_selection() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Solo");
    let solo_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.select_asset(&solo_id);
    assert!(view_model.remove_asset_subtree(&solo_id));

    assert!(view_model.visible_console_asset_rows().is_empty());
    assert_eq!(view_model.focused_asset_id, None);
    assert!(view_model.selected_asset_ids.is_empty());
}

#[test]
fn rename_context_action_opens_single_field_rename_modal() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(asset_id.clone()),
        96.0,
        144.0,
    );
    view_model.handle_context_menu_leaf_action("rename-asset");

    assert!(!view_model.context_menu_open);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::RenameAsset {
            asset_id: ref modal_asset_id,
            ref original_name,
            ref draft_name,
        }) if modal_asset_id == &asset_id
            && original_name == "Prod"
            && draft_name == "Prod"
    ));
}

#[test]
fn rename_modal_commit_updates_label_and_closes_modal() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let asset_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_rename_asset_modal(asset_id.clone());
    view_model.update_rename_asset_modal_name("Infra".into());
    view_model.confirm_asset_modal();

    let row = view_model
        .visible_console_asset_rows()
        .into_iter()
        .find(|row| row.id == asset_id)
        .expect("renamed row should still exist");
    assert_eq!(row.label, "Infra");
    assert!(view_model.asset_modal_state.is_none());
}

#[test]
fn rename_modal_duplicate_name_disables_confirm() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    view_model.seed_test_asset(ConsoleAssetKind::SshConnection, "Ops");
    let ops_id = view_model.visible_console_asset_rows()[1].id.clone();

    view_model.open_rename_asset_modal(ops_id);
    view_model.update_rename_asset_modal_name("Prod".into());

    assert!(!view_model.can_confirm_asset_modal());
}

#[test]
fn delete_context_action_opens_destructive_confirm_modal() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let folder_id = view_model.visible_console_asset_rows()[0].id.clone();

    view_model.open_new_ssh_modal(Some(folder_id.clone()));
    view_model.update_ssh_modal_name("Bastion".into());
    view_model.update_ssh_modal_host("10.0.0.12".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.confirm_asset_modal();

    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(folder_id.clone()),
        96.0,
        144.0,
    );
    view_model.handle_context_menu_leaf_action("delete-asset");

    assert!(!view_model.context_menu_open);
    assert!(matches!(
        view_model.asset_modal_state,
        Some(AssetModalState::DeleteAssetConfirm {
            asset_id: ref modal_asset_id,
            ref label,
            descendant_count,
        }) if modal_asset_id == &folder_id
            && label == "Prod"
            && descendant_count == 1
    ));
}

#[test]
fn confirming_folder_delete_removes_descendants_and_restores_focus() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Alpha");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Beta");
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Gamma");

    let initial_rows = view_model.visible_console_asset_rows();
    let beta_id = initial_rows[1].id.clone();
    let gamma_id = initial_rows[2].id.clone();

    view_model.open_new_ssh_modal(Some(beta_id.clone()));
    view_model.update_ssh_modal_name("Nested SSH".into());
    view_model.update_ssh_modal_host("10.0.0.13".into());
    view_model.update_ssh_modal_field("user", "ops".into());
    view_model.update_ssh_modal_field("password", "secret".into());
    view_model.confirm_asset_modal();

    view_model.select_asset(&beta_id);
    view_model.open_delete_asset_confirm(beta_id.clone());
    view_model.confirm_delete_asset();

    let rows = view_model.visible_console_asset_rows();
    assert_eq!(rows.len(), 2);
    assert!(
        rows.iter()
            .all(|row| row.label != "Beta" && row.label != "Nested SSH")
    );
    assert_eq!(
        view_model.focused_asset_id.as_deref(),
        Some(gamma_id.as_str())
    );
    assert_eq!(view_model.selected_asset_ids, vec![gamma_id]);
    assert!(view_model.asset_modal_state.is_none());
}

#[test]
fn replacing_console_asset_tree_reprojects_loaded_nodes_and_clears_runtime_session_state() {
    let mut view_model = ShellViewModel::default();
    view_model.seed_test_asset(ConsoleAssetKind::Folder, "Prod");
    let original_id = view_model.visible_console_asset_rows()[0].id.clone();
    view_model.select_asset(&original_id);
    view_model.open_context_menu_for_target(
        ContextTargetKind::Folder,
        Some(original_id),
        96.0,
        144.0,
    );

    let mut replacement = AssetTree::new();
    let imported_id = replacement.insert_root(ConsoleAssetKind::Folder, "Imported");

    view_model.replace_console_asset_tree(replacement);

    assert_eq!(view_model.console_asset_tree().root_ids(), &[imported_id]);
    assert_eq!(view_model.visible_console_asset_rows()[0].label, "Imported");
    assert!(view_model.selected_asset_ids.is_empty());
    assert_eq!(view_model.focused_asset_id, None);
    assert!(!view_model.context_menu_open);
    assert_eq!(view_model.context_target_asset_id, None);
}
