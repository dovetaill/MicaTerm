use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store;
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, StoredKeychainIdentitySecretBundle,
    keychain_identity_credential_ref, load_keychain_identity_secret_bundle,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use russh::keys::ssh_key::{LineEnding, rand_core::OsRng};
use russh::keys::{Algorithm, HashAlg, PrivateKey};
use slint::Model;
use tokio::sync::mpsc;

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

fn sample_key_material() -> (String, String, String) {
    let private_key =
        PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("generate sample private key");
    let private_key_text = private_key
        .to_openssh(LineEnding::LF)
        .expect("encode private key")
        .to_string();
    let public_key_text = private_key
        .public_key()
        .to_openssh()
        .expect("encode public key");
    let fingerprint = private_key.fingerprint(HashAlg::Sha256).to_string();
    (private_key_text, public_key_text, fingerprint)
}

fn find_keychain_row_id(app: &AppWindow, kind: &str, label: &str) -> String {
    let rows = app.get_keychain_asset_items();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .find(|row| row.kind.as_str() == kind && row.label.as_str() == label)
        .map(|row| row.id.to_string())
        .expect("matching keychain row")
}

fn open_new_identity_modal(app: &AppWindow) {
    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-identity".into());
}

fn create_keychain_ssh_key(app: &AppWindow, name: &str) -> String {
    let (private_key, public_key, fingerprint) = sample_key_material();

    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-ssh-key".into());
    app.invoke_keychain_ssh_key_modal_draft_changed("name".into(), name.into());
    app.invoke_keychain_ssh_key_modal_draft_changed("private_key".into(), private_key.into());
    app.invoke_keychain_ssh_key_modal_draft_changed("public_key".into(), public_key.into());
    app.invoke_keychain_ssh_key_modal_draft_changed("fingerprint".into(), fingerprint.into());
    app.invoke_confirm_asset_modal_requested();

    find_keychain_row_id(app, "ssh-key", name)
}

#[test]
fn confirming_identity_modal_waits_for_valid_input_and_persists_password_secret() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_credential_store(&app, Arc::clone(&credential_store));

    open_new_identity_modal(&app);

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-identity");
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);

    app.invoke_keychain_identity_modal_draft_changed("name".into(), "Prod Identity".into());
    app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
    assert!(!app.get_asset_modal_can_confirm());

    app.invoke_keychain_identity_modal_draft_changed("password".into(), "secret".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();

    let identity_id = find_keychain_row_id(&app, "identity", "Prod Identity");
    let credential_ref = keychain_identity_credential_ref(identity_id.as_str());
    let bundle =
        load_keychain_identity_secret_bundle(credential_store.as_ref(), credential_ref.as_str())
            .expect("load saved identity secret");

    assert_eq!(app.get_keychain_asset_items().row_count(), 1);
    assert_eq!(bundle.password.as_deref(), Some("secret"));
}

