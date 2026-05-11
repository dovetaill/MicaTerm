use std::fs;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_profile,
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher,
};
use mica_term::app::runtime_profile::AppRuntimeProfile;
use mica_term::app::ssh::profile::ConnectionProfile;
use mica_term::app::ssh::runtime::{
    SessionRuntimeEvent, TerminalKeyEvent, TerminalMouseInput, TerminalSurfaceState,
};
use mica_term::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
use mica_term::app::ssh::session_manager::{SessionRuntimeControl, SessionRuntimeLauncher};
use mica_term::app::window_effects::default_platform_window_effects;
use mica_term::shell::tabs::WorkspaceTab;
use mica_term::shell::view_model::ShellViewModel;
use slint::Model;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Clone, Default)]
struct FakeLauncher;

struct NoopRuntimeControl;

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
        _session_id: Uuid,
        _attempt_id: Uuid,
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

fn bind_with_fake_sessions(app: &AppWindow) {
    bind_top_status_bar_with_store_and_effects_and_asset_repo_and_launcher(
        app,
        None,
        default_platform_window_effects(),
        None,
        Arc::new(FakeLauncher),
    );
}

fn sample_handle(title: &str, subtitle: &str, state: SessionState) -> SessionHandle {
    let can_reconnect = matches!(
        &state,
        SessionState::Cancelled | SessionState::Disconnected | SessionState::Error(_)
    );
    SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: "asset-prod".into(),
        title: title.into(),
        subtitle: subtitle.into(),
        state,
        can_reconnect,
        enhanced_session_state: EnhancedSessionState::Plain,
    }
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

#[test]
fn tab_model_prefers_asset_name_then_host_for_title() {
    let named = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connecting,
    ));
    let unnamed = WorkspaceTab::from_session(&sample_handle(
        "",
        "ops@example.com:22",
        SessionState::Connecting,
    ));

    assert_eq!(named.title, "Prod Bastion");
    assert_eq!(unnamed.title, "example.com");
}

#[test]
fn sftp_workspace_tab_uses_tab_identity_instead_of_terminal_session_identity() {
    let tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");

    assert_eq!(tab.tab_id, "tab-files-1");
    assert_eq!(tab.file_browser_session_id, "browser-1");
    assert_eq!(tab.title, "Files: Prod");
    assert_eq!(tab.kind, mica_term::shell::tabs::WorkspaceTabKind::Sftp);
    assert!(!tab.uses_terminal_surface());
}

#[test]
fn terminal_workspace_tabs_keep_tab_id_equal_to_their_terminal_session_id_for_now() {
    let handle = sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    );

    let tab = WorkspaceTab::from_session(&handle);

    assert_eq!(tab.tab_id, tab.session_id);
    assert!(tab.file_browser_session_id.is_empty());
}

#[test]
fn workspace_can_activate_sftp_tab_without_losing_terminal_tab_identity() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab.clone(), sftp_tab.clone()]);

    assert!(view_model.activate_workspace_tab(sftp_tab.tab_id.as_str()));
    assert_eq!(
        view_model
            .active_workspace_tab()
            .expect("active workspace tab")
            .title,
        "Files: Prod"
    );
    assert!(view_model.active_workspace_terminal_session_id().is_none());
}

#[test]
fn workspace_active_tab_summary_projects_host_then_display_name_for_titlebar() {
    let terminal_tab = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@10.0.0.12:22",
        SessionState::Connected,
    ));

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![terminal_tab]);

    let summary = view_model
        .active_workspace_tab_summary()
        .expect("active workspace tab summary");
    assert_eq!(summary.primary_summary_text, "10.0.0.12 · Prod Bastion");
    assert_eq!(
        summary.tooltip_text,
        "Prod Bastion\nHost: 10.0.0.12\nUser: ops\nPort: 22\nStatus: Connected"
    );
}

#[test]
fn workspace_active_tab_summary_avoids_empty_separator_when_host_is_missing() {
    let sftp_tab = WorkspaceTab::sftp("tab-files-1", "browser-1", "Files: Prod");

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![sftp_tab]);

    let summary = view_model
        .active_workspace_tab_summary()
        .expect("active workspace tab summary");
    assert_eq!(summary.primary_summary_text, "Files: Prod");
}

#[test]
fn workspace_tabs_hide_connection_details_from_visible_copy() {
    let named = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));

    assert_eq!(named.title, "Prod Bastion");
    assert_eq!(
        named.subtitle, "",
        "workspace tabs should not surface root@host connection details in visible chrome"
    );
}

#[test]
fn workspace_launcher_tab_projects_welcome_mode_without_runtime_session() {
    let mut view_model = ShellViewModel::default();

    view_model.open_workspace_launcher_tab();

    assert_eq!(view_model.workspace_tabs().len(), 1);
    assert!(view_model.workspace_tabs()[0].is_launcher());
    assert_eq!(view_model.workspace_session_host_mode(), "welcome");
    assert_eq!(
        view_model.active_workspace_session_id(),
        Some("workspace-launcher")
    );
}

#[test]
fn workspace_launcher_tab_is_singleton_when_opened_repeatedly() {
    let mut view_model = ShellViewModel::default();

    view_model.open_workspace_launcher_tab();
    view_model.open_workspace_launcher_tab();

    assert_eq!(view_model.workspace_tabs().len(), 1);
    assert!(view_model.workspace_tabs()[0].active);
}

#[test]
fn tab_model_tracks_active_session_and_closeability() {
    let first = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let second = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Connecting,
    ));

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![first.clone(), second.clone()]);

    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(first.session_id.as_str())
    );
    assert!(view_model.active_workspace_session_can_close());

    view_model.activate_workspace_session(second.session_id.as_str());

    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(second.session_id.as_str())
    );
    assert!(!view_model.show_welcome);
}

#[test]
fn disconnected_session_stays_visible_and_can_reconnect() {
    let mut disconnected = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Disconnected,
    ));
    disconnected.active = true;

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![disconnected.clone()]);

    assert_eq!(view_model.workspace_tabs().len(), 1);
    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());
}

#[test]
fn opening_slow_connecting_workspace_session_uses_connection_progress_mode() {
    let mut connecting = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connecting,
    ));
    connecting.active = true;
    assert!(
        !connecting.uses_terminal_surface(),
        "connecting tabs should stop claiming the terminal host before the runtime is ready"
    );

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![connecting]);

    assert_eq!(
        view_model.workspace_session_host_mode(),
        "connection-progress"
    );
}

#[test]
fn workspace_tab_projection_does_not_encode_container_width_behavior() {
    let tabs_projection = fs::read_to_string("src/shell/tabs.rs").expect("read tabs projection");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");

    assert!(
        !tabs_projection.contains("216px"),
        "workspace tab projection should not encode fixed container widths"
    );
    assert!(
        workspace_pane.contains("export component WorkspacePane"),
        "workspace width behavior should live in WorkspacePane instead of the tab projection"
    );
    assert!(
        workspace_pane.contains("TabBar {"),
        "WorkspacePane should own the tab strip contract"
    );
    assert!(
        workspace_pane.contains("TerminalSessionHost {"),
        "WorkspacePane should own the content host contract"
    );
}

