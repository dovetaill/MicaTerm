use std::fs;
use std::sync::mpsc;
use std::time::Duration;

use mica_term::app::async_runtime::AppAsyncRuntime;

#[test]
fn app_async_runtime_can_spawn_and_complete_background_tasks() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let (tx, rx) = mpsc::channel();

    runtime.handle().spawn(async move {
        tx.send("done").expect("send completion");
    });

    assert_eq!(
        rx.recv_timeout(Duration::from_secs(1))
            .expect("receive completion"),
        "done"
    );
}

#[test]
fn app_async_runtime_exposes_handle_for_ssh_services() {
    let runtime = AppAsyncRuntime::new().expect("create app async runtime");
    let task = runtime.handle().spawn(async { String::from("ssh-ready") });

    let result = runtime.block_on(async { task.await.expect("join task") });

    assert_eq!(result, "ssh-ready");
}

#[test]
fn app_async_runtime_uses_bounded_worker_threads_for_mainline_profile() {
    let content = fs::read_to_string("src/app/async_runtime.rs").expect("read async runtime");

    assert!(
        content.contains("available_parallelism"),
        "AppAsyncRuntime should bound worker threads from available_parallelism"
    );
    assert!(
        content.contains("min(2)"),
        "AppAsyncRuntime should clamp worker threads to at most two for the mainline profile"
    );
}

#[test]
fn session_bridge_reuses_supplied_runtime_handle_instead_of_creating_another_runtime() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("fn build_session_bridge(\n    runtime_handle: tokio::runtime::Handle,"),
        "session bridge should accept the runtime handle created by main startup"
    );
    assert!(
        content.contains("SessionManager::new_with_launcher(\n            runtime_handle,"),
        "session manager should be built directly from the supplied runtime handle"
    );
    assert!(
        !content.contains("failed to create app async runtime for ssh session bridge"),
        "session bridge should no longer create a second AppAsyncRuntime internally"
    );
}

#[test]
fn terminal_native_clipboard_shortcuts_are_not_guarded_by_windows_only_stub() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        !content.contains(
            "#[cfg(not(target_os = \"windows\"))]\nfn bind_windows_window_state_tracking("
        ),
        "terminal clipboard shortcut fallback should not become a non-Windows no-op"
    );
    assert!(
        content.contains("window()\n        .on_winit_window_event(move |_slint_window, event| {"),
        "bootstrap should register winit keyboard handling for terminal clipboard shortcuts"
    );
}
