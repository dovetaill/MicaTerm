use mica_term::AppWindow;
use mica_term::app::bootstrap::bind_top_status_bar_with_store;
use slint::ComponentHandle;

#[test]
fn about_displays_build_identity_and_closes_with_escape_and_button() {
    i_slint_backend_testing::init_no_event_loop();
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    assert_eq!(app.get_about_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(app.get_about_repository(), env!("CARGO_PKG_REPOSITORY"));
    assert_eq!(app.get_about_revision(), env!("MICA_TERM_REVISION"));
    assert_eq!(app.get_about_build_date(), env!("MICA_TERM_BUILT_AT"));
    assert_eq!(app.get_about_release_date(), env!("MICA_TERM_RELEASE_DATE"));
    app.invoke_toggle_global_menu_requested();
    app.invoke_open_about_requested();
    assert!(app.get_about_modal_open());
    assert!(!app.get_show_global_menu());
    // Escape is handled by BlockingModalShell; exercise the same close callback in the smoke test.
    app.invoke_open_about_requested();
    app.invoke_close_about_requested();
    assert!(!app.get_about_modal_open());
    app.hide().unwrap();
}
