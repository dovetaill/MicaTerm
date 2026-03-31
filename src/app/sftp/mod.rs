//! Pure SFTP domain state used by later runtime and UI integration tasks.

pub mod browser_state;
pub mod browser_controller;
pub mod local_ops;
pub mod model;
pub mod queue;
pub mod runtime;
pub mod session_binding;

pub use browser_state::SftpBrowserSessionState;
pub use browser_controller::{
    SftpBrowserController, SftpBrowserLoadRequest,
};
pub use local_ops::{
    LocalTransferEntry, build_local_download_path, build_remote_upload_path, scan_local_sources,
};
pub use model::{
    SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode, SftpPathHistory,
    SftpSessionBindingState,
};
pub use queue::{
    TransferConflictPolicy, TransferDirection, TransferQueue, TransferQueueSummary, TransferTask,
    TransferTaskAction, TransferTaskState,
};
pub use runtime::{SftpBackend, SftpOperationFuture, SftpRuntimeHandle};
pub use session_binding::{
    SftpSessionBinding, delete_entries, execute_queued_transfers, move_entry_between_directories,
};
