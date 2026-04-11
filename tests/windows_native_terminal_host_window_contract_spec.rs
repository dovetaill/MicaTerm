use std::fs;

#[test]
fn vendored_winit_backend_can_force_opaque_host_window_for_native_terminal() {
    let backend_source = fs::read_to_string("vendor/i-slint-backend-winit/winitwindowadapter.rs")
        .expect("read vendored winit window adapter");

    assert!(
        backend_source.contains("MICA_TERM_FORCE_OPAQUE_HOST_WINDOW")
            && backend_source.contains("with_transparent(false)"),
        "vendored Slint winit backend should expose a Windows host-window transparency override so retained-native terminal bring-up can create an opaque host HWND instead of always forcing a transparent shell window"
    );
}

#[test]
fn bootstrap_sets_opaque_host_override_before_creating_retained_native_window() {
    let bootstrap_source =
        fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap source");

    assert!(
        bootstrap_source.contains("MICA_TERM_FORCE_OPAQUE_HOST_WINDOW")
            && bootstrap_source.contains("AppWindow::new()?"),
        "bootstrap should configure the Windows host-window transparency override before constructing the main Slint window so retained-native child HWND presentation is not forced under a transparent host shell"
    );
}
