use mica_term::app::ssh::shell_integration::{
    BootstrapOptions, MicaPrivateAction, ShellIntegrationEvent, ShellKind, build_shell_bootstrap,
    parse_shell_integration_events, runtime_shell_events,
};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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
fn bash_bootstrap_registers_real_prompt_and_command_hooks() {
    let script = build_shell_bootstrap(ShellKind::Bash, BootstrapOptions::default());

    assert!(
        script.contains("PROMPT_COMMAND"),
        "bash bootstrap should attach to the prompt lifecycle instead of only defining emit helpers"
    );
    assert!(
        script.contains("PS0=") || script.contains("trap '__mica_term"),
        "bash bootstrap should hook command execution via PS0 or a guarded DEBUG trap"
    );
}

#[test]
fn zsh_bootstrap_registers_precmd_and_preexec_hooks() {
    let script = build_shell_bootstrap(ShellKind::Zsh, BootstrapOptions::default());

    assert!(
        script.contains("add-zsh-hook precmd") || script.contains("precmd_functions+="),
        "zsh bootstrap should register a precmd hook to emit prompt/cwd markers"
    );
    assert!(
        script.contains("add-zsh-hook preexec") || script.contains("preexec_functions+="),
        "zsh bootstrap should register a preexec hook to emit command-start markers"
    );
}

#[test]
fn fish_bootstrap_registers_prompt_preexec_and_postexec_hooks() {
    let script = build_shell_bootstrap(ShellKind::Fish, BootstrapOptions::default());

    assert!(
        script.contains("--on-event fish_prompt"),
        "fish bootstrap should attach prompt markers to the fish_prompt event"
    );
    assert!(
        script.contains("--on-event fish_preexec"),
        "fish bootstrap should attach command-start markers to the fish_preexec event"
    );
    assert!(
        script.contains("--on-event fish_postexec"),
        "fish bootstrap should attach command-finished markers to the fish_postexec event"
    );
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
    assert!(parsed.prompt_started);
    assert!(parsed.prompt_ended);
    assert!(parsed.command_started);
    assert!(parsed.command_finished);
    assert_eq!(parsed.command_finish_exit_code, Some(0));
}

#[test]
fn bash_bootstrap_live_session_emits_prompt_input_and_output_regions() {
    let script = build_shell_bootstrap(ShellKind::Bash, BootstrapOptions::default());
    let script_path = write_temp_shell_script("bash-shell-integration", &script);
    let session = format!(
        "bash --noprofile --rcfile '{}' -i <<'EOF'\n\
printf '__READY__\\n'\n\
cd /tmp\n\
pwd\n\
printf '> quote-like output\\n'\n\
false\n\
exit 0\n\
EOF",
        script_path.display()
    );

    let output = Command::new("script")
        .args(["-qefc", &session, "/dev/null"])
        .env("TERM", "xterm-256color")
        .env("HOSTNAME", "mica-live-test")
        .output()
        .expect("run bash live shell integration session");

    let _ = fs::remove_file(&script_path);

    assert!(
        output.status.success(),
        "bash live shell integration session should exit successfully"
    );

    let events = parse_shell_integration_events(&output.stdout);
    let filtered = events
        .iter()
        .filter_map(|event| match event {
            ShellIntegrationEvent::PromptStart => Some("A"),
            ShellIntegrationEvent::PromptEnd => Some("B"),
            ShellIntegrationEvent::CommandStart => Some("C"),
            ShellIntegrationEvent::CommandFinished(Some(0)) => Some("D0"),
            ShellIntegrationEvent::CommandFinished(Some(1)) => Some("D1"),
            ShellIntegrationEvent::CommandFinished(_) => Some("D"),
            ShellIntegrationEvent::CurrentDirectory(_)
            | ShellIntegrationEvent::PrivateAction(_) => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(
        filtered.first().copied(),
        Some("A"),
        "the first prompt should start with OSC 133;A instead of a synthetic command-finished marker"
    );
    assert!(
        starts_with_subsequence(
            &filtered,
            &[
                "A", "B", "C", "D0", "A", "B", "C", "D0", "A", "B", "C", "D0", "A", "B", "C", "D0",
                "A", "B", "C", "D1",
            ]
        ),
        "live bash session should keep prompt/input/output boundaries in A/B/C/D order; got {filtered:?}"
    );
    assert!(
        events.iter().any(|event| {
            matches!(
                event,
                ShellIntegrationEvent::CurrentDirectory(path) if path == "/tmp"
            )
        }),
        "live bash session should emit OSC 7 after cd so cwd tracking follows the real shell"
    );

    let parsed = runtime_shell_events(&output.stdout);
    let sanitized = String::from_utf8_lossy(&parsed.sanitized_bytes);
    assert!(
        sanitized.contains("__READY__"),
        "shell output should remain readable after stripping shell integration markers"
    );
    assert!(
        sanitized.contains("/tmp"),
        "shell output should keep normal command output alongside shell integration markers"
    );
    assert!(
        sanitized.contains("> quote-like output"),
        "prompt-like prose emitted by a real shell command should stay ordinary output after marker sanitization"
    );
    assert!(
        !sanitized.contains("]133;") && !sanitized.contains("]7;file://"),
        "shell integration markers should be fully stripped before visible terminal output is applied"
    );
}

fn starts_with_subsequence(haystack: &[&str], expected: &[&str]) -> bool {
    haystack.starts_with(expected)
}

fn write_temp_shell_script(prefix: &str, script: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("current time")
        .as_nanos();
    path.push(format!(
        "mica-term-{prefix}-{}-{unique}.sh",
        std::process::id()
    ));
    fs::write(&path, script).expect("write temporary shell integration script");
    path
}
