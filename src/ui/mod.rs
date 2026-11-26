pub mod brand;
pub mod fonts;
pub mod modals;
pub mod sftp_panel;
pub mod sidebar;
pub mod terminal_view;
pub mod theme;
pub mod theme_catalog;
pub mod welcome_view;
pub mod workspace;

pub use fonts::{BUNDLED_MAPLE_FONT_FAMILY, BUNDLED_SARASA_FONT_FAMILY, register_bundled_fonts};
pub use workspace::MicaWorkspace;

pub fn register_keybindings(cx: &mut gpui_kit::App) {
    cx.bind_keys(
        ["tab", "shift-tab", "ctrl-c", "cmd-c", "ctrl-shift-i", "cmd-alt-i"]
            .map(|key| gpui_kit::KeyBinding::new(key, gpui_kit::NoAction, Some("TerminalPane"))),
    );
    cx.bind_keys(
        ["escape", "enter", "space", "tab", "shift-tab", "backspace", "delete", "ctrl-c",
         "cmd-c", "ctrl-shift-i", "cmd-alt-i"]
            .map(|key| gpui_kit::KeyBinding::new(key, gpui_kit::NoAction, Some("ShortcutRecorder"))),
    );
}
