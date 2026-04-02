use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

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
    }

    pub fn begin_trigger(
        &self,
        trigger: VaultSyncTrigger,
        background_ready: bool,
        requires_initial_remote_sync: bool,
    ) -> Option<VaultSyncExecution> {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        if state.running {
            return None;
        }

        let should_attempt_push =
            state.dirty || (matches!(trigger, VaultSyncTrigger::Manual) && requires_initial_remote_sync);
        let should_attempt_refresh = !should_attempt_push
            && matches!(trigger, VaultSyncTrigger::Manual | VaultSyncTrigger::Periodic);

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

        let execution = if should_attempt_push {
            VaultSyncExecution::Push
        } else if should_attempt_refresh {
            VaultSyncExecution::Refresh
        } else {
            return None;
        };

        state.running = true;
        Some(execution)
    }

    pub fn finish(&self, execution: VaultSyncExecution, success: bool) {
        let mut state = self
            .state
            .lock()
            .expect("vault sync state mutex should not be poisoned");
        state.running = false;
        if success && matches!(execution, VaultSyncExecution::Push) {
            state.dirty = false;
        }
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
