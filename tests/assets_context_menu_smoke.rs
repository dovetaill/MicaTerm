//! Smoke coverage for the bootstrap-level assets context menu state bridge.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher,
};
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use slint::ComponentHandle;
use slint::Model;
use slint::PhysicalSize;
use tokio::sync::mpsc;

#[derive(Clone, Default)]
struct FakeLauncher;

#[derive(Clone, Default)]
struct PasteProjectionLauncher;

struct NoopRuntimeControl;

struct PasteProjectionRuntimeControl {
    session_id: uuid::Uuid,
    event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
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

impl SessionRuntimeLauncher for PasteProjectionLauncher {
    fn launch(
        &self,
        _profile: ConnectionProfile,
        session_id: uuid::Uuid,
        event_tx: mpsc::UnboundedSender<SessionRuntimeEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<Box<dyn SessionRuntimeControl>>> + Send + 'static>>
    {
        Box::pin(async move {
            let _ = event_tx.send(SessionRuntimeEvent::Connected);
            let _ = event_tx.send(SessionRuntimeEvent::SurfaceChanged(
                mica_term::app::ssh::runtime::TerminalSurfaceState::from_visible_lines(
                    session_id,
                    1,
                    24,
                    80,
                    vec!["welcome to mica-term".into()],
                ),
            ));
            Ok(Box::new(PasteProjectionRuntimeControl {
                session_id,
                event_tx,
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

impl SessionRuntimeControl for PasteProjectionRuntimeControl {
    fn disconnect(&self) -> Result<()> {
        Ok(())
    }

    fn send_text_input(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            mica_term::app::ssh::runtime::TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("text {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_key_input(&self, _event: TerminalKeyEvent) -> Result<()> {
        Ok(())
    }

    fn send_paste(&self, text: String) -> Result<()> {
        let _ = self.event_tx.send(SessionRuntimeEvent::SurfaceChanged(
            mica_term::app::ssh::runtime::TerminalSurfaceState::from_visible_lines(
                self.session_id,
                2,
                24,
                80,
                vec!["welcome to mica-term".into(), format!("paste {}", text)],
            ),
        ));
        Ok(())
    }

    fn send_mouse_input(&self, _event: TerminalMouseInput) -> Result<()> {
        Ok(())
    }

    fn resize(&self, _rows: u32, _cols: u32) -> Result<()> {
        Ok(())
    }
}

fn bind_with_fake_sessions(app: &AppWindow) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
    );
}

fn create_root_ssh(app: &AppWindow, name: &str, host: &str) -> String {
    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), host.into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    app.get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn create_root_snippet(app: &AppWindow, name: &str, script: &str) -> String {
    app.invoke_sidebar_destination_selected("snippets".into());
    app.invoke_assets_create_action_selected("new-snippet".into());
    app.invoke_asset_snippet_modal_draft_changed("name".into(), name.into());
    app.invoke_asset_snippet_modal_draft_changed("script".into(), script.into());
    app.invoke_confirm_asset_modal_requested();

    app.get_snippet_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string()
}

fn flush_runtime_projection() {
    std::thread::sleep(Duration::from_millis(20));
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

#[test]
fn bootstrap_exposes_context_menu_closed_by_default() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_assets_context_menu_open());
    assert_eq!(app.get_assets_context_menu_anchor_x(), 0.0);
    assert_eq!(app.get_assets_context_menu_anchor_y(), 0.0);
    assert_eq!(app.get_context_menu_feedback_text().as_str(), "");
}

#[test]
fn right_click_request_opens_context_menu_and_sets_anchor() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("ssh-prod-01".into(), "ssh".into(), 144.0, 188.0);

    assert!(app.get_assets_context_menu_open());
    assert_eq!(app.get_assets_context_menu_anchor_x(), 144.0);
    assert_eq!(app.get_assets_context_menu_anchor_y(), 188.0);
}

#[test]
fn invoking_open_from_ssh_context_menu_creates_a_new_session_tab() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);
    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");

    app.invoke_asset_context_menu_requested(ssh_id.clone().into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());
    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
}

#[test]
fn invoking_paste_snippet_from_context_menu_forwards_script_to_active_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        &app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(PasteProjectionLauncher),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    let snippet_id =
        create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_context_menu_requested(snippet_id.into(), "snippet".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("paste-snippet".into());
    flush_runtime_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "paste kubectl rollout restart deploy/api"
    );
}

#[test]
fn invoking_run_snippet_from_context_menu_forwards_script_as_text_input() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        &app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(PasteProjectionLauncher),
    );

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.into());
    flush_runtime_projection();

