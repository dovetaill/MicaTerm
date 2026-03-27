//! Runtime profile coverage for build flavor and renderer selection invariants.

use std::fs;

use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn mainline_profile_is_software_only() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Mainline);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("software"));
}

#[test]
fn runtime_profile_source_no_longer_exposes_gpu_mainline_assumptions() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(!content.contains("formal("));
    assert!(content.contains("Software"));
    assert!(!content.contains("FemtoVgWgpu"));
    assert!(!content.contains("femtovg-wgpu"));
    assert!(!content.contains("requires_wgpu_28"));
}

#[test]
fn cargo_manifest_restores_software_renderer_default() {
    let content = fs::read_to_string("Cargo.toml").expect("read cargo manifest");

    assert!(content.contains("default = [\"slint-renderer-software\"]"));
    assert!(content.contains("slint-renderer-software ="));
    assert!(content.contains("renderer-software"));
    assert!(!content.contains("slint-renderer-femtovg-wgpu ="));
    assert!(!content.contains("unstable-wgpu-28"));
}
