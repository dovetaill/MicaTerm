use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar;

#[test]
fn bootstrap_binds_top_status_bar_callbacks_to_window_state() {
    i_slint_backend_testing::init_no_event_loop();

    let app = AppWindow::new().unwrap();

    app.set_show_right_panel(true);
    app.set_show_global_menu(true);
    app.set_is_window_maximized(true);
    app.set_is_window_active(false);

    bind_top_status_bar(&app);

    assert!(!app.get_show_right_panel());
    assert!(!app.get_show_global_menu());
    assert!(!app.get_is_window_maximized());
    assert!(app.get_is_window_active());

    app.invoke_toggle_right_panel_requested();
    assert!(app.get_show_right_panel());

    app.invoke_toggle_global_menu_requested();
    assert!(app.get_show_global_menu());

    app.invoke_close_global_menu_requested();
    assert!(!app.get_show_global_menu());

    app.invoke_maximize_toggle_requested();
    assert!(app.get_is_window_maximized());

    app.invoke_drag_double_clicked();
    assert!(!app.get_is_window_maximized());
}
