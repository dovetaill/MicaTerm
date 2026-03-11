#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeAction {
    NewConnection,
    OpenRecent,
    Snippets,
    Sftp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShellViewModel {
    pub show_welcome: bool,
    pub show_right_panel: bool,
    pub show_settings_menu: bool,
    pub is_window_maximized: bool,
    pub is_window_active: bool,
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
            show_settings_menu: false,
            is_window_maximized: false,
            is_window_active: true,
        }
    }
}

impl ShellViewModel {
    pub fn toggle_right_panel(&mut self) {
        self.show_right_panel = !self.show_right_panel;
    }

    pub fn toggle_settings_menu(&mut self) {
        self.show_settings_menu = !self.show_settings_menu;
    }

    pub fn close_settings_menu(&mut self) {
        self.show_settings_menu = false;
    }

    pub fn set_window_maximized(&mut self, value: bool) {
        self.is_window_maximized = value;
    }

    pub fn set_window_active(&mut self, value: bool) {
        self.is_window_active = value;
    }
}

pub fn welcome_actions() -> &'static [WelcomeAction] {
    &[
        WelcomeAction::NewConnection,
        WelcomeAction::OpenRecent,
        WelcomeAction::Snippets,
        WelcomeAction::Sftp,
    ]
}