#[test]
fn runtime_profile_source_exposes_terminal_render_mode_accessor() {
    let runtime_profile =
        fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(
        runtime_profile.contains("pub fn terminal_render_mode(self) -> TerminalRenderMode"),
        "runtime profile should expose an accessor for the selected terminal render mode"
    );
}

#[test]
fn native_profile_binding_keeps_native_terminal_frame_contract_available() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().expect("create app window");
    bind_top_status_bar_with_profile(&app, AppRuntimeProfile::mainline_native());

    assert_eq!(app.get_workspace_session_native_frame_token(), 0);
}

#[test]
fn closing_active_tab_falls_back_to_right_then_left_then_welcome() {
    let first = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    ));
    let second = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Connected,
    ));
    let third = WorkspaceTab::from_session(&sample_handle(
        "Dev Bastion",
        "ops@dev.example.com:22",
        SessionState::Connected,
    ));

    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![first.clone(), second.clone(), third.clone()]);
    view_model.activate_workspace_session(second.session_id.as_str());

    assert!(view_model.close_workspace_session(second.session_id.as_str()));
    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(third.session_id.as_str()),
        "closing the active tab should activate the tab on the right first"
    );

    assert!(view_model.close_workspace_session(third.session_id.as_str()));
    assert_eq!(
        view_model.active_workspace_session_id(),
        Some(first.session_id.as_str()),
        "closing the rightmost active tab should fall back to the left tab"
    );

    assert!(view_model.close_workspace_session(first.session_id.as_str()));
    assert_eq!(view_model.active_workspace_session_id(), None);
    assert!(view_model.show_welcome);
    assert_eq!(view_model.workspace_session_host_mode(), "welcome");
}

#[test]
fn close_affordance_is_modeled_separately_from_select_action() {
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");

    assert!(
        active_tab.contains("callback selected();"),
        "ActiveTab should expose a dedicated select callback"
    );
    assert!(
        active_tab.contains("callback close-requested();"),
        "ActiveTab should expose a dedicated close callback"
    );
    assert!(
        active_tab.contains("@image-url(\"../../assets/icons/fluent/dismiss-20-regular.svg\")"),
        "close affordance should use the shared Fluent dismiss icon instead of a text glyph"
    );
    assert!(
        !active_tab.contains("text: \"×\";"),
        "close affordance should be rendered as a real icon button instead of a text x"
    );
    assert!(
        active_tab.contains("root.close-requested();"),
        "close affordance should be wired independently instead of relying on the root select action"
    );
}

#[test]
fn active_tab_layout_preserves_close_hit_target_and_elides_text() {
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert!(
        active_tab.contains("min-width: 0px;"),
        "title/subtitle container should opt into shrink-safe layout"
    );
    assert!(
        active_tab.contains("overflow: elide;"),
        "tab text should elide instead of overflowing into the close hit target"
    );
    assert!(
        !active_tab.contains("text: root.subtitle;"),
        "workspace tab chips should keep only the saved terminal name visible"
    );
    assert!(
        !active_tab.contains("width: 16px;"),
        "close affordance should expose a larger hit target than the old 16px box"
    );
    assert!(
        active_tab.contains("content-hit-target := TouchArea {"),
        "selection should use its own hit target instead of relying on an implicit root overlay"
    );
    assert!(
        active_tab.contains("close-hit-target := TouchArea {"),
        "close affordance should use a dedicated hit target"
    );
    assert!(
        active_tab.contains("close-visible"),
        "tab chrome should explicitly model when the close button is visible"
    );
    assert!(
        !active_tab.contains("background: root.active ? ThemeTokens.accent : transparent;"),
        "VS Code-like tabs should rely on surface contrast instead of a browser-style accent strip"
    );
    assert!(
        !tabbar.contains("width: 216px;"),
        "tab strip should not hard-code a fixed tab width"
    );
}

#[test]
fn close_affordance_uses_stable_hit_geometry() {
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");

    assert!(
        !active_tab.contains("root.close-visible ? close-button.x : parent.width"),
        "content hit target width must not depend on close-visible hover state"
    );
    assert!(
        active_tab.contains("close-hit-target := TouchArea {"),
        "close hit target should remain a dedicated stable touch area"
    );
}

#[test]
fn tabbar_sizes_workspace_tabs_from_title_content_instead_of_even_stretch() {
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");
    let active_tab =
        fs::read_to_string("ui/components/active-tab.slint").expect("read active tab component");

    assert!(
        tabbar.contains(
            "for item[index] in root.items : ActiveTab {\n            horizontal-stretch: 0;"
        ),
        "workspace tabs should explicitly opt out of stretch layout inside the repeated tab row"
    );
    assert!(
        tabbar.contains("horizontal-stretch: 0;"),
        "workspace tabs should explicitly opt out of stretch layout so a single tab does not fill the row"
    );
    assert!(
        tabbar.contains("trailing-spacer := Rectangle {"),
        "tab row should keep unused width in a trailing spacer instead of stretching the last tab"
    );
    assert!(
        tabbar.contains("background: ThemeTokens.tabbar-background;"),
        "tab strip should sit on the dedicated semantic tabbar surface instead of the plain editor surface"
    );
    assert!(
        active_tab.contains("preferred-width"),
        "active tab layout should define an intrinsic width budget for title-driven sizing"
    );
}

#[test]
fn connected_session_projects_terminal_surface_state_without_placeholder_copy() {
    let handle = sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Connected,
    );
    let tab = WorkspaceTab::from_session(&handle);
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![tab]);
    view_model.set_active_workspace_terminal_surface(Some(
        TerminalSurfaceState::from_visible_lines(
            handle.session_id,
            3,
            24,
            80,
            vec!["last login: Tue Mar 24".into(), "pwd".into()],
        ),
    ));

    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert_eq!(view_model.workspace_session_host_mode(), "terminal");
    assert!(view_model.workspace_terminal_surface_ready());
    assert_eq!(view_model.workspace_terminal_surface_seqno(), 3);
    assert!(
        view_model
            .workspace_terminal_visible_lines()
            .iter()
            .any(|line| line.contains("last login"))
    );

    view_model.set_active_workspace_terminal_surface(Some(
        TerminalSurfaceState::from_visible_lines(
            handle.session_id,
            4,
            24,
            80,
            vec!["last login: Tue Mar 24".into(), "$ pwd".into()],
        ),
    ));

    assert_eq!(view_model.workspace_terminal_surface_seqno(), 4);
    assert!(
        view_model
            .workspace_terminal_visible_lines()
            .iter()
            .any(|line| line.contains("$ pwd")),
        "active terminal surface refreshes should update the visible terminal output projection"
    );

    view_model.set_active_workspace_terminal_surface(None);

    assert!(!view_model.workspace_terminal_surface_ready());
    assert_eq!(
        view_model.workspace_terminal_visible_lines(),
        Vec::<String>::new(),
        "clearing the active terminal surface should clear the published terminal frame state"
    );
    assert!(
        !terminal_host.contains("Interactive terminal ready."),
        "terminal host should not render decorative interactive-ready chrome above the terminal surface"
    );
    assert!(
        !terminal_host.contains("Remote shell is ready but has not produced output yet."),
        "terminal host should stop rendering placeholder copy once a real terminal surface contract exists"
    );
    assert!(
        !terminal_host.contains("Terminal Session"),
        "terminal host should not render a synthetic title above the terminal surface"
    );
    assert!(
        !terminal_host.contains("if root.session-subtitle != \"\""),
        "terminal host should not render session subtitles above the terminal surface"
    );
}

