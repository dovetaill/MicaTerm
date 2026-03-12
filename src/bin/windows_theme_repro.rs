#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

include!(concat!(env!("OUT_DIR"), "/windows_theme_repro.rs"));

use slint::ComponentHandle;

fn main() -> Result<(), slint::PlatformError> {
    let window = WindowsThemeRepro::new()?;
    let weak = window.as_weak();

    window.on_toggle_theme_requested(move || {
        if let Some(window) = weak.upgrade() {
            let next_mode = !window.get_dark_mode();
            window.set_dark_mode(next_mode);
        }
    });

    window.run()
}
