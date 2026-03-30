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
