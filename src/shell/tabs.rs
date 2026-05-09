//! Workspace tab projections derived from SSH session handles.

use crate::app::sftp::FileBrowserSessionId;
use crate::app::ssh::profile::ConnectionProfile;
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
    pub display_name: String,
    pub host: String,
    pub username: String,
    pub port: u16,
    pub connection_status: String,
    pub title: String,
    pub subtitle: String,
    pub state: String,
    pub enhanced_session_state: String,
    pub error_detail: String,
    pub active: bool,
    pub kind: WorkspaceTabKind,
    pub reconnectable: bool,
    pub connection_profile: Option<ConnectionProfile>,
}

impl WorkspaceTab {
    pub fn from_session(handle: &SessionHandle) -> Self {
        Self::from_session_with_tab_id(handle, handle.session_id.to_string())
    }

    pub fn from_session_with_tab_id(
        handle: &SessionHandle,
        tab_id: impl Into<WorkspaceTabId>,
    ) -> Self {
        let connection_status = session_state_id(&handle.state).to_string();
        let (username, host, port) = parse_connection_identity(handle.subtitle.as_str());
        let display_name = resolve_title(&handle.title, &host, handle.subtitle.as_str());
        Self {
            tab_id: tab_id.into(),
            session_id: handle.session_id.to_string(),
            file_browser_session_id: String::new(),
            asset_id: handle.asset_id.clone(),
            display_name: display_name.clone(),
            host,
            username,
            port,
            connection_status: connection_status.clone(),
            title: display_name,
            subtitle: String::new(),
            state: connection_status,
            enhanced_session_state: enhanced_session_state_id(handle.enhanced_session_state).into(),
            error_detail: session_error_detail(&handle.state).into(),
            active: false,
            kind: WorkspaceTabKind::Terminal,
            reconnectable: handle.can_reconnect,
            connection_profile: None,
        }
    }

    pub fn terminal_error(
        tab_id: impl Into<WorkspaceTabId>,
        asset_id: impl Into<String>,
        display_name: impl Into<String>,
        username: impl Into<String>,
        host: impl Into<String>,
        port: u16,
        error_detail: impl Into<String>,
        connection_profile: Option<ConnectionProfile>,
    ) -> Self {
        let display_name = display_name.into();
        let username = username.into();
        let host = host.into();
        Self {
            tab_id: tab_id.into(),
            session_id: String::new(),
            file_browser_session_id: String::new(),
            asset_id: asset_id.into(),
            display_name: display_name.clone(),
            host,
            username,
            port,
            connection_status: "error".into(),
            title: display_name,
            subtitle: String::new(),
            state: "error".into(),
            enhanced_session_state: String::new(),
            error_detail: error_detail.into(),
            active: false,
            kind: WorkspaceTabKind::Terminal,
            reconnectable: connection_profile.is_some(),
            connection_profile,
        }
    }

