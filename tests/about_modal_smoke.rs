use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use i_slint_backend_testing::ElementHandle;
use mica_term::AppWindow;
use mica_term::app::bootstrap::{
    bind_top_status_bar_with_store, install_url_open_handler_for_test,
};
use slint::platform::{Key, PointerEventButton, WindowEvent};
use slint::{ComponentHandle, LogicalPosition};

fn settle() {
    i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(50));
    slint::platform::update_timers_and_animations();
}

fn key(app: &AppWindow, text: slint::SharedString) {
    app.window()
        .dispatch_event(WindowEvent::KeyPressed { text: text.clone() });
    app.window()
        .dispatch_event(WindowEvent::KeyReleased { text });
    settle();
}

fn click(app: &AppWindow, label: &str) {
    settle();
    let button = ElementHandle::find_by_accessible_label(app, label)
        .find(|element| element.size().width > 0.0)
        .unwrap_or_else(|| panic!("missing button: {label}"));
    let origin = button.absolute_position();
    let size = button.size();
    let position = LogicalPosition::new(origin.x + size.width / 2., origin.y + size.height / 2.);
    app.window()
        .dispatch_event(WindowEvent::PointerMoved { position });
    app.window().dispatch_event(WindowEvent::PointerPressed {
        position,
        button: PointerEventButton::Left,
    });
    app.window().dispatch_event(WindowEvent::PointerReleased {
        position,
        button: PointerEventButton::Left,
    });
    settle();
}

fn clipboard() -> String {
    i_slint_backend_selector::with_platform(|platform| {
        Ok(platform
            .clipboard_text(slint::platform::Clipboard::DefaultClipboard)
            .unwrap_or_default())
    })
    .unwrap()
}

#[test]
fn about_displays_build_identity_and_closes_with_escape_and_button() {
    i_slint_backend_testing::init_no_event_loop();
    let app = AppWindow::new().unwrap();
    bind_top_status_bar_with_store(&app, None);
    app.show().unwrap();
    app.window().set_size(slint::LogicalSize::new(1000., 700.));
    app.window()
        .dispatch_event(WindowEvent::WindowActiveChanged(true));
    assert_eq!(app.get_about_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(app.get_about_repository(), env!("CARGO_PKG_REPOSITORY"));
    assert_eq!(app.get_about_revision(), env!("MICA_TERM_REVISION"));
    assert_eq!(app.get_about_build_date(), env!("MICA_TERM_BUILT_AT"));
    assert_eq!(app.get_about_release_date(), env!("MICA_TERM_RELEASE_DATE"));
    app.set_workspace_session_host_mode("terminal".into());
    let terminal_keys = Rc::new(RefCell::new(Vec::new()));
    let received = terminal_keys.clone();
    app.on_workspace_session_key_input(move |text, _, _, _| received.borrow_mut().push(text));
    app.invoke_focus_workspace_primary();
    settle();
    key(&app, Key::UpArrow.into());
    assert_eq!(terminal_keys.borrow().len(), 1);
    terminal_keys.borrow_mut().clear();
    app.invoke_toggle_global_menu_requested();
    app.invoke_open_about_requested();
    assert!(app.get_about_modal_open());
    assert!(!app.get_show_global_menu());
    // Materialize the conditional modal, as rendering would in the desktop backend.
    assert!(
        ElementHandle::find_by_accessible_label(&app, "Close")
            .next()
            .is_some()
    );
    settle();
    key(&app, Key::UpArrow.into());
    assert!(
        terminal_keys.borrow().is_empty(),
        "modal owns keyboard focus"
    );
    key(&app, Key::Escape.into());
    assert!(!app.get_about_modal_open());
    key(&app, Key::UpArrow.into());
    assert_eq!(
        terminal_keys.borrow().len(),
        1,
        "Escape restores terminal focus"
    );
    terminal_keys.borrow_mut().clear();
    app.invoke_open_about_requested();
    click(&app, "Copy details");
    assert_eq!(
        clipboard(),
        format!(
            "MicaTerm\nVersion: {}\nReleased: {}\nBuilt: {}\nCommit: {}\nRepository: {}",
            app.get_about_version(),
            app.get_about_release_date(),
            app.get_about_build_date(),
            app.get_about_revision(),
            app.get_about_repository()
        )
    );
    assert_eq!(app.get_about_feedback(), "Copied to clipboard");
    assert!(app.get_about_modal_open());
    click(&app, "Copy commit");
    assert_eq!(clipboard(), app.get_about_revision().as_str());
    key(&app, Key::UpArrow.into());
    assert!(
        terminal_keys.borrow().is_empty(),
        "copy keeps focus in the modal"
    );
    key(&app, Key::Escape.into());
    assert!(
        !app.get_about_modal_open(),
        "Escape still works after copying"
    );
    app.invoke_open_about_requested();
    assert_eq!(app.get_about_feedback(), "", "reopening clears feedback");
    let opened_urls = Arc::new(Mutex::new(Vec::new()));
    let urls = opened_urls.clone();
    let _url_guard = install_url_open_handler_for_test(move |url| {
        urls.lock().unwrap().push(url.to_string());
        Ok(())
    });
    click(&app, "Open project repository");
    key(&app, Key::Return.into());
    assert_eq!(
        *opened_urls.lock().unwrap(),
        vec![env!("CARGO_PKG_REPOSITORY"); 2]
    );
    click(&app, "Close");
    assert!(!app.get_about_modal_open());
    terminal_keys.borrow_mut().clear();
    key(&app, Key::UpArrow.into());
    assert_eq!(
        terminal_keys.borrow().len(),
        1,
        "Close restores terminal focus"
    );
    app.invoke_open_about_requested();
    click(&app, "Close dialog");
    assert!(!app.get_about_modal_open());
    terminal_keys.borrow_mut().clear();
    key(&app, Key::UpArrow.into());
    assert_eq!(
        terminal_keys.borrow().len(),
        1,
        "header close restores terminal focus"
    );
    app.hide().unwrap();
}

#[cfg(feature = "slint-renderer-software")]
#[test]
#[ignore = "writes visual review snapshots under target/about-snapshots"]
fn about_render_snapshots() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(render_snapshots)
        .unwrap()
        .join()
        .unwrap();
}

