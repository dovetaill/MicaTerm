//! Workspace tab projections derived from SSH session handles.

use crate::app::sftp::FileBrowserSessionId;
use crate::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};

pub type WorkspaceTabId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTabKind {
    Terminal,
    Sftp,
    Launcher,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceTab {
    pub tab_id: WorkspaceTabId,
    pub session_id: String,
    pub file_browser_session_id: FileBrowserSessionId,
    pub asset_id: String,
    pub title: String,
    pub subtitle: String,
    pub state: String,
    pub enhanced_session_state: String,
    pub error_detail: String,
    pub active: bool,
    pub kind: WorkspaceTabKind,
}

impl WorkspaceTab {
    pub fn from_session(handle: &SessionHandle) -> Self {
        Self {
            tab_id: handle.session_id.to_string(),
            session_id: handle.session_id.to_string(),
            file_browser_session_id: String::new(),
            asset_id: handle.asset_id.clone(),
            title: resolve_title(&handle.title, &handle.subtitle),
            subtitle: String::new(),
            state: session_state_id(&handle.state).into(),
            enhanced_session_state: enhanced_session_state_id(handle.enhanced_session_state).into(),
            error_detail: session_error_detail(&handle.state).into(),
            active: false,
            kind: WorkspaceTabKind::Terminal,
        }
    }

    pub fn sftp(
        tab_id: impl Into<WorkspaceTabId>,
        file_browser_session_id: impl Into<FileBrowserSessionId>,
        title: impl Into<String>,
    ) -> Self {
        Self {
            tab_id: tab_id.into(),
            session_id: String::new(),
            file_browser_session_id: file_browser_session_id.into(),
            asset_id: String::new(),
            title: title.into(),
            subtitle: String::new(),
            state: "ready".into(),
            enhanced_session_state: String::new(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Sftp,
        }
    }

    pub fn launcher() -> Self {
        Self {
            tab_id: "workspace-launcher".into(),
            session_id: "workspace-launcher".into(),
            file_browser_session_id: String::new(),
            asset_id: String::new(),
            title: "New Tab".into(),
            subtitle: String::new(),
            state: "launcher".into(),
            enhanced_session_state: String::new(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Launcher,
        }
    }

    pub fn is_launcher(&self) -> bool {
        self.kind == WorkspaceTabKind::Launcher
    }

    pub fn can_reconnect(&self) -> bool {
        self.kind == WorkspaceTabKind::Terminal
            && matches!(self.state.as_str(), "cancelled" | "disconnected" | "error")
    }

    pub fn uses_terminal_surface(&self) -> bool {
        self.kind == WorkspaceTabKind::Terminal && matches!(self.state.as_str(), "connected")
    }

    pub fn uses_connection_progress_surface(&self) -> bool {
        self.kind == WorkspaceTabKind::Terminal
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

fn enhanced_session_state_id(state: EnhancedSessionState) -> &'static str {
    match state {
        EnhancedSessionState::Plain => "plain",
        EnhancedSessionState::Enhanced => "enhanced",
        EnhancedSessionState::Fallback => "fallback",
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
