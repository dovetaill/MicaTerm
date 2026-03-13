use std::fs;

use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn mainline_profile_is_gpu_only() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Mainline);
    assert_eq!(profile.renderer_mode, RendererMode::FemtoVgWgpu);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("femtovg-wgpu"));
    assert!(profile.requires_wgpu_28());
}

#[test]
fn runtime_profile_source_no_longer_exposes_formal_software_or_experimental_split() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(!content.contains("formal("));
    assert!(!content.contains("Software"));
    assert!(!content.contains("FemtoVgWgpuExperimental"));
    assert!(!content.contains("femtovg_wgpu_experimental"));
}

#[test]
fn cargo_manifest_no_longer_exposes_software_renderer_feature_toggle() {
    let content = fs::read_to_string("Cargo.toml").expect("read cargo manifest");

    assert!(!content.contains("slint-renderer-software ="));
    assert!(!content.contains("renderer-software"));
}
