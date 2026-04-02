use mica_term::app::vault::sync_service::{
    VaultSyncExecution, VaultSyncIntent, VaultSyncService, VaultSyncServiceConfig,
    VaultSyncTrigger,
};
use std::time::Duration;

#[test]
fn service_coalesces_duplicate_remote_head_refresh_requests() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    assert!(service.request(VaultSyncIntent::RefreshRemoteHead));
    assert!(!service.request(VaultSyncIntent::RefreshRemoteHead));
}

#[test]
fn service_allows_remote_head_refresh_after_previous_refresh_finishes() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    assert!(service.request(VaultSyncIntent::RefreshRemoteHead));
    service.finish_remote_head_refresh();
    assert!(service.request(VaultSyncIntent::RefreshRemoteHead));
}

#[test]
fn service_tracks_dirty_state_without_dropping_manual_requests() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    assert!(service.request(VaultSyncIntent::LocalMutation));
    assert!(service.request(VaultSyncIntent::ManualSync));
}

#[test]
fn service_background_mode_uses_explicit_runtime_handle_even_without_session_runtime_guard() {
    let runtime =
        mica_term::app::async_runtime::AppAsyncRuntime::new().expect("create app async runtime");
    let service = VaultSyncService::new(
        VaultSyncServiceConfig::default().with_runtime_handle(Some(runtime.handle())),
    );

    assert!(service.can_run_in_background());
}

#[test]
fn service_promotes_dirty_periodic_sync_from_push_to_refresh_after_success() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.mark_dirty();
    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Periodic, true, false),
        Some(VaultSyncExecution::Push)
    );

    service.finish(VaultSyncExecution::Push, true);

    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Periodic, true, false),
        Some(VaultSyncExecution::Refresh)
    );
}

#[test]
fn service_runs_blocking_work_on_explicit_runtime_handle() {
    let runtime =
        mica_term::app::async_runtime::AppAsyncRuntime::new().expect("create app async runtime");
    let service = VaultSyncService::new(
        VaultSyncServiceConfig::default().with_runtime_handle(Some(runtime.handle())),
    );
    let (tx, rx) = std::sync::mpsc::channel();

    let join = service
        .spawn_blocking(move || {
            tx.send("done").expect("send background result");
        })
        .expect("spawn blocking work on explicit runtime");

    join.await_completion(Duration::from_secs(1))
        .expect("background task should finish");
    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("receive background result"),
        "done"
    );
}
