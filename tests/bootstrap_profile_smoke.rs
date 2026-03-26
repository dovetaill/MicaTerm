//! Smoke coverage for the selected runtime profile contract.

use std::fs;

use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn mainline_profile_requests_internal_selector_lock() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}

#[test]
fn run_with_profile_accepts_external_async_handle_for_ssh_services() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("pub fn run_with_profile(\n    profile: AppRuntimeProfile,\n    async_runtime_handle: tokio::runtime::Handle,"),
        "run_with_profile should explicitly accept the async runtime handle used by shell services"
    );
    assert!(
        content.contains(
            "let session_bridge =\n                build_session_bridge(async_runtime_handle,"
        ),
        "run_with_profile should thread the supplied runtime handle into session bridge construction"
    );
    assert!(
        !content.contains("_async_runtime_handle"),
        "the runtime handle should be consumed for ssh services instead of being ignored"
    );
}
