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
    pub command_finished: bool,
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
    let private_helper = build_posix_private_helper(options);
    format!(
        r#"export TERM_PROGRAM={term_program}
export {enhanced_flag}=1
__mica_term_emit_prompt_start() {{ printf '\033]133;A\007'; }}
__mica_term_emit_prompt_end() {{ printf '\033]133;B\007'; }}
__mica_term_emit_command_start() {{ printf '\033]133;C\007'; }}
__mica_term_emit_command_finished() {{ printf '\033]133;D;%s\007' "${{1:-0}}"; }}
__mica_term_emit_cwd() {{ printf '\033]7;file://%s%s\007' "${{HOSTNAME:-remote}}" "$PWD"; }}
{private_helper}
if [[ $- == *i* ]]; then
__mica_term_bash_command_running=0
__mica_term_bash_prompt_end_marker=$'\[\033]133;B\007\]'
__mica_term_bash_emit_command_start=1
__mica_term_bash_prompt_command_is_array() {{
    case "$(declare -p PROMPT_COMMAND 2>/dev/null)" in
        declare\ -a*) return 0 ;;
        *) return 1 ;;
    esac
}}
__mica_term_wrap_bash_prompts() {{
    case "$PS1" in
        *$'\033]133;B\007'*) ;;
        *) PS1="${{PS1}}${{__mica_term_bash_prompt_end_marker}}" ;;
    esac
    case "$PS2" in
        ''|*$'\033]133;B\007'*) ;;
        *) PS2="${{PS2}}${{__mica_term_bash_prompt_end_marker}}" ;;
    esac
}}
__mica_term_prompt_command() {{
    local status="$?"
    if [[ "${{__mica_term_bash_command_running:-0}}" == "1" ]]; then
        __mica_term_emit_command_finished "$status"
        __mica_term_bash_command_running=0
    fi
    __mica_term_emit_prompt_start
    __mica_term_emit_cwd
    __mica_term_bash_interactive_ready=1
}}
__mica_term_before_command() {{
    __mica_term_bash_interactive_ready=0
    __mica_term_bash_command_running=1
    if [[ "${{__mica_term_bash_emit_command_start:-1}}" == "1" ]]; then
        __mica_term_emit_command_start
    fi
}}
__mica_term_prompt_command_installed() {{
    if __mica_term_bash_prompt_command_is_array; then
        local command
        for command in "${{PROMPT_COMMAND[@]}}"; do
            [[ "$command" == "__mica_term_prompt_command" ]] && return 0
        done
        return 1
    fi
    case "${{PROMPT_COMMAND:-}}" in
        *"__mica_term_prompt_command"*) return 0 ;;
        *) return 1 ;;
    esac
}}
__mica_term_install_prompt_command() {{
    if __mica_term_prompt_command_installed; then
        return
    fi
    if __mica_term_bash_prompt_command_is_array; then
        local existing_commands=("${{PROMPT_COMMAND[@]}}")
        local command
        PROMPT_COMMAND=(__mica_term_prompt_command)
        for command in "${{existing_commands[@]}}"; do
            [[ "$command" == "__mica_term_prompt_command" ]] && continue
            PROMPT_COMMAND+=("$command")
        done
        return
    fi
    if [[ -n "${{PROMPT_COMMAND:-}}" ]]; then
        PROMPT_COMMAND=$'__mica_term_prompt_command\n'"${{PROMPT_COMMAND}}"
    else
        PROMPT_COMMAND="__mica_term_prompt_command"
    fi
}}
__mica_term_bash_prompt_command_contains() {{
    if __mica_term_bash_prompt_command_is_array; then
        local command
        for command in "${{PROMPT_COMMAND[@]}}"; do
            [[ "$command" == "${{BASH_COMMAND:-}}" ]] && return 0
        done
        return 1
    fi
    case "${{PROMPT_COMMAND:-}}" in
        *"${{BASH_COMMAND:-}}"*) return 0 ;;
        *) return 1 ;;
    esac
}}
__mica_term_debug_trap() {{
    [[ -n "${{COMP_LINE:-}}" ]] && return
    [[ "${{__mica_term_bash_interactive_ready:-0}}" != "1" ]] && return
    case "${{BASH_COMMAND:-}}" in
        __mica_term_*|history*|builtin\ history*) return ;;
    esac
    if __mica_term_bash_prompt_command_contains; then
        return
    fi
    __mica_term_before_command
}}
if (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )); then
    PS0=$'\033]133;C\007'
    __mica_term_bash_emit_command_start=0
