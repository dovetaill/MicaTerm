use std::fs;

use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn formal_profile_stays_on_software_renderer() {
    let profile = AppRuntimeProfile::formal();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Formal);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert!(!profile.is_experimental());
    assert_eq!(profile.forced_backend(), None);
    assert_eq!(profile.forced_renderer(), None);
    assert!(!profile.requires_wgpu_28());
}

#[test]
fn femtovg_wgpu_experimental_profile_is_gpu_only() {
    let profile = AppRuntimeProfile::femtovg_wgpu_experimental();

    assert_eq!(
        profile.build_flavor,
        AppBuildFlavor::FemtoVgWgpuExperimental
    );
    assert_eq!(profile.renderer_mode, RendererMode::FemtoVgWgpu);
    assert!(profile.is_experimental());
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}

#[test]
fn runtime_profile_source_no_longer_exposes_skia_experimental_path() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(!content.contains("SkiaExperimental"));
    assert!(!content.contains("SkiaSoftware"));
    assert!(!content.contains("skia_experimental"));
    assert!(!content.contains("winit-skia-software"));
}
