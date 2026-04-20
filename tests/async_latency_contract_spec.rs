//! Source-level guards for async latency instrumentation on SSH/SFTP flows.

use std::fs;

#[test]
fn sftp_sources_expose_open_and_switch_latency_markers() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp source");
    let shell_view_model =
        fs::read_to_string("src/shell/view_model.rs").expect("read shell view model source");

    for expected in [
        "app.async_latency",
        "sftp-panel-open",
        "sftp-panel-switch",
        "ui-return",
        "result-drained",
        "browser-state-applied",
        "right-panel-sync-start",
        "right-panel-sync-finished",
        "request-finished",
    ] {
        assert!(
            bootstrap_sftp.contains(expected) || shell_view_model.contains(expected),
            "SFTP async latency instrumentation should expose `{expected}` so slow right-panel open/switch paths can be timed end-to-end instead of guessed from UI hitches"
        );
    }
}

#[test]
fn sftp_toggle_open_path_is_instrumented() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    for expected in [
        "on_toggle_right_panel_requested",
        "begin_sftp_panel_open_latency",
        "flow = \"sftp-panel-open\"",
        "stage = \"ui-return\"",
    ] {
        assert!(
            bootstrap_source.contains(expected),
            "the generic right-panel toggle path should expose `{expected}` so opening SFTP from the titlebar toggle emits the same latency flow as the explicit Open SFTP action"
        );
    }
}

#[test]
fn sftp_sources_recover_manager_runtime_before_blocking_fallback() {
    let bootstrap_sftp =
        fs::read_to_string("src/app/bootstrap/sftp.rs").expect("read bootstrap sftp source");

    assert!(
        bootstrap_sftp.contains("async_runtime")
            && bootstrap_sftp.contains("unwrap_or_else(|| manager.runtime_handle())"),
        "SFTP browser requests should recover the SessionManager runtime handle before considering any blocking fallback so a missing optional UI handle cannot freeze the main thread"
    );
}

#[test]
fn ssh_sources_expose_async_open_latency_markers() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let assets_source =
        fs::read_to_string("src/app/bootstrap/assets_keychain.rs").expect("read asset bootstrap");
    let session_manager_source =
        fs::read_to_string("src/app/ssh/session_manager.rs").expect("read session manager");

    for expected in [
        "app.async_latency",
        "ssh-open",
        "ui-return",
        "session-connected",
    ] {
        assert!(
            bootstrap_source.contains(expected)
                || assets_source.contains(expected)
                || session_manager_source.contains(expected),
            "SSH async latency instrumentation should expose `{expected}` so new-session launch can be measured from UI handoff through runtime connection establishment"
        );
    }
}

#[test]
fn ssh_modal_sources_expose_modal_open_latency_markers() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let assets_source =
        fs::read_to_string("src/app/bootstrap/assets_keychain.rs").expect("read asset bootstrap");
    let plan = fs::read_to_string("docs/plans/2026-04-20-async-latency-instrumentation.md")
        .expect("read async latency instrumentation plan");

    for expected in [
        "ssh-modal-connect",
        "ssh-modal-save-connect",
        "session-profile-built",
        "modal-confirmed",
        "secrets-persisted",
        "asset-catalog-saved",
        "session-dispatched",
        "ui-return",
    ] {
        assert!(
            bootstrap_source.contains(expected)
                || assets_source.contains(expected)
                || plan.contains(expected),
            "SSH modal latency instrumentation should expose `{expected}` so save/connect can be split into synchronous preparation, persistence, dispatch, and final UI-return stages"
        );
    }
}

#[test]
fn docs_plan_describes_async_latency_probe_points() {
    let plan = fs::read_to_string("docs/plans/2026-04-20-async-latency-instrumentation.md")
        .expect("read async latency instrumentation plan");

    for expected in [
        "SFTP panel open",
        "SFTP session switch",
        "SSH open",
        "app.async_latency",
        "ui-return",
        "result-drained",
        "browser-state-applied",
        "right-panel-sync-finished",
        "request-finished",
        "session-connected",
        "ssh-modal-connect",
        "ssh-modal-save-connect",
        "modal-confirmed",
        "session-dispatched",
    ] {
        assert!(
            plan.contains(expected),
            "the docs/plans note should mention `{expected}` so future profiling has one stable place describing how to read the new async latency probes"
        );
    }
}