#[test]
fn workspace_tab_projects_enhanced_session_state_badge() {
    let handle = SessionHandle {
        session_id: Uuid::new_v4(),
        asset_id: "asset-prod".into(),
        title: "Prod".into(),
        subtitle: "ops@10.0.0.12:22".into(),
        state: SessionState::Connected,
        can_reconnect: false,
        enhanced_session_state: EnhancedSessionState::Fallback,
    };

    let tab = WorkspaceTab::from_session(&handle);

    assert_eq!(tab.enhanced_session_state, "fallback");
}

#[test]
fn workspace_session_host_hides_enhancement_state_and_disable_action() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        !terminal_host.contains("workspace-session-enhanced-state"),
        "terminal host should not expose enhanced-session state in the visible UI contract"
    );
    assert!(
        !terminal_host.contains("disable-enhanced-session"),
        "terminal host should not expose local enhanced-session disable actions"
    );
}

#[test]
fn terminal_session_host_exposes_text_key_and_resize_callbacks() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window.contains("callback workspace-session-text-input(string);"),
        "AppWindow should expose a workspace text-input callback for the active terminal session"
    );
    assert!(
        app_window.contains("callback workspace-session-key-input(string, bool, bool, bool);"),
        "AppWindow should expose a workspace key-input callback for non-printable terminal keys"
    );
    assert!(
        app_window.contains("callback workspace-session-resize-requested(int, int);"),
        "AppWindow should expose a workspace resize callback for terminal surfaces"
    );
    assert!(
        app_window.contains("callback workspace-session-normalize-hit-col(int, int) -> int;"),
        "AppWindow should expose a wide-char cell-hit normalization callback for the active terminal session"
    );
    assert!(
        app_window
            .contains("callback workspace-session-normalize-selection-hit-col(int, int) -> int;"),
        "AppWindow should expose a wide-char-aware selection-boundary normalization callback for the active terminal session"
    );
    assert!(
        workspace_pane.contains("text-input(text) =>"),
        "WorkspacePane should forward printable terminal input back to the app shell"
    );
    assert!(
        workspace_pane.contains("key-input(key, alt, ctrl, shift) =>"),
        "WorkspacePane should forward named key input back to the app shell"
    );
    assert!(
        workspace_pane.contains("surface-resize-requested(rows, cols) =>"),
        "WorkspacePane should forward terminal resize events back to the app shell"
    );
    assert!(
        workspace_pane.contains("callback normalize-hit-col(int, int) -> int;"),
        "WorkspacePane should expose a cell-hit normalization callback for TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("callback normalize-selection-hit-col(int, int) -> int;"),
        "WorkspacePane should expose a selection-boundary normalization callback for TerminalSessionHost"
    );
    assert!(
        terminal_host.contains("callback text-input(string);"),
        "TerminalSessionHost should emit printable text input"
    );
    assert!(
        terminal_host.contains("callback key-input(string, bool, bool, bool);"),
        "TerminalSessionHost should emit named key input with modifier state"
    );
    assert!(
        terminal_host.contains("callback surface-resize-requested(int, int);"),
        "TerminalSessionHost should emit resize requests with terminal rows/cols"
    );
    assert!(
        terminal_host.contains("callback normalize-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose a callback for Rust-side wide-char cell-hit normalization"
    );
    assert!(
        terminal_host.contains("callback normalize-selection-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose a callback for Rust-side wide-char selection-boundary normalization"
    );
    assert!(
        !terminal_host.contains("Remote shell is ready but has not produced output yet."),
        "interactive terminal host should stop rendering the old placeholder copy"
    );
}

#[test]
fn terminal_session_host_exposes_cell_cursor_selection_and_context_menu_contract() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window.contains("workspace-session-render-mode"),
        "AppWindow should expose a workspace terminal render mode so the software wrapper can select the bitmap fallback"
    );
    assert!(
        app_window.contains("workspace-session-surface-image"),
        "AppWindow should expose a rendered terminal image surface for atlas-backed fallback output"
    );
    assert!(
        app_window.contains("workspace-session-native-frame-token"),
        "AppWindow should expose the retained native frame token for native surface invalidation"
    );
    assert!(
        app_window.contains("workspace-session-cell-width"),
        "AppWindow should expose terminal cell width so overlays can track atlas geometry"
    );
    assert!(
        app_window.contains("workspace-session-cell-height"),
        "AppWindow should expose terminal cell height so overlays can track atlas geometry"
    );
    assert!(
        !app_window.contains("workspace-session-cells"),
        "AppWindow should not keep the old per-cell terminal model once atlas rendering owns the text surface"
    );
    assert!(
        app_window.contains("workspace-session-cursor-row"),
        "AppWindow should expose cursor row projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-cursor-fg"),
        "AppWindow should expose cursor foreground palette projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-cursor-bg"),
        "AppWindow should expose cursor background palette projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-default-fg"),
        "AppWindow should expose default foreground palette projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-default-bg"),
        "AppWindow should expose default background palette projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-cursor-col"),
        "AppWindow should expose cursor column projection for the terminal surface"
    );
    assert!(
        app_window.contains("workspace-session-copy-selection-requested(int, int, int, int);"),
        "AppWindow should expose a copy-selection callback for terminal text selection"
    );
    assert!(
        app_window.contains("callback workspace-session-normalize-hit-col(int, int) -> int;"),
        "AppWindow should expose a hit-column normalization callback so pointer-driven mouse reporting can collapse trailing wide cells back onto their owning glyph span"
    );
    assert!(
        app_window
            .contains("callback workspace-session-normalize-selection-hit-col(int, int) -> int;"),
        "AppWindow should expose a selection-hit normalization callback so half-cell terminal selection can snap wide-character interior boundaries onto stable cluster edges"
    );
    assert!(
        app_window.contains("workspace-session-paste-requested();"),
        "AppWindow should expose a paste callback for the terminal context menu"
    );
    assert!(
        app_window
            .contains("workspace-session-mouse-input(string, string, int, int, bool, bool, bool);"),
        "AppWindow should expose terminal mouse input forwarding for cell-relative pointer events"
    );
    assert!(
        app_window.contains("in-out property <bool> workspace-session-link-hovered: false;")
            && app_window.contains("in-out property <bool> workspace-session-link-armed: false;"),
        "AppWindow should store Rust-projected hovered and armed terminal-link affordance state instead of exposing a separate open-url callback pair"
    );
    assert!(
        app_window.contains("in-out property <int> workspace-session-viewport-offset-lines: 0;"),
        "AppWindow should expose the projected terminal viewport offset"
    );
    assert!(
        app_window
            .contains("in-out property <int> workspace-session-viewport-max-offset-lines: 0;"),
        "AppWindow should expose the projected terminal max viewport offset"
    );
    assert!(
        app_window.contains("in-out property <bool> workspace-session-viewport-at-bottom: true;"),
        "AppWindow should expose whether the terminal viewport is already at bottom"
    );
    assert!(
        !app_window.contains("workspace-session-follow-paused"),
        "AppWindow should stop exposing paused-follow chrome after removing the jump-to-latest affordance"
    );
    assert!(
        !app_window.contains("workspace-session-pending-output-lines"),
        "AppWindow should stop exposing pending-output badge state after removing the jump-to-latest affordance"
    );
    assert!(
        app_window.contains("in-out property <bool> workspace-session-selection-active: false;"),
        "AppWindow should expose terminal selection state for native clipboard shortcut fallbacks"
    );
    assert!(
        workspace_pane.contains("workspace-session-render-mode"),
        "WorkspacePane should forward the workspace terminal render mode into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-surface-image"),
        "WorkspacePane should forward the rendered terminal image surface into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-native-frame-token"),
        "WorkspacePane should forward the retained native frame token into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-cell-width"),
        "WorkspacePane should forward terminal cell width into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-cell-height"),
        "WorkspacePane should forward terminal cell height into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("session-cursor-fg: root.workspace-session-cursor-fg;"),
        "WorkspacePane should forward cursor foreground palette into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("session-cursor-bg: root.workspace-session-cursor-bg;"),
        "WorkspacePane should forward cursor background palette into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("session-default-fg: root.workspace-session-default-fg;"),
        "WorkspacePane should forward default foreground palette into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("session-default-bg: root.workspace-session-default-bg;"),
        "WorkspacePane should forward default background palette into TerminalSessionHost"
    );
    assert!(
        workspace_pane
            .contains("copy-selection-requested(start-row, start-col, end-row, end-col) =>"),
        "WorkspacePane should forward copy-selection requests back to the app shell"
    );
    assert!(
        workspace_pane.contains("callback normalize-hit-col(int, int) -> int;"),
        "WorkspacePane should expose a pointer-hit normalization callback for TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains(
            "normalize-selection-hit-col(row, col) => {\n                    return root.normalize-selection-hit-col(row, col);\n                }"
        ),
        "WorkspacePane should forward selection-hit normalization requests back to the app shell"
    );
    assert!(
        workspace_pane.contains("paste-requested() =>"),
        "WorkspacePane should forward paste requests back to the app shell"
    );
    assert!(
        workspace_pane.contains("mouse-input(kind, button, row, col, shift, ctrl, alt) =>"),
        "WorkspacePane should forward terminal mouse events back to the app shell"
    );
    assert!(
        workspace_pane.contains("in property <bool> workspace-session-link-hovered: false;")
            && workspace_pane.contains("in property <bool> workspace-session-link-armed: false;")
            && workspace_pane.contains("link-hovered: root.workspace-session-link-hovered;")
            && workspace_pane.contains("link-armed: root.workspace-session-link-armed;"),
        "WorkspacePane should forward Rust-projected hovered and armed terminal-link affordance state into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("normalize-hit-col(row, col) =>"),
        "WorkspacePane should forward wide-char cell-hit normalization requests back to the app shell"
    );
    assert!(
        workspace_pane.contains("normalize-selection-hit-col(row, col) =>"),
        "WorkspacePane should forward wide-char selection-boundary normalization requests back to the app shell"
    );
    assert!(
        workspace_pane.contains("workspace-session-viewport-offset-lines"),
        "WorkspacePane should forward the viewport offset projection into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-viewport-max-offset-lines"),
        "WorkspacePane should forward the viewport max offset projection into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-viewport-at-bottom"),
        "WorkspacePane should forward the viewport bottom-state projection into TerminalSessionHost"
    );
    assert!(
        !workspace_pane.contains("workspace-session-follow-paused"),
        "WorkspacePane should stop threading paused-follow chrome into TerminalSessionHost"
    );
    assert!(
        !workspace_pane.contains("workspace-session-pending-output-lines"),
        "WorkspacePane should stop threading pending-output badge state into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("selection-active <=> root.workspace-session-selection-active;"),
        "WorkspacePane should forward terminal selection state into TerminalSessionHost"
    );
    assert!(
        terminal_host.contains("callback copy-selection-requested(int, int, int, int);"),
        "TerminalSessionHost should emit a copy-selection callback"
    );
    assert!(
        terminal_host.contains("callback normalize-hit-col(int, int) -> int;")
            && terminal_host.contains("callback normalize-selection-hit-col(int, int) -> int;"),
        "TerminalSessionHost should expose separate pointer-hit and selection-hit normalization callbacks so mouse reporting and half-cell selection can share Rust-side wide-character ownership logic without reintroducing UI-side guesses"
    );
    assert!(
        terminal_host.contains("callback paste-requested();"),
        "TerminalSessionHost should emit a paste callback"
    );
    assert!(
        terminal_host.contains("callback mouse-input(string, string, int, int, bool, bool, bool);"),
        "TerminalSessionHost should emit mouse input callbacks with terminal-relative coordinates"
    );
    assert!(
        terminal_host.contains("in property <bool> link-hovered: false;")
            && terminal_host.contains("in property <bool> link-armed: false;"),
        "TerminalSessionHost should consume hovered and armed terminal-link affordance state from Rust instead of reparsing URLs in Slint"
    );
    assert!(
        terminal_host.contains("private property <bool> link-press-active: false;"),
        "TerminalSessionHost should keep a local pressed-link state so Ctrl+left-click can show a pressed affordance without adding another host callback"
    );
    assert!(
        terminal_host.contains("root.link-open-candidate = true;")
            && terminal_host.contains("root.link-press-active = true;")
            && terminal_host.contains("root.link-press-active = false;"),
        "TerminalSessionHost should arm and clear a local pressed-link affordance around Ctrl+left-click interactions"
    );
    assert!(
        terminal_host.contains("mouse-cursor: root.link-hovered")
            && terminal_host.contains("MouseCursor.pointer")
            && terminal_host.contains("Hold Ctrl and click to open link")
            && terminal_host.contains("Ctrl+click to open link")
            && terminal_host.contains("animate y")
            && terminal_host.contains("animate opacity"),
        "TerminalSessionHost should surface browser-safe links with pointer, hover copy, armed copy, and a small pressed animation"
    );
    assert!(
        terminal_host.contains("root.mouse-input(")
            && terminal_host.contains("\"move\"")
            && terminal_host.contains("\"down\"")
            && terminal_host.contains("\"up\"")
            && terminal_host.contains("\"left\""),
        "TerminalSessionHost should keep Ctrl+left-click link interaction on the existing mouse-input callback chain instead of routing through a dedicated open-url callback"
    );
    assert!(
        terminal_host.contains("callback scroll-thumb-drag-requested(float);"),
        "TerminalSessionHost should emit thumb drag requests for local scrollback"
    );
    assert!(
        app_window.contains("normalize-hit-col(row, col) =>"),
        "AppWindow should forward cell-hit normalization requests from WorkspacePane back into bootstrap"
    );
    assert!(
        app_window.contains("normalize-selection-hit-col(row, col) =>"),
        "AppWindow should forward selection-boundary normalization requests from WorkspacePane back into bootstrap"
    );
    assert!(
        terminal_host.contains("callback scroll-jump-requested(float);"),
        "TerminalSessionHost should emit track jump requests for local scrollback"
    );
    assert!(
        !terminal_host.contains("jump-to-latest-requested();"),
        "TerminalSessionHost should stop exposing a jump-to-latest callback"
    );
    assert!(
        !terminal_host.contains("session-follow-paused"),
        "TerminalSessionHost should stop accepting paused-follow state once the pill is removed"
    );
    assert!(
        !terminal_host.contains("session-pending-output-lines"),
        "TerminalSessionHost should stop accepting pending-output state once the pill is removed"
    );
    assert!(
        terminal_host.contains("private property <length> terminal-font-size"),
        "TerminalSessionHost should centralize the terminal font size in one visual contract"
    );
    assert!(
        terminal_host.contains("private property <length> terminal-font-size: 16px;"),
        "TerminalSessionHost should default to a desktop-readable font size"
    );
    assert!(
        terminal_host.contains("in property <string> session-render-mode: \"bitmap\";"),
        "TerminalSessionHost should accept a rendered terminal mode contract for the bitmap fallback"
    );
    assert!(
        terminal_host.contains("in property <image> session-surface-image;"),
        "TerminalSessionHost should accept the rendered terminal surface image contract for the bitmap fallback"
    );
    assert!(
        terminal_host.contains("in property <int> session-native-frame-token: 0;"),
        "TerminalSessionHost should accept a native frame token as the root-level retained surface contract"
    );
    assert!(
        terminal_host.contains("function terminal-cell-x("),
        "TerminalSessionHost should centralize cell x-position geometry in a shared helper"
    );
    assert!(
        terminal_host.contains("function terminal-cell-y("),
        "TerminalSessionHost should centralize cell y-position geometry in a shared helper"
    );
    assert!(
        terminal_host.contains("function terminal-hit-row("),
        "TerminalSessionHost should centralize row hit testing in a shared helper"
    );
    assert!(
        terminal_host.contains("function terminal-hit-col("),
        "TerminalSessionHost should centralize column hit testing in a shared helper"
    );
    assert!(
        terminal_host.contains("blank-surface := Rectangle {"),
        "TerminalSessionHost should render a dedicated blank terminal canvas behind cell content"
    );
    assert!(
        terminal_host.contains("background: root.session-default-bg;"),
        "TerminalSessionHost should paint the blank canvas from the runtime-projected default background"
    );
    assert!(
        terminal_host.contains("background: root.session-cursor-bg;"),
        "TerminalSessionHost should paint the cursor from the runtime-projected cursor background"
    );
    assert!(
        terminal_host.contains("Image {"),
        "TerminalSessionHost should display the terminal body through a Slint image surface when the bitmap fallback is active"
    );
    assert!(
        !terminal_host.contains("for cell in root.session-cells"),
        "TerminalSessionHost should stop expanding one UI node per terminal cell"
    );
    assert!(
        !terminal_host.contains("font-family: root.terminal-font-family;"),
        "TerminalSessionHost should no longer depend on Slint text nodes for terminal body rendering"
    );
    assert!(
        terminal_host.contains("event.modifiers.control && event.modifiers.shift"),
        "TerminalSessionHost should reserve Ctrl+Shift shortcuts for local clipboard actions"
    );
    assert!(
        terminal_host.contains("\\u{3}"),
        "TerminalSessionHost should recognize control-character copy tokens when Ctrl+Shift+C does not yield a printable `C`"
    );
    assert!(
        terminal_host.contains("\\u{16}"),
        "TerminalSessionHost should recognize control-character paste tokens when Ctrl+Shift+V does not yield a printable `V`"
    );
    assert!(
        terminal_host.contains("event.text == Key.Control"),
        "TerminalSessionHost should explicitly ignore a bare Control press before any remote forwarding branch"
    );
    assert!(
        terminal_host.contains("event.text == Key.Shift"),
        "TerminalSessionHost should explicitly ignore a bare Shift press before any remote forwarding branch"
    );
    assert!(
        terminal_host.contains("event.modifiers.control && event.text == Key.Insert"),
        "TerminalSessionHost should handle Ctrl+Insert as a copy shortcut"
    );
    assert!(
        terminal_host.contains("event.text == Key.Insert"),
        "TerminalSessionHost should handle Shift+Insert as a paste shortcut"
    );
    assert!(
        terminal_host.contains("in-out property <bool> selection-active: false;"),
        "TerminalSessionHost should project selection activity so native shortcut fallbacks can see current terminal selection state"
    );
    assert!(
        terminal_host.contains("function sync-selection-state()"),
        "TerminalSessionHost should centralize selection projection updates instead of leaving native copy fallback state stale"
    );
    assert!(
        terminal_host.contains("event.modifiers.shift && event.text == Key.PageUp"),
        "TerminalSessionHost should handle Shift+PageUp as a local scrollback shortcut"
    );
    assert!(
        terminal_host.contains("event.modifiers.shift && event.text == Key.PageDown"),
        "TerminalSessionHost should handle Shift+PageDown as a local scrollback shortcut"
    );
    assert!(
        terminal_host.contains("event.modifiers.shift && event.text == Key.Home"),
        "TerminalSessionHost should handle Shift+Home as a jump-to-top shortcut"
    );
    assert!(
        terminal_host.contains("event.modifiers.shift && event.text == Key.End"),
        "TerminalSessionHost should handle Shift+End as a jump-to-bottom shortcut"
    );
    assert!(
        !terminal_host.contains("event.modifiers.control && !event.modifiers.alt && !event.modifiers.shift && event.text == \"c\""),
        "TerminalSessionHost should not intercept plain Ctrl+C for local copy"
    );
    assert!(
        !terminal_host.contains("event.modifiers.control && !event.modifiers.alt && !event.modifiers.shift && event.text == \"v\""),
        "TerminalSessionHost should not intercept plain Ctrl+V for local paste"
    );
    assert!(
        !terminal_host.contains("event.modifiers.control && !event.modifiers.alt && !event.modifiers.shift && event.text == \"a\""),
        "TerminalSessionHost should not hijack plain Ctrl+A for local select-all because screen/tmux use it as a terminal prefix key"
    );
    assert!(
        terminal_host.contains("cursor-blink-timer := Timer {"),
        "terminal host should manage a visible cursor blink timer instead of rendering a cursorless text dump"
    );
    assert!(
        terminal_host.contains("interval: 600ms;"),
        "terminal cursor blink timing should stay near a 1.2s full cycle instead of the more urgent 520ms cadence"
    );
    assert!(
        !terminal_host.contains("changed session-surface-seqno => {"),
        "bitmap cursor blinking should not be reset from every incoming frame seqno because that turns terminal activity into a perpetual blink restart"
    );
    assert!(
        terminal_host.contains("changed session-cursor-row => {")
            && terminal_host.contains("changed session-cursor-col => {"),
        "bitmap cursor blink resets should follow real cursor movement instead of generic surface churn"
    );
    assert!(
        terminal_host.contains("opacity: root.session-cursor-shape == \"block\" ? 0.6 : 1.0;"),
        "bitmap block cursor should use a softer fill so underlying text stays readable while bar and underline cursors remain fully crisp"
    );
    assert!(
        terminal_host.contains("scrollbar-track := Rectangle {"),
        "TerminalSessionHost should render a dedicated scrollbar track"
    );
    assert!(
        terminal_host.contains("scrollbar-thumb := Rectangle {"),
        "TerminalSessionHost should render a dedicated scrollbar thumb"
    );
    assert!(
        !terminal_host.contains("Jump to latest"),
        "TerminalSessionHost should no longer render a jump-to-latest affordance"
    );
    assert!(
        !terminal_host.contains("terminal-lines := ListView {"),
        "terminal host should stop rendering the terminal as a ListView of plain strings"
    );
    assert!(
        !app_window.contains("in-out property <color> workspace-session-cursor-bg: #52ad70;"),
        "AppWindow should not retain the stale green cursor fallback constant"
    );
    assert!(
        !app_window.contains("in-out property <color> workspace-session-default-bg: #ffffff;"),
        "AppWindow should not retain the stale white terminal background fallback constant"
    );
    assert!(
        !workspace_pane.contains("in property <color> workspace-session-cursor-bg: #52ad70;"),
        "WorkspacePane should not retain the stale green cursor fallback constant"
    );
    assert!(
        !workspace_pane.contains("in property <color> workspace-session-default-bg: #ffffff;"),
        "WorkspacePane should not retain the stale white terminal background fallback constant"
    );
    assert!(
        !terminal_host.contains("in property <color> session-cursor-bg: #52ad70;"),
        "TerminalSessionHost should not retain the stale green cursor fallback constant"
    );
    assert!(
        !terminal_host.contains("in property <color> session-default-bg: #ffffff;"),
        "TerminalSessionHost should not retain the stale white terminal background fallback constant"
    );
}

