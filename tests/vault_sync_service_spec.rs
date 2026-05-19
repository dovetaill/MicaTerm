use mica_term::app::vault::model::GitRemoteSafetyStatus;
use mica_term::app::vault::sync_service::{
    VaultSyncExecution, VaultSyncIntent, VaultSyncPlan, VaultSyncService, VaultSyncServiceConfig,
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
        Some(VaultSyncPlan {
            execution: VaultSyncExecution::Push,
            revalidate_remote: true,
        })
    );

    service.finish(VaultSyncExecution::Push, true);

    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Periodic, true, false),
        Some(VaultSyncPlan {
            execution: VaultSyncExecution::Refresh,
            revalidate_remote: true,
        })
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

#[test]
fn opening_sync_modal_refreshes_remote_head_without_blocking() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.mark_dirty();
    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Manual, true, false),
        Some(VaultSyncPlan {
            execution: VaultSyncExecution::Push,
            revalidate_remote: true,
        })
    );

    assert!(service.request(VaultSyncIntent::RefreshRemoteHead));
    assert!(!service.request(VaultSyncIntent::RefreshRemoteHead));
}

#[test]
fn remote_changed_to_public_pauses_sync() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.mark_dirty();
    let plan = service
        .begin_trigger(VaultSyncTrigger::Manual, true, false)
        .expect("manual sync should attempt a push while dirty");
    assert_eq!(plan.execution, VaultSyncExecution::Push);
    assert!(plan.revalidate_remote);

    service.pause_remote(
        "remote repository `owner/repo` must stay private before sync can be enabled",
    );
    service.finish(plan.execution, false);

    let state = service.runtime_state();
    assert_eq!(state.remote_safety_status, GitRemoteSafetyStatus::Paused);
    assert_eq!(
        state.last_sync_error.as_deref(),
        Some("remote repository `owner/repo` must stay private before sync can be enabled")
    );
}

#[test]
fn local_mutation_does_not_push_to_unsafe_remote() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.pause_remote("remote repository is paused");
    service.mark_dirty();

    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::DebouncedAuto, true, false),
        None
    );
    assert!(service.runtime_state().dirty);
}

#[test]
fn manual_sync_fails_closed_when_visibility_cannot_be_checked() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.mark_dirty();
    service.set_remote_safety_status(GitRemoteSafetyStatus::Stale);

    let plan = service
        .begin_trigger(VaultSyncTrigger::Manual, true, false)
        .expect("manual sync should schedule revalidation");
    assert_eq!(
        plan,
        VaultSyncPlan {
            execution: VaultSyncExecution::Push,
            revalidate_remote: true,
        }
    );

    service.pause_remote(
        "remote repository visibility could not be confirmed; refusing to enable sync without verified private visibility",
    );
    service.finish(plan.execution, false);

    let state = service.runtime_state();
    assert_eq!(state.remote_safety_status, GitRemoteSafetyStatus::Paused);
    assert!(
        state
            .last_sync_error
            .as_deref()
            .is_some_and(|message| message.contains("visibility could not be confirmed"))
    );
}

#[test]
fn periodic_sync_retries_after_remote_revalidated_private() {
    let service = VaultSyncService::new(VaultSyncServiceConfig::default());

    service.pause_remote(
        "remote repository `owner/repo` must stay private before sync can be enabled",
    );
    service.mark_dirty();

    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Periodic, true, false),
        Some(VaultSyncPlan {
            execution: VaultSyncExecution::Refresh,
            revalidate_remote: true,
        })
    );
    service.set_remote_safety_status(GitRemoteSafetyStatus::Safe);
    service.clear_last_sync_error();
    service.finish(VaultSyncExecution::Refresh, true);

    assert_eq!(
        service.begin_trigger(VaultSyncTrigger::Periodic, true, false),
        Some(VaultSyncPlan {
            execution: VaultSyncExecution::Push,
            revalidate_remote: true,
        })
    );
}