#[test]
fn editing_identity_can_switch_between_password_and_ssh_key_auth_without_losing_shared_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_credential_store(&app, Arc::clone(&credential_store));

    let ssh_key_id = create_keychain_ssh_key(&app, "Prod Key");
    assert!(!ssh_key_id.is_empty());

    open_new_identity_modal(&app);
    app.invoke_keychain_identity_modal_draft_changed("name".into(), "Prod Identity".into());
    app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
    app.invoke_keychain_identity_modal_draft_changed("password".into(), "secret".into());
    app.invoke_keychain_identity_modal_draft_changed("remark".into(), "primary".into());
    app.invoke_confirm_asset_modal_requested();

    let identity_id = find_keychain_row_id(&app, "identity", "Prod Identity");
    let credential_ref = keychain_identity_credential_ref(identity_id.as_str());

    app.invoke_asset_context_menu_requested(
        identity_id.clone().into(),
        "identity".into(),
        96.0,
        160.0,
    );
    app.invoke_assets_context_menu_action_invoked("edit-keychain-identity".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Prod Identity"
    );
    assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
    assert_eq!(
        app.get_keychain_identity_modal_password().as_str(),
        "secret"
    );
    assert_eq!(app.get_keychain_identity_modal_remark().as_str(), "primary");

    app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "ssh-key".into());

    assert_eq!(
        app.get_keychain_identity_modal_auth_kind().as_str(),
        "ssh-key"
    );
    assert_eq!(app.get_keychain_identity_modal_password().as_str(), "");
    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Prod Identity"
    );
    assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
    assert_eq!(app.get_keychain_identity_modal_remark().as_str(), "primary");

    app.invoke_keychain_identity_modal_action_requested("use-existing-ssh-key".into());
    assert_eq!(
        app.get_keychain_identity_modal_ssh_key_label().as_str(),
        "Prod Key"
    );
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_confirm_asset_modal_requested();

    let after_ssh_key =
        load_keychain_identity_secret_bundle(credential_store.as_ref(), credential_ref.as_str())
            .expect("load identity bundle after switching to ssh key");
    assert_eq!(after_ssh_key, StoredKeychainIdentitySecretBundle::default());

    app.invoke_asset_context_menu_requested(
        identity_id.clone().into(),
        "identity".into(),
        96.0,
        160.0,
    );
    app.invoke_assets_context_menu_action_invoked("edit-keychain-identity".into());

    assert_eq!(
        app.get_keychain_identity_modal_auth_kind().as_str(),
        "ssh-key"
    );
    assert_eq!(
        app.get_keychain_identity_modal_ssh_key_label().as_str(),
        "Prod Key"
    );
    assert_eq!(app.get_keychain_identity_modal_password().as_str(), "");
    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Prod Identity"
    );
    assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
    assert_eq!(app.get_keychain_identity_modal_remark().as_str(), "primary");

    app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "password".into());
    assert_eq!(
        app.get_keychain_identity_modal_auth_kind().as_str(),
        "password"
    );
    assert_eq!(app.get_keychain_identity_modal_ssh_key_label().as_str(), "");
    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Prod Identity"
    );
    assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
    assert_eq!(app.get_keychain_identity_modal_remark().as_str(), "primary");

    app.invoke_keychain_identity_modal_draft_changed("password".into(), "rotated-secret".into());
    assert!(app.get_asset_modal_can_confirm());
    app.invoke_confirm_asset_modal_requested();

    app.invoke_asset_context_menu_requested(identity_id.into(), "identity".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-keychain-identity".into());

    assert_eq!(
        app.get_keychain_identity_modal_auth_kind().as_str(),
        "password"
    );
    assert_eq!(
        app.get_keychain_identity_modal_password().as_str(),
        "rotated-secret"
    );
    assert_eq!(
        app.get_keychain_identity_modal_name().as_str(),
        "Prod Identity"
    );
    assert_eq!(app.get_keychain_identity_modal_username().as_str(), "ops");
    assert_eq!(app.get_keychain_identity_modal_remark().as_str(), "primary");
}

#[test]
fn keychain_identity_password_reveal_resets_when_switching_auth_kind() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_credential_store(&app, credential_store);

    open_new_identity_modal(&app);
    app.invoke_keychain_identity_modal_draft_changed(
        "password_visibility".into(),
        "visible".into(),
    );
    assert!(app.get_keychain_identity_modal_password_visible());

    app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "ssh-key".into());

    assert_eq!(
        app.get_keychain_identity_modal_auth_kind().as_str(),
        "ssh-key"
    );
    assert!(!app.get_keychain_identity_modal_password_visible());
}

#[test]
fn keychain_identity_password_reveal_resets_when_reopening_edit_modal() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_credential_store(&app, Arc::clone(&credential_store));

    open_new_identity_modal(&app);
    app.invoke_keychain_identity_modal_draft_changed("name".into(), "Prod Identity".into());
    app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
    app.invoke_keychain_identity_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let identity_id = find_keychain_row_id(&app, "identity", "Prod Identity");
    app.invoke_asset_context_menu_requested(
        identity_id.clone().into(),
        "identity".into(),
        96.0,
        160.0,
    );
    app.invoke_assets_context_menu_action_invoked("edit-keychain-identity".into());
    app.invoke_keychain_identity_modal_draft_changed(
        "password_visibility".into(),
        "visible".into(),
    );
    assert!(app.get_keychain_identity_modal_password_visible());

    app.invoke_close_asset_modal_requested();
    assert!(!app.get_asset_modal_open());

    app.invoke_asset_context_menu_requested(identity_id.into(), "identity".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-keychain-identity".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(
        app.get_keychain_identity_modal_password().as_str(),
        "secret"
    );
    assert!(!app.get_keychain_identity_modal_password_visible());
}