#[test]
fn terminal_session_host_keeps_reserved_ctrl_shift_shortcuts_local_contract() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        terminal_host.contains(
            "event.text == Key.Control || event.text == Key.Shift || event.text == Key.Alt"
        ),
        "TerminalSessionHost should explicitly ignore bare modifier keys before any remote forwarding branch"
    );
    assert!(
        terminal_host.contains("text == \"f\" || text == \"F\""),
        "TerminalSessionHost should reserve Ctrl+Shift+F inside the local shortcut namespace"
    );
    assert!(
        terminal_host.contains("root.terminal-local-action-id(event.text) != \"\""),
        "TerminalSessionHost should route reserved Ctrl+Shift local actions through the centralized local action resolver"
    );
    assert!(
        terminal_host.contains(
            "} else if ((event.modifiers.control || event.modifiers.alt) && event.text != \"\")"
        ),
        "TerminalSessionHost should forward generic control and alt modified text through the shared remote forwarding branch"
    );
    assert!(
        terminal_host.contains(
            "root.key-input(event.text, event.modifiers.alt, event.modifiers.control, event.modifiers.shift);"
        ),
        "TerminalSessionHost should preserve the shift modifier when forwarding generic control and alt text input"
    );
}

#[test]
fn terminal_session_host_uses_compact_terminal_layout_contract() {
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");
    let font_backend =
        fs::read_to_string("src/app/terminal_font/backend.rs").expect("read font backend");

    assert!(
        !terminal_host.contains("terminal-cell-width: 9px;"),
        "terminal host should tighten the cell width from the old wide placeholder layout"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-font-size: 12px;"),
        "terminal host should tighten the prototype font size to a denser IDE-like default"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-font-size: 18px;"),
        "terminal host should stop mirroring the old 18px-style terminal default"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-cell-width: 8px;"),
        "terminal host should tighten the prototype cell width to a denser IDE-like default"
    );
    assert!(
        !terminal_host.contains("terminal-cell-height: 18px;"),
        "terminal host should tighten the cell height from the old loose placeholder layout"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-cell-height: 16px;"),
        "terminal host should tighten the prototype cell height to a denser IDE-like default"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-padding-left: 24px;"),
        "terminal host should not keep the oversized terminal-surface left padding"
    );
    assert!(
        !terminal_host.contains("private property <length> terminal-padding-top: 24px;"),
        "terminal host should not keep the oversized terminal-surface top padding"
    );
    assert!(
        !terminal_host.contains("font-size: 20px;"),
        "terminal host should not keep the oversized title treatment"
    );
    assert!(
        !terminal_host
            .contains("Reconnect is available once session lifecycle wiring is completed."),
        "session error host should remove the placeholder reconnect copy"
    );
    assert!(
        font_backend.contains("pub const DEFAULT_TERMINAL_FONT_SIZE_PX: f32 = 14.0;"),
        "workspace terminal contracts should source their default typography from the new 14px shared terminal font size"
    );
}