    let snippet_id =
        create_root_snippet(&app, "Deploy prod", "kubectl rollout restart deploy/api");

    app.invoke_asset_context_menu_requested(snippet_id.into(), "snippet".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("run-snippet".into());
    flush_runtime_projection();

    let visible_lines = app.get_workspace_session_visible_lines();
    assert_eq!(visible_lines.row_count(), 2);
    assert_eq!(
        visible_lines.row_data(1).unwrap().as_str(),
        "text kubectl rollout restart deploy/api"
    );
}

#[test]
fn bootstrap_starts_with_empty_console_assets() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn blank_area_menu_projects_compact_overlay_height() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let overlay_height = app.get_layout_assets_context_menu_height();
    assert!(overlay_height > 0.0);
    assert!(overlay_height < 160.0);
    assert!(app.get_assets_context_menu_primary_items().row_count() > 0);
}

#[test]
fn blank_area_right_click_opens_minimal_primary_menu() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);

    let primary = app.get_assets_context_menu_primary_items();
    let ids: Vec<String> = (0..primary.row_count())
        .filter_map(|index| primary.row_data(index))
        .map(|item| item.id.to_string())
        .collect();

    assert_eq!(ids, vec!["new-folder", "new-ssh-connection"]);
    assert_eq!(app.get_assets_context_menu_secondary_items().row_count(), 0);
}

#[test]
fn create_menu_action_opens_folder_modal_without_placeholder_item() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_console_asset_items().row_count(), 0);
}

#[test]
fn folder_modal_confirm_projects_root_row_into_window_model() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_modal_requested();

    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .unwrap()
            .label
            .as_str(),
        "Infra"
    );
}

#[test]
fn folder_modal_cancel_and_reopen_resets_name_draft() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "");

    app.invoke_assets_create_action_selected("new-folder".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-folder");
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "Folder 1");
}

#[test]
fn closing_folder_modal_resets_kind_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Infra".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());
    assert_eq!(app.get_asset_folder_modal_name().as_str(), "");
}

#[test]
fn ssh_modal_cancel_and_reopen_resets_grouped_form_draft_fields() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("proxy_method".into(), "jump-host".into());
    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_proxy_method().as_str(), "");

    app.invoke_assets_create_action_selected("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "SSH Connection 1");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_proxy_method().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_port().as_str(), "22");
}

#[test]
fn closing_ssh_modal_resets_kind_and_confirm_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    assert!(app.get_asset_modal_can_confirm());

    app.invoke_close_asset_modal_requested();

    assert!(!app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "");
    assert!(!app.get_asset_modal_can_confirm());
    assert_eq!(app.get_asset_ssh_modal_name().as_str(), "");
    assert_eq!(app.get_asset_ssh_modal_host().as_str(), "");
}

#[test]
fn folder_context_create_opens_child_targeted_ssh_modal() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(asset_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());

    assert!(app.get_asset_modal_open());
    assert_eq!(app.get_asset_modal_kind().as_str(), "new-ssh-connection");
    assert_eq!(app.get_console_asset_items().row_count(), 1);

    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Prod Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    let rows = app.get_console_asset_items();
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().id.as_str(), asset_id.as_str());
    assert_eq!(rows.row_data(1).unwrap().depth, 1);
    assert_eq!(rows.row_data(1).unwrap().kind.as_str(), "ssh");
}

#[test]
fn right_click_near_edge_still_keeps_overlay_within_window_bounds() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.window().set_size(PhysicalSize::new(760, 640));
    app.invoke_shell_layout_invalidated(760.0, 640.0);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 748.0, 632.0);

    let size = app.window().size();
    let origin_x = app.get_layout_assets_context_menu_origin_x();
    let origin_y = app.get_layout_assets_context_menu_origin_y();
    let overlay_width = app.get_layout_assets_context_menu_width();
    let overlay_height = app.get_layout_assets_context_menu_height();

    assert!(origin_x >= 0.0);
    assert!(origin_y >= 0.0);
    assert!(origin_x + overlay_width <= size.width as f32);
    assert!(origin_y + overlay_height <= size.height as f32);
}

