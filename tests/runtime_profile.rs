use std::fs;

use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_defaults_to_software_renderer() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.is_experimental());
    assert!(!profile.requires_backend_lock());
    assert_eq!(profile.forced_backend(), None);
}

#[test]
fn runtime_profile_source_no_longer_exposes_skia_experimental_path() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(!content.contains("SkiaExperimental"));
    assert!(!content.contains("SkiaSoftware"));
    assert!(!content.contains("skia_experimental"));
}
