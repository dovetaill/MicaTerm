use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_defaults_to_software_renderer() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.is_experimental());
}

#[test]
fn skia_experimental_profile_is_pure_skia() {
    let profile = AppRuntimeProfile::skia_experimental();

    assert_eq!(profile.build_flavor, AppBuildFlavor::SkiaExperimental);
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
    assert!(profile.is_experimental());
    assert!(profile.requires_backend_lock());
}
