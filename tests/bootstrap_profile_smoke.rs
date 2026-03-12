use mica_term::app::runtime_profile::{AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_does_not_require_backend_lock() {
    let profile = AppRuntimeProfile::formal();

    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
}

#[test]
fn formal_profile_is_the_only_bootstrap_runtime_path() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(
        profile.build_flavor,
        mica_term::app::runtime_profile::AppBuildFlavor::Formal
    );
    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
}
