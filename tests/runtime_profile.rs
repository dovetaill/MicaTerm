//! Runtime profile coverage for build flavor and renderer selection invariants.

use std::fs;

use mica_term::app::runtime_profile::{
    AppBuildFlavor, AppRuntimeProfile, NativePresentPath, RendererMode,
    TerminalCompositionMode,
};

#[test]
fn mainline_profile_describes_windows_skia_package() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.build_flavor, AppBuildFlavor::WindowsMainline);
    assert_eq!(profile.renderer_mode, RendererMode::SkiaSoftware);
    assert_eq!(profile.native_present_path(), NativePresentPath::RenderingNotifier);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("skia-software"));
    assert_eq!(profile.native_present_path_label(), "rendering-notifier");
}

#[test]
fn packaged_profile_defaults_to_development_software_without_build_env() {
    let profile = AppRuntimeProfile::packaged();

    assert_eq!(profile.build_flavor, AppBuildFlavor::Development);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(profile.native_present_path(), NativePresentPath::EventLoop);
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("software"));
    assert_eq!(profile.terminal_render_mode_label(), "native");
    assert_eq!(profile.native_present_path_label(), "event-loop");
}

#[test]
fn runtime_profile_source_exposes_packaged_env_contract() {
    let content = fs::read_to_string("src/app/runtime_profile.rs").expect("read runtime profile");

    assert!(content.contains("pub fn packaged() -> Self"));
    assert!(content.contains("option_env!(\"MICA_TERM_BUILD_FLAVOR\")"));
    assert!(content.contains("option_env!(\"MICA_TERM_PACKAGE_RENDERER\")"));
    assert!(content.contains("option_env!(\"MICA_TERM_PACKAGE_TERMINAL_RENDERER\")"));
    assert!(content.contains("option_env!(\"MICA_TERM_PACKAGE_NATIVE_PRESENT_PATH\")"));
    assert!(content.contains("WindowsSoftwareCompat"));
    assert!(content.contains("SkiaSoftware"));
    assert!(content.contains("Software"));
    assert!(content.contains("pub enum NativePresentPath"));
    assert!(content.contains("pub enum TerminalCompositionMode"));
    assert!(content.contains("SceneImage"));
    assert!(content.contains("PostRenderNativeSurface"));
    assert!(content.contains("RenderingNotifier"));
    assert!(content.contains("EventLoop"));
    assert!(content.contains("pub fn native_present_path(self) -> NativePresentPath"));
    assert!(
        content.contains("pub fn terminal_composition_mode(self) -> TerminalCompositionMode")
    );
    assert!(content.contains("pub fn native_present_path_label(self) -> &'static str"));
    assert!(content.contains("TerminalRenderMode::Bitmap"));
    assert!(content.contains("TerminalRenderMode::Native"));
    assert!(content.contains("Some(\"bitmap\")"));
    assert!(content.contains("Some(\"native\")"));
    assert!(content.contains("Self::mainline_native()"));
    assert!(content.contains("Self::software_compat()"));
    assert!(content.contains("Preferred native-only shipping profile"));
    assert!(content.contains("native-first Windows software profile"));
}

#[test]
fn software_compat_profile_prefers_native_terminal_renderer() {
    let profile = AppRuntimeProfile::software_compat();

    assert_eq!(profile.build_flavor, AppBuildFlavor::WindowsSoftwareCompat);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(profile.native_present_path(), NativePresentPath::RenderingNotifier);
    assert_eq!(profile.terminal_render_mode_label(), "native");
    assert_eq!(profile.native_present_path_label(), "rendering-notifier");
    assert_eq!(
        profile.terminal_composition_mode(),
        TerminalCompositionMode::SceneImage
    );
    assert!(profile.prefers_native_terminal_renderer());
}

#[test]
fn mainline_profile_keeps_post_render_native_surface_contract() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(
        profile.terminal_composition_mode(),
        TerminalCompositionMode::PostRenderNativeSurface
    );
}

#[test]
fn cargo_manifest_exposes_software_and_skia_renderers() {
    let content = fs::read_to_string("Cargo.toml").expect("read cargo manifest");

    assert!(
        content.contains("default = [\"slint-renderer-software\", \"terminal-native-renderer\"]")
    );
    assert!(content.contains("slint-renderer-software ="));
    assert!(content.contains("slint-renderer-skia ="));
    assert!(content.contains("terminal-native-renderer = [\"dep:rustybuzz\"]"));
    assert!(content.contains("rustybuzz = { version = \"0.20.1\", optional = true }"));
    assert!(
        !content.contains("harfbuzz_rs ="),
        "native terminal shaping should stop depending on the C harfbuzz binding so the repo can move toward a WezTerm-style pure-Rust shaping boundary"
    );
    assert!(content.contains("renderer-software"));
    assert!(content.contains("renderer-skia"));
    assert!(content.contains("unstable-fontique-07"));
}

#[test]
fn desktop_packaging_script_copies_default_terminal_font_license() {
    let content = fs::read_to_string("build-desktop.sh").expect("read build-desktop script");

    assert!(
        content.contains("assets/fonts/Fusion-JetBrainsMapleMono/OFL.txt"),
        "desktop packaging should copy the bundled Fusion-JetBrainsMapleMono OFL into the staged package"
    );
    assert!(
        content.contains("OFL.txt"),
        "desktop packaging should preserve the upstream OFL filename in packaged artifacts"
    );
}
