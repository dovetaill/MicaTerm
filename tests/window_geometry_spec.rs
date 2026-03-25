//! Window geometry coverage for titlebar and shell body layout exports.

use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use mica_term::shell::metrics::ShellMetrics;
use slint::{ComponentHandle, PhysicalSize};

#[test]
fn shell_body_height_matches_window_height_minus_titlebar() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    let body_height = app.get_layout_shell_body_height() as u32;
    let titlebar_height = app.get_layout_titlebar_height() as u32;
    assert_eq!(titlebar_height, ShellMetrics::TITLEBAR_HEIGHT);
    assert_eq!(
        body_height,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT - ShellMetrics::TITLEBAR_HEIGHT
    );
    assert_eq!(
        app.get_layout_shell_body_actual_height() as u32,
        ShellMetrics::WINDOW_DEFAULT_HEIGHT - ShellMetrics::TITLEBAR_HEIGHT
    );
}

#[test]
fn titlebar_spans_window_width_for_button_layout() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(
        app.get_layout_titlebar_width() as u32,
        ShellMetrics::WINDOW_DEFAULT_WIDTH
    );
}

#[test]
fn titlebar_content_zones_receive_layout_width() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(
        app.get_layout_titlebar_content_width() as u32,
        ShellMetrics::WINDOW_DEFAULT_WIDTH - 12
    );
    assert_eq!(
        app.get_layout_titlebar_nav_zone_width() as u32,
        ShellMetrics::TITLEBAR_NAV_WIDTH
    );
    assert_eq!(
        app.get_layout_titlebar_window_controls_width() as u32,
        ShellMetrics::TITLEBAR_WINDOW_CONTROL_WIDTH
    );
}

#[test]
fn larger_window_expands_shell_body_instead_of_leaving_blank_space() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.window().set_size(PhysicalSize::new(1600, 1000));
    app.show().unwrap();
    app.invoke_shell_layout_invalidated(1600.0, 1000.0);

    assert_eq!(
        app.get_layout_shell_body_height() as u32,
        1000 - ShellMetrics::TITLEBAR_HEIGHT
    );
    assert_eq!(
        app.get_layout_shell_body_actual_height() as u32,
        1000 - ShellMetrics::TITLEBAR_HEIGHT
    );
}

#[test]
fn collapse_order_matches_design_under_narrow_widths() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();

    app.window().set_size(PhysicalSize::new(1335, 900));
    app.show().unwrap();
    app.invoke_shell_layout_invalidated(1335.0, 900.0);
    assert_eq!(app.get_layout_assets_sidebar_width() as u32, 0);
    assert_eq!(
        app.get_layout_right_panel_width() as u32,
        ShellMetrics::RIGHT_PANEL_WIDTH
    );

    app.window().set_size(PhysicalSize::new(1079, 900));
    app.invoke_shell_layout_invalidated(1079.0, 900.0);
    assert_eq!(app.get_layout_right_panel_width() as u32, 0);
}

#[test]
fn restored_window_uses_square_shell_frame() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn maximize_toggle_does_not_change_shell_frame_radius() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);

    app.invoke_maximize_toggle_requested();
    assert_eq!(app.get_layout_shell_frame_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_radius() as u32, 0);
    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
}

#[test]
fn shell_exports_internal_chrome_geometry_contracts() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_titlebar_border_width() as u32, 0);
    assert_eq!(app.get_layout_right_panel_radius() as u32, 0);
    assert_eq!(app.get_layout_right_panel_border_width() as u32, 0);
}

#[test]
fn expanded_right_panel_is_flat_and_owns_no_full_card_border() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(app.get_layout_right_panel_radius() as u32, 0);
    assert_eq!(app.get_layout_right_panel_border_width() as u32, 0);
}

#[test]
fn expanded_right_panel_fills_shell_body_height() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    assert_eq!(
        app.get_layout_right_panel_height() as u32,
        app.get_layout_shell_body_actual_height() as u32
    );
}

#[test]
fn expanded_right_panel_stays_inside_shell_body_and_shrinks_workspace() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.invoke_toggle_right_panel_requested();
    app.show().unwrap();

    let shell_body_width = app.get_layout_shell_body_width() as u32;
    let sidebar_width =
        (app.get_layout_activity_bar_width() + app.get_layout_assets_sidebar_width()) as u32;
    let main_workspace_x = app.get_layout_main_workspace_x() as u32;
    let main_workspace_width = app.get_layout_main_workspace_width() as u32;
    let right_panel_x = app.get_layout_right_panel_x() as u32;
    let right_panel_width = app.get_layout_right_panel_width() as u32;

    assert_eq!(main_workspace_x, sidebar_width);
    assert_eq!(main_workspace_x + main_workspace_width, right_panel_x);
    assert_eq!(right_panel_x + right_panel_width, shell_body_width);
}

#[test]
fn maximize_button_geometry_is_exported_for_native_frame_adapter() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_titlebar_maximize_button_width() as u32, 36);
    assert_eq!(app.get_layout_titlebar_maximize_button_height() as u32, 36);
    assert!(app.get_layout_titlebar_maximize_button_x() > 0.0);
}

#[test]
fn frameless_window_exports_resize_border_budget() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();

    assert_eq!(app.get_layout_resize_border_width() as u32, 6);
}
