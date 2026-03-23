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