#[test]
fn invoking_planned_action_shows_status_pill_feedback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("ssh-prod-01".into(), "ssh".into(), 144.0, 188.0);
    app.invoke_assets_context_menu_action_invoked("proxy-chrome-via-server".into());

    assert!(app.get_assets_context_menu_open());
    assert_eq!(
        app.get_context_menu_feedback_text().as_str(),
        "Proxy Chrome via Server is not wired yet."
    );
}

#[test]
fn closing_context_menu_clears_planned_action_feedback() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("ssh-prod-01".into(), "ssh".into(), 144.0, 188.0);
    app.invoke_assets_context_menu_action_invoked("proxy-chrome-via-server".into());
    app.invoke_close_assets_context_menu_requested();

    assert!(!app.get_assets_context_menu_open());
    assert_eq!(app.get_context_menu_feedback_text().as_str(), "");
}

#[test]
fn pointer_move_callback_exists_for_context_menu_corridor() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_asset_context_menu_requested("".into(), "blank".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_pointer_moved(120.0, 180.0);

    assert!(app.get_assets_context_menu_open());
}

#[test]
fn rename_action_opens_rename_modal_with_existing_name() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(asset_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());

    assert!(app.get_asset_rename_modal_open());
    assert_eq!(app.get_asset_rename_modal_name().as_str(), "Prod");
    assert_eq!(app.get_asset_rename_modal_validation_message().as_str(), "");
    assert!(app.get_asset_rename_modal_can_confirm());
}

#[test]
fn rename_modal_confirm_round_trips_through_window_properties() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(asset_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("rename-asset".into());
    app.invoke_asset_rename_modal_name_changed("Infra".into());
    app.invoke_confirm_asset_rename_requested();

    assert!(!app.get_asset_rename_modal_open());
    assert_eq!(
        app.get_console_asset_items()
            .row_data(0)
            .unwrap()
            .label
            .as_str(),
        "Infra"
    );
}

#[test]
fn delete_action_opens_delete_confirm_modal_with_nested_count() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Prod".into());
    app.invoke_confirm_asset_modal_requested();
    let asset_id = app
        .get_console_asset_items()
        .row_data(0)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(asset_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Bastion".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.12".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    app.invoke_asset_context_menu_requested(asset_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());

    assert!(app.get_asset_delete_confirm_modal_open());
    assert_eq!(app.get_asset_delete_confirm_target_label().as_str(), "Prod");
    assert_eq!(app.get_asset_delete_confirm_descendant_count(), 1);
}

#[test]
fn delete_confirm_round_trips_and_removes_window_rows() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Alpha".into());
    app.invoke_confirm_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Beta".into());
    app.invoke_confirm_asset_modal_requested();
    app.invoke_assets_create_action_selected("new-folder".into());
    app.invoke_asset_folder_modal_name_changed("Gamma".into());
    app.invoke_confirm_asset_modal_requested();

    let beta_id = app
        .get_console_asset_items()
        .row_data(1)
        .unwrap()
        .id
        .to_string();

    app.invoke_asset_context_menu_requested(beta_id.clone().into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("new-ssh-connection".into());
    app.invoke_asset_ssh_modal_draft_changed("name".into(), "Nested SSH".into());
    app.invoke_asset_ssh_modal_draft_changed("host".into(), "10.0.0.13".into());
    app.invoke_asset_ssh_modal_draft_changed("user".into(), "ops".into());
    app.invoke_asset_ssh_modal_draft_changed("password".into(), "secret".into());
    app.invoke_confirm_asset_modal_requested();

    app.invoke_asset_context_menu_requested(beta_id.into(), "folder".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("delete-asset".into());
    app.invoke_confirm_delete_asset_requested();

    let rows = app.get_console_asset_items();
    assert!(!app.get_asset_delete_confirm_modal_open());
    assert_eq!(rows.row_count(), 2);
    assert_eq!(rows.row_data(0).unwrap().label.as_str(), "Alpha");
    assert_eq!(rows.row_data(1).unwrap().label.as_str(), "Gamma");
}
