use mica_term::shell::view_model::{ShellViewModel, WelcomeAction, welcome_actions};

#[test]
fn welcome_actions_match_the_approved_order() {
    assert_eq!(
        welcome_actions(),
        &[
            WelcomeAction::NewConnection,
            WelcomeAction::OpenRecent,
            WelcomeAction::Snippets,
            WelcomeAction::Sftp,
        ]
    );
}

#[test]
fn shell_view_model_starts_in_welcome_mode_with_right_panel_hidden() {
    let view_model = ShellViewModel::default();
    assert!(view_model.show_welcome);
    assert!(!view_model.show_right_panel);
}
