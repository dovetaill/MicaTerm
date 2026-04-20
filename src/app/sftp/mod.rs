//! Pure SFTP domain state used by later runtime and UI integration tasks.

pub mod browser_controller;
pub mod browser_session;
pub mod browser_state;
pub mod local_open;
pub mod local_ops;
pub mod model;
pub mod operation_dispatch;
pub mod queue;
pub mod runtime;
pub mod session_binding;
pub mod transfer_store;
pub mod working_copy;

pub use browser_controller::{SftpBrowserController, SftpBrowserLoadRequest};
pub use browser_session::{
    FILE_BROWSER_MODIFIED_COLUMN_MIN_PX, FILE_BROWSER_SIZE_COLUMN_MIN_PX,
    FILE_BROWSER_TYPE_COLUMN_MIN_PX, FileBrowserColumnLayout, FileBrowserSession,
    FileBrowserSessionId, FileBrowserSortColumn, FileBrowserSortDirection, FileBrowserSortState,
    HostProfileRef,
};
pub use browser_state::SftpBrowserSessionState;
pub use local_open::{
    SftpOpenAction, can_open_file_path_locally, can_open_folder_path_locally,
    open_path_in_folder_locally, open_path_locally, prepare_local_open_path, reveal_path_locally,
    trash_path_locally,
};
pub use local_ops::{
    LocalTransferEntry, build_local_download_path, build_remote_upload_path, scan_local_sources,
};
pub use model::{
    SftpDirectoryEntry, SftpDirectoryEntryKind, SftpFollowMode, SftpPanelMode, SftpPathHistory,
    SftpSessionBindingState,
};
pub use operation_dispatch::{
    SftpBrowserOperationResult, SftpOperationKind, SftpOperationToken,
    dispatch_sftp_load_dir_operation,
};
pub use queue::{
    DownloadTransferEntry, TransferConflictPolicy, TransferDirection, TransferQueue,
    TransferQueueSummary, TransferResumeMode, TransferTask, TransferTaskAction,
    TransferTaskState, download_part_path,
};
pub use runtime::{SftpBackend, SftpOperationFuture, SftpRuntimeHandle};
pub use session_binding::{
    SftpSessionBinding, collect_download_targets, delete_entries, execute_queued_transfers,
    execute_queued_transfers_with_progress, move_entry_between_directories,
};
pub use transfer_store::RedbTransferStore;
pub use working_copy::{
    SftpWorkingCopy, WorkingCopySnapshot, snapshot_working_copy, working_copy_has_changed,
};
