//! Runtime profile coverage for build flavor and renderer selection invariants.

use std::fs;

#[path = "support/retired_windows_subsystem.rs"]
mod retired_windows_subsystem;

use mica_term::app::runtime_profile::{
    AppBuildFlavor, AppRuntimeProfile, GraphicsApiRequirement, NativePresentPath, RendererMode,
};

#[test]
fn mainline_profile_describes_windows_skia_package() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.build_flavor, AppBuildFlavor::WindowsMainline);
    assert_eq!(profile.renderer_mode, RendererMode::Skia);
    assert_eq!(
        profile.native_present_path(),
        NativePresentPath::RenderingNotifier
    );
    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("skia"));
    assert!(profile.prefers_direct3d());
    assert_eq!(
        profile.preferred_graphics_api(),
        Some(GraphicsApiRequirement::Direct3D)
    );
    assert_eq!(
        profile.renderer_fallback_chain(),
        &[
            RendererMode::Skia,
            RendererMode::SkiaSoftware,
            RendererMode::Software,
        ]
    );
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
    assert!(content.contains("Skia"));
    assert!(content.contains("SkiaSoftware"));
    assert!(content.contains("Software"));
    assert!(content.contains("Some(\"skia\")"));
    assert!(content.contains("pub enum NativePresentPath"));
    assert!(content.contains("RenderingNotifier"));
    assert!(content.contains("EventLoop"));
    assert!(content.contains("pub fn native_present_path(self) -> NativePresentPath"));
    assert!(content.contains("pub fn native_present_path_label(self) -> &'static str"));
    assert!(content.contains("pub enum GraphicsApiRequirement"));
    assert!(
        content.contains("pub fn preferred_graphics_api(self) -> Option<GraphicsApiRequirement>")
    );
    assert!(content.contains("pub fn renderer_fallback_chain(self) -> &'static [RendererMode]"));
    assert!(content.contains("TerminalRenderMode::Bitmap"));
    assert!(content.contains("TerminalRenderMode::Native"));
    assert!(content.contains("Some(\"bitmap\")"));
    assert!(content.contains("Some(\"native\")"));
    assert!(content.contains("Self::mainline_native()"));
    assert!(content.contains("Self::software_compat()"));
    assert!(content.contains("pub fn prefers_direct3d(self) -> bool"));
    assert!(content.contains("pub fn prefers_native_terminal_renderer(self) -> bool"));
    assert!(!content.contains(&retired_windows_subsystem::retired_pascal_name()));
    assert!(!content.contains(&retired_windows_subsystem::retired_kebab_name()));
    assert!(!content.contains("MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM"));
    assert!(!content.contains("MICA_TERM_TERMINAL_SUBSYSTEM"));
    assert!(!content.contains("pub enum TerminalCompositionMode"));
    assert!(!content.contains("pub enum TerminalSubsystemMode"));
    assert!(!content.contains("pub fn terminal_composition_mode(self) -> TerminalCompositionMode"));
    assert!(!content.contains("pub fn terminal_subsystem_mode(self) -> TerminalSubsystemMode"));
    assert!(!content.contains("pub fn terminal_subsystem_mode_label(self) -> 'static str"));
    assert!(!content.contains(&retired_windows_subsystem::retired_subsystem_match_expr()));
    assert!(content.contains("native-first Windows software profile"));
}

#[test]
fn software_compat_profile_prefers_native_terminal_renderer() {
    let profile = AppRuntimeProfile::software_compat();

    assert_eq!(profile.build_flavor, AppBuildFlavor::WindowsSoftwareCompat);
    assert_eq!(profile.renderer_mode, RendererMode::Software);
    assert_eq!(
        profile.native_present_path(),
        NativePresentPath::RenderingNotifier
    );
    assert_eq!(profile.terminal_render_mode_label(), "native");
    assert_eq!(profile.native_present_path_label(), "rendering-notifier");
    assert!(profile.prefers_native_terminal_renderer());
    assert_eq!(profile.preferred_graphics_api(), None);
    assert_eq!(profile.renderer_fallback_chain(), &[RendererMode::Software]);
}

#[test]
fn mainline_profile_keeps_retained_native_terminal_settings() {
    let profile = AppRuntimeProfile::mainline();

    assert!(profile.prefers_native_terminal_renderer());
    assert_eq!(profile.terminal_render_mode_label(), "native");
    assert_eq!(profile.native_present_path_label(), "rendering-notifier");
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
        content.contains("assets/fonts/MiSans/LICENSE.txt"),
        "desktop packaging should copy the bundled MiSans license into the staged package"
    );
    assert!(
        content.contains("assets/fonts/SarasaTermSCNerd/LICENSE.txt"),
        "desktop packaging should copy the bundled Sarasa Term SC license into the staged package"
    );
    assert!(
        !content.contains("assets/fonts/JetBrainsMono/OFL.txt"),
        "desktop packaging should stop copying the retired JetBrains Mono OFL into the staged package"
    );
    assert!(
        !content.contains("assets/fonts/SarasaUiSC/LICENSE.txt"),
        "desktop packaging should stop copying the retired Sarasa UI SC license into the staged package"
    );
    assert!(
        content.contains("licenses/fonts"),
        "desktop packaging should stage bundled font licenses under a dedicated licenses/fonts tree"
    );
    assert!(
        content.contains("MiSans/LICENSE.txt"),
        "desktop packaging should preserve the MiSans license filename inside the staged font license bundle"
    );
}
