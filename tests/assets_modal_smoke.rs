use mica_term::AppWindow;
use mica_term::WorkspaceTabItem;
use mica_term::app::assets_catalog::{
    ASSET_CATALOG_SCHEMA_VERSION, AssetCatalogRepository, PersistedAssetCatalog,
    PersistedAssetPayload,
};
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_effects_and_asset_repo,
};
use mica_term::app::ssh::known_hosts::{KnownHostCheck, KnownHostsService};
use mica_term::app::window_effects::default_platform_window_effects;
use russh::keys::{HashAlg, PublicKey};
use slint::Model;
use slint::{ModelRc, VecModel};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
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

fn sample_known_hosts_path(label: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "mica-term-assets-modal-known-hosts-{}-{}.txt",
        label,
        std::process::id()
    ));
    path
}

fn sample_public_key() -> PublicKey {
    PublicKey::from_openssh(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAILM+rvN+ot98qgEN796jTiQfZfG1KaT0PtFDJ/XFSqti test-1@example.com",
    )
    .expect("parse public key")
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
fn ssh_modal_round_trips_grouped_form_fields_without_top_level_tab_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_name("Prod Bastion".into());
    app.set_asset_ssh_modal_host("10.0.0.12".into());
    app.set_asset_ssh_modal_user("ops".into());
    app.set_asset_ssh_modal_port("22".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "Prod Bastion");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "10.0.0.12");
    assert_eq!(app.get_asset_ssh_modal_user().as_str(), "ops");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "22");
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
    assert_eq!(
        app.get_asset_ssh_modal_auth_method().as_str(),
        "private-key"
    );
    assert_eq!(
        app.get_asset_ssh_modal_private_key_source().as_str(),
        "path"
    );
    assert_eq!(app.get_asset_ssh_modal_password().as_str(), "secret");
    assert_eq!(
        app.get_asset_ssh_modal_remark().as_str(),
        "Primary entry point"
    );
}

#[test]
fn ssh_modal_action_callback_contract_exposes_full_connect_family() {
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
fn ssh_modal_contract_round_trips_button_state_and_inline_feedback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_modal_can_confirm(true);
    app.set_asset_modal_validation_message("Host is required.".into());
    app.set_asset_ssh_modal_connect_family_enabled(false);
    app.set_asset_ssh_modal_feedback_state("busy".into());
    app.set_asset_ssh_modal_feedback_message("Testing connection...".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert!(app.get_asset_modal_can_confirm());
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Host is required."
    );
    assert!(!app.get_asset_ssh_modal_connect_family_enabled());
    assert_eq!(app.get_asset_ssh_modal_feedback_state().as_str(), "busy");
    assert_eq!(
        app.get_asset_ssh_modal_feedback_message().as_str(),
        "Testing connection..."
    );
}

#[test]
fn ssh_modal_round_trips_secret_retention_copy_and_clear_affordance() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_asset_modal_open(true);
    app.set_asset_modal_kind("new-ssh-connection".into());
    app.set_asset_ssh_modal_secret_retention_message(
        "Leave password / private key / passphrase blank to keep the saved secret.".into(),
    );
    app.set_asset_ssh_modal_can_clear_saved_secret(true);
    app.set_asset_ssh_modal_clear_saved_secret_requested(false);

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_asset_ssh_modal_secret_retention_message().as_str(),
        "Leave password / private key / passphrase blank to keep the saved secret."
    );
    assert!(app.get_asset_ssh_modal_can_clear_saved_secret());
    assert!(!app.get_asset_ssh_modal_clear_saved_secret_requested());
}

#[test]
fn app_window_round_trips_workspace_tab_items_and_active_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.set_workspace_tab_items(ModelRc::new(VecModel::from(vec![
        WorkspaceTabItem {
            session_id: "session-1".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@example.com:22".into(),
            state: "connected".into(),
            active: false,
        },
        WorkspaceTabItem {
            session_id: "session-2".into(),
            title: "Staging Bastion".into(),
            subtitle: "ops@staging.example.com:22".into(),
            state: "error".into(),
            active: true,
        },
    ])));
    app.set_active_workspace_session_id("session-2".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_eq!(
        app.get_workspace_tab_items()
            .row_data(1)
            .expect("workspace tab item")
            .title
            .as_str(),
        "Staging Bastion"
    );
    assert_eq!(app.get_active_workspace_session_id().as_str(), "session-2");
}