#[cfg(feature = "slint-renderer-software")]
fn render_snapshots() {
    use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};

    struct SnapshotPlatform(Rc<MinimalSoftwareWindow>);
    impl slint::platform::Platform for SnapshotPlatform {
        fn create_window_adapter(
            &self,
        ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
            Ok(self.0.clone())
        }
    }
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(SnapshotPlatform(window))).unwrap();
    let app = AppWindow::new().unwrap();
    app.set_about_version("0.1.0".into());
    app.set_about_release_date("Unreleased".into());
    app.set_about_build_date("2026-09-18 12:00:00 UTC".into());
    app.set_about_revision("8d57cf482ffa (modified)".into());
    app.set_about_repository(env!("CARGO_PKG_REPOSITORY").into());
    app.set_about_modal_open(true);
    app.show().unwrap();
    std::fs::create_dir_all("target/about-snapshots").unwrap();
    for (name, width, height, scale, dark) in [
        ("dark", 1000., 700., 1., true),
        ("light", 1000., 700., 1., false),
        ("compact", 440., 400., 1., true),
        ("hidpi", 1000., 700., 2., true),
    ] {
        app.set_dark_mode(dark);
        app.window()
            .dispatch_event(WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
            });
        app.window()
            .set_size(slint::LogicalSize::new(width, height));
        let pixels = app.window().take_snapshot().unwrap();
        assert!(
            pixels
                .as_bytes()
                .chunks_exact(4)
                .any(|pixel| pixel != &pixels.as_bytes()[..4])
        );
        image::save_buffer(
            format!("target/about-snapshots/{name}.png"),
            pixels.as_bytes(),
            pixels.width(),
            pixels.height(),
            image::ColorType::Rgba8,
        )
        .unwrap();
    }
    app.hide().unwrap();
}
