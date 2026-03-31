use crate::app::sftp::{SftpDirectoryEntry, SftpFollowMode, SftpPanelMode, SftpPathHistory};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpBrowserSessionState {
    pub mode: SftpPanelMode,
    pub follow_mode: SftpFollowMode,
    pub current_path: String,
    pub history: SftpPathHistory,
    pub entries: Vec<SftpDirectoryEntry>,
    pub selected_entry_ids: Vec<String>,
    pub last_error: Option<String>,
    pub active_request_id: Option<u64>,
}

impl Default for SftpBrowserSessionState {
    fn default() -> Self {
        Self {
            mode: SftpPanelMode::Empty,
            follow_mode: SftpFollowMode::FollowCwd,
            current_path: String::new(),
            history: SftpPathHistory::default(),
            entries: Vec::new(),
            selected_entry_ids: Vec::new(),
            last_error: None,
            active_request_id: None,
        }
    }
}

impl SftpBrowserSessionState {
    pub fn set_connecting(&mut self, path: &str, request_id: u64) {
        self.follow_mode = SftpFollowMode::FollowCwd;
        self.current_path = path.to_string();
        self.history.push(path.to_string());
        self.mode = SftpPanelMode::Connecting;
        self.last_error = None;
        self.active_request_id = Some(request_id);
    }

    pub fn set_loading_follow(&mut self, path: &str, request_id: u64) {
        self.follow_mode = SftpFollowMode::FollowCwd;
        self.current_path = path.to_string();
        self.history.push(path.to_string());
        self.mode = SftpPanelMode::Loading;
        self.last_error = None;
        self.active_request_id = Some(request_id);
    }

    pub fn set_loading_manual(&mut self, path: &str, request_id: u64) {
        self.follow_mode = SftpFollowMode::ManualBrowse;
        self.current_path = path.to_string();
        self.history.push(path.to_string());
        self.mode = SftpPanelMode::Loading;
        self.last_error = None;
        self.active_request_id = Some(request_id);
    }

    pub fn set_ready(&mut self, path: &str, entries: Vec<SftpDirectoryEntry>) {
        self.mode = SftpPanelMode::Ready;
        self.current_path = path.to_string();
        self.entries = entries;
        self.last_error = None;
    }

    pub fn set_error(&mut self, path: &str, message: String) {
        self.mode = SftpPanelMode::Error;
        self.current_path = path.to_string();
        self.last_error = Some(message);
        self.active_request_id = None;
    }

    pub fn set_retrying(&mut self, path: &str, request_id: u64) {
        self.current_path = path.to_string();
        self.mode = SftpPanelMode::Connecting;
        self.last_error = None;
        self.active_request_id = Some(request_id);
    }

    pub fn mark_disconnected(&mut self) {
        self.mode = SftpPanelMode::Disconnected;
        self.active_request_id = None;
    }

    pub fn accepts_request(&self, request_id: u64) -> bool {
        self.active_request_id == Some(request_id)
    }
}
