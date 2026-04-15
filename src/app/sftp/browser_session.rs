//! File-browser session identities shared by quick browser and workspace tabs.

use std::sync::atomic::{AtomicU64, Ordering};

use super::model::{SftpDirectoryEntry, SftpFollowMode, SftpPanelMode, SftpPathHistory};

pub type FileBrowserSessionId = String;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserSortColumn {
    Name,
    Type,
    Modified,
    Size,
}

impl FileBrowserSortColumn {
    pub fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Type => "type",
            Self::Modified => "modified",
            Self::Size => "size",
        }
    }

    pub fn from_id(value: &str) -> Option<Self> {
        match value {
            "name" => Some(Self::Name),
            "type" => Some(Self::Type),
            "modified" => Some(Self::Modified),
            "size" => Some(Self::Size),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileBrowserSortDirection {
    Asc,
    Desc,
}

impl FileBrowserSortDirection {
    pub fn id(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileBrowserSortState {
    pub column: Option<FileBrowserSortColumn>,
    pub direction: Option<FileBrowserSortDirection>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileBrowserColumnLayout {
    pub name_px: f32,
    pub type_px: f32,
    pub modified_px: f32,
    pub size_px: f32,
}

impl Default for FileBrowserColumnLayout {
    fn default() -> Self {
        Self {
            name_px: 226.0,
            type_px: 78.0,
            modified_px: 150.0,
            size_px: 72.0,
        }
    }
}

pub const FILE_BROWSER_TYPE_COLUMN_MIN_PX: f32 = 72.0;
pub const FILE_BROWSER_MODIFIED_COLUMN_MIN_PX: f32 = 132.0;
pub const FILE_BROWSER_SIZE_COLUMN_MIN_PX: f32 = 72.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostProfileRef {
    pub asset_id: String,
    pub label: String,
}

impl HostProfileRef {
    pub fn new(asset_id: impl Into<String>) -> Self {
        let asset_id = asset_id.into();
        Self {
            label: asset_id.clone(),
            asset_id,
        }
    }

    pub fn with_label(asset_id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            asset_id: asset_id.into(),
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileBrowserSession {
    pub file_browser_session_id: FileBrowserSessionId,
    pub host_profile_ref: HostProfileRef,
    pub linked_terminal_session_id: Option<String>,
    pub mode: SftpPanelMode,
    pub follow_mode: SftpFollowMode,
    pub current_path: String,
    pub history: SftpPathHistory,
    pub entries: Vec<SftpDirectoryEntry>,
    pub selected_entry_ids: Vec<String>,
    pub last_error: Option<String>,
    pub active_request_id: Option<u64>,
    pub sort_state: FileBrowserSortState,
    pub column_layout: FileBrowserColumnLayout,
}

impl FileBrowserSession {
    pub fn quick_browser(
        host_profile_ref: HostProfileRef,
        current_path: impl Into<String>,
    ) -> Self {
        let current_path = current_path.into();
        Self {
            file_browser_session_id: new_file_browser_session_id(),
            host_profile_ref,
            linked_terminal_session_id: None,
            mode: SftpPanelMode::Empty,
            follow_mode: SftpFollowMode::FollowCwd,
            current_path: current_path.clone(),
            history: SftpPathHistory::with_initial(current_path),
            entries: Vec::new(),
            selected_entry_ids: Vec::new(),
            last_error: None,
            active_request_id: None,
            sort_state: FileBrowserSortState::default(),
            column_layout: FileBrowserColumnLayout::default(),
        }
    }

    pub fn clone_for_workspace(&self) -> Self {
        Self {
            file_browser_session_id: new_file_browser_session_id(),
            host_profile_ref: self.host_profile_ref.clone(),
            linked_terminal_session_id: self.linked_terminal_session_id.clone(),
            mode: self.mode,
            follow_mode: self.follow_mode,
            current_path: self.current_path.clone(),
            history: self.history.clone(),
            entries: self.entries.clone(),
            selected_entry_ids: Vec::new(),
            last_error: self.last_error.clone(),
            active_request_id: self.active_request_id,
            sort_state: self.sort_state,
            column_layout: self.column_layout,
        }
    }

    pub fn attach_terminal_session_id(&mut self, session_id: impl Into<String>) {
        self.linked_terminal_session_id = Some(session_id.into());
    }

    pub fn mark_connecting(&mut self) {
        self.mode = SftpPanelMode::Connecting;
    }

    pub fn mark_loading(&mut self) {
        self.mode = SftpPanelMode::Loading;
    }

    pub fn mark_ready(&mut self) {
        self.mode = SftpPanelMode::Ready;
    }

    pub fn mark_disconnected(&mut self) {
        self.mode = SftpPanelMode::Disconnected;
        self.active_request_id = None;
    }

    pub fn navigate_manual(&mut self, path: impl Into<String>) {
        let path = path.into();
        self.follow_mode = SftpFollowMode::ManualBrowse;
        self.current_path = path.clone();
        self.mode = SftpPanelMode::Ready;
        self.history.push(path);
    }

    pub fn follow_terminal_path(&mut self, path: impl Into<String>) {
        if self.follow_mode != SftpFollowMode::FollowCwd {
            return;
        }

        let path = path.into();
        self.current_path = path.clone();
        self.history.push(path);
    }

    pub fn reenable_follow(&mut self, path: impl Into<String>) {
        self.follow_mode = SftpFollowMode::FollowCwd;
        self.follow_terminal_path(path);
    }

    pub fn can_navigate_up(&self) -> bool {
        remote_parent_path(&self.current_path).is_some()
    }

    pub fn navigate_back(&mut self) -> bool {
        let Some(path) = self.history.back().map(ToString::to_string) else {
            return false;
        };
        self.follow_mode = SftpFollowMode::ManualBrowse;
        self.current_path = path;
        true
    }

    pub fn navigate_forward(&mut self) -> bool {
        let Some(path) = self.history.forward().map(ToString::to_string) else {
            return false;
        };
        self.follow_mode = SftpFollowMode::ManualBrowse;
        self.current_path = path;
        true
    }

    pub fn navigate_up(&mut self) -> bool {
        let Some(parent) = remote_parent_path(&self.current_path) else {
            return false;
        };
        self.navigate_manual(parent);
        true
    }
}

fn new_file_browser_session_id() -> FileBrowserSessionId {
    static NEXT_BROWSER_SESSION_ID: AtomicU64 = AtomicU64::new(1);

    format!(
        "browser-session-{}",
        NEXT_BROWSER_SESSION_ID.fetch_add(1, Ordering::Relaxed)
    )
}

fn remote_parent_path(path: &str) -> Option<String> {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return None;
    }

    let normalized = trimmed.trim_end_matches('/');
    match normalized.rsplit_once('/') {
        Some(("", _)) => Some("/".into()),
        Some((parent, _)) if !parent.is_empty() => Some(parent.into()),
        _ => None,
    }
}
