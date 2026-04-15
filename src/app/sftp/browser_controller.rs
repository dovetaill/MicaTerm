use std::collections::{HashMap, HashSet};

use uuid::Uuid;

use crate::app::sftp::{FileBrowserSession, SftpBrowserSessionState, SftpDirectoryEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpBrowserLoadRequest {
    pub file_browser_session_id: String,
    pub session_id: Uuid,
    pub path: String,
    pub request_id: u64,
}

#[derive(Debug, Default)]
pub struct SftpBrowserController {
    next_request_id: u64,
    sessions: HashMap<String, SftpBrowserSessionState>,
    in_flight_request_ids: HashSet<u64>,
}

impl SftpBrowserController {
    pub fn open(&mut self, session_id: Uuid, path: &str) -> SftpBrowserLoadRequest {
        let browser_session_id = session_id.to_string();
        let request = self.new_request(browser_session_id.clone(), session_id, path);
        let state = self.sessions.entry(browser_session_id).or_default();
        state.set_connecting(request.path.as_str(), request.request_id);
        request
    }

    pub fn open_file_browser_session(
        &mut self,
        browser_session: FileBrowserSession,
    ) -> SftpBrowserLoadRequest {
        let session_id = browser_session
            .linked_terminal_session_id
            .as_deref()
            .and_then(|session_id| Uuid::parse_str(session_id).ok())
            .unwrap_or_else(Uuid::nil);
        let request = self.new_request(
            browser_session.file_browser_session_id.clone(),
            session_id,
            browser_session.current_path.as_str(),
        );
        let state = self
            .sessions
            .entry(browser_session.file_browser_session_id)
            .or_default();
        state.set_connecting(request.path.as_str(), request.request_id);
        request
    }

    pub fn session_activated(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let browser_session_id = session_id.to_string();
        let path = self.sessions.get(&browser_session_id)?.current_path.clone();
        if path.is_empty() {
            return None;
        }

        let request = self.new_request(browser_session_id.clone(), session_id, path.as_str());
        let state = self
            .sessions
            .get_mut(&browser_session_id)
            .expect("state must exist");
        state.set_loading_follow(request.path.as_str(), request.request_id);
        Some(request)
    }

    pub fn follow_cwd(&mut self, session_id: Uuid, path: &str) -> Option<SftpBrowserLoadRequest> {
        let browser_session_id = session_id.to_string();
        let state = self.sessions.entry(browser_session_id.clone()).or_default();
        if !state.current_path.is_empty()
            && state.follow_mode != crate::app::sftp::SftpFollowMode::FollowCwd
        {
            return None;
        }
        if state.current_path == path {
            return None;
        }

        let request = self.new_request(browser_session_id.clone(), session_id, path);
        let state = self.sessions.entry(browser_session_id).or_default();
        state.set_loading_follow(request.path.as_str(), request.request_id);
        Some(request)
    }

    pub fn navigate(&mut self, session_id: Uuid, path: &str) -> SftpBrowserLoadRequest {
        let browser_session_id = session_id.to_string();
        let request = self.new_request(browser_session_id.clone(), session_id, path);
        let state = self.sessions.entry(browser_session_id).or_default();
        state.set_loading_manual(request.path.as_str(), request.request_id);
        request
    }

    pub fn refresh(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let browser_session_id = session_id.to_string();
        let path = self.sessions.get(&browser_session_id)?.current_path.clone();
        if path.is_empty() {
            return None;
        }

        let request = self.new_request(browser_session_id.clone(), session_id, path.as_str());
        let state = self
            .sessions
            .get_mut(&browser_session_id)
            .expect("state must exist");
        if state.follow_mode == crate::app::sftp::SftpFollowMode::FollowCwd {
            state.set_loading_follow(request.path.as_str(), request.request_id);
        } else {
            state.set_loading_manual(request.path.as_str(), request.request_id);
        }
        Some(request)
    }

    pub fn retry(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let browser_session_id = session_id.to_string();
        let path = self.sessions.get(&browser_session_id)?.current_path.clone();
        if path.is_empty() {
            return None;
        }

        let request = self.new_request(browser_session_id.clone(), session_id, path.as_str());
        let state = self
            .sessions
            .get_mut(&browser_session_id)
            .expect("state must exist");
        state.set_retrying(request.path.as_str(), request.request_id);
        Some(request)
    }

    pub fn navigate_back(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let request_id = self.next_request_id();
        let browser_session_id = session_id.to_string();
        let path = self
            .sessions
            .get_mut(&browser_session_id)?
            .navigate_back(request_id)?;
        Some(SftpBrowserLoadRequest {
            file_browser_session_id: browser_session_id,
            session_id,
            path,
            request_id,
        })
    }

    pub fn navigate_forward(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let request_id = self.next_request_id();
        let browser_session_id = session_id.to_string();
        let path = self
            .sessions
            .get_mut(&browser_session_id)?
            .navigate_forward(request_id)?;
        Some(SftpBrowserLoadRequest {
            file_browser_session_id: browser_session_id,
            session_id,
            path,
            request_id,
        })
    }

    pub fn navigate_up(&mut self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let request_id = self.next_request_id();
        let browser_session_id = session_id.to_string();
        let path = self
            .sessions
            .get_mut(&browser_session_id)?
            .navigate_up(request_id)?;
        Some(SftpBrowserLoadRequest {
            file_browser_session_id: browser_session_id,
            session_id,
            path,
            request_id,
        })
    }

    pub fn pending_request(&self, session_id: Uuid) -> Option<SftpBrowserLoadRequest> {
        let browser_session_id = session_id.to_string();
        let state = self.sessions.get(&browser_session_id)?;
        let request_id = state.active_request_id?;
        if state.current_path.is_empty() {
            return None;
        }

        Some(SftpBrowserLoadRequest {
            file_browser_session_id: browser_session_id,
            session_id,
            path: state.current_path.clone(),
            request_id,
        })
    }

    pub fn pending_request_for_browser_session(
        &self,
        browser_session_id: &str,
        session_id: Uuid,
    ) -> Option<SftpBrowserLoadRequest> {
        let state = self.sessions.get(browser_session_id)?;
        let request_id = state.active_request_id?;
        if state.current_path.is_empty() {
            return None;
        }

        Some(SftpBrowserLoadRequest {
            file_browser_session_id: browser_session_id.to_string(),
            session_id,
            path: state.current_path.clone(),
            request_id,
        })
    }

    pub fn mark_disconnected(&mut self, session_id: Uuid) {
        if let Some(state) = self.sessions.get_mut(&session_id.to_string()) {
            state.mark_disconnected();
        }
    }

    pub fn mark_disconnected_browser_session(&mut self, browser_session_id: &str) {
        if let Some(state) = self.sessions.get_mut(browser_session_id) {
            state.mark_disconnected();
        }
    }

    pub fn apply_load_error(
        &mut self,
        session_id: Uuid,
        request_id: u64,
        path: &str,
        message: String,
    ) {
        let Some(state) = self.sessions.get_mut(&session_id.to_string()) else {
            return;
        };
        if !state.accepts_request(request_id) {
            return;
        }

        state.set_error(path, message);
    }

    pub fn apply_load_error_for_browser_session(
        &mut self,
        browser_session_id: &str,
        request_id: u64,
        path: &str,
        message: String,
    ) {
        let Some(state) = self.sessions.get_mut(browser_session_id) else {
            return;
        };
        if !state.accepts_request(request_id) {
            return;
        }

        state.set_error(path, message);
    }

    pub fn apply_loaded_directory(
        &mut self,
        session_id: Uuid,
        request_id: u64,
        path: &str,
        entries: Vec<SftpDirectoryEntry>,
    ) {
        let Some(state) = self.sessions.get_mut(&session_id.to_string()) else {
            return;
        };
        if !state.accepts_request(request_id) {
            return;
        }

        state.set_ready(path, entries);
    }

    pub fn session_state(&self, session_id: Uuid) -> Option<&SftpBrowserSessionState> {
        self.sessions.get(&session_id.to_string())
    }

    pub fn apply_loaded_directory_for_browser_session(
        &mut self,
        browser_session_id: &str,
        request_id: u64,
        path: &str,
        entries: Vec<SftpDirectoryEntry>,
    ) {
        let Some(state) = self.sessions.get_mut(browser_session_id) else {
            return;
        };
        if !state.accepts_request(request_id) {
            return;
        }

        state.set_ready(path, entries);
    }

    pub fn browser_session_state(&self, browser_session_id: &str) -> Option<&SftpBrowserSessionState> {
        self.sessions.get(browser_session_id)
    }

    pub fn mark_request_in_flight(&mut self, request_id: u64) -> bool {
        self.in_flight_request_ids.insert(request_id)
    }

    pub fn complete_request(&mut self, request_id: u64) {
        self.in_flight_request_ids.remove(&request_id);
    }

    fn new_request(
        &mut self,
        file_browser_session_id: String,
        session_id: Uuid,
        path: &str,
    ) -> SftpBrowserLoadRequest {
        let request_id = self.next_request_id();
        SftpBrowserLoadRequest {
            file_browser_session_id,
            session_id,
            path: path.to_string(),
            request_id,
        }
    }

    fn next_request_id(&mut self) -> u64 {
        self.next_request_id += 1;
        self.next_request_id
    }
}
