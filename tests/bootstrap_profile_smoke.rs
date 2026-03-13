use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn formal_profile_does_not_request_selector_lock() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.forced_backend(), None);
    assert_eq!(profile.forced_renderer(), None);
    assert!(!profile.requires_wgpu_28());
}

#[test]
fn experimental_profile_requests_internal_selector_lock() {
    let profile = AppRuntimeProfile::femtovg_wgpu_experimental();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}
