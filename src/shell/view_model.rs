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
}

impl Default for ShellViewModel {
    fn default() -> Self {
        Self {
            show_welcome: true,
            show_right_panel: false,
        }
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
