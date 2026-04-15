//! File-browser session identities shared by quick browser and workspace tabs.

use std::sync::atomic::{AtomicU64, Ordering};

pub type FileBrowserSessionId = String;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileBrowserSession {
    pub file_browser_session_id: FileBrowserSessionId,
    pub host_profile_ref: HostProfileRef,
    pub current_path: String,
    pub selected_entry_ids: Vec<String>,
}

impl FileBrowserSession {
    pub fn quick_browser(
        host_profile_ref: HostProfileRef,
        current_path: impl Into<String>,
    ) -> Self {
        Self {
            file_browser_session_id: new_file_browser_session_id(),
            host_profile_ref,
            current_path: current_path.into(),
            selected_entry_ids: Vec::new(),
        }
    }

    pub fn clone_for_workspace(&self) -> Self {
        Self {
            file_browser_session_id: new_file_browser_session_id(),
            host_profile_ref: self.host_profile_ref.clone(),
            current_path: self.current_path.clone(),
            selected_entry_ids: Vec::new(),
        }
    }
}

fn new_file_browser_session_id() -> FileBrowserSessionId {
    static NEXT_BROWSER_SESSION_ID: AtomicU64 = AtomicU64::new(1);

    format!(
        "browser-session-{}",
        NEXT_BROWSER_SESSION_ID.fetch_add(1, Ordering::Relaxed)
    )
}
