use anyhow::Result;
use slint::ComponentHandle;

use crate::AppWindow;

pub fn app_title() -> &'static str {
    "Mica Term"
}

pub fn default_window_size() -> (u32, u32) {
    (1440, 900)
}

pub fn run() -> Result<()> {
    let window = AppWindow::new()?;
    window.run()?;
    Ok(())
}