fi
trap '__mica_term_debug_trap' DEBUG
__mica_term_wrap_bash_prompts
__mica_term_install_prompt_command
fi"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_zsh_bootstrap(options: &BootstrapOptions) -> String {
    let private_helper = build_posix_private_helper(options);
    format!(
        r#"export TERM_PROGRAM={term_program}
export {enhanced_flag}=1
function __mica_term_emit_prompt_start() {{ printf '\033]133;A\007'; }}
function __mica_term_emit_prompt_end() {{ printf '\033]133;B\007'; }}
function __mica_term_emit_command_start() {{ printf '\033]133;C\007'; }}
function __mica_term_emit_command_finished() {{ printf '\033]133;D;%s\007' "${{1:-0}}"; }}
function __mica_term_emit_cwd() {{ printf '\033]7;file://%s%s\007' "${{HOST:-remote}}" "$PWD"; }}
{private_helper}
if [[ -o interactive ]]; then
typeset -gi __mica_term_zsh_command_running=0
__mica_term_zsh_prompt_end_marker=$'%{{\e]133;B\a%}}'
__mica_term_wrap_zsh_prompts() {{
    if [[ -n "${{RPROMPT:-}}" ]]; then
        case "$RPROMPT" in
            *$'\e]133;B\a'*) ;;
            *) RPROMPT="${{RPROMPT}}${{__mica_term_zsh_prompt_end_marker}}" ;;
        esac
    elif [[ -n "${{RPS1:-}}" ]]; then
        case "$RPS1" in
            *$'\e]133;B\a'*) ;;
            *) RPS1="${{RPS1}}${{__mica_term_zsh_prompt_end_marker}}" ;;
        esac
    else
        case "$PROMPT" in
            *$'\e]133;B\a'*) ;;
            *) PROMPT="${{PROMPT}}${{__mica_term_zsh_prompt_end_marker}}" ;;
        esac
    fi
    case "${{PROMPT2:-}}" in
        ''|*$'\e]133;B\a'*) ;;
        *) PROMPT2="${{PROMPT2}}${{__mica_term_zsh_prompt_end_marker}}" ;;
    esac
}}
function __mica_term_zsh_precmd() {{
    local status="$?"
    if (( __mica_term_zsh_command_running )); then
        __mica_term_emit_command_finished "$status"
        __mica_term_zsh_command_running=0
    fi
    __mica_term_emit_prompt_start
    __mica_term_emit_cwd
}}
function __mica_term_zsh_preexec() {{
    __mica_term_zsh_command_running=1
    __mica_term_emit_command_start
}}
__mica_term_wrap_zsh_prompts
autoload -Uz add-zsh-hook 2>/dev/null || true
if typeset -f add-zsh-hook >/dev/null 2>&1; then
    add-zsh-hook precmd __mica_term_zsh_precmd
    add-zsh-hook preexec __mica_term_zsh_preexec
else
    typeset -ga precmd_functions
    typeset -ga preexec_functions
    [[ "${{precmd_functions[(Ie)__mica_term_zsh_precmd]}}" -eq 0 ]] && precmd_functions+=(__mica_term_zsh_precmd)
    [[ "${{preexec_functions[(Ie)__mica_term_zsh_preexec]}}" -eq 0 ]] && preexec_functions+=(__mica_term_zsh_preexec)
fi
fi"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_fish_bootstrap(options: &BootstrapOptions) -> String {
    let private_helper = build_fish_private_helper(options);
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
{private_helper}
if status --is-interactive
    function __mica_term_fish_has_native_markers
        set -l version (status fish-version)
        set -l parts (string split . -- $version)
        set -l major 0
        set -l minor 0
        if test (count $parts) -ge 1
            set major $parts[1]
        end
        if test (count $parts) -ge 2
            set minor $parts[2]
        end
        if test "$major" -gt 4
            return 0
        end
        if test "$major" -eq 4 -a "$minor" -ge 6
            return 0
        end
        return 1
    end
    if not __mica_term_fish_has_native_markers
        if not functions -q __mica_term_original_fish_prompt
            if functions -q fish_prompt
                functions -c fish_prompt __mica_term_original_fish_prompt
            else
                function __mica_term_original_fish_prompt
                end
            end
            function fish_prompt
                __mica_term_original_fish_prompt
                __mica_term_emit_prompt_end
            end
        end
        function __mica_term_fish_prompt_event --on-event fish_prompt
            __mica_term_emit_prompt_start
            __mica_term_emit_cwd
        end
        function __mica_term_fish_preexec_event --on-event fish_preexec
            set -g __mica_term_fish_command_running 1
            __mica_term_emit_command_start
        end
        function __mica_term_fish_postexec_event --on-event fish_postexec
            if set -q __mica_term_fish_command_running
                __mica_term_emit_command_finished $status
                set -e __mica_term_fish_command_running
            end
        end
    end
end"#,
        term_program = options.term_program,
        enhanced_flag = options.enhanced_flag,
        private_helper = private_helper,
    )
}

fn build_posix_private_helper(options: &BootstrapOptions) -> String {
    if !options.private_actions_enabled {
        return String::new();
    }

    format!(
        r#"__mica_term_private_action() {{ printf '\033]9001;{channel};%s;%s\007' "$1" "$2"; }}"#,
        channel = options.private_channel_tag,
    )
}

fn build_fish_private_helper(options: &BootstrapOptions) -> String {
    if !options.private_actions_enabled {
        return String::new();
    }

    format!(
        r#"function __mica_term_private_action
    printf '\033]9001;{channel};%s;%s\007' "$argv[1]" "$argv[2]"
end"#,
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
            parsed.command_finished = true;
            parsed.command_finish_exit_code = *code;
        }
        ShellIntegrationEvent::PrivateAction(_) => {}
    }
}