#[test]
fn disconnected_and_error_tabs_remain_reconnectable() {
    let disconnected = WorkspaceTab::from_session(&sample_handle(
        "Prod Bastion",
        "ops@example.com:22",
        SessionState::Disconnected,
    ));
    let errored = WorkspaceTab::from_session(&sample_handle(
        "Staging Bastion",
        "ops@staging.example.com:22",
        SessionState::Error("authentication failed".into()),
    ));
    let mut view_model = ShellViewModel::default();
    view_model.set_workspace_tabs(vec![disconnected.clone(), errored.clone()]);

    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());

    assert!(view_model.activate_workspace_session(errored.session_id.as_str()));
    assert_eq!(view_model.workspace_session_host_mode(), "session-error");
    assert!(view_model.active_workspace_session_can_reconnect());
    assert!(
        tabbar.contains("connecting"),
        "tabbar should carry the connecting state contract through to the tab items"
    );
    assert!(
        tabbar.contains("error"),
        "tabbar should carry the error state contract through to the tab items"
    );
}

#[test]
fn workspace_pane_only_renders_tabbar_when_tabs_exist() {
    let workspace =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    assert!(
        workspace.contains("if root.workspace-tab-items.length > 0 : tab-strip := TabBar {"),
        "workspace pane should only render the tab strip when at least one tab exists"
    );
}

