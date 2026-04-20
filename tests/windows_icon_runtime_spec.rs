use std::fs;

#[test]
fn app_window_declares_embedded_runtime_icon() {
    let app_window_source =
        fs::read_to_string("ui/app-window.slint").expect("read app window source");

    assert!(
        app_window_source.contains("icon: @image-url(\"../assets/icons/mica-term-app.svg\")"),
        "AppWindow should declare an embedded runtime icon so the packaged Windows taskbar button does not depend solely on the exe resource section"
    );
}

#[test]
fn bootstrap_source_installs_windows_icon_diagnostics() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let app_module_source = fs::read_to_string("src/app/mod.rs").expect("read app module");
    let windows_icon_source =
        fs::read_to_string("src/app/windows_icon.rs").expect("read windows icon module");

    assert!(
        app_module_source.contains("pub(crate) mod windows_icon;"),
        "app module should expose the windows_icon helper module so startup can log packaged icon state"
    );
    assert!(
        bootstrap_source
            .contains("windows_icon::log_window_icon_state(&window, \"after_window_new\")")
            && bootstrap_source
                .contains("windows_icon::log_window_icon_state(&window, \"before_window_run\")"),
        "bootstrap should log Windows icon state after creating the window and again before entering the Slint event loop so packaged-icon regressions remain traceable"
    );
    for expected in [
        "target: \"app.windows_icon\"",
        "WM_GETICON",
        "ICON_SMALL",
        "ICON_BIG",
        "GCLP_HICONSM",
        "GCLP_HICON",
    ] {
        assert!(
            windows_icon_source.contains(expected),
            "windows icon diagnostics module should include `{expected}` so small/big/class icon handles can be inspected from startup logs"
        );
    }
}

#[test]
fn vendored_backend_preserves_native_windows_icon_when_slint_icon_is_empty() {
    let backend_source = fs::read_to_string("vendor/i-slint-backend-winit/winitwindowadapter.rs")
        .expect("read vendored winit backend");

    for expected in [
        "fn icon_is_effectively_empty(",
        "let icon_is_effectively_empty = icon_is_effectively_empty(&icon_image);",
        "slint icon is empty on Windows; preserving existing native window/taskbar icons",
        "!icon_is_effectively_empty",
        "*self.window_icon_cache_key.borrow() != icon_image_cache_key",
    ] {
        assert!(
            backend_source.contains(expected),
            "vendored backend should include `{expected}` so an empty Slint icon cannot wipe the packaged Windows icon handles"
        );
    }
}
