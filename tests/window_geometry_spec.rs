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
