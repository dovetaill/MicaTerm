pub const BOOTSTRAP_ACK_ACCEPTED: &str = "__MICA_TERM_BOOTSTRAP_OK__";
pub const BOOTSTRAP_ACK_REJECTED: &str = "__MICA_TERM_BOOTSTRAP_REJECT__";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    Zsh,
    Fish,
    Unsupported(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MicaPrivateAction {
    OpenPath(String),
    EditPath(String),
    DownloadPath(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellIntegrationEvent {
    CurrentDirectory(String),
    PromptStart,
    PromptEnd,
    CommandStart,
    CommandFinished(Option<i32>),
    PrivateAction(MicaPrivateAction),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapOptions {
    pub term_program: String,
    pub enhanced_flag: String,
    pub private_channel_tag: String,
    pub private_actions_enabled: bool,
}

impl Default for BootstrapOptions {
    fn default() -> Self {
        Self {
            term_program: "mica-term".into(),
            enhanced_flag: "MICA_TERM_ENHANCED".into(),
            private_channel_tag: "mterm".into(),
            private_actions_enabled: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeShellEvents {
    pub cwd: Option<String>,
    pub prompt_started: bool,
    pub prompt_ended: bool,
    pub command_started: bool,
    pub command_finish_exit_code: Option<i32>,
    pub sanitized_bytes: Vec<u8>,
}

pub fn shell_probe_command() -> &'static str {
    r#"printf '%s\n' "${SHELL:-}" "#
}

pub fn parse_detected_shell(bytes: &[u8]) -> ShellKind {
    let output = String::from_utf8_lossy(bytes);
    let shell_name = output.trim().rsplit('/').next().unwrap_or_default().trim();

    match shell_name {
        "bash" => ShellKind::Bash,
        "zsh" => ShellKind::Zsh,
        "fish" => ShellKind::Fish,
        _ if shell_name.is_empty() => ShellKind::Unsupported("unknown".into()),
        _ => ShellKind::Unsupported(shell_name.to_string()),
    }
}

pub fn shell_supports_integration(shell: &ShellKind) -> bool {
    matches!(shell, ShellKind::Bash | ShellKind::Zsh | ShellKind::Fish)
}

pub fn parse_shell_integration_events(bytes: &[u8]) -> Vec<ShellIntegrationEvent> {
    let mut events = Vec::new();
    let mut cursor = 0;

    while cursor + 1 < bytes.len() {
        if bytes[cursor] != 0x1b || bytes[cursor + 1] != b']' {
            cursor += 1;
            continue;
        }

        let payload_start = cursor + 2;
        let mut payload_end = payload_start;
        let mut terminator_len = 0;

        while payload_end < bytes.len() {
            match bytes[payload_end] {
                0x07 => {
                    terminator_len = 1;
                    break;
                }
                0x1b if payload_end + 1 < bytes.len() && bytes[payload_end + 1] == b'\\' => {
                    terminator_len = 2;
                    break;
                }
                _ => payload_end += 1,
            }
        }

        if terminator_len == 0 {
            break;
        }

        let payload = String::from_utf8_lossy(&bytes[payload_start..payload_end]);
        if let Some(event) = parse_osc_payload(payload.as_ref()) {
            events.push(event);
        }
        cursor = payload_end + terminator_len;
    }

    events
}

pub fn runtime_shell_events(bytes: &[u8]) -> RuntimeShellEvents {
    let mut parsed = RuntimeShellEvents {
        sanitized_bytes: Vec::with_capacity(bytes.len()),
        ..RuntimeShellEvents::default()
    };
    let mut cursor = 0;

    while cursor < bytes.len() {
        if cursor + 1 >= bytes.len() || bytes[cursor] != 0x1b || bytes[cursor + 1] != b']' {
            parsed.sanitized_bytes.push(bytes[cursor]);
            cursor += 1;
            continue;
        }

        let payload_start = cursor + 2;
        let mut payload_end = payload_start;
        let mut terminator_len = 0;

        while payload_end < bytes.len() {
            match bytes[payload_end] {
                0x07 => {
                    terminator_len = 1;
                    break;
                }
                0x1b if payload_end + 1 < bytes.len() && bytes[payload_end + 1] == b'\\' => {
                    terminator_len = 2;
                    break;
                }
                _ => payload_end += 1,
            }
        }

        if terminator_len == 0 {
            parsed.sanitized_bytes.extend_from_slice(&bytes[cursor..]);
            break;
        }

        let payload = String::from_utf8_lossy(&bytes[payload_start..payload_end]);
        if let Some(event) = parse_osc_payload(payload.as_ref()) {
            apply_runtime_event(&mut parsed, &event);
            cursor = payload_end + terminator_len;
            continue;
        }

        parsed
            .sanitized_bytes
            .extend_from_slice(&bytes[cursor..payload_end + terminator_len]);
        cursor = payload_end + terminator_len;
    }

    parsed
}

pub fn build_shell_bootstrap(shell: ShellKind, options: BootstrapOptions) -> String {
    match shell {
        ShellKind::Bash => build_bash_bootstrap(&options),
        ShellKind::Zsh => build_zsh_bootstrap(&options),
        ShellKind::Fish => build_fish_bootstrap(&options),
        ShellKind::Unsupported(_) => String::new(),
    }
}

fn parse_osc_payload(payload: &str) -> Option<ShellIntegrationEvent> {
    if let Some(cwd) = payload
        .strip_prefix("7;")
        .and_then(parse_osc7_current_directory)
    {
        return Some(ShellIntegrationEvent::CurrentDirectory(cwd));
    }

    if let Some(event) = payload.strip_prefix("133;").and_then(parse_osc133_event) {
        return Some(event);
    }

    if let Some(cwd) = payload.strip_prefix("1337;CurrentDir=") {
        return Some(ShellIntegrationEvent::CurrentDirectory(cwd.to_string()));
    }

    payload
        .strip_prefix("9001;")
        .and_then(parse_private_action)
        .map(ShellIntegrationEvent::PrivateAction)
}

fn parse_osc7_current_directory(payload: &str) -> Option<String> {
    let location = payload.strip_prefix("file://")?;
    let slash_index = location.find('/')?;
    let path = &location[slash_index..];
    if path.is_empty() {
        return None;
    }
    Some(path.to_string())
}

fn parse_osc133_event(payload: &str) -> Option<ShellIntegrationEvent> {
    match payload {
        "A" => Some(ShellIntegrationEvent::PromptStart),
        "B" => Some(ShellIntegrationEvent::PromptEnd),
        "C" => Some(ShellIntegrationEvent::CommandStart),
        "D" => Some(ShellIntegrationEvent::CommandFinished(None)),
        _ => payload
            .strip_prefix("D;")
            .and_then(|value| value.parse::<i32>().ok())
            .map(|code| ShellIntegrationEvent::CommandFinished(Some(code))),
    }
}

fn parse_private_action(payload: &str) -> Option<MicaPrivateAction> {
    let mut segments = payload.splitn(3, ';');
    let channel = segments.next()?;
    let action = segments.next()?;
    let target = segments.next()?;

    if channel != "mterm" || target.is_empty() {
        return None;
    }

    match action {
        "open" => Some(MicaPrivateAction::OpenPath(target.to_string())),
        "edit" => Some(MicaPrivateAction::EditPath(target.to_string())),
        "download" => Some(MicaPrivateAction::DownloadPath(target.to_string())),
        _ => None,
    }
}

fn build_bash_bootstrap(options: &BootstrapOptions) -> String {
    let private_helper = build_private_helper(options);
    format!(
        r#"export TERM_PROGRAM={term_program}
export {enhanced_flag}=1
__mica_term_emit_prompt_start() {{ printf '\033]133;A\007'; }}
__mica_term_emit_prompt_end() {{ printf '\033]133;B\007'; }}
__mica_term_emit_command_start() {{ printf '\033]133;C\007'; }}
__mica_term_emit_command_finished() {{ printf '\033]133;D;%s\007' "${{1:-0}}"; }}
__mica_term_emit_cwd() {{ printf '\033]7;file://%s%s\007' "${{HOSTNAME:-remote}}" "$PWD"; }}
{private_helper}"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_zsh_bootstrap(options: &BootstrapOptions) -> String {
    let private_helper = build_private_helper(options);
    format!(
        r#"export TERM_PROGRAM={term_program}
export {enhanced_flag}=1
function __mica_term_emit_prompt_start() {{ printf '\033]133;A\007'; }}
function __mica_term_emit_prompt_end() {{ printf '\033]133;B\007'; }}
function __mica_term_emit_command_start() {{ printf '\033]133;C\007'; }}
function __mica_term_emit_command_finished() {{ printf '\033]133;D;%s\007' "${{1:-0}}"; }}
function __mica_term_emit_cwd() {{ printf '\033]7;file://%s%s\007' "${{HOST:-remote}}" "$PWD"; }}
{private_helper}"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_fish_bootstrap(options: &BootstrapOptions) -> String {
    let private_helper = build_private_helper(options);
    format!(
        r#"set -gx TERM_PROGRAM {term_program}
set -gx {enhanced_flag} 1
function __mica_term_emit_prompt_start
    printf '\033]133;A\007'
end
function __mica_term_emit_prompt_end
    printf '\033]133;B\007'
end
function __mica_term_emit_command_start
    printf '\033]133;C\007'
end
function __mica_term_emit_command_finished
    printf '\033]133;D;%s\007' "$argv[1]"
end
function __mica_term_emit_cwd
    printf '\033]7;file://%s%s\007' (hostname) "$PWD"
end
{private_helper}"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_private_helper(options: &BootstrapOptions) -> String {
    if !options.private_actions_enabled {
        return String::new();
    }

    format!(
        r#"__mica_term_private_action() {{ printf '\033]9001;{channel};%s;%s\007' "$1" "$2"; }}"#,
        channel = options.private_channel_tag,
    )
}

fn apply_runtime_event(parsed: &mut RuntimeShellEvents, event: &ShellIntegrationEvent) {
    match event {
        ShellIntegrationEvent::CurrentDirectory(path) => {
            parsed.cwd = Some(path.clone());
        }
        ShellIntegrationEvent::PromptStart => {
            parsed.prompt_started = true;
        }
        ShellIntegrationEvent::PromptEnd => {
            parsed.prompt_ended = true;
        }
        ShellIntegrationEvent::CommandStart => {
            parsed.command_started = true;
        }
        ShellIntegrationEvent::CommandFinished(code) => {
            parsed.command_finish_exit_code = *code;
        }
        ShellIntegrationEvent::PrivateAction(_) => {}
    }
}
