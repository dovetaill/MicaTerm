use mica_term::app::ssh::shell_integration::{
    BootstrapOptions, MicaPrivateAction, ShellIntegrationEvent, ShellKind, build_shell_bootstrap,
    parse_shell_integration_events, runtime_shell_events,
};

#[test]
fn parser_extracts_standard_and_private_shell_integration_events() {
    let input = concat!(
        "\u{1b}]7;file://remote/tmp/project\u{7}",
        "\u{1b}]133;A\u{7}",
        "\u{1b}]133;B\u{7}",
        "\u{1b}]133;C\u{7}",
        "\u{1b}]133;D;0\u{7}",
        "\u{1b}]1337;CurrentDir=/tmp/project\u{7}",
        "\u{1b}]9001;mterm;open;/tmp/readme.md\u{7}",
    );

    let events = parse_shell_integration_events(input.as_bytes());

    assert!(events.contains(&ShellIntegrationEvent::CurrentDirectory(
        "/tmp/project".into()
    )));
    assert!(events.contains(&ShellIntegrationEvent::PromptStart));
    assert!(events.contains(&ShellIntegrationEvent::PromptEnd));
    assert!(events.contains(&ShellIntegrationEvent::CommandStart));
    assert!(events.contains(&ShellIntegrationEvent::CommandFinished(Some(0))));
    assert!(events.contains(&ShellIntegrationEvent::PrivateAction(
        MicaPrivateAction::OpenPath("/tmp/readme.md".into()),
    )));
}

#[test]
fn bash_bootstrap_builder_prefers_standard_markers_and_gates_private_channel() {
    let script = build_shell_bootstrap(ShellKind::Bash, BootstrapOptions::default());

    assert!(script.contains("\\033]133;A"));
    assert!(script.contains("\\033]7;file://"));
    assert!(script.contains("TERM_PROGRAM=mica-term"));
    assert!(script.contains("MICA_TERM_ENHANCED=1"));
    assert!(!script.contains("OSC 133"));
}

#[test]
fn runtime_extracts_cwd_and_command_marks_from_shell_integration_sequences() {
    let bytes = concat!(
        "\u{1b}]133;A\u{7}",
        "\u{1b}]7;file://remote/tmp/project\u{7}",
        "\u{1b}]133;B\u{7}",
        "\u{1b}]133;C\u{7}",
        "\u{1b}]133;D;0\u{7}",
    )
    .as_bytes();

    let parsed = runtime_shell_events(bytes);

    assert_eq!(parsed.cwd.as_deref(), Some("/tmp/project"));
    assert_eq!(parsed.command_finish_exit_code, Some(0));
}
