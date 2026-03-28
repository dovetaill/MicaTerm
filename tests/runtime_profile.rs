//! Runtime profile coverage for build flavor and renderer selection invariants.

use std::fs;

use mica_term::app::runtime_profile::{AppBuildFlavor, AppRuntimeProfile, RendererMode};

#[test]
fn mainline_profile_describes_windows_skia_package() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.build_flavor, AppBuildFlavor::WindowsMainline);
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("skia-software"));
}

#[test]
fn packaged_profile_defaults_to_development_software_without_build_env() {
    let profile = AppRuntimeProfile::packaged();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Development);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("software"));
}

#[test]
fn runtime_profile_source_exposes_packaged_env_contract() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(content.contains("pub fn packaged() -> Self"));
    assert!(content.contains("option_env!(\"MICA_TERM_BUILD_FLAVOR\")"));
    assert!(content.contains("option_env!(\"MICA_TERM_PACKAGE_RENDERER\")"));
    assert!(content.contains("WindowsSoftwareCompat"));
    assert!(content.contains("SkiaSoftware"));
    assert!(content.contains("Software"));
}

#[test]
fn cargo_manifest_exposes_software_and_skia_renderers() {
    let content = fs::read_to_string("Cargo.toml").expect("read cargo manifest");

    assert!(content.contains("default = [\"slint-renderer-software\"]"));
    assert!(content.contains("slint-renderer-software ="));
    assert!(content.contains("slint-renderer-skia ="));
    assert!(content.contains("renderer-software"));
    assert!(content.contains("renderer-skia"));
    assert!(content.contains("unstable-fontique-07"));
}
