use std::sync::mpsc::Sender;

use uuid::Uuid;

use crate::app::sftp::{SftpBrowserLoadRequest, SftpDirectoryEntry, SftpPanelMode};
use crate::app::ssh::session_manager::{SessionManager, SessionState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpOperationKind {
    LoadDir,
    DownloadAndOpen,
    PrepareEditWorkingCopy,
    UploadWorkingCopy,
    RenameEntry,
    DeleteEntries,
    CreateFolder,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SftpOperationToken {
    pub browser_session_id: String,
    pub generation: u64,
    pub operation_id: u64,
}

#[derive(Debug)]
pub struct SftpBrowserOperationResult {
    pub kind: SftpOperationKind,
    pub request: SftpBrowserLoadRequest,
    pub result: Result<Vec<SftpDirectoryEntry>, String>,
    pub disconnected: bool,
}

pub fn dispatch_sftp_load_dir_operation(
    runtime_handle: &tokio::runtime::Handle,
    manager: SessionManager,
    request: SftpBrowserLoadRequest,
    result_tx: Sender<SftpBrowserOperationResult>,
) {
    runtime_handle.spawn(async move {
        let (result, disconnected) = load_sftp_directory_result(
            manager,
            request.session_id,
            request.path.clone(),
        )
        .await;
        let _ = result_tx.send(SftpBrowserOperationResult {
            kind: SftpOperationKind::LoadDir,
            request,
            result,
            disconnected,
        });
    });
}

async fn load_sftp_directory_result(
    manager: SessionManager,
    session_id: Uuid,
    path: String,
) -> (Result<Vec<SftpDirectoryEntry>, String>, bool) {
    const MAX_RECONNECT_WAIT_STEPS: usize = 100;
    const RECONNECT_WAIT_STEP: std::time::Duration = std::time::Duration::from_millis(10);

    let mut wait_steps = 0usize;
    loop {
        let binding = manager.sftp_binding(session_id);
        if binding
            .as_ref()
            .is_some_and(|binding| binding.mode() != SftpPanelMode::Disconnected)
        {
            let result = manager
                .sftp_read_dir_async(session_id, path.as_str())
                .await
                .map_err(|err| err.to_string());
            let disconnected = manager
                .sftp_binding(session_id)
                .is_none_or(|binding| binding.mode() == SftpPanelMode::Disconnected);
            return (result, disconnected);
        }

        let session_state = manager.session(session_id).map(|session| session.state);
        let should_wait_for_reconnect = matches!(
            session_state,
            Some(SessionState::Connecting | SessionState::Connected | SessionState::WaitingUser)
        );
        if !should_wait_for_reconnect || wait_steps >= MAX_RECONNECT_WAIT_STEPS {
            return (Err("sftp session disconnected".to_string()), true);
        }

        wait_steps += 1;
        tokio::time::sleep(RECONNECT_WAIT_STEP).await;
    }
}