#[test]
fn connection_progress_workspace_host_contract_exposes_timeline_models_and_footer_actions() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window.contains("workspace-session-connection-steps"),
        "AppWindow should expose a connection timeline steps model for the active workspace session"
    );
    assert!(
        app_window.contains("workspace-session-connection-diagnostics"),
        "AppWindow should expose a connection diagnostics model for the active workspace session"
    );
    assert!(
        app_window.contains("workspace-session-connection-current-detail"),
        "AppWindow should expose the current connection detail line for the timeline host"
    );
    assert!(
        workspace_pane.contains("workspace-session-connection-steps"),
        "WorkspacePane should forward connection timeline rows into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-connection-diagnostics"),
        "WorkspacePane should forward connection diagnostics into TerminalSessionHost"
    );
    assert!(
        terminal_host.contains("if root.mode == \"connection-progress\""),
        "TerminalSessionHost should render a dedicated connection-progress branch"
    );
    assert!(
        terminal_host.contains("header-block := Rectangle {"),
        "connection-progress host should expose a compact page header inside the redesigned single-sheet skeleton"
    );
    assert!(
        terminal_host.contains("hop-card := Rectangle {"),
        "connection-progress host should expose a compact hop chain card inside the redesigned single-sheet skeleton"
    );
    assert!(
        terminal_host.contains("for step in root.connection-progress-steps"),
        "connection-progress host should keep iterating over connection progress steps inside the connecting state"
    );
    assert!(
        terminal_host.contains("main-card := Rectangle {"),
        "connection-progress host should expose a unified main task card inside the redesigned single-sheet skeleton"
    );
    assert!(
        terminal_host.contains("details-card := Rectangle {"),
        "connection-progress host should expose a dedicated connection details disclosure section inside the redesigned single-sheet skeleton"
    );
    assert!(
        !terminal_host.contains("header-card := Rectangle {"),
        "connection-progress host should drop the old stacked header card"
    );
    assert!(
        !terminal_host.contains("timeline-card := Rectangle {"),
        "connection-progress host should drop the old stacked timeline card"
    );
    assert!(
        !terminal_host.contains("current-detail-card := Rectangle {"),
        "connection-progress host should drop the old split current-detail card"
    );
    assert!(
        !terminal_host.contains("diagnostics-card := Rectangle {"),
        "connection-progress host should drop the old standalone diagnostics card"
    );
    assert!(
        terminal_host.contains("Diagnostics"),
        "connection-progress host should expose a diagnostics disclosure label"
    );
    assert!(
        !terminal_host.contains("Show Diagnostics"),
        "connection-progress host should stop exposing the old show diagnostics copy"
    );
    assert!(
        !terminal_host.contains("Hide Diagnostics"),
        "connection-progress host should stop exposing the old hide diagnostics copy"
    );
    assert!(
        terminal_host.contains("Copy details"),
        "connection-progress host should expose a copy details action"
    );
    assert!(
        !terminal_host.contains("Copy Diagnostics"),
        "connection-progress host should stop exposing the old copy diagnostics action"
    );
    assert!(
        terminal_host.contains("ThemeTokens.ssh-status-card-elevated-surface"),
        "connection-progress hop chain should use the quieter elevated SSH status surfaces instead of the old bright workflow treatment"
    );
    assert!(
        terminal_host.contains("Cancel"),
        "connection-progress host should expose a cancel footer action while connecting"
    );
    assert!(
        terminal_host.contains("Retry"),
        "connection-progress host should expose a retry footer action for failed attempts"
    );
}

