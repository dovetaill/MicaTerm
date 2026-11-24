//! Bundled fonts for the terminal grid and application UI.

use std::borrow::Cow;

use gpui_kit::App;

pub use crate::domain::{BUNDLED_MAPLE_FONT_FAMILY, BUNDLED_SARASA_FONT_FAMILY};

const SARASA_TERM_SC_NERD_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-Regular.ttf");
const SARASA_TERM_SC_NERD_SEMIBOLD: &[u8] =
    include_bytes!("../../assets/fonts/SarasaTermSCNerd/SarasaTermSCNerd-SemiBold.ttf");
const JETBRAINS_MAPLE_MONO_REGULAR: &[u8] =
    include_bytes!("../../assets/fonts/JetBrainsMapleMono/JetBrainsMapleMono-Regular.ttf");

pub fn register_bundled_fonts(cx: &App) {
    if let Err(err) = cx.text_system().add_fonts(vec![
        Cow::Borrowed(SARASA_TERM_SC_NERD_REGULAR),
        Cow::Borrowed(SARASA_TERM_SC_NERD_SEMIBOLD),
        Cow::Borrowed(JETBRAINS_MAPLE_MONO_REGULAR),
    ]) {
        tracing::warn!(error = %err, "加载内置字体失败");
    }
}
