//! Smoke coverage for titlebar bindings, theme sync, and auxiliary actions.

use std::cell::RefCell;
use std::fs;
use std::rc::Rc;
use std::time::Duration;

use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, bind_top_status_bar_with_store_and_effects,
    runtime_window_title,
};
use mica_term::app::logging::config::{AppLogMode, AppLoggingConfig};
use mica_term::app::logging::paths::{LoggingPaths, LoggingRootSource};
use mica_term::app::logging::runtime::build_test_logging_runtime;
use mica_term::app::runtime_profile::AppRuntimeProfile;
use mica_term::app::ui_preferences::UiPreferencesStore;
use mica_term::app::window_effects::{
    BackdropApplyStatus, NativeWindowAppearanceRequest, NativeWindowCornerPreference,
    NativeWindowTheme, PlatformWindowEffects, WindowAppearanceSyncReport,
};
use mica_term::shell::metrics::ShellMetrics;
use slint::platform::{PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition, PhysicalSize};

#[derive(Clone)]
struct RecordingWindowEffects {
    requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>,
}

impl RecordingWindowEffects {
    fn new(requests: Rc<RefCell<Vec<NativeWindowAppearanceRequest>>>) -> Self {
        Self { requests }
    }
}

impl PlatformWindowEffects for RecordingWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        self.requests.borrow_mut().push(*request);
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Applied,
            backdrop_error: None,
            redraw_requested: request.request_redraw,
        }
    }
}

#[derive(Clone)]
struct FailingBackdropWindowEffects {
    error_text: &'static str,
}

impl PlatformWindowEffects for FailingBackdropWindowEffects {
    fn apply_to_app_window(
        &self,
        _window: &AppWindow,
        request: &NativeWindowAppearanceRequest,
    ) -> WindowAppearanceSyncReport {
        WindowAppearanceSyncReport {
            theme_applied: true,
            backdrop_status: BackdropApplyStatus::Failed,
            backdrop_error: Some(self.error_text.to_string()),
            redraw_requested: request.request_redraw,
        }
    }
}

#[test]
fn app_title_stays_stable_for_mainline_profile() {
    assert_eq!(
        runtime_window_title(AppRuntimeProfile::mainline()),
        "Mica Term"
    );
}

#[test]
fn app_window_title_is_runtime_bound() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(content.contains("in property <string> window-title"));
    assert!(content.contains("title: root.window-title;"));
}

#[test]
fn app_window_source_no_longer_exposes_recovery_mask_contract() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!content.contains("render-revision"));
    assert!(!content.contains("experimental-recovery-mask"));
}

#[test]
fn app_window_source_does_not_expose_flat_window_chrome_binding() {
    let content = std::fs::read_to_string("ui/app-window.slint").unwrap();

    assert!(!content.contains("use-flat-window-chrome"));
}

#[test]
fn bootstrap_binds_top_status_bar_callbacks_to_window_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-ui-preferences.json");
    let _ = fs::remove_file(&temp_path);

    app.set_dark_mode(false);
    app.set_show_right_panel(true);
    app.set_show_global_menu(true);
    app.set_is_window_maximized(true);
    app.set_is_window_active(false);
    app.set_is_window_always_on_top(true);

    bind_top_status_bar_with_store(&app, Some(UiPreferencesStore::new(temp_path.clone())));

    assert!(app.get_dark_mode());
    assert!(!app.get_show_right_panel());
    assert!(!app.get_show_global_menu());
    assert!(!app.get_is_window_maximized());
    assert!(app.get_is_window_active());
    assert!(!app.get_is_window_always_on_top());
    assert!(app.get_show_assets_sidebar());
    assert_eq!(app.get_active_sidebar_destination().as_str(), "console");

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_right_panel());
    assert_eq!(app.get_right_panel_view().as_str(), "sftp");

    app.invoke_toggle_global_menu_requested();
    assert!(app.get_show_global_menu());

    app.invoke_close_global_menu_requested();
    assert!(!app.get_show_global_menu());

    app.invoke_toggle_theme_mode_requested();
    assert!(!app.get_dark_mode());

    app.invoke_toggle_window_always_on_top_requested();
    assert!(app.get_is_window_always_on_top());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());

    let _ = fs::remove_file(temp_path);
}

#[test]
fn titlebar_exposes_sync_as_a_first_class_action() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_sync_modal_open());
    app.invoke_sync_now_requested();
    assert!(app.get_sync_modal_open());
}

#[test]
fn titlebar_exposes_transfer_icon_with_queue_badge() {
    let content = std::fs::read_to_string("ui/shell/titlebar.slint").unwrap();

    assert!(
        content.contains("transfer-button"),
        "titlebar should expose a dedicated transfer action button"
    );
    assert!(
        content.contains("transfer-badge"),
        "titlebar should surface a queue badge on the transfer action"
    );
}

