//! Workspace tab projections derived from SSH session handles.

use crate::app::ssh::session_manager::{SessionHandle, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTabKind {
    Session,
    Launcher,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTab {
    pub session_id: String,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: String,
    pub error_detail: String,
    pub active: bool,
    pub kind: WorkspaceTabKind,
}

impl WorkspaceTab {
    pub fn from_session(handle: &SessionHandle) -> Self {
        Self {
            session_id: handle.session_id.to_string(),
            asset_id: handle.asset_id.clone(),
            title: resolve_title(&handle.title, &handle.subtitle),
            subtitle: String::new(),
            state: session_state_id(&handle.state).into(),
            error_detail: session_error_detail(&handle.state).into(),
            active: false,
            kind: WorkspaceTabKind::Session,
        }
    }

    pub fn launcher() -> Self {
        Self {
            session_id: "workspace-launcher".into(),
            asset_id: String::new(),
            title: "New Tab".into(),
            subtitle: String::new(),
            state: "launcher".into(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Launcher,
        }
    }

    pub fn is_launcher(&self) -> bool {
        self.kind == WorkspaceTabKind::Launcher
    }

    pub fn can_reconnect(&self) -> bool {
        self.kind == WorkspaceTabKind::Session
            && matches!(self.state.as_str(), "cancelled" | "disconnected" | "error")
    }

    pub fn uses_terminal_surface(&self) -> bool {
        self.kind == WorkspaceTabKind::Session && matches!(self.state.as_str(), "connected")
    }

    pub fn uses_connection_progress_surface(&self) -> bool {
        self.kind == WorkspaceTabKind::Session
            && matches!(
                self.state.as_str(),
                "connecting" | "waiting-user" | "cancelled"
            )
    }
}

fn session_error_detail(state: &SessionState) -> &str {
    match state {
        SessionState::Error(message) => message.as_str(),
        _ => "",
    }
}

fn session_state_id(state: &SessionState) -> &'static str {
    match state {
        SessionState::Connecting => "connecting",
        SessionState::WaitingUser => "waiting-user",
        SessionState::Connected => "connected",
        SessionState::Cancelled => "cancelled",
        SessionState::Disconnected => "disconnected",
        SessionState::Error(_) => "error",
    }
}

fn resolve_title(title: &str, subtitle: &str) -> String {
    let trimmed = title.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    subtitle
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(subtitle)
        .split(':')
        .next()
        .unwrap_or(subtitle)
        .to_string()
}
