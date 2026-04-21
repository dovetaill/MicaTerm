use std::fs;

#[test]
fn startup_window_bounds_use_slint_position_before_event_loop() {
    let bootstrap = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let normalized_bootstrap = bootstrap.split_whitespace().collect::<String>();
    let slint_backend =
        fs::read_to_string("vendor/i-slint-backend-winit/lib.rs").expect("read slint winit docs");
    let slint_adapter = fs::read_to_string("vendor/i-slint-backend-winit/winitwindowadapter.rs")
        .expect("read slint winit adapter");

    assert!(
        slint_backend
            .contains("with_winit_window()`] will only succeed when the event loop is active"),
        "the vendored Slint docs should keep documenting that direct winit window access is unavailable before the event loop starts"
    );
    assert!(
        slint_adapter.contains("fn set_position(&self, position: corelib::api::WindowPosition)"),
        "the vendored Slint winit adapter should expose the buffered window position hook that works before the native window is active"
    );
    assert!(
        normalized_bootstrap.contains(
            "window.window().set_position(slint::WindowPosition::Physical("
        ),
        "startup placement should go through Slint's window position API so the requested bounds survive pre-run window creation"
    );
    assert!(
        !normalized_bootstrap.contains("winit_window.set_outer_position"),
        "startup placement should not depend on direct winit outer-position calls before the event loop is active"
    );
}