#[test]
fn clicking_transfer_icon_opens_transfer_center_surface() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_transfer_center_open());

    app.invoke_open_transfer_center_requested();
    assert!(app.get_transfer_center_open());

    app.invoke_open_transfer_center_requested();
    assert!(!app.get_transfer_center_open());
}

#[test]
fn settings_no_longer_routes_into_vault_flow() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_open_settings_panel_requested();

    assert_ne!(app.get_right_panel_view().as_str(), "vault");
}

#[test]
fn bootstrap_syncs_native_window_effects_on_bind_and_theme_toggle() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_path = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("top-status-bar-window-effects.json");
    let _ = fs::remove_file(&temp_path);

    let requests = Rc::new(RefCell::new(Vec::new()));
    let effects = Rc::new(RecordingWindowEffects::new(Rc::clone(&requests)));

    bind_top_status_bar_with_store_and_effects(
        &app,
        Some(UiPreferencesStore::new(temp_path.clone())),
        effects,
    );

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].theme, NativeWindowTheme::Dark);
        assert_eq!(
            requests[0].corner_preference,
            NativeWindowCornerPreference::DoNotRound
        );
        assert!(requests[0].request_redraw);
    }

    app.invoke_toggle_theme_mode_requested();

    {
        let requests = requests.borrow();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[1].theme, NativeWindowTheme::Light);
        assert_eq!(
            requests[1].corner_preference,
            NativeWindowCornerPreference::DoNotRound
        );
        assert!(requests[1].request_redraw);
    }

    let _ = fs::remove_file(temp_path);
}

#[test]
fn bootstrap_applies_default_restored_size_before_run() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    app.window().set_size(PhysicalSize::new(800, 500));

    bind_top_status_bar_with_store(&app, None);

    let size = app.window().size();
    assert_eq!((size.width, size.height), (1440, 900));
}

#[test]
fn maximize_toggle_updates_window_maximized_binding() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert!(!app.get_is_window_maximized());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}

#[test]
fn maximize_toggle_keeps_drag_related_window_state_bindings_consistent() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}

#[test]
fn maximize_toggle_does_not_change_shell_geometry_exports_yet() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);

    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);

    app.invoke_maximize_toggle_requested();
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn bootstrap_logs_backdrop_error_details_when_native_sync_fails() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    let temp_root = std::env::temp_dir()
        .join("mica-term")
        .join("tests")
        .join("theme-sync-backdrop-error-log");
    let _ = fs::remove_dir_all(&temp_root);
    fs::create_dir_all(temp_root.join("logs")).unwrap();
    fs::create_dir_all(temp_root.join("crash")).unwrap();

    let temp_prefs = temp_root.join("ui-preferences.json");
    let paths = LoggingPaths {
        root_source: LoggingRootSource::EnvOverride,
        root_dir: temp_root.clone(),
        logs_dir: temp_root.join("logs"),
        crash_dir: temp_root.join("crash"),
    };
    let config = AppLoggingConfig::new(AppLogMode::Debug);
    let runtime = build_test_logging_runtime(&paths, &config).unwrap();

    tracing::dispatcher::with_default(&runtime.dispatch, || {
        bind_top_status_bar_with_store_and_effects(
            &app,
            Some(UiPreferencesStore::new(temp_prefs.clone())),
            Rc::new(FailingBackdropWindowEffects {
                error_text: "mock backdrop failure",
            }),
        );
    });

    drop(runtime.guard);

    let content = fs::read_to_string(paths.logs_dir.join("system-error.log")).unwrap();
    assert!(content.contains("backdrop_error=mock backdrop failure"));
    assert!(content.contains("failed to apply native window appearance"));
    assert!(!content.contains("native window appearance sync finished"));

    let _ = fs::remove_dir_all(temp_root);
}

#[test]
fn pointer_click_on_panel_toggle_flips_right_panel_request_state_after_sync_button_is_promoted() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    let content_width = app.get_layout_titlebar_content_width();
    let window_controls_width = app.get_layout_titlebar_window_controls_width();
    let utility_zone_x =
        6.0 + content_width - window_controls_width - ShellMetrics::TITLEBAR_UTILITY_WIDTH as f32;
    let position = LogicalPosition::new(
        utility_zone_x + 80.0 + (ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE as f32 / 2.0),
        6.0 + (ShellMetrics::TITLEBAR_TOOL_BUTTON_SIZE as f32 / 2.0),
    );

    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });

    assert!(app.get_show_right_panel());
    assert!(app.get_effective_show_right_panel());
    assert!(app.get_layout_right_panel_width() > 0.0);
}
