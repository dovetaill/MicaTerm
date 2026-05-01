use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHeadlineState {
    Connecting,
    WaitingUser,
    Connected,
    Cancelled,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStepState {
    Pending,
    Running,
    Done,
    Blocked,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionVisualState {
    VerifyingHostKey,
    Connecting,
    HostKeyWarning,
    Failed,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHopKind {
    Local,
    JumpHost,
    Target,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionHopVisualState {
    Completed,
    Current,
    Pending,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionPreviewState {
    VerifyingHostKeyDirect,
    VerifyingHostKeyWithJumpHost,
    ConnectingTrustedDirect,
    ConnectingTrustedWithJumpHost,
    HostKeyChangedWarning,
    FailedJumpHost,
}

impl ConnectionPreviewState {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim() {
            "verifying_host_key_direct" => Some(Self::VerifyingHostKeyDirect),
            "verifying_host_key_with_jump_host" => Some(Self::VerifyingHostKeyWithJumpHost),
            "connecting_trusted_direct" => Some(Self::ConnectingTrustedDirect),
            "connecting_trusted_with_jump_host" => Some(Self::ConnectingTrustedWithJumpHost),
            "host_key_changed_warning" => Some(Self::HostKeyChangedWarning),
            "failed_jump_host" => Some(Self::FailedJumpHost),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifyingHostKeyDirect => "verifying_host_key_direct",
            Self::VerifyingHostKeyWithJumpHost => "verifying_host_key_with_jump_host",
            Self::ConnectingTrustedDirect => "connecting_trusted_direct",
            Self::ConnectingTrustedWithJumpHost => "connecting_trusted_with_jump_host",
            Self::HostKeyChangedWarning => "host_key_changed_warning",
            Self::FailedJumpHost => "failed_jump_host",
        }
    }

    pub fn fixture(self) -> ConnectionPreviewFixture {
        match self {
            Self::VerifyingHostKeyDirect => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::WaitingUser,
                visual_state: ConnectionVisualState::VerifyingHostKey,
                current_hop_label: "Target".into(),
                current_detail: "Waiting for you to verify the target host key.".into(),
                task_title: "Verify host key".into(),
                task_detail: "The authenticity of the target host cannot be established. Please verify the host key fingerprint below before continuing.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Current),
                ],
                progress_steps: vec![],
                main_fields: vec![
                    field("Host", "server.interserver.com", Some("server.interserver.com"), false),
                    field("Port", "22 (SSH)", Some("22"), false),
                    field("Fingerprint", "SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c", Some("SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c"), true),
                ],
                detail_fields: vec![
                    field("User", "deploy", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "Direct connection", None, false),
                ],
                diagnostics: vec![
                    "Host key verification required for server.interserver.com:22".into(),
                ],
                warning_expected: String::new(),
                warning_current: String::new(),
                warning_host: String::new(),
                prompt: Some(ConnectionHostKeyPrompt {
                    host: "server.interserver.com".into(),
                    port: 22,
                    fingerprint: "SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c".into(),
                    public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakePreviewDirectKey".into(),
                }),
            },
            Self::VerifyingHostKeyWithJumpHost => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::WaitingUser,
                visual_state: ConnectionVisualState::VerifyingHostKey,
                current_hop_label: "Target".into(),
                current_detail: "Waiting for you to verify the target host key.".into(),
                task_title: "Verify host key".into(),
                task_detail: "The authenticity of the target host cannot be established. Please verify the host key fingerprint below before continuing.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    jump_hop(1, "jump.example.com", 22, ConnectionHopVisualState::Completed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Current),
                ],
                progress_steps: vec![],
                main_fields: vec![
                    field("Host", "server.interserver.com", Some("server.interserver.com"), false),
                    field("Port", "22 (SSH)", Some("22"), false),
                    field("Fingerprint", "SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c", Some("SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c"), true),
                    field("Jump Host", "jump.example.com:22", Some("jump.example.com:22"), false),
                ],
                detail_fields: vec![
                    field("User", "deploy", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "Local -> Jump Host 1 -> Target", None, false),
                ],
                diagnostics: vec![
                    "Connected to jump host jump.example.com:22".into(),
                    "Host key verification required for server.interserver.com:22".into(),
                ],
                warning_expected: String::new(),
                warning_current: String::new(),
                warning_host: String::new(),
                prompt: Some(ConnectionHostKeyPrompt {
                    host: "server.interserver.com".into(),
                    port: 22,
                    fingerprint: "SHA256:8b:7d:4a:77:3c:19:6e:2f:bc:91:5a:0d:3e:88:bf:4c".into(),
                    public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFakePreviewJumpTargetKey".into(),
                }),
            },
            Self::ConnectingTrustedDirect => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::Connecting,
                visual_state: ConnectionVisualState::Connecting,
                current_hop_label: "Target".into(),
                current_detail: "Authenticating the secure session with deploy@server.interserver.com.".into(),
                task_title: "Connecting".into(),
                task_detail: "Using the trusted host key. Finishing authentication and preparing the remote shell.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Current),
                ],
                progress_steps: vec![
                    step("done", "Local", "Resolving profile", "Loaded saved SSH profile"),
                    step("done", "Target", "Connecting", "Connected to server.interserver.com:22"),
                    step("running", "Target", "Authenticating", "Authenticating as deploy"),
                    step("pending", "Target", "Opening session", "Waiting for terminal channel"),
                ],
                main_fields: vec![
                    field("Target", "server.interserver.com", Some("server.interserver.com"), false),
                    field("Path", "Direct", None, false),
                    field("Port", "22 (SSH)", Some("22"), false),
                    field("Auth", "Private key", None, false),
                ],
                detail_fields: vec![
                    field("User", "deploy", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "Direct connection", None, false),
                ],
                diagnostics: vec![
                    "Loaded profile Interserver".into(),
                    "Connected to target server.interserver.com:22".into(),
                ],
                warning_expected: String::new(),
                warning_current: String::new(),
                warning_host: String::new(),
                prompt: None,
            },
            Self::ConnectingTrustedWithJumpHost => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::Connecting,
                visual_state: ConnectionVisualState::Connecting,
                current_hop_label: "Target".into(),
                current_detail: "Jump Host 1 is ready. Opening the remote shell on the target host.".into(),
                task_title: "Connecting".into(),
                task_detail: "Trusted host keys are already known. Completing the multi-hop path and preparing the terminal session.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    jump_hop(1, "jump.example.com", 22, ConnectionHopVisualState::Completed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Current),
                ],
                progress_steps: vec![
                    step("done", "Local", "Resolving profile", "Loaded saved SSH profile"),
                    step("done", "Jump Host 1", "Connecting", "Connected to jump.example.com:22"),
                    step("done", "Jump Host 1", "Authenticating", "Authenticated as jump-user"),
                    step("done", "Target", "Connecting", "Connected to server.interserver.com:22"),
                    step("running", "Target", "Opening session", "Creating terminal channel"),
                ],
                main_fields: vec![
                    field("Target", "server.interserver.com", Some("server.interserver.com"), false),
                    field("Path", "jump.example.com:22", Some("jump.example.com:22"), false),
                    field("Port", "22 (SSH)", Some("22"), false),
                    field("Auth", "Private key", None, false),
                ],
                detail_fields: vec![
                    field("User", "deploy", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "jump.example.com:22", None, false),
                ],
                diagnostics: vec![
                    "Connected to jump host jump.example.com:22".into(),
                    "Connected to target server.interserver.com:22".into(),
                ],
                warning_expected: String::new(),
                warning_current: String::new(),
                warning_host: String::new(),
                prompt: None,
            },
            Self::HostKeyChangedWarning => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::Error,
                visual_state: ConnectionVisualState::HostKeyWarning,
                current_hop_label: "Target".into(),
                current_detail: "The previously trusted host key no longer matches.".into(),
                task_title: "Host key changed".into(),
                task_detail: "The server presented a different host key than the one previously trusted for this host. This may indicate the server was reinstalled, the key rotated, or a man-in-the-middle risk.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    jump_hop(1, "jump.example.com", 22, ConnectionHopVisualState::Completed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Failed),
                ],
                progress_steps: vec![],
                main_fields: vec![
                    field("Host", "server.interserver.com", Some("server.interserver.com"), false),
                    field("Port", "22 (SSH)", Some("22"), false),
                    field("Previously trusted", "SHA256:ab:17:d2:99:93:ca:ff:41:ce:80:02:1f:51:91:44:00", Some("SHA256:ab:17:d2:99:93:ca:ff:41:ce:80:02:1f:51:91:44:00"), true),
                    field("Presented now", "SHA256:3f:90:55:7a:10:da:25:8f:b8:81:44:21:91:1c:be:72", Some("SHA256:3f:90:55:7a:10:da:25:8f:b8:81:44:21:91:1c:be:72"), true),
                ],
                detail_fields: vec![
                    field("User", "deploy", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "jump.example.com:22", None, false),
                ],
                diagnostics: vec![
                    "Connected to jump host jump.example.com:22".into(),
                    "SSH host key changed for server.interserver.com:22".into(),
                ],
                warning_expected: "SHA256:ab:17:d2:99:93:ca:ff:41:ce:80:02:1f:51:91:44:00".into(),
                warning_current: "SHA256:3f:90:55:7a:10:da:25:8f:b8:81:44:21:91:1c:be:72".into(),
                warning_host: "server.interserver.com:22".into(),
                prompt: None,
            },
            Self::FailedJumpHost => ConnectionPreviewFixture {
                session_title: "Interserver".into(),
                headline: ConnectionHeadlineState::Error,
                visual_state: ConnectionVisualState::Failed,
                current_hop_label: "Jump Host 1".into(),
                current_detail: "Authentication failed while connecting to the jump host.".into(),
                task_title: "Connection failed".into(),
                task_detail: "The connection could not continue because Jump Host 1 rejected the configured credentials.".into(),
                hops: vec![
                    local_hop(ConnectionHopVisualState::Completed),
                    jump_hop(1, "jump.example.com", 22, ConnectionHopVisualState::Failed),
                    target_hop("Target", "server.interserver.com", 22, ConnectionHopVisualState::Pending),
                ],
                progress_steps: vec![],
                main_fields: vec![
                    field("Failed at", "Jump Host 1", None, false),
                    field("Host", "jump.example.com", Some("jump.example.com"), false),
                    field("Port", "22 (SSH)", Some("22"), false),
                ],
                detail_fields: vec![
                    field("User", "jump-user", None, false),
                    field("Authentication", "Private key", None, false),
                    field("Port", "22", None, false),
                    field("Strict host key checking", "On", None, false),
                    field("Jump chain", "jump.example.com:22", None, false),
                ],
                diagnostics: vec![
                    "Connected to jump host jump.example.com:22".into(),
                    "Authentication failed for jump-user@jump.example.com".into(),
                ],
                warning_expected: String::new(),
                warning_current: String::new(),
                warning_host: String::new(),
                prompt: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionHopStateItem {
    pub kind: ConnectionHopKind,
    pub label: String,
    pub subtitle: String,
    pub host: String,
    pub port: u16,
    pub state: ConnectionHopVisualState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionInfoField {
    pub label: String,
    pub value: String,
    pub copy_value: Option<String>,
    pub monospace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionPreviewFixture {
    pub session_title: String,
    pub headline: ConnectionHeadlineState,
    pub visual_state: ConnectionVisualState,
    pub current_hop_label: String,
    pub current_detail: String,
    pub task_title: String,
    pub task_detail: String,
    pub hops: Vec<ConnectionHopStateItem>,
    pub progress_steps: Vec<ConnectionStepStateItem>,
    pub main_fields: Vec<ConnectionInfoField>,
    pub detail_fields: Vec<ConnectionInfoField>,
    pub diagnostics: Vec<String>,
    pub warning_expected: String,
    pub warning_current: String,
    pub warning_host: String,
    pub prompt: Option<ConnectionHostKeyPrompt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionStepStateItem {
    pub step_id: String,
    pub step_kind: String,
    pub title: String,
    pub detail: String,
    pub hop_label: String,
    pub state: ConnectionStepState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDiagnosticLine {
    pub attempt_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionHostKeyPrompt {
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub public_key_openssh: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionAttemptState {
    pub attempt_id: Uuid,
    pub headline: ConnectionHeadlineState,
    pub steps: Vec<ConnectionStepStateItem>,
    pub diagnostics: Vec<ConnectionDiagnosticLine>,
    pub prompt: Option<ConnectionHostKeyPrompt>,
}

impl ConnectionAttemptState {
    pub fn new(headline: ConnectionHeadlineState) -> Self {
        Self::with_attempt_id(Uuid::new_v4(), headline)
    }

    pub fn with_attempt_id(attempt_id: Uuid, headline: ConnectionHeadlineState) -> Self {
        Self {
            attempt_id,
            headline,
            steps: Vec::new(),
            diagnostics: Vec::new(),
            prompt: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionProgressEvent {
    AttemptStarted {
        attempt_id: Uuid,
        headline: ConnectionHeadlineState,
    },
    HeadlineChanged {
        attempt_id: Uuid,
        headline: ConnectionHeadlineState,
    },
    StepUpdated {
        attempt_id: Uuid,
        step: ConnectionStepStateItem,
    },
    DiagnosticAppended {
        attempt_id: Uuid,
        message: String,
    },
}

fn local_hop(state: ConnectionHopVisualState) -> ConnectionHopStateItem {
    ConnectionHopStateItem {
        kind: ConnectionHopKind::Local,
        label: "Local".into(),
        subtitle: "You".into(),
        host: "local".into(),
        port: 0,
        state,
    }
}

fn jump_hop(
    index: usize,
    host: &str,
    port: u16,
    state: ConnectionHopVisualState,
) -> ConnectionHopStateItem {
    ConnectionHopStateItem {
        kind: ConnectionHopKind::JumpHost,
        label: format!("Jump Host {index}"),
        subtitle: host.into(),
        host: host.into(),
        port,
        state,
    }
}

fn target_hop(
    label: &str,
    host: &str,
    port: u16,
    state: ConnectionHopVisualState,
) -> ConnectionHopStateItem {
    ConnectionHopStateItem {
        kind: ConnectionHopKind::Target,
        label: label.into(),
        subtitle: host.into(),
        host: host.into(),
        port,
        state,
    }
}

fn field(
    label: &str,
    value: &str,
    copy_value: Option<&str>,
    monospace: bool,
) -> ConnectionInfoField {
    ConnectionInfoField {
        label: label.into(),
        value: value.into(),
        copy_value: copy_value.map(str::to_string),
        monospace,
    }
}

fn step(state: &str, hop_label: &str, title: &str, detail: &str) -> ConnectionStepStateItem {
    let step_state = match state {
        "done" => ConnectionStepState::Done,
        "running" => ConnectionStepState::Running,
        "failed" => ConnectionStepState::Failed,
        "blocked" => ConnectionStepState::Blocked,
        "cancelled" => ConnectionStepState::Cancelled,
        _ => ConnectionStepState::Pending,
    };
    ConnectionStepStateItem {
        step_id: format!(
            "{}-{}",
            hop_label.to_ascii_lowercase().replace(' ', "-"),
            title.to_ascii_lowercase().replace(' ', "-")
        ),
        step_kind: title.to_ascii_lowercase().replace(' ', "-"),
        title: title.into(),
        detail: detail.into(),
        hop_label: hop_label.into(),
        state: step_state,
    }
}

#[cfg(test)]
mod tests {
    use super::{ConnectionPreviewState, ConnectionVisualState};

    #[test]
    fn parses_all_preview_states() {
        let states = [
            "verifying_host_key_direct",
            "verifying_host_key_with_jump_host",
            "connecting_trusted_direct",
            "connecting_trusted_with_jump_host",
            "host_key_changed_warning",
            "failed_jump_host",
        ];

        for state in states {
            assert!(
                ConnectionPreviewState::parse(state).is_some(),
                "preview state `{state}` should parse"
            );
        }
    }

    #[test]
    fn jump_host_preview_fixture_contains_three_hops() {
        let fixture = ConnectionPreviewState::VerifyingHostKeyWithJumpHost.fixture();
        assert_eq!(
            fixture.visual_state,
            ConnectionVisualState::VerifyingHostKey
        );
        assert_eq!(fixture.hops.len(), 3);
        assert_eq!(fixture.hops[1].label, "Jump Host 1");
        assert_eq!(fixture.hops[2].label, "Target");
    }

    #[test]
    fn warning_preview_fixture_contains_expected_and_current_keys() {
        let fixture = ConnectionPreviewState::HostKeyChangedWarning.fixture();
        assert_eq!(fixture.visual_state, ConnectionVisualState::HostKeyWarning);
        assert!(!fixture.warning_expected.is_empty());
        assert!(!fixture.warning_current.is_empty());
    }
}
