use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    ImportedPrivateKey, PrivateKeyImporter,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer,
};
use mica_term::app::ssh::credentials::{
    CredentialStore, MemoryCredentialStore, keychain_key_credential_ref,
    load_keychain_key_secret_bundle,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use russh::keys::ssh_key::{LineEnding, rand_core::OsRng};
use russh::keys::{Algorithm, HashAlg, PrivateKey, PublicKey};
use slint::Model;
use tokio::sync::mpsc;

struct NoopRuntimeControl;

#[derive(Clone, Default)]
struct FakeLauncher;

#[derive(Clone)]
struct StaticPrivateKeyImporter {
    imported: Option<ImportedPrivateKey>,
}

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

impl StaticPrivateKeyImporter {
    fn imported(path: &str, content: String) -> Self {
        Self {
            imported: Some(ImportedPrivateKey {
                path: PathBuf::from(path),
                content,
            }),
        }
    }

    fn cancelled() -> Self {
        Self { imported: None }
    }
}

impl PrivateKeyImporter for StaticPrivateKeyImporter {
    fn import_private_key(&self) -> Result<Option<ImportedPrivateKey>> {
        Ok(self.imported.clone())
    }
}

fn bind_with_importer(app: &AppWindow, importer: Arc<dyn PrivateKeyImporter>) {
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_importer_and_store(app, importer, credential_store);
}

fn bind_with_importer_and_store(
    app: &AppWindow,
    importer: Arc<dyn PrivateKeyImporter>,
    credential_store: Arc<dyn CredentialStore>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store_and_private_key_importer(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
        credential_store,
        importer,
    );
}

fn open_keychain_ssh_key_modal(app: &AppWindow) {
    app.invoke_sidebar_destination_selected("keychain".into());
    app.invoke_assets_create_action_selected("new-ssh-key".into());
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

#[test]
fn new_ssh_key_action_opens_modal_before_creating_keychain_item() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    bind_with_importer(&app, Arc::new(StaticPrivateKeyImporter::cancelled()));

    assert_eq!(app.get_keychain_asset_items().row_count(), 0);

    open_keychain_ssh_key_modal(&app);

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-ssh-key");
    assert_eq!(app.get_keychain_asset_items().row_count(), 0);
}

#[test]
fn importing_private_key_into_keychain_modal_populates_private_public_and_fingerprint() {
    i_slint_backend_testing::init_no_event_loop();

    let private_key =
        PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("generate sample private key");
    let private_key_openssh = private_key
        .to_openssh(LineEnding::LF)
        .expect("encode private key");
    let public_key_openssh = private_key
        .public_key()
        .to_openssh()
        .expect("encode public key");
    let fingerprint = private_key.fingerprint(HashAlg::Sha256).to_string();

    let app = AppWindow::new().expect("create app window");
    bind_with_importer(
        &app,
        Arc::new(StaticPrivateKeyImporter::imported(
            "/tmp/id_ed25519",
            private_key_openssh.to_string(),
        )),
    );

    open_keychain_ssh_key_modal(&app);
    app.invoke_keychain_ssh_key_modal_action_requested("import-private-key".into());

    assert_eq!(
        app.get_keychain_ssh_key_modal_private_key().as_str(),
        private_key_openssh.as_str()
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_public_key().as_str(),
        public_key_openssh
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_fingerprint().as_str(),
        fingerprint
    );
}

#[test]
fn importing_public_key_into_keychain_modal_populates_public_key_and_fingerprint_only() {
    i_slint_backend_testing::init_no_event_loop();

    let private_key =
        PrivateKey::random(&mut OsRng, Algorithm::Ed25519).expect("generate sample private key");
    let public_key_openssh = private_key
        .public_key()
        .to_openssh()
        .expect("encode public key");
    let fingerprint = PublicKey::from_openssh(&public_key_openssh)
        .expect("parse public key")
        .fingerprint(HashAlg::Sha256)
        .to_string();

    let app = AppWindow::new().expect("create app window");
    bind_with_importer(
        &app,
        Arc::new(StaticPrivateKeyImporter::imported(
            "/tmp/id_ed25519.pub",
            public_key_openssh.clone(),
        )),
    );

    open_keychain_ssh_key_modal(&app);
    app.invoke_keychain_ssh_key_modal_action_requested("import-public-key".into());

    assert_eq!(app.get_keychain_ssh_key_modal_private_key().as_str(), "");
    assert_eq!(
        app.get_keychain_ssh_key_modal_public_key().as_str(),
        public_key_openssh
    );
    assert_eq!(
        app.get_keychain_ssh_key_modal_fingerprint().as_str(),
        fingerprint
    );
}

#[test]
fn generating_key_pair_can_copy_public_key_to_system_clipboard() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    bind_with_importer(&app, Arc::new(StaticPrivateKeyImporter::cancelled()));

    i_slint_backend_selector::with_platform(|platform| {
        platform.set_clipboard_text("", slint::platform::Clipboard::DefaultClipboard);
        Ok::<(), slint::PlatformError>(())
    })
    .expect("clear clipboard");

    open_keychain_ssh_key_modal(&app);
    app.invoke_keychain_ssh_key_modal_action_requested("generate-key-pair".into());

    let generated_private_key = app.get_keychain_ssh_key_modal_private_key().to_string();
    let generated_public_key = app.get_keychain_ssh_key_modal_public_key().to_string();
    let generated_fingerprint = app.get_keychain_ssh_key_modal_fingerprint().to_string();

    assert!(
        generated_private_key.contains("BEGIN OPENSSH PRIVATE KEY"),
        "generate-key-pair should produce a new private key"
    );
    assert!(
        generated_public_key.starts_with("ssh-ed25519 "),
        "generate-key-pair should produce an Ed25519 public key"
    );
    assert!(
        generated_fingerprint.starts_with("SHA256:"),
        "generate-key-pair should derive a SHA256 fingerprint"
    );

    app.invoke_keychain_ssh_key_modal_action_requested("copy-public-key".into());

    let clipboard = i_slint_backend_selector::with_platform(|platform| {
        Ok(platform.clipboard_text(slint::platform::Clipboard::DefaultClipboard))
    })
    .expect("read clipboard")
    .expect("clipboard text");

    assert_eq!(clipboard, generated_public_key);
}