#[test]
fn connection_progress_workspace_host_contract_exposes_inline_host_key_actions() {
    fn action_block<'a>(source: &'a str, action: &str) -> &'a str {
        let action_index = source
            .find(action)
            .unwrap_or_else(|| panic!("missing action block: {action}"));
        let block_start = source[..action_index]
            .rfind("if root.connection-progress-visual-state")
            .unwrap_or(0);
        let block_end = source[action_index..]
            .find("if root.connection-progress-visual-state")
            .map(|offset| action_index + offset)
            .unwrap_or(source.len());
        &source[block_start..block_end]
    }

    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window.contains("workspace-session-host-key-prompt-host"),
        "AppWindow should expose the active workspace host-key prompt host"
    );
    assert!(
        app_window.contains("workspace-session-host-key-prompt-fingerprint"),
        "AppWindow should expose the active workspace host-key prompt fingerprint"
    );
    assert!(
        workspace_pane.contains("workspace-session-host-key-prompt-host"),
        "WorkspacePane should forward host-key prompt host state into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains("workspace-session-host-key-prompt-fingerprint"),
        "WorkspacePane should forward host-key prompt fingerprint state into TerminalSessionHost"
    );

    let trust_action_window = action_block(&terminal_host, "trust-host-key");
    assert!(
        trust_action_window.contains("Trust & Continue"),
        "connection-progress host should expose the reference-style trust action for unknown host keys"
    );
    assert!(
        terminal_host.contains("trust-host-key"),
        "host-key trust should route back through the workspace local-action callback"
    );
    let reject_action_window = action_block(&terminal_host, "reject-host-key");
    assert!(
        reject_action_window.contains("Reject"),
        "connection-progress host should expose the reference-style reject action for unknown host keys"
    );
    assert!(
        terminal_host.contains("reject-host-key"),
        "host-key rejection should route back through the workspace local-action callback"
    );
    let footer_cancel_guard_index = terminal_host
        .find("function connection-progress-shows-cancel() -> bool {")
        .expect("page-level cancel guard should remain defined");
    let footer_cancel_guard_window = &terminal_host
        [footer_cancel_guard_index..(footer_cancel_guard_index + 240).min(terminal_host.len())];
    assert!(
        footer_cancel_guard_window
            .contains("root.connection-progress-visual-state == \"verifying_host_key\""),
        "page-level cancel actions should stay available during verification and connecting states in the reference flow"
    );
    let action_bar_index = terminal_host
        .find("action-bar := Rectangle {")
        .expect("connection-progress host should still expose a dedicated page-level action bar");
    let action_bar_window =
        &terminal_host[action_bar_index..(action_bar_index + 1400).min(terminal_host.len())];
    assert!(
        action_bar_window.contains("Cancel connection"),
        "page-level action bar should keep the low-emphasis cancel affordance aligned with the reference layout"
    );
    assert!(
        !terminal_host.contains("host-key-card := Rectangle {"),
        "connection-progress host should avoid reviving the old standalone host-key card shell"
    );
}