#[test]
fn host_key_confirm_modal_round_trips_target_host_and_fingerprint() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_ssh_host_key_modal_open(true);
    app.set_ssh_host_key_modal_host("example.com".into());
    app.set_ssh_host_key_modal_fingerprint("SHA256:abc123".into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "example.com");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        "SHA256:abc123"
    );
}

#[test]
fn unknown_host_key_prompts_once_then_reconnect_uses_trusted_key() {
    i_slint_backend_testing::init_no_event_loop();

    let path = sample_known_hosts_path("trusted-once");
    let _ = fs::remove_file(&path);
    let service = KnownHostsService::new(&path);
    let key = sample_public_key();

    let first_check = service
        .check("example.com", 22, &key)
        .expect("check unknown host");
    let fingerprint = match first_check {
        KnownHostCheck::Unknown { fingerprint } => fingerprint,
        other => panic!("expected unknown host result, got {other:?}"),
    };

    let app = AppWindow::new().unwrap();
    app.set_ssh_host_key_modal_open(true);
    app.set_ssh_host_key_modal_host("example.com".into());
    app.set_ssh_host_key_modal_fingerprint(fingerprint.clone().into());

    assert!(app.get_ssh_host_key_modal_open());
    assert_eq!(app.get_ssh_host_key_modal_host().as_str(), "example.com");
    assert_eq!(
        app.get_ssh_host_key_modal_fingerprint().as_str(),
        key.fingerprint(HashAlg::Sha256).to_string()
    );

    service
        .accept_unknown("example.com", 22, &key)
        .expect("accept trusted host key");

    let second_check = service
        .check("example.com", 22, &key)
        .expect("recheck trusted host");
    assert!(matches!(second_check, KnownHostCheck::Trusted));

    let _ = fs::remove_file(&path);
}

#[test]
fn ssh_modal_reopens_with_default_authentication_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("auth_method".into(), "private-key".into());
    app.invoke_close_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_auth_method().as_str(), "password");
    assert_eq!(app.get_asset_ssh_modal_dialog_title().as_str(), "New SSH Connection");
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
fn blocking_modal_shell_owns_shared_asset_modal_chrome_contract() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");
    let folder = fs::read_to_string("ui/components/assets-folder-create-modal.slint")
        .expect("read folder modal");
    let ssh = fs::read_to_string("ui/components/assets-ssh-connection-modal.slint")
        .expect("read ssh modal");

    assert!(shell.contains("in property <string> dialog-title"));
    assert!(shell.contains("callback close-requested();"));
    assert!(shell.contains("header := Rectangle {"));
    assert!(shell.contains("close-button := Rectangle {"));
    assert!(!folder.contains("drag-touch := TouchArea {"));
    assert!(!ssh.contains("drag-touch := TouchArea {"));
}

#[test]
fn blocking_modal_shell_header_and_content_are_top_anchored() {
    let shell = fs::read_to_string("ui/components/blocking-modal-shell.slint")
        .expect("read blocking modal shell");

    assert!(
        shell.contains("header := Rectangle {\n            x: 0px;\n            y: 0px;"),
        "blocking modal shell header must be pinned to the frame origin instead of relying on Slint centering defaults"
    );
    assert!(
        shell.contains("content-host := Rectangle {\n            x: 0px;\n            y: header.height;"),
        "blocking modal shell content host must start immediately below the header"
    );
}

#[test]
fn blocking_modal_children_bind_overlay_parent_dimensions() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");

    assert!(app_window.contains(
        "asset-folder-modal-overlay := AssetsFolderCreateModal {\n            x: 0px;\n            y: 0px;\n            width: parent.width;"
    ));
    assert!(app_window.contains(
        "asset-ssh-modal-overlay := AssetsSshConnectionModal {\n            x: 0px;\n            y: 0px;\n            width: parent.width;"
    ));
    assert!(app_window.contains(
        "asset-rename-modal-overlay := AssetsRenameModal {\n            x: 0px;\n            y: 0px;\n            width: parent.width;"
    ));
    assert!(app_window.contains(
        "asset-delete-confirm-modal-overlay := AssetsDeleteConfirmModal {\n            x: 0px;\n            y: 0px;\n            width: parent.width;"
    ));
    assert!(app_window.contains(
        "ssh-host-key-modal-overlay := SshHostKeyConfirmModal {\n            x: 0px;\n            y: 0px;\n            width: parent.width;"
    ));
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