#[test]
fn editing_saved_key_rehydrates_private_public_and_fingerprint() {
    i_slint_backend_testing::init_no_event_loop();

    let (private_key, public_key, fingerprint) = sample_key_material();
    let app = AppWindow::new().expect("create app window");
    let credential_store: Arc<dyn CredentialStore> = Arc::new(MemoryCredentialStore::default());
    bind_with_importer_and_store(
        &app,
        Arc::new(StaticPrivateKeyImporter::cancelled()),
        Arc::clone(&credential_store),
    );

    open_keychain_ssh_key_modal(&app);
    app.invoke_keychain_ssh_key_modal_draft_changed("name".into(), "Prod Key".into());
    app.invoke_keychain_ssh_key_modal_draft_changed("private_key".into(), private_key.clone().into());
    app.invoke_keychain_ssh_key_modal_draft_changed("public_key".into(), public_key.clone().into());
    app.invoke_keychain_ssh_key_modal_draft_changed("fingerprint".into(), fingerprint.clone().into());
    app.invoke_confirm_asset_modal_requested();

    let key_id = find_keychain_row_id(&app, "ssh-key", "Prod Key");
    let stored = load_keychain_key_secret_bundle(
        credential_store.as_ref(),
        keychain_key_credential_ref(key_id.as_str()).as_str(),
    )
    .expect("load saved key secret bundle");
    assert_eq!(stored.private_key_content.as_deref(), Some(private_key.as_str()));

    app.invoke_asset_context_menu_requested(key_id.into(), "ssh-key".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("edit-keychain-ssh-key".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-keychain-ssh-key");
    assert_eq!(app.get_keychain_ssh_key_modal_name().as_str(), "Prod Key");
    assert_eq!(app.get_keychain_ssh_key_modal_private_key().as_str(), private_key);
    assert_eq!(app.get_keychain_ssh_key_modal_public_key().as_str(), public_key);
    assert_eq!(app.get_keychain_ssh_key_modal_fingerprint().as_str(), fingerprint);
}

#[test]
fn public_only_key_cannot_be_selected_for_identity_auth() {
    i_slint_backend_testing::init_no_event_loop();

    let (_private_key, public_key, fingerprint) = sample_key_material();
    let app = AppWindow::new().expect("create app window");
    bind_with_importer(&app, Arc::new(StaticPrivateKeyImporter::cancelled()));

    open_keychain_ssh_key_modal(&app);
    app.invoke_keychain_ssh_key_modal_draft_changed("name".into(), "Public Only Key".into());
    app.invoke_keychain_ssh_key_modal_draft_changed("public_key".into(), public_key.into());
    app.invoke_keychain_ssh_key_modal_draft_changed("fingerprint".into(), fingerprint.into());
    app.invoke_confirm_asset_modal_requested();

    assert_eq!(app.get_keychain_asset_items().row_count(), 1);

    app.invoke_assets_create_action_selected("new-identity".into());
    app.invoke_keychain_identity_modal_draft_changed("name".into(), "Prod Identity".into());
    app.invoke_keychain_identity_modal_draft_changed("username".into(), "ops".into());
    app.invoke_keychain_identity_modal_draft_changed("auth_kind".into(), "ssh-key".into());
    app.invoke_keychain_identity_modal_action_requested("use-existing-ssh-key".into());

    assert_eq!(
        app.get_keychain_identity_modal_ssh_key_label().as_str(),
        "Public Only Key"
    );
    assert!(!app.get_asset_modal_can_confirm());
    assert_eq!(
        app.get_asset_modal_validation_message().as_str(),
        "Selected SSH key must include private key material."
    );

    app.invoke_confirm_asset_modal_requested();
    assert_eq!(app.get_keychain_asset_items().row_count(), 1);
}