    pub fn sftp(
        tab_id: impl Into<WorkspaceTabId>,
        file_browser_session_id: impl Into<FileBrowserSessionId>,
        title: impl Into<String>,
    ) -> Self {
        let display_name = title.into();
        Self {
            tab_id: tab_id.into(),
            session_id: String::new(),
            file_browser_session_id: file_browser_session_id.into(),
            asset_id: String::new(),
            display_name: display_name.clone(),
            host: String::new(),
            username: String::new(),
            port: 0,
            connection_status: "ready".into(),
            title: display_name,
            subtitle: String::new(),
            state: "ready".into(),
            enhanced_session_state: String::new(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Sftp,
            reconnectable: false,
            connection_profile: None,
        }
    }

    pub fn launcher() -> Self {
        Self {
            tab_id: "workspace-launcher".into(),
            session_id: String::new(),
            file_browser_session_id: String::new(),
            asset_id: String::new(),
            display_name: "New Tab".into(),
            host: String::new(),
            username: String::new(),
            port: 0,
            connection_status: "launcher".into(),
            title: "New Tab".into(),
            subtitle: String::new(),
            state: "launcher".into(),
            enhanced_session_state: String::new(),
            error_detail: String::new(),
            active: false,
            kind: WorkspaceTabKind::Launcher,
            reconnectable: false,
            connection_profile: None,
        }
    }

    pub fn is_launcher(&self) -> bool {
        self.kind == WorkspaceTabKind::Launcher
    }

    pub fn can_reconnect(&self) -> bool {
        match self.kind {
            WorkspaceTabKind::Terminal => self.reconnectable,
            WorkspaceTabKind::Sftp => matches!(self.state.as_str(), "disconnected" | "error"),
            WorkspaceTabKind::Launcher => false,
        }
    }

    pub fn can_clone_connection(&self) -> bool {
        self.kind == WorkspaceTabKind::Terminal && self.connection_profile.is_some()
    }

    pub fn connection_status_label(&self) -> &'static str {
        connection_status_label(self.connection_status.as_str())
    }

    pub fn summary_tooltip_text(&self) -> String {
        let mut lines = Vec::new();
        if !self.display_name.is_empty() {
            lines.push(self.display_name.clone());
        }
        if !self.host.is_empty() {
            lines.push(format!("Host: {}", self.host));
        }
        if !self.username.is_empty() {
            lines.push(format!("User: {}", self.username));
        }
        if self.port > 0 {
            lines.push(format!("Port: {}", self.port));
        }
        lines.push(format!("Status: {}", self.connection_status_label()));
        lines.join("\n")
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

fn connection_status_label(status: &str) -> &'static str {
    match status {
        "connecting" => "Connecting",
        "waiting-user" => "Waiting for approval",
        "connected" => "Connected",
        "cancelled" => "Cancelled",
        "disconnected" => "Disconnected",
        "error" => "Error",
        "launcher" => "New Tab",
        "ready" => "Ready",
        _ => "Unknown",
    }
}

fn enhanced_session_state_id(state: EnhancedSessionState) -> &'static str {
    match state {
        EnhancedSessionState::Plain => "plain",
        EnhancedSessionState::Enhanced => "enhanced",
        EnhancedSessionState::Fallback => "fallback",
    }
}

fn resolve_title(title: &str, host: &str, subtitle: &str) -> String {
    let trimmed = title.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    if !host.is_empty() {
        return host.to_string();
    }

    subtitle.trim().to_string()
}

fn parse_connection_identity(subtitle: &str) -> (String, String, u16) {
    let trimmed = subtitle.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new(), 0);
    }

    let (username, host_port) = trimmed
        .rsplit_once('@')
        .map(|(username, host_port)| (username.trim(), host_port.trim()))
        .unwrap_or(("", trimmed));
    let (host, port) = host_port
        .rsplit_once(':')
        .and_then(|(host, port)| {
            port.trim()
                .parse::<u16>()
                .ok()
                .map(|port| (host.trim(), port))
        })
        .unwrap_or((host_port, 0));

    (username.to_string(), host.to_string(), port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::ssh::session_manager::{EnhancedSessionState, SessionHandle, SessionState};
    use uuid::Uuid;

    #[test]
    fn summary_tooltip_text_uses_structured_session_metadata_lines() {
        let session_id = Uuid::new_v4();
        let tab = WorkspaceTab::from_session(&SessionHandle {
            session_id,
            asset_id: "asset-prod".into(),
            title: "Prod Bastion".into(),
            subtitle: "ops@10.0.0.12:22".into(),
            state: SessionState::Connected,
            can_reconnect: false,
            enhanced_session_state: EnhancedSessionState::Plain,
        });

        assert_eq!(
            tab.summary_tooltip_text(),
            "Prod Bastion\nHost: 10.0.0.12\nUser: ops\nPort: 22\nStatus: Connected"
        );
    }
}
