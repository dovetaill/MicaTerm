//! Smoke coverage for the selected runtime profile contract.

use std::fs;

use mica_term::app::runtime_profile::AppRuntimeProfile;

#[test]
fn mainline_profile_requests_skia_selector_lock() {
    let profile = AppRuntimeProfile::mainline();

    assert_eq!(profile.forced_backend(), Some("winit"));
    assert_eq!(profile.forced_renderer(), Some("skia"));
    assert!(profile.prefers_direct3d());
}

#[test]
fn main_entrypoint_reads_packaged_profile_and_dynamic_renderer() {
    let content = fs::read_to_string("src/main.rs").expect("read main");

    assert!(content.contains("AppRuntimeProfile::packaged()"));
    assert!(content.contains("renderer_selection_attempts(profile)"));
    assert!(content.contains(".backend_name(backend_name.into())"));
    assert!(content.contains(".renderer_name(attempt.renderer_mode.renderer_name().into())"));
    assert!(content.contains("selector = selector.require_d3d();"));
    assert!(content.contains("renderer_fallback_chain()"));
    assert!(content.contains("preferred_graphics_api()"));
    assert!(content.contains("attempt.renderer_mode.renderer_name()"));
    assert!(content.contains("fallback_level"));
    assert!(!content.contains("renderer_name(\"software\".into())"));
}

#[test]
fn run_with_profile_accepts_external_async_handle_for_ssh_services() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("pub fn run_with_profile(\n    profile: AppRuntimeProfile,\n    async_runtime_handle: tokio::runtime::Handle,"),
        "run_with_profile should explicitly accept the async runtime handle used by shell services"
    );
    assert!(
        content.contains("let session_bridge = build_session_bridge(")
            && content.contains("async_runtime_handle.clone(),"),
        "run_with_profile should thread the supplied runtime handle into session bridge construction"
    );
    assert!(
        !content.contains("    _async_runtime_handle: tokio::runtime::Handle,"),
        "the runtime handle should be consumed for ssh services instead of being ignored"
    );
}

#[test]
fn build_win_x64_wrapper_defaults_packaged_windows_to_retained_native_surface() {
    let content = fs::read_to_string("build-win-x64.sh").expect("read build wrapper");

    assert!(
        content.contains("export MICA_TERM_PACKAGE_TERMINAL_SUBSYSTEM=\"scene-image\""),
        "build-win-x64.sh should pin packaged Windows mainline builds to the retained-native-surface terminal subsystem now that packaged Windows mainline defaults to the repaired child-HWND presenter path"
    );
    assert!(
        content.contains("packaged terminal subsystem: retained-native-surface"),
        "build-win-x64.sh help text should describe the retained-native-surface packaged default so packaging output matches the runtime path users should expect"
    );
}

#[test]
fn bootstrap_logs_terminal_subsystem_and_render_fallback_labels() {
    let content = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");

    assert!(
        content.contains("terminal_subsystem = profile.terminal_subsystem_mode_label()"),
        "bootstrap should include the selected terminal subsystem label in render-fallback diagnostics so packaged retained-native-surface and manual scene-image override bring-up logs are distinguishable"
    );
    assert!(
        content.contains("fallback_render_mode = TerminalRenderMode::Bitmap.as_str()"),
        "bootstrap should keep fallback render mode labels visible in diagnostics when a presenter downgrade happens"
    );
}

