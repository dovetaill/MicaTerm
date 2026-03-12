use mica_term::app::runtime_profile::{AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_does_not_require_backend_lock() {
    let profile = AppRuntimeProfile::formal();

    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
}

#[test]
fn skia_experimental_profile_requires_winit_skia_software() {
    let profile = AppRuntimeProfile::skia_experimental();

    assert!(profile.requires_backend_lock());
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
    assert_eq!(profile.forced_backend(), Some("winit-skia-software"));
}
