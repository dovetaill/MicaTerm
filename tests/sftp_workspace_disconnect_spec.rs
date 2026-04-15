use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::assets_catalog::AssetCatalogRepository;
use mica_term::app::bootstrap::bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store;
use mica_term::app::sftp::{SftpBackend, SftpRuntimeHandle};
use mica_term::app::ssh::credentials::{CredentialStore, MemoryCredentialStore};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::Model;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Default)]
struct NoopSftpBackend;

impl SftpBackend for NoopSftpBackend {
    fn read_dir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<
        Box<dyn Future<Output = Result<Vec<mica_term::app::sftp::SftpDirectoryEntry>>> + Send + 'a>,
    > {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn mkdir<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn rename<'a>(
        &'a self,
        _from: &'a str,
        _to: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn path_exists<'a>(
        &'a self,
        _path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move { Ok(true) })
    }

    fn upload_file<'a>(
        &'a self,
        _remote_path: &'a str,
        data: Vec<u8>,
    ) -> Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move { Ok(data.len() as u64) })
    }

    fn download_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<u8>>> + Send + 'a>> {
        Box::pin(async move { Ok(Vec::new()) })
    }

    fn remove_file<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }

    fn remove_dir<'a>(
        &'a self,
        _remote_path: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move { Ok(()) })
    }
}

#[derive(Clone)]
struct SessionCwdLauncher {
    cwd_by_host: Arc<Vec<(String, String)>>,
}

struct SessionCwdRuntimeControl {
    runtime: SftpRuntimeHandle,
}

impl SessionRuntimeLauncher for SessionCwdLauncher {
    fn launch(
        &self,
        profile: ConnectionProfile,
        _session_id: Uuid,
        _attempt_id: Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        let cwd = self
            .cwd_by_host
            .iter()
            .find(|(host, _)| host == &profile.host)
            .map(|(_, cwd)| cwd.clone())
            .unwrap_or_else(|| "/".to_string());
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::CurrentDirectoryChanged(cwd));
            Ok(Box::new(SessionCwdRuntimeControl {
                runtime: SftpRuntimeHandle::new(Arc::new(NoopSftpBackend)),
            }) as Box<dyn SessionRuntimeControl>)
        })
    }

    fn probe(
        &self,
        _profile: ConnectionProfile,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + 'static>> {
        Box::pin(async move { Ok(()) })
    }
}

impl SessionRuntimeControl for SessionCwdRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, _text: String) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }

    fn sftp_runtime(&self) -> Option<SftpRuntimeHandle> {
        Some(self.runtime.clone())
    }
}

fn bind_with_launcher(
    app: &AppWindow,
    asset_repo: Option<Rc<dyn AssetCatalogRepository>>,
    launcher: Arc<dyn SessionRuntimeLauncher>,
    credential_store: Arc<dyn CredentialStore>,
) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher_and_credential_store(
        app,
        None,
        default_platform_window_effects(),
        asset_repo,
        launcher,
        credential_store,
    );
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) -> String {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_asset_ssh_modal_action_requested("save".into());

    app.get_console_asset_items()
        .row_data(0)
        .expect("saved ssh asset")
        .id
        .to_string()
}

fn find_console_asset_id(app: &AppWindow, label: &str) -> String {
    let rows = app.get_console_asset_items();
    (0..rows.row_count())
        .filter_map(|index| rows.row_data(index))
        .find(|row| row.label.as_str() == label)
        .map(|row| row.id.to_string())
        .expect("asset id by label")
}

fn flush_runtime_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

#[test]
fn closing_the_source_terminal_keeps_the_sftp_workspace_tab_and_reconnects_without_restoring_a_terminal_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_launcher(
        &app,
        None,
        Arc::new(SessionCwdLauncher {
            cwd_by_host: Arc::new(vec![("10.0.0.12".into(), "/srv/app".into())]),
        }),
        Arc::new(MemoryCredentialStore::default()),
    );

    create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    let prod_asset = find_console_asset_id(&app, "Prod Bastion");
    app.invoke_asset_activated(prod_asset.into());
    flush_runtime_projection();
    let prod_session_id = app.get_active_workspace_session_id().to_string();

    app.invoke_open_sftp_panel_requested();
    flush_runtime_projection();
    app.invoke_sftp_panel_expand_requested();
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "sftp");
    assert_eq!(app.get_workspace_session_title().as_str(), "Files: Prod Bastion");

    app.invoke_workspace_tab_close_requested(prod_session_id.into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "sftp");
    assert_eq!(app.get_workspace_session_state().as_str(), "disconnected");
    assert!(app.get_workspace_session_can_reconnect());

    app.invoke_workspace_session_local_action_requested("reconnect-sftp-workspace".into());
    flush_runtime_projection();

    assert_eq!(app.get_workspace_tab_items().row_count(), 1);
    assert_eq!(app.get_workspace_session_host_mode().as_str(), "sftp");
    assert_eq!(app.get_workspace_session_state().as_str(), "ready");
    assert!(!app.get_workspace_session_can_reconnect());
}