#[test]
fn native_surface_diagnostics_smoke_reports_child_host_relationship() {
    let bootstrap_source = fs::read_to_string("src/app/bootstrap.rs").expect("read bootstrap");
    let diagnostics_source =
        fs::read_to_string("src/app/terminal_renderer/diagnostics.rs").expect("read diagnostics");
    let windows_frame_source =
        fs::read_to_string("src/app/windows_frame.rs").expect("read windows frame");

    assert!(
        bootstrap_source.contains("window_scale_factor(window)"),
        "bootstrap should keep logging the live window scale factor so child-host geometry stays debuggable in physical pixels"
    );
    assert!(
        bootstrap_source.contains("host_hwnd = diagnostics.host_hwnd.unwrap_or_default()")
            && bootstrap_source
                .contains("surface_hwnd = diagnostics.surface_hwnd.unwrap_or_default()")
            && bootstrap_source
                .contains("surface_visible = diagnostics.surface_visible.unwrap_or(false)")
            && bootstrap_source
                .contains("render_target_ready = diagnostics.render_target_ready.unwrap_or(false)"),
        "bootstrap diagnostics should log the host/child HWND relationship plus child visibility and target readiness"
    );
    assert!(
        diagnostics_source.contains("pub host_hwnd: Option<isize>")
            && diagnostics_source.contains("pub surface_hwnd: Option<isize>"),
        "native surface diagnostics should expose dedicated host and child HWND fields"
    );
    assert!(
        windows_frame_source.contains("pub fn native_surface_host_hwnd(")
            && windows_frame_source.contains("pub fn native_surface_surface_hwnd("),
        "windows frame helpers should expose stable accessors for host and child HWND diagnostics"
    );
}

#[test]
fn terminal_completion_docs_state_current_core_and_migration_boundaries_honestly() {
    let audit_doc = fs::read_to_string(
        "docs/plans/2026-04-07-terminal-subsystem-completion-audit-and-corrective-design.md",
    )
    .expect("read completion audit design");

    assert!(
        audit_doc.contains("still drives the default terminal core"),
        "completion audit should state that WezTerm still drives the default core today"
    );
    assert!(
        audit_doc.contains("experimental adapter seam"),
        "completion audit should state that the Alacritty path is still experimental"
    );
    assert!(
        audit_doc.contains("architectural reference"),
        "completion audit should state that Rio is an architectural reference rather than migrated runtime code"
    );
}

#[test]
fn readme_states_current_terminal_core_and_reference_boundaries_honestly() {
    let readme = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        readme.contains("WezTerm-backed terminal core remains the shipped default today"),
        "readme should state that the shipped default core is still WezTerm-backed so the repo does not overclaim the migration status"
    );
    assert!(
        readme.contains("Alacritty adapter path is experimental"),
        "readme should state that the Alacritty path is still experimental"
    );
    assert!(
        readme.contains("Rio remains an architectural reference"),
        "readme should state that Rio is a reference rather than migrated runtime code"
    );
    assert!(
        readme.contains(
            "real `alacritty_terminal` core now exists behind the experimental adapter boundary"
        ),
        "readme should state that the experimental Alacritty path now binds to the real upstream core rather than staying a WezTerm wrapper seam"
    );
}

#[test]
fn readme_keeps_default_switch_blocked_on_packaged_verification_gate() {
    let readme = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        readme.contains("default switch remains gated on packaged Windows verification"),
        "readme should state that the terminal default switch is still gated instead of implying the migration is already complete"
    );
    assert!(
        readme.contains("fast scrollback perf"),
        "readme should call out fast scrollback perf as part of the packaged verification gate before any default switch"
    );
    assert!(
        readme.contains("typography sign-off"),
        "readme should call out typography sign-off as part of the packaged verification gate before any default switch"
    );
    assert!(
        readme.contains("Catppuccin palette sign-off"),
        "readme should call out Catppuccin palette sign-off as part of the packaged verification gate before any default switch"
    );
}

#[test]
fn terminal_renderer_readme_documents_runtime_fallback_diagnostics() {
    let content = fs::read_to_string("readme.md").expect("read readme");

    assert!(
        content.contains("selected terminal subsystem")
            && content.contains("requested render mode")
            && content.contains("active presenter mode")
            && content.contains("fallback transitions"),
        "readme should describe the runtime diagnostics emitted for terminal subsystem selection and presenter fallback transitions so packaged bring-up has an explicit debugging contract"
    );
}
