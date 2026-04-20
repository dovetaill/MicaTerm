//! Guards against reintroducing one-off async latency probe logging after the
//! investigation has been completed.

use std::fs;
use std::path::Path;

#[test]
fn repo_no_longer_contains_async_latency_probe_markers() {
    let paths = [
        "src/app/bootstrap.rs",
        "src/app/bootstrap/sftp.rs",
        "src/app/bootstrap/assets_keychain.rs",
        "src/app/ssh/session_manager.rs",
        "src/shell/view_model.rs",
        "src/shell/view_model/sftp.rs",
        "docs/plans/2026-04-20-async-latency-instrumentation.md",
        "docs/plans/2026-04-20-sftp-main-thread-latency-probes-implementation-plan.md",
        "docs/plans/2026-04-20-ssh-modal-open-latency-probes-implementation-plan.md",
        "docs/plans/2026-04-20-sftp-right-panel-virtualization-design.md",
        "tests/async_latency_contract_spec.rs",
    ];

    for path in paths {
        if !Path::new(path).exists() {
            continue;
        }

        let source = fs::read_to_string(path).unwrap_or_else(|err| {
            panic!("failed to read `{path}` while checking probe cleanup: {err}");
        });

        for marker in [
            "app.async_latency",
            "log_ssh_async_latency",
            "log_ssh_modal_latency",
            "log_sftp_async_latency",
            "sftp-panel-open",
            "sftp-panel-switch",
            "ssh-open",
            "ssh-modal-connect",
            "ssh-modal-save-connect",
        ] {
            assert!(
                !source.contains(marker),
                "`{path}` should not retain temporary async latency probe marker `{marker}` once the logging investigation is complete"
            );
        }
    }
}