#[test]
fn connection_progress_workspace_host_contract_exposes_presentation_semantics_props() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let terminal_host =
        fs::read_to_string("ui/shell/terminal-session-host.slint").expect("read terminal host");

    assert!(
        app_window
            .contains("in-out property <string> workspace-session-connection-page-mode: \"\";"),
        "AppWindow should expose a projected connection page-mode property"
    );
    assert!(
        app_window
            .contains("in-out property <string> workspace-session-connection-task-title: \"\";"),
        "AppWindow should expose a projected connection task-title property"
    );
    assert!(
        app_window
            .contains("in-out property <string> workspace-session-connection-task-detail: \"\";"),
        "AppWindow should expose a projected connection task-detail property"
    );
    assert!(
        app_window.contains(
            "workspace-session-connection-page-mode: root.workspace-session-connection-page-mode;"
        ),
        "AppWindow should forward the connection page-mode property into WorkspacePane"
    );
    assert!(
        app_window.contains(
            "workspace-session-connection-task-title: root.workspace-session-connection-task-title;"
        ),
        "AppWindow should forward the connection task-title property into WorkspacePane"
    );
    assert!(
        app_window.contains("workspace-session-connection-task-detail: root.workspace-session-connection-task-detail;"),
        "AppWindow should forward the connection task-detail property into WorkspacePane"
    );
    assert!(
        workspace_pane
            .contains("in property <string> workspace-session-connection-page-mode: \"\";"),
        "WorkspacePane should accept the projected connection page-mode property"
    );
    assert!(
        workspace_pane
            .contains("in property <string> workspace-session-connection-task-title: \"\";"),
        "WorkspacePane should accept the projected connection task-title property"
    );
    assert!(
        workspace_pane
            .contains("in property <string> workspace-session-connection-task-detail: \"\";"),
        "WorkspacePane should accept the projected connection task-detail property"
    );
    assert!(
        workspace_pane.contains(
            "connection-progress-page-mode: root.workspace-session-connection-page-mode;"
        ),
        "WorkspacePane should forward the connection page-mode property into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains(
            "connection-progress-task-title: root.workspace-session-connection-task-title;"
        ),
        "WorkspacePane should forward the connection task-title property into TerminalSessionHost"
    );
    assert!(
        workspace_pane.contains(
            "connection-progress-task-detail: root.workspace-session-connection-task-detail;"
        ),
        "WorkspacePane should forward the connection task-detail property into TerminalSessionHost"
    );
    assert!(
        terminal_host.contains("in property <string> connection-progress-page-mode: \"\";"),
        "TerminalSessionHost should accept the projected connection page-mode property"
    );
    assert!(
        terminal_host.contains("in property <string> connection-progress-task-title: \"\";"),
        "TerminalSessionHost should accept the projected connection task-title property"
    );
    assert!(
        terminal_host.contains("in property <string> connection-progress-task-detail: \"\";"),
        "TerminalSessionHost should accept the projected connection task-detail property"
    );
    assert!(
        terminal_host
            .contains("if root.connection-progress-visual-state == \"failed\" : Rectangle {"),
        "TerminalSessionHost should keep a dedicated failed-state block inside the unified connection sheet"
    );
    let session_error_index = terminal_host
        .find("if root.mode == \"session-error\" : Rectangle {")
        .expect("terminal host should still expose the generic session-error fallback branch");
    let session_error_window = &terminal_host[session_error_index..];
    assert!(
        !session_error_window.contains("Retry"),
        "generic session-error fallback should stay distinct from the retry-capable connection sheet troubleshooting state"
    );
}

#[test]
fn single_click_only_selects_saved_asset_without_opening_session() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_selected(ssh_id.clone().into());
    assert_eq!(app.get_workspace_tab_items().row_count(), 0);
    assert!(
        app.get_console_asset_items()
            .row_data(0)
            .expect("selected ssh row")
            .selected
    );
}

#[test]
fn double_click_and_context_menu_open_create_distinct_sessions() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_with_fake_sessions(&app);

    let ssh_id = create_root_ssh(&app, "Prod Bastion", "10.0.0.12");
    app.invoke_asset_activated(ssh_id.clone().into());
    let first_session_id = app
        .get_workspace_tab_items()
        .row_data(0)
        .expect("first workspace tab")
        .tab_id
        .to_string();

    app.invoke_asset_context_menu_requested(ssh_id.into(), "ssh".into(), 96.0, 160.0);
    app.invoke_assets_context_menu_action_invoked("open-connection".into());

    assert_eq!(app.get_workspace_tab_items().row_count(), 2);
    let second_session_id = app
        .get_workspace_tab_items()
        .row_data(1)
        .expect("second workspace tab")
        .tab_id
        .to_string();
    assert_ne!(first_session_id, second_session_id);
    assert_eq!(
        app.get_active_workspace_session_id().as_str(),
        second_session_id
    );
}

#[test]
fn workspace_tab_projection_uses_tab_identity_for_generic_tabbar_selection() {
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert!(
        bootstrap.contains("tab_id: tab.tab_id.clone().into()"),
        "workspace tab projection should feed tab ids into the tabbar model so terminal and sftp tabs share one selection identity contract"
    );
    assert!(
        tabbar.contains("title: item.title;"),
        "TabBar should keep rendering the generic workspace tab title binding so SFTP tabs can display `Files: <host>` without a special case"
    );
}

#[test]
fn workspace_tabbar_drag_reorder_contract_is_wired_end_to_end() {
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let workspace_pane =
        fs::read_to_string("ui/shell/workspace-pane.slint").expect("read workspace pane");
    let tabbar = fs::read_to_string("ui/shell/tabbar.slint").expect("read tabbar");

    assert!(
        tabbar.contains("callback tab-reorder-requested(string, int);"),
        "TabBar should expose a final drop callback so drag reorder commits through stable tab ids instead of mutating UI order implicitly"
    );
    assert!(
        workspace_pane.contains("callback workspace-tab-reorder-requested(string, int);"),
        "WorkspacePane should forward tab reorder requests through its shared workspace callback surface"
    );
    assert!(
        app_window.contains("callback workspace-tab-reorder-requested(string, int);"),
        "AppWindow should surface the workspace tab reorder callback so Rust owns the final UI-order commit"
    );
    assert!(
        bootstrap.contains("window.on_workspace_tab_reorder_requested"),
        "bootstrap should handle final tab reorder drops and route them into ShellViewModel::reorder_workspace_tab"
    );
}

#[test]
fn titlebar_active_session_summary_contract_exposes_primary_summary_lane() {
    let app_window = fs::read_to_string("ui/app-window.slint").expect("read app window");
    let titlebar = fs::read_to_string("ui/shell/titlebar.slint").expect("read titlebar");

    assert!(
        app_window.contains("in-out property <string> active-session-primary-summary: \"\";"),
        "AppWindow should expose a dedicated primary summary string for the titlebar lane"
    );
    assert!(
        app_window.contains("active-session-primary-summary: root.active-session-primary-summary;"),
        "AppWindow should forward the primary summary string into Titlebar"
    );
    assert!(
        titlebar.contains("in property <string> active-session-primary-summary: \"\";"),
        "Titlebar should accept a dedicated primary summary string"
    );
    assert!(
        titlebar.contains("text: root.active-session-primary-summary;"),
        "Titlebar should render the primary summary lane from the dedicated summary string"
    );
}
