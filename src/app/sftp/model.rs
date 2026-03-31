//! Session-bound SFTP panel state and navigation reducers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpPanelMode {
    Empty,
    Connecting,
    Loading,
    Ready,
    Disconnected,
    Error,
}

impl SftpPanelMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Connecting => "connecting",
            Self::Loading => "loading",
            Self::Ready => "ready",
            Self::Disconnected => "disconnected",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpFollowMode {
    FollowCwd,
    ManualBrowse,
}

impl SftpFollowMode {
    pub fn id(self) -> &'static str {
        match self {
            Self::FollowCwd => "follow-cwd",
            Self::ManualBrowse => "manual-browse",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpDirectoryEntryKind {
    File,
    Directory,
    Symlink,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpDirectoryEntry {
    pub id: String,
    pub name: String,
    pub path: String,
    pub kind: SftpDirectoryEntryKind,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SftpPathHistory {
    entries: Vec<String>,
    cursor: Option<usize>,
}

impl SftpPathHistory {
    pub fn with_initial(path: impl Into<String>) -> Self {
        let mut history = Self::default();
        history.push(path);
        history
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn current(&self) -> Option<&str> {
        self.cursor
            .and_then(|cursor| self.entries.get(cursor).map(String::as_str))
    }

    pub fn push(&mut self, path: impl Into<String>) {
        let path = path.into();
        if self.current() == Some(path.as_str()) {
            return;
        }

        match self.cursor {
            Some(cursor) => self.entries.truncate(cursor + 1),
            None => self.entries.clear(),
        }

        self.entries.push(path);
        self.cursor = Some(self.entries.len() - 1);
    }

    pub fn can_back(&self) -> bool {
        self.cursor.is_some_and(|cursor| cursor > 0)
    }

    pub fn can_forward(&self) -> bool {
        self.cursor
            .is_some_and(|cursor| cursor + 1 < self.entries.len())
    }

    pub fn back(&mut self) -> Option<&str> {
        let cursor = self.cursor?;
        if cursor == 0 {
            return None;
        }

        self.cursor = Some(cursor - 1);
        self.current()
    }

    pub fn forward(&mut self) -> Option<&str> {
        let cursor = self.cursor?;
        if cursor + 1 >= self.entries.len() {
            return None;
        }

        self.cursor = Some(cursor + 1);
        self.current()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpSessionBindingState {
    pub mode: SftpPanelMode,
    pub follow_mode: SftpFollowMode,
    pub current_path: String,
    pub history: SftpPathHistory,
    pub entries: Vec<SftpDirectoryEntry>,
    pub selected_entry_ids: Vec<String>,
    pub last_error: Option<String>,
}

impl Default for SftpSessionBindingState {
    fn default() -> Self {
        Self {
            mode: SftpPanelMode::Empty,
            follow_mode: SftpFollowMode::FollowCwd,
            current_path: String::new(),
            history: SftpPathHistory::default(),
            entries: Vec::new(),
            selected_entry_ids: Vec::new(),
            last_error: None,
        }
    }
}

impl SftpSessionBindingState {
    pub fn follow(initial_path: impl Into<String>) -> Self {
        let initial_path = initial_path.into();
        Self {
            current_path: initial_path.clone(),
            history: SftpPathHistory::with_initial(initial_path),
            ..Self::default()
        }
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

    pub fn can_navigate_up(&self) -> bool {
        remote_parent_path(&self.current_path).is_some()
    }

    pub fn navigate_up(&mut self) -> bool {
        let Some(parent) = remote_parent_path(&self.current_path) else {
            return false;
        };
        self.navigate_manual(parent);
        true
    }
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
