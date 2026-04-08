use std::collections::BTreeMap;
use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::app_paths::{AppRootPathInputs, AppRootSource, resolve_app_root_paths};
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store;
use mica_term::app::keychain::redb_store::RedbKeychainCatalogStore;
use mica_term::app::keychain::repository::KeychainCatalogRepository;
use mica_term::app::keychain::{
    KeychainCatalog, KeychainIdentityAuthKind, KeychainIdentitySpec, KeychainNode,
    KeychainNodeKind, KeychainNodePayload, KeychainSshKeySpec,
};
use mica_term::app::ssh::credentials::{CredentialStore, MemoryCredentialStore};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::Model;
use tokio::sync::mpsc;

static APP_DIR_ENV_LOCK: Mutex<()> = Mutex::new(());

struct NoopRuntimeControl;

#[derive(Clone, Default)]
struct FakeLauncher;

impl SessionRuntimeControl for NoopRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

impl SessionRuntimeLauncher for FakeLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        _session_id: uuid::Uuid,
        _attempt_id: uuid::Uuid,
        _event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move { Ok(Box::new(NoopRuntimeControl) as Box<dyn SessionRuntimeControl>) })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

fn temp_data_dir(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join(format!("{name}-{unique}"));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

fn sample_catalog() -> KeychainCatalog {
    KeychainCatalog {
        root_ids: vec!["folder-team".into()],
        nodes: BTreeMap::from([
            (
                "folder-team".into(),
                KeychainNode {
                    id: "folder-team".into(),
                    parent_id: None,
                    title: "Team".into(),
                    kind: KeychainNodeKind::Folder,
                    child_ids: vec!["identity-ops".into(), "key-prod".into()],
                    payload: KeychainNodePayload::Folder,
                },
            ),
            (
                "identity-ops".into(),
                KeychainNode {
                    id: "identity-ops".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Ops".into(),
                    kind: KeychainNodeKind::Identity,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::Identity(KeychainIdentitySpec {
                        username: "ops".into(),
                        auth_kind: KeychainIdentityAuthKind::SshKey,
                        ssh_key_id: Some("key-prod".into()),
                        credential_ref: None,
                        remark: "shared login".into(),
                    }),
                },
            ),
            (
                "key-prod".into(),
                KeychainNode {
                    id: "key-prod".into(),
                    parent_id: Some("folder-team".into()),
                    title: "Prod Key".into(),
                    kind: KeychainNodeKind::SshKey,
                    child_ids: Vec::new(),
                    payload: KeychainNodePayload::SshKey(KeychainSshKeySpec {
                        algorithm: "ed25519".into(),
                        fingerprint: "SHA256:key-prod".into(),
                        public_key: "ssh-ed25519 AAAAC3NzaKeyProd".into(),
                        comment: "prod@example".into(),
                        credential_ref: Some("keychain/key/key-prod".into()),
                        remark: "generated".into(),
                    }),
                },
            ),
        ]),
        merge_metadata: BTreeMap::new(),
    }
}

fn lock_app_dir_env() -> std::sync::MutexGuard<'static, ()> {
    APP_DIR_ENV_LOCK
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn bind_with_credential_store(app: &AppWindow, credential_store: Arc<dyn CredentialStore>) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        credential_store,
    );
}

fn find_keychain_row_id(app: &AppWindow, kind: &str, label: &str) -> String {
    let rows = app.get_keychain_asset_items();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .find(|row| row.kind.as_str() == kind && row.label.as_str() == label)
        .map(|row| row.id.to_string())
        .expect("matching keychain row")
}

#[test]
fn app_root_exposes_keychain_catalog_database_path_under_data_dir() {
    let temp_root = temp_data_dir("app-paths-keychain-db");
    let paths = resolve_app_root_paths(&AppRootPathInputs {
        env_root_dir: Some(temp_root.join("override-root")),
        executable_dir: temp_root.join("bin"),
        standard_local_data_dir: temp_root.join("standard-root"),
        portable_marker_name: ".mica-term-portable",
    })
    .unwrap();

    assert_eq!(paths.root_source, AppRootSource::EnvOverride);
    assert_eq!(
        paths.keychain_catalog_database_path(),
        temp_root
            .join("override-root")
            .join("data")
            .join("keychain.redb")
    );
}

#[test]
fn load_returns_empty_catalog_when_keychain_store_is_missing() {
    let data_dir = temp_data_dir("keychain-store-missing");
    let store = RedbKeychainCatalogStore::new(data_dir.clone());

    let catalog = store.load().unwrap();

    assert!(catalog.root_ids.is_empty());
    assert!(catalog.nodes.is_empty());
    assert!(!store.database_path.exists());
}

