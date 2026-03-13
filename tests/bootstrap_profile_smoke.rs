use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn mainline_profile_requests_internal_selector_lock() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}
