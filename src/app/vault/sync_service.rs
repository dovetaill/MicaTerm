use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::app::vault::model::GitRemoteSafetyStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultSyncIntent {
    ManualSync,
    LocalMutation,
    PeriodicRefresh,
    RefreshRemoteHead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultSyncTrigger {
    Manual,
    DebouncedAuto,
    Periodic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultSyncExecution {
    Push,
    Refresh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultSyncPlan {
    pub execution: VaultSyncExecution,
    pub revalidate_remote: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RemoteHeadSnapshot {
    pub revision: Option<String>,
    pub committed_at: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

#[derive(Debug, Default)]
struct VaultSyncState {
    dirty: bool,
    running: bool,
    pending_trigger: Option<VaultSyncTrigger>,
    base_revision: Option<String>,
    local_snapshot_hash: Option<String>,
    last_local_change_at: Option<String>,
    last_successful_push_at: Option<String>,
    last_successful_pull_at: Option<String>,
    last_sync_error: Option<String>,
    remote_safety_status: GitRemoteSafetyStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VaultSyncRuntimeState {
    pub dirty: bool,
    pub running: bool,
    pub base_revision: Option<String>,
    pub local_snapshot_hash: Option<String>,
    pub last_local_change_at: Option<String>,
    pub last_successful_push_at: Option<String>,
    pub last_successful_pull_at: Option<String>,
    pub last_sync_error: Option<String>,
    pub remote_safety_status: GitRemoteSafetyStatus,
}

#[derive(Clone, Default)]
pub struct VaultSyncServiceConfig {
    runtime_handle: Option<tokio::runtime::Handle>,
}

impl VaultSyncServiceConfig {
    pub fn with_runtime_handle(mut self, runtime_handle: Option<tokio::runtime::Handle>) -> Self {
        self.runtime_handle = runtime_handle;
        self
    }
}

pub struct VaultSyncService {
    runtime_handle: Option<tokio::runtime::Handle>,
    remote_head_refresh_in_flight: AtomicBool,
    state: Mutex<VaultSyncState>,
}

pub struct VaultSyncBackgroundTask {
    completion_rx: std::sync::mpsc::Receiver<()>,
}

impl VaultSyncBackgroundTask {
    pub fn await_completion(
        self,
        timeout: Duration,
    ) -> std::result::Result<(), std::sync::mpsc::RecvTimeoutError> {
        self.completion_rx.recv_timeout(timeout)
    }
}

impl VaultSyncService {
    pub fn new(config: VaultSyncServiceConfig) -> Self {
        Self {
            runtime_handle: config.runtime_handle,
            remote_head_refresh_in_flight: AtomicBool::new(false),
            state: Mutex::new(VaultSyncState::default()),
        }
    }

    pub fn request(&self, intent: VaultSyncIntent) -> bool {
        match intent {
            VaultSyncIntent::LocalMutation => {
                let mut state = self
                    .state
                    .lock()
                    .expect("vault sync state mutex should not be poisoned");
                state.dirty = true;
                true
            }
            VaultSyncIntent::RefreshRemoteHead => self
                .remote_head_refresh_in_flight
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok(),
            VaultSyncIntent::ManualSync | VaultSyncIntent::PeriodicRefresh => true,
        }
    }

    pub fn can_run_in_background(&self) -> bool {
        self.runtime_handle.is_some()
    }

    pub fn mark_dirty(&self) {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.dirty = true;
        state.last_local_change_at = state.last_local_change_at.clone();
    }

    pub fn runtime_state(&self) -> VaultSyncRuntimeState {
        let state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        VaultSyncRuntimeState {
            dirty: state.dirty,
            running: state.running,
            base_revision: state.base_revision.clone(),
            local_snapshot_hash: state.local_snapshot_hash.clone(),
            last_local_change_at: state.last_local_change_at.clone(),
            last_successful_push_at: state.last_successful_push_at.clone(),
            last_successful_pull_at: state.last_successful_pull_at.clone(),
            last_sync_error: state.last_sync_error.clone(),
            remote_safety_status: state.remote_safety_status,
        }
    }

    pub fn set_remote_safety_status(&self, safety_status: GitRemoteSafetyStatus) {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.remote_safety_status = safety_status;
    }

    pub fn clear_last_sync_error(&self) {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.last_sync_error = None;
    }

    pub fn pause_remote(&self, error: impl Into<String>) {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.remote_safety_status = GitRemoteSafetyStatus::Paused;
        state.last_sync_error = Some(error.into());
    }

    pub fn begin_trigger(
        &self,
        trigger: VaultSyncTrigger,
        background_ready: bool,
        requires_initial_remote_sync: bool,
    ) -> Option<VaultSyncPlan> {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        if state.running {
            state.pending_trigger = Some(merge_vault_sync_trigger(state.pending_trigger, trigger));
            return None;
        }

        let should_attempt_push = state.dirty
            || (matches!(trigger, VaultSyncTrigger::Manual) && requires_initial_remote_sync);
        let should_attempt_refresh = !should_attempt_push
            && matches!(
                trigger,
                VaultSyncTrigger::Manual | VaultSyncTrigger::Periodic
            );

        if matches!(
            trigger,
            VaultSyncTrigger::DebouncedAuto | VaultSyncTrigger::Periodic
        ) && !background_ready
        {
            return None;
        }

        if matches!(trigger, VaultSyncTrigger::DebouncedAuto) && !should_attempt_push {
            return None;
        }

        let plan = if should_attempt_push {
            if state.remote_safety_status == GitRemoteSafetyStatus::Paused {
                if matches!(trigger, VaultSyncTrigger::DebouncedAuto) {
                    return None;
                }
                VaultSyncPlan {
                    execution: VaultSyncExecution::Refresh,
                    revalidate_remote: true,
                }
            } else {
                VaultSyncPlan {
                    execution: VaultSyncExecution::Push,
                    revalidate_remote: true,
                }
            }
        } else if should_attempt_refresh {
            VaultSyncPlan {
                execution: VaultSyncExecution::Refresh,
                revalidate_remote: true,
            }
        } else {
            return None;
        };

        state.running = true;
        Some(plan)
    }

    pub fn finish(&self, execution: VaultSyncExecution, success: bool) -> Option<VaultSyncTrigger> {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.running = false;
        if success && matches!(execution, VaultSyncExecution::Push) {
            state.dirty = false;
        }
        state.pending_trigger.take()
    }

    pub fn finish_remote_head_refresh(&self) {
        self.remote_head_refresh_in_flight
            .store(false, Ordering::Release);
    }

    pub fn spawn_blocking<F>(&self, work: F) -> Option<VaultSyncBackgroundTask>
    where
        F: FnOnce() + Send + 'static,
    {
        let runtime_handle = self.runtime_handle.clone()?;
        let (completion_tx, completion_rx) = std::sync::mpsc::channel();

        runtime_handle.spawn(async move {
            let _ = tokio::task::spawn_blocking(work).await;
            let _ = completion_tx.send(());
        });

        Some(VaultSyncBackgroundTask { completion_rx })
    }
}

fn merge_vault_sync_trigger(
    pending: Option<VaultSyncTrigger>,
    next: VaultSyncTrigger,
) -> VaultSyncTrigger {
    match (pending, next) {
        (Some(VaultSyncTrigger::Manual), _) | (_, VaultSyncTrigger::Manual) => {
            VaultSyncTrigger::Manual
        }
        (Some(VaultSyncTrigger::Periodic), _) | (_, VaultSyncTrigger::Periodic) => {
            VaultSyncTrigger::Periodic
        }
        _ => VaultSyncTrigger::DebouncedAuto,
    }
}
