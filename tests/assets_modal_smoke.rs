use mica_term::AppWindow;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetPayload,
};
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_effects_and_asset_repo,
};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::Model;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::Result;

#[derive(Default)]
struct ModalAssetRepoState {
    save_attempts: Vec<PersistedAssetCatalog>,
}

struct RecordingModalAssetRepo {
    state: Rc<RefCell<ModalAssetRepoState>>,
}

impl RecordingModalAssetRepo {
    fn new(state: Rc<RefCell<ModalAssetRepoState>>) -> Self {
        Self { state }
    }
}

impl AssetCatalogRepository for RecordingModalAssetRepo {
    fn load(&self) -> Result<PersistedAssetCatalog> {
        Ok(PersistedAssetCatalog {
            schema_version: ASSET_CATALOG_SCHEMA_VERSION,
            root_ids: Vec::new(),
            nodes: BTreeMap::new(),
        })
    }

    fn save(&self, catalog: &PersistedAssetCatalog) -> Result<()> {
        self.state.borrow_mut().save_attempts.push(catalog.clone());
        Ok(())
    }
}

#[test]
fn folder_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-folder".into());
    app.set_asset_folder_modal_name("Infra".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Infra");
}

#[test]
fn ssh_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_active_tab("proxy".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "proxy");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
}

#[test]
fn ssh_modal_round_trips_standard_fields_and_auth_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("2222".into());
    app.set_asset_ssh_modal_auth_method("private-key".into());
    app.set_asset_ssh_modal_private_key_source("path".into());
    app.set_asset_ssh_modal_password("secret".into());
    app.set_asset_ssh_modal_remark("Primary entry point".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
    assert_eq!(app.get_asset_ssh_modal_user().as_str(), "ops");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "2222");
    assert_eq!(app.get_asset_ssh_modal_auth_method().as_str(), "private-key");
    assert_eq!(
        app.get_asset_ssh_modal_private_key_source().as_str(),
        "path"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert_eq!(app.get_asset_ssh_modal_remark().as_str(), "Primary entry point");
}

#[test]
fn ssh_modal_exposes_action_buttons_for_save_connect_test_and_save_connect() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let actions = Rc::new(RefCell::new(Vec::<String>::new()));
    let recorded_actions = Rc::clone(&actions);

    app.on_asset_ssh_modal_action_requested(move |action| {
        recorded_actions.borrow_mut().push(action.to_string());
    });

    app.invoke_asset_ssh_modal_action_requested("save".into());
    app.invoke_asset_ssh_modal_action_requested("connect".into());
    app.invoke_asset_ssh_modal_action_requested("test".into());
    app.invoke_asset_ssh_modal_action_requested("save-and-connect".into());

    assert_eq!(
        actions.borrow().as_slice(),
        ["save", "connect", "test", "save-and-connect"]
    );
}

#[test]
fn ssh_modal_resets_to_standard_english_shell_when_reopened() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_tab_selected("proxy".into());
    app.invoke_close_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_active_tab().as_str(), "standard");
}

#[test]
fn rename_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_rename_modal_open(true);
    app.set_asset_rename_modal_name("Prod".into());
    app.set_asset_rename_modal_validation_message("Duplicate name".into());
    app.set_asset_rename_modal_can_confirm(false);

    assert!(app.get_asset_rename_modal_open());
    assert_eq!(app.get_asset_rename_modal_name().as_str(), "Prod");
    assert_eq!(
        app.get_asset_rename_modal_validation_message().as_str(),
        "Duplicate name"
    );
    assert!(!app.get_asset_rename_modal_can_confirm());
}

#[test]
fn delete_modal_visibility_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_delete_confirm_modal_open(true);
    app.set_asset_delete_confirm_target_label("Prod".into());
    app.set_asset_delete_confirm_descendant_count(3);

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(app.get_asset_delete_confirm_target_label().as_str(), "Prod");
    assert_eq!(app.get_asset_delete_confirm_descendant_count(), 3);
}

#[test]
fn create_modals_project_inline_validation_message_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Folder 1");
    assert_eq!(app.get_asset_modal_validation_message().as_str(), "");
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "SSH Connection 1");
    assert_eq!(app.get_asset_modal_validation_message().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Folder 1".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());

    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Folder 1");
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Name already exists in this folder."
    );
    assert!(!app.get_asset_modal_can_confirm());
}

#[test]
fn ssh_modal_confirm_updates_runtime_tree_and_persists_ssh_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let repo_state = Rc::new(RefCell::new(ModalAssetRepoState::default()));
    let asset_repo: Rc<dyn AssetCatalogRepository> =
        Rc::new(RecordingModalAssetRepo::new(Rc::clone(&repo_state)));

    bind_top_status_bar_with_store_and_effects_and_asset_repo(
        &app,
        None,
        default_platform_window_effects(),
        Some(asset_repo),
    );

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("port".into(), "2022".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_draft_changed("environment".into(), "prod".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_method".into(), "jump-host".into());
    app.invoke_confirm_asset_modal_requested();

    assert_eq!(app.get_console_asset_items().row_count(), 1);
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .unwrap()
            .label
            .as_str(),
        "Prod Bastion"
    );

    let save_attempts = &repo_state.borrow().save_attempts;
    assert_eq!(save_attempts.len(), 1);
    assert_eq!(save_attempts[0].root_ids.len(), 1);
    let node = save_attempts[0]
        .nodes
        .get(save_attempts[0].root_ids[0].as_str())
        .unwrap();
    match &node.payload {
        PersistedAssetPayload::SshConnection(spec) => {
            assert_eq!(spec.host, "10.0.0.12");
            assert_eq!(spec.user, "ops");
            assert_eq!(spec.port, "2022");
            assert_eq!(spec.environment, "prod");
            assert_eq!(spec.proxy_method, "jump-host");
        }
        PersistedAssetPayload::Folder => panic!("expected ssh payload"),
    }
}