#[test]
fn save_and_reload_preserves_folder_identity_key_order_and_links() {
    let data_dir = temp_data_dir("keychain-store-roundtrip");
    let store = RedbKeychainCatalogStore::new(data_dir);
    let catalog = sample_catalog();

    store.save(&catalog).unwrap();
    let reloaded = store.load().unwrap();

    assert_eq!(reloaded.root_ids, vec!["folder-team"]);
    assert_eq!(reloaded.nodes.len(), 3);
    assert_eq!(reloaded.nodes["folder-team"].child_ids.len(), 2);
    assert_eq!(
        reloaded.nodes["identity-ops"].parent_id.as_deref(),
        Some("folder-team")
    );
    match &reloaded.nodes["identity-ops"].payload {
        KeychainNodePayload::Identity(identity) => {
            assert_eq!(identity.username, "ops");
            assert_eq!(identity.ssh_key_id.as_deref(), Some("key-prod"));
        }
        other => panic!("expected identity payload, got {other:?}"),
    }
    match &reloaded.nodes["key-prod"].payload {
        KeychainNodePayload::SshKey(ssh_key) => {
            assert_eq!(ssh_key.algorithm, "ed25519");
            assert_eq!(ssh_key.comment, "prod@example");
        }
        other => panic!("expected ssh key payload, got {other:?}"),
    }
}

#[test]
fn bootstrap_roundtrip_reloads_keychain_catalog_with_folder_identity_and_key_links() {
    i_slint_backend_testing::init_no_event_loop();

    let _env_lock = lock_app_dir_env();
    let app_root = temp_data_dir("keychain-bootstrap-roundtrip");
    unsafe {
        std::env::set_var("MICA_TERM_APP_DIR", &app_root);
    }

    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    let folder_id;
    let key_id;
    let identity_id;
    {
        let app = AppWindow::new().unwrap();
        bind_with_credential_store(&app, Arc::clone(&credential_store));

        app.invoke_sidebar_destination_selected("keychain".into());
        app.invoke_assets_create_action_selected("new-folder".into());

        let folder_row = app
            .get_keychain_asset_items()
            .row_data(0)
            .expect("root keychain folder");
        folder_id = folder_row.id.to_string();

        app.invoke_asset_context_menu_requested(
            folder_id.clone().into(),
            "folder".into(),
            96.0,
            160.0,
        );
        app.invoke_assets_context_menu_action_invoked("new-ssh-key".into());
        app.invoke_keychain_ssh_key_modal_draft_changed("name".into(), "Prod Key".into());
        app.invoke_keychain_ssh_key_modal_draft_changed("private_key".into(), "PRIVATE".into());
        app.invoke_keychain_ssh_key_modal_draft_changed(
            "public_key".into(),
            "ssh-ed25519 AAAATEST prod@example".into(),
        );
        app.invoke_keychain_ssh_key_modal_draft_changed("fingerprint".into(), "SHA256:prod".into());
        app.invoke_confirm_asset_modal_requested();
        key_id = find_keychain_row_id(&app, "ssh-key", "Prod Key");

        app.invoke_asset_context_menu_requested(
            folder_id.clone().into(),
            "folder".into(),
            96.0,
            160.0,
        );
        app.invoke_assets_context_menu_action_invoked("new-identity".into());
        app.invoke_keychain_identity_modal_draft_changed("name".into(), "Ops".into());
        app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
        app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "ssh-key".into());
        app.invoke_keychain_identity_modal_action_requested("use-existing-ssh-key".into());
        app.invoke_confirm_asset_modal_requested();
        identity_id = find_keychain_row_id(&app, "identity", "Ops");
    }

    let store = RedbKeychainCatalogStore::new(app_root.join("data"));
    let reloaded = store.load().unwrap();

    assert_eq!(reloaded.root_ids, vec![folder_id.clone()]);
    assert_eq!(reloaded.nodes[&folder_id].child_ids.len(), 2);
    assert_eq!(
        reloaded.nodes[&key_id].parent_id.as_deref(),
        Some(folder_id.as_str())
    );
    assert_eq!(
        reloaded.nodes[&identity_id].parent_id.as_deref(),
        Some(folder_id.as_str())
    );
    match &reloaded.nodes[&identity_id].payload {
        KeychainNodePayload::Identity(identity) => {
            assert_eq!(identity.username, "ops");
            assert_eq!(identity.ssh_key_id.as_deref(), Some(key_id.as_str()));
        }
        other => panic!("expected identity payload, got {other:?}"),
    }

    let second_app = AppWindow::new().unwrap();
    bind_with_credential_store(&second_app, credential_store);
    second_app.invoke_sidebar_destination_selected("keychain".into());

    let rows = second_app.get_keychain_asset_items();
    assert_eq!(rows.row_count(), 3);
    assert_eq!(
        find_keychain_row_id(
            &second_app,
            "folder",
            reloaded.nodes[&folder_id].title.as_str()
        ),
        folder_id
    );
    assert_eq!(
        find_keychain_row_id(&second_app, "ssh-key", "Prod Key"),
        key_id
    );
    assert_eq!(
        find_keychain_row_id(&second_app, "identity", "Ops"),
        identity_id
    );

    unsafe {
        std::env::remove_var("MICA_TERM_APP_DIR");
    }
}
